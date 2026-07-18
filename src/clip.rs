// SPDX-License-Identifier: AGPL-3.0
//! # Clipboard Operations
//!
//! Cross-platform clipboard integration. On Windows, uses the native
//! clipboard API through the `arboard` crate. Provides the functionality
//! to copy strings (like zone labels, grep results) directly to the
//! system clipboard.
//!
//! ## Windows-Specific Notes
//!
//! On Windows, the clipboard is a system-wide resource. This module
//! properly opens and closes the clipboard to avoid conflicts with
//! other applications.
//!
//! ## Usage
//!
//! ```bash
//! # Copy zone string to clipboard
//! regedited clip myfile.md MySection 2
//!
//! # The string is now in the system clipboard, ready to paste
//! ```

use crate::{RegeditedError, Result};
#[cfg(feature = "clipboard")]
use std::sync::{Mutex, MutexGuard, OnceLock};

#[cfg(feature = "clipboard")]
static CLIPBOARD_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(feature = "clipboard")]
fn clipboard_guard() -> Result<MutexGuard<'static, ()>> {
    CLIPBOARD_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| RegeditedError::Clipboard("Clipboard lock poisoned".to_string()))
}

/// Copy text to the system clipboard
///
/// # Arguments
/// * `text` - The text to copy
///
/// # Platform Support
/// - Windows: Native Win32 clipboard API
/// - macOS: NSPasteboard
/// - Linux: X11 or Wayland clipboard
///
/// # Example
///
/// ```ignore
/// use regedited::clip::copy_to_clipboard;
///
/// // Copy a string to the clipboard
/// copy_to_clipboard("Hello from Regedited!").unwrap();
/// ```
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    #[cfg(not(feature = "clipboard"))]
    {
        let _ = text;
        return Err(RegeditedError::Clipboard(
            "native clipboard support is disabled in this build".to_string(),
        ));
    }

    #[cfg(feature = "clipboard")]
    {
        let _guard = clipboard_guard()?;
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| RegeditedError::Clipboard(format!("Failed to open clipboard: {e}")))?;

        clipboard
            .set_text(text)
            .map_err(|e| RegeditedError::Clipboard(format!("Failed to set clipboard text: {e}")))?;

        Ok(())
    }
}

/// Get text from the system clipboard
///
/// # Example
///
/// ```ignore
/// use regedited::clip::get_from_clipboard;
///
/// let text = get_from_clipboard().unwrap();
/// println!("Clipboard contains: {}", text);
/// ```
pub fn get_from_clipboard() -> Result<String> {
    #[cfg(not(feature = "clipboard"))]
    {
        return Err(RegeditedError::Clipboard(
            "native clipboard support is disabled in this build".to_string(),
        ));
    }

    #[cfg(feature = "clipboard")]
    {
        let _guard = clipboard_guard()?;
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| RegeditedError::Clipboard(format!("Failed to open clipboard: {e}")))?;

        let text = clipboard
            .get_text()
            .map_err(|e| RegeditedError::Clipboard(format!("Failed to get clipboard text: {e}")))?;

        Ok(text)
    }
}

/// Copy a zone's associated string to clipboard
///
/// This is the primary function for the `regedited clip` command.
/// It copies the string at the given index from a section's database line.
///
/// # Arguments
/// * `section_name` - Name of the section
/// * `string_index` - Which string to copy (0-2)
/// * `content` - The file content
/// * `section` - The section info
pub fn copy_zone_string(
    _section_name: &str,
    string_index: usize,
    content: &str,
    section: &crate::header::SectionInfo,
) -> Result<String> {
    if string_index >= 3 {
        return Err(RegeditedError::Parse(format!(
            "String index {string_index} out of range (0-2)"
        )));
    }

    // Read the database line to get the string
    let lines: Vec<&str> = content.lines().collect();

    let string_line = match string_index {
        0 => section.string1_line,
        1 => section.string2_line,
        2 => section.string3_line,
        _ => unreachable!(),
    };

    if string_line >= lines.len() {
        return Err(RegeditedError::ZoneOutOfBounds {
            line: string_line,
            max_lines: lines.len(),
        });
    }

    let text = lines[string_line].trim();

    copy_to_clipboard(text)?;

    Ok(text.to_string())
}

