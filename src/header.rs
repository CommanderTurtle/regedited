// SPDX-License-Identifier: AGPL-3.0
//! # Header Parser
//!
//! Scans files for canonical `regedited open` triggers and builds an index of
//! their fixed six-line records. The trigger is an exact lowercase substring
//! and may have arbitrary text before or after it.
//!
//! ## Index Format
//!
//! ```markdown
//! anything before regedited open anything after
//! index: 12345
//! 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
//! 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9
//! First string line, generic oneliner
//! Second string line, generic oneliner
//! Third string line, generic oneliner
//! ```

use crate::{MmapFile, RegeditedError, Result};
use std::collections::BTreeMap;
use std::path::Path;

/// Information about one index record's location in the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionInfo {
    /// Canonical layout key (`index:<n>`).
    pub name: String,
    /// Numeric identity read from the following `index:` line
    pub registry_index: Option<u64>,
    /// Line number of the header (0-indexed)
    pub header_line: usize,
    /// Line number of the index number (header_line + 1)
    pub index_line: usize,
    /// Line number of the Hex-word line (header_line + 2)
    pub ascii_line: usize,
    /// Line number of the numeric data line (header_line + 3)
    pub numeric_line: usize,
    /// Line number of string 1 (header_line + 4)
    pub string1_line: usize,
    /// Line number of string 2 (header_line + 5)
    pub string2_line: usize,
    /// Line number of string 3 (header_line + 6)
    pub string3_line: usize,
    /// Byte offset of the header line start
    pub header_byte_offset: usize,
}

impl SectionInfo {
    /// Create a new SectionInfo with computed fields
    pub fn new(
        name: String,
        registry_index: Option<u64>,
        header_line: usize,
        header_byte_offset: usize,
    ) -> Self {
        Self {
            name,
            registry_index,
            header_line,
            index_line: header_line + 1,
            ascii_line: header_line + 2,
            numeric_line: header_line + 3,
            string1_line: header_line + 4,
            string2_line: header_line + 5,
            string3_line: header_line + 6,
            header_byte_offset,
        }
    }

    /// Get the data block lines (index + Hex-word line + numeric + 3 strings)
    /// Returns (start_line, end_line) inclusive
    pub fn data_block_range(&self) -> (usize, usize) {
        (self.index_line, self.string3_line)
    }

    /// Get the marker plus six structured record lines.
    pub fn total_lines(&self) -> usize {
        7
    }

    /// Canonical identity for display and diagnostics.
    pub fn index_label(&self) -> String {
        self.registry_index
            .map(|index| format!("index:{}", index))
            .unwrap_or_else(|| self.name.clone())
    }

    /// Format for display
    pub fn display(&self) -> String {
        format!(
            "  {0} (marker @ line {1}, record lines {2}-{3})",
            self.index_label(),
            self.header_line,
            self.index_line,
            self.string3_line,
        )
    }
}

/// Document header containing all discovered index records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentHeader {
    /// Ordered map of internal layout key -> index info
    pub sections: BTreeMap<String, SectionInfo>,
    /// Total lines in the file
    pub total_lines: usize,
    /// Total bytes in the file
    pub total_bytes: usize,
}

impl DocumentHeader {
    /// Create empty document header
    pub fn new() -> Self {
        Self {
            sections: BTreeMap::new(),
            total_lines: 0,
            total_bytes: 0,
        }
    }

    /// Find an index by canonical key or compact numeric reference.
    pub fn get_section(&self, reference: &str) -> Option<&SectionInfo> {
        if let Some(index) = parse_index_reference(reference) {
            return self
                .sections
                .values()
                .find(|entry| entry.registry_index == Some(index));
        }
        self.sections.get(reference)
    }

    /// Compatibility lookup for canonical index keys, case-insensitively.
    pub fn get_section_case_insensitive(&self, name: &str) -> Option<&SectionInfo> {
        if let Some(index) = parse_index_reference(name) {
            return self
                .sections
                .values()
                .find(|entry| entry.registry_index == Some(index));
        }
        let lower = name.to_lowercase();
        self.sections
            .iter()
            .find(|(k, _)| k.to_lowercase() == lower)
            .map(|(_, v)| v)
    }

