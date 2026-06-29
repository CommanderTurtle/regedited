// SPDX-License-Identifier: AGPL-3.0
//! # Zone Type System
//!
//! Each hex-word's first character (the type digit in `TxLLLLLLL`) encodes the zone type:
//!
//! | Nibble | Type        | Description                          |
//! |--------|-------------|--------------------------------------|
//! | `0`    | Markdown    | Plain markdown text content          |
//! | `1`    | Code        | Code snippets, scripts, commands     |
//! | `2`    | Media       | Images, audio, video references      |
//! | `3`    | Database    | Tabular data, structured content     |
//! | `4-F`  | Reserved    | Future expansion                     |
//!
//! ## Encoding
//!
//! A hex-word `TxLLLLLLL` where:
//! - `T` = type nibble (1 hex digit = 4 bits = 16 types)
//! - `LLLLLLL` = line number (7 hex digits = 28 bits = 268M lines max)
//!
//! ## Examples
//!
//! ```text
//! 0x000000A  // Type 0 (Markdown), line 10
//! 1x0000050  // Type 1 (Code), line 80
//! 2x0000A00  // Type 2 (Media), line 2560
//! 3x0000001  // Type 3 (Database), line 1
//! ```

use crate::{RegeditedError, Result};

/// Zone type encoded in the first nibble of each hex-word
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ZoneType {
    /// Type 0: Plain markdown text content (default)
    #[default]
    Markdown = 0,
    /// Type 1: Code snippets, scripts, shell commands
    Code = 1,
    /// Type 2: Media — images, audio, video references
    Media = 2,
    /// Type 3: Database — tabular data, structured content
    Database = 3,
}

impl ZoneType {
    /// All defined zone types
    pub const ALL: &'static [ZoneType] = &[
        ZoneType::Markdown,
        ZoneType::Code,
        ZoneType::Media,
        ZoneType::Database,
    ];

    /// Parse from a type nibble (0-15)
    pub fn from_nibble(nibble: u8) -> Option<Self> {
        match nibble {
            0 => Some(ZoneType::Markdown),
            1 => Some(ZoneType::Code),
            2 => Some(ZoneType::Media),
            3 => Some(ZoneType::Database),
            _ => None, // Reserved for future use
        }
    }

    /// Get the type nibble value
    pub fn nibble(&self) -> u8 {
        *self as u8
    }

    /// Human-readable name for display
    pub fn name(&self) -> &'static str {
        match self {
            ZoneType::Markdown => "Markdown",
            ZoneType::Code => "Code Snippet",
            ZoneType::Media => "Media",
            ZoneType::Database => "Database",
        }
    }

    /// Short prefix for grep output (e.g., "[CODE] ")
    pub fn prefix(&self) -> String {
        match self {
            ZoneType::Markdown => String::new(), // No prefix for markdown (most common)
            _ => format!("[{}] ", self.short()),
        }
    }

    /// Short uppercase tag
    pub fn short(&self) -> &'static str {
        match self {
            ZoneType::Markdown => "MD",
            ZoneType::Code => "CODE",
            ZoneType::Media => "MEDIA",
            ZoneType::Database => "DB",
        }
    }

    /// Full display label with emoji-style indicator
    pub fn label(&self) -> String {
        match self {
            ZoneType::Markdown => "Markdown".to_string(),
            ZoneType::Code => "Code Snippet".to_string(),
            ZoneType::Media => "Media".to_string(),
            ZoneType::Database => "Database".to_string(),
        }
    }

    /// Parse from a string name (case-insensitive)
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "markdown" | "md" | "0" => Some(ZoneType::Markdown),
            "code" | "snippet" | "codesnippet" | "1" => Some(ZoneType::Code),
            "media" | "img" | "image" | "audio" | "video" | "2" => Some(ZoneType::Media),
            "database" | "db" | "data" | "table" | "3" => Some(ZoneType::Database),
            _ => None,
        }
    }
}

impl std::fmt::Display for ZoneType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Encode a line number and type into a hex-word: `TxLLLLLLL`
///
/// The type digit (0-F) is the FIRST character, immediately visible.
/// `x` is a literal separator. The line number occupies 7 hex digits.
///
/// # Examples
/// - `0x000000A` — Markdown (type 0), line 10
/// - `1x0000050`  — Code (type 1), line 80
/// - `2x0000A00`  — Media (type 2), line 2560
/// - `3x0000001`  — Database (type 3), line 1
///
/// # Panics
/// Panics if `line > 0xFFFFFFF` (268,435,455)
pub fn encode_hex_word(line: u32, zone_type: ZoneType) -> String {
    assert!(
        line <= 0xFFFFFFF,
        "Line number {} exceeds max 0xFFFFFFF",
        line
    );
    format!("{}x{:07X}", zone_type.nibble(), line & 0xFFFFFFF)
}

