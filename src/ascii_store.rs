//! # ASCII Data Store — Hex-Word Format
//!
//! The ASCII datastore uses human-readable `0x`-prefixed hex words:
//!
//! ```text
//! 0x0000000A : 0x00000064 : 0x100000C8 : 0x1000012C : 0x00000000 : 0x00000000
//! ```
//!
//! Each hex-word is `0xTLLLLLLL` where:
//! - `T` = type nibble (0=Markdown, 1=Code, 2=Media, 3=Database)
//! - `LLLLLLL` = line number (7 hex digits = 28 bits = 268M max)
//!
//! Three zone pairs: `(start1, end1), (start2, end2), (start3, end3)`

use crate::zone_type::*;
use crate::{Result, RegeditedError};

/// Number of zone pairs
pub const ZONE_COUNT: usize = 3;

/// A zone pair with embedded type information
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ZonePair {
    /// Starting line number (0-indexed)
    pub start: u32,
    /// Ending line number (inclusive, 0-indexed)
    pub end: u32,
    /// Type of content in this zone
    pub zone_type: ZoneType,
}

impl ZonePair {
    pub fn new(start: u32, end: u32, zone_type: ZoneType) -> Self {
        Self { start, end, zone_type }
    }

    pub fn is_empty(&self) -> bool {
        self.start == 0 && self.end == 0
    }

    pub fn is_valid(&self) -> bool {
        self.start <= self.end
    }

    /// Format start and end as hex-words
    pub fn to_hex_words(&self) -> (String, String) {
        (
            encode_hex_word(self.start, self.zone_type),
            encode_hex_word(self.end, self.zone_type),
        )
    }
}

impl std::fmt::Display for ZonePair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_empty() {
            write!(f, "(empty)")
        } else {
            let (s, e) = self.to_hex_words();
            write!(f, "{} {} (lines {}-{})", s, e, self.start, self.end)
        }
    }
}

/// The ASCII datastore with 3 typed zone pairs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsciiStore {
    pub zones: [ZonePair; ZONE_COUNT],
}

impl Default for AsciiStore {
    fn default() -> Self {
        Self {
            zones: [
                ZonePair::default(),
                ZonePair::default(),
                ZonePair::default(),
            ],
        }
    }
}

impl AsciiStore {
    pub fn new() -> Self { Self::default() }

    /// Parse from a hex-word line
    pub fn from_line(line: &str) -> Result<Self> {
        let (pairs, types) = parse_hex_word_line(line)?;
        let mut zones = [ZonePair::default(); ZONE_COUNT];
        for i in 0..ZONE_COUNT.min(pairs.len()) {
            let (start, end) = pairs[i];
            let (st, _et) = types[i]; // Use start type for the zone
            zones[i] = ZonePair::new(start, end, st);
        }
        Ok(Self { zones })
    }

    /// Convert to hex-word line format
    pub fn to_line(&self) -> String {
        build_ascii_line(&[
            (self.zones[0].start, self.zones[0].end, self.zones[0].zone_type),
            (self.zones[1].start, self.zones[1].end, self.zones[1].zone_type),
            (self.zones[2].start, self.zones[2].end, self.zones[2].zone_type),
        ])
    }

    pub fn zone(&self, index: usize) -> Option<&ZonePair> {
        self.zones.get(index)
    }

    pub fn zone_mut(&mut self, index: usize) -> Option<&mut ZonePair> {
        self.zones.get_mut(index)
    }

    pub fn set_zone(&mut self, index: usize, start: u32, end: u32, zone_type: ZoneType) -> Result<()> {
        if index >= ZONE_COUNT {
            return Err(RegeditedError::Parse(format!("Zone index {index} out of range")));
        }
        self.zones[index] = ZonePair::new(start, end, zone_type);
        Ok(())
    }

    pub fn active_zones(&self) -> Vec<(usize, &ZonePair)> {
        self.zones.iter().enumerate().filter(|(_, z)| !z.is_empty()).collect()
    }

    pub fn display(&self) -> String {
        let mut lines = vec![
            "  Hex-Word Store | 3 zone pairs | 0xTYPE_LINENUM format".to_string()
        ];
        for (i, zone) in self.zones.iter().enumerate() {
            if zone.is_empty() {
                lines.push(format!("    Zone {}: (empty)", i));
            } else {
                let (s, e) = zone.to_hex_words();
                lines.push(format!(
                    "    Zone {}: {} : {} → lines {}-{} [{}]",
                    i, s, e, zone.start, zone.end, zone.zone_type.short()
                ));
            }
        }
        lines.push(format!("    Raw: {}", self.to_line()));
        lines.join("\n")
    }

    pub fn validate(&self) -> Result<()> {
        for (i, zone) in self.zones.iter().enumerate() {
            if !zone.is_empty() && !zone.is_valid() {
                return Err(RegeditedError::Parse(
                    format!("Zone {i}: start ({}) > end ({})", zone.start, zone.end)
                ));
            }
        }
        Ok(())
    }
}

impl std::fmt::Display for AsciiStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display())
    }
}

/// Blank store (all zeros)
pub fn blank_ascii_store() -> String {
    "0x00000000 : 0x00000000 : 0x00000000 : 0x00000000 : 0x00000000 : 0x00000000".to_string()
}

/// Parse ASCII store from a specific line in file content
pub fn parse_ascii_store(content: &str, line_index: usize) -> Result<AsciiStore> {
    let line = content.lines().nth(line_index)
        .ok_or_else(|| RegeditedError::ZoneOutOfBounds {
            line: line_index,
            max_lines: content.lines().count(),
        })?;
    AsciiStore::from_line(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let store = AsciiStore {
            zones: [
                ZonePair::new(10, 100, ZoneType::Markdown),
                ZonePair::new(200, 300, ZoneType::Code),
                ZonePair::new(0, 0, ZoneType::Markdown),
            ],
        };
        let line = store.to_line();
        let parsed = AsciiStore::from_line(&line).unwrap();
        assert_eq!(store.zones[0], parsed.zones[0]);
        assert_eq!(store.zones[1], parsed.zones[1]);
        assert!(parsed.zones[2].is_empty());
    }

    #[test]
    fn test_large_values() {
        let store = AsciiStore {
            zones: [
                ZonePair::new(1_000_000, 2_000_000, ZoneType::Code),
                ZonePair::new(0, 0, ZoneType::Markdown),
                ZonePair::new(100_000_000, 200_000_000, ZoneType::Media),
            ],
        };
        let line = store.to_line();
        let parsed = AsciiStore::from_line(&line).unwrap();
        assert_eq!(store.zones[0], parsed.zones[0]);
        assert_eq!(store.zones[2], parsed.zones[2]);
    }

    #[test]
    fn test_blank() {
        let store = AsciiStore::from_line(&blank_ascii_store()).unwrap();
        assert!(store.zones[0].is_empty());
        assert!(store.zones[1].is_empty());
        assert!(store.zones[2].is_empty());
    }

    #[test]
    fn test_empty_line() {
        let store = AsciiStore::from_line("").unwrap();
        assert!(store.zones[0].is_empty());
    }

    #[test]
    fn test_display() {
        let mut store = AsciiStore::new();
        store.set_zone(0, 115, 230, ZoneType::Markdown).unwrap();
        store.set_zone(1, 500, 610, ZoneType::Code).unwrap();
        let d = store.display();
        assert!(d.contains("0x00000073"));
        assert!(d.contains("0x000000E6"));
        assert!(d.contains("[MD]"));
        assert!(d.contains("[CODE]"));
    }
}
