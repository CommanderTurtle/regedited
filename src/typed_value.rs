// SPDX-License-Identifier: AGPL-3.0
//! # Typed Value System
//!
//! Rich data types beyond plain strings — the kind of type system the Windows
//! Registry wishes it had. Supports structured data (JSON, TOML), binary data,
//! and multi-value arrays, all stored as human-readable markdown content.
//!
//! ## Type Registry
//!
//! | Type | Storage | Example |
//! |------|---------|---------|
//! | `REG_SZ` | Plain string | `"hello world"` |
//! | `REG_DWORD` | u32 integer | `42` |
//! | `REG_QWORD` | u64 integer | `9007199254740992` |
//! | `REG_BINARY` | Hex block | `0x48 0x65 0x6C 0x6C 0x6F` |
//! | `REG_MULTI_SZ` | Array | `["a", "b", "c"]` |
//! | `REG_JSON` | JSON value | `{"name":"test","value":42}` |
//! | `REG_TOML` | TOML value | `name = "test"\nvalue = 42` |
//! | `REG_EXPAND_SZ` | Expandable path | `%SYSTEMROOT%\system32` |
//!
//! ## Why This Matters
//!
//! The Windows Registry has a limited set of types (SZ, DWORD, QWORD, BINARY,
//! MULTI_SZ, EXPAND_SZ). Regedited extends this with JSON and TOML for
//! structured configuration — something that makes complex settings actually
//! maintainable.

use crate::{RegeditedError, Result};

/// Windows Registry-compatible types with Regedited extensions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedValue {
    /// Null-terminated string (REG_SZ)
    String(String),
    /// 32-bit unsigned integer (REG_DWORD)
    Dword(u32),
    /// 64-bit unsigned integer (REG_QWORD)
    Qword(u64),
    /// Raw binary data (REG_BINARY)
    Binary(Vec<u8>),
    /// Multiple null-terminated strings (REG_MULTI_SZ)
    MultiSz(Vec<String>),
    /// JSON structured data (Regedited extension)
    Json(serde_json::Value),
    /// TOML structured data (Regedited extension)
    Toml(String),
    /// Expandable string with environment variables (REG_EXPAND_SZ)
    ExpandSz(String),
    /// 64-bit signed integer (convenience)
    Int64(i64),
    /// Boolean flag (convenience)
    Bool(bool),
}

impl TypedValue {
    // ==================== CONSTRUCTORS ====================

    /// Create a REG_SZ value
    pub fn sz(value: impl Into<String>) -> Self {
        TypedValue::String(value.into())
    }

    /// Create a REG_DWORD value
    pub fn dword(value: u32) -> Self {
        TypedValue::Dword(value)
    }

    /// Create a REG_QWORD value
    pub fn qword(value: u64) -> Self {
        TypedValue::Qword(value)
    }

    /// Create a REG_BINARY value from hex string
    pub fn binary_from_hex(hex: &str) -> Result<Self> {
        let bytes = parse_hex_string(hex)?;
        Ok(TypedValue::Binary(bytes))
    }

    /// Create a REG_MULTI_SZ value
    pub fn multi_sz(values: Vec<String>) -> Self {
        TypedValue::MultiSz(values)
    }

    /// Create a REG_JSON value
    pub fn json(value: serde_json::Value) -> Self {
        TypedValue::Json(value)
    }

    /// Parse a JSON string into REG_JSON
    pub fn json_from_str(s: &str) -> Result<Self> {
        let value = serde_json::from_str(s)
            .map_err(|e| RegeditedError::Parse(format!("Invalid JSON: {}", e)))?;
        Ok(TypedValue::Json(value))
    }

    /// Create a REG_TOML value
    pub fn toml(value: impl Into<String>) -> Self {
        TypedValue::Toml(value.into())
    }

    /// Create a REG_EXPAND_SZ value
    pub fn expand_sz(value: impl Into<String>) -> Self {
        TypedValue::ExpandSz(value.into())
    }

    /// Create a boolean value
    pub fn bool(value: bool) -> Self {
        TypedValue::Bool(value)
    }

    // ==================== TYPE QUERIES ====================

