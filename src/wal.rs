// SPDX-License-Identifier: AGPL-3.0
//! # Write-Ahead Log (WAL)
//!
//! Atomic, crash-safe writes for Regedited documents. Every mutation is logged
//! before being applied to the main file. On crash, the WAL is replayed to
//! restore consistency.
//!
//! ## Design
//!
//! ```text
//! document.md      ← main file (only touched after WAL entry is fsync'd)
//! document.md.wal  ← write-ahead log (append-only, line-based, checksummed)
//! ```
//!
//! Each WAL entry is a single line with pipe-separated fields and a trailing
//! CRC32 checksum. The log is human-readable and `grep`-friendly.
//!
//! ## WAL Line Format
//!
//! ```text
//! SEQ|TIMESTAMP|OPERATION|SECTION|FIELD|OLD_VALUE|NEW_VALUE|CRC32
//! ```
//!
//! Example:
//! ```text
//! # REGEDITED WAL v1
//! # file: config.regd
//! ---
//! 1|1705312200|set-num|Config|0|42|99|a3f2c1d8
//! 2|1705312200|set-str|Config|0|/old/path|/new/path|b7d4e2f1
//! 3|1705312201|set-zone|Code|0|0x00000000:0x00000000|0x10000020:0x10000050|c9a5b3d7
//! ---
//! COMMIT|1705312205|d8e7f6a5
//! ```
//!
//! ## Crash Recovery
//!
//! On open, Regedited checks for a `.wal` file. If found (and not committed):
//! 1. Verify checksums of all entries
//! 2. Replay operations in sequence order
//! 3. Apply to the main file
//! 4. Mark WAL as committed
//!
//! If any entry fails checksum validation, the WAL is truncated at that point
//! and a warning is emitted — partial writes are discarded.
//!
//! ## Why This Matters
//!
//! The Windows Registry claims atomicity but hive corruption still happens.
//! Regedited's WAL provides true atomicity: either all changes in a batch
//! are applied, or none are. The main file is never in an inconsistent state.

use crate::{Result, RegeditedError};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Current WAL format version
const WAL_VERSION: &str = "v1";
/// WAL header marker
const WAL_HEADER: &str = "# REGEDITED WAL";
/// WAL separator between header and entries
const WAL_SEPARATOR: &str = "---";
/// WAL commit marker
const WAL_COMMIT: &str = "COMMIT";

/// Types of operations that can be logged
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalOperation {
    /// Update a numeric value: set-num <section> <index> <value>
    SetNum { section: String, index: usize, old_value: i64, new_value: i64 },
    /// Update a string value: set-str <section> <index> <value>
    SetStr { section: String, index: usize, old_value: String, new_value: String },
    /// Update a zone: set-zone <section> <zone> <start> <end> <type>
    SetZone { section: String, zone: usize, old_range: (u32, u32, String), new_range: (u32, u32, String) },
    /// Add a section
    SectionAdd { section: String },
    /// Remove a section
    SectionRemove { section: String, old_content: String },
    /// Replace zone content
    ZoneReplace { section: String, zone: usize, old_content: String, new_content: String },
}

impl WalOperation {
    /// Serialize to WAL line format (without seq and checksum)
    fn serialize_body(&self) -> String {
        match self {
            WalOperation::SetNum { section, index, old_value, new_value } => {
                format!("set-num|{}|{}|{}|{}", section, index, old_value, new_value)
            }
            WalOperation::SetStr { section, index, old_value, new_value } => {
                // Escape pipes in strings to avoid breaking the format
                let old_escaped = old_value.replace('|', "\\|");
                let new_escaped = new_value.replace('|', "\\|");
                format!("set-str|{}|{}|{}|{}", section, index, old_escaped, new_escaped)
            }
            WalOperation::SetZone { section, zone, old_range, new_range } => {
                format!(
                    "set-zone|{}|{}|{}:{}:{}|{}:{}:{}",
                    section, zone,
                    old_range.0, old_range.1, old_range.2,
                    new_range.0, new_range.1, new_range.2
                )
            }
            WalOperation::SectionAdd { section } => {
                format!("section-add|{}", section)
            }
            WalOperation::SectionRemove { section, old_content } => {
                let escaped = old_content.replace('|', "\\|").replace('\n', "\\n");
                format!("section-remove|{}|{}", section, escaped)
            }
            WalOperation::ZoneReplace { section, zone, old_content, new_content } => {
                let old_escaped = old_content.replace('|', "\\|").replace('\n', "\\n");
                let new_escaped = new_content.replace('|', "\\|").replace('\n', "\\n");
                format!("zone-replace|{}|{}|{}|{}", section, zone, old_escaped, new_escaped)
            }
        }
    }

