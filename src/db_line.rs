// SPDX-License-Identifier: AGPL-3.0
//! # Database Line Parser
//!
//! Parses and manages the structured data lines in each section:
//! - Index number in `index: N` format
//! - hex-word line with 6 hex-words (colon-separated)
//! - 9 base-10 numerical values (pipe-separated ` | ` — Obsidian-friendly)
//! - 3 plain string values (one per line)
//!
//! Displayed as a pipes-and-dashes markdown table for human readability.
//!
//! ## Format in the markdown file
//!
//! ```markdown
//! ## SECTION: MySection
//! index: 123
//! 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
//! 1 | 100 | 50 | 200 | 25 | 75 | 10 | 20 | 30     <- 9 numbers (pipe-separated)
//! First string line, generic oneliner
//! Second string line, generic oneliner
//! Third string line, generic oneliner
//! ---
//! ... (content) ...
//! ```

use crate::{RegeditedError, Result};

/// Number of numeric fields in a database line
pub const NUMERIC_COUNT: usize = 9;

/// Number of string fields in a database line
pub const STRING_COUNT: usize = 3;

/// A structured database line containing 9 numeric values and 3 strings
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbLine {
    /// Nine numeric values (tabular data, easily callable/replaceable)
    pub numbers: [i64; NUMERIC_COUNT],
    /// Three string values (labels, notes, references)
    pub strings: [String; STRING_COUNT],
}

impl Default for DbLine {
    fn default() -> Self {
        Self {
            numbers: [0; NUMERIC_COUNT],
            strings: [String::new(), String::new(), String::new()],
        }
    }
}

impl DbLine {
    /// Create a new DbLine with all zeros and empty strings
    pub fn new() -> Self {
        Self::default()
    }

    /// Create from raw values
    pub fn from_values(numbers: [i64; NUMERIC_COUNT], strings: [String; STRING_COUNT]) -> Self {
        Self { numbers, strings }
    }

    /// Parse a DbLine from a section's data lines
    ///
    /// `lines` should contain:
    /// - line 0: 9 pipe-separated numbers (the numeric line)
    /// - line 1: string 1
    /// - line 2: string 2
    /// - line 3: string 3
    pub fn from_lines(lines: &[&str]) -> Result<Self> {
        if lines.len() < 4 {
            return Err(RegeditedError::InvalidDbLine(format!(
                "Expected at least 4 lines (1 numeric + 3 strings), got {}",
                lines.len()
            )));
        }

        // Parse the numeric line (pipe-separated for Obsidian compatibility)
        // Accepts both " | " (new) and "\t" (legacy) as separators
        let separator = if lines[0].contains(" | ") {
            " | "
        } else {
            "\t"
        };
        let numeric_parts: Vec<&str> = lines[0].split(separator).collect();
        if numeric_parts.len() != NUMERIC_COUNT {
            return Err(RegeditedError::InvalidDbLine(format!(
                "Expected {NUMERIC_COUNT} pipe-separated numbers, got {} (line: '{}')",
                numeric_parts.len(),
                lines[0]
            )));
        }

        let mut numbers = [0i64; NUMERIC_COUNT];
        for (i, part) in numeric_parts.iter().enumerate() {
            numbers[i] = part.trim().parse::<i64>().map_err(|e| {
                RegeditedError::InvalidDbLine(format!("Cannot parse number {i}: '{part}' ({e})"))
            })?;
        }

        // Parse strings (take exactly 3, trim whitespace)
        let mut strings = [String::new(), String::new(), String::new()];
        for i in 0..STRING_COUNT {
            strings[i] = lines[i + 1].trim().to_string();
        }

        Ok(Self { numbers, strings })
    }

    /// Parse from a single multi-line string block
    pub fn from_block(block: &str) -> Result<Self> {
        let lines: Vec<&str> = block.lines().collect();
        Self::from_lines(&lines)
    }

    /// Convert to the 4-line format (numeric line + 3 string lines)
    pub fn to_lines(&self) -> String {
        let mut result = String::new();

        // Numeric line (pipe-separated for Obsidian compatibility)
        result.push_str(&self.numeric_line());
        result.push('\n');

        // String lines
        for s in &self.strings {
            result.push_str(s);
            result.push('\n');
        }

        result
    }

    /// Get the numeric line only (pipe-separated ` | `)
    pub fn numeric_line(&self) -> String {
        self.numbers
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(" | ")
    }

