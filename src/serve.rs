// SPDX-License-Identifier: AGPL-3.0
//! # Registry Container Mode (HTTP Server)
//!
//! Serve a Regedited document over HTTP as a REST API. This enables:
//! - Remote registry access
//! - Containerized configuration
//! - CI-friendly configuration queries
//! - Testable registry endpoints
//!
//! ## Endpoints
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | GET | `/` | Server status + section list |
//! | GET | `/sections` | List all sections |
//! | GET | `/section/{name}` | Get section metadata + content |
//! | GET | `/section/{name}/db` | Get database table |
//! | GET | `/section/{name}/hexline` | Get hex-word line |
//! | GET | `/section/{name}/ascii` | Legacy alias for `/hexline` |
//! | GET | `/section/{name}/zone/{index}` | Extract zone content |
//! | GET | `/grep?pattern={p}&section={s}` | Search for pattern |
//! | GET | `/state` | Current Regedited state JSON |
//! | GET | `/ref?spec={spec}` | Read a native ref spec |
//! | GET | `/ref-bool?left={a}&op={op}&right={b}` | Boolean comparison over refs/literals |
//! | GET | `/types` | List zone types |
//! | GET | `/wal` | WAL status |
//! | POST | `/query` | Execute boolean query |
//!
//! ## Example Usage
//!
//! ```bash
//! # Start the server
//! regedited serve --file config.regd --port 5000
//!
//! # Query from anywhere
//! curl http://localhost:5000/sections
//! curl http://localhost:5000/section/Config/db
//! curl "http://localhost:5000/grep?pattern=enabled&section=Config"
//! ```

use crate::{
    db_line::parse_numeric_line,
    fast_ops::{fast_scan_content, ScannedSection},
    header::{scan_content, DocumentHeader},
    wal::WalStatus,
    zone_editor::extract_zone_content,
    zone_type::{decode_hex_word, encode_hex_word},
    Result,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tiny_http::{Method, Request, Response, Server, StatusCode};

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServeConfig {
    /// Port to listen on
    pub port: u16,
    /// Document file path
    pub file_path: String,
    /// Enable CORS
    pub cors: bool,
    /// Read-only mode (no modifications)
    pub read_only: bool,
}

impl Default for ServeConfig {
    fn default() -> Self {
        Self {
            port: 5000,
            file_path: String::new(),
            cors: true,
            read_only: true,
        }
    }
}

/// Shared server state
struct ServerState {
    config: ServeConfig,
    /// Cached document header
    header: Mutex<DocumentHeader>,
    /// Cached file content
    content: Mutex<String>,
}

/// Start the HTTP server
pub fn serve(config: ServeConfig) -> Result<()> {
    let addr = format!("0.0.0.0:{}", config.port);
    let server = Server::http(&addr).map_err(|e| {
        crate::RegeditedError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to bind to {}: {}", addr, e),
        ))
    })?;

    // Load document
    let content = std::fs::read_to_string(&config.file_path)?;
    let header = scan_content(&content)?;

    let state = Arc::new(ServerState {
        config: config.clone(),
        header: Mutex::new(header),
        content: Mutex::new(content),
    });

    println!("Regedited server running on http://{}", addr);
    println!("Serving: {}", config.file_path);
    println!("Read-only: {}", config.read_only);
    println!("\nEndpoints:");
    println!("  GET /              — Status + sections");
    println!("  GET /sections      — List all sections");
    println!("  GET /section/{{name}}     — Section metadata");
    println!("  GET /section/{{name}}/db  — Database table");
    println!("  GET /section/{{name}}/hexline — Hex-word line");
    println!("  GET /section/{{name}}/ascii — Legacy alias for /hexline");
    println!("  GET /section/{{name}}/zone/{{i}} — Zone content");
    println!("  GET /grep?pattern= &section= — Search");
    println!("  GET /state         — Current Regedited state");
    println!("  GET /ref?spec=     — Read native ref spec");
    println!("  GET /ref-bool?left=&op=&right= — Boolean ref check");
    println!("  GET /types         — Zone types");
    println!("  GET /wal           — WAL status");

    for request in server.incoming_requests() {
        let state = Arc::clone(&state);
        handle_request(request, state);
    }

    Ok(())
}

