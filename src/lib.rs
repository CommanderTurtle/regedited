// SPDX-License-Identifier: AGPL-3.0
//! # Regedited - Fast Plaintext Parse-Ment Database
//!
//! A high-performance plaintext database system that uses structured markdown headers
//! with offset/length metadata for instant seeking — no full-file parsing required.
//!
//! Inspired by the safetensors format's ability to quickly update keys in multi-GB files,
//! Regedited brings that same philosophy to structured plaintext documents.
//!
//! ## Core Concepts
//!
//! - **Structured Headers**: Each section has a header with byte offsets and line ranges,
//!   allowing O(1) jumps to any section.
//! - **Database Lines**: Structured tabular data with 9 fields (6 numeric values, 3 strings)
//!   that define content zones and metadata.
//! - **Zero-Copy Parsing**: Uses memory-mapped I/O to read files without loading them into RAM.
//! - **Fast Updates**: Rewrite only changed sections, not the entire file.
//! - **Zone Extraction**: Grep-like functionality using line ranges stored in database fields.
//!
//! ## Example File Format
//!
//! ```markdown
//! # Regedited Document v1
//! <!--DB:INDEX|offset=0|length=245|checksum=a3f2-->
//!
//! ## SECTION:Config
//! <!--DB:HEADER|section=Config|db_line=5|content_start=7|content_end=50-->
//! 1	100	200	50	60	70	"settings"	"path"	"notes"
//!
//! (content lines 7-50...)
//!
//! ## SECTION:Data
//! <!--DB:HEADER|section=Data|db_line=52|content_start=54|content_end=120-->
//! 51	300	400	100	110	120	"records"	"format"	"backup"
//! ```

use std::path::Path;

pub mod ascii_store;
pub mod bool_ops;
pub mod clip;
pub mod db_line;
pub mod echo;
pub mod encapsulate;
pub mod fast_ops;
pub mod header;
pub mod html_extract;
pub mod schema;
pub mod serve;
pub mod store;
pub mod transaction;
pub mod typed_value;
pub mod utf16;
pub mod wal;
pub mod zone;
pub mod zone_editor;
pub mod zone_type;

/// Re-export commonly used types
pub use ascii_store::{AsciiStore, ZonePair};
pub use bool_ops::{bool_and, bool_nand, bool_or, bool_xor, count, if_contains, BoolResult};
pub use db_line::{DbLine, SectionData};
pub use encapsulate::{
    convert_mode, detect_mode, encapsulate, extract, format_set_command, EncapMode,
};
pub use fast_ops::{
    fast_diff, fast_grep, fast_grep_multi, fast_grep_section, fast_replace, fast_scan,
};
pub use header::{DocumentHeader, SectionInfo};
pub use html_extract::{
    extract_attributes, format_as_set_vars, format_numbered, index_to_suffix, HtmlExtract,
};
pub use schema::{DocumentSchema, FieldConstraint, SchemaField, SchemaFieldType, SectionSchema};
pub use serve::{serve, ServeConfig};
pub use store::{Store, StoreConfig};
pub use transaction::{StagedOperation, Transaction, TransactionManager, TransactionState};
pub use typed_value::{list_registry_types, TypedValue};
pub use utf16::{getutf, getutf_decode};
pub use wal::{Wal, WalEntry, WalOperation, WalStatus};
pub use zone::{Zone, ZoneExtractor};
pub use zone_editor::{
    append_zone_content, copy_zone_content, extract_zone_content, format_zone_info,
    replace_zone_content, swap_zone_content,
};
pub use zone_type::{
    build_ascii_line, decode_hex_word, encode_hex_word, parse_hex_word_line, ZoneType,
};