    /// Get a specific number by index (0-8)
    pub fn get_number(&self, index: usize) -> Option<i64> {
        self.numbers.get(index).copied()
    }

    /// Set a specific number by index (0-8)
    pub fn set_number(&mut self, index: usize, value: i64) -> Result<()> {
        if index >= NUMERIC_COUNT {
            return Err(RegeditedError::Parse(format!(
                "Number index {index} out of range (0-{NUMERIC_COUNT})"
            )));
        }
        self.numbers[index] = value;
        Ok(())
    }

    /// Get a specific string by index (0-2)
    pub fn get_string(&self, index: usize) -> Option<&str> {
        self.strings.get(index).map(|s| s.as_str())
    }

    /// Set a specific string by index (0-2)
    pub fn set_string(&mut self, index: usize, value: String) -> Result<()> {
        if index >= STRING_COUNT {
            return Err(RegeditedError::Parse(format!(
                "String index {index} out of range (0-{STRING_COUNT})"
            )));
        }
        self.strings[index] = value;
        Ok(())
    }

    /// Format as a pipes-and-dashes markdown table
    ///
    /// ```markdown
    /// | Val1 | Val2 | Val3 | Val4 | Val5 | Val6 | Val7 | Val8 | Val9 |
    /// |------|------|------|------|------|------|------|------|------|
    /// | 1    | 100  | 50   | 200  | 25   | 75   | 10   | 20   | 30   |
    /// | Str1: First string line, generic oneliner
    /// | Str2: Second string line, generic oneliner
    /// | Str3: Third string line, generic oneliner
    /// ```
    pub fn to_markdown_table(&self) -> String {
        let mut lines = Vec::new();

        // Header row
        let headers: Vec<String> = (1..=NUMERIC_COUNT).map(|i| format!("Val{i}")).collect();
        lines.push(format!("| {} |", headers.join(" | ")));

        // Separator
        let sep: Vec<String> = (0..NUMERIC_COUNT).map(|_| "------".to_string()).collect();
        lines.push(format!("| {} |", sep.join(" | ")));

        // Values row
        let values: Vec<String> = self.numbers.iter().map(|n| format!("{n:>4}")).collect();
        lines.push(format!("| {} |", values.join(" | ")));

        // Separator
        lines.push(format!("| {} |", sep.join(" | ")));

        // String rows
        for (i, s) in self.strings.iter().enumerate() {
            if s.is_empty() {
                lines.push(format!(
                    "| *Str{}:* (empty)                                        |",
                    i + 1
                ));
            } else {
                lines.push(format!("| *Str{}:* {s}", i + 1));
            }
        }

        lines.join("\n")
    }

    /// Compact display format
    pub fn display_compact(&self) -> String {
        let nums = self
            .numbers
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let strs = self
            .strings
            .iter()
            .enumerate()
            .map(|(i, s)| format!("S{}: {s}", i + 1))
            .collect::<Vec<_>>()
            .join(" | ");
        format!("[{nums}] | {strs}")
    }

    /// Full display with all fields labeled
    pub fn display_full(&self) -> String {
        let mut lines = vec!["  Database Values (9):".to_string()];
        for (i, n) in self.numbers.iter().enumerate() {
            lines.push(format!("    Val{0}: {1}", i + 1, n));
        }
        lines.push("  Strings (3):".to_string());
        for (i, s) in self.strings.iter().enumerate() {
            if s.is_empty() {
                lines.push(format!("    Str{0}: (empty)", i + 1));
            } else {
                lines.push(format!("    Str{0}: {1}", i + 1, s));
            }
        }
        lines.join("\n")
    }
}

impl std::fmt::Display for DbLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_compact())
    }
}

/// Parse just the numeric line (9 pipe-separated values)
/// Accepts both " | " (new format) and "\t" (legacy) as separators
pub fn parse_numeric_line(line: &str) -> Result<[i64; NUMERIC_COUNT]> {
    let separator = if line.contains(" | ") { " | " } else { "\t" };
    let parts: Vec<&str> = line.split(separator).collect();
    if parts.len() != NUMERIC_COUNT {
        return Err(RegeditedError::InvalidDbLine(format!(
            "Expected {NUMERIC_COUNT} pipe-separated numbers, got {} (line: '{}')",
            parts.len(),
            line
        )));
    }

    let mut numbers = [0i64; NUMERIC_COUNT];
    for (i, part) in parts.iter().enumerate() {
        numbers[i] = part.trim().parse::<i64>().map_err(|e| {
            RegeditedError::InvalidDbLine(format!("Cannot parse number {i}: '{part}' ({e})"))
        })?;
    }

    Ok(numbers)
}

