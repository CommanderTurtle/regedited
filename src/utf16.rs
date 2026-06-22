// SPDX-License-Identifier: AGPL-3.0
//! # UTF-16LE Encoding for Line Numbers
//!
//! The ASCII datastore uses UTF-16LE encoding to store line number pairs.
//! This enables:
//! - Absolute line numbers (not relative) matching the real file
//! - Support for billion-line files (full u32 range)
//! - Windows compatibility (Windows uses UTF-16LE in its APIs)
//!
//! ## Format
//!
//! The ASCII store line contains 6 UTF-16LE code units (12 bytes total):
//! - 3 zone pairs: (start1, end1), (start2, end2), (start3, end3)
//! - Each value is a u16 (2 bytes in UTF-16LE)
//! - Empty/default: all 12 bytes are 0x00 → appears as nulls/empty
//!
//! For values > 65535, each value uses a surrogate pair (4 bytes),
//! extending the line to 24 bytes for full u32 range.
//!
//! ## getutf() - The Windows Registry Approach
//!
//! Similar to how Windows Registry uses DWORDs, getutf() converts
//! between decimal line numbers and their UTF-16LE representations.
//!
//! ```bash
//! # Encode 85 as UTF-16LE
//! regedited getutf 85
//! → U+0055 (bytes: 55 00)
//!
//! # Decode UTF-16LE back to decimal
//! regedited getutf --decode "\x55\x00"
//! → 85
//! ```

use crate::{Result, RegeditedError};

/// Fixed-width encoding: always 4 bytes per u32 value
///
/// Each u32 is encoded as two u16 code units in little-endian:
/// - bytes[0..2] = low 16 bits
/// - bytes[2..4] = high 16 bits
///
/// This gives fixed-size encoding (always 4 bytes) which is much
/// simpler to parse than variable-length UTF-16LE surrogate pairs.
/// Supports full u32 range: 0 to 4,294,967,295 (billion-line files).
pub fn encode_u32_utf16le(value: u32) -> Vec<u8> {
    let low = (value & 0xFFFF) as u16;
    let high = ((value >> 16) & 0xFFFF) as u16;
    let mut result = Vec::with_capacity(4);
    result.extend_from_slice(&low.to_le_bytes());
    result.extend_from_slice(&high.to_le_bytes());
    result
}

/// Decode fixed-width 4-byte encoding back to u32
///
/// Expects exactly 4 bytes: [low_u16_le, high_u16_le]
pub fn decode_utf16le_bytes(bytes: &[u8]) -> Result<u32> {
    if bytes.len() < 4 {
        return Err(RegeditedError::Parse(
            format!("Fixed-width decode needs 4 bytes, got {}", bytes.len())
        ));
    }

    let low = u16::from_le_bytes([bytes[0], bytes[1]]) as u32;
    let high = u16::from_le_bytes([bytes[2], bytes[3]]) as u32;
    Ok((high << 16) | low)
}

/// Decode from 2 bytes (single u16, for backward compat with small values)
pub fn decode_u16(bytes: &[u8]) -> Result<u32> {
    if bytes.len() < 2 {
        return Err(RegeditedError::Parse(
            "u16 decode needs 2 bytes".to_string()
        ));
    }
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]) as u32)
}

/// The getutf() function - convert u32 to encoded display string
///
/// This is the primary utility function. Given a line number,
/// returns a human-readable representation of its encoding.
/// Uses fixed 4-byte encoding: [low_u16_le, high_u16_le]
///
/// # Examples
///
/// ```
/// use regedited::utf16::getutf;
///
/// let result = getutf(85);
/// assert!(result.contains("55 00 00 00"));
///
/// let result = getutf(0);
/// assert!(result.contains("00 00 00 00"));
///
/// let result = getutf(1_000_000_000);
/// assert!(result.contains("00 CA 9A 3B"));
/// ```
pub fn getutf(value: u32) -> String {
    let bytes = encode_u32_utf16le(value);
    
    let hex_bytes: Vec<String> = bytes.iter()
        .map(|b| format!("{:02X}", b))
        .collect();
    
    format!("DWORD:{:010} [ {} ]", value, hex_bytes.join(" "))
}

/// Reverse of getutf() - decode a 4-byte hex string back to u32
///
/// Accepts formats like "55 00 00 00" or plain decimal numbers
pub fn getutf_decode(input: &str) -> Result<u32> {
    let trimmed = input.trim();
    
    // First, try parsing as a plain decimal number
    if let Ok(val) = trimmed.parse::<u32>() {
        return Ok(val);
    }
    
    // Try parsing as hex bytes (e.g., "55 00 00 00")
    let cleaned: String = trimmed.chars()
        .filter(|&c| c.is_ascii_hexdigit() || c.is_whitespace())
        .collect();
    
    let hex_bytes: Vec<u8> = cleaned.split_whitespace()
        .filter(|s| s.len() == 2) // Only valid 2-char hex bytes
        .map(|s| u8::from_str_radix(s, 16))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| RegeditedError::Parse(
            format!("Cannot parse '{}' as hex or decimal", input)
        ))?;
    
    if hex_bytes.len() >= 4 {
        decode_utf16le_bytes(&hex_bytes[..4])
    } else if hex_bytes.len() >= 2 {
        let mut padded = vec![0u8; 4];
        for (i, &b) in hex_bytes.iter().take(4).enumerate() {
            padded[i] = b;
        }
        decode_utf16le_bytes(&padded)
    } else {
        Err(RegeditedError::Parse(
            format!("Cannot parse '{}' as hex or decimal", input)
        ))
    }
}

