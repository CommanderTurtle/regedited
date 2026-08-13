// SPDX-License-Identifier: AGPL-3.0
//! # Fast Operations — Safetensors-Style Speed
//!
//! This module provides blazing-fast operations inspired by the safetensors
//! format's ability to scan, diff, and replace keys in multi-GB files
//! without loading the full data into RAM.
//!
//! ## How Safetensors Achieves Speed
//!
//! 1. **Header contains metadata only** — key names, shapes, byte offsets
//! 2. **Data is memory-mapped** — tensors accessed via offsets, not copies
//! 3. **Scan reads only headers** — O(n) on metadata, not O(n) on data
//! 4. **Replace rewrites only headers** — data blocks stay in place
//!
//! ## Regedited Parallel
//!
//! | Safetensors | Regedited |
//! |-------------|--------|
//! | `load_file()` header JSON | `scan_file()` numeric index map |
//! | Key name filter | Numeric index/key filter |
//! | Shape filter | Database value filter |
//! | Tensor offset | Line number offset |
//! | `save_file()` patched header | `fast_replace()` patched fixed records |
//!
//! ## Design
//!
//! All operations use `DocumentHeader` (the index) to work with offsets,
//! not content. The actual markdown content is only read when extracting
//! a specific zone or performing a replace.

use crate::{
    ascii_store::AsciiStore,
    db_line::{parse_numeric_line, DecimalValue},
    header::scan_content,
    zone_type::ZoneType,
    MmapFile, Result,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

// ==================== FAST SCAN ====================

/// A scanned fixed index record with its key attributes
#[derive(Debug, Clone)]
pub struct ScannedSection {
    pub name: String,
    pub index: u64,
    pub header_line: usize,
    pub ascii_line: usize,
    pub numeric_line: usize,
    pub db_values: [DecimalValue; 9],
    pub strings: [String; 3],
    pub zone_pairs: [(u32, u32); 3],
    pub zone_types: [ZoneType; 3],
    pub record_lines: usize,
}

/// Fast scan — like safetensors' header scan, reads metadata through mmap.
///
/// Parses each fixed record's index, hex-word line, and database line without
/// copying the whole document into Rust heap memory. The OS maps the file and
/// the scanner walks borrowed string slices over that mapped view.
pub fn fast_scan(file_path: &Path) -> Result<Vec<ScannedSection>> {
    let mmap = MmapFile::open(file_path)?;
    fast_scan_content(mmap.as_str())
}

/// Fast scan from already-loaded content
pub fn fast_scan_content(content: &str) -> Result<Vec<ScannedSection>> {
    let header = scan_content(content)?;
    let mut needed_lines = BTreeSet::new();
    for info in header.sections.values() {
        needed_lines.insert(info.index_line);
        needed_lines.insert(info.ascii_line);
        needed_lines.insert(info.numeric_line);
        needed_lines.insert(info.string1_line);
        needed_lines.insert(info.string2_line);
        needed_lines.insert(info.string3_line);
    }

    let mut line_lookup: BTreeMap<usize, &str> = BTreeMap::new();
    for (line_num, line) in content.lines().enumerate() {
        if needed_lines.contains(&line_num) {
            line_lookup.insert(line_num, line);
            if line_lookup.len() == needed_lines.len() {
                break;
            }
        }
    }
    let mut results = Vec::new();

    for (name, info) in &header.sections {
        // Quick bounds check
        if info.string3_line >= header.total_lines {
            continue;
        }

        // The header scanner already resolved and de-duplicated the numeric
        // identity. Never turn a malformed record into a valid-looking index 0.
        let index = info.registry_index.ok_or_else(|| {
            crate::RegeditedError::Parse(format!(
                "Malformed index record at marker line {}: the following line must be 'index: N'",
                info.header_line
            ))
        })?;

        // Read Hex-word line
        let ascii_line = info.header_line + 2;
        let ascii = if let Some(line) = line_lookup.get(&ascii_line) {
            AsciiStore::from_line(line).unwrap_or_default()
        } else {
            AsciiStore::default()
        };

        // Read database values (9 pipe-separated numbers)
        let db_values = if let Some(line) = line_lookup.get(&info.numeric_line) {
            parse_numeric_line(line)?
        } else {
            std::array::from_fn(|_| DecimalValue::default())
        };

        // Read 3 strings
        let mut strings = [String::new(), String::new(), String::new()];
        for (i, string) in strings.iter_mut().enumerate() {
            let line_idx = info.numeric_line + 1 + i;
            if let Some(line) = line_lookup.get(&line_idx) {
                *string = line.trim().to_string();
            }
        }

        // Extract zone info
        let mut zone_pairs = [(0u32, 0u32); 3];
        let mut zone_types = [ZoneType::Markdown; 3];
        for i in 0..3 {
            if let Some(zone) = ascii.zone(i) {
                zone_pairs[i] = (zone.start, zone.end);
                zone_types[i] = zone.zone_type;
            }
        }

        results.push(ScannedSection {
            name: name.clone(),
            index,
            header_line: info.header_line,
            ascii_line,
            numeric_line: info.numeric_line,
            db_values,
            strings,
            zone_pairs,
            zone_types,
            record_lines: info.total_lines(),
        });
    }

    Ok(results)
}

/// Filter scanned indexes by numeric identity or internal layout key.
pub fn filter_by_name<'a>(
    sections: &'a [ScannedSection],
    pattern: &str,
) -> Vec<&'a ScannedSection> {
    let lower_pat = pattern.to_lowercase();
    sections
        .iter()
        .filter(|section| {
            section.name.to_lowercase().contains(&lower_pat)
                || section.index.to_string().contains(&lower_pat)
                || format!("i{}", section.index).contains(&lower_pat)
        })
        .collect()
}