    /// Resolve a canonical numeric index reference, then an internal layout key.
    pub fn resolve_section(&self, reference: &str) -> Result<&SectionInfo> {
        if let Some(index) = parse_index_reference(reference) {
            let mut matches = self
                .sections
                .values()
                .filter(|section| section.registry_index == Some(index));
            let first = matches.next();
            if matches.next().is_some() {
                return Err(RegeditedError::Parse(format!(
                    "Registry index {} is ambiguous",
                    index
                )));
            }
            return first
                .ok_or_else(|| RegeditedError::SectionNotFound(format!("index:{}", index)));
        }

        self.get_section(reference)
            .or_else(|| self.get_section_case_insensitive(reference))
            .ok_or_else(|| RegeditedError::SectionNotFound(reference.to_string()))
    }

    /// List all internal layout keys (the public method name is retained for API compatibility).
    pub fn section_names(&self) -> Vec<&str> {
        self.sections.keys().map(|s| s.as_str()).collect()
    }

    /// Get number of index records.
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    /// Display summary
    pub fn display(&self) -> String {
        let mut lines = vec![
            format!(
                "Document: {0} indexes, {1} lines, {2} bytes",
                self.section_count(),
                self.total_lines,
                self.total_bytes
            ),
            "Indexes:".to_string(),
        ];
        for info in self.sections.values() {
            lines.push(format!(
                "  {0}: marker {1}, record {2}-{3}",
                info.index_label(),
                info.header_line,
                info.index_line,
                info.string3_line,
            ));
        }
        lines.join("\n")
    }
}

impl Default for DocumentHeader {
    fn default() -> Self {
        Self::new()
    }
}

/// Scan a file and build the document header index
///
/// This parses the file line-by-line to find canonical `regedited open`
/// triggers. For large files, this is done using memory-mapped I/O with fast
/// byte scanning.
pub fn scan_file<P: AsRef<Path>>(path: P) -> Result<DocumentHeader> {
    let mmap = MmapFile::open(path)?;
    let content = mmap.as_str();
    scan_content(content)
}

/// Scan content string and build document header index
///
/// Finds exact lowercase `regedited open` substrings anywhere in a line. Text
/// around the substring is ignored. The immediately following `index:` line
/// supplies the numeric identity; the remaining five record lines have fixed
/// positions after it.
pub fn scan_content(content: &str) -> Result<DocumentHeader> {
    let total_bytes = content.len();
    let mut sections: BTreeMap<String, SectionInfo> = BTreeMap::new();
    let mut total_lines = 0usize;
    let mut byte_offset = 0usize;

    for raw_line in content.split_inclusive('\n') {
        let line_num = total_lines;
        let line = raw_line.strip_suffix('\n').unwrap_or(raw_line);
        if contains_regedited_open_trigger(line) {
            let (section_name, registry_index) =
                canonical_trigger_identity(content, byte_offset + raw_line.len(), line_num)?;
            insert_index(
                &mut sections,
                SectionInfo::new(section_name, registry_index, line_num, byte_offset),
            )?;
        }

        total_lines += 1;
        byte_offset += raw_line.len();
    }

    if content.is_empty() {
        total_lines = 1;
    }

    Ok(DocumentHeader {
        sections,
        total_lines,
        total_bytes,
    })
}

fn insert_index(indexes: &mut BTreeMap<String, SectionInfo>, info: SectionInfo) -> Result<()> {
    if let Some(index) = info.registry_index {
        if indexes
            .values()
            .any(|existing| existing.registry_index == Some(index))
        {
            return Err(RegeditedError::Parse(format!(
                "Duplicate registry index {}",
                index
            )));
        }
    }
    if indexes.contains_key(&info.name) {
        return Err(RegeditedError::Parse(format!(
            "Duplicate index layout key '{}'",
            info.name
        )));
    }
    indexes.insert(info.name.clone(), info);
    Ok(())
}

/// Quick scan that finds canonical index keys and their marker line numbers.
pub fn quick_scan_names(content: &str) -> Vec<(String, usize)> {
    let mut result = Vec::new();
    let mut byte_offset = 0usize;

    for (line_num, raw_line) in content.split_inclusive('\n').enumerate() {
        let line = raw_line.strip_suffix('\n').unwrap_or(raw_line);
        if contains_regedited_open_trigger(line) {
            if let Ok((section_key, _)) =
                canonical_trigger_identity(content, byte_offset + raw_line.len(), line_num)
            {
                result.push((section_key, line_num));
            }
        }

        byte_offset += raw_line.len();
    }

    result
}

/// Detect the literal "regedited open" trigger from any position in a line.
///
/// No text before or after the trigger is parsed. The trigger only means:
/// "the following structured lines define an index."
/// Exact byte search — zero allocation, O(n) on the line.
fn contains_regedited_open_trigger(line: &str) -> bool {
    line.as_bytes()
        .windows(b"regedited open".len())
        .any(|window| window == b"regedited open")
}

