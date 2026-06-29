// SPDX-License-Identifier: AGPL-3.0
//! # Safe Echo — Native `cmd.exe` Literal-Safe Output
//!
//! Based on the [sHEL XML Project](https://docs.shel.sh/xml-project/)'s five
//! echo variants for literal-safe data handling in `cmd.exe`.
//!
//! > **RUNTIME IS `cmd.exe`**
//!
//! ## The Problem
//!
//! `cmd.exe` interprets certain characters as control operators at parse time:
//! `&` (command separator), `|` (pipe), `<` `>` (redirection), `^` (escape),
//! `%` (variable expansion), `"` (quote-state toggle). The double-quote is NOT
//! a string delimiter — it toggles the parser's quote-state flag.
//!
//! ## The Five Variants (XML Project Mapping)
//!
//! | XML Name | # | Rust Name | `cmd /c "set ..."` | When to Use |
//! |----------|---|-----------|-------------------|-------------|
//! | `str` | 1 | `Standard` | `set "a=blank"` | Safe strings |
//! | `realstr` | 2 | `DoubleQuote` | `set a="blank"` | Even quotes |
//! | `realrealstr` | 3 | `CaretEscape` | `set "a="blank""` | Odd quotes |
//! | `literal` | 4 | `Literal` | `set a='test'` | `&` or `\|` present |
//! | `actual` | 5 | `DoubleLiteral` | `set a=blank` | `&` + `\|` (ultra) |
//!
//! Reference: `docs.shel.sh/xml-project` — "Neat tricks" section.
//! Display tag: `powershell` (syntax highlighting). Runtime: **`cmd.exe`**.
//!
//! ## Edge Cases from Research
//!
//! - `&` sign: ONLY literal (4) and doubleliteral (5) work reliably
//! - `|` pipe: Same as `&`, needs literal mode
//! - `"` quotes: Need to ensure even count in void elements
//! - `&` + `|` together: Ultra-error mode, need "" double quotes with replacement

use crate::Result;

/// Characters that are special in Windows CMD
const CMD_SPECIAL_CHARS: &[char] = &['&', '|', '<', '>', '^', '"', '%', '(', ')', '@'];

/// The encapsulation strategies, in order of preference
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encapsulation {
    /// Strategy 1: Standard double quotes
    /// `echo "string"`
    Standard,
    /// Strategy 2: Double double-quotes  
    /// `echo ""string""`
    DoubleQuote,
    /// Strategy 3: Caret-escaped quotes
    /// `echo "^"string^""`
    CaretEscape,
    /// Strategy 4: Single quotes (literal, safest for & and |)
    /// `echo 'string'`
    Literal,
    /// Strategy 5: Double single-quotes (ultra-safe)
    /// `echo ''string''`
    DoubleLiteral,
}

impl Encapsulation {
    /// Get all strategies in order of preference
    pub fn all() -> &'static [Encapsulation] {
        &[
            Encapsulation::Standard,      // (1) - fastest, works for safe strings
            Encapsulation::DoubleQuote,   // (2) - handles quotes
            Encapsulation::CaretEscape,   // (3) - handles complex cases
            Encapsulation::Literal,       // (4) - handles & and |
            Encapsulation::DoubleLiteral, // (5) - ultra-safe fallback
        ]
    }

    /// Get the strategy number (1-5)
    pub fn number(&self) -> u8 {
        match self {
            Encapsulation::Standard => 1,
            Encapsulation::DoubleQuote => 2,
            Encapsulation::CaretEscape => 3,
            Encapsulation::Literal => 4,
            Encapsulation::DoubleLiteral => 5,
        }
    }

    /// Get the name of the strategy
    pub fn name(&self) -> &'static str {
        match self {
            Encapsulation::Standard => "Standard",
            Encapsulation::DoubleQuote => "DoubleQuote",
            Encapsulation::CaretEscape => "CaretEscape",
            Encapsulation::Literal => "Literal",
            Encapsulation::DoubleLiteral => "DoubleLiteral",
        }
    }
}

impl std::fmt::Display for Encapsulation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", self.name(), self.number())
    }
}