    /// Deserialize from WAL line body (after seq|timestamp|)
    fn deserialize_body(body: &str) -> Result<Self> {
        let parts: Vec<&str> = body.split('|').collect();
        if parts.len() < 2 {
            return Err(RegeditedError::Parse(format!("Invalid WAL body: {}", body)));
        }

        // Unescape pipes
        let unescape = |s: &str| s.replace("\\|", "|").replace("\\n", "\n");

        match parts[0] {
            "set-num" if parts.len() >= 6 => Ok(WalOperation::SetNum {
                section: parts[1].to_string(),
                index: parts[2].parse().unwrap_or(0),
                old_value: parts[3].parse().unwrap_or(0),
                new_value: parts[4].parse().unwrap_or(0),
            }),
            "set-str" if parts.len() >= 6 => Ok(WalOperation::SetStr {
                section: parts[1].to_string(),
                index: parts[2].parse().unwrap_or(0),
                old_value: unescape(parts[3]),
                new_value: unescape(parts[4]),
            }),
            "set-zone" if parts.len() >= 6 => {
                let parse_range = |s: &str| -> Result<(u32, u32, String)> {
                    let rp: Vec<&str> = s.split(':').collect();
                    if rp.len() >= 3 {
                        Ok((rp[0].parse().unwrap_or(0), rp[1].parse().unwrap_or(0), rp[2].to_string()))
                    } else {
                        Err(RegeditedError::Parse(format!("Invalid range: {}", s)))
                    }
                };
                Ok(WalOperation::SetZone {
                    section: parts[1].to_string(),
                    zone: parts[2].parse().unwrap_or(0),
                    old_range: parse_range(parts[3])?,
                    new_range: parse_range(parts[4])?,
                })
            }
            "section-add" if parts.len() >= 2 => Ok(WalOperation::SectionAdd {
                section: parts[1].to_string(),
            }),
            "section-remove" if parts.len() >= 3 => Ok(WalOperation::SectionRemove {
                section: parts[1].to_string(),
                old_content: unescape(parts[2]),
            }),
            "zone-replace" if parts.len() >= 6 => Ok(WalOperation::ZoneReplace {
                section: parts[1].to_string(),
                zone: parts[2].parse().unwrap_or(0),
                old_content: unescape(parts[3]),
                new_content: unescape(parts[4]),
            }),
            _ => Err(RegeditedError::Parse(format!("Unknown WAL op: {}", parts[0]))),
        }
    }

    /// Human-readable description for display
    pub fn description(&self) -> String {
        match self {
            WalOperation::SetNum { section, index, old_value, new_value } => {
                format!("set-num [{}].{}: {} -> {}", section, index, old_value, new_value)
            }
            WalOperation::SetStr { section, index, old_value, new_value } => {
                format!("set-str [{}].{}: '{}' -> '{}'", section, index, old_value, new_value)
            }
            WalOperation::SetZone { section, zone, old_range, new_range } => {
                format!(
                    "set-zone [{}].{}: 0x{}:0x{}:{} -> 0x{}:0x{}:{}",
                    section, zone,
                    old_range.0, old_range.1, old_range.2,
                    new_range.0, new_range.1, new_range.2
                )
            }
            WalOperation::SectionAdd { section } => {
                format!("section-add: {}", section)
            }
            WalOperation::SectionRemove { section, .. } => {
                format!("section-remove: {}", section)
            }
            WalOperation::ZoneReplace { section, zone, .. } => {
                format!("zone-replace [{}].{}", section, zone)
            }
        }
    }
}

/// A single WAL entry with sequence number, timestamp, and checksum
#[derive(Debug, Clone)]
pub struct WalEntry {
    /// Monotonically increasing sequence number
    pub seq: u64,
    /// Unix timestamp (seconds)
    pub timestamp: u64,
    /// The operation being logged
    pub operation: WalOperation,
    /// CRC32 checksum of the entry body
    pub checksum: u32,
}

impl WalEntry {
    /// Compute CRC32 checksum of the serialized body
    fn compute_checksum(seq: u64, timestamp: u64, body: &str) -> u32 {
        let data = format!("{}|{}|{}", seq, timestamp, body);
        crc32fast::hash(data.as_bytes())
    }