fn canonical_trigger_identity(
    content: &str,
    next_line_byte_offset: usize,
    marker_line: usize,
) -> Result<(String, Option<u64>)> {
    let remaining = content.get(next_line_byte_offset..).ok_or_else(|| {
        RegeditedError::HeaderCorruption(format!(
            "Index marker at line {} is not followed by six record lines",
            marker_line
        ))
    })?;
    if remaining.split('\n').take(6).count() != 6 {
        return Err(RegeditedError::HeaderCorruption(format!(
            "Index marker at line {} is not followed by six record lines",
            marker_line
        )));
    }
    let index_line = line_at_or_after(content, next_line_byte_offset).ok_or_else(|| {
        RegeditedError::HeaderCorruption(format!(
            "Index marker at line {} has no index line",
            marker_line
        ))
    })?;
    let index = parse_registry_index_line(index_line).ok_or_else(|| {
        RegeditedError::HeaderCorruption(format!(
            "Index marker at line {} must be followed by 'index: N'",
            marker_line
        ))
    })?;
    Ok((format!("index:{}", index), Some(index)))
}

fn line_at_or_after(content: &str, byte_offset: usize) -> Option<&str> {
    if byte_offset >= content.len() {
        return None;
    }
    let rest = &content[byte_offset..];
    Some(rest.split_once('\n').map_or(rest, |(line, _)| line))
}

fn parse_registry_index_line(line: &str) -> Option<u64> {
    let trimmed = line.trim();
    if let Some(rest) = trimmed
        .strip_prefix("index:")
        .or_else(|| trimmed.strip_prefix("INDEX:"))
    {
        return rest.trim().parse::<u64>().ok();
    }
    trimmed.parse::<u64>().ok()
}

/// Parse `64`, `i64`, or `index:64` as the same canonical index reference.
pub fn parse_index_reference(reference: &str) -> Option<u64> {
    let value = reference.trim();
    if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) {
        return value.parse().ok();
    }
    if value.len() > 1
        && matches!(value.as_bytes()[0], b'i' | b'I')
        && value.as_bytes()[1..]
            .iter()
            .all(|byte| byte.is_ascii_digit())
    {
        return value[1..].parse().ok();
    }
    let (prefix, index) = value.split_once(':')?;
    if prefix.eq_ignore_ascii_case("index")
        && !index.is_empty()
        && index.bytes().all(|byte| byte.is_ascii_digit())
    {
        index.parse().ok()
    } else {
        None
    }
}

/// Extract one index record's six structured lines from content.
///
/// Returns all 6 record lines (index + hex-word + numeric + 3 strings).
pub fn extract_section_data(content: &str, section: &SectionInfo) -> Result<String> {
    let lines: Vec<&str> = content.lines().collect();

    let (start, end) = section.data_block_range();
    if end >= lines.len() {
        return Err(RegeditedError::ZoneOutOfBounds {
            line: end,
            max_lines: lines.len(),
        });
    }

    let data_lines = &lines[start..=end];
    Ok(data_lines.join("\n"))
}

/// Return the document visible from a resolved index.
///
/// Index records do not own surrounding content. This compatibility helper
/// verifies that the fixed record exists, then returns the complete UTF-8
/// document. Callers that need a bounded payload should resolve a zone.
pub fn extract_section_content(content: &str, section: &SectionInfo) -> Result<String> {
    let lines: Vec<&str> = content.lines().collect();
    if section.string3_line >= lines.len() {
        return Err(RegeditedError::ZoneOutOfBounds {
            line: section.string3_line,
            max_lines: lines.len(),
        });
    }
    Ok(content.to_string())
}

/// Update one index record's six structured lines in content.
///
/// Returns new content with the data block replaced
pub fn update_section_data(content: &str, section: &SectionInfo, new_data: &str) -> Result<String> {
    let lines: Vec<&str> = content.lines().collect();
    let (data_start, data_end) = section.data_block_range();

    if data_end >= lines.len() {
        return Err(RegeditedError::ZoneOutOfBounds {
            line: data_end,
            max_lines: lines.len(),
        });
    }

    let mut new_lines = Vec::new();

    // Lines before data block
    new_lines.extend_from_slice(&lines[..data_start]);

    // New data lines
    for line in new_data.lines() {
        new_lines.push(line);
    }

    // Lines after data block
    new_lines.extend_from_slice(&lines[data_end + 1..]);

    Ok(new_lines.join("\n"))
}