    /// Get the Windows registry type name
    pub fn reg_type_name(&self) -> &'static str {
        match self {
            TypedValue::String(_) => "REG_SZ",
            TypedValue::Dword(_) => "REG_DWORD",
            TypedValue::Qword(_) => "REG_QWORD",
            TypedValue::Binary(_) => "REG_BINARY",
            TypedValue::MultiSz(_) => "REG_MULTI_SZ",
            TypedValue::Json(_) => "REG_JSON",
            TypedValue::Toml(_) => "REG_TOML",
            TypedValue::ExpandSz(_) => "REG_EXPAND_SZ",
            TypedValue::Int64(_) => "REG_INT64",
            TypedValue::Bool(_) => "REG_BOOL",
        }
    }

    /// Get the human-readable type name
    pub fn type_name(&self) -> &'static str {
        match self {
            TypedValue::String(_) => "String",
            TypedValue::Dword(_) => "Dword (u32)",
            TypedValue::Qword(_) => "Qword (u64)",
            TypedValue::Binary(_) => "Binary",
            TypedValue::MultiSz(_) => "MultiSz (string array)",
            TypedValue::Json(_) => "JSON",
            TypedValue::Toml(_) => "TOML",
            TypedValue::ExpandSz(_) => "Expandable String",
            TypedValue::Int64(_) => "Int64",
            TypedValue::Bool(_) => "Bool",
        }
    }

    /// Get the byte size of the stored value
    pub fn byte_size(&self) -> usize {
        match self {
            TypedValue::String(s) => s.len(),
            TypedValue::Dword(_) => 4,
            TypedValue::Qword(_) => 8,
            TypedValue::Binary(b) => b.len(),
            TypedValue::MultiSz(v) => v.iter().map(|s| s.len() + 1).sum(),
            TypedValue::Json(v) => v.to_string().len(),
            TypedValue::Toml(s) => s.len(),
            TypedValue::ExpandSz(s) => s.len(),
            TypedValue::Int64(_) => 8,
            TypedValue::Bool(_) => 1,
        }
    }

    // ==================== CONVERSIONS ====================

    /// Convert to string representation (for storage)
    pub fn to_store_string(&self) -> String {
        match self {
            TypedValue::String(s) => s.clone(),
            TypedValue::Dword(v) => v.to_string(),
            TypedValue::Qword(v) => v.to_string(),
            TypedValue::Binary(b) => hex_encode(b),
            TypedValue::MultiSz(v) => v.join("\0"),
            TypedValue::Json(v) => v.to_string(),
            TypedValue::Toml(s) => s.clone(),
            TypedValue::ExpandSz(s) => s.clone(),
            TypedValue::Int64(v) => v.to_string(),
            TypedValue::Bool(v) => v.to_string(),
        }
    }

    /// Parse from string representation
    pub fn from_store_string(s: &str, type_name: &str) -> Result<Self> {
        match type_name {
            "REG_SZ" | "string" | "str" => Ok(TypedValue::String(s.to_string())),
            "REG_DWORD" | "dword" | "u32" => {
                let v = s
                    .parse()
                    .map_err(|e| RegeditedError::Parse(format!("Invalid DWORD '{}': {}", s, e)))?;
                Ok(TypedValue::Dword(v))
            }
            "REG_QWORD" | "qword" | "u64" => {
                let v = s
                    .parse()
                    .map_err(|e| RegeditedError::Parse(format!("Invalid QWORD '{}': {}", s, e)))?;
                Ok(TypedValue::Qword(v))
            }
            "REG_BINARY" | "binary" | "hex" => {
                let bytes = parse_hex_string(s)?;
                Ok(TypedValue::Binary(bytes))
            }
            "REG_MULTI_SZ" | "multisz" | "array" => {
                let parts: Vec<String> = s.split('\0').map(|p| p.to_string()).collect();
                Ok(TypedValue::MultiSz(parts))
            }
            "REG_JSON" | "json" => {
                let v = serde_json::from_str(s)
                    .map_err(|e| RegeditedError::Parse(format!("Invalid JSON: {}", e)))?;
                Ok(TypedValue::Json(v))
            }
            "REG_TOML" | "toml" => Ok(TypedValue::Toml(s.to_string())),
            "REG_EXPAND_SZ" | "expandsz" | "expand" => Ok(TypedValue::ExpandSz(s.to_string())),
            "REG_INT64" | "int64" | "i64" => {
                let v = s
                    .parse()
                    .map_err(|e| RegeditedError::Parse(format!("Invalid INT64 '{}': {}", s, e)))?;
                Ok(TypedValue::Int64(v))
            }
            "REG_BOOL" | "bool" | "boolean" | "flag" => {
                let v = s.eq_ignore_ascii_case("true") || s == "1" || s.eq_ignore_ascii_case("yes");
                Ok(TypedValue::Bool(v))
            }
            _ => Err(RegeditedError::Parse(format!(
                "Unknown type: {}",
                type_name
            ))),
        }
    }

    /// Expand environment variables (for REG_EXPAND_SZ)
    #[cfg(not(windows))]
    pub fn expand(&self) -> Result<String> {
        match self {
            TypedValue::ExpandSz(s) => Ok(s.clone()), // No expansion on non-Windows
            TypedValue::String(s) => Ok(s.clone()),
            _ => Ok(self.to_store_string()),
        }
    }

    /// Expand environment variables (Windows version)
    #[cfg(windows)]
    pub fn expand(&self) -> Result<String> {
        match self {
            TypedValue::ExpandSz(s) => {
                // Use Windows ExpandEnvironmentStringsW
                Ok(s.clone()) // Placeholder — would use windows-sys
            }
            _ => Ok(self.to_store_string()),
        }
    }

    /// Display in human-readable format
    pub fn display(&self) -> String {
        match self {
            TypedValue::String(s) => format!("\"{}\"", s),
            TypedValue::Dword(v) => format!("0x{:08X} ({})", v, v),
            TypedValue::Qword(v) => format!("0x{:016X} ({})", v, v),
            TypedValue::Binary(b) => format!("{} bytes: {}", b.len(), hex_encode_preview(b)),
            TypedValue::MultiSz(v) => format!("[{}]", v.join(", ")),
            TypedValue::Json(v) => format!(
                "JSON: {}",
                v.to_string().chars().take(60).collect::<String>()
            ),
            TypedValue::Toml(s) => format!("TOML ({} chars)", s.len()),
            TypedValue::ExpandSz(s) => format!("{} (expandable)", s),
            TypedValue::Int64(v) => v.to_string(),
            TypedValue::Bool(v) => v.to_string(),
        }
    }
}