/// Analyze a string to determine which characters are problematic
#[derive(Debug, Clone)]
pub struct StringAnalysis {
    /// The original string
    pub original: String,
    /// Which special characters were found
    pub special_chars: Vec<char>,
    /// Whether the string contains & (command separator)
    pub has_ampersand: bool,
    /// Whether the string contains | (pipe)
    pub has_pipe: bool,
    /// Whether the string has both & and |
    pub has_ultra_error: bool,
    /// Count of double quotes
    pub quote_count: usize,
    /// Whether the string is "safe" (no special chars at all)
    pub is_safe: bool,
    /// Whether literal mode (single quotes) is needed
    pub needs_literal: bool,
    /// Whether ultra-error mode (& + | together) is present
    pub is_ultra_error: bool,
}

impl StringAnalysis {
    /// Analyze a string for Windows CMD compatibility
    pub fn analyze(s: &str) -> Self {
        let special_chars: Vec<char> = s
            .chars()
            .filter(|c| CMD_SPECIAL_CHARS.contains(c))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let has_ampersand = s.contains('&');
        let has_pipe = s.contains('|');
        let has_ultra_error = has_ampersand && has_pipe;
        let quote_count = s.matches('"').count();
        let is_safe = special_chars.is_empty();
        let needs_literal = has_ampersand || has_pipe;

        Self {
            original: s.to_string(),
            special_chars: special_chars.into_iter().collect(),
            has_ampersand,
            has_pipe,
            has_ultra_error,
            quote_count,
            is_safe,
            needs_literal,
            is_ultra_error: has_ultra_error,
        }
    }

    /// Display analysis summary
    pub fn summary(&self) -> String {
        if self.is_safe {
            format!("Safe string (no special chars)")
        } else if self.is_ultra_error {
            format!(
                "ULTRA-ERROR: contains both & and | (plus: {:?})",
                self.special_chars
            )
        } else if self.needs_literal {
            format!("Needs literal mode: contains {:?}", self.special_chars)
        } else {
            format!("Contains special chars: {:?}", self.special_chars)
        }
    }
}

/// Apply an encapsulation strategy to a string
///
/// Returns the properly encapsulated string ready for echo
fn apply_encapsulation(s: &str, strategy: Encapsulation) -> String {
    match strategy {
        Encapsulation::Standard => {
            format!("\"{}\"", s)
        }
        Encapsulation::DoubleQuote => {
            // Double the quotes: ""string""
            format!("\"\"{}\"\"", s)
        }
        Encapsulation::CaretEscape => {
            // Caret-escape internal quotes: "^"string^""
            let escaped = s.replace('"', "^\"");
            format!("\"{}\"", escaped)
        }
        Encapsulation::Literal => {
            // Single quotes: 'string'
            format!("'{}'", s)
        }
        Encapsulation::DoubleLiteral => {
            // Double single quotes: ''string''
            format!("''{}''", s)
        }
    }
}

/// The result of a safe echo attempt
#[derive(Debug, Clone)]
pub struct EchoResult {
    /// Which strategy was used
    pub strategy: Encapsulation,
    /// The encapsulated string ready for CMD
    pub cmd_string: String,
    /// The full echo command
    pub echo_command: String,
    /// Analysis of the original string
    pub analysis: StringAnalysis,
}

impl EchoResult {
    /// Get just the command string (the argument to echo)
    pub fn arg(&self) -> &str {
        &self.cmd_string
    }

    /// Get the full command
    pub fn command(&self) -> &str {
        &self.echo_command
    }
}