/// Update a single line in content
///
/// This is the fastest update method - only changes one line
pub fn update_line(content: &str, line_index: usize, new_line: &str) -> Result<String> {
    let lines: Vec<&str> = content.lines().collect();

    if line_index >= lines.len() {
        return Err(RegeditedError::ZoneOutOfBounds {
            line: line_index,
            max_lines: lines.len(),
        });
    }

    let mut new_lines = lines.clone();
    new_lines[line_index] = new_line;

    Ok(new_lines.join("\n"))
}

/// Update multiple lines in content (for batch updates)
pub fn update_lines(content: &str, changes: &[(usize, String)]) -> Result<String> {
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

    for (line_index, new_content) in changes {
        if *line_index >= lines.len() {
            return Err(RegeditedError::ZoneOutOfBounds {
                line: *line_index,
                max_lines: lines.len(),
            });
        }
        lines[*line_index] = new_content.clone();
    }

    Ok(lines.join("\n"))
}

/// Find the fixed index record that physically contains a line.
pub fn find_section_for_line(doc: &DocumentHeader, line: usize) -> Option<&SectionInfo> {
    doc.sections
        .values()
        .find(|s| line >= s.header_line && line <= s.string3_line)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_DOC: &str = r#"# My Document

regedited open
index: 100
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9
intro string one
intro string two
intro string three
Welcome to the intro section.
This is the content.

regedited open
index: 200
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
10 | 20 | 30 | 40 | 50 | 60 | 70 | 80 | 90
config path
config notes
config ref
Configuration details here.

regedited open
index: 300
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900
data summary
data notes
data ref
Data content starts here.
More data content.
"#;

    #[test]
    fn test_scan_content() {
        let doc = scan_content(TEST_DOC).unwrap();

        assert_eq!(doc.section_count(), 3);
        assert!(doc.get_section("index:100").is_some());
        assert!(doc.get_section("index:200").is_some());
        assert!(doc.get_section("index:300").is_some());

        let intro = doc.get_section("index:100").unwrap();
        assert_eq!(intro.header_line, 2);
        assert_eq!(intro.index_line, 3);
        assert_eq!(intro.ascii_line, 4);
        assert_eq!(intro.numeric_line, 5);
        assert_eq!(intro.string3_line, 8);

        let config = doc.get_section("index:200").unwrap();
        assert_eq!(config.header_line, 12);
        assert_eq!(config.string3_line, 18);

        let data = doc.get_section("index:300").unwrap();
        assert_eq!(data.header_line, 21);
        assert_eq!(data.string3_line, 27);
    }

    #[test]
    fn test_extract_section_data() {
        let doc = scan_content(TEST_DOC).unwrap();
        let intro = doc.get_section("index:100").unwrap();

        let data = extract_section_data(TEST_DOC, intro).unwrap();
        assert!(data.contains("index: 100"));
        assert!(data.contains("1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9"));
        assert!(data.contains("intro string one"));
        assert!(data.contains("intro string three"));
    }

    #[test]
    fn test_extract_section_content() {
        let doc = scan_content(TEST_DOC).unwrap();
        let intro = doc.get_section("index:100").unwrap();

        let content = extract_section_content(TEST_DOC, intro).unwrap();
        assert!(content.contains("Welcome to the intro section."));
    }

    #[test]
    fn test_update_section_data() {
        let doc = scan_content(TEST_DOC).unwrap();
        let intro = doc.get_section("index:100").unwrap();

        let new_data = "index: 100\n0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000\n7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15\nnew1\nnew2\nnew3";
        let updated = update_section_data(TEST_DOC, intro, new_data).unwrap();

        assert!(updated.contains("7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15"));
        assert!(updated.contains("new1"));
        assert!(updated.contains("Welcome to the intro section."));
    }

    #[test]
    fn test_update_line() {
        // Line 5 is the numeric line (9 pipe-separated values)
        let updated =
            update_line(TEST_DOC, 5, "99 | 88 | 77 | 66 | 55 | 44 | 33 | 22 | 11").unwrap();
        assert!(updated.contains("99 | 88 | 77 | 66 | 55 | 44 | 33 | 22 | 11"));
        assert!(!updated.contains("1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9"));
    }

    #[test]
    fn test_find_section_for_line() {
        let doc = scan_content(TEST_DOC).unwrap();

        let s = find_section_for_line(&doc, 2).unwrap();
        assert_eq!(s.name, "index:100");

        let s = find_section_for_line(&doc, 16).unwrap();
        assert_eq!(s.name, "index:200");

        assert!(find_section_for_line(&doc, 30).is_none());
    }

    #[test]
    fn test_case_insensitive_lookup() {
        let doc = scan_content(TEST_DOC).unwrap();

        assert!(doc.get_section_case_insensitive("INDEX:100").is_some());
        assert!(doc.get_section_case_insensitive("INDEX:200").is_some());
        assert!(doc.get_section_case_insensitive("INDEX:300").is_some());
    }

    #[test]
    fn numeric_index_references_are_canonical() {
        let doc = scan_content(TEST_DOC).unwrap();
        for reference in ["200", "i200", "I200", "index:200", "INDEX:200"] {
            let section = doc.resolve_section(reference).unwrap();
            assert_eq!(section.name, "index:200");
            assert_eq!(section.registry_index, Some(200));
        }
        assert!(doc.resolve_section("i999").is_err());

        for reference in ["64", "i64", "I64", "index:64", "INDEX:64"] {
            assert_eq!(parse_index_reference(reference), Some(64));
        }
        for reference in ["", "i", "index:", "i64s1", "Section64"] {
            assert_eq!(parse_index_reference(reference), None);
        }
    }

    #[test]
    fn duplicate_numeric_indexes_are_rejected_even_with_different_legacy_names() {
        let duplicate = r#"regedited open
index: 64
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0



regedited open
index: 64
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0



"#;
        let error = scan_content(duplicate).unwrap_err().to_string();
        assert!(error.contains("Duplicate registry index 64"), "{error}");
    }

    #[test]
    fn test_quick_scan_names() {
        let names = quick_scan_names(TEST_DOC);
        assert_eq!(names.len(), 3);
        assert_eq!(names[0], ("index:100".to_string(), 2));
        assert_eq!(names[1], ("index:200".to_string(), 12));
        assert_eq!(names[2], ("index:300".to_string(), 21));
    }

    #[test]
    fn test_regedited_open_trigger() {
        let html_doc = r#"<!DOCTYPE html>
<html>
<head><title>Test</title></head>
<body>
<!-- arbitrary prefix regedited open arbitrary suffix -->
index: 500
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9
html string one
html string two
html string three
<p>Some HTML content here</p>

/* arbitrary prefix regedited open arbitrary suffix */
index: 600
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
10 | 20 | 30 | 40 | 50 | 60 | 70 | 80 | 90
script notes
more notes
ref notes
<script>console.log("hello");</script>

// regedited open
index: 700
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0



<p>Another section</p>

## This ordinary heading is not an index marker
index: 800
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900
trad str1
trad str2
trad str3
Traditional content here.
</body>
</html>"#;

        let doc = scan_content(html_doc).unwrap();

        // Only the three canonical literal triggers are indexes.
        assert_eq!(
            doc.section_count(),
            3,
            "Expected 3 indexes, found {}",
            doc.section_count()
        );

        // Canonical triggers ignore surrounding text and key off the following index line.
        assert!(
            doc.get_section("index:500").is_some(),
            "index:500 not found"
        );
        assert!(
            doc.get_section("index:600").is_some(),
            "index:600 not found"
        );
        assert!(
            doc.get_section("index:700").is_some(),
            "index:700 not found"
        );

        assert!(doc.get_section("TraditionalHeader").is_none());
    }

    #[test]
    fn test_regedited_open_trigger_inline() {
        let doc_str = r#"# Some Document
wdfkbsdfknwdbfkwbfkbwekfbwekfbregedited openwjfjbwdkjfbwjnfbwjnf
index: 999
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
42 | 7 | 3 | 256 | 1024 | 4096 | 100 | 200 | 300
some notes
more notes
ref notes
Content here.
"#;

        let doc = scan_content(doc_str).unwrap();
        assert_eq!(doc.section_count(), 1);
        assert!(doc.get_section("index:999").is_some());
    }

    #[test]
    fn test_regedited_open_trigger_is_exact_lowercase_literal() {
        let doc_str = r#"# Some Document
Regedited open
index: 111
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0



This block is intentionally not opened by mixed case.

prefixregedited opensuffix
index: 222
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0



This block is opened by the exact lowercase trigger.
"#;

        let doc = scan_content(doc_str).unwrap();
        assert_eq!(doc.section_count(), 1);
        assert!(doc.get_section("index:222").is_some());
        assert!(doc.get_section("index:111").is_none());
    }
}