/// Format a u32 for display in the getutf style (single value)
pub fn getutf_single(value: u32) -> String {
    format!("{} → {}", value, getutf(value))
}

/// Batch encode multiple values
pub fn getutf_batch(values: &[u32]) -> Vec<String> {
    values.iter().map(|&v| getutf(v)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_basic() {
        // Value 85 → fixed 4-byte encoding [55, 00, 00, 00]
        let encoded = encode_u32_utf16le(85);
        assert_eq!(encoded, vec![0x55, 0x00, 0x00, 0x00]);
        
        let decoded = decode_utf16le_bytes(&encoded).unwrap();
        assert_eq!(decoded, 85);
    }

    #[test]
    fn test_encode_decode_zero() {
        let encoded = encode_u32_utf16le(0);
        assert_eq!(encoded, vec![0x00, 0x00, 0x00, 0x00]); // Fixed 4 bytes
        
        let decoded = decode_utf16le_bytes(&encoded).unwrap();
        assert_eq!(decoded, 0);
    }

    #[test]
    fn test_encode_decode_max_u16() {
        let encoded = encode_u32_utf16le(65535);
        assert_eq!(encoded, vec![0xFF, 0xFF, 0x00, 0x00]); // Fixed 4 bytes
        
        let decoded = decode_utf16le_bytes(&encoded).unwrap();
        assert_eq!(decoded, 65535);
    }

    #[test]
    fn test_encode_decode_surrogate_pair() {
        // Value 100000 (needs surrogate pair, within UTF-16 range)
        let encoded = encode_u32_utf16le(100000);
        assert_eq!(encoded.len(), 4); // Surrogate pair = 4 bytes
        
        let decoded = decode_utf16le_bytes(&encoded).unwrap();
        assert_eq!(decoded, 100000);
    }

    #[test]
    fn test_encode_decode_large() {
        // 1 billion — uses extended u32 encoding (above 0x10FFFF)
        let encoded = encode_u32_utf16le(1_000_000_000);
        assert_eq!(encoded.len(), 4); // Two u16 code units
        
        let decoded = decode_utf16le_bytes(&encoded).unwrap();
        assert_eq!(decoded, 1_000_000_000);
    }

    #[test]
    fn test_encode_decode_max_u32() {
        // Max u32 value
        let encoded = encode_u32_utf16le(u32::MAX);
        assert_eq!(encoded.len(), 4);
        
        let decoded = decode_utf16le_bytes(&encoded).unwrap();
        assert_eq!(decoded, u32::MAX);
    }

    #[test]
    fn test_getutf_format() {
        let r = getutf(85);
        assert!(r.contains("55 00 00 00"), "Got: {}", r);
        let r = getutf(0);
        assert!(r.contains("00 00 00 00"), "Got: {}", r);
        let r = getutf(1_000_000_000);
        assert!(r.contains("00 CA 9A 3B"), "Got: {}", r);
    }

    #[test]
    fn test_getutf_decode_hex() {
        assert_eq!(getutf_decode("55 00 00 00").unwrap(), 85);
        assert_eq!(getutf_decode("00 00 00 00").unwrap(), 0);
        assert_eq!(getutf_decode("FF FF 00 00").unwrap(), 65535);
    }

    #[test]
    fn test_getutf_decode_decimal() {
        assert_eq!(getutf_decode("85").unwrap(), 85);
        assert_eq!(getutf_decode("1000000").unwrap(), 1000000);
    }

    #[test]
    fn test_getutf_single_display() {
        let r = getutf_single(85);
        assert!(r.contains("85 →"), "Got: {}", r);
        assert!(r.contains("55 00 00 00"), "Got: {}", r);
    }

    #[test]
    fn test_batch_encode() {
        let results = getutf_batch(&[85, 100, 1000]);
        assert_eq!(results.len(), 3);
        assert!(results[0].contains("55 00 00 00"));
        assert!(results[1].contains("64 00 00 00"));
        assert!(results[2].contains("E8 03 00 00"));
    }

    #[test]
    fn test_decode_error_too_short() {
        assert!(decode_utf16le_bytes(&[0x55]).is_err());
    }

    #[test]
    fn test_roundtrip_random_values() {
        for value in [0, 1, 100, 65535, 65536, 100000, 999_999, 1_000_000, 1_000_000_000, u32::MAX] {
            let encoded = encode_u32_utf16le(value);
            let decoded = decode_utf16le_bytes(&encoded).unwrap();
            assert_eq!(decoded, value, "Roundtrip failed for {}", value);
        }
    }
}
