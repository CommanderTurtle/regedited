// SPDX-License-Identifier: AGPL-3.0
//! # Schema Enforcement
//!
//! Optional per-section schemas for type-safe configuration. Schemas define
//! field names, types, allowed values, and constraints. A document that
//! defines a schema can be validated before writes are accepted.
//!
//! ## Schema File Format
//!
//! Schemas are stored in a `.schema` file alongside the document:
//! ```text
//! config.regd      ← main document
//! config.regd.schema  ← schema definition
//! ```
//!
//! Schema uses a simple line-based format:
//! ```text
//! # Regedited Schema v1
//! ---
//! section Config
//!   field version    : string    : required
//!   field max_size   : int       : range(1, 1000000)
//!   field enabled    : bool      : default(true)
//!   field mode       : string    : one_of("auto", "manual", "hybrid")
//!   field path       : path      : required
//! ---
//! section Code
//!   field language   : string    : required
//!   field version    : string    : default("stable")
//!   field features   : array     : optional
//! ---
//! ```
//!
//! ## Why This Matters
//!
//! The Windows Registry has zero schema. That's why it's a mess — any
//! application can write any garbage anywhere. Regedited schemas provide
//! guardrails: type enforcement, allowed values, required fields.
//! This is something Windows has never had.

use crate::{Result, RegeditedError};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
#[allow(unused_imports)]
use std::str::FromStr;

/// Schema version
const SCHEMA_VERSION: &str = "v1";
/// Schema header marker
const SCHEMA_HEADER: &str = "# Regedited Schema";
/// Schema separator
const SCHEMA_SEP: &str = "---";

/// Value types that schema fields can enforce
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaFieldType {
    /// String value
    String,
    /// Integer value (i64)
    Integer,
    /// Boolean value (true/false)
    Boolean,
    /// File path
    Path,
    /// One of a set of allowed values
    Enum(Vec<String>),
    /// Array of values (comma-separated)
    Array,
    /// Raw hex data (0xNN format)
    Hex,
}

impl SchemaFieldType {
    /// Parse from string name
    pub fn from_name(name: &str, constraint: Option<&str>) -> Result<Self> {
        match name {
            "string" | "str" | "text" => Ok(SchemaFieldType::String),
            "int" | "integer" | "number" | "i64" => Ok(SchemaFieldType::Integer),
            "bool" | "boolean" | "flag" => Ok(SchemaFieldType::Boolean),
            "path" | "filepath" | "dir" => Ok(SchemaFieldType::Path),
            "array" | "list" | "vec" => Ok(SchemaFieldType::Array),
            "hex" | "binary" | "bytes" => Ok(SchemaFieldType::Hex),
            "enum" | "one_of" | "choice" => {
                if let Some(c) = constraint {
                    let values = parse_enum_values(c);
                    Ok(SchemaFieldType::Enum(values))
                } else {
                    Err(RegeditedError::Parse(
                        "Enum type requires one_of(...) constraint".to_string()
                    ))
                }
            }
            _ => Err(RegeditedError::Parse(format!("Unknown schema type: {}", name))),
        }
    }

    /// Human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            SchemaFieldType::String => "string",
            SchemaFieldType::Integer => "integer",
            SchemaFieldType::Boolean => "boolean",
            SchemaFieldType::Path => "path",
            SchemaFieldType::Enum(_) => "enum",
            SchemaFieldType::Array => "array",
            SchemaFieldType::Hex => "hex",
        }
    }

    /// Validate a string value against this type
    pub fn validate(&self, value: &str) -> Result<()> {
        match self {
            SchemaFieldType::String => Ok(()),
            SchemaFieldType::Integer => {
                if value.parse::<i64>().is_err() {
                    return Err(RegeditedError::Parse(
                        format!("'{}' is not a valid integer", value)
                    ));
                }
                Ok(())
            }
            SchemaFieldType::Boolean => {
                let v = value.to_lowercase();
                if v != "true" && v != "false" && v != "1" && v != "0" && v != "yes" && v != "no" {
                    return Err(RegeditedError::Parse(
                        format!("'{}' is not a valid boolean (true/false/1/0)", value)
                    ));
                }
                Ok(())
            }
            SchemaFieldType::Path => {
                // Basic path validation — just check non-empty
                if value.is_empty() {
                    return Err(RegeditedError::Parse("Path cannot be empty".to_string()));
                }
                Ok(())
            }
            SchemaFieldType::Enum(allowed) => {
                if !allowed.iter().any(|a| a.eq_ignore_ascii_case(value)) {
                    return Err(RegeditedError::Parse(
                        format!("'{}' not in allowed values: {}", value, allowed.join(", "))
                    ));
                }
                Ok(())
            }
            SchemaFieldType::Array => Ok(()), // Arrays accept any comma-separated content
            SchemaFieldType::Hex => {
                if !value.starts_with("0x") && !value.starts_with("0X") {
                    return Err(RegeditedError::Parse(
                        format!("'{}' is not valid hex (must start with 0x)", value)
                    ));
                }
                Ok(())
            }
        }
    }
}

