//! # Header Parser
//!
//! Scans markdown files for `## SECTION:` headers and builds an index of
//! section locations. This enables O(1) jumps to any section without
//! parsing the entire file.
//!
//! ## Section Format (v2 — with hex-word stores and 9 database values)
//!
//! ```markdown
//! ## SECTION: SectionName
//! 12345 (index number)
//! <UTF-16LE ASCII store — 6 values, 3 pairs>
//! 0 1 2 3 4 5 6 7 8 (9 tab-separated database numbers)
//! First string line, generic oneliner
//! Second string line, generic oneliner
//! Third string line, generic oneliner
//! ---
//! ... content ...
//! ```

use crate::{find_line_offsets, MmapFile, Result, RegeditedError};
use std::collections::BTreeMap;
use std::path::Path;

/// The prefix that marks a section header
pub const SECTION_PREFIX: &str = "## SECTION:";

/// The separator that marks end of section metadata / start of content
pub const CONTENT_SEPARATOR: &str = "---";

/// Information about a section's location in the file
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionInfo {
    /// Section name (the part after "## SECTION:")
    pub name: String,
    /// Line number of the header (0-indexed)
    pub header_line: usize,
    /// Line number of the index number (header_line + 1)
    pub index_line: usize,
    /// Line number of the ASCII store (header_line + 2)
    pub ascii_line: usize,
    /// Line number of the numeric data line (header_line + 3)
    pub numeric_line: usize,
    /// Line number of string 1 (header_line + 4)
    pub string1_line: usize,
    /// Line number of string 2 (header_line + 5)
    pub string2_line: usize,
    /// Line number of string 3 (header_line + 6)
    pub string3_line: usize,
    /// Line number of the content separator "---" (header_line + 7)
    pub separator_line: usize,
    /// Line number where content starts (separator_line + 1)
    pub content_start: usize,
    /// Line number where content ends (start of next section - 1, or EOF)
    pub content_end: usize,
    /// Byte offset of the header line start
    pub header_byte_offset: usize,
    /// Byte offset of the content end
    pub content_end_byte_offset: usize,
}

impl SectionInfo {
    /// Create a new SectionInfo with computed fields
    pub fn new(
        name: String,
        header_line: usize,
        header_byte_offset: usize,
        content_end: usize,
        content_end_byte_offset: usize,
    ) -> Self {
        Self {
            name,
            header_line,
            index_line: header_line + 1,
            ascii_line: header_line + 2,
            numeric_line: header_line + 3,
            string1_line: header_line + 4,
            string2_line: header_line + 5,
            string3_line: header_line + 6,
            separator_line: header_line + 7,
            content_start: header_line + 8,
            content_end,
            header_byte_offset,
            content_end_byte_offset,
        }
    }

    /// Get the data block lines (index + ASCII store + numeric + 3 strings)
    /// Returns (start_line, end_line) inclusive
    pub fn data_block_range(&self) -> (usize, usize) {
        (self.index_line, self.string3_line)
    }

    /// Get the content lines
    /// Returns (start_line, end_line) inclusive
    pub fn content_range(&self) -> (usize, usize) {
        (self.content_start, self.content_end)
    }

    /// Get the total number of lines in this section
    pub fn total_lines(&self) -> usize {
        self.content_end.saturating_sub(self.header_line) + 1
    }

    /// Format for display
    pub fn display(&self) -> String {
        format!(
            "  {0} (header @ line {1}, content lines {2}-{3}, {4} lines total)",
            self.name,
            self.header_line,
            self.content_start,
            self.content_end,
            self.total_lines()
        )
    }
}