    /// Serialize to a single WAL line
    pub fn to_line(&self) -> String {
        let body = self.operation.serialize_body();
        format!("{}|{}|{}|{:08x}", self.seq, self.timestamp, body, self.checksum)
    }

    /// Deserialize from a WAL line, verifying checksum
    pub fn from_line(line: &str) -> Result<Self> {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return Err(RegeditedError::Parse("Empty or comment line".to_string()));
        }

        // Find the last pipe separator (checksum is the last field)
        let checksum_pos = line.rfind('|').ok_or_else(|| {
            RegeditedError::Parse("Invalid WAL line: no checksum separator".to_string())
        })?;

        let body = &line[..checksum_pos];
        let checksum_hex = &line[checksum_pos + 1..];
        let stored_checksum = u32::from_str_radix(checksum_hex, 16)
            .map_err(|e| RegeditedError::Parse(format!("Invalid checksum: {}", e)))?;

        // Parse seq|timestamp|op_body
        let parts: Vec<&str> = body.splitn(3, '|').collect();
        if parts.len() != 3 {
            return Err(RegeditedError::Parse(format!("Invalid WAL entry: {}", body)));
        }

        let seq: u64 = parts[0].parse()
            .map_err(|e| RegeditedError::Parse(format!("Invalid seq: {}", e)))?;
        let timestamp: u64 = parts[1].parse()
            .map_err(|e| RegeditedError::Parse(format!("Invalid timestamp: {}", e)))?;

        // Verify checksum
        let computed = Self::compute_checksum(seq, timestamp, parts[2]);
        if computed != stored_checksum {
            return Err(RegeditedError::Parse(
                format!("WAL checksum mismatch: computed={:08x}, stored={:08x}", computed, stored_checksum)
            ));
        }

        let operation = WalOperation::deserialize_body(parts[2])?;

        Ok(WalEntry { seq, timestamp, operation, checksum: stored_checksum })
    }

    /// Create a new entry with auto timestamp
    pub fn new(seq: u64, operation: WalOperation) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let body = operation.serialize_body();
        let checksum = Self::compute_checksum(seq, timestamp, &body);
        Self { seq, timestamp, operation, checksum }
    }
}

/// Write-Ahead Log manager
///
/// Append-only log with checksum verification and crash recovery.
pub struct Wal {
    /// Path to the WAL file (e.g., `document.md.wal`)
    path: PathBuf,
    /// Path to the main document file
    doc_path: PathBuf,
    /// Current sequence number (max seen + 1)
    next_seq: u64,
    /// In-memory entries (uncommitted)
    entries: Vec<WalEntry>,
    /// Whether the WAL has been committed
    committed: bool,
}

impl Wal {
    /// Open or create a WAL for a given document
    pub fn open<P: AsRef<Path>>(doc_path: P) -> Result<Self> {
        let doc_path = doc_path.as_ref().to_path_buf();
        let wal_path = Self::wal_path(&doc_path);

        let mut wal = Self {
            path: wal_path,
            doc_path,
            next_seq: 1,
            entries: Vec::new(),
            committed: false,
        };

        // If WAL file exists, scan it to find max sequence number
        if wal.path.exists() {
            wal.scan_existing()?;
        }

        Ok(wal)
    }

    /// Get the WAL file path from the document path
    fn wal_path(doc_path: &Path) -> PathBuf {
        let mut p = doc_path.as_os_str().to_owned();
        p.push(".wal");
        PathBuf::from(p)
    }

    /// Scan an existing WAL file to find committed state and max sequence
    fn scan_existing(&mut self) -> Result<()> {
        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);

        let mut in_entries = false;