/// Constraints on a schema field
#[derive(Debug, Clone)]
pub enum FieldConstraint {
    /// No constraints
    None,
    /// Range constraint (min, max) for integers
    Range(i64, i64),
    /// Set of allowed values
    OneOf(Vec<String>),
    /// Default value if not specified
    Default(String),
    /// Field is required
    Required,
    /// Field is optional
    Optional,
    /// Regex pattern for string validation
    Pattern(String),
}

/// A field definition in a section schema
#[derive(Debug, Clone)]
pub struct SchemaField {
    /// Field name
    pub name: String,
    /// Field type
    pub field_type: SchemaFieldType,
    /// Constraints
    pub constraint: FieldConstraint,
    /// Description (from comment)
    pub description: Option<String>,
}

impl SchemaField {
    /// Create a simple field
    pub fn new(name: &str, field_type: SchemaFieldType) -> Self {
        Self {
            name: name.to_string(),
            field_type,
            constraint: FieldConstraint::None,
            description: None,
        }
    }

    /// Check if field is required
    pub fn is_required(&self) -> bool {
        matches!(self.constraint, FieldConstraint::Required)
    }
}

/// A schema for a single section
#[derive(Debug, Clone)]
pub struct SectionSchema {
    /// Section name
    pub section_name: String,
    /// Field definitions (indexed by name)
    pub fields: BTreeMap<String, SchemaField>,
}

impl SectionSchema {
    /// Create a new empty section schema
    pub fn new(name: &str) -> Self {
        Self {
            section_name: name.to_string(),
            fields: BTreeMap::new(),
        }
    }

    /// Add a field
    pub fn add_field(&mut self, field: SchemaField) {
        self.fields.insert(field.name.clone(), field);
    }

    /// Get a field by name
    pub fn get_field(&self, name: &str) -> Option<&SchemaField> {
        self.fields.get(name)
    }

    /// Validate a set of key-value pairs against this schema
    pub fn validate(&self, values: &BTreeMap<String, String>) -> Vec<String> {
        let mut errors = Vec::new();

        // Check required fields are present
        for (name, field) in &self.fields {
            if field.is_required() && !values.contains_key(name) {
                errors.push(format!("Required field '{}' is missing", name));
            }
        }

        // Validate present values
        for (key, value) in values {
            if let Some(field) = self.fields.get(key) {
                // Check allowed values (OneOf)
                if let FieldConstraint::OneOf(ref allowed) = field.constraint {
                    if !allowed.iter().any(|a| a.eq_ignore_ascii_case(value)) {
                        errors.push(format!(
                            "Field '{}': '{}' not in allowed values: {}",
                            key, value, allowed.join(", ")
                        ));
                    }
                }

                // Check range for integers
                if let FieldConstraint::Range(min, max) = field.constraint {
                    if let Ok(v) = value.parse::<i64>() {
                        if v < min || v > max {
                            errors.push(format!(
                                "Field '{}': {} is outside range [{}, {}]",
                                key, v, min, max
                            ));
                        }
                    }
                }

                // Validate type
                if let Err(e) = field.field_type.validate(value) {
                    errors.push(format!("Field '{}': {}", key, e));
                }
            } else {
                errors.push(format!("Unknown field '{}' in section '{}'", key, self.section_name));
            }
        }

        errors
    }
}

/// Full document schema
#[derive(Debug, Clone)]
pub struct DocumentSchema {
    /// Schema version
    pub version: String,
    /// Section schemas keyed by section name
    pub sections: BTreeMap<String, SectionSchema>,
}

impl DocumentSchema {
    /// Create empty schema
    pub fn new() -> Self {
        Self {
            version: SCHEMA_VERSION.to_string(),
            sections: BTreeMap::new(),
        }
    }

    /// Get or create a section schema
    pub fn section(&mut self, name: &str) -> &mut SectionSchema {
        self.sections.entry(name.to_string())
            .or_insert_with(|| SectionSchema::new(name))
    }

