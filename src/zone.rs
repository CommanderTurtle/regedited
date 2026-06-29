// SPDX-License-Identifier: AGPL-3.0
//! # Zone Extractor
//!
//! Extracts content zones from the markdown file using line ranges stored
//! in the hex-word line store. Each section can define up to 3 zones that
//! point to specific line ranges in the file.
//!
//! ## Zone Mapping
//!
//! The Hex-word line's 3 zone pairs map to the 3 strings in the database line:
//! - Zone 0 -> String 0 (e.g., "grab the first clip")
//! - Zone 1 -> String 1 (e.g., "grab the second clip")
//! - Zone 2 -> String 2 (e.g., "grab the third clip")
//!
//! ## Usage
//!
//! ```bash
//! # Extract zone 0 from section "MySection"
//! regedited grep myfile.md MySection 0
//!
//! # Copy zone 2's string to clipboard
//! regedited clip myfile.md MySection 2
//! ```

use crate::{extract_lines, header::SectionInfo, RegeditedError, Result};

/// An extracted zone with metadata
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zone {
    /// Section this zone belongs to
    pub section_name: String,
    /// Zone index (0-2)
    pub zone_index: usize,
    /// Start line (0-indexed, inclusive)
    pub start_line: usize,
    /// End line (0-indexed, inclusive)
    pub end_line: usize,
    /// The extracted content
    pub content: String,
    /// The associated string label from the database line
    pub label: String,
    /// Zone type (Markdown, Code, Media, Database)
    pub zone_type: crate::zone_type::ZoneType,
    /// Total lines in the zone
    pub line_count: usize,
    /// Total bytes in the zone
    pub byte_size: usize,
}

impl Zone {
    /// Create a new zone
    pub fn new(
        section_name: String,
        zone_index: usize,
        start_line: usize,
        end_line: usize,
        content: String,
        label: String,
        zone_type: crate::zone_type::ZoneType,
    ) -> Self {
        let line_count = content.lines().count();
        let byte_size = content.len();
        Self {
            section_name,
            zone_index,
            start_line,
            end_line,
            content,
            label,
            zone_type,
            line_count,
            byte_size,
        }
    }

    /// Format for display with type prefix
    pub fn display(&self) -> String {
        let type_tag = if self.zone_type != crate::zone_type::ZoneType::Markdown {
            format!(" [{}]", self.zone_type.short())
        } else {
            String::new()
        };
        format!(
            "--- Zone {} from '{}'{} ---\n  Lines: {}-{} ({} lines, {} bytes)\n  Type: {}\n  Label: {}\n\n{}",
            self.zone_index,
            self.section_name,
            type_tag,
            self.start_line,
            self.end_line,
            self.line_count,
            self.byte_size,
            self.zone_type.label(),
            self.label,
            self.content
        )
    }

    /// Get just the content (no metadata)
    pub fn content_only(&self) -> &str {
        &self.content
    }

    /// Get first N lines of content
    pub fn preview(&self, n: usize) -> String {
        self.content.lines().take(n).collect::<Vec<_>>().join("\n")
    }

    /// Check if zone is empty
    pub fn is_empty(&self) -> bool {
        self.content.trim().is_empty()
    }
}

impl std::fmt::Display for Zone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display())
    }
}

/// Extract a zone from file content
///
/// # Arguments
/// * `content` - The full file content
/// * `section` - The section info containing zone definitions
/// * `zone_index` - Which zone to extract (0-2)
/// * `label` - The associated string label
pub fn extract_zone(
    content: &str,
    section: &SectionInfo,
    zone_index: usize,
    label: &str,
) -> Result<Zone> {
    if zone_index >= 3 {
        return Err(RegeditedError::Parse(format!(
            "Zone index {zone_index} out of range (0-2)"
        )));
    }

    // Get the Hex-word line to find zone line ranges
    // We need to read the hex-word line first
    let ascii_store = read_ascii_store(content, section)?;

    let zone_pair = ascii_store.zone(zone_index).ok_or_else(|| {
        RegeditedError::Parse(format!(
            "Zone {zone_index} not found in section '{}'",
            section.name
        ))
    })?;

    if zone_pair.is_empty() {
        return Err(RegeditedError::Parse(format!(
            "Zone {zone_index} in section '{}' is not set (0,0)",
            section.name
        )));
    }

    // Extract the content between start_line and end_line
    let zone_content = extract_lines(
        content.as_bytes(),
        zone_pair.start as usize,
        zone_pair.end as usize,
    )?;
    let zone_content = String::from_utf8(zone_content)
        .map_err(|e| RegeditedError::Parse(format!("Zone content is not valid UTF-8: {e}")))?;

    Ok(Zone::new(
        section.name.clone(),
        zone_index,
        zone_pair.start as usize,
        zone_pair.end as usize,
        zone_content,
        label.to_string(),
        zone_pair.zone_type,
    ))
}

