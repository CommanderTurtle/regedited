//! # HTML Extract — Native HTML Attribute Extraction
//!
//! Mirrors the Windows CMD GRAB B/C/D workflow for extracting HTML
//! attributes (HREF, SRC, etc.) and formatting them as encapsulated
//! set variables.
//!
//! ## The Original CMD Commands
//!
//! ```cmd
//! GRAB B  — Double-quoted:    echo -0=["%A"]
//! GRAB C  — Single-quoted:    echo -0=['%A']
//! GRAB D  — Double-single:    echo -0=["'%A'"]
//! ```
//!
//! ## Regedited Equivalents
//!
//! ```bash
//! # Extract all HREF attributes, output in search mode (b)
//! regedited grab-html file.html HREF --mode b
//!
//! # Extract in delimit mode (c) — safe for piping
//! regedited grab-html file.html HREF --mode c | clip
//!
//! # Extract in store mode (d) — universal storage format
//! regedited grab-html file.html HREF --mode d
//!
//! # Extract SRC attributes from specific tags
//! regedited grab-html file.html SRC --tag img --mode d
//!
//! # Output as set variables (shel.sh database style)
//! regedited grab-html file.html HREF --mode d --set 0aaa
//! # → set "0aaa=["'https://example.com'"]"
//! # → set "0aab=["'https://another.com'"]"
//! ```

use crate::encapsulate::{encapsulate, EncapMode};
use crate::{Result, RegeditedError};

/// An extracted HTML attribute
#[derive(Debug, Clone)]
pub struct HtmlExtract {
    /// Line number in the HTML file
    pub line_num: usize,
    /// The tag name (a, img, link, etc.)
    pub tag: String,
    /// The attribute name (HREF, SRC, etc.)
    pub attr: String,
    /// The attribute value
    pub value: String,
    /// Full line context
    pub context: String,
}

/// Extract attributes from HTML content
///
/// Fast line-by-line extraction using string matching. Not a full HTML parser —
/// designed for speed on raw HTML files, like the original `findstr` approach.
pub fn extract_attributes(
    content: &str,
    attr_name: &str,
    tag_filter: Option<&str>,
) -> Vec<HtmlExtract> {
    let attr_lower = attr_name.to_lowercase();
    let mut results = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        let line_lower = line.to_lowercase();

        // Quick check: does this line contain the attribute?
        if !line_lower.contains(&attr_lower) {
            continue;
        }

        // Optional tag filter
        if let Some(tag) = tag_filter {
            let tag_lower = tag.to_lowercase();
            if !line_lower.contains(&format!("<{}" , tag_lower)) {
                continue;
            }
        }

        // Extract the attribute value
        // Pattern: attr="value" or attr='value'
        if let Some(value) = parse_attr_value(line, &attr_lower) {
            let tag = detect_tag(line).unwrap_or_default();

            results.push(HtmlExtract {
                line_num,
                tag,
                attr: attr_name.to_string(),
                value,
                context: line.trim().to_string(),
            });
        }
    }

    results
}

/// Parse an attribute value from a line
///
/// Handles: `attr="value"`, `attr='value'`, `attr=value`
fn parse_attr_value(line: &str, attr_name: &str) -> Option<String> {
    let lower = line.to_lowercase();

    // Find the attribute position
    let attr_pos = lower.find(attr_name)?;
    let after_attr = &line[attr_pos + attr_name.len()..];

    // Skip whitespace and =
    let after_attr = after_attr.trim_start();
    if !after_attr.starts_with('=') {
        return None;
    }
    let after_equals = &after_attr[1..].trim_start();

    // Extract quoted or unquoted value
    if after_equals.starts_with('"') {
        // Double-quoted
        let rest = &after_equals[1..];
        let end = rest.find('"').unwrap_or(rest.len());
        Some(rest[..end].to_string())
    } else if after_equals.starts_with('\'') {
        // Single-quoted
        let rest = &after_equals[1..];
        let end = rest.find('\'').unwrap_or(rest.len());
        Some(rest[..end].to_string())
    } else {
        // Unquoted — take until whitespace or >
        let end = after_equals
            .find(|c: char| c.is_whitespace() || c == '>')
            .unwrap_or(after_equals.len());
        Some(after_equals[..end].to_string())
    }
}