    /// Get a section schema
    pub fn get_section(&self, name: &str) -> Option<&SectionSchema> {
        self.sections.get(name)
    }

    /// Load schema from a file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        Self::parse(&content)
    }

    /// Parse schema from string
    pub fn parse(content: &str) -> Result<Self> {
        let mut schema = DocumentSchema::new();
        let mut current_section: Option<&mut SectionSchema> = None;
        let mut in_body = false;

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed == SCHEMA_SEP {
                in_body = !in_body;
                continue;
            }
            if !in_body {
                continue;
            }

            if trimmed.starts_with("section ") {
                let name = trimmed[8..].trim();
                schema.sections.insert(name.to_string(), SectionSchema::new(name));
                current_section = schema.sections.get_mut(name);
                continue;
            }

            if trimmed.starts_with("field ") {
                if let Some(ref mut sec) = current_section {
                    if let Ok(field) = parse_field_line(trimmed) {
                        sec.add_field(field);
                    }
                }
            }
        }

        Ok(schema)
    }

    /// Save schema to a file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut lines = vec![
            format!("{} {}", SCHEMA_HEADER, self.version),
            SCHEMA_SEP.to_string(),
        ];

        for (name, section) in &self.sections {
            lines.push(format!("section {}", name));
            for (fname, field) in &section.fields {
                let constraint_str = match &field.constraint {
                    FieldConstraint::None => String::new(),
                    FieldConstraint::Range(min, max) => format!(" : range({}, {})", min, max),
                    FieldConstraint::OneOf(vals) => format!(" : one_of({})", vals.join(", ")),
                    FieldConstraint::Default(v) => format!(" : default({})", v),
                    FieldConstraint::Required => " : required".to_string(),
                    FieldConstraint::Optional => " : optional".to_string(),
                    FieldConstraint::Pattern(p) => format!(" : pattern({})", p),
                };
                lines.push(format!(
                    "  field {} : {}{}",
                    fname, field.field_type.name(), constraint_str
                ));
            }
        }

        lines.push(SCHEMA_SEP.to_string());
        fs::write(path, lines.join("\n") + "\n")?;
        Ok(())
    }

    /// Get the schema file path for a document
    pub fn schema_path<P: AsRef<Path>>(doc_path: P) -> PathBuf {
        let mut p = doc_path.as_ref().as_os_str().to_owned();
        p.push(".schema");
        PathBuf::from(p)
    }

    /// Check if a schema exists for a document
    pub fn exists_for<P: AsRef<Path>>(doc_path: P) -> bool {
        Self::schema_path(doc_path).exists()
    }

    /// Format summary for display
    pub fn summary(&self) -> String {
        let mut lines = vec![
            format!("Schema version: {}", self.version),
            format!("Sections defined: {}", self.sections.len()),
            String::new(),
        ];
        for (name, section) in &self.sections {
            lines.push(format!("  [{}] — {} fields", name, section.fields.len()));
            for (fname, field) in &section.fields {
                let req = if field.is_required() { "*" } else { " " };
                lines.push(format!("    [{}] {} : {}{}",
                    req, fname, field.field_type.name(),
                    field.description.as_ref().map(|d| format!(" — {}", d)).unwrap_or_default()
                ));
            }
        }
        lines.join("\n")
    }
}

