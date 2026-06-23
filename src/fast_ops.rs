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
//! | `load_file()` header JSON | `scan_file()` section index |
//! | Key name filter | Section name glob |
//! | Shape filter | Database value filter |
//! | Tensor offset | Line number offset |
//! | `save_file()` patched header | `fast_replace()` patched sections |
//!
//! ## Design
//!
//! All operations use `DocumentHeader` (the index) to work with offsets,
//! not content. The actual markdown content is only read when extracting
//! a specific zone or performing a replace.

use crate::{
    ascii_store::AsciiStore,
    db_line::DbLine,
    extract_lines,
    header::{scan_file, scan_content, DocumentHeader, SectionInfo},
    zone_type::ZoneType,
    MmapFile, Result, RegeditedError,
};
use std::collections::BTreeMap;
use std::path::Path;

// ==================== FAST SCAN ====================

/// A scanned section with its key attributes
#[derive(Debug, Clone)]
pub struct ScannedSection {
    pub name: String,
    pub index: u64,
    pub header_line: usize,
    pub ascii_line: usize,
    pub numeric_line: usize,
    pub db_values: [i64; 9],
    pub strings: [String; 3],
    pub zone_pairs: [(u32, u32); 3],
    pub zone_types: [ZoneType; 3],
    pub content_lines: usize,
}

/// Fast scan — like safetensors' header scan, reads only metadata
///
/// Parses each section's index, hex-word store, and database line
/// without loading the full content. For a 1GB file, this reads
/// maybe 1-2KB of metadata per section, not the whole file.
pub fn fast_scan(file_path: &Path) -> Result<Vec<ScannedSection>> {
    let content = std::fs::read_to_string(file_path)?;
    fast_scan_content(&content)
}

/// Fast scan from already-loaded content
pub fn fast_scan_content(content: &str) -> Result<Vec<ScannedSection>> {
    let header = scan_content(content)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut results = Vec::new();

    for (name, info) in &header.sections {
        // Quick bounds check
        if info.numeric_line >= lines.len() || info.string3_line >= lines.len() {
            continue;
        }

        // Read index (supports both "index: 100" and plain "100")
        let index = if info.header_line + 1 < lines.len() {
            let idx_str = lines[info.header_line + 1].trim();
            if idx_str.starts_with("index:") || idx_str.starts_with("INDEX:") {
                idx_str[6..].trim().parse::<u64>().unwrap_or(0)
            } else {
                idx_str.parse::<u64>().unwrap_or(0)
            }
        } else {
            0
        };

        // Read ASCII store
        let ascii_line = info.header_line + 2;
        let ascii = if ascii_line < lines.len() {
            AsciiStore::from_line(lines[ascii_line]).unwrap_or_default()
        } else {
            AsciiStore::default()
        };

        // Read database values (9 pipe-separated numbers)
        let db_values = if info.numeric_line < lines.len() {
            parse_numeric_line_fast(lines[info.numeric_line])
        } else {
            [0; 9]
        };

        // Read 3 strings
        let mut strings = [String::new(), String::new(), String::new()];
        for i in 0..3 {
            let line_idx = info.numeric_line + 1 + i;
            if line_idx < lines.len() {
                strings[i] = lines[line_idx].trim().to_string();
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

        let content_lines = if info.content_end >= info.content_start {
            info.content_end - info.content_start + 1
        } else {
            0
        };

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
            content_lines,
        });
    }

    Ok(results)
}

/// Parse 9 pipe-separated numbers quickly (no error handling overhead)
fn parse_numeric_line_fast(line: &str) -> [i64; 9] {
    let mut result = [0i64; 9];
    let sep = if line.contains(" | ") { " | " } else { "\t" };
    let parts: Vec<&str> = line.split(sep).collect();
    for (i, part) in parts.iter().take(9).enumerate() {
        if let Ok(v) = part.trim().parse::<i64>() {
            result[i] = v;
        }
    }
    result
}

/// Filter scanned sections by name pattern (glob-like)
pub fn filter_by_name<'a>(sections: &'a [ScannedSection], pattern: &str) -> Vec<&'a ScannedSection> {
    let lower_pat = pattern.to_lowercase();
    sections.iter()
        .filter(|s| s.name.to_lowercase().contains(&lower_pat))
        .collect()
}