/// Filter scanned indexes by database value range.
pub fn filter_by_value<'a>(
    sections: &'a [ScannedSection],
    index: usize,
    min: &DecimalValue,
    max: &DecimalValue,
) -> Vec<&'a ScannedSection> {
    if index >= 9 {
        return Vec::new();
    }
    sections
        .iter()
        .filter(|s| &s.db_values[index] >= min && &s.db_values[index] <= max)
        .collect()
}

/// Build the deterministic read-only aggregate represented by a whole-index
/// reference. The aggregate contains the index identity, hex-word line, all
/// nine exact DB values, all three strings, and every non-empty defined zone.
pub fn aggregate_index_content(content: &str, section: &ScannedSection) -> Result<String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut values = vec![
        format!("index: {}", section.index),
        lines
            .get(section.ascii_line)
            .copied()
            .unwrap_or_default()
            .to_string(),
        section
            .db_values
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" | "),
    ];
    values.extend(section.strings.iter().cloned());
    for (start, end) in section.zone_pairs {
        if start == 0 && end == 0 {
            continue;
        }
        let start = start as usize;
        let end = end as usize;
        if start > end || end >= lines.len() {
            return Err(crate::RegeditedError::Parse(format!(
                "Index {} zone range {}-{} is outside {} lines",
                section.index,
                start,
                end,
                lines.len()
            )));
        }
        values.push(lines[start..=end].join("\n"));
    }
    Ok(values.join("\n"))
}

/// Filter scanned indexes by zone type.
pub fn filter_by_type(sections: &[ScannedSection], zt: ZoneType) -> Vec<&ScannedSection> {
    sections
        .iter()
        .filter(|s| s.zone_types.contains(&zt))
        .collect()
}

/// Filter scanned indexes by string content.
pub fn filter_by_string<'a>(
    sections: &'a [ScannedSection],
    index: usize,
    pattern: &str,
) -> Vec<&'a ScannedSection> {
    if index >= 3 {
        return Vec::new();
    }
    let lower = pattern.to_lowercase();
    sections
        .iter()
        .filter(|s| s.strings[index].to_lowercase().contains(&lower))
        .collect()
}

// ==================== FAST DIFF ====================

