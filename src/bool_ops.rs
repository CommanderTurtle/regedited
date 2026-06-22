// SPDX-License-Identifier: AGPL-3.0
//! # Boolean Operations — If-Then Content Matching
//!
//! Inspired by the user's CMD boolean macros using `find /I` with `&&` and `||`:
//!
//! ```cmd
//! :: Boolean AND:  contains AND must
//! find /I "contains" file && find /I "must" file && echo TRUE || echo FALSE
//!
//! :: Boolean NAND: contains AND must NOT
//! find /I "contains" file && find /I "must" file >nul && echo FALSE || echo TRUE
//!
//! :: Boolean OR:   contains OR must (one must exist)
//! find /I "contains" file >nul || find /I "must" file >nul && echo TRUE || echo FALSE
//! ```
//!
//! Regedited provides the same boolean logic on content blocks, sections, and files.
//!
//! ## Usage
//!
//! ```bash
//! # If section contains pattern, then extract zone
//! regedited if-contains doc.md MySection "pattern" --then-exec "regedited zone-extract doc.md MySection 1"
//!
//! # Boolean AND: section must contain BOTH patterns
//! regedited bool-and doc.md MySection "rust" "fn"
//!
//! # Boolean NAND: contains first but NOT second
//! regedited bool-nand doc.md MySection "fn" "python"
//!
//! # Boolean OR: contains ANY of the patterns
//! regedited bool-or doc.md MySection "rust" "python" "go"
//!
//! # Count occurrences of pattern
//! regedited count doc.md MySection "fn"
//!
//! # Check if zone is empty
//! regedited is-empty doc.md MySection 1
//! ```

use crate::Result;

/// Boolean result with context
#[derive(Debug, Clone)]
pub struct BoolResult {
    /// The overall boolean result
    pub value: bool,
    /// Human-readable description
    pub description: String,
    /// Matching lines (for context)
    pub matches: Vec<(usize, String)>,
    /// Total lines checked
    pub lines_checked: usize,
}

impl BoolResult {
    /// Format as exit-code friendly output
    pub fn display(&self) -> String {
        let status = if self.value { "TRUE" } else { "FALSE" };
        format!("{} | {} | matches={}",
            status,
            self.description,
            self.matches.len()
        )
    }

    /// Exit code (0 = true, 1 = false)
    pub fn exit_code(&self) -> i32 {
        if self.value { 0 } else { 1 }
    }
}

/// Boolean AND: content must contain ALL patterns
///
//! ```bash
//! regedited bool-and doc.md MySection "rust" "fn" "main"
//! # TRUE | Section 'MySection' contains ALL 3 patterns | matches=5
//! ```
pub fn bool_and(content: &str, patterns: &[String]) -> BoolResult {
    let lines: Vec<&str> = content.lines().collect();
    let lower_lines: Vec<String> = lines.iter().map(|l| l.to_lowercase()).collect();
    let lower_patterns: Vec<String> = patterns.iter().map(|p| p.to_lowercase()).collect();

    let mut all_matches = Vec::new();
    let mut all_found = true;

    for pattern in &lower_patterns {
        let mut found = false;
        for (i, line) in lower_lines.iter().enumerate() {
            if line.contains(pattern) {
                found = true;
                all_matches.push((i, lines[i].to_string()));
            }
        }
        if !found {
            all_found = false;
        }
    }

    // Remove duplicate lines
    all_matches.sort_by(|a, b| a.0.cmp(&b.0));
    all_matches.dedup_by(|a, b| a.0 == b.0);

    let desc = format!("Section contains ALL {} pattern(s): {}",
        patterns.len(),
        patterns.join(", ")
    );

    BoolResult {
        value: all_found,
        description: desc,
        matches: all_matches,
        lines_checked: lines.len(),
    }
}

/// Boolean NAND: contains first pattern but NOT second
///
//! ```bash
//! regedited bool-nand doc.md MySection "fn" "python"
//! # TRUE | Section contains 'fn' but NOT 'python' | matches=3
//! ```
pub fn bool_nand(content: &str, must_contain: &str, must_not: &str) -> BoolResult {
    let lines: Vec<&str> = content.lines().collect();
    let lower_lines: Vec<String> = lines.iter().map(|l| l.to_lowercase()).collect();
    let lower_must = must_contain.to_lowercase();
    let lower_not = must_not.to_lowercase();

    let has_must = lower_lines.iter().any(|l| l.contains(&lower_must));
    let has_not = lower_lines.iter().any(|l| l.contains(&lower_not));

    let result = has_must && !has_not;

    let matches: Vec<(usize, String)> = lines.iter().enumerate()
        .filter(|(_, l)| l.to_lowercase().contains(&lower_must))
        .map(|(i, l)| (i, l.to_string()))
        .collect();

    BoolResult {
        value: result,
        description: format!(
            "Section contains '{}' but NOT '{}'",
            must_contain, must_not
        ),
        matches,
        lines_checked: lines.len(),
    }
}

/// Boolean OR: content contains ANY of the patterns
///
//! ```bash
//! regedited bool-or doc.md MySection "rust" "python" "go"
//! # TRUE | Section contains ANY of 3 patterns | matches=5
//! ```
pub fn bool_or(content: &str, patterns: &[String]) -> BoolResult {
    let lines: Vec<&str> = content.lines().collect();
    let lower_lines: Vec<String> = lines.iter().map(|l| l.to_lowercase()).collect();
    let lower_patterns: Vec<String> = patterns.iter().map(|p| p.to_lowercase()).collect();

    let mut all_matches = Vec::new();

    for pattern in &lower_patterns {
        for (i, line) in lower_lines.iter().enumerate() {
            if line.contains(pattern) {
                all_matches.push((i, lines[i].to_string()));
            }
        }
    }

    // Deduplicate
    all_matches.sort_by(|a, b| a.0.cmp(&b.0));
    all_matches.dedup_by(|a, b| a.0 == b.0);

    BoolResult {
        value: !all_matches.is_empty(),
        description: format!(
            "Section contains ANY of {} pattern(s): {}",
            patterns.len(),
            patterns.join(", ")
        ),
        matches: all_matches,
        lines_checked: lines.len(),
    }
}

