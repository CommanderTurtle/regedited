// SPDX-License-Identifier: AGPL-3.0
//! # Encapsulation — The Three-Mode System (sHEL/XML)
//!
//! Direct implementation of the [sHEL XML Project](https://docs.shel.sh/xml-project/)
//! three-mode variable encapsulation system for literal-safe `cmd.exe` data handling.
//!
//! | Mode | Wrap | Can Search | Can Delimit | Can Echo | Notes |
//! |------|------|-----------|-------------|----------|-------|
//! | **b** | `["..."]` | Yes | **No** | Yes (nested `"" ""`) | Best for `findstr` |
//! | **c** | `['...']` | **No** | Yes | Yes (nested `" "`) | Compresses to single line |
//! | **d** | `["'...'"]` | Yes | **No** | Yes (nested `"" ""`) | Almost always safe |
//!
//! Source: `docs.shel.sh/xml-project` — "Expand to show — What you just did"
//!
//! > **RUNTIME IS `cmd.exe` — NOT POWERSHELL.**
//! >
//! > Codeblocks in the XML Project docs are tagged `powershell` for syntax
//! > highlighting only. The `set` command shown below is native **batch**.
//! > From PowerShell: type `cmd`, press Enter, then run. Or save as `.bat`.
//!
//! ## Philosophy
//!
//! Different contexts need different quoting. `cmd.exe` interprets `&`, `|`,
//! `<`, `>`, `^`, `%`, `"` as control operators at parse time. Instead of
//! fighting with escapes, use the right encapsulation mode for the job.
//!
//! ## Construction (XML Project style)
//!
//! Native `cmd.exe` batch — enter via `cmd` from PowerShell first:
//! ```batch
//! REM X/Y/Z base wrappers (run in cmd.exe, NOT PowerShell)
//! set "x=[""]"          REM b-mode:  %x:~0,2%content%x:~2% → ["content"]
//! set "y=['']"          REM c-mode:  %y:~0,2%content%y:~2% → ['content']
//! set "z=["'']"         REM d-mode:  %z:~0,3%content%z:~3% → ["'content'"]
//! ```
//!
//! ## Rust API Usage (this module)
//!
//! ```rust
//! use regedited::encapsulate::{encapsulate, EncapMode};
//!
//! // b-mode: searchable strings (findstr-compatible)
//! let b = encapsulate("hello world", EncapMode::Search);
//! // → ["hello world"]
//!
//! // c-mode: delimitable content (FOR /F parsing)
//! let c = encapsulate("hello world", EncapMode::Delimit);
//! // → ['hello world']
//!
//! // d-mode: universal storage (extract via %var:~3,-3%)
//! let d = encapsulate("hello world", EncapMode::Store);
//! // → ["'hello world'"]
//! ```

use crate::Result;

/// The three encapsulation modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncapMode {
    /// b-mode: `["..."]` — Double-quoted brackets for searching/echoing
    Search,
    /// c-mode: `['...']` — Single-quoted brackets for delimiting/piping
    Delimit,
    /// d-mode: `["'...'"]` — Double-single quoted for universal storage
    Store,
}

impl EncapMode {
    /// Parse from mode name (b/c/d or search/delimit/store)
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "b" | "search" | "echo" => Some(EncapMode::Search),
            "c" | "delimit" | "pipe" | "literal" => Some(EncapMode::Delimit),
            "d" | "store" | "save" | "universal" => Some(EncapMode::Store),
            _ => None,
        }
    }

    /// Get the mode letter
    pub fn letter(&self) -> char {
        match self {
            EncapMode::Search => 'b',
            EncapMode::Delimit => 'c',
            EncapMode::Store => 'd',
        }
    }

    /// Human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            EncapMode::Search => "search",
            EncapMode::Delimit => "delimit",
            EncapMode::Store => "store",
        }
    }

    /// Description of the mode
    pub fn description(&self) -> &'static str {
        match self {
            EncapMode::Search => "Double-quoted brackets [\"...\"] — for searching/echoing",
            EncapMode::Delimit => "Single-quoted brackets ['...'] — for delimiting/piping",
            EncapMode::Store => "Double-single quoted [\"'...'\"] — for universal storage",
        }
    }

    /// Format: brackets + quotes used
    pub fn format_desc(&self) -> &'static str {
        match self {
            EncapMode::Search => "[\"...\"]",
            EncapMode::Delimit => "['...']",
            EncapMode::Store => "[\"'...'\"]",
        }
    }
}