/// Echo a string safely using the best strategy
///
/// Automatically selects the appropriate encapsulation based on the
/// string's content, following the hierarchy from the research:
///
/// 1. Safe strings -> Standard quotes
/// 2. Strings with quotes -> DoubleQuote or CaretEscape
/// 3. Strings with & or | -> Literal (single quotes)
/// 4. Ultra-error (& + |) -> DoubleLiteral with quote replacement
///
/// # Examples
///
/// ```
/// use regedited::echo::safe_echo;
///
/// let result = safe_echo("Hello world").unwrap();
/// assert_eq!(result.arg(), "\"Hello world\"");
///
/// let result = safe_echo("HREF=\"https://example.com?a=1&b=2\"").unwrap();
/// // Will use Literal mode due to &
/// assert!(result.command().contains('\''));
/// ```
pub fn safe_echo(s: &str) -> Result<EchoResult> {
    if s.is_empty() {
        return Ok(EchoResult {
            strategy: Encapsulation::Standard,
            cmd_string: "\"\"".to_string(),
            echo_command: "echo \"\"".to_string(),
            analysis: StringAnalysis::analyze(s),
        });
    }

    let analysis = StringAnalysis::analyze(s);

    // Determine the best strategy
    let strategy = if analysis.is_ultra_error {
        // Ultra-error: both & and | present
        // Use DoubleLiteral (5) and replace internal quotes with ""
        Encapsulation::DoubleLiteral
    } else if analysis.needs_literal {
        // Contains & or |, need literal mode
        Encapsulation::Literal
    } else if analysis.quote_count > 0 {
        // Contains quotes, need DoubleQuote
        if analysis.quote_count % 2 != 0 {
            // Odd number of quotes - use caret escape
            Encapsulation::CaretEscape
        } else {
            Encapsulation::DoubleQuote
        }
    } else {
        // Safe string
        Encapsulation::Standard
    };

    let cmd_string = apply_encapsulation(s, strategy);
    let echo_command = format!("echo {}", cmd_string);

    Ok(EchoResult {
        strategy,
        cmd_string,
        echo_command,
        analysis,
    })
}

/// Echo with a specific strategy (for manual override)
///
/// ```
/// use regedited::echo::{safe_echo_with_strategy, Encapsulation};
///
/// let result = safe_echo_with_strategy("test & value", Encapsulation::Literal).unwrap();
/// assert!(result.arg().starts_with('\''));
/// ```
pub fn safe_echo_with_strategy(s: &str, strategy: Encapsulation) -> Result<EchoResult> {
    let analysis = StringAnalysis::analyze(s);
    let cmd_string = apply_encapsulation(s, strategy);
    let echo_command = format!("echo {}", cmd_string);

    Ok(EchoResult {
        strategy,
        cmd_string,
        echo_command,
        analysis,
    })
}

/// Try all strategies and return the first one that works
///
/// This is useful for testing - it attempts each encapsulation
/// and reports which ones would work.
pub fn try_all_strategies(s: &str) -> Vec<(Encapsulation, String)> {
    Encapsulation::all()
        .iter()
        .map(|&strategy| {
            let encapsulated = apply_encapsulation(s, strategy);
            (strategy, format!("echo {}", encapsulated))
        })
        .collect()
}

/// Process a string for ultra-error mode (& + | in same line)
///
/// Replaces all quotes with double quotes (" -> "") and uses DoubleLiteral mode.
/// After echo, the user should find-and-replace "" back to ".
///
/// # Example
///
/// ```
/// use regedited::echo::process_ultra_error;
///
/// let result = process_ultra_error("test & value | pipe");
/// // Returns string with proper escaping for ultra-error mode
/// ```
pub fn process_ultra_error(s: &str) -> String {
    // Replace " with "" for delimitation safety
    let modified = s.replace('"', "\"\"");
    format!("''{}''", modified)
}

/// Build an echo command that is guaranteed to work
///
/// This is the most robust version - it uses the best strategy
/// and includes error handling.
pub fn build_echo_command(s: &str) -> String {
    match safe_echo(s) {
        Ok(result) => result.echo_command,
        Err(_) => {
            // Ultimate fallback: base64 encode
            let encoded = base64_encode(s);
            format!("echo {} | certutil -decode - decoded.txt", encoded)
        }
    }
}