/// Result of comparing two Regedited files
#[derive(Debug, Clone)]
pub struct DiffResult {
    /// Sections only in file A
    pub only_in_a: Vec<String>,
    /// Sections only in file B
    pub only_in_b: Vec<String>,
    /// Sections with different database values
    pub changed_db: Vec<(String, [DecimalValue; 9], [DecimalValue; 9])>,
    /// Sections with different strings
    pub changed_strings: Vec<(String, [String; 3], [String; 3])>,
    /// Sections with different hex-word lines
    pub changed_ascii: Vec<(String, String, String)>,
    /// Sections with identical metadata
    pub identical: Vec<String>,
}

/// Fast diff between two Regedited files — like `diff` but metadata-only
///
/// Both files are scanned through memory maps while only fixed-record metadata
/// is retained in Rust-owned collections.
pub fn fast_diff(file_a: &Path, file_b: &Path) -> Result<DiffResult> {
    let scan_a = fast_scan(file_a)?;
    let scan_b = fast_scan(file_b)?;

    let map_a: BTreeMap<u64, &ScannedSection> = scan_a.iter().map(|s| (s.index, s)).collect();
    let map_b: BTreeMap<u64, &ScannedSection> = scan_b.iter().map(|s| (s.index, s)).collect();

    let mut only_in_a = Vec::new();
    let mut only_in_b = Vec::new();
    let mut changed_db = Vec::new();
    let mut changed_strings = Vec::new();
    let mut changed_ascii = Vec::new();
    let mut identical = Vec::new();

    // Check sections in A
    for (index, sec_a) in &map_a {
        let key = format!("index:{}", index);
        if let Some(sec_b) = map_b.get(index) {
            // Compare database values
            if sec_a.db_values != sec_b.db_values {
                changed_db.push((
                    key.clone(),
                    sec_a.db_values.clone(),
                    sec_b.db_values.clone(),
                ));
            }
            // Compare strings
            if sec_a.strings != sec_b.strings {
                changed_strings.push((key.clone(), sec_a.strings.clone(), sec_b.strings.clone()));
            }
            // Compare hex-word lines (compare zone pairs)
            if sec_a.zone_pairs != sec_b.zone_pairs || sec_a.zone_types != sec_b.zone_types {
                let ascii_a = format_ascii_diff(&sec_a.zone_pairs, &sec_a.zone_types);
                let ascii_b = format_ascii_diff(&sec_b.zone_pairs, &sec_b.zone_types);
                changed_ascii.push((key.clone(), ascii_a, ascii_b));
            }
            // Check if completely identical
            if sec_a.db_values == sec_b.db_values
                && sec_a.strings == sec_b.strings
                && sec_a.zone_pairs == sec_b.zone_pairs
                && sec_a.zone_types == sec_b.zone_types
            {
                identical.push(key);
            }
        } else {
            only_in_a.push(key);
        }
    }

    // Check sections only in B
    for index in map_b.keys() {
        if !map_a.contains_key(index) {
            only_in_b.push(format!("index:{}", index));
        }
    }

    Ok(DiffResult {
        only_in_a,
        only_in_b,
        changed_db,
        changed_strings,
        changed_ascii,
        identical,
    })
}

fn format_ascii_diff(pairs: &[(u32, u32); 3], types: &[ZoneType; 3]) -> String {
    use crate::zone_type::encode_hex_word;
    let mut parts = Vec::new();
    for i in 0..3 {
        parts.push(encode_hex_word(pairs[i].0, types[i]));
        parts.push(encode_hex_word(pairs[i].1, types[i]));
    }
    parts.join(" : ")
}

impl DiffResult {
    pub fn has_changes(&self) -> bool {
        !self.only_in_a.is_empty()
            || !self.only_in_b.is_empty()
            || !self.changed_db.is_empty()
            || !self.changed_strings.is_empty()
            || !self.changed_ascii.is_empty()
    }