impl std::fmt::Display for EncapMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Encapsulate a string in the given mode
///
/// ```
/// use regedited::encapsulate::{encapsulate, EncapMode};
///
/// assert_eq!(encapsulate("hello", EncapMode::Search), "[\"hello\"]");
/// assert_eq!(encapsulate("hello", EncapMode::Delimit), "['hello']");
/// assert_eq!(encapsulate("hello", EncapMode::Store), "[\"'hello'\"]");
/// ```
pub fn encapsulate(s: &str, mode: EncapMode) -> String {
    match mode {
        EncapMode::Search => format!("[\"{}\"]", s),
        EncapMode::Delimit => format!("['{}']", s),
        EncapMode::Store => format!("[\"'{}'\"]", s),
    }
}

/// Extract the inner content from an encapsulated string
///
/// Handles all three modes automatically:
/// ```
/// use regedited::encapsulate::extract;
///
/// assert_eq!(extract("[\"hello\"]").unwrap(), "hello");
/// assert_eq!(extract("['hello']").unwrap(), "hello");
/// assert_eq!(extract("[\"'hello'\"]").unwrap(), "hello");
/// ```
pub fn extract(encapsulated: &str) -> Result<String> {
    let trimmed = encapsulated.trim();

    // Check for store mode first ["'...'"]
    if trimmed.starts_with("[\"'") && trimmed.ends_with("'\"]") {
        return Ok(trimmed[3..trimmed.len() - 3].to_string());
    }

    // Search mode ["..."]
    if trimmed.starts_with("[\"") && trimmed.ends_with("\"]") {
        return Ok(trimmed[2..trimmed.len() - 2].to_string());
    }

    // Delimit mode ['...']
    if trimmed.starts_with("['") && trimmed.ends_with("']") {
        return Ok(trimmed[2..trimmed.len() - 2].to_string());
    }

    // If no brackets, return as-is (raw string)
    Ok(trimmed.to_string())
}

/// Detect the encapsulation mode of a string
pub fn detect_mode(encapsulated: &str) -> Option<EncapMode> {
    let trimmed = encapsulated.trim();

    if trimmed.starts_with("[\"'") && trimmed.ends_with("'\"]") {
        Some(EncapMode::Store)
    } else if trimmed.starts_with("[\"") && trimmed.ends_with("\"]") {
        Some(EncapMode::Search)
    } else if trimmed.starts_with("['") && trimmed.ends_with("']") {
        Some(EncapMode::Delimit)
    } else {
        None
    }
}

/// Convert between encapsulation modes
///
/// ```bash
/// regedited encap "['hello']" --to b
/// # → ["hello"]
/// ```
pub fn convert_mode(encapsulated: &str, target: EncapMode) -> Result<String> {
    let inner = extract(encapsulated)?;
    Ok(encapsulate(&inner, target))
}

/// Format a list of strings in the given mode (for GRAB-style output)
///
/// ```bash
/// regedited grab-lines file.md --mode b
/// # → ["line 1"]
/// # → ["line 2"]
/// ```
pub fn format_lines(lines: &[String], mode: EncapMode) -> Vec<String> {
    lines.iter().map(|l| encapsulate(l, mode)).collect()
}

/// Build a set-variable style output (shel.sh database format)
///
/// ```bash
/// regedited encap "hello" --set 0aaa --mode d
/// # → set "0aaa=["'hello'"]"
/// ```
pub fn format_set_command(name: &str, value: &str, mode: EncapMode) -> String {
    let encap = encapsulate(value, mode);
    match mode {
        EncapMode::Search => format!("set \"{}={}\"", name, encap),
        EncapMode::Delimit => format!("set '{}={}'", name, encap),
        EncapMode::Store => format!("set \"{}={}\"", name, encap),
    }
}