/// A section's complete data block (index + Hex-word line + DbLine)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionData {
    /// Section index number (stored as `index: N`)
    pub index: u64,
    /// The hex-word line store line (6 hex-words, colon-separated)
    pub ascii_store: crate::ascii_store::AsciiStore,
    /// The database line (9 numbers + 3 strings)
    pub db_line: DbLine,
}

impl SectionData {
    /// Create new empty section data
    pub fn new() -> Self {
        Self {
            index: 0,
            ascii_store: crate::ascii_store::AsciiStore::new(),
            db_line: DbLine::new(),
        }
    }

    /// Parse from the data lines of a section
    ///
    /// Lines expected:
    /// 0: `index: N` (new format) or plain `N` (legacy)
    /// 1: Hex-word line (6 hex-words, colon-separated)
    /// 2: 9 pipe-separated numbers
    /// 3: string 1
    /// 4: string 2
    /// 5: string 3
    pub fn from_lines(lines: &[&str]) -> Result<Self> {
        if lines.len() < 6 {
            return Err(RegeditedError::InvalidDbLine(format!(
                "SectionData needs 6 lines (1 index + 1 ASCII + 1 numeric + 3 strings), got {}",
                lines.len()
            )));
        }

        // Parse index: supports both "index: 123" (new) and "123" (legacy)
        let index_str = lines[0].trim();
        let index = if index_str.starts_with("index:") || index_str.starts_with("INDEX:") {
            index_str[6..].trim().parse::<u64>()
        } else {
            index_str.parse::<u64>()
        }
        .map_err(|e| {
            RegeditedError::InvalidDbLine(format!("Invalid section index '{}': {e}", lines[0]))
        })?;

        let ascii_store = crate::ascii_store::AsciiStore::from_line(lines[1])?;
        let db_line = DbLine::from_lines(&lines[2..])?;

        Ok(Self {
            index,
            ascii_store,
            db_line,
        })
    }

    /// Convert back to 6 lines
    pub fn to_lines(&self) -> String {
        let mut result = String::new();
        result.push_str(&format!("index: {}", self.index));
        result.push('\n');
        result.push_str(&self.ascii_store.to_line());
        result.push('\n');
        result.push_str(&self.db_line.to_lines());
        result
    }

    /// Full display including index, Hex-word line, and DB line
    pub fn display(&self) -> String {
        let parts = vec![
            format!("  Index: {}", self.index),
            self.ascii_store.display(),
            self.db_line.display_full(),
        ];
        parts.join("\n")
    }

    /// Get a zone from the Hex-word line and extract the corresponding string
    pub fn get_zone_string(&self, zone_index: usize) -> Option<&str> {
        self.db_line.get_string(zone_index)
    }
}

impl Default for SectionData {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_line_from_lines() {
        let lines = vec![
            "1 | 100 | 50 | 200 | 25 | 75 | 10 | 20 | 30",
            "First string line, generic oneliner",
            "Second string line, generic oneliner",
            "Third string line, generic oneliner",
        ];

        let db = DbLine::from_lines(&lines).unwrap();
        assert_eq!(db.numbers, [1, 100, 50, 200, 25, 75, 10, 20, 30]);
        assert_eq!(db.strings[0], "First string line, generic oneliner");
        assert_eq!(db.strings[1], "Second string line, generic oneliner");
        assert_eq!(db.strings[2], "Third string line, generic oneliner");
    }

    #[test]
    fn test_db_line_from_lines_legacy_tabs() {
        // Legacy tab-separated format should still parse
        let lines = vec![
            "1\t100\t50\t200\t25\t75\t10\t20\t30",
            "First string",
            "Second string",
            "Third string",
        ];

        let db = DbLine::from_lines(&lines).unwrap();
        assert_eq!(db.numbers, [1, 100, 50, 200, 25, 75, 10, 20, 30]);
    }

    #[test]
    fn test_db_line_numeric_line() {
        let db = DbLine::from_values(
            [42, -10, 999999, 0, 1, 2, 3, 4, 5],
            [String::new(), String::new(), String::new()],
        );
        assert_eq!(
            db.numeric_line(),
            "42 | -10 | 999999 | 0 | 1 | 2 | 3 | 4 | 5"
        );
    }