    pub fn summary(&self) -> String {
        let mut lines = vec!["Diff Summary:".to_string()];
        lines.push(format!("  Only in A: {}", self.only_in_a.len()));
        lines.push(format!("  Only in B: {}", self.only_in_b.len()));
        lines.push(format!("  Changed DB: {}", self.changed_db.len()));
        lines.push(format!("  Changed strings: {}", self.changed_strings.len()));
        lines.push(format!(
            "  Changed hex-word line: {}",
            self.changed_ascii.len()
        ));
        lines.push(format!("  Identical: {}", self.identical.len()));
        lines.join("\n")
    }

    pub fn display(&self) -> String {
        let mut lines = vec![self.summary()];

        if !self.only_in_a.is_empty() {
            lines.push("\n  Only in A:".to_string());
            for name in &self.only_in_a {
                lines.push(format!("    - {}", name));
            }
        }

        if !self.only_in_b.is_empty() {
            lines.push("\n  Only in B:".to_string());
            for name in &self.only_in_b {
                lines.push(format!("    + {}", name));
            }
        }

        if !self.changed_db.is_empty() {
            lines.push("\n  Changed database values:".to_string());
            for (name, a, b) in &self.changed_db {
                let a_str: Vec<String> = a.iter().map(|v| v.to_string()).collect();
                let b_str: Vec<String> = b.iter().map(|v| v.to_string()).collect();
                lines.push(format!("    {}:", name));
                lines.push(format!("      A: {}", a_str.join("\t")));
                lines.push(format!("      B: {}", b_str.join("\t")));
            }
        }

        if !self.changed_ascii.is_empty() {
            lines.push("\n  Changed hex-word lines:".to_string());
            for (name, a, b) in &self.changed_ascii {
                lines.push(format!("    {}:", name));
                lines.push(format!("      A: {}", a));
                lines.push(format!("      B: {}", b));
            }
        }

        lines.join("\n")
    }
}

// ==================== FAST REPLACE (SAFETENSORS-STYLE) ====================

/// Replace numeric indexes from a source file into a target file
///
/// Like safetensors' tensor replacement: find matching numeric registry
/// indexes, copy their metadata (index, hex-word line, DB values, strings)
/// from source to target, and leave unmatched indexes untouched.
///
/// # Example
///
/// ```no_run
/// # use std::path::Path;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use regedited::fast_ops::fast_replace;
/// // Replace all matching indexes from patched.md into base.md
/// fast_replace(Path::new("base.md"), Path::new("patched.md"), None)?;
/// # Ok(())
/// # }
/// ```
pub fn fast_replace(
    target_path: &Path,
    source_path: &Path,
    section_names: Option<&[String]>,
) -> Result<String> {
    let target_content = std::fs::read_to_string(target_path)?;
    let source_content = std::fs::read_to_string(source_path)?;

    let target_scan = fast_scan_content(&target_content)?;
    let source_scan = fast_scan_content(&source_content)?;

    let source_map: BTreeMap<u64, &ScannedSection> = source_scan
        .iter()
        .map(|section| (section.index, section))
        .collect();

    let mut result = target_content.clone();

    for sec_target in &target_scan {
        let should_replace = match section_names {
            Some(names) => names.iter().any(|reference| {
                crate::header::parse_index_reference(reference) == Some(sec_target.index)
                    || reference.eq_ignore_ascii_case(&sec_target.name)
            }),
            None => source_map.contains_key(&sec_target.index),
        };

        if !should_replace {
            continue;
        }

        if let Some(sec_source) = source_map.get(&sec_target.index) {
            // Replace index line
            let idx_line = sec_target.header_line + 1;
            let new_index = format!("index: {}", sec_source.index);
            result = crate::header::update_line(&result, idx_line, &new_index)?;

            // Replace hex-word line
            let ascii_line = sec_target.header_line + 2;
            let new_ascii = format_ascii_diff(&sec_source.zone_pairs, &sec_source.zone_types);
            result = crate::header::update_line(&result, ascii_line, &new_ascii)?;

            // Replace numeric line
            let new_numeric: Vec<String> =
                sec_source.db_values.iter().map(|v| v.to_string()).collect();
            result = crate::header::update_line(
                &result,
                sec_target.numeric_line,
                &new_numeric.join(" | "),
            )?;

            // Replace 3 string lines
            for i in 0..3 {
                let line_idx = sec_target.numeric_line + 1 + i;
                result = crate::header::update_line(&result, line_idx, &sec_source.strings[i])?;
            }
        }
    }

    Ok(result)
}