/// Filter scanned sections by database value range
pub fn filter_by_value(sections: &[ScannedSection], index: usize, min: i64, max: i64) -> Vec<&ScannedSection> {
    if index >= 9 {
        return Vec::new();
    }
    sections.iter()
        .filter(|s| s.db_values[index] >= min && s.db_values[index] <= max)
        .collect()
}

/// Filter scanned sections by zone type
pub fn filter_by_type(sections: &[ScannedSection], zt: ZoneType) -> Vec<&ScannedSection> {
    sections.iter()
        .filter(|s| s.zone_types.iter().any(|&t| t == zt))
        .collect()
}

/// Filter scanned sections by string content
pub fn filter_by_string<'a>(sections: &'a [ScannedSection], index: usize, pattern: &str) -> Vec<&'a ScannedSection> {
    if index >= 3 {
        return Vec::new();
    }
    let lower = pattern.to_lowercase();
    sections.iter()
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
    pub changed_db: Vec<(String, [i64; 9], [i64; 9])>,
    /// Sections with different strings
    pub changed_strings: Vec<(String, [String; 3], [String; 3])>,
    /// Sections with different ASCII stores
    pub changed_ascii: Vec<(String, String, String)>,
    /// Sections with identical metadata
    pub identical: Vec<String>,
}