/// Document header containing all sections
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentHeader {
    /// Ordered map of section name -> section info
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

    /// Find a section by name (exact match)
    pub fn get_section(&self, name: &str) -> Option<&SectionInfo> {
        self.sections.get(name)
    }

    /// Find a section by name (case-insensitive)
    pub fn get_section_case_insensitive(&self, name: &str) -> Option<&SectionInfo> {
        let lower = name.to_lowercase();
        self.sections
            .iter()
            .find(|(k, _)| k.to_lowercase() == lower)
            .map(|(_, v)| v)
    }

    /// List all section names
    pub fn section_names(&self) -> Vec<&str> {
        self.sections.keys().map(|s| s.as_str()).collect()
    }

    /// Get number of sections
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    /// Display summary
    pub fn display(&self) -> String {
        let mut lines = vec![
            format!("Document: {0} sections, {1} lines, {2} bytes",
                self.section_count(), self.total_lines, self.total_bytes),
            "Sections:".to_string(),
        ];
        for (name, info) in &self.sections {
            lines.push(format!("  {0}: lines {1}-{2}",
                name, info.header_line, info.content_end));
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
/// This parses the file line-by-line to find all `## SECTION:` headers
/// and build an index. For large files, this is done using memory-mapped I/O
/// with fast byte scanning.
pub fn scan_file<P: AsRef<Path>>(path: P) -> Result<DocumentHeader> {
    let mmap = MmapFile::open(path)?;
    let content = mmap.as_str();
    scan_content(content)
}

/// Scan content string and build document header index
/// 
/// More efficient when content is already in memory.
pub fn scan_content(content: &str) -> Result<DocumentHeader> {
    let line_offsets = find_line_offsets(content.as_bytes());
    let total_lines = line_offsets.len();
    let total_bytes = content.len();

    let mut sections: BTreeMap<String, SectionInfo> = BTreeMap::new();
    let mut current_header: Option<(String, usize, usize)> = None;

    for (line_num, byte_offset) in &line_offsets {
        let line = get_line_at(content, *byte_offset);
        
        if let Some(name) = parse_section_header(line) {
            // If we had a previous section, finalize it
            if let Some((prev_name, prev_line, prev_byte)) = current_header.take() {
                let info = SectionInfo::new(
                    prev_name,
                    prev_line,
                    prev_byte,
                    line_num.saturating_sub(1),
                    *byte_offset,
                );
                sections.insert(info.name.clone(), info);
            }
            current_header = Some((name, *line_num, *byte_offset));
        }
    }

    // Finalize the last section
    if let Some((name, line, byte_offset)) = current_header {
        let info = SectionInfo::new(
            name,
            line,
            byte_offset,
            total_lines.saturating_sub(1),
            total_bytes,
        );
        sections.insert(info.name.clone(), info);
    }

    Ok(DocumentHeader {
        sections,
        total_lines,
        total_bytes,
    })
}

/// Quick scan that only finds section names and their header line numbers
/// 
/// This is useful for listing sections without full parsing.
pub fn quick_scan_names(content: &str) -> Vec<(String, usize)> {
    let mut result = Vec::new();
    
    for (line_num, line) in content.lines().enumerate() {
        if let Some(name) = parse_section_header(line) {
            result.push((name, line_num));
        }
    }
    
    result
}

/// Parse a section header line
/// 
/// Returns `Some(name)` if the line is a `## SECTION: Name` header
/// Returns `None` otherwise
fn parse_section_header(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if trimmed.starts_with(SECTION_PREFIX) {
        let name = trimmed[SECTION_PREFIX.len()..].trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}

/// Get the line at a specific byte offset
fn get_line_at(content: &str, byte_offset: usize) -> &str {
    let start = byte_offset.min(content.len());
    if let Some(pos) = content[start..].find('\n') {
        &content[start..start + pos]
    } else {
        &content[start..]
    }
}

/// Extract a specific section's data block from content
/// 
/// Returns the 5 data lines (ASCII store + numeric line + 3 strings)
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

/// Extract a section's content (markdown between --- and next section)
pub fn extract_section_content(content: &str, section: &SectionInfo) -> Result<String> {
    let lines: Vec<&str> = content.lines().collect();
    
    let (start, end) = section.content_range();
    if start >= lines.len() {
        return Ok(String::new());
    }
    
    let actual_end = end.min(lines.len() - 1);
    let content_lines = &lines[start..=actual_end];
    Ok(content_lines.join("\n"))
}

/// Update a section's data block in content
/// 
/// Returns new content with the data block replaced
pub fn update_section_data(
    content: &str,
    section: &SectionInfo,
    new_data: &str,
) -> Result<String> {
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
pub fn update_lines(
    content: &str,
    changes: &[(usize, String)],
) -> Result<String> {
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

/// Find which section a line belongs to
pub fn find_section_for_line(doc: &DocumentHeader, line: usize) -> Option<&SectionInfo> {
    doc.sections.values().find(|s| {
        line >= s.header_line && line <= s.content_end
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_DOC: &str = r#"# My Document

## SECTION: Intro
100
0x00000000 : 0x00000000 : 0x00000000 : 0x00000000 : 0x00000000 : 0x00000000
1	2	3	4	5	6	7	8	9
intro string one
intro string two
intro string three
---
Welcome to the intro section.
This is the content.

## SECTION: Config
200
0x00000000 : 0x00000000 : 0x00000000 : 0x00000000 : 0x00000000 : 0x00000000
10	20	30	40	50	60	70	80	90
config path
config notes
config ref
---
Configuration details here.

## SECTION: Data
300
0x00000000 : 0x00000000 : 0x00000000 : 0x00000000 : 0x00000000 : 0x00000000
100	200	300	400	500	600	700	800	900
data summary
data notes
data ref
---
Data content starts here.
More data content.
"#;

    #[test]
    fn test_parse_section_header() {
        assert_eq!(
            parse_section_header("## SECTION: MySection"),
            Some("MySection".to_string())
        );
        assert_eq!(
            parse_section_header("  ## SECTION: Indented"),
            Some("Indented".to_string())
        );
        assert_eq!(
            parse_section_header("## SECTION:"),
            None // Empty name
        );
        assert_eq!(
            parse_section_header("# Just a heading"),
            None
        );
        assert_eq!(
            parse_section_header("## Not a section"),
            None
        );
    }

    #[test]
    fn test_scan_content() {
        let doc = scan_content(TEST_DOC).unwrap();
        
        assert_eq!(doc.section_count(), 3);
        assert!(doc.get_section("Intro").is_some());
        assert!(doc.get_section("Config").is_some());
        assert!(doc.get_section("Data").is_some());

        let intro = doc.get_section("Intro").unwrap();
        assert_eq!(intro.header_line, 2);
        assert_eq!(intro.index_line, 3);
        assert_eq!(intro.ascii_line, 4);
        assert_eq!(intro.numeric_line, 5);
        assert_eq!(intro.content_start, 10);
        assert_eq!(intro.content_end, 12);
        
        let config = doc.get_section("Config").unwrap();
        assert_eq!(config.header_line, 13);
        assert_eq!(config.content_end, 22);
        
        let data = doc.get_section("Data").unwrap();
        assert_eq!(data.header_line, 23);
        assert_eq!(data.content_end, 32);
    }

    #[test]
    fn test_extract_section_data() {
        let doc = scan_content(TEST_DOC).unwrap();
        let intro = doc.get_section("Intro").unwrap();
        
        let data = extract_section_data(TEST_DOC, intro).unwrap();
        assert!(data.contains("100")); // Index number
        assert!(data.contains("1\t2\t3\t4\t5\t6\t7\t8\t9"));
        assert!(data.contains("intro string one"));
        assert!(data.contains("intro string three"));
    }

    #[test]
    fn test_extract_section_content() {
        let doc = scan_content(TEST_DOC).unwrap();
        let intro = doc.get_section("Intro").unwrap();
        
        let content = extract_section_content(TEST_DOC, intro).unwrap();
        assert!(content.contains("Welcome to the intro section."));
    }

    #[test]
    fn test_update_section_data() {
        let doc = scan_content(TEST_DOC).unwrap();
        let intro = doc.get_section("Intro").unwrap();
        
        let new_data = "NEWASCII\n7\t8\t9\t10\t11\t12\nnew1\nnew2\nnew3\n---";
        let updated = update_section_data(TEST_DOC, intro, new_data).unwrap();
        
        assert!(updated.contains("NEWASCII"));
        assert!(updated.contains("7\t8\t9\t10\t11\t12"));
        assert!(updated.contains("new1"));
        assert!(updated.contains("Welcome to the intro section."));
    }

    #[test]
    fn test_update_line() {
        // Line 5 is the numeric line (9 tab-separated values)
        let updated = update_line(TEST_DOC, 5, "99\t88\t77\t66\t55\t44\t33\t22\t11").unwrap();
        assert!(updated.contains("99\t88\t77\t66\t55\t44\t33\t22\t11"));
        assert!(!updated.contains("1\t2\t3\t4\t5\t6\t7\t8\t9"));
    }

    #[test]
    fn test_find_section_for_line() {
        let doc = scan_content(TEST_DOC).unwrap();
        
        let s = find_section_for_line(&doc, 2).unwrap();
        assert_eq!(s.name, "Intro");
        
        let s = find_section_for_line(&doc, 16).unwrap();
        assert_eq!(s.name, "Config");
        
        // Data section - check a line within its content
        let s = find_section_for_line(&doc, 30).unwrap();
        assert_eq!(s.name, "Data");
    }

    #[test]
    fn test_case_insensitive_lookup() {
        let doc = scan_content(TEST_DOC).unwrap();
        
        assert!(doc.get_section_case_insensitive("intro").is_some());
        assert!(doc.get_section_case_insensitive("CONFIG").is_some());
        assert!(doc.get_section_case_insensitive("data").is_some());
    }

    #[test]
    fn test_quick_scan_names() {
        let names = quick_scan_names(TEST_DOC);
        assert_eq!(names.len(), 3);
        assert_eq!(names[0], ("Intro".to_string(), 2));
        assert_eq!(names[1], ("Config".to_string(), 13));
        assert_eq!(names[2], ("Data".to_string(), 23));
    }
}