/// Compatibility entry point for replacing complete index records.
///
/// An index owns only its fixed six structured lines. Zone payloads are
/// absolute file ranges and are never implicitly replaced by this operation.
pub fn fast_replace_content(
    target_path: &Path,
    source_path: &Path,
    index_refs: Option<&[String]>,
) -> Result<String> {
    fast_replace(target_path, source_path, index_refs)
}

// ==================== FAST GREP (RIPGREP-STYLE) ====================

/// Memory-mapped line grep — ripgrep-style fast search
///
/// Uses a memory-mapped file and retains only matching lines as owned strings.
pub fn fast_grep(file_path: &Path, pattern: &str) -> Result<Vec<(usize, String)>> {
    let mmap = MmapFile::open(file_path)?;
    Ok(grep_content(mmap.as_str(), pattern))
}

/// In-memory line grep used by native files and browser/Wasm callers.
pub fn grep_content(content: &str, pattern: &str) -> Vec<(usize, String)> {
    let lower_pattern = pattern.to_lowercase();
    let mut matches = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        if line.to_lowercase().contains(&lower_pattern) {
            matches.push((line_num, line.to_string()));
        }
    }

    matches
}

/// Index-qualified grep.
///
/// Resolving the index validates the caller's context; the search still spans
/// the full file because indexes do not own document sections.
pub fn fast_grep_section(
    file_path: &Path,
    section_name: &str,
    pattern: &str,
) -> Result<Vec<(usize, String)>> {
    let mmap = MmapFile::open(file_path)?;
    grep_content_section(mmap.as_str(), section_name, pattern)
}

/// In-memory index-qualified grep with the same matching rules as native grep.
/// The index is validated first; search spans the shared document.
pub fn grep_content_section(
    content: &str,
    section_name: &str,
    pattern: &str,
) -> Result<Vec<(usize, String)>> {
    let header = scan_content(content)?;

    header.resolve_section(section_name)?;
    Ok(grep_content(content, pattern))
}

/// Multi-pattern grep — search for any of multiple patterns (OR logic)
pub fn fast_grep_multi(
    file_path: &Path,
    patterns: &[String],
) -> Result<Vec<(usize, String, Vec<String>)>> {
    let content = std::fs::read_to_string(file_path)?;
    Ok(grep_content_multi(&content, patterns))
}

/// In-memory multi-pattern grep used by native files and browser/Wasm callers.
pub fn grep_content_multi(content: &str, patterns: &[String]) -> Vec<(usize, String, Vec<String>)> {
    let lower_patterns: Vec<String> = patterns.iter().map(|p| p.to_lowercase()).collect();
    let mut matches = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        let lower_line = line.to_lowercase();
        let matched: Vec<String> = lower_patterns
            .iter()
            .filter(|p| lower_line.contains(*p))
            .cloned()
            .collect();
        if !matched.is_empty() {
            matches.push((line_num, line.to_string(), matched));
        }
    }

    matches
}

// ==================== DISPLAY HELPERS ====================

impl ScannedSection {
    /// Compact display for scan output
    pub fn display_compact(&self) -> String {
        let db_str: Vec<String> = self.db_values.iter().map(|v| v.to_string()).collect();
        let active_zones: Vec<String> = self
            .zone_pairs
            .iter()
            .enumerate()
            .filter(|(_, (s, e))| *s != 0 || *e != 0)
            .map(|(i, (s, e))| {
                let tag = self.zone_types[i].short();
                format!("Z{}:{}..{}[{}]", i, s, e, tag)
            })
            .collect();

        format!(
            "  [{:>4}] {:<20} DB:[{}] Zones:[{}] Record:{}",
            self.index,
            self.name,
            db_str.join(" "),
            if active_zones.is_empty() {
                "none".to_string()
            } else {
                active_zones.join(" ")
            },
            self.record_lines,
        )
    }