    #[test]
    fn test_db_line_to_lines_roundtrip() {
        let original = DbLine::from_values(
            [1, 2, 3, 4, 5, 6, 7, 8, 9],
            ["hello".into(), "world".into(), "test".into()],
        );

        let lines_str = original.to_lines();
        let lines: Vec<&str> = lines_str.lines().collect();
        let parsed = DbLine::from_lines(&lines).unwrap();

        assert_eq!(original, parsed);
    }

    #[test]
    fn test_db_line_markdown_table() {
        let db = DbLine::from_values(
            [1, 100, 50, 200, 25, 75, 10, 20, 30],
            ["code summary".into(), "notes".into(), "ref".into()],
        );

        let table = db.to_markdown_table();
        assert!(table.contains("| Val1 | Val2 | Val3 | Val4 | Val5 | Val6 | Val7 | Val8 | Val9 |"));
        assert!(table.contains("*Str1:* code summary"));
        assert!(table.contains("*Str2:* notes"));
        assert!(table.contains("*Str3:* ref"));
    }

    #[test]
    fn test_db_line_get_set() {
        let mut db = DbLine::new();

        db.set_number(0, 42).unwrap();
        db.set_number(8, 999).unwrap();
        assert_eq!(db.get_number(0), Some(42));
        assert_eq!(db.get_number(8), Some(999));
        assert_eq!(db.get_number(9), None);

        db.set_string(0, "hello".into()).unwrap();
        db.set_string(2, "world".into()).unwrap();
        assert_eq!(db.get_string(0), Some("hello"));
        assert_eq!(db.get_string(2), Some("world"));
        assert_eq!(db.get_string(1), Some(""));
        assert_eq!(db.get_string(3), None);
    }

    #[test]
    fn test_db_line_invalid_index() {
        let mut db = DbLine::new();
        assert!(db.set_number(9, 1).is_err());
        assert!(db.set_string(3, "x".into()).is_err());
    }

    #[test]
    fn test_parse_numeric_line() {
        let nums = parse_numeric_line("1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9").unwrap();
        assert_eq!(nums, [1, 2, 3, 4, 5, 6, 7, 8, 9]);

        let nums = parse_numeric_line("-10 | 0 | 999999 | 42 | -1 | 1000 | 0 | 0 | 0").unwrap();
        assert_eq!(nums, [-10, 0, 999999, 42, -1, 1000, 0, 0, 0]);
    }

    #[test]
    fn test_parse_numeric_line_legacy_tabs() {
        // Legacy tab-separated format should still parse
        let nums = parse_numeric_line("1\t2\t3\t4\t5\t6\t7\t8\t9").unwrap();
        assert_eq!(nums, [1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn test_parse_numeric_line_wrong_count() {
        assert!(parse_numeric_line("1 | 2 | 3").is_err());
        assert!(parse_numeric_line("1 | 2 | 3 | 4 | 5 | 6 | 7 | 8").is_err());
        assert!(parse_numeric_line("1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10").is_err());
    }

    #[test]
    fn test_section_data_roundtrip() {
        let ascii = crate::ascii_store::AsciiStore::new();

        let db = DbLine::from_values(
            [1, 100, 50, 200, 25, 75, 10, 20, 30],
            ["code".into(), "notes".into(), "ref".into()],
        );

        let section = SectionData {
            index: 42,
            ascii_store: ascii,
            db_line: db,
        };
        let lines_str = section.to_lines();

        let lines: Vec<&str> = lines_str.lines().collect();
        let parsed = SectionData::from_lines(&lines).unwrap();

        assert_eq!(section.index, parsed.index);
        assert_eq!(section.db_line, parsed.db_line);
    }

    #[test]
    fn test_section_data_index_format() {
        // Test parsing the new "index: 123" format
        let lines = vec![
            "index: 456",
            "0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000",
            "1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9",
            "str1",
            "str2",
            "str3",
        ];

        let section = SectionData::from_lines(&lines).unwrap();
        assert_eq!(section.index, 456);
    }

    #[test]
    fn test_section_data_legacy_index_format() {
        // Test parsing the legacy plain number format
        let lines = vec![
            "789",
            "0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000",
            "1\t2\t3\t4\t5\t6\t7\t8\t9",
            "str1",
            "str2",
            "str3",
        ];

        let section = SectionData::from_lines(&lines).unwrap();
        assert_eq!(section.index, 789);
    }
}