/// Extract all active zones from a section
pub fn extract_all_zones(
    content: &str,
    section: &SectionInfo,
    labels: &[String],
) -> Result<Vec<Zone>> {
    let ascii_store = read_ascii_store(content, section)?;
    let mut zones = Vec::new();

    for (i, _) in ascii_store.active_zones() {
        let label = labels.get(i).map(|s| s.as_str()).unwrap_or("");
        match extract_zone(content, section, i, label) {
            Ok(zone) => zones.push(zone),
            Err(e) => {
                eprintln!("Warning: Could not extract zone {i}: {e}");
            }
        }
    }

    Ok(zones)
}

/// Extract a zone by absolute line numbers (bypasses Hex-word line)
///
/// This is useful for ad-hoc zone extraction without having the zone
/// defined in the Hex-word line.
pub fn extract_zone_by_lines(
    content: &str,
    section_name: &str,
    zone_index: usize,
    start_line: usize,
    end_line: usize,
    label: &str,
    zone_type: crate::zone_type::ZoneType,
) -> Result<Zone> {
    let zone_content = extract_lines(content.as_bytes(), start_line, end_line)?;
    let zone_content = String::from_utf8(zone_content)
        .map_err(|e| RegeditedError::Parse(format!("Zone content is not valid UTF-8: {e}")))?;

    Ok(Zone::new(
        section_name.to_string(),
        zone_index,
        start_line,
        end_line,
        zone_content,
        label.to_string(),
        zone_type,
    ))
}

/// Read the Hex-word line from a section
fn read_ascii_store(
    content: &str,
    section: &SectionInfo,
) -> Result<crate::ascii_store::AsciiStore> {
    let lines: Vec<&str> = content.lines().collect();

    if section.ascii_line >= lines.len() {
        return Err(RegeditedError::ZoneOutOfBounds {
            line: section.ascii_line,
            max_lines: lines.len(),
        });
    }

    crate::ascii_store::AsciiStore::from_line(lines[section.ascii_line])
}

/// Grep-like search within a zone
///
/// Searches for a pattern within the zone's content and returns
/// matching lines with their relative line numbers.
pub fn grep_in_zone(zone: &Zone, pattern: &str) -> Vec<(usize, String)> {
    let mut matches = Vec::new();

    for (rel_line, line_content) in zone.content.lines().enumerate() {
        if line_content.contains(pattern) {
            matches.push((rel_line, line_content.to_string()));
        }
    }

    matches
}

/// Grep-like search within a section's content
///
/// Searches all lines in a section's content range.
pub fn grep_in_section(
    content: &str,
    section: &SectionInfo,
    pattern: &str,
) -> Vec<(usize, String)> {
    let mut matches = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let (start, end) = section.content_range();

    for line_num in start..=end {
        if line_num >= lines.len() {
            break;
        }
        if lines[line_num].contains(pattern) {
            matches.push((line_num, lines[line_num].to_string()));
        }
    }

    matches
}

/// The ZoneExtractor struct provides a high-level API for zone operations
pub struct ZoneExtractor<'a> {
    content: &'a str,
}

impl<'a> ZoneExtractor<'a> {
    /// Create a new zone extractor
    pub fn new(content: &'a str) -> Self {
        Self { content }
    }