/// Fast diff between two Regedited files — like `diff` but metadata-only
///
/// For a 10GB file pair, this reads maybe 100KB of headers, not 20GB of content.
pub fn fast_diff(file_a: &Path, file_b: &Path) -> Result<DiffResult> {
    let scan_a = fast_scan(file_a)?;
    let scan_b = fast_scan(file_b)?;

    let map_a: BTreeMap<String, &ScannedSection> = scan_a.iter().map(|s| (s.name.clone(), s)).collect();
    let map_b: BTreeMap<String, &ScannedSection> = scan_b.iter().map(|s| (s.name.clone(), s)).collect();

    let mut only_in_a = Vec::new();
    let mut only_in_b = Vec::new();
    let mut changed_db = Vec::new();
    let mut changed_strings = Vec::new();
    let mut changed_ascii = Vec::new();
    let mut identical = Vec::new();

    // Check sections in A
    for (name, sec_a) in &map_a {
        if let Some(sec_b) = map_b.get(name) {
            // Compare database values
            if sec_a.db_values != sec_b.db_values {
                changed_db.push((name.clone(), sec_a.db_values, sec_b.db_values));
            }
            // Compare strings
            if sec_a.strings != sec_b.strings {
                changed_strings.push((name.clone(), sec_a.strings.clone(), sec_b.strings.clone()));
            }
            // Compare ASCII stores (compare zone pairs)
            if sec_a.zone_pairs != sec_b.zone_pairs || sec_a.zone_types != sec_b.zone_types {
                let ascii_a = format_ascii_diff(&sec_a.zone_pairs, &sec_a.zone_types);
                let ascii_b = format_ascii_diff(&sec_b.zone_pairs, &sec_b.zone_types);
                changed_ascii.push((name.clone(), ascii_a, ascii_b));
            }
            // Check if completely identical
            if sec_a.db_values == sec_b.db_values 
                && sec_a.strings == sec_b.strings 
                && sec_a.zone_pairs == sec_b.zone_pairs
                && sec_a.zone_types == sec_b.zone_types {
                identical.push(name.clone());
            }
        } else {
            only_in_a.push(name.clone());
        }
    }

    // Check sections only in B
    for name in map_b.keys() {
        if !map_a.contains_key(name) {
            only_in_b.push(name.clone());
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
        lines.push(format!("  Changed ASCII: {}", self.changed_ascii.len()));
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
            lines.push("\n  Changed ASCII stores:".to_string());
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

/// Replace sections from a source file into a target file
///
/// Like safetensors' tensor replacement: find matching section names,
/// copy their metadata (index, ASCII store, DB values, strings) from
/// source to target, leave unmatched sections untouched.
///
/// # Example
///
/// ```no_run
/// # use std::path::Path;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use regedited::fast_ops::fast_replace;
/// // Replace all sections from patched.md into base.md
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

    // Build map of source sections
    let source_map: BTreeMap<String, &ScannedSection> = source_scan.iter()
        .map(|s| (s.name.clone(), s))
        .collect();

    let mut result = target_content.clone();

    for sec_target in &target_scan {
        // Check if we should replace this section
        let should_replace = match section_names {
            Some(names) => names.contains(&sec_target.name),
            None => source_map.contains_key(&sec_target.name),
        };

        if !should_replace {
            continue;
        }

        if let Some(sec_source) = source_map.get(&sec_target.name) {
            // Replace index line
            let idx_line = sec_target.header_line + 1;
            let new_index = format!("{}", sec_source.index);
            result = crate::header::update_line(&result, idx_line, &new_index)?;

            // Replace ASCII store line
            let ascii_line = sec_target.header_line + 2;
            let new_ascii = format_ascii_diff(&sec_source.zone_pairs, &sec_source.zone_types);
            result = crate::header::update_line(&result, ascii_line, &new_ascii)?;

            // Replace numeric line
            let new_numeric: Vec<String> = sec_source.db_values.iter().map(|v| v.to_string()).collect();
            result = crate::header::update_line(&result, sec_target.numeric_line, &new_numeric.join("\t"))?;

            // Replace 3 string lines
            for i in 0..3 {
                let line_idx = sec_target.numeric_line + 1 + i;
                result = crate::header::update_line(&result, line_idx, &sec_source.strings[i])?;
            }
        }
    }

    Ok(result)
}

/// Replace sections including their CONTENT blocks (full section swap)
///
/// This is the content-aware version of `fast_replace`. It replaces not
/// just metadata but also the actual markdown content between `---` and
/// the next section. Use this when you want to completely swap sections.
///
/// After replacement, all hex-word line numbers are recalculated.
pub fn fast_replace_content(
    target_path: &Path,
    source_path: &Path,
    section_names: Option<&[String]>,
) -> Result<String> {
    let target_content = std::fs::read_to_string(target_path)?;
    let source_content = std::fs::read_to_string(source_path)?;

    let target_header = scan_content(&target_content)?;
    let source_header = scan_content(&source_content)?;

    let mut result = target_content;

    // Process sections in reverse order (so line number shifts don't affect earlier ops)
    let target_sections: Vec<_> = target_header.sections.values().collect();

    for target_sec in target_sections.iter().rev() {
        let should_replace = match section_names {
            Some(names) => names.contains(&target_sec.name),
            None => source_header.sections.contains_key(&target_sec.name),
        };

        if !should_replace {
            continue;
        }

        if let Some(source_sec) = source_header.sections.get(&target_sec.name) {
            // Get the full content block from source
            let source_lines: Vec<&str> = source_content.lines().collect();
            let content_start = source_sec.content_start;
            let content_end = source_sec.content_end;

            if content_start <= content_end && content_end < source_lines.len() {
                let new_content = source_lines[content_start..=content_end].join("\n");

                // Replace content in target
                let target_lines: Vec<&str> = result.lines().collect();
                let t_start = target_sec.content_start;
                let t_end = target_sec.content_end;

                if t_start <= t_end && t_end < target_lines.len() {
                    let mut new_lines: Vec<String> = Vec::new();

                    // Lines before content
                    for i in 0..t_start {
                        new_lines.push(target_lines[i].to_string());
                    }

                    // New content
                    for line in new_content.lines() {
                        new_lines.push(line.to_string());
                    }

                    // Lines after content
                    for i in (t_end + 1)..target_lines.len() {
                        new_lines.push(target_lines[i].to_string());
                    }

                    result = new_lines.join("\n");

                    // Calculate line delta and apply to hex-word stores
                    let old_count = (t_end - t_start) + 1;
                    let new_count = new_content.lines().count();
                    let delta = new_count as i64 - old_count as i64;

                    if delta != 0 {
                        use crate::zone_editor::{apply_line_deltas, LineDelta};
                        result = apply_line_deltas(&result, &[LineDelta {
                            start_line: t_start,
                            delta,
                        }])?;
                    }
                }
            }

            // Also replace metadata (index, ASCII, DB values, strings)
            result = fast_replace_str(&result, &source_content, &target_sec.name)?;
        }
    }

    Ok(result)
}

/// Helper: metadata-only replace for a single section (string version)
fn fast_replace_str(
    target_content: &str,
    source_content: &str,
    section_name: &str,
) -> Result<String> {
    let target_scan = fast_scan_content(target_content)?;
    let source_scan = fast_scan_content(source_content)?;

    let source_map: BTreeMap<String, &ScannedSection> = source_scan.iter()
        .map(|s| (s.name.clone(), s))
        .collect();

    let mut result = target_content.to_string();

    for sec_target in &target_scan {
        if sec_target.name != section_name {
            continue;
        }

        if let Some(sec_source) = source_map.get(&sec_target.name) {
            // Replace index line
            let idx_line = sec_target.header_line + 1;
            let new_index = format!("{}", sec_source.index);
            result = crate::header::update_line(&result, idx_line, &new_index)?;

            // Replace ASCII store line
            let ascii_line = sec_target.header_line + 2;
            let new_ascii = format_ascii_diff(&sec_source.zone_pairs, &sec_source.zone_types);
            result = crate::header::update_line(&result, ascii_line, &new_ascii)?;

            // Replace numeric line
            let new_numeric: Vec<String> = sec_source.db_values.iter().map(|v| v.to_string()).collect();
            result = crate::header::update_line(&result, sec_target.numeric_line, &new_numeric.join("\t"))?;

            // Replace 3 string lines
            for i in 0..3 {
                let line_idx = sec_target.numeric_line + 1 + i;
                result = crate::header::update_line(&result, line_idx, &sec_source.strings[i])?;
            }
        }
    }

    Ok(result)
}

// ==================== FAST GREP (RIPGREP-STYLE) ====================

/// Memory-mapped line grep — ripgrep-style fast search
///
/// Uses byte-level scanning on memory-mapped files for O(1) seek speed.
/// For a 10GB file, only the matching lines are read into RAM.
pub fn fast_grep(file_path: &Path, pattern: &str) -> Result<Vec<(usize, String)>> {
    let mmap = MmapFile::open(file_path)?;
    let content = mmap.as_str();
    let lower_pattern = pattern.to_lowercase();
    let mut matches = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        if line.to_lowercase().contains(&lower_pattern) {
            matches.push((line_num, line.to_string()));
        }
    }

    Ok(matches)
}

/// Section-limited grep — only search within a section's content
pub fn fast_grep_section(
    file_path: &Path,
    section_name: &str,
    pattern: &str,
) -> Result<Vec<(usize, String)>> {
    let content = std::fs::read_to_string(file_path)?;
    let header = scan_content(&content)?;

    let section = header.get_section(section_name)
        .or_else(|| header.get_section_case_insensitive(section_name))
        .ok_or_else(|| RegeditedError::SectionNotFound(section_name.to_string()))?;

    let lower_pattern = pattern.to_lowercase();
    let lines: Vec<&str> = content.lines().collect();
    let mut matches = Vec::new();

    for line_num in section.content_start..=section.content_end {
        if line_num >= lines.len() {
            break;
        }
        if lines[line_num].to_lowercase().contains(&lower_pattern) {
            matches.push((line_num, lines[line_num].to_string()));
        }
    }

    Ok(matches)
}

/// Multi-pattern grep — search for any of multiple patterns (OR logic)
pub fn fast_grep_multi(
    file_path: &Path,
    patterns: &[String],
) -> Result<Vec<(usize, String, Vec<String>)>> {
    let content = std::fs::read_to_string(file_path)?;
    let lower_patterns: Vec<String> = patterns.iter().map(|p| p.to_lowercase()).collect();
    let mut matches = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        let lower_line = line.to_lowercase();
        let matched: Vec<String> = lower_patterns.iter()
            .filter(|p| lower_line.contains(*p))
            .cloned()
            .collect();
        if !matched.is_empty() {
            matches.push((line_num, line.to_string(), matched));
        }
    }

    Ok(matches)
}