// ==================== HELPERS ====================

/// Parse a hex string into bytes (accepts "0x48 0x65" or "48 65" or "48656C6C6F")
fn parse_hex_string(s: &str) -> Result<Vec<u8>> {
    let s = s.trim();

    // Handle "0x48 0x65 0x6C" format
    if s.starts_with("0x") || s.starts_with("0X") {
        let parts: Vec<&str> = s.split_whitespace().collect();
        let mut bytes = Vec::new();
        for part in parts {
            let hex = part.trim_start_matches("0x").trim_start_matches("0X");
            let byte = u8::from_str_radix(hex, 16)
                .map_err(|e| RegeditedError::Parse(format!("Invalid hex byte '{}': {}", hex, e)))?;
            bytes.push(byte);
        }
        return Ok(bytes);
    }

    // Handle "48 65 6C 6F" format (space-separated hex)
    if s.contains(' ') {
        let parts: Vec<&str> = s.split_whitespace().collect();
        let mut bytes = Vec::new();
        for part in parts {
            let byte = u8::from_str_radix(part, 16).map_err(|e| {
                RegeditedError::Parse(format!("Invalid hex byte '{}': {}", part, e))
            })?;
            bytes.push(byte);
        }
        return Ok(bytes);
    }

    // Handle "48656C6C6F" format (continuous hex)
    let clean = s
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect::<String>();
    if clean.len() % 2 != 0 {
        return Err(RegeditedError::Parse(
            "Hex string has odd length".to_string(),
        ));
    }
    let mut bytes = Vec::new();
    for i in (0..clean.len()).step_by(2) {
        let byte = u8::from_str_radix(&clean[i..i + 2], 16)
            .map_err(|e| RegeditedError::Parse(format!("Invalid hex: {}", e)))?;
        bytes.push(byte);
    }
    Ok(bytes)
}

