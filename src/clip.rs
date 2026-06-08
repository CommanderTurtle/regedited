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

use crate::{Result, RegeditedError};

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
/// ```
/// use regedited::clip::copy_to_clipboard;
///
/// // Copy a string to the clipboard
/// copy_to_clipboard("Hello from Regedited!").unwrap();
/// ```
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|e| RegeditedError::Clipboard(format!("Failed to open clipboard: {e}")))?;

    clipboard
        .set_text(text)
        .map_err(|e| RegeditedError::Clipboard(format!("Failed to set clipboard text: {e}")))?;

    Ok(())
}

/// Get text from the system clipboard
///
/// # Example
///
/// ```
/// use regedited::clip::get_from_clipboard;
///
/// let text = get_from_clipboard().unwrap();
/// println!("Clipboard contains: {}", text);
/// ```
pub fn get_from_clipboard() -> Result<String> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|e| RegeditedError::Clipboard(format!("Failed to open clipboard: {e}")))?;

    let text = clipboard
        .get_text()
        .map_err(|e| RegeditedError::Clipboard(format!("Failed to get clipboard text: {e}")))?;

    Ok(text)
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
        return Err(RegeditedError::Parse(
            format!("String index {string_index} out of range (0-2)")
        ));
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
            println!("Copied to clipboard: {}", text.chars().take(60).collect::<String>());
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
    println!("{} copied to clipboard: {}{}", description, preview, ellipsis);
    Ok(())
}

/// Clear the clipboard
///
/// Sets the clipboard to empty text.
pub fn clear_clipboard() -> Result<()> {
    copy_to_clipboard("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copy_and_get() {
        let test_text = "Regedited test string 123";
        
        // Copy
        copy_to_clipboard(test_text).unwrap();
        
        // Get back
        let retrieved = get_from_clipboard().unwrap();
        assert_eq!(retrieved, test_text);
    }

    #[test]
    fn test_copy_empty_string() {
        copy_to_clipboard("").unwrap();
        let retrieved = get_from_clipboard().unwrap();
        assert_eq!(retrieved, "");
    }

    #[test]
    fn test_copy_unicode() {
        let unicode = "Hello 世界 🦀 Привет мир";
        copy_to_clipboard(unicode).unwrap();
        let retrieved = get_from_clipboard().unwrap();
        assert_eq!(retrieved, unicode);
    }

    #[test]
    fn test_copy_special_chars() {
        let special = "<>&|\"'%;$@#!*()[]{}";
        copy_to_clipboard(special).unwrap();
        let retrieved = get_from_clipboard().unwrap();
        assert_eq!(retrieved, special);
    }

    #[test]
    fn test_copy_long_string() {
        let long = "a".repeat(10000);
        copy_to_clipboard(&long).unwrap();
        let retrieved = get_from_clipboard().unwrap();
        assert_eq!(retrieved, long);
    }

    #[test]
    fn test_clear_clipboard() {
        copy_to_clipboard("some text").unwrap();
        clear_clipboard().unwrap();
        let retrieved = get_from_clipboard().unwrap();
        assert_eq!(retrieved, "");
    }
}