/// Decode a hex-word back to (line_number, zone_type)
///
/// Primary format: `TxLLLLLLL` — type is the first character (0-F), `x` is literal.
/// Also accepts legacy `TxLLLLLLL` format for backward compatibility.
pub fn decode_hex_word(hex_word: &str) -> Result<(u32, ZoneType)> {
    let trimmed = hex_word.trim();

    // Primary format: "TxLLLLLLL" (9 chars) — type as first char
    if trimmed.len() == 9 && trimmed.chars().nth(1) == Some('x') {
        let type_char = trimmed.chars().next().unwrap();
        let type_nibble = u8::from_str_radix(&type_char.to_string(), 16).map_err(|_| {
            RegeditedError::Parse(format!(
                "Invalid type digit '{}' in hex-word '{}'",
                type_char, hex_word
            ))
        })?;
        let line = u32::from_str_radix(&trimmed[2..], 16).map_err(|e| {
            RegeditedError::Parse(format!("Invalid hex-word '{}': {}", hex_word, e))
        })?;
        let zone_type = ZoneType::from_nibble(type_nibble).unwrap_or(ZoneType::Markdown);
        return Ok((line, zone_type));
    }

    // Legacy format: "0xTLLLLLLL" (10 chars) — type in the high nibble
    if trimmed.len() == 10 && trimmed.starts_with("0x") {
        let hex_str = &trimmed[2..];
        let value = u32::from_str_radix(hex_str, 16).map_err(|e| {
            RegeditedError::Parse(format!("Invalid hex-word '{}': {}", hex_word, e))
        })?;
        let type_nibble = (value >> 28) as u8;
        let line = value & 0xFFFFFFF;
        let zone_type = ZoneType::from_nibble(type_nibble).unwrap_or(ZoneType::Markdown);
        return Ok((line, zone_type));
    }

    // All-zero edge case: "0x0000000" and variants
    if trimmed == "0x0000000" || trimmed == "0" {
        return Ok((0, ZoneType::Markdown));
    }

    Err(RegeditedError::Parse(format!(
        "Hex-word '{}' must be 'TxLLLLLLL' (9 chars) or '0xTLLLLLLL' (10 chars)",
        hex_word
    )))
}

/// Format a complete hex-word line with all 6 values
///
/// ```text
/// 0x0000000 : 0x0000010 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
/// ```
pub fn format_hex_word_line(pairs: &[(u32, u32)], types: &[(ZoneType, ZoneType)]) -> String {
    let mut parts = Vec::new();
    for i in 0..3 {
        let (start_line, end_line) = pairs.get(i).copied().unwrap_or((0, 0));
        let (start_type, end_type) = types
            .get(i)
            .copied()
            .unwrap_or((ZoneType::Markdown, ZoneType::Markdown));
        parts.push(encode_hex_word(start_line, start_type));
        parts.push(encode_hex_word(end_line, end_type));
    }
    parts.join(" : ")
}

/// Parse a hex-word line back into pairs and types
pub fn parse_hex_word_line(line: &str) -> Result<(Vec<(u32, u32)>, Vec<(ZoneType, ZoneType)>)> {
    let trimmed = line.trim();

    // Handle blank/empty
    if trimmed.is_empty()
        || trimmed
            .chars()
            .all(|c| c == '0' || c == 'x' || c == 'X' || c == ' ' || c == ':' || c == '0')
    {
        let hex_part = trimmed.trim_start_matches("0x").trim_start_matches("0X");
        let all_zero = hex_part.chars().all(|c| c == '0' || c == ' ' || c == ':');
        if all_zero || trimmed.is_empty() {
            return Ok((
                vec![(0, 0); 3],
                vec![(ZoneType::Markdown, ZoneType::Markdown); 3],
            ));
        }
    }

    // Split by " : " to get individual hex-words
    let words: Vec<&str> = trimmed.split(" : ").collect();

    if words.len() != 6 {
        return Err(RegeditedError::Parse(format!(
            "Hex-word line must have 6 values separated by ' : ', got {}: '{}'",
            words.len(),
            trimmed
        )));
    }

    let mut pairs = Vec::new();
    let mut types = Vec::new();

    for i in 0..3 {
        let (start_line, start_type) = decode_hex_word(words[i * 2])?;
        let (end_line, end_type) = decode_hex_word(words[i * 2 + 1])?;
        pairs.push((start_line, end_line));
        types.push((start_type, end_type));
    }

    Ok((pairs, types))
}