fn handle_request(request: Request, state: Arc<ServerState>) {
    // Handle /query specially since it needs to consume the request body
    if *request.method() == Method::Post && request.url() == "/query" {
        handle_query(request, &state);
        return;
    }

    let response = match (request.method(), request.url()) {
        (Method::Get, "/") => handle_root(&state),
        (Method::Get, "/sections") => handle_sections(&state),
        (Method::Get, path) if path.starts_with("/section/") && path.ends_with("/db") => {
            handle_section_db(path, &state)
        }
        (Method::Get, path) if path.starts_with("/section/") && path.contains("/zone/") => {
            handle_section_zone(path, &state)
        }
        (Method::Get, path)
            if path.starts_with("/section/")
                && (path.ends_with("/hexline")
                    || path.ends_with("/hex-word-line")
                    || path.ends_with("/ascii")) =>
        {
            handle_section_ascii(path, &state)
        }
        (Method::Get, path) if path.starts_with("/section/") => handle_section(path, &state),
        (Method::Get, path) if path.starts_with("/grep") => handle_grep(request.url(), &state),
        (Method::Get, "/state") => handle_state(&state),
        (Method::Get, path) if path.starts_with("/ref?") => handle_ref(request.url(), &state),
        (Method::Get, path) if path.starts_with("/ref-bool?") => {
            handle_ref_bool(request.url(), &state)
        }
        (Method::Get, "/types") => handle_types(),
        (Method::Get, "/wal") => handle_wal(&state),
        (Method::Get, "/health") => handle_health(&state),
        _ => json_response(404, r#"{"error": "Not found"}"#),
    };

    if let Err(e) = request.respond(response) {
        eprintln!("Response error: {}", e);
    }
}

// ==================== HANDLERS ====================

fn handle_root(state: &ServerState) -> Response<std::io::Cursor<Vec<u8>>> {
    let header = state.header.lock().unwrap();
    let sections: Vec<String> = header
        .section_names()
        .iter()
        .map(|s| s.to_string())
        .collect();

    let body = format!(
        r#"{{"status":"ok","regedited":"0.2.0","sections":{},"read_only":{},"sections_count":{}}}"#,
        serde_json::to_string(&sections).unwrap_or_default(),
        state.config.read_only,
        header.section_count()
    );
    json_response(200, &body)
}

fn handle_sections(state: &ServerState) -> Response<std::io::Cursor<Vec<u8>>> {
    let header = state.header.lock().unwrap();
    let sections: Vec<BTreeMap<String, String>> = header
        .sections
        .iter()
        .map(|(name, info)| {
            let mut map = BTreeMap::new();
            map.insert("name".to_string(), name.clone());
            map.insert("header_line".to_string(), info.header_line.to_string());
            map.insert("content_start".to_string(), info.content_start.to_string());
            map.insert("content_end".to_string(), info.content_end.to_string());
            map.insert("total_lines".to_string(), info.total_lines().to_string());
            map
        })
        .collect();

    let body = serde_json::to_string(&sections).unwrap_or_default();
    json_response(200, &body)
}

fn handle_section(path: &str, state: &ServerState) -> Response<std::io::Cursor<Vec<u8>>> {
    let name = path.trim_start_matches("/section/");
    let name = name.trim_end_matches("/");

    let header = state.header.lock().unwrap();
    let content = state.content.lock().unwrap();

    if let Some(info) = header
        .get_section(name)
        .or_else(|| header.get_section_case_insensitive(name))
    {
        let lines: Vec<&str> = content.lines().collect();
        let db_line = if info.numeric_line < lines.len() {
            lines[info.numeric_line]
        } else {
            ""
        };

        let body = format!(
            r#"{{"name":"{}","header_line":{},"index_line":{},"ascii_line":{},"numeric_line":{},"content_start":{},"content_end":{},"db_line":"{}","total_lines":{}}}"#,
            name,
            info.header_line,
            info.index_line,
            info.ascii_line,
            info.numeric_line,
            info.content_start,
            info.content_end,
            db_line.replace('"', "\\\""),
            info.total_lines()
        );
        json_response(200, &body)
    } else {
        json_response(
            404,
            &format!(r#"{{"error": "Section '{}' not found"}}"#, name),
        )
    }
}

fn handle_section_db(path: &str, state: &ServerState) -> Response<std::io::Cursor<Vec<u8>>> {
    let name = path.trim_start_matches("/section/");
    let name = name.trim_end_matches("/db");

    let header = state.header.lock().unwrap();
    let content = state.content.lock().unwrap();

    if let Some(info) = header
        .get_section(name)
        .or_else(|| header.get_section_case_insensitive(name))
    {
        let lines: Vec<&str> = content.lines().collect();

        // Extract index, hex-word line, numeric line, and strings.
        let index = if info.header_line + 1 < lines.len() {
            lines[info.header_line + 1]
        } else {
            ""
        };
        let ascii = if info.header_line + 2 < lines.len() {
            lines[info.header_line + 2]
        } else {
            ""
        };
        let numeric = if info.numeric_line < lines.len() {
            lines[info.numeric_line]
        } else {
            ""
        };

        let str1 = if info.string1_line < lines.len() {
            lines[info.string1_line]
        } else {
            ""
        };
        let str2 = if info.string2_line < lines.len() {
            lines[info.string2_line]
        } else {
            ""
        };
        let str3 = if info.string3_line < lines.len() {
            lines[info.string3_line]
        } else {
            ""
        };

        let db_values: Vec<i64> = parse_numeric_line(numeric)
            .map(|values| values.to_vec())
            .unwrap_or_default();

        let body = format!(
            r#"{{"section":"{}","index":{},"hex_word_line":"{}","ascii_store":"{}","db_values":{},"strings":["{}","{}","{}"]}}"#,
            name,
            index,
            ascii,
            ascii,
            serde_json::to_string(&db_values).unwrap_or_default(),
            str1.replace('"', "\\\""),
            str2.replace('"', "\\\""),
            str3.replace('"', "\\\"")
        );
        json_response(200, &body)
    } else {
        json_response(
            404,
            &format!(r#"{{"error": "Section '{}' not found"}}"#, name),
        )
    }
}

fn handle_section_ascii(path: &str, state: &ServerState) -> Response<std::io::Cursor<Vec<u8>>> {
    let name = path.trim_start_matches("/section/");
    let name = name
        .trim_end_matches("/hex-word-line")
        .trim_end_matches("/hexline")
        .trim_end_matches("/ascii");

    let header = state.header.lock().unwrap();
    let content = state.content.lock().unwrap();

    if let Some(info) = header
        .get_section(name)
        .or_else(|| header.get_section_case_insensitive(name))
    {
        let lines: Vec<&str> = content.lines().collect();
        let ascii = if info.header_line + 2 < lines.len() {
            lines[info.header_line + 2]
        } else {
            ""
        };

        json_response(
            200,
            &format!(
                r#"{{"section":"{}","hex_word_line":"{}","ascii_store":"{}"}}"#,
                name, ascii, ascii
            ),
        )
    } else {
        json_response(
            404,
            &format!(r#"{{"error":"Section '{}' not found"}}"#, name),
        )
    }
}

fn handle_section_zone(path: &str, state: &ServerState) -> Response<std::io::Cursor<Vec<u8>>> {
    // Format: /section/{name}/zone/{index}
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 5 {
        return json_response(
            400,
            r#"{"error": "Invalid path. Use /section/{name}/zone/{index}"}"#,
        );
    }

    let name = parts[2];
    let zone_idx: usize = parts[4].parse().unwrap_or(0);

    let header = state.header.lock().unwrap();
    let content = state.content.lock().unwrap();

    if let Some(info) = header
        .get_section(name)
        .or_else(|| header.get_section_case_insensitive(name))
    {
        match extract_zone_content(&content, info, zone_idx) {
            Ok(zone_content) => {
                let body = format!(
                    r#"{{"section":"{}","zone":{},"content":"{}"}}"#,
                    name,
                    zone_idx,
                    zone_content.replace('"', "\\\"").replace('\n', "\\n")
                );
                json_response(200, &body)
            }
            Err(e) => json_response(500, &format!(r#"{{"error":"{}"}}"#, e)),
        }
    } else {
        json_response(
            404,
            &format!(r#"{{"error":"Section '{}' not found"}}"#, name),
        )
    }
}

fn handle_grep(url: &str, state: &ServerState) -> Response<std::io::Cursor<Vec<u8>>> {
    let params = parse_query_string(url);
    let pattern = params.get("pattern").map(|s| s.as_str()).unwrap_or("");
    let section_filter = params.get("section");

    if pattern.is_empty() {
        return json_response(400, r#"{"error": "Missing 'pattern' parameter"}"#);
    }

    let header = state.header.lock().unwrap();
    let content = state.content.lock().unwrap();
    let lines: Vec<&str> = content.lines().collect();

    let mut matches: Vec<BTreeMap<String, String>> = Vec::new();

    for (sec_name, info) in header.sections.iter() {
        if let Some(filter) = section_filter {
            if !sec_name.eq_ignore_ascii_case(filter) {
                continue;
            }
        }

        for (i, line) in lines
            .iter()
            .enumerate()
            .skip(info.content_start)
            .take(info.content_end.saturating_sub(info.content_start) + 1)
        {
            if line.to_lowercase().contains(&pattern.to_lowercase()) {
                let mut m = BTreeMap::new();
                m.insert("section".to_string(), sec_name.clone());
                m.insert("line".to_string(), i.to_string());
                m.insert("content".to_string(), line.to_string());
                matches.push(m);
            }
        }
    }

    let body = format!(
        r#"{{"pattern":"{}","matches":{}}}"#,
        pattern,
        serde_json::to_string(&matches).unwrap_or_default()
    );
    json_response(200, &body)
}

#[derive(Debug, Clone)]
enum ServeRef {
    Literal(String),
    IndexString { registry_index: u64, slot: usize },
    IndexDb { registry_index: u64, slot: usize },
    IndexDbLine { registry_index: u64 },
    IndexAscii { registry_index: u64 },
    IndexZone { registry_index: u64, zone: usize },
    IndexZoneHex { registry_index: u64, zone: usize },
    HexRange { start: String, end: String },
}

fn json_escape(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn parse_user_slot(raw: &str, max: usize, label: &str) -> std::result::Result<usize, String> {
    let value = raw
        .parse::<usize>()
        .map_err(|e| format!("Invalid {} slot '{}': {}", label, raw, e))?;
    if value == 0 || value > max {
        return Err(format!(
            "{} slot {} out of range; use 1-{}",
            label, value, max
        ));
    }
    Ok(value - 1)
}

fn parse_hex_ref(rest: &str) -> ServeRef {
    let trimmed = rest.trim();
    if let Some((start, end)) = trimmed.split_once("..") {
        return ServeRef::HexRange {
            start: start.trim().to_string(),
            end: end.trim().to_string(),
        };
    }
    if let Some((start, end)) = trimmed.split_once(" : ") {
        return ServeRef::HexRange {
            start: start.trim().to_string(),
            end: end.trim().to_string(),
        };
    }
    if let Some((start, end)) = trimmed.split_once(',') {
        return ServeRef::HexRange {
            start: start.trim().to_string(),
            end: end.trim().to_string(),
        };
    }
    ServeRef::HexRange {
        start: trimmed.to_string(),
        end: trimmed.to_string(),
    }
}

fn parse_ref_spec(spec: &str) -> std::result::Result<ServeRef, String> {
    let trimmed = spec.trim();
    if let Some(value) = trimmed
        .strip_prefix("text:")
        .or_else(|| trimmed.strip_prefix("literal:"))
    {
        return Ok(ServeRef::Literal(value.to_string()));
    }
    if let Some(rest) = trimmed.strip_prefix("hex:") {
        return Ok(parse_hex_ref(rest));
    }
    if decode_hex_word(trimmed).is_ok() {
        return Ok(ServeRef::HexRange {
            start: trimmed.to_string(),
            end: trimmed.to_string(),
        });
    }

    let parts: Vec<&str> = trimmed.split(':').collect();
    if parts.len() >= 3 && parts[0].eq_ignore_ascii_case("index") {
        let registry_index = parts[1]
            .parse::<u64>()
            .map_err(|e| format!("Invalid registry index '{}': {}", parts[1], e))?;
        let kind = parts[2].to_ascii_lowercase();
        return match kind.as_str() {
            "str" | "string" => {
                if parts.len() != 4 {
                    return Err("index string spec must be index:<n>:string:<1-3>".to_string());
                }
                Ok(ServeRef::IndexString {
                    registry_index,
                    slot: parse_user_slot(parts[3], 3, "string")?,
                })
            }
            "db" | "num" | "number" => {
                if parts.len() != 4 {
                    return Err("index DB spec must be index:<n>:db:<1-9>".to_string());
                }
                Ok(ServeRef::IndexDb {
                    registry_index,
                    slot: parse_user_slot(parts[3], 9, "DB")?,
                })
            }
            "dbline" | "db-line" => Ok(ServeRef::IndexDbLine { registry_index }),
            "hexline" | "hex-word-line" | "hex_word_line" | "ascii" | "ranges" => {
                Ok(ServeRef::IndexAscii { registry_index })
            }
            "zone" | "range" | "defined" => {
                if parts.len() != 4 {
                    return Err("index zone spec must be index:<n>:zone:<1-3>".to_string());
                }
                Ok(ServeRef::IndexZone {
                    registry_index,
                    zone: parse_user_slot(parts[3], 3, "zone")?,
                })
            }
            "zonehex" | "rangehex" | "defhex" | "definedhex" => {
                if parts.len() != 4 {
                    return Err("index zonehex spec must be index:<n>:zonehex:<1-3>".to_string());
                }
                Ok(ServeRef::IndexZoneHex {
                    registry_index,
                    zone: parse_user_slot(parts[3], 3, "zone")?,
                })
            }
            _ => Err(format!("Unknown index ref kind '{}'", parts[2])),
        };
    }

    Ok(ServeRef::Literal(trimmed.to_string()))
}

fn line_range_text(
    content: &str,
    start_line: usize,
    end_line: usize,
) -> std::result::Result<String, String> {
    let lines: Vec<&str> = content.lines().collect();
    if start_line >= lines.len() || end_line >= lines.len() {
        return Err(format!(
            "Line range {}-{} is out of bounds for {} lines",
            start_line,
            end_line,
            lines.len()
        ));
    }
    Ok(lines[start_line..=end_line].join("\n"))
}

fn find_scanned_section(
    content: &str,
    registry_index: u64,
) -> std::result::Result<ScannedSection, String> {
    let matches: Vec<_> = fast_scan_content(content)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|section| section.index == registry_index)
        .collect();
    match matches.len() {
        0 => Err(format!("Registry index {} not found", registry_index)),
        1 => Ok(matches[0].clone()),
        _ => Err(format!("Registry index {} is ambiguous", registry_index)),
    }
}

fn read_ref_value(content: &str, spec: &ServeRef) -> std::result::Result<String, String> {
    match spec {
        ServeRef::Literal(value) => Ok(value.clone()),
        ServeRef::IndexString {
            registry_index,
            slot,
        } => Ok(find_scanned_section(content, *registry_index)?.strings[*slot].clone()),
        ServeRef::IndexDb {
            registry_index,
            slot,
        } => Ok(find_scanned_section(content, *registry_index)?.db_values[*slot].to_string()),
        ServeRef::IndexDbLine { registry_index } => {
            let section = find_scanned_section(content, *registry_index)?;
            Ok(section
                .db_values
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(" | "))
        }
        ServeRef::IndexAscii { registry_index } => {
            let section = find_scanned_section(content, *registry_index)?;
            let lines: Vec<&str> = content.lines().collect();
            Ok(lines.get(section.ascii_line).unwrap_or(&"").to_string())
        }
        ServeRef::IndexZone {
            registry_index,
            zone,
        } => {
            let section = find_scanned_section(content, *registry_index)?;
            let (start, end) = section.zone_pairs[*zone];
            if start == 0 && end == 0 {
                return Ok(String::new());
            }
            line_range_text(content, start as usize, end as usize)
        }
        ServeRef::IndexZoneHex {
            registry_index,
            zone,
        } => {
            let section = find_scanned_section(content, *registry_index)?;
            let (start, end) = section.zone_pairs[*zone];
            Ok(format!(
                "{} : {}",
                encode_hex_word(start, section.zone_types[*zone]),
                encode_hex_word(end, section.zone_types[*zone])
            ))
        }
        ServeRef::HexRange { start, end } => {
            let (start_line, _) = decode_hex_word(start).map_err(|e| e.to_string())?;
            let (end_line, _) = decode_hex_word(end).map_err(|e| e.to_string())?;
            if start_line > end_line {
                return Err(format!("Hex range start {} is after end {}", start, end));
            }
            line_range_text(content, start_line as usize, end_line as usize)
        }
    }
}

fn state_json(content: &str, file_path: &str) -> std::result::Result<String, String> {
    let scan = fast_scan_content(content).map_err(|e| e.to_string())?;
    let lines: Vec<&str> = content.lines().collect();
    let mut sections = Vec::new();
    for section in scan {
        let ascii = lines.get(section.ascii_line).unwrap_or(&"").to_string();
        let mut zones = Vec::new();
        for slot in 0..3 {
            let (start, end) = section.zone_pairs[slot];
            let zone_text = if start == 0 && end == 0 {
                String::new()
            } else {
                line_range_text(content, start as usize, end as usize).unwrap_or_default()
            };
            let mut zone = BTreeMap::new();
            zone.insert("slot".to_string(), serde_json::json!(slot + 1));
            zone.insert("start".to_string(), serde_json::json!(start));
            zone.insert("end".to_string(), serde_json::json!(end));
            zone.insert(
                "zone_type".to_string(),
                serde_json::json!(section.zone_types[slot].short()),
            );
            zone.insert(
                "content_len".to_string(),
                serde_json::json!(zone_text.len()),
            );
            zone.insert(
                "content_checksum".to_string(),
                serde_json::json!(crate::checksum_hex(zone_text.as_bytes())),
            );
            zones.push(zone);
        }
        let mut item = BTreeMap::new();
        item.insert("index".to_string(), serde_json::json!(section.index));
        item.insert("name".to_string(), serde_json::json!(section.name));
        item.insert("hex_word_line".to_string(), serde_json::json!(ascii));
        item.insert("ascii".to_string(), serde_json::json!(ascii));
        item.insert(
            "db_values".to_string(),
            serde_json::json!(section.db_values),
        );
        item.insert("strings".to_string(), serde_json::json!(section.strings));
        item.insert("zones".to_string(), serde_json::json!(zones));
        sections.push(item);
    }
    let body = serde_json::json!({
        "format": "regedited-native-state-v1",
        "file": file_path,
        "file_checksum": crate::checksum_hex(content.as_bytes()),
        "sections": sections
    });
    Ok(body.to_string())
}

fn handle_state(state: &ServerState) -> Response<std::io::Cursor<Vec<u8>>> {
    let content = state.content.lock().unwrap();
    match state_json(&content, &state.config.file_path) {
        Ok(body) => json_response(200, &body),
        Err(e) => json_response(500, &format!(r#"{{"error":{}}}"#, json_escape(&e))),
    }
}

fn handle_ref(url: &str, state: &ServerState) -> Response<std::io::Cursor<Vec<u8>>> {
    let params = parse_query_string(url);
    let Some(spec) = params.get("spec") else {
        return json_response(400, r#"{"error":"Missing spec parameter"}"#);
    };
    let content = state.content.lock().unwrap();
    match parse_ref_spec(spec).and_then(|parsed| read_ref_value(&content, &parsed)) {
        Ok(value) => {
            let body = format!(
                r#"{{"spec":{},"value":{},"bytes":{}}}"#,
                json_escape(spec),
                json_escape(&value),
                value.len()
            );
            json_response(200, &body)
        }
        Err(e) => json_response(400, &format!(r#"{{"error":{}}}"#, json_escape(&e))),
    }
}

fn resolve_literal_or_ref(content: &str, value: &str) -> std::result::Result<String, String> {
    let looks_like_ref = value.starts_with("index:")
        || value.starts_with("hex:")
        || value.starts_with("text:")
        || value.starts_with("literal:")
        || decode_hex_word(value).is_ok();
    if looks_like_ref {
        parse_ref_spec(value).and_then(|parsed| read_ref_value(content, &parsed))
    } else {
        Ok(value.to_string())
    }
}

fn handle_ref_bool(url: &str, state: &ServerState) -> Response<std::io::Cursor<Vec<u8>>> {
    let params = parse_query_string(url);
    let Some(left) = params.get("left") else {
        return json_response(400, r#"{"error":"Missing left parameter"}"#);
    };
    let Some(op) = params.get("op") else {
        return json_response(400, r#"{"error":"Missing op parameter"}"#);
    };
    let Some(right) = params.get("right") else {
        return json_response(400, r#"{"error":"Missing right parameter"}"#);
    };
    let content = state.content.lock().unwrap();
    let result = (|| -> std::result::Result<bool, String> {
        let left_value = resolve_literal_or_ref(&content, left)?;
        let right_value = resolve_literal_or_ref(&content, right)?;
        let normalized = op.to_ascii_lowercase();
        match normalized.as_str() {
            "contains" => Ok(left_value
                .to_lowercase()
                .contains(&right_value.to_lowercase())),
            "eq" | "==" | "=" => Ok(left_value == right_value),
            "ne" | "!=" => Ok(left_value != right_value),
            "gt" | ">" | "gte" | ">=" | "lt" | "<" | "lte" | "<=" => {
                let left_num = left_value.trim().parse::<f64>().map_err(|e| {
                    format!("Left value '{}' is not numeric: {}", left_value.trim(), e)
                })?;
                let right_num = right_value.trim().parse::<f64>().map_err(|e| {
                    format!("Right value '{}' is not numeric: {}", right_value.trim(), e)
                })?;
                Ok(match normalized.as_str() {
                    "gt" | ">" => left_num > right_num,
                    "gte" | ">=" => left_num >= right_num,
                    "lt" | "<" => left_num < right_num,
                    "lte" | "<=" => left_num <= right_num,
                    _ => unreachable!(),
                })
            }
            _ => Err(format!("Unknown op '{}'", op)),
        }
    })();

    match result {
        Ok(value) => json_response(200, &format!(r#"{{"value":{}}}"#, value)),
        Err(e) => json_response(400, &format!(r#"{{"error":{}}}"#, json_escape(&e))),
    }
}

fn handle_types() -> Response<std::io::Cursor<Vec<u8>>> {
    use crate::typed_value::list_registry_types;
    let types: Vec<BTreeMap<String, String>> = list_registry_types()
        .into_iter()
        .map(|(name, desc)| {
            let mut m = BTreeMap::new();
            m.insert("name".to_string(), name.to_string());
            m.insert("description".to_string(), desc.to_string());
            m
        })
        .collect();
    json_response(200, &serde_json::to_string(&types).unwrap_or_default())
}

fn handle_wal(state: &ServerState) -> Response<std::io::Cursor<Vec<u8>>> {
    let status = WalStatus::check(&state.config.file_path);
    match status {
        Ok(s) => {
            let body = format!(
                r#"{{"has_wal":{},"is_committed":{},"entry_count":{},"wal_path":"{}"}}"#,
                s.has_wal,
                s.is_committed,
                s.entry_count,
                s.wal_path.display()
            );
            json_response(200, &body)
        }
        Err(e) => json_response(500, &format!(r#"{{"error":"{}"}}"#, e)),
    }
}

fn handle_health(state: &ServerState) -> Response<std::io::Cursor<Vec<u8>>> {
    let header = state.header.lock().unwrap();
    let body = format!(
        r#"{{"status":"healthy","sections":{},"read_only":{}}}"#,
        header.section_count(),
        state.config.read_only
    );
    json_response(200, &body)
}

fn handle_query(mut request: Request, state: &ServerState) {
    let mut body_text = String::new();
    let response = if request.as_reader().read_to_string(&mut body_text).is_ok() {
        let parsed: serde_json::Value = match serde_json::from_str(&body_text) {
            Ok(value) => value,
            Err(e) => {
                let response = json_response(400, &format!(r#"{{"error":"Invalid JSON: {}"}}"#, e));
                if let Err(e) = request.respond(response) {
                    eprintln!("Query response error: {}", e);
                }
                return;
            }
        };
        let content = state.content.lock().unwrap();

        if let (Some(left), Some(op), Some(right)) = (
            parsed.get("left").and_then(|v| v.as_str()),
            parsed.get("op").and_then(|v| v.as_str()),
            parsed.get("right").and_then(|v| v.as_str()),
        ) {
            let left_value = resolve_literal_or_ref(&content, left);
            let right_value = resolve_literal_or_ref(&content, right);
            match (left_value, right_value) {
                (Ok(left_value), Ok(right_value)) => {
                    let normalized = op.to_ascii_lowercase();
                    let value = match normalized.as_str() {
                        "contains" => Ok(left_value
                            .to_lowercase()
                            .contains(&right_value.to_lowercase())),
                        "eq" | "==" | "=" => Ok(left_value == right_value),
                        "ne" | "!=" => Ok(left_value != right_value),
                        "gt" | ">" | "gte" | ">=" | "lt" | "<" | "lte" | "<=" => {
                            let left_num = left_value.trim().parse::<f64>().map_err(|e| {
                                format!("Left value '{}' is not numeric: {}", left_value.trim(), e)
                            });
                            let right_num = right_value.trim().parse::<f64>().map_err(|e| {
                                format!(
                                    "Right value '{}' is not numeric: {}",
                                    right_value.trim(),
                                    e
                                )
                            });
                            match (left_num, right_num) {
                                (Ok(left_num), Ok(right_num)) => Ok(match normalized.as_str() {
                                    "gt" | ">" => left_num > right_num,
                                    "gte" | ">=" => left_num >= right_num,
                                    "lt" | "<" => left_num < right_num,
                                    "lte" | "<=" => left_num <= right_num,
                                    _ => unreachable!(),
                                }),
                                (Err(e), _) | (_, Err(e)) => Err(e),
                            }
                        }
                        _ => Err(format!("Unknown op '{}'", op)),
                    };
                    match value {
                        Ok(value) => json_response(200, &format!(r#"{{"value":{}}}"#, value)),
                        Err(e) => {
                            json_response(400, &format!(r#"{{"error":{}}}"#, json_escape(&e)))
                        }
                    }
                }
                (Err(e), _) | (_, Err(e)) => {
                    json_response(400, &format!(r#"{{"error":{}}}"#, json_escape(&e)))
                }
            }
        } else if let (Some(section), Some(operation), Some(patterns)) = (
            parsed.get("section").and_then(|v| v.as_str()),
            parsed.get("operation").and_then(|v| v.as_str()),
            parsed.get("patterns").and_then(|v| v.as_array()),
        ) {
            let header = state.header.lock().unwrap();
            let Some(info) = header
                .get_section(section)
                .or_else(|| header.get_section_case_insensitive(section))
            else {
                let response = json_response(
                    404,
                    &format!(r#"{{"error":"Section '{}' not found"}}"#, section),
                );
                if let Err(e) = request.respond(response) {
                    eprintln!("Query response error: {}", e);
                }
                return;
            };
            let section_text =
                line_range_text(&content, info.content_start, info.content_end).unwrap_or_default();
            let pats: Vec<String> = patterns
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            let lower = section_text.to_lowercase();
            let value = match operation.to_ascii_lowercase().as_str() {
                "and" => pats.iter().all(|p| lower.contains(&p.to_lowercase())),
                "or" => pats.iter().any(|p| lower.contains(&p.to_lowercase())),
                _ => {
                    let response = json_response(
                        400,
                        &format!(r#"{{"error":"Unknown operation '{}'"}}"#, operation),
                    );
                    if let Err(e) = request.respond(response) {
                        eprintln!("Query response error: {}", e);
                    }
                    return;
                }
            };
            json_response(200, &format!(r#"{{"value":{}}}"#, value))
        } else {
            json_response(
                400,
                r#"{"error":"Use {left,op,right} or {section,operation,patterns}"}"#,
            )
        }
    } else {
        json_response(400, r#"{"error": "Failed to read request body"}"#)
    };
    if let Err(e) = request.respond(response) {
        eprintln!("Query response error: {}", e);
    }
}

// ==================== UTILITIES ====================

fn json_response(status: u16, body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let data = body.as_bytes().to_vec();
    let status = match status {
        200 => StatusCode(200),
        400 => StatusCode(400),
        404 => StatusCode(404),
        500 => StatusCode(500),
        _ => StatusCode(status),
    };

    Response::new(
        status,
        vec![
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
        ],
        std::io::Cursor::new(data),
        Some(body.len()),
        None,
    )
}

fn parse_query_string(url: &str) -> BTreeMap<String, String> {
    let mut params = BTreeMap::new();
    if let Some(pos) = url.find('?') {
        let query = &url[pos + 1..];
        for pair in query.split('&') {
            let parts: Vec<&str> = pair.splitn(2, '=').collect();
            if parts.len() == 2 {
                params.insert(url_decode(parts[0]), url_decode(parts[1]));
            }
        }
    }
    params
}

fn url_decode(s: &str) -> String {
    percent_decode(s)
}

fn percent_decode(s: &str) -> String {
    let mut result = String::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                result.push(hex as char);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}