/// Boolean XOR: contains EXACTLY ONE of the patterns
pub fn bool_xor(content: &str, pattern_a: &str, pattern_b: &str) -> BoolResult {
    let lines: Vec<&str> = content.lines().collect();
    let lower_lines: Vec<String> = lines.iter().map(|l| l.to_lowercase()).collect();
    let lower_a = pattern_a.to_lowercase();
    let lower_b = pattern_b.to_lowercase();

    let has_a = lower_lines.iter().any(|l| l.contains(&lower_a));
    let has_b = lower_lines.iter().any(|l| l.contains(&lower_b));

    // XOR: exactly one must be true
    let result = has_a ^ has_b;

    let matches: Vec<(usize, String)> = lines.iter().enumerate()
        .filter(|(_, l)| {
            let lower = l.to_lowercase();
            lower.contains(&lower_a) || lower.contains(&lower_b)
        })
        .map(|(i, l)| (i, l.to_string()))
        .collect();

    BoolResult {
        value: result,
        description: format!(
            "Section contains EXACTLY ONE of '{}' or '{}'",
            pattern_a, pattern_b
        ),
        matches,
        lines_checked: lines.len(),
    }
}

/// Count occurrences of a pattern
///
//! ```bash
//! regedited count doc.md MySection "fn"
//! # 3 | Pattern 'fn' found 3 times across 45 lines
//! ```
pub fn count(content: &str, pattern: &str) -> (usize, Vec<(usize, String)>) {
    let lines: Vec<&str> = content.lines().collect();
    let lower_pattern = pattern.to_lowercase();

    let matches: Vec<(usize, String)> = lines.iter().enumerate()
        .filter(|(_, l)| l.to_lowercase().contains(&lower_pattern))
        .map(|(i, l)| (i, l.to_string()))
        .collect();

    (matches.len(), matches)
}

/// Check if a zone is empty (has no content lines)
pub fn is_zone_empty(content: &str) -> BoolResult {
    let lines: Vec<&str> = content.lines().collect();
    let non_empty = lines.iter().any(|l| !l.trim().is_empty());

    BoolResult {
        value: !non_empty,
        description: format!("Zone has {} lines ({} non-empty)",
            lines.len(),
            lines.iter().filter(|l| !l.trim().is_empty()).count()
        ),
        matches: Vec::new(),
        lines_checked: lines.len(),
    }
}

/// If-contains-then: if content contains pattern, return then_val, else return else_val
///
//! ```bash
//! regedited if-contains doc.md MySection "fn" --then "HAS_CODE" --else "NO_CODE"
//! # HAS_CODE
//! ```
pub fn if_contains(content: &str, pattern: &str, then_val: &str, else_val: &str) -> String {
    let lower_content = content.to_lowercase();
    let lower_pattern = pattern.to_lowercase();

    if lower_content.contains(&lower_pattern) {
        then_val.to_string()
    } else {
        else_val.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CONTENT: &str = r#"fn main() {
    println!("Hello");
    let x = 42;
}

fn helper() {
    println!("World");
}

struct Point { x: i32, y: i32 }"#;

    #[test]
    fn test_bool_and_true() {
        let result = bool_and(TEST_CONTENT, &["fn".to_string(), "println".to_string()]);
        assert!(result.value);
        assert!(result.description.contains("ALL"));
    }

    #[test]
    fn test_bool_and_false() {
        let result = bool_and(TEST_CONTENT, &["fn".to_string(), "python".to_string()]);
        assert!(!result.value);
    }

    #[test]
    fn test_bool_nand_true() {
        // Contains "fn" but NOT "python"
        let result = bool_nand(TEST_CONTENT, "fn", "python");
        assert!(result.value);
    }

    #[test]
    fn test_bool_nand_false() {
        // Contains "fn" AND "println" — so NAND with "println" should be false
        let result = bool_nand(TEST_CONTENT, "fn", "println");
        assert!(!result.value);
    }

    #[test]
    fn test_bool_or_true() {
        let result = bool_or(TEST_CONTENT, &["python".to_string(), "fn".to_string()]);
        assert!(result.value);
    }

    #[test]
    fn test_bool_or_false() {
        let result = bool_or(TEST_CONTENT, &["python".to_string(), "java".to_string()]);
        assert!(!result.value);
    }

    #[test]
    fn test_bool_xor() {
        // Contains "fn" but NOT "python" — XOR should be true
        let result = bool_xor(TEST_CONTENT, "fn", "python");
        assert!(result.value);

        // Contains both "fn" and "println" — XOR should be false
        let result = bool_xor(TEST_CONTENT, "fn", "println");
        assert!(!result.value);
    }

    #[test]
    fn test_count() {
        let (count, matches) = count(TEST_CONTENT, "fn");
        assert_eq!(count, 2);
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_is_zone_empty() {
        let result = is_zone_empty("");
        assert!(result.value); // Empty = true

        let result = is_zone_empty("some content");
        assert!(!result.value); // Not empty = false
    }

    #[test]
    fn test_if_contains() {
        assert_eq!(if_contains(TEST_CONTENT, "fn", "YES", "NO"), "YES");
        assert_eq!(if_contains(TEST_CONTENT, "python", "YES", "NO"), "NO");
    }
}