    /// Full display with all metadata
    pub fn display_full(&self) -> String {
        let mut lines = vec![
            format!("=== [{}] {} ===", self.index, self.name),
            format!("  Header @ line {}", self.header_line),
            format!("  DB: {:?}", &self.db_values[..]),
        ];
        for (i, s) in self.strings.iter().enumerate() {
            if !s.is_empty() {
                lines.push(format!(
                    "  Str{}: \"{}\"",
                    i,
                    s.chars().take(60).collect::<String>()
                ));
            }
        }
        for (i, ((start, end), zt)) in self
            .zone_pairs
            .iter()
            .zip(self.zone_types.iter())
            .enumerate()
        {
            if *start != 0 || *end != 0 {
                use crate::zone_type::encode_hex_word;
                lines.push(format!(
                    "  Zone{}: {} : {} → lines {}-{} [{}]",
                    i,
                    encode_hex_word(*start, *zt),
                    encode_hex_word(*end, *zt),
                    start,
                    end,
                    zt.short()
                ));
            }
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_doc() -> String {
        r#"# Test

regedited open
index: 100
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9
alpha str1
alpha str2
alpha str3
Alpha content here.

regedited open
index: 200
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
10 | 20 | 30 | 40 | 50 | 60 | 70 | 80 | 90
beta str1
beta str2
beta str3
Beta content here.
More beta.
"#
        .to_string()
    }

    #[test]
    fn test_fast_scan() {
        let doc = test_doc();
        let scanned = fast_scan_content(&doc).unwrap();
        assert_eq!(scanned.len(), 2);
        assert_eq!(scanned[0].name, "index:100");
        assert_eq!(scanned[0].index, 100);
        assert_eq!(
            scanned[0].db_values,
            [1, 2, 3, 4, 5, 6, 7, 8, 9].map(DecimalValue::from)
        );
        assert_eq!(scanned[1].name, "index:200");
        assert_eq!(scanned[1].index, 200);
    }

    #[test]
    fn test_filter_by_name() {
        let doc = test_doc();
        let scanned = fast_scan_content(&doc).unwrap();
        let filtered = filter_by_name(&scanned, "100");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "index:100");
    }

    #[test]
    fn test_filter_by_value() {
        let doc = test_doc();
        let scanned = fast_scan_content(&doc).unwrap();
        let min = DecimalValue::from(5);
        let max = DecimalValue::from(50);
        let filtered = filter_by_value(&scanned, 0, &min, &max);
        assert_eq!(filtered.len(), 1); // Only Beta(10) in range [5,50]; Alpha(1) is below
    }

    #[test]
    fn whole_index_aggregate_contains_only_its_metadata_strings_and_zones() {
        let doc = "zone alpha\nzone beta\nother index zone\nanything regedited open anywhere\nindex: 8\n1x0000000 : 1x0000001 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000\n0.125 | -1.5 | 2 | 3 | 4 | 5 | 6 | 7 | 9007199254740993\nfirst string\nsecond string\nthird string\n";
        let scanned = fast_scan_content(doc).unwrap();
        let aggregate = aggregate_index_content(doc, &scanned[0]).unwrap();
        assert!(aggregate.contains("index: 8"));
        assert!(aggregate.contains("0.125 | -1.5"));
        assert!(aggregate.contains("first string"));
        assert!(aggregate.contains("zone alpha\nzone beta"));
        assert!(!aggregate.contains("other index zone"));
        assert!(!aggregate.contains("regedited open"));
    }

    #[test]
    fn test_fast_grep() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), test_doc()).unwrap();
        let matches = fast_grep(tmp.path(), "content").unwrap();
        assert_eq!(matches.len(), 2); // "Alpha content" and "Beta content"
    }

    #[test]
    fn test_fast_grep_section() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), test_doc()).unwrap();
        let matches = fast_grep_section(tmp.path(), "i100", "content").unwrap();
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_fast_grep_multi() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), test_doc()).unwrap();
        let matches =
            fast_grep_multi(tmp.path(), &["Alpha".to_string(), "Beta".to_string()]).unwrap();
        // Each line containing either Alpha or Beta
        assert!(matches.len() >= 2);
    }

    #[test]
    fn in_memory_grep_matches_native_file_grep() {
        let content = test_doc();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), &content).unwrap();

        assert_eq!(
            grep_content(&content, "CONTENT"),
            fast_grep(tmp.path(), "CONTENT").unwrap()
        );
        assert_eq!(
            grep_content_section(&content, "i100", "content").unwrap(),
            fast_grep_section(tmp.path(), "i100", "content").unwrap()
        );
        let patterns = ["Alpha".to_string(), "Beta".to_string()];
        assert_eq!(
            grep_content_multi(&content, &patterns),
            fast_grep_multi(tmp.path(), &patterns).unwrap()
        );
    }

    #[test]
    fn test_diff() {
        let tmp_a = tempfile::NamedTempFile::new().unwrap();
        let tmp_b = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp_a.path(), test_doc()).unwrap();

        // B has different DB values for Alpha
        let mut doc_b = test_doc();
        doc_b = doc_b.replace(
            "1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9",
            "99 | 88 | 77 | 66 | 55 | 44 | 33 | 22 | 11",
        );
        std::fs::write(tmp_b.path(), doc_b).unwrap();

        let diff = fast_diff(tmp_a.path(), tmp_b.path()).unwrap();
        assert!(diff.has_changes());
        assert_eq!(diff.changed_db.len(), 1);
        assert_eq!(diff.identical.len(), 1); // Beta unchanged
    }

    #[test]
    fn test_fast_replace() {
        let tmp_base = tempfile::NamedTempFile::new().unwrap();
        let tmp_patch = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp_base.path(), test_doc()).unwrap();

        // Patch has different values
        let mut doc_patch = test_doc();
        doc_patch = doc_patch.replace(
            "1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9",
            "99 | 88 | 77 | 66 | 55 | 44 | 33 | 22 | 11",
        );
        std::fs::write(tmp_patch.path(), doc_patch).unwrap();

        let result = fast_replace(tmp_base.path(), tmp_patch.path(), None).unwrap();
        assert!(result.contains("99 | 88 | 77"));
        assert!(result.contains("alpha str1")); // Strings preserved from patch
    }

    #[test]
    fn diff_and_replace_join_by_numeric_index_despite_marker_wrappers() {
        let tmp_base = tempfile::NamedTempFile::new().unwrap();
        let tmp_patch = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp_base.path(), test_doc()).unwrap();

        let patch = test_doc()
            .replacen("regedited open", "prefixregedited opensuffix", 1)
            .replace(
                "1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9",
                "99 | 88 | 77 | 66 | 55 | 44 | 33 | 22 | 11",
            );
        std::fs::write(tmp_patch.path(), patch).unwrap();

        let diff = fast_diff(tmp_base.path(), tmp_patch.path()).unwrap();
        assert_eq!(diff.changed_db.len(), 1);
        assert!(diff.changed_db[0].0.contains("index:100"));

        let replaced = fast_replace(tmp_base.path(), tmp_patch.path(), None).unwrap();
        assert!(replaced.contains("99 | 88 | 77"));
        assert!(replaced.contains("regedited open"));
    }

    #[test]
    fn full_content_replace_is_a_metadata_only_compatibility_alias() {
        let tmp_base = tempfile::NamedTempFile::new().unwrap();
        let tmp_patch = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp_base.path(), test_doc()).unwrap();

        let patch = test_doc().replace("Alpha content here.", "Patched alpha body.");
        std::fs::write(tmp_patch.path(), patch).unwrap();

        let replaced = fast_replace_content(tmp_base.path(), tmp_patch.path(), None).unwrap();
        assert!(replaced.contains("Alpha content here."));
        assert!(!replaced.contains("Patched alpha body."));
    }

    #[test]
    fn test_scanned_section_display() {
        let doc = test_doc();
        let scanned = fast_scan_content(&doc).unwrap();
        let display = scanned[0].display_compact();
        assert!(display.contains("index:100"));
        assert!(display.contains("100"));
    }
}