/// Display mode information
pub fn display_modes() -> String {
    let mut lines = vec![
        "Encapsulation Modes (shel.sh/XML inspired)".to_string(),
        String::new(),
    ];

    for mode in &[EncapMode::Search, EncapMode::Delimit, EncapMode::Store] {
        lines.push(format!(
            "  Mode {} ({}): {}",
            mode.letter().to_string().to_uppercase(),
            mode.name(),
            mode.description()
        ));
        lines.push(format!("    Format: {}", mode.format_desc()));
        lines.push(format!(
            "    Example: {}",
            encapsulate("hello world", *mode)
        ));
        lines.push(String::new());
    }

    lines.push("Usage:".to_string());
    lines.push("  regedited encap \"text\" --mode b      # Search mode [\"...\"]".to_string());
    lines.push("  regedited encap \"text\" --mode c      # Delimit mode ['...']".to_string());
    lines.push("  regedited encap \"text\" --mode d      # Store mode [\"'...'\"]".to_string());
    lines.push("  regedited encap \"[\\\"text\\\"]\" --extract  # Extract inner".to_string());
    lines.push("  regedited encap \"['text']\" --to b       # Convert to search mode".to_string());

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encapsulate_all_modes() {
        assert_eq!(encapsulate("hello", EncapMode::Search), "[\"hello\"]");
        assert_eq!(encapsulate("hello", EncapMode::Delimit), "['hello']");
        assert_eq!(encapsulate("hello", EncapMode::Store), "[\"'hello'\"]");
    }

    #[test]
    fn test_extract_all_modes() {
        assert_eq!(extract("[\"hello\"]").unwrap(), "hello");
        assert_eq!(extract("['hello']").unwrap(), "hello");
        assert_eq!(extract("[\"'hello'\"]").unwrap(), "hello");
    }

    #[test]
    fn test_extract_no_brackets() {
        assert_eq!(extract("raw string").unwrap(), "raw string");
    }

    #[test]
    fn test_detect_mode() {
        assert_eq!(detect_mode("[\"hello\"]"), Some(EncapMode::Search));
        assert_eq!(detect_mode("['hello']"), Some(EncapMode::Delimit));
        assert_eq!(detect_mode("[\"'hello'\"]"), Some(EncapMode::Store));
        assert_eq!(detect_mode("raw"), None);
    }

    #[test]
    fn test_convert_mode() {
        let converted = convert_mode("['hello']", EncapMode::Search).unwrap();
        assert_eq!(converted, "[\"hello\"]");

        let converted = convert_mode("[\"hello\"]", EncapMode::Store).unwrap();
        assert_eq!(converted, "[\"'hello'\"]");
    }

    #[test]
    fn test_format_set_command() {
        let cmd = format_set_command("0aaa", "hello", EncapMode::Store);
        assert_eq!(cmd, "set \"0aaa=[\"'hello'\"]\"");
    }

    #[test]
    fn test_with_special_chars() {
        // Characters that break in CMD should still encapsulate correctly
        let s = "https://site.com?a=1&b=2";
        assert_eq!(
            encapsulate(s, EncapMode::Search),
            "[\"https://site.com?a=1&b=2\"]"
        );

        let s = r#"HREF="https://example.com""#;
        let encap = encapsulate(s, EncapMode::Store);
        assert!(encap.starts_with("[\"'"));
        assert!(encap.ends_with("'\"]"));

        // Roundtrip
        let extracted = extract(&encap).unwrap();
        assert_eq!(extracted, s);
    }

    #[test]
    fn test_from_name() {
        assert_eq!(EncapMode::from_name("b"), Some(EncapMode::Search));
        assert_eq!(EncapMode::from_name("search"), Some(EncapMode::Search));
        assert_eq!(EncapMode::from_name("c"), Some(EncapMode::Delimit));
        assert_eq!(EncapMode::from_name("pipe"), Some(EncapMode::Delimit));
        assert_eq!(EncapMode::from_name("d"), Some(EncapMode::Store));
        assert_eq!(EncapMode::from_name("save"), Some(EncapMode::Store));
        assert_eq!(EncapMode::from_name("unknown"), None);
    }
}