/// Interactive converter: given a line range and type, produce hex-words
pub fn convert_to_hex_words(
    start_line: u32,
    end_line: u32,
    zone_type: ZoneType,
) -> (String, String) {
    (
        encode_hex_word(start_line, zone_type),
        encode_hex_word(end_line, zone_type),
    )
}

/// Build a complete hex-word line from zone definitions
pub fn build_ascii_line(zones: &[(u32, u32, ZoneType)]) -> String {
    // zones: up to 3 of (start, end, type)
    let mut parts = Vec::new();
    for i in 0..3 {
        if let Some((start, end, zt)) = zones.get(i) {
            parts.push(encode_hex_word(*start, *zt));
            parts.push(encode_hex_word(*end, *zt));
        } else {
            parts.push(encode_hex_word(0, ZoneType::Markdown));
            parts.push(encode_hex_word(0, ZoneType::Markdown));
        }
    }
    parts.join(" : ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_hex_word() {
        // Markdown, line 10
        let encoded = encode_hex_word(10, ZoneType::Markdown);
        assert_eq!(encoded, "0x000000A");
        let (line, zt) = decode_hex_word(&encoded).unwrap();
        assert_eq!(line, 10);
        assert_eq!(zt, ZoneType::Markdown);

        // Code, line 80
        let encoded = encode_hex_word(80, ZoneType::Code);
        assert_eq!(encoded, "1x0000050");
        let (line, zt) = decode_hex_word(&encoded).unwrap();
        assert_eq!(line, 80);
        assert_eq!(zt, ZoneType::Code);

        // Media, line 2560
        let encoded = encode_hex_word(2560, ZoneType::Media);
        assert_eq!(encoded, "2x0000A00");
        let (line, zt) = decode_hex_word(&encoded).unwrap();
        assert_eq!(line, 2560);
        assert_eq!(zt, ZoneType::Media);

        // Database, line 1
        let encoded = encode_hex_word(1, ZoneType::Database);
        assert_eq!(encoded, "3x0000001");
        let (line, zt) = decode_hex_word(&encoded).unwrap();
        assert_eq!(line, 1);
        assert_eq!(zt, ZoneType::Database);
    }

    #[test]
    fn test_decode_new_format_variations() {
        // Test the new "TxLLLLLLL" format
        let (line, zt) = decode_hex_word("0x000000A").unwrap();
        assert_eq!(line, 10);
        assert_eq!(zt, ZoneType::Markdown);

        let (line, zt) = decode_hex_word("1x0000050").unwrap();
        assert_eq!(line, 80);
        assert_eq!(zt, ZoneType::Code);

        let (line, zt) = decode_hex_word("3x0000001").unwrap();
        assert_eq!(line, 1);
        assert_eq!(zt, ZoneType::Database);
    }

    #[test]
    fn test_decode_blank_line() {
        let (pairs, types) = parse_hex_word_line("").unwrap();
        assert_eq!(pairs, vec![(0, 0); 3]);
        assert_eq!(types[0].0, ZoneType::Markdown);
    }

    #[test]
    fn test_all_zeros_line() {
        let line = "0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000";
        let (pairs, types) = parse_hex_word_line(line).unwrap();
        assert_eq!(pairs, vec![(0, 0); 3]);
        assert_eq!(types[0].0, ZoneType::Markdown);
    }

    #[test]
    fn test_format_and_parse_hex_word_line() {
        let pairs = vec![(10, 100), (200, 300), (0, 0)];
        let types = vec![
            (ZoneType::Markdown, ZoneType::Markdown),
            (ZoneType::Code, ZoneType::Code),
            (ZoneType::Markdown, ZoneType::Markdown),
        ];

        let formatted = format_hex_word_line(&pairs, &types);
        let (parsed_pairs, parsed_types) = parse_hex_word_line(&formatted).unwrap();

        assert_eq!(pairs, parsed_pairs);
        assert_eq!(types, parsed_types);
    }

    #[test]
    fn test_zone_type_from_name() {
        assert_eq!(ZoneType::from_name("markdown"), Some(ZoneType::Markdown));
        assert_eq!(ZoneType::from_name("CODE"), Some(ZoneType::Code));
        assert_eq!(ZoneType::from_name("2"), Some(ZoneType::Media));
        assert_eq!(ZoneType::from_name("db"), Some(ZoneType::Database));
        assert_eq!(ZoneType::from_name("unknown"), None);
    }

    #[test]
    fn test_convert_to_hex_words() {
        let (start, end) = convert_to_hex_words(50, 80, ZoneType::Code);
        assert_eq!(start, "1x0000032"); // Code, line 50 (0x32)
        assert_eq!(end, "1x0000050"); // Code, line 80
    }

    #[test]
    fn test_build_ascii_line() {
        let line = build_ascii_line(&[(10, 100, ZoneType::Markdown), (200, 300, ZoneType::Code)]);
        assert!(line.contains("0x000000A"));
        assert!(line.contains("0x0000064"));
        assert!(line.contains("1x00000C8"));
        assert!(line.contains("1x000012C"));
        assert!(line.contains("0x0000000")); // Empty zone 2
    }

    #[test]
    fn test_zone_type_prefix() {
        assert_eq!(ZoneType::Markdown.prefix(), "");
        assert_eq!(ZoneType::Code.prefix(), "[CODE] ");
        assert_eq!(ZoneType::Media.prefix(), "[MEDIA] ");
        assert_eq!(ZoneType::Database.prefix(), "[DB] ");
    }

    #[test]
    fn test_zone_type_display() {
        assert_eq!(format!("{}", ZoneType::Code), "Code Snippet");
        assert_eq!(format!("{}", ZoneType::Media), "Media");
    }

    #[test]
    fn test_max_line_number() {
        // Max line with type 0: 0xFFFFFFF = 268,435,455
        let encoded = encode_hex_word(0xFFFFFFF, ZoneType::Markdown);
        assert_eq!(encoded, "0xFFFFFFF");
        let (line, zt) = decode_hex_word(&encoded).unwrap();
        assert_eq!(line, 268_435_455);
        assert_eq!(zt, ZoneType::Markdown);
    }

    #[test]
    fn test_type_nibble_is_first_hex_digit() {
        // The type nibble is the FIRST hex digit after 0x.
        // Format: 0xTLLLLLLL where T = type (0-F), L = line (28 bits)
        //
        // This test proves the type is encoded in the first hex digit.

        let cases = [
            (ZoneType::Markdown, '0'),
            (ZoneType::Code, '1'),
            (ZoneType::Media, '2'),
            (ZoneType::Database, '3'),
        ];

        for (zt, expected_first_hex) in cases {
            let encoded = encode_hex_word(50, zt);
            // encoded = "1x0000032" — the '1' at position 0 is the type
            let first_hex_digit = encoded.chars().next().unwrap();
            assert_eq!(
                first_hex_digit, expected_first_hex,
                "Type {:?} should produce first hex digit '{}', got '{}' in {}",
                zt, expected_first_hex, first_hex_digit, encoded
            );

            // Verify roundtrip: decode and get the same type back
            let (_, decoded_zt) = decode_hex_word(&encoded).unwrap();
            assert_eq!(
                decoded_zt, zt,
                "Roundtrip failed for type {:?} in {}",
                zt, encoded
            );
        }
    }

    #[test]
    fn test_type_nibble_all_positions_in_hex_word() {
        // Verify all type/line combinations roundtrip correctly
        // Format: "TxLLLLLLL" — type as first char, x separator, 7 hex digits.
        for line in [0, 1, 10, 100, 1000, 0x0FFFFFF] {
            for zt in [
                ZoneType::Markdown,
                ZoneType::Code,
                ZoneType::Media,
                ZoneType::Database,
            ] {
                let encoded = encode_hex_word(line, zt);
                let (_, decoded_zt) = decode_hex_word(&encoded).unwrap();
                assert_eq!(
                    decoded_zt, zt,
                    "Type mismatch for line={}, type={:?}, encoded={}",
                    line, zt, encoded
                );
            }
        }
    }

    #[test]
    fn test_hex_word_bit_layout() {
        // New format: "TxLLLLLLL" — type as first char, 7 hex digits for line
        // For Code (type=1) at line 50: "1x0000032" (50 = 0x32)
        // The type is immediately visible as the first character.
        let encoded = encode_hex_word(50, ZoneType::Code);
        assert_eq!(encoded, "1x0000032");
        assert_eq!(encoded.chars().next().unwrap(), '1'); // Type = Code

        let (line, zt) = decode_hex_word(&encoded).unwrap();
        assert_eq!(line, 50);
        assert_eq!(zt, ZoneType::Code);
    }
}