/// Echo a string to stdout (with safe echo handling)
///
/// This is the fallback when clipboard is not available.
/// It uses the safe echo module to properly escape the string.
pub fn echo_string(text: &str) -> Result<()> {
    let result = crate::echo::safe_echo(text)?;
    println!("{}", result.command());
    Ok(())
}

/// Copy or echo a string depending on platform/availability
///
/// On Windows, always tries clipboard first. Falls back to safe echo.
pub fn copy_or_echo(text: &str) -> Result<()> {
    match copy_to_clipboard(text) {
        Ok(()) => {
            #[cfg(windows)]
            println!(
                "Copied to clipboard: {}",
                text.chars().take(60).collect::<String>()
            );
            #[cfg(not(windows))]
            println!("Copied to clipboard");
            Ok(())
        }
        Err(_) => {
            // Fallback: just print it
            println!("{}", text);
            Ok(())
        }
    }
}

/// Copy with a notification message
///
/// Copies text to clipboard and prints a confirmation message.
pub fn copy_with_notification(text: &str, description: &str) -> Result<()> {
    copy_to_clipboard(text)?;
    let preview: String = text.chars().take(50).collect();
    let ellipsis = if text.len() > 50 { "..." } else { "" };
    println!(
        "{} copied to clipboard: {}{}",
        description, preview, ellipsis
    );
    Ok(())
}

/// Clear the clipboard
///
/// Sets the clipboard to empty text.
pub fn clear_clipboard() -> Result<()> {
    copy_to_clipboard("")
}

// ==================== ENHANCED CLIPBOARD OPERATIONS ====================

/// Copy a manually-keyed hex-word range to clipboard
///
/// Creates a hex-word pair from start/end lines and a zone type,
/// then copies the result to clipboard for manual pasting.
///
/// # Example
/// ```bash
/// regedited clip-hexword 50 80 --zone-type code
/// # → Copies "1x0000032 : 1x0000050" to clipboard
/// ```
pub fn clip_hexword_range(
    start: u32,
    end: u32,
    zone_type: crate::zone_type::ZoneType,
) -> Result<String> {
    use crate::zone_type::encode_hex_word;
    let start_hw = encode_hex_word(start, zone_type);
    let end_hw = encode_hex_word(end, zone_type);
    let result = format!("{} : {}", start_hw, end_hw);
    copy_to_clipboard(&result)?;
    Ok(result)
}

/// Copy a zone's content (by index 0-2) to clipboard
///
/// Extracts the content from a specific zone and copies it.
///
/// # Example
/// ```bash
/// regedited clip-zone myfile.md MySection 1
/// # → Copies zone 1 content to clipboard
/// ```
pub fn clip_zone_content(
    content: &str,
    section: &crate::header::SectionInfo,
    zone_index: usize,
) -> Result<String> {
    use crate::zone_editor::extract_zone_content;

    if zone_index >= 3 {
        return Err(RegeditedError::Parse(format!(
            "Zone index {} out of range (0-2)",
            zone_index
        )));
    }

    let zone_text = extract_zone_content(content, section, zone_index)?;
    copy_to_clipboard(&zone_text)?;
    Ok(zone_text)
}

