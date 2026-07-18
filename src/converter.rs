// SPDX-License-Identifier: AGPL-3.0

use crate::zone_type::{encode_hex_word, ZoneType};

const MAX_LINE: u32 = 0x0FFF_FFFF;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversion {
    pub output: String,
    pub clip: bool,
}

pub fn parse_conversion(tokens: &[String], default_type: &str) -> Result<Conversion, String> {
    let mut zone_type = parse_zone_type(default_type)?;
    let mut words = Vec::new();
    let mut clip = false;

    for token in tokens {
        let normalized = token.to_ascii_lowercase();
        if normalized == "clip" || normalized == "c" {
            if clip {
                return Err("clipboard suffix may only be specified once".to_string());
            }
            clip = true;
            continue;
        }

        if let Some(next_type) = parse_inline_zone_type(&normalized) {
            zone_type = next_type;
            continue;
        }

        let line = token.parse::<u32>().map_err(|_| {
            format!(
                "'{}' is not a line number, type token (p/b/m/d), or clipboard suffix",
                token
            )
        })?;
        if line > MAX_LINE {
            return Err(format!(
                "line number {} exceeds the maximum {} (0xFFFFFFF)",
                line, MAX_LINE
            ));
        }
        if words.len() == 6 {
            return Err("convert accepts at most six line numbers (three ranges)".to_string());
        }
        words.push(encode_hex_word(line, zone_type));
    }

    if words.is_empty() {
        return Err("convert requires at least one line number".to_string());
    }

    Ok(Conversion {
        output: words.join(" : "),
        clip,
    })
}

fn parse_inline_zone_type(value: &str) -> Option<ZoneType> {
    match value {
        "p" | "plain" | "markdown" | "md" => Some(ZoneType::Markdown),
        "b" | "block" | "code" => Some(ZoneType::Code),
        "m" | "media" => Some(ZoneType::Media),
        "d" | "database" | "db" => Some(ZoneType::Database),
        _ => None,
    }
}

fn parse_zone_type(value: &str) -> Result<ZoneType, String> {
    parse_inline_zone_type(&value.to_ascii_lowercase())
        .or_else(|| ZoneType::from_name(value))
        .ok_or_else(|| {
            format!(
                "unknown zone type '{}'; use plain/markdown, block/code, media, or database",
                value
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn converts_one_to_six_values_without_padding() {
        let cases = [
            (vec!["58"], "0x000003A"),
            (vec!["58", "59"], "0x000003A : 0x000003B"),
            (
                vec!["58", "59", "80", "90"],
                "0x000003A : 0x000003B : 0x0000050 : 0x000005A",
            ),
            (
                vec!["58", "59", "80", "90", "300", "325"],
                "0x000003A : 0x000003B : 0x0000050 : 0x000005A : 0x000012C : 0x0000145",
            ),
        ];

        for (input, expected) in cases {
            let parsed = parse_conversion(&tokens(&input), "markdown").unwrap();
            assert_eq!(parsed.output, expected);
            assert!(!parsed.clip);
        }
    }

    #[test]
    fn type_tokens_apply_until_changed() {
        let cases = [
            (vec!["d", "58"], "3x000003A"),
            (vec!["d", "58", "59"], "3x000003A : 3x000003B"),
            (vec!["d", "58", "d", "59"], "3x000003A : 3x000003B"),
            (vec!["d", "58", "p", "59"], "3x000003A : 0x000003B"),
            (
                vec!["b", "1", "2", "m", "3", "4", "d", "5", "p", "6"],
                "1x0000001 : 1x0000002 : 2x0000003 : 2x0000004 : 3x0000005 : 0x0000006",
            ),
        ];

        for (input, expected) in cases {
            assert_eq!(
                parse_conversion(&tokens(&input), "markdown")
                    .unwrap()
                    .output,
                expected
            );
        }
    }

    #[test]
    fn supports_legacy_default_type_and_clip_suffixes() {
        let legacy = parse_conversion(&tokens(&["58", "59"]), "code").unwrap();
        assert_eq!(legacy.output, "1x000003A : 1x000003B");

        for suffix in ["clip", "c"] {
            let parsed = parse_conversion(&tokens(&["d", "58", suffix]), "markdown").unwrap();
            assert_eq!(parsed.output, "3x000003A");
            assert!(parsed.clip);
        }
    }

    #[test]
    fn rejects_ambiguous_or_out_of_range_inputs() {
        for input in [
            vec![],
            vec!["p"],
            vec!["58", "clip", "c"],
            vec!["268435456"],
            vec!["1", "2", "3", "4", "5", "6", "7"],
            vec!["wat"],
        ] {
            assert!(parse_conversion(&tokens(&input), "markdown").is_err());
        }
    }
}