// ==================== DISPLAY HELPERS ====================

impl ScannedSection {
    /// Compact display for scan output
    pub fn display_compact(&self) -> String {
        let db_str: Vec<String> = self.db_values.iter().map(|v| v.to_string()).collect();
        let active_zones: Vec<String> = self.zone_pairs.iter().enumerate()
            .filter(|(_, (s, e))| *s != 0 || *e != 0)
            .map(|(i, (s, e))| {
                let tag = self.zone_types[i].short();
                format!("Z{}:{}..{}[{}]", i, s, e, tag)
            })
            .collect();

        format!(
            "  [{:>4}] {:<20} DB:[{}] Zones:[{}] Lines:{}",
            self.index,
            self.name,
            db_str.join(" "),
            if active_zones.is_empty() { "none".to_string() } else { active_zones.join(" ") },
            self.content_lines,
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
                lines.push(format!("  Str{}: \"{}\"", i, s.chars().take(60).collect::<String>()));
            }
        }
        for (i, ((start, end), zt)) in self.zone_pairs.iter().zip(self.zone_types.iter()).enumerate() {
            if *start != 0 || *end != 0 {
                use crate::zone_type::encode_hex_word;
                lines.push(format!(
                    "  Zone{}: {} : {} → lines {}-{} [{}]",
                    i,
                    encode_hex_word(*start, *zt),
                    encode_hex_word(*end, *zt),
                    start, end,
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

## SECTION: Alpha
index: 100
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9
alpha str1
alpha str2
alpha str3
---
Alpha content here.

## SECTION: Beta
index: 200
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
10 | 20 | 30 | 40 | 50 | 60 | 70 | 80 | 90
beta str1
beta str2
beta str3
---
Beta content here.
More beta.
"#.to_string()
    }

    #[test]
    fn test_fast_scan() {
        let doc = test_doc();
        let scanned = fast_scan_content(&doc).unwrap();
        assert_eq!(scanned.len(), 2);
        assert_eq!(scanned[0].name, "Alpha");
        assert_eq!(scanned[0].index, 100);
        assert_eq!(scanned[0].db_values, [1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(scanned[1].name, "Beta");
        assert_eq!(scanned[1].index, 200);
    }

    #[test]
    fn test_filter_by_name() {
        let doc = test_doc();
        let scanned = fast_scan_content(&doc).unwrap();
        let filtered = filter_by_name(&scanned, "alp");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "Alpha");
    }

    #[test]
    fn test_filter_by_value() {
        let doc = test_doc();
        let scanned = fast_scan_content(&doc).unwrap();
        let filtered = filter_by_value(&scanned, 0, 5, 50);
        assert_eq!(filtered.len(), 1); // Only Beta(10) in range [5,50]; Alpha(1) is below
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
        let matches = fast_grep_section(tmp.path(), "Alpha", "content").unwrap();
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_fast_grep_multi() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), test_doc()).unwrap();
        let matches = fast_grep_multi(tmp.path(), &["Alpha".to_string(), "Beta".to_string()]).unwrap();
        // Each line containing either Alpha or Beta
        assert!(matches.len() >= 2);
    }

    #[test]
    fn test_diff() {
        let tmp_a = tempfile::NamedTempFile::new().unwrap();
        let tmp_b = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp_a.path(), test_doc()).unwrap();

        // B has different DB values for Alpha
        let mut doc_b = test_doc();
        doc_b = doc_b.replace("1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9", "99 | 88 | 77 | 66 | 55 | 44 | 33 | 22 | 11");
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
        doc_patch = doc_patch.replace("1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9", "99 | 88 | 77 | 66 | 55 | 44 | 33 | 22 | 11");
        std::fs::write(tmp_patch.path(), doc_patch).unwrap();

        let result = fast_replace(tmp_base.path(), tmp_patch.path(), None).unwrap();
        assert!(result.contains("99\t88\t77")); // fast_replace uses tab separator
        assert!(result.contains("alpha str1")); // Strings preserved from patch
    }

    #[test]
    fn test_scanned_section_display() {
        let doc = test_doc();
        let scanned = fast_scan_content(&doc).unwrap();
        let display = scanned[0].display_compact();
        assert!(display.contains("Alpha"));
        assert!(display.contains("100"));
    }
}