/// Custom error type for Regedited operations
#[derive(Debug, thiserror::Error)]
pub enum RegeditedError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Section not found: {0}")]
    SectionNotFound(String),

    #[error("Invalid database line format: {0}")]
    InvalidDbLine(String),

    #[error("Header corruption detected: {0}")]
    HeaderCorruption(String),

    #[error("Zone out of bounds: line {line}, file has {max_lines} lines")]
    ZoneOutOfBounds { line: usize, max_lines: usize },

    #[error("Clipboard error: {0}")]
    Clipboard(String),

    #[error("Echo encoding error: {0}")]
    EchoEncoding(String),
}

/// Result type alias for Regedited operations
pub type Result<T> = std::result::Result<T, RegeditedError>;

/// Memory-mapped file handle for zero-copy access
pub struct MmapFile {
    mmap: memmap2::Mmap,
    path: std::path::PathBuf,
}

impl MmapFile {
    /// Open and memory-map a file
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = std::fs::File::open(&path)?;
        let mmap = unsafe { memmap2::Mmap::map(&file)? };
        Ok(Self {
            mmap,
            path: path.as_ref().to_path_buf(),
        })
    }

    /// Get the memory-mapped content as a byte slice
    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8] {
        &self.mmap
    }

    /// Get the memory-mapped content as a string slice
    ///
    /// # Panics
    /// Panics if the file is not valid UTF-8
    #[inline(always)]
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(self.as_bytes()).expect("File is not valid UTF-8")
    }

    /// Get file path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get file size in bytes
    pub fn len(&self) -> usize {
        self.mmap.len()
    }

    /// Check if file is empty
    pub fn is_empty(&self) -> bool {
        self.mmap.is_empty()
    }
}

/// Fast byte-level scanner for finding patterns in memory-mapped files
pub struct ByteScanner<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> ByteScanner<'a> {
    /// Create a new byte scanner
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    /// Find the next occurrence of a byte pattern
    pub fn find_next(&mut self, pattern: &[u8]) -> Option<usize> {
        if pattern.is_empty() || self.position >= self.data.len() {
            return None;
        }

        let search_slice = &self.data[self.position..];

        // Use memchr for single-byte patterns (fastest)
        if pattern.len() == 1 {
            if let Some(pos) = search_slice.iter().position(|&b| b == pattern[0]) {
                let absolute_pos = self.position + pos;
                self.position = absolute_pos + 1;
                return Some(absolute_pos);
            }
            return None;
        }

        // For multi-byte patterns, use naive search (good for small patterns)
        // TODO: Could use Boyer-Moore or KMP for larger patterns
        if search_slice.len() >= pattern.len() {
            for i in 0..=search_slice.len() - pattern.len() {
                if &search_slice[i..i + pattern.len()] == pattern {
                    let absolute_pos = self.position + i;
                    self.position = absolute_pos + pattern.len();
                    return Some(absolute_pos);
                }
            }
        }

        None
    }

    /// Find the nth line in the file (0-indexed)
    /// Returns the byte offset of the start of that line
    pub fn find_line(&mut self, n: usize) -> Option<usize> {
        if n == 0 {
            return Some(0);
        }

        let mut line_count = 0;
        self.position = 0;

        while let Some(_pos) = self.find_next(b"\n") {
            line_count += 1;
            if line_count == n {
                // Position is now after the newline, which is the start of the next line
                if self.position <= self.data.len() {
                    return Some(self.position);
                }
                return None;
            }
        }

        None
    }

    /// Get the current position
    pub fn position(&self) -> usize {
        self.position
    }

    /// Reset scanner to beginning
    pub fn reset(&mut self) {
        self.position = 0;
    }

    /// Get a slice from the current position
    pub fn slice_from(&self, len: usize) -> Option<&'a [u8]> {
        self.data.get(self.position..self.position + len)
    }

    /// Read until a delimiter byte
    pub fn read_until(&mut self, delimiter: u8) -> Option<&'a [u8]> {
        let start = self.position;
        if let Some(pos) = self.data[self.position..]
            .iter()
            .position(|&b| b == delimiter)
        {
            let end = self.position + pos;
            self.position = end + 1; // Skip delimiter
            Some(&self.data[start..end])
        } else if self.position < self.data.len() {
            // Return rest of data if no delimiter found
            let result = &self.data[start..];
            self.position = self.data.len();
            Some(result)
        } else {
            None
        }
    }
}