/// Copy a database value (numeric, index 0-8) to clipboard
///
/// # Example
/// ```bash
/// regedited clip-db myfile.md MySection 0
/// # → Copies numeric value at index 0 to clipboard
/// ```
pub fn clip_db_value(
    content: &str,
    section: &crate::header::SectionInfo,
    value_index: usize,
) -> Result<String> {
    use crate::db_line::parse_numeric_line;

    if value_index >= 9 {
        return Err(RegeditedError::Parse(format!(
            "DB value index {} out of range (0-8)",
            value_index
        )));
    }

    let lines: Vec<&str> = content.lines().collect();
    if section.numeric_line >= lines.len() {
        return Err(RegeditedError::ZoneOutOfBounds {
            line: section.numeric_line,
            max_lines: lines.len(),
        });
    }

    let numeric_line = lines[section.numeric_line];
    let values = parse_numeric_line(numeric_line)?;
    let value = values[value_index].to_string();

    copy_to_clipboard(&value)?;
    Ok(value)
}

/// Copy the entire database line to clipboard
///
/// # Example
/// ```bash
/// regedited clip-dbline myfile.md MySection
/// # → Copies "42 | 7 | 3 | 256 | ..." to clipboard
/// ```
pub fn clip_db_line(content: &str, section: &crate::header::SectionInfo) -> Result<String> {
    let lines: Vec<&str> = content.lines().collect();
    if section.numeric_line >= lines.len() {
        return Err(RegeditedError::ZoneOutOfBounds {
            line: section.numeric_line,
            max_lines: lines.len(),
        });
    }

    let line = lines[section.numeric_line].to_string();
    copy_to_clipboard(&line)?;
    Ok(line)
}

/// Copy the hex-word line to clipboard
///
/// # Example
/// ```bash
/// regedited clip-ascii myfile.md MySection
/// # → Copies "0x0000000 : 1x000003C : ..." to clipboard
/// ```
pub fn clip_ascii_store(content: &str, section: &crate::header::SectionInfo) -> Result<String> {
    let lines: Vec<&str> = content.lines().collect();
    if section.ascii_line >= lines.len() {
        return Err(RegeditedError::ZoneOutOfBounds {
            line: section.ascii_line,
            max_lines: lines.len(),
        });
    }

    let line = lines[section.ascii_line].to_string();
    copy_to_clipboard(&line)?;
    Ok(line)
}

#[cfg(all(test, feature = "clipboard"))]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    static TEST_CLIPBOARD_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn test_clipboard_guard() -> MutexGuard<'static, ()> {
        TEST_CLIPBOARD_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap()
    }

    #[test]
    fn test_copy_and_get() {
        let _guard = test_clipboard_guard();
        let test_text = "Regedited test string 123";

        // Copy
        copy_to_clipboard(test_text).unwrap();

        // Get back
        let retrieved = get_from_clipboard().unwrap();
        assert_eq!(retrieved, test_text);
    }

    #[test]
    fn test_copy_empty_string() {
        let _guard = test_clipboard_guard();
        copy_to_clipboard("").unwrap();
        let retrieved = get_from_clipboard().unwrap();
        assert_eq!(retrieved, "");
    }

    #[test]
    fn test_copy_unicode() {
        let _guard = test_clipboard_guard();
        let unicode = "Hello 世界 🦀 Привет мир";
        copy_to_clipboard(unicode).unwrap();
        let retrieved = get_from_clipboard().unwrap();
        assert_eq!(retrieved, unicode);
    }

    #[test]
    fn test_copy_special_chars() {
        let _guard = test_clipboard_guard();
        let special = "<>&|\"'%;$@#!*()[]{}";
        copy_to_clipboard(special).unwrap();
        let retrieved = get_from_clipboard().unwrap();
        assert_eq!(retrieved, special);
    }

    #[test]
    fn test_copy_long_string() {
        let _guard = test_clipboard_guard();
        let long = "a".repeat(10000);
        copy_to_clipboard(&long).unwrap();
        let retrieved = get_from_clipboard().unwrap();
        assert_eq!(retrieved, long);
    }

    #[test]
    fn test_clear_clipboard() {
        let _guard = test_clipboard_guard();
        copy_to_clipboard("some text").unwrap();
        clear_clipboard().unwrap();
        let retrieved = get_from_clipboard().unwrap();
        assert_eq!(retrieved, "");
    }
}
