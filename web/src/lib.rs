// SPDX-License-Identifier: AGPL-3.0

use serde_json::json;
use wasm_bindgen::prelude::*;

use regedited::header::{DocumentHeader, SectionInfo};

#[wasm_bindgen]
pub struct Registry {
    content: String,
    header: DocumentHeader,
}

#[wasm_bindgen]
impl Registry {
    #[wasm_bindgen(constructor)]
    pub fn new(content: String) -> Result<Registry, JsValue> {
        let header = regedited::header::scan_content(&content)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        Ok(Self { content, header })
    }

    pub fn scan(&self) -> Result<String, JsValue> {
        scan_json(&self.header)
    }

    pub fn grep(&self, pattern: &str, scope: Option<String>) -> Result<String, JsValue> {
        grep_json(&self.content, pattern, scope.as_deref())
    }

    pub fn read_index(&self, index: JsValue) -> Result<String, JsValue> {
        index_json(&self.content, &self.header, parse_js_index(&index)?)
    }

    pub fn section_content(&self, key: &str) -> Result<String, JsValue> {
        let section = find_section(&self.header, key)?;
        regedited::header::extract_section_content(&self.content, section)
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    pub fn section_data(&self, key: &str) -> Result<String, JsValue> {
        let section = find_section(&self.header, key)?;
        regedited::header::extract_section_data(&self.content, section)
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }
}

#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[wasm_bindgen]
pub fn convert(input: &str, default_type: Option<String>) -> Result<String, JsValue> {
    let tokens: Vec<String> = input
        .split_whitespace()
        .map(ToString::to_string)
        .collect();
    let default_type = default_type.unwrap_or_else(|| "markdown".to_string());
    regedited::converter::parse_conversion(&tokens, &default_type)
        .map(|conversion| conversion.output)
        .map_err(|error| JsValue::from_str(&error))
}

#[wasm_bindgen]
pub fn compact_ref(value: &str) -> String {
    regedited::qol::compact_ref(value).unwrap_or_else(|| value.to_string())
}

#[wasm_bindgen]
pub fn scan_document(content: &str) -> Result<String, JsValue> {
    let document = regedited::header::scan_content(content)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    scan_json(&document)
}

#[wasm_bindgen]
pub fn grep_document(
    content: &str,
    pattern: &str,
    scope: Option<String>,
) -> Result<String, JsValue> {
    grep_json(content, pattern, scope.as_deref())
}

#[wasm_bindgen]
pub fn read_index(content: &str, index: JsValue) -> Result<String, JsValue> {
    let document = regedited::header::scan_content(content)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    index_json(content, &document, parse_js_index(&index)?)
}

fn scan_json(document: &DocumentHeader) -> Result<String, JsValue> {
    let sections: Vec<_> = document
        .sections
        .values()
        .map(|section| {
            json!({
                "name": section.name,
                "headerLine": section.header_line,
                "indexLine": section.index_line,
                "hexLine": section.ascii_line,
                "numericLine": section.numeric_line,
                "stringLines": [section.string1_line, section.string2_line, section.string3_line],
                "separatorLine": section.separator_line,
                "contentStart": section.content_start,
                "contentEnd": section.content_end,
                "headerByteOffset": section.header_byte_offset
            })
        })
        .collect();
    serde_json::to_string(&json!({
        "totalLines": document.total_lines,
        "totalBytes": document.total_bytes,
        "sections": sections
    }))
    .map_err(|error| JsValue::from_str(&error.to_string()))
}

fn grep_json(content: &str, pattern: &str, scope: Option<&str>) -> Result<String, JsValue> {
    let matches = match scope {
        Some(scope) => regedited::fast_ops::grep_content_section(content, scope, pattern)
            .map_err(|error| JsValue::from_str(&error.to_string()))?,
        None => regedited::fast_ops::grep_content(content, pattern),
    };
    let matches: Vec<_> = matches
        .into_iter()
        .map(|(line, text)| json!({ "line": line, "lineNumber": line + 1, "text": text }))
        .collect();
    serde_json::to_string(&json!({
        "pattern": pattern,
        "scope": scope,
        "count": matches.len(),
        "matches": matches
    }))
    .map_err(|error| JsValue::from_str(&error.to_string()))
}

fn index_json(content: &str, document: &DocumentHeader, index: u64) -> Result<String, JsValue> {
    let key = format!("index:{}", index);
    let section = find_section(document, &key)?;
    let lines: Vec<&str> = content.lines().collect();
    let line = |line: usize, label: &str| {
        lines
            .get(line)
            .copied()
            .ok_or_else(|| JsValue::from_str(&format!("{} line {} is out of bounds", label, line)))
    };
    let db = regedited::db_line::parse_numeric_line(line(section.numeric_line, "numeric")?)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let content = regedited::header::extract_section_content(content, section)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;

    serde_json::to_string(&json!({
        "index": index,
        "key": key,
        "hexLine": line(section.ascii_line, "hexword")?,
        "db": db,
        "strings": [
            line(section.string1_line, "string 1")?,
            line(section.string2_line, "string 2")?,
            line(section.string3_line, "string 3")?
        ],
        "content": content,
        "contentStart": section.content_start,
        "contentEnd": section.content_end
    }))
    .map_err(|error| JsValue::from_str(&error.to_string()))
}

fn find_section<'a>(document: &'a DocumentHeader, key: &str) -> Result<&'a SectionInfo, JsValue> {
    document
        .get_section(key)
        .or_else(|| document.get_section_case_insensitive(key))
        .ok_or_else(|| JsValue::from_str(&format!("section '{}' not found", key)))
}

fn parse_js_index(value: &JsValue) -> Result<u64, JsValue> {
    if let Some(number) = value.as_f64() {
        if number.is_finite()
            && number >= 0.0
            && number.fract() == 0.0
            && number <= 9_007_199_254_740_991.0
        {
            return Ok(number as u64);
        }
        return Err(JsValue::from_str(
            "index number must be a non-negative safe integer",
        ));
    }
    if let Some(text) = value.as_string() {
        return text
            .parse::<u64>()
            .map_err(|_| JsValue::from_str("index string must contain an unsigned integer"));
    }
    Err(JsValue::from_str(
        "index must be a JavaScript number or numeric string",
    ))
}

#[wasm_bindgen]
pub fn section_content(content: &str, key: &str) -> Result<String, JsValue> {
    let document = regedited::header::scan_content(content)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let section = find_section(&document, key)?;
    regedited::header::extract_section_content(content, section)
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen]
pub fn section_data(content: &str, key: &str) -> Result<String, JsValue> {
    let document = regedited::header::scan_content(content)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let section = find_section(&document, key)?;
    regedited::header::extract_section_data(content, section)
        .map_err(|error| JsValue::from_str(&error.to_string()))
}