/// Detect the tag name from a line
fn detect_tag(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('<') {
        return None;
    }

    let after_bracket = &trimmed[1..];
    let end = after_bracket
        .find(|c: char| c.is_whitespace() || c == '>')
        .unwrap_or(after_bracket.len());

    let tag = after_bracket[..end].to_lowercase();

    // Skip closing tags and comments
    if tag.starts_with('/') || tag.starts_with('!') {
        return None;
    }

    Some(tag)
}

/// Format extractions as set variables (shel.sh database style)
///
/// ```bash
//! regedited grab-html file.html HREF --mode d --set 0aaa
//! # → set "0aaa=["'https://example.com'"]"
//! # → set "0aab=["'https://another.com'"]"
//! ```
pub fn format_as_set_vars(
    extracts: &[HtmlExtract],
    mode: EncapMode,
    base_name: &str,
) -> Vec<String> {
    let mut results = Vec::new();

    for (i, extract) in extracts.iter().enumerate() {
        let name = format!("{}{}", base_name, index_to_suffix(i));
        let encap = encapsulate(&extract.value, mode);
        results.push(format!("set \"{}={}\"", name, encap));
    }

    results
}

/// Format extractions as simple encapsulated lines
pub fn format_as_encapsulated(
    extracts: &[HtmlExtract],
    mode: EncapMode,
) -> Vec<String> {
    extracts.iter().map(|e| encapsulate(&e.value, mode)).collect()
}

/// Format with line numbers (like original GRAB output)
///
//! ```bash
//! regedited grab-html file.html HREF --mode b --numbered
//! # → -0=["https://example.com"]
//! # → -1=["https://another.com"]
//! ```
pub fn format_numbered(
    extracts: &[HtmlExtract],
    mode: EncapMode,
) -> Vec<String> {
    extracts.iter().enumerate().map(|(i, e)| {
        let encap = encapsulate(&e.value, mode);
        format!("-{}={}", i, encap)
    }).collect()
}

/// Format with counter (original CMD style with count=0)
pub fn format_with_counter(
    extracts: &[HtmlExtract],
    mode: EncapMode,
) -> Vec<String> {
    extracts.iter().enumerate().map(|(i, e)| {
        let encap = encapsulate(&e.value, mode);
        let aa_name = format!("aa{}", i);
        let bb_name = format!("bb{}", i);
        format!(
            "set /a count+={} && set \"{}={}\" && set \"{}={}\"",
            i + 1, aa_name, encap, bb_name, encap
        )
    }).collect()
}

/// Convert a numeric index to a suffix (0→aaa, 1→aab, 2→aac, ...)
///
/// This matches the user's naming convention: 0aaa, 0aab, 0aac, 0aad, etc.
pub fn index_to_suffix(index: usize) -> String {
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
    let mut result = String::new();
    let mut n = index;

    // Generate 3-letter suffix (aaa, aab, aac, ..., aaz, aba, abb, ...)
    for _ in 0..3 {
        result.push(CHARS[n % 26] as char);
        n /= 26;
    }

    // Reverse since we built it backwards
    result.chars().rev().collect()
}

/// Parse a suffix back to an index
pub fn suffix_to_index(suffix: &str) -> Option<usize> {
    if suffix.len() != 3 {
        return None;
    }

    let mut result = 0usize;
    for c in suffix.chars() {
        if !c.is_ascii_lowercase() {
            return None;
        }
        result = result * 26 + (c as u8 - b'a') as usize;
    }

    Some(result)
}