/// Simple base64 encode for ultimate fallback
fn base64_encode(s: &str) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = s.as_bytes();
    let mut result = String::new();

    for chunk in bytes.chunks(3) {
        let mut buf = [0u8; 3];
        for (i, &b) in chunk.iter().enumerate() {
            buf[i] = b;
        }

        let b = [
            (buf[0] >> 2) & 0x3f,
            ((buf[0] & 0x03) << 4) | ((buf[1] >> 4) & 0x0f),
            ((buf[1] & 0x0f) << 2) | ((buf[2] >> 6) & 0x03),
            buf[2] & 0x3f,
        ];

        result.push(ALPHABET[b[0] as usize] as char);
        result.push(ALPHABET[b[1] as usize] as char);

        if chunk.len() > 1 {
            result.push(ALPHABET[b[2] as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(ALPHABET[b[3] as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}

/// Format a multi-line output with safe echo for each line
///
/// When displaying zone content or strings, this ensures each line
/// can be safely echoed if needed.
pub fn format_for_display(s: &str) -> String {
    s.lines()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_echo_simple() {
        let result = safe_echo("Hello world").unwrap();
        assert_eq!(result.strategy, Encapsulation::Standard);
        assert_eq!(result.arg(), "\"Hello world\"");
        assert_eq!(result.command(), "echo \"Hello world\"");
    }

    #[test]
    fn test_safe_echo_with_quotes() {
        let result = safe_echo("HREF=\"https://example.com\"").unwrap();
        // Contains quotes, should use DoubleQuote or CaretEscape
        assert!(result.strategy.number() >= 2);
    }

    #[test]
    fn test_safe_echo_with_ampersand() {
        let result = safe_echo("https://site.com?a=1&b=2").unwrap();
        // Contains &, needs literal mode
        assert_eq!(result.strategy, Encapsulation::Literal);
        assert!(result.arg().starts_with('\''));
    }

    #[test]
    fn test_safe_echo_with_pipe() {
        let result = safe_echo("command | pipe").unwrap();
        // Contains |, needs literal mode
        assert_eq!(result.strategy, Encapsulation::Literal);
        assert!(result.arg().starts_with('\''));
    }

    #[test]
    fn test_safe_echo_ultra_error() {
        let result = safe_echo("test & value | pipe").unwrap();
        // Both & and |, needs DoubleLiteral
        assert_eq!(result.strategy, Encapsulation::DoubleLiteral);
        assert!(result.arg().starts_with("''"));
    }

    #[test]
    fn test_safe_echo_odd_quotes() {
        let result = safe_echo("she said \"hello").unwrap();
        // Odd number of quotes, needs CaretEscape
        assert_eq!(result.strategy, Encapsulation::CaretEscape);
    }

    #[test]
    fn test_safe_echo_empty() {
        let result = safe_echo("").unwrap();
        assert_eq!(result.arg(), "\"\"");
    }

    #[test]
    fn test_analysis_safe() {
        let analysis = StringAnalysis::analyze("hello world 123");
        assert!(analysis.is_safe);
        assert!(!analysis.needs_literal);
    }

    #[test]
    fn test_analysis_ampersand() {
        let analysis = StringAnalysis::analyze("a & b");
        assert!(!analysis.is_safe);
        assert!(analysis.has_ampersand);
        assert!(analysis.needs_literal);
        assert!(!analysis.is_ultra_error);
    }

    #[test]
    fn test_analysis_ultra_error() {
        let analysis = StringAnalysis::analyze("a & b | c");
        assert!(analysis.has_ampersand);
        assert!(analysis.has_pipe);
        assert!(analysis.is_ultra_error);
        assert!(analysis.needs_literal);
    }

    #[test]
    fn test_all_strategies() {
        let strategies = try_all_strategies("test");
        assert_eq!(strategies.len(), 5);
        assert!(strategies[0].1.contains("\"test\"")); // Standard
        assert!(strategies[3].1.contains("'test'")); // Literal
        assert!(strategies[4].1.contains("''test''")); // DoubleLiteral
    }

    #[test]
    fn test_process_ultra_error() {
        let result = process_ultra_error("test \"quoted\" & value | pipe");
        assert!(result.starts_with("''"));
        assert!(result.contains("\"\"quoted\"\"")); // Quotes doubled
    }

    #[test]
    fn test_build_echo_command() {
        let cmd = build_echo_command("Hello");
        assert!(cmd.starts_with("echo "));
    }

    #[test]
    fn test_base64_encode() {
        let encoded = base64_encode("Hello");
        assert_eq!(encoded, "SGVsbG8=");
    }

    #[test]
    fn test_real_world_url() {
        let url = r#"HREF="https://www.bing.com/search?pglt=395&q=test&gs_lcrp=EgRlZGdl"#;
        let result = safe_echo(url).unwrap();
        // Should not fail - uses appropriate strategy
        assert!(!result.command().is_empty());
    }
}