    /// Extract a zone from a section
    pub fn extract(&self, section: &SectionInfo, zone_index: usize, label: &str) -> Result<Zone> {
        extract_zone(self.content, section, zone_index, label)
    }

    /// Extract all active zones from a section
    pub fn extract_all(&self, section: &SectionInfo, labels: &[String]) -> Result<Vec<Zone>> {
        extract_all_zones(self.content, section, labels)
    }

    /// Grep within a zone
    pub fn grep(&self, zone: &Zone, pattern: &str) -> Vec<(usize, String)> {
        grep_in_zone(zone, pattern)
    }

    /// Grep within a section
    pub fn grep_section(&self, section: &SectionInfo, pattern: &str) -> Vec<(usize, String)> {
        grep_in_section(self.content, section, pattern)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::scan_content;

    const TEST_DOC: &str = r#"# My Document

## SECTION: Code
index: 100
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9
important code summary
notes about code
reference link
---
function hello() {
    println!("Hello, world!");
}

function goodbye() {
    println!("Goodbye!");
}

## SECTION: Docs
index: 200
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
10 | 20 | 30 | 40 | 50 | 60 | 70 | 80 | 90
documentation summary
doc notes
doc ref
---
# Documentation

This is the documentation section.
It has multiple lines of content.
"#;

    #[test]
    fn test_extract_zone_by_lines() {
        // Direct line extraction - no dependency on Hex-word line
        let zone = extract_zone_by_lines(
            TEST_DOC,
            "Code",
            0,
            10,
            11,
            "test label",
            crate::zone_type::ZoneType::Markdown,
        )
        .unwrap();

        assert_eq!(zone.start_line, 10);
        assert_eq!(zone.end_line, 11);
        assert!(zone.content.contains("function hello()"));
    }

    #[test]
    fn test_grep_in_zone() {
        // Create a zone directly for grep testing
        let zone = Zone::new(
            "Code".to_string(),
            0,
            10,
            16,
            "function hello() {\n    println!(\"Hello\");\n}\n\nfunction goodbye() {\n    println!(\"Goodbye\");\n}".to_string(),
            "code".to_string(),
            crate::zone_type::ZoneType::Code,
        );

        let matches = grep_in_zone(&zone, "println!");
        assert_eq!(matches.len(), 2);
        assert!(matches[0].1.contains("Hello"));
        assert!(matches[1].1.contains("Goodbye"));
    }

    #[test]
    fn test_grep_in_section() {
        let doc = scan_content(TEST_DOC).unwrap();
        let code_section = doc.get_section("Code").unwrap();

        let matches = grep_in_section(TEST_DOC, code_section, "function");
        // function hello() and function goodbye()
        assert!(matches.len() >= 2);
    }

    #[test]
    fn test_zone_display() {
        let zone = Zone::new(
            "TestSection".to_string(),
            1,
            10,
            20,
            "line 10\nline 11\nline 12\n".to_string(),
            "my label".to_string(),
            crate::zone_type::ZoneType::Markdown,
        );

        let display = zone.display();
        assert!(display.contains("Zone 1 from 'TestSection'"));
        assert!(display.contains("Lines: 10-20"));
        assert!(display.contains("Label: my label"));
        assert!(display.contains("line 10"));
    }

    #[test]
    fn test_empty_zone() {
        let zone = Zone::new(
            "Test".to_string(),
            0,
            0,
            0,
            "".to_string(),
            "".to_string(),
            crate::zone_type::ZoneType::Markdown,
        );
        assert!(zone.is_empty());
    }

    #[test]
    fn test_zone_extractor() {
        let extractor = ZoneExtractor::new(TEST_DOC);
        let doc = scan_content(TEST_DOC).unwrap();
        let code = doc.get_section("Code").unwrap();

        // Test section grep
        let matches = extractor.grep_section(code, "function");
        assert_eq!(matches.len(), 2);

        // Test direct zone by lines
        let zone = extract_zone_by_lines(
            TEST_DOC,
            "Code",
            0,
            10,
            16,
            "test",
            crate::zone_type::ZoneType::Code,
        )
        .unwrap();
        assert!(!zone.is_empty());
        assert!(zone.content.contains("function"));
    }
}