/// Display extraction results
pub fn display_extracts(extracts: &[HtmlExtract], mode: EncapMode) -> String {
    let mut lines = vec![
        format!("  Found {} {} attribute(s) in mode '{}' {}",
            extracts.len(),
            extracts.first().map(|e| e.attr.clone()).unwrap_or_default(),
            mode.letter(),
            mode.format_desc()
        ),
    ];

    for (i, extract) in extracts.iter().enumerate() {
        let encap = encapsulate(&extract.value, mode);
        lines.push(format!(
            "  [{}] Line {} <{} {}={}>",
            i, extract.line_num, extract.tag, extract.attr, encap
        ));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_HTML: &str = r#"<html>
<head><title>Test</title></head>
<body>
<a href="https://example.com">Link 1</a>
<a href='https://another.com' class="link">Link 2</a>
<img src="image.png" alt="test">
<a HREF="https://third.com">Link 3</a>
<link rel="stylesheet" href="style.css">
</body>
</html>"#;

    #[test]
    fn test_extract_href() {
        let results = extract_attributes(TEST_HTML, "href", None);
        assert_eq!(results.len(), 4);
        assert_eq!(results[0].value, "https://example.com");
        assert_eq!(results[1].value, "https://another.com");
        assert_eq!(results[2].value, "https://third.com");
        assert_eq!(results[3].value, "style.css");
    }

    #[test]
    fn test_extract_src() {
        let results = extract_attributes(TEST_HTML, "src", None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].value, "image.png");
    }

    #[test]
    fn test_tag_filter() {
        let results = extract_attributes(TEST_HTML, "href", Some("a"));
        assert_eq!(results.len(), 3); // Only <a> tags, not <link>
    }

    #[test]
    fn test_case_insensitive() {
        let results = extract_attributes(TEST_HTML, "HREF", None);
        assert_eq!(results.len(), 4); // Should match regardless of case
    }

    #[test]
    fn test_format_as_set_vars() {
        let results = extract_attributes(TEST_HTML, "href", Some("a"));
        let formatted = format_as_set_vars(&results, EncapMode::Store, "0");
        assert_eq!(formatted[0], "set \"0aaa=[\"'https://example.com'\"]\"");
        assert_eq!(formatted[1], "set \"0aab=[\"'https://another.com'\"]\"");
        assert_eq!(formatted[2], "set \"0aac=[\"'https://third.com'\"]\"");
    }

    #[test]
    fn test_format_numbered() {
        let results = extract_attributes(TEST_HTML, "href", Some("a"));
        let formatted = format_numbered(&results, EncapMode::Search);
        assert_eq!(formatted[0], "-0=[\"https://example.com\"]");
        assert_eq!(formatted[1], "-1=[\"https://another.com\"]");
    }

    #[test]
    fn test_index_to_suffix() {
        assert_eq!(index_to_suffix(0), "aaa");
        assert_eq!(index_to_suffix(1), "aab");
        assert_eq!(index_to_suffix(25), "aaz");
        assert_eq!(index_to_suffix(26), "aba");
        assert_eq!(index_to_suffix(27), "abb");
    }

    #[test]
    fn test_suffix_roundtrip() {
        for i in 0..100 {
            let suffix = index_to_suffix(i);
            let parsed = suffix_to_index(&suffix).unwrap();
            assert_eq!(parsed, i);
        }
    }

    #[test]
    fn test_parse_attr_value_quoted() {
        assert_eq!(
            parse_attr_value(r#"<a href="https://site.com">"#, "href"),
            Some("https://site.com".to_string())
        );
        assert_eq!(
            parse_attr_value(r#"<a href='https://site.com'>"#, "href"),
            Some("https://site.com".to_string())
        );
    }

    #[test]
    fn test_parse_attr_value_unquoted() {
        assert_eq!(
            parse_attr_value("<tag attr=value>", "attr"),
            Some("value".to_string())
        );
    }

    #[test]
    fn test_detect_tag() {
        assert_eq!(detect_tag("<a href=\"test\">"), Some("a".to_string()));
        assert_eq!(detect_tag("<img src=\"test\">"), Some("img".to_string()));
        assert_eq!(detect_tag("</a>"), None); // Closing tag
        assert_eq!(detect_tag("<!-- comment -->"), None); // Comment
    }

    #[test]
    fn test_empty_html() {
        let results = extract_attributes("", "href", None);
        assert!(results.is_empty());
    }

    #[test]
    fn test_no_matches() {
        let results = extract_attributes("<html></html>", "href", None);
        assert!(results.is_empty());
    }
}