/// Encode bytes as hex string (0xNN format)
fn hex_encode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("0x{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Encode bytes as hex preview (first 16 bytes)
fn hex_encode_preview(bytes: &[u8]) -> String {
    let preview: Vec<String> = bytes
        .iter()
        .take(16)
        .map(|b| format!("0x{:02X}", b))
        .collect();
    if bytes.len() > 16 {
        format!(
            "{} ... ({} more bytes)",
            preview.join(" "),
            bytes.len() - 16
        )
    } else {
        preview.join(" ")
    }
}

/// List all available registry types
pub fn list_registry_types() -> Vec<(&'static str, &'static str)> {
    vec![
        ("REG_SZ", "Null-terminated string"),
        ("REG_DWORD", "32-bit unsigned integer"),
        ("REG_QWORD", "64-bit unsigned integer"),
        ("REG_BINARY", "Raw binary data (hex)"),
        ("REG_MULTI_SZ", "Multiple null-terminated strings"),
        ("REG_EXPAND_SZ", "Expandable environment string"),
        ("REG_JSON", "JSON structured data (Regedited ext.)"),
        ("REG_TOML", "TOML structured data (Regedited ext.)"),
        ("REG_INT64", "64-bit signed integer"),
        ("REG_BOOL", "Boolean flag"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typed_value_dword() {
        let v = TypedValue::dword(42);
        assert_eq!(v.reg_type_name(), "REG_DWORD");
        assert_eq!(v.to_store_string(), "42");
        assert_eq!(v.byte_size(), 4);
    }

    #[test]
    fn test_typed_value_binary_roundtrip() {
        let original = vec![0x48, 0x65, 0x6C, 0x6C, 0x6F];
        let v = TypedValue::Binary(original.clone());
        let stored = v.to_store_string();
        assert!(stored.contains("0x48"));

        let parsed = TypedValue::from_store_string(&stored, "REG_BINARY").unwrap();
        match parsed {
            TypedValue::Binary(b) => assert_eq!(b, original),
            _ => panic!("Expected Binary"),
        }
    }

    #[test]
    fn test_typed_value_json() {
        let json = serde_json::json!({"name": "test", "enabled": true, "count": 42});
        let v = TypedValue::json(json.clone());
        assert_eq!(v.reg_type_name(), "REG_JSON");

        let stored = v.to_store_string();
        let parsed = TypedValue::from_store_string(&stored, "REG_JSON").unwrap();
        match parsed {
            TypedValue::Json(j) => assert_eq!(j, json),
            _ => panic!("Expected Json"),
        }
    }

    #[test]
    fn test_hex_encode_decode() {
        let bytes = b"Hello";
        let encoded = hex_encode(bytes);
        assert_eq!(encoded, "0x48 0x65 0x6C 0x6C 0x6F");

        let decoded = parse_hex_string(&encoded).unwrap();
        assert_eq!(decoded, bytes.to_vec());
    }

    #[test]
    fn test_hex_continuous() {
        let decoded = parse_hex_string("48656C6C6F").unwrap();
        assert_eq!(decoded, b"Hello".to_vec());
    }

    #[test]
    fn test_multisz_roundtrip() {
        let values = vec!["one".to_string(), "two".to_string(), "three".to_string()];
        let v = TypedValue::multi_sz(values.clone());
        let stored = v.to_store_string();

        let parsed = TypedValue::from_store_string(&stored, "REG_MULTI_SZ").unwrap();
        match parsed {
            TypedValue::MultiSz(v) => assert_eq!(v, values),
            _ => panic!("Expected MultiSz"),
        }
    }

    #[test]
    fn test_bool_parsing() {
        assert!(matches!(
            TypedValue::from_store_string("true", "REG_BOOL").unwrap(),
            TypedValue::Bool(true)
        ));
        assert!(matches!(
            TypedValue::from_store_string("1", "REG_BOOL").unwrap(),
            TypedValue::Bool(true)
        ));
        assert!(matches!(
            TypedValue::from_store_string("no", "REG_BOOL").unwrap(),
            TypedValue::Bool(false)
        ));
    }

    #[test]
    fn test_list_registry_types() {
        let types = list_registry_types();
        assert!(types.len() >= 10);
        assert!(types.iter().any(|(n, _)| *n == "REG_SZ"));
        assert!(types.iter().any(|(n, _)| *n == "REG_JSON"));
    }
}