/// Calculate a simple checksum for integrity verification
pub fn checksum(data: &[u8]) -> u32 {
    use std::hash::Hasher;
    let mut hasher = fxhash::FxHasher32::default();
    hasher.write(data);
    hasher.finish() as u32
}

/// Format a checksum as a hex string
pub fn checksum_hex(data: &[u8]) -> String {
    format!("{:08x}", checksum(data))
}

/// Find line offsets in a byte slice
/// Returns a Vec of (line_number, byte_offset) for each line
pub fn find_line_offsets(data: &[u8]) -> Vec<(usize, usize)> {
    let mut offsets = vec![(0, 0)];
    let mut line_num = 1;

    for (i, &byte) in data.iter().enumerate() {
        if byte == b'\n' {
            let next_offset = i + 1;
            if next_offset < data.len() {
                offsets.push((line_num, next_offset));
                line_num += 1;
            }
        }
    }

    offsets
}

/// Extract a range of lines from data given start and end line numbers (inclusive, 0-indexed)
pub fn extract_lines(data: &[u8], start_line: usize, end_line: usize) -> Result<Vec<u8>> {
    let offsets = find_line_offsets(data);

    if start_line >= offsets.len() {
        return Err(RegeditedError::ZoneOutOfBounds {
            line: start_line,
            max_lines: offsets.len(),
        });
    }

    let start_offset = offsets[start_line].1;

    // Find end offset
    let end_offset = if end_line + 1 < offsets.len() {
        offsets[end_line + 1].1
    } else {
        data.len()
    };

    Ok(data[start_offset..end_offset].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum() {
        let data = b"hello world";
        let cs = checksum(data);
        assert_ne!(cs, 0);

        // Same data should produce same checksum
        let cs2 = checksum(data);
        assert_eq!(cs, cs2);

        // Different data should likely produce different checksum
        let cs3 = checksum(b"different");
        assert_ne!(cs, cs3);
    }

    #[test]
    fn test_checksum_hex() {
        let cs = checksum_hex(b"test");
        assert_eq!(cs.len(), 8); // 8 hex characters for u32
    }

    #[test]
    fn test_find_line_offsets() {
        let data = b"line1\nline2\nline3\n";
        let offsets = find_line_offsets(data);
        assert_eq!(offsets.len(), 3);
        assert_eq!(offsets[0], (0, 0));
        assert_eq!(offsets[1], (1, 6)); // After "line1\n"
        assert_eq!(offsets[2], (2, 12)); // After "line2\n"
    }

    #[test]
    fn test_extract_lines() {
        let data = b"line0\nline1\nline2\nline3\nline4\n";
        let extracted = extract_lines(data, 1, 3).unwrap();
        assert_eq!(extracted, b"line1\nline2\nline3\n");
    }

    #[test]
    fn test_byte_scanner() {
        let data = b"hello\nworld\ntest\n";
        let mut scanner = ByteScanner::new(data);

        // Find first newline
        let pos = scanner.find_next(b"\n");
        assert_eq!(pos, Some(5));

        // Find next newline
        let pos2 = scanner.find_next(b"\n");
        assert_eq!(pos2, Some(11));

        // Find a line
        scanner.reset();
        let line_pos = scanner.find_line(2);
        assert_eq!(line_pos, Some(12)); // Start of "test\n"
    }

    #[test]
    fn test_mmap_file() {
        // Create a temp file
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("regedited_test_mmap.txt");
        std::fs::write(&test_file, b"test content for mmap").unwrap();

        let mmap = MmapFile::open(&test_file).unwrap();
        assert_eq!(mmap.as_bytes(), b"test content for mmap");
        assert_eq!(mmap.as_str(), "test content for mmap");

        // Cleanup
        std::fs::remove_file(test_file).unwrap();
    }
}
