// SPDX-License-Identifier: AGPL-3.0
//! # Store
//!
//! The main high-level API for Regedited operations. Provides methods for
//! reading, writing, and manipulating indexed UTF-8 documents.
//!
//! ## Core Operations
//!
//! - **Read**: Parse index records, extract data blocks, view database tables
//! - **Write**: Update hex-word lines, numeric values, strings
//! - **Zones**: Extract content by absolute line ranges
//! - **Clipboard**: Copy strings to system clipboard
//!
//! ## Example
//!
//! ```rust,ignore
//! use regedited::store::{Store, StoreConfig};
//!
//! let store = Store::open("myfile.md").unwrap();
//!
//! // List all fixed index records (legacy method name retained)
//! let indexes = store.list_sections();
//!
//! // View an index's database table
//! let table = store.get_db_table("i64").unwrap();
//! println!("{}", table);
//!
//! // Extract an absolute zone
//! let zone = store.get_zone("i64", 0).unwrap();
//! println!("{}", zone.content());
//! ```

use crate::{
    ascii_store::AsciiStore,
    clip,
    db_line::{DbLine, DecimalValue, SectionData},
    echo::{safe_echo, EchoResult},
    header::{
        extract_section_content, extract_section_data, scan_content, update_section_data,
        DocumentHeader, SectionInfo,
    },
    zone::{extract_zone, Zone},
    RegeditedError, Result,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Configuration for the store
#[derive(Debug, Clone)]
pub struct StoreConfig {
    /// Auto-save after each write operation
    pub auto_save: bool,
    /// Create backup files before writes
    pub create_backups: bool,
    /// Verbose output
    pub verbose: bool,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            auto_save: true,
            create_backups: true,
            verbose: false,
        }
    }
}

/// The main Store struct - primary interface for Regedited
pub struct Store {
    /// File path
    path: PathBuf,
    /// File content (kept in memory for fast access)
    content: String,
    /// Parsed document header (section index)
    header: DocumentHeader,
    /// Cached section data (Hex-word line + DbLine)
    section_cache: BTreeMap<String, SectionData>,
    /// Configuration
    config: StoreConfig,
    /// Dirty flag (content has been modified)
    dirty: bool,
}

impl Store {
    // ==================== CONSTRUCTION ====================

    /// Open a markdown file and parse its structure
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let content = std::fs::read_to_string(&path)?;
        let header = scan_content(&content)?;