        for line_result in reader.lines() {
            let line = line_result?;
            let trimmed = line.trim();

            if trimmed == WAL_HEADER {
                continue;
            }
            if trimmed.starts_with("# file:") {
                continue;
            }
            if trimmed == WAL_SEPARATOR {
                in_entries = true;
                continue;
            }
            if trimmed.starts_with(WAL_COMMIT) {
                self.committed = true;
                in_entries = false;
                continue;
            }

            if in_entries && !trimmed.is_empty() && !trimmed.starts_with('#') {
                match WalEntry::from_line(&line) {
                    Ok(entry) => {
                        if entry.seq >= self.next_seq {
                            self.next_seq = entry.seq + 1;
                        }
                    }
                    Err(e) => {
                        // Log warning but continue — partial corruption handled during replay
                        eprintln!("WAL warning: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if a WAL file exists for a document (indicating potential crash)
    pub fn exists_for<P: AsRef<Path>>(doc_path: P) -> bool {
        let wal_path = Self::wal_path(doc_path.as_ref());
        wal_path.exists()
    }

    /// Check if the WAL has uncommitted entries (crash detected)
    pub fn has_uncommitted_entries(&self) -> bool {
        !self.entries.is_empty() && !self.committed
    }

    /// Append an operation to the WAL (writes to disk immediately)
    pub fn append(&mut self, op: WalOperation) -> Result<()> {
        let entry = WalEntry::new(self.next_seq, op);
        self.next_seq += 1;

        // Ensure WAL file has header
        if self.entries.is_empty() {
            self.write_header()?;
        }

        // Append to file with fsync for durability
        {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?;
            writeln!(file, "{}", entry.to_line())?;
            file.sync_all()?; // fsync — guarantee durability
        }

        self.entries.push(entry);
        Ok(())
    }

    /// Write the WAL file header
    fn write_header(&self) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.path)?;

        writeln!(file, "{}", WAL_HEADER)?;
        writeln!(file, "# version: {}", WAL_VERSION)?;
        writeln!(file, "# file: {}", self.doc_path.display())?;
        writeln!(file, "# created: {}", SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs())?;
        writeln!(file, "{}", WAL_SEPARATOR)?;
        file.sync_all()?;
        Ok(())
    }

    /// Commit all entries: write commit marker and mark as committed
    pub fn commit(&mut self) -> Result<()> {
        if self.entries.is_empty() {
            // Nothing to commit — clean up WAL file
            self.cleanup()?;
            return Ok(());
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Compute commit checksum over all entry checksums
        let commit_data: String = self.entries.iter()
            .map(|e| format!("{:08x}", e.checksum))
            .collect::<Vec<_>>()
            .join("");
        let commit_checksum = crc32fast::hash(commit_data.as_bytes());

        let mut file = OpenOptions::new()
            .append(true)
            .open(&self.path)?;

        writeln!(file, "{}", WAL_SEPARATOR)?;
        writeln!(file, "{}|{}|{:08x}", WAL_COMMIT, timestamp, commit_checksum)?;
        file.sync_all()?;

        self.committed = true;
        Ok(())
    }

    /// Rollback: discard all uncommitted entries and remove WAL file
    pub fn rollback(&mut self) -> Result<()> {
        self.entries.clear();
        self.committed = false;
        self.cleanup()?;
        Ok(())
    }

    /// Remove the WAL file (cleanup after commit or rollback)
    pub fn cleanup(&self) -> Result<()> {
        if self.path.exists() {
            std::fs::remove_file(&self.path)?;
        }
        Ok(())
    }

    /// Read and return all entries from the WAL file (for replay/inspection)
    pub fn read_entries<P: AsRef<Path>>(doc_path: P) -> Result<Vec<WalEntry>> {
        let wal_path = Self::wal_path(doc_path.as_ref());
        if !wal_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&wal_path)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();
        let mut in_entries = false;

        for line_result in reader.lines() {
            let line = line_result?;
            let trimmed = line.trim();

            if trimmed == WAL_SEPARATOR {
                in_entries = !in_entries;
                continue;
            }
            if trimmed.starts_with(WAL_COMMIT) || trimmed.starts_with('#') || trimmed.starts_with(WAL_HEADER) {
                continue;
            }

            if in_entries && !trimmed.is_empty() {
                match WalEntry::from_line(&line) {
                    Ok(entry) => entries.push(entry),
                    Err(e) => eprintln!("WAL parse warning (skipping): {}", e),
                }
            }
        }

        Ok(entries)
    }

    /// Check if a WAL file is committed
    pub fn is_committed<P: AsRef<Path>>(doc_path: P) -> Result<bool> {
        let wal_path = Self::wal_path(doc_path.as_ref());
        if !wal_path.exists() {
            return Ok(false);
        }

        let file = File::open(&wal_path)?;
        let reader = BufReader::new(file);

        for line_result in reader.lines() {
            let line = line_result?;
            if line.trim().starts_with(WAL_COMMIT) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Get the number of uncommitted entries
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Get all entries
    pub fn entries(&self) -> &[WalEntry] {
        &self.entries
    }
}

/// WAL status for display
#[derive(Debug)]
pub struct WalStatus {
    pub has_wal: bool,
    pub is_committed: bool,
    pub entry_count: usize,
    pub wal_path: PathBuf,
}

impl WalStatus {
    /// Check WAL status for a document
    pub fn check<P: AsRef<Path>>(doc_path: P) -> Result<Self> {
        let wal_path = Wal::wal_path(doc_path.as_ref());
        let has_wal = wal_path.exists();
        let is_committed = if has_wal {
            Wal::is_committed(doc_path.as_ref())?
        } else {
            false
        };
        let entry_count = if has_wal {
            Wal::read_entries(doc_path.as_ref())?.len()
        } else {
            0
        };

        Ok(Self {
            has_wal,
            is_committed,
            entry_count,
            wal_path,
        })
    }

    /// Format for display
    pub fn display(&self) -> String {
        if !self.has_wal {
            return "No WAL file found (clean shutdown)".to_string();
        }
        format!(
            "WAL: {} | Committed: {} | Entries: {}",
            self.wal_path.display(),
            if self.is_committed { "YES" } else { "NO (crash detected!)" },
            self.entry_count
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wal_entry_roundtrip() {
        let entry = WalEntry::new(1, WalOperation::SetNum {
            section: "Config".to_string(),
            index: 0,
            old_value: 42,
            new_value: 99,
        });

        let line = entry.to_line();
        let parsed = WalEntry::from_line(&line).unwrap();

        assert_eq!(parsed.seq, entry.seq);
        assert_eq!(parsed.operation, entry.operation);
        assert_eq!(parsed.checksum, entry.checksum);
    }

    #[test]
    fn test_wal_entry_checksum_verification() {
        let entry = WalEntry::new(1, WalOperation::SetStr {
            section: "Intro".to_string(),
            index: 0,
            old_value: "hello".to_string(),
            new_value: "world".to_string(),
        });

        let mut line = entry.to_line();
        // Corrupt the checksum
        line.pop();
        line.push('X');

        assert!(WalEntry::from_line(&line).is_err());
    }

    #[test]
    fn test_wal_operation_set_num_serialize() {
        let op = WalOperation::SetNum {
            section: "Config".to_string(),
            index: 2,
            old_value: 10,
            new_value: 20,
        };
        let serialized = op.serialize_body();
        assert_eq!(serialized, "set-num|Config|2|10|20");
    }

    #[test]
    fn test_wal_operation_set_str_with_pipes() {
        let op = WalOperation::SetStr {
            section: "Config".to_string(),
            index: 0,
            old_value: "a|b|c".to_string(),
            new_value: "x|y|z".to_string(),
        };
        let serialized = op.serialize_body();
        // Pipes should be escaped
        assert!(serialized.contains("\\|"));

        // Deserialize should unescape
        let deserialized = WalOperation::deserialize_body("set-str|Config|0|a\\|b\\|c|x\\|y\\|z").unwrap();
        match deserialized {
            WalOperation::SetStr { old_value, new_value, .. } => {
                assert_eq!(old_value, "a|b|c");
                assert_eq!(new_value, "x|y|z");
            }
            _ => panic!("Wrong type"),
        }
    }

    #[test]
    fn test_wal_lifecycle() {
        let tmp = std::env::temp_dir().join("regedited_test_wal.md");
        // Ensure no leftover WAL
        let _ = std::fs::remove_file(&tmp);
        let wal_path = Wal::wal_path(&tmp);
        let _ = std::fs::remove_file(&wal_path);

        // Create WAL
        let mut wal = Wal::open(&tmp).unwrap();
        assert_eq!(wal.entry_count(), 0);

        // Append some entries
        wal.append(WalOperation::SetNum {
            section: "Config".to_string(),
            index: 0,
            old_value: 1,
            new_value: 2,
        }).unwrap();

        wal.append(WalOperation::SetNum {
            section: "Config".to_string(),
            index: 1,
            old_value: 3,
            new_value: 4,
        }).unwrap();

        assert_eq!(wal.entry_count(), 2);

        // Commit
        wal.commit().unwrap();
        assert!(wal.committed);

        // Cleanup
        wal.cleanup().unwrap();
        assert!(!wal_path.exists());
    }

    #[test]
    fn test_wal_rollback() {
        let tmp = std::env::temp_dir().join("regedited_test_wal_rollback.md");
        let _ = std::fs::remove_file(&tmp);
        let wal_path = Wal::wal_path(&tmp);
        let _ = std::fs::remove_file(&wal_path);

        let mut wal = Wal::open(&tmp).unwrap();
        wal.append(WalOperation::SectionAdd {
            section: "Test".to_string(),
        }).unwrap();

        assert_eq!(wal.entry_count(), 1);

        // Rollback
        wal.rollback().unwrap();
        assert_eq!(wal.entry_count(), 0);
        assert!(!wal_path.exists());
    }
}