impl Default for DocumentSchema {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== PARSING HELPERS ====================

fn parse_field_line(line: &str) -> Result<SchemaField> {
    // Format: field NAME : TYPE [: CONSTRAINT]
    let parts: Vec<&str> = line[6..].split(':').map(|s| s.trim()).collect();
    if parts.len() < 2 {
        return Err(RegeditedError::Parse(format!("Invalid field line: {}", line)));
    }

    let name = parts[0];
    let type_name = parts[1];

    // Parse constraint if present
    let constraint_str = parts.get(2).copied();
    let constraint = if let Some(c) = constraint_str {
        parse_constraint(c)
    } else {
        FieldConstraint::None
    };

    let field_type = if type_name.starts_with("one_of") || type_name.starts_with("enum") {
        SchemaFieldType::from_name("enum", Some(type_name))?
    } else {
        SchemaFieldType::from_name(type_name, constraint_str)?
    };

    Ok(SchemaField {
        name: name.to_string(),
        field_type,
        constraint,
        description: None,
    })
}

fn parse_constraint(s: &str) -> FieldConstraint {
    let s = s.trim();
    if s.eq_ignore_ascii_case("required") {
        FieldConstraint::Required
    } else if s.eq_ignore_ascii_case("optional") {
        FieldConstraint::Optional
    } else if s.starts_with("range(") && s.ends_with(')') {
        let inner = &s[6..s.len()-1];
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        if parts.len() == 2 {
            let min = parts[0].parse().unwrap_or(i64::MIN);
            let max = parts[1].parse().unwrap_or(i64::MAX);
            FieldConstraint::Range(min, max)
        } else {
            FieldConstraint::None
        }
    } else if s.starts_with("one_of(") && s.ends_with(')') {
        let inner = &s[7..s.len()-1];
        let values = inner.split(',').map(|p| p.trim().to_string()).collect();
        FieldConstraint::OneOf(values)
    } else if s.starts_with("default(") && s.ends_with(')') {
        FieldConstraint::Default(s[8..s.len()-1].to_string())
    } else if s.starts_with("pattern(") && s.ends_with(')') {
        FieldConstraint::Pattern(s[8..s.len()-1].to_string())
    } else {
        FieldConstraint::None
    }
}

fn parse_enum_values(s: &str) -> Vec<String> {
    if s.starts_with("one_of(") && s.ends_with(')') {
        s[7..s.len()-1].split(',').map(|p| p.trim().to_string()).collect()
    } else {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_parse() {
        let schema_str = r#"# Regedited Schema v1
---
section Config
  field version : string : required
  field max_size : int : range(1, 1000000)
  field mode : string : one_of("auto", "manual")
  field enabled : bool : default(true)
---"#;

        let schema = DocumentSchema::parse(schema_str).unwrap();
        assert_eq!(schema.sections.len(), 1);

        let config = schema.get_section("Config").unwrap();
        assert!(config.get_field("version").is_some());
        assert!(config.get_field("max_size").is_some());
        assert!(config.get_field("mode").is_some());

        let version_field = config.get_field("version").unwrap();
        assert!(version_field.is_required());

        let max_size = config.get_field("max_size").unwrap();
        assert!(matches!(max_size.constraint, FieldConstraint::Range(1, 1000000)));
    }

    #[test]
    fn test_schema_validate() {
        let mut schema = DocumentSchema::new();
        let sec = schema.section("Config");
        sec.add_field(SchemaField::new("version", SchemaFieldType::String));
        sec.fields.get_mut("version").unwrap().constraint = FieldConstraint::Required;
        sec.add_field(SchemaField::new("mode", SchemaFieldType::Enum(vec!["auto".to_string(), "manual".to_string()])));

        // Valid values
        let mut valid = BTreeMap::new();
        valid.insert("version".to_string(), "1.0".to_string());
        valid.insert("mode".to_string(), "auto".to_string());
        let errors = schema.get_section("Config").unwrap().validate(&valid);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);

        // Missing required
        let mut missing = BTreeMap::new();
        missing.insert("mode".to_string(), "manual".to_string());
        let errors = schema.get_section("Config").unwrap().validate(&missing);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("version"));

        // Invalid enum
        let mut invalid = BTreeMap::new();
        invalid.insert("version".to_string(), "1.0".to_string());
        invalid.insert("mode".to_string(), "invalid".to_string());
        let errors = schema.get_section("Config").unwrap().validate(&invalid);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("not in allowed"));
    }

    #[test]
    fn test_schema_type_validate() {
        assert!(SchemaFieldType::String.validate("anything").is_ok());
        assert!(SchemaFieldType::Integer.validate("42").is_ok());
        assert!(SchemaFieldType::Integer.validate("not_a_number").is_err());
        assert!(SchemaFieldType::Boolean.validate("true").is_ok());
        assert!(SchemaFieldType::Boolean.validate("yes").is_ok());
        assert!(SchemaFieldType::Boolean.validate("invalid").is_err());
    }

    #[test]
    fn test_schema_save_load() {
        let tmp = std::env::temp_dir().join("regedited_test_schema.schema");
        let _ = std::fs::remove_file(&tmp);

        let mut schema = DocumentSchema::new();
        let sec = schema.section("App");
        sec.add_field(SchemaField::new("name", SchemaFieldType::String));
        sec.fields.get_mut("name").unwrap().constraint = FieldConstraint::Required;
        sec.add_field(SchemaField::new("debug", SchemaFieldType::Boolean));

        schema.save(&tmp).unwrap();
        let loaded = DocumentSchema::load(&tmp).unwrap();

        assert!(loaded.get_section("App").is_some());
        assert!(loaded.get_section("App").unwrap().get_field("name").is_some());

        let _ = std::fs::remove_file(&tmp);
    }
}