        Ok(Self {
            path,
            content,
            header,
            section_cache: BTreeMap::new(),
            config: StoreConfig::default(),
            dirty: false,
        })
    }

    /// Open with custom configuration
    pub fn open_with_config<P: AsRef<Path>>(path: P, config: StoreConfig) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let content = std::fs::read_to_string(&path)?;
        let header = scan_content(&content)?;

        Ok(Self {
            path,
            content,
            header,
            section_cache: BTreeMap::new(),
            config,
            dirty: false,
        })
    }

    /// Create a new store with an empty document
    pub fn create<P: AsRef<Path>>(path: P, title: &str) -> Result<Self> {
        let content = format!("# {}\n\n", title);
        std::fs::write(&path, &content)?;

        let header = scan_content(&content)?;

        Ok(Self {
            path: path.as_ref().to_path_buf(),
            content,
            header,
            section_cache: BTreeMap::new(),
            config: StoreConfig::default(),
            dirty: false,
        })
    }

    // ==================== ACCESSORS ====================

    /// Get the file path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get configuration
    pub fn config(&self) -> &StoreConfig {
        &self.config
    }

    /// Get the document header
    pub fn header(&self) -> &DocumentHeader {
        &self.header
    }

    /// Check if content has been modified
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    // ==================== INDEX OPERATIONS ====================

    /// List all internal index keys.
    pub fn list_sections(&self) -> Vec<&str> {
        self.header.section_names()
    }

    /// Get index info by numeric reference, with legacy internal keys as fallback.
    pub fn get_section(&self, reference: &str) -> Result<&SectionInfo> {
        self.header.resolve_section(reference)
    }

    /// Check if an index exists.
    pub fn has_section(&self, name: &str) -> bool {
        self.get_section(name).is_ok()
    }

    /// Compatibility spelling for adding a canonical numeric index.
    pub fn add_section(&mut self, reference: &str) -> Result<()> {
        let registry_index = crate::header::parse_index_reference(reference).ok_or_else(|| {
            RegeditedError::Parse(
                "Indexes must use a numeric reference such as 64, i64, or index:64".to_string(),
            )
        })?;
        self.add_index(registry_index)
    }

    /// Add a canonical `regedited open` index without a section-name identity.
    pub fn add_index(&mut self, registry_index: u64) -> Result<()> {
        if self
            .header
            .resolve_section(&registry_index.to_string())
            .is_ok()
        {
            return Err(RegeditedError::Parse(format!(
                "Index {} already exists",
                registry_index
            )));
        }

        let blank_ascii = crate::ascii_store::blank_ascii_store();
        let index_text = format!(
            "\nregedited open\nindex: {0}\n{1}\n0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0\n\n\n\n",
            registry_index, blank_ascii
        );
        self.content.push_str(&index_text);
        self.dirty = true;
        self.header = scan_content(&self.content)?;
        self.section_cache.clear();

        if self.config.auto_save {
            self.save()?;
        }
        Ok(())
    }

    /// Remove only the marker and its six structured lines.
    pub fn remove_section(&mut self, name: &str) -> Result<()> {
        let section = self.get_section(name)?.clone();

        let lines: Vec<&str> = self.content.lines().collect();
        let start = section.header_line;
        let end = section.string3_line + 1;

        if end > lines.len() {
            return Err(RegeditedError::ZoneOutOfBounds {
                line: end,
                max_lines: lines.len(),
            });
        }

        // Keep lines before and after the section
        let mut new_lines: Vec<&str> = Vec::new();
        new_lines.extend_from_slice(&lines[..start]);
        if end < lines.len() {
            new_lines.extend_from_slice(&lines[end..]);
        }

        self.content = new_lines.join("\n");
        self.content = crate::zone_editor::apply_line_deltas(
            &self.content,
            &[crate::zone_editor::LineDelta {
                start_line: end,
                delta: -((end - start) as i64),
            }],
        )?;
        self.dirty = true;
        self.header = scan_content(&self.content)?;
        self.section_cache.remove(name);

        if self.config.auto_save {
            self.save()?;
        }

        Ok(())
    }

    // ==================== DATA READING ====================

    /// Get an index record's data (hex-word line + DbLine).
    fn get_section_data(&mut self, name: &str) -> Result<SectionData> {
        let resolved_name = self.get_section(name)?.name.clone();
        // Check cache first
        if let Some(data) = self.section_cache.get(&resolved_name) {
            return Ok(data.clone());
        }

        let section = self.get_section(&resolved_name)?.clone();
        let data_str = extract_section_data(&self.content, &section)?;
        let lines: Vec<&str> = data_str.lines().collect();
        let data = SectionData::from_lines(&lines)?;

        // Cache it
        self.section_cache.insert(resolved_name, data.clone());

        Ok(data)
    }

    /// Get an index record's hex-word line.
    pub fn get_ascii_store(&mut self, name: &str) -> Result<AsciiStore> {
        let data = self.get_section_data(name)?;
        Ok(data.ascii_store)
    }

    /// Get an index record's database line.
    pub fn get_db_line(&mut self, name: &str) -> Result<DbLine> {
        let data = self.get_section_data(name)?;
        Ok(data.db_line)
    }

    /// Get an index record's database table as Markdown.
    pub fn get_db_table(&mut self, name: &str) -> Result<String> {
        let data = self.get_section_data(name)?;
        Ok(data.db_line.to_markdown_table())
    }

    /// Get the complete document after validating an index reference.
    pub fn get_section_content(&self, name: &str) -> Result<String> {
        let section = self.get_section(name)?.clone();
        extract_section_content(&self.content, &section)
    }

    /// Get a specific numeric value from an index record.
    pub fn get_number(&mut self, section_name: &str, index: usize) -> Result<DecimalValue> {
        let db = self.get_db_line(section_name)?;
        db.get_number(index)
            .cloned()
            .ok_or_else(|| RegeditedError::Parse(format!("Number index {index} out of range")))
    }

    /// Get a specific string from an index record.
    pub fn get_string(&mut self, section_name: &str, index: usize) -> Result<String> {
        let db = self.get_db_line(section_name)?;
        db.get_string(index)
            .map(|s| s.to_string())
            .ok_or_else(|| RegeditedError::Parse(format!("String index {index} out of range")))
    }

    /// Resolve a zone through an index record.
    pub fn get_zone(&mut self, section_name: &str, zone_index: usize) -> Result<Zone> {
        let section = self.get_section(section_name)?.clone();
        let data = self.get_section_data(section_name)?;
        let label = data
            .db_line
            .get_string(zone_index)
            .unwrap_or("")
            .to_string();
        extract_zone(&self.content, &section, zone_index, &label)
    }

    // ==================== DATA WRITING ====================

    /// Write an index record's six structured lines back to the document.
    fn write_section_data(&mut self, name: &str, data: &SectionData) -> Result<()> {
        let section = self.get_section(name)?.clone();
        let resolved_name = section.name.clone();
        let new_data = data.to_lines();
        self.content = update_section_data(&self.content, &section, &new_data)?;
        self.dirty = true;

        // Update cache
        self.section_cache.insert(resolved_name, data.clone());

        if self.config.auto_save {
            self.save()?;
        }

        Ok(())
    }

    /// Update the entire hex-word line for an index record.
    pub fn update_ascii_store(&mut self, name: &str, ascii: AsciiStore) -> Result<()> {
        let mut data = self.get_section_data(name)?;
        data.ascii_store = ascii;
        self.write_section_data(name, &data)
    }

    /// Update a zone in the Hex-word line (with type)
    pub fn update_zone(
        &mut self,
        name: &str,
        zone_index: usize,
        start: u32,
        end: u32,
        zone_type: crate::zone_type::ZoneType,
    ) -> Result<()> {
        let mut data = self.get_section_data(name)?;
        data.ascii_store
            .set_zone(zone_index, start, end, zone_type)?;
        self.write_section_data(name, &data)
    }

    /// Update a specific numeric value
    pub fn update_number(
        &mut self,
        section_name: &str,
        index: usize,
        value: DecimalValue,
    ) -> Result<()> {
        let mut data = self.get_section_data(section_name)?;
        data.db_line.set_number(index, value)?;
        self.write_section_data(section_name, &data)
    }

    /// Update a specific string value
    pub fn update_string(&mut self, section_name: &str, index: usize, value: String) -> Result<()> {
        let mut data = self.get_section_data(section_name)?;
        data.db_line.set_string(index, value)?;
        self.write_section_data(section_name, &data)
    }

    /// Update the database line for an index record.
    pub fn update_db_line(&mut self, section_name: &str, db_line: DbLine) -> Result<()> {
        let mut data = self.get_section_data(section_name)?;
        data.db_line = db_line;
        self.write_section_data(section_name, &data)
    }

    /// Batch update multiple values
    pub fn batch_update(
        &mut self,
        section_name: &str,
        numbers: &[(usize, DecimalValue)],
        strings: &[(usize, String)],
    ) -> Result<()> {
        let mut data = self.get_section_data(section_name)?;

        for (index, value) in numbers {
            data.db_line.set_number(*index, value.clone())?;
        }

        for (index, value) in strings {
            data.db_line.set_string(*index, value.clone())?;
        }

        self.write_section_data(section_name, &data)
    }

    // ==================== CLIPBOARD OPERATIONS ====================

    /// Copy a section's string to clipboard
    pub fn copy_string(&mut self, section_name: &str, string_index: usize) -> Result<String> {
        let section = self.get_section(section_name)?.clone();
        let text = clip::copy_zone_string(section_name, string_index, &self.content, &section)?;
        clip::copy_with_notification(
            &text,
            &format!("String {} from '{}'", string_index, section_name),
        )?;
        Ok(text)
    }

    /// Safe echo a section's string
    pub fn echo_string(&mut self, section_name: &str, string_index: usize) -> Result<EchoResult> {
        let s = self.get_string(section_name, string_index)?;
        safe_echo(&s)
    }

    // ==================== FILE OPERATIONS ====================

    /// Save the document to disk
    pub fn save(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }

        if self.config.create_backups {
            self.create_backup()?;
        }

        std::fs::write(&self.path, &self.content)?;
        self.dirty = false;

        if self.config.verbose {
            println!(
                "Saved: {} ({} bytes)",
                self.path.display(),
                self.content.len()
            );
        }

        Ok(())
    }

    /// Create a backup file
    fn create_backup(&self) -> Result<()> {
        let backup_path = PathBuf::from(format!("{}.undo", self.path.display()));
        std::fs::copy(&self.path, &backup_path)?;

        if self.config.verbose {
            println!("Backup: {}", backup_path.display());
        }

        Ok(())
    }

    /// Reload from disk
    pub fn reload(&mut self) -> Result<()> {
        self.content = std::fs::read_to_string(&self.path)?;
        self.header = scan_content(&self.content)?;
        self.section_cache.clear();
        self.dirty = false;
        Ok(())
    }

    /// Get the full content
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Display document summary
    pub fn display_summary(&self) -> String {
        self.header.display()
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        if self.dirty && self.config.auto_save {
            let _ = self.save();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_test_file() -> (tempfile::NamedTempFile, String) {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let content = r#"# Test Document

regedited open
index: 100
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9
intro one
intro two
intro three
Welcome to the intro.

regedited open
index: 200
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
10 | 20 | 30 | 40 | 50 | 60 | 70 | 80 | 90
config path
config notes
config ref
Configuration here.
"#;
        file.write_all(content.as_bytes()).unwrap();
        (file, content.to_string())
    }

    #[test]
    fn test_open_store() {
        let (file, _) = create_test_file();
        let store = Store::open(file.path()).unwrap();

        assert_eq!(store.list_sections().len(), 2);
        assert!(store.has_section("i100"));
        assert!(store.has_section("i200"));
    }

    #[test]
    fn test_get_db_line() {
        let (file, _) = create_test_file();
        let mut store = Store::open(file.path()).unwrap();

        let db = store.get_db_line("i100").unwrap();
        assert_eq!(db.numbers, [1, 2, 3, 4, 5, 6, 7, 8, 9].map(DecimalValue::from));
        assert_eq!(db.strings[0], "intro one");
    }

    #[test]
    fn test_get_db_table() {
        let (file, _) = create_test_file();
        let mut store = Store::open(file.path()).unwrap();

        let table = store.get_db_table("i200").unwrap();
        assert!(table.contains("10"));
        assert!(table.contains("20"));
        assert!(table.contains("config path"));
    }

    #[test]
    fn test_update_number() {
        let (file, _) = create_test_file();
        let mut store = Store::open(file.path()).unwrap();
        store.config.auto_save = false;

        store.update_number("i100", 0, 99.into()).unwrap();

        let db = store.get_db_line("i100").unwrap();
        assert_eq!(db.numbers[0], DecimalValue::from(99));
        assert_eq!(db.numbers[1], DecimalValue::from(2)); // Others unchanged
    }

    #[test]
    fn test_update_string() {
        let (file, _) = create_test_file();
        let mut store = Store::open(file.path()).unwrap();
        store.config.auto_save = false;

        store
            .update_string("i100", 0, "new string".to_string())
            .unwrap();

        let db = store.get_db_line("i100").unwrap();
        assert_eq!(db.strings[0], "new string");
    }

    #[test]
    fn test_update_zone() {
        let (file, _) = create_test_file();
        let mut store = Store::open(file.path()).unwrap();
        store.config.auto_save = false;

        store
            .update_zone("i200", 0, 100, 200, crate::zone_type::ZoneType::Code)
            .unwrap();

        let ascii = store.get_ascii_store("i200").unwrap();
        assert_eq!(ascii.zones[0].start, 100);
        assert_eq!(ascii.zones[0].end, 200);
        assert_eq!(ascii.zones[0].zone_type, crate::zone_type::ZoneType::Code);
    }

    #[test]
    fn test_batch_update() {
        let (file, _) = create_test_file();
        let mut store = Store::open(file.path()).unwrap();
        store.config.auto_save = false;

        store
            .batch_update(
                "i100",
                &[(0, 10.into()), (1, 20.into())],
                &[(0, "batch1".to_string()), (1, "batch2".to_string())],
            )
            .unwrap();

        let db = store.get_db_line("i100").unwrap();
        assert_eq!(db.numbers[0], DecimalValue::from(10));
        assert_eq!(db.numbers[1], DecimalValue::from(20));
        assert_eq!(db.strings[0], "batch1");
        assert_eq!(db.strings[1], "batch2");
    }

    #[test]
    fn test_add_section() {
        let (file, _) = create_test_file();
        let mut store = Store::open(file.path()).unwrap();
        store.config.auto_save = false;

        store.add_section("900").unwrap();

        assert!(store.has_section("i900"));
        assert_eq!(store.list_sections().len(), 3);

        // Check raw content has the new section with correct format
        assert!(store.content.contains("regedited open\nindex: 900"));
        assert!(store.content.contains("0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0"));
    }

    #[test]
    fn test_remove_section() {
        let (file, _) = create_test_file();
        let mut store = Store::open(file.path()).unwrap();
        store.config.auto_save = false;

        store.remove_section("i100").unwrap();

        assert!(!store.has_section("i100"));
        assert!(store.has_section("i200"));
    }

    #[test]
    fn test_canonical_index_lookup() {
        let (file, _) = create_test_file();
        let store = Store::open(file.path()).unwrap();

        assert!(store.has_section("index:100"));
        assert!(store.has_section("INDEX:100"));
        assert!(store.has_section("i100"));
        assert!(store.has_section("100"));
    }

    #[test]
    fn test_get_section_content() {
        let (file, _) = create_test_file();
        let store = Store::open(file.path()).unwrap();

        let content = store.get_section_content("i100").unwrap();
        assert!(content.contains("Welcome to the intro."));
        assert!(content.contains("Configuration here."));
    }
}
