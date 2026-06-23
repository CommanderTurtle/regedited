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
//! | GET | `/section/{name}/ascii` | Get hex-word store |
//! | GET | `/section/{name}/zone/{index}` | Extract zone content |
//! | GET | `/grep?pattern={p}&section={s}` | Search for pattern |
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
    header::{scan_content, DocumentHeader},
    wal::WalStatus,
    zone_editor::{extract_zone_content, format_zone_info},
    zone_type::ZoneType,
    Result,
};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
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
    let server = Server::http(&addr)
        .map_err(|e| crate::RegeditedError::Io(std::io::Error::new(
            std::io::ErrorKind::Other, format!("Failed to bind to {}: {}", addr, e)
        )))?;

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
    println!("  GET /section/{{name}}/zone/{{i}} — Zone content");
    println!("  GET /grep?pattern= &section= — Search");
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
        let response = handle_query(request, &state);
        // handle_query calls respond internally
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
        (Method::Get, path) if path.starts_with("/section/") && path.ends_with("/ascii") => {
            handle_section_ascii(path, &state)
        }
        (Method::Get, path) if path.starts_with("/section/") => {
            handle_section(path, &state)
        }
        (Method::Get, "/grep") => handle_grep(request.url(), &state),
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
    let sections: Vec<String> = header.section_names().iter().map(|s| s.to_string()).collect();

    let body = format!(
        r#"{{"status":"ok","regedited":"0.1.0","sections":{},"read_only":{},"sections_count":{}}}"#,
        serde_json::to_string(&sections).unwrap_or_default(),
        state.config.read_only,
        header.section_count()
    );
    json_response(200, &body)
}

fn handle_sections(state: &ServerState) -> Response<std::io::Cursor<Vec<u8>>> {
    let header = state.header.lock().unwrap();
    let sections: Vec<BTreeMap<String, String>> = header.sections.iter().map(|(name, info)| {
        let mut map = BTreeMap::new();
        map.insert("name".to_string(), name.clone());
        map.insert("header_line".to_string(), info.header_line.to_string());
        map.insert("content_start".to_string(), info.content_start.to_string());
        map.insert("content_end".to_string(), info.content_end.to_string());
        map.insert("total_lines".to_string(), info.total_lines().to_string());
        map
    }).collect();

    let body = serde_json::to_string(&sections).unwrap_or_default();
    json_response(200, &body)
}

fn handle_section(path: &str, state: &ServerState) -> Response<std::io::Cursor<Vec<u8>>> {
    let name = path.trim_start_matches("/section/");
    let name = name.trim_end_matches("/");

    let header = state.header.lock().unwrap();
    let content = state.content.lock().unwrap();

    if let Some(info) = header.get_section(name)
        .or_else(|| header.get_section_case_insensitive(name)) {

        let lines: Vec<&str> = content.lines().collect();
        let db_line = if info.numeric_line < lines.len() {
            lines[info.numeric_line]
        } else {
            ""
        };

        let body = format!(
            r#"{{"name":"{}","header_line":{},"index_line":{},"ascii_line":{},"numeric_line":{},"content_start":{},"content_end":{},"db_line":"{}","total_lines":{}}}"#,
            name, info.header_line, info.index_line, info.ascii_line,
            info.numeric_line, info.content_start, info.content_end,
            db_line.replace('"', "\\\""),
            info.total_lines()
        );
        json_response(200, &body)
    } else {
        json_response(404, &format!(r#"{{"error": "Section '{}' not found"}}"#, name))
    }
}

fn handle_section_db(path: &str, state: &ServerState) -> Response<std::io::Cursor<Vec<u8>>> {
    let name = path.trim_start_matches("/section/");
    let name = name.trim_end_matches("/db");

    let header = state.header.lock().unwrap();
    let content = state.content.lock().unwrap();

    if let Some(info) = header.get_section(name)
        .or_else(|| header.get_section_case_insensitive(name)) {

        let lines: Vec<&str> = content.lines().collect();

        // Extract index, ascii store, numeric line, and strings
        let index = if info.header_line + 1 < lines.len() {
            lines[info.header_line + 1]
        } else { "" };
        let ascii = if info.header_line + 2 < lines.len() {
            lines[info.header_line + 2]
        } else { "" };
        let numeric = if info.numeric_line < lines.len() {
            lines[info.numeric_line]
        } else { "" };

        let str1 = if info.string1_line < lines.len() { lines[info.string1_line] } else { "" };
        let str2 = if info.string2_line < lines.len() { lines[info.string2_line] } else { "" };
        let str3 = if info.string3_line < lines.len() { lines[info.string3_line] } else { "" };

        let db_values: Vec<i64> = numeric.split('\t')
            .filter_map(|s| s.parse().ok())
            .collect();

        let body = format!(
            r#"{{"section":"{}","index":{},"ascii_store":"{}","db_values":{},"strings":["{}","{}","{}"]}}"#,
            name,
            index,
            ascii,
            serde_json::to_string(&db_values).unwrap_or_default(),
            str1.replace('"', "\\\""),
            str2.replace('"', "\\\""),
            str3.replace('"', "\\\"")
        );
        json_response(200, &body)
    } else {
        json_response(404, &format!(r#"{{"error": "Section '{}' not found"}}"#, name))
    }
}

fn handle_section_ascii(path: &str, state: &ServerState) -> Response<std::io::Cursor<Vec<u8>>> {
    let name = path.trim_start_matches("/section/");
    let name = name.trim_end_matches("/ascii");

    let header = state.header.lock().unwrap();
    let content = state.content.lock().unwrap();

    if let Some(info) = header.get_section(name)
        .or_else(|| header.get_section_case_insensitive(name)) {
        let lines: Vec<&str> = content.lines().collect();
        let ascii = if info.header_line + 2 < lines.len() {
            lines[info.header_line + 2]
        } else { "" };

        json_response(200, &format!(r#"{{"section":"{}","ascii_store":"{}"}}"#, name, ascii))
    } else {
        json_response(404, &format!(r#"{{"error":"Section '{}' not found"}}"#, name))
    }
}

fn handle_section_zone(path: &str, state: &ServerState) -> Response<std::io::Cursor<Vec<u8>>> {
    // Format: /section/{name}/zone/{index}
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 5 {
        return json_response(400, r#"{"error": "Invalid path. Use /section/{name}/zone/{index}"}"#);
    }

    let name = parts[2];
    let zone_idx: usize = parts[4].parse().unwrap_or(0);

    let header = state.header.lock().unwrap();
    let content = state.content.lock().unwrap();

    if let Some(info) = header.get_section(name)
        .or_else(|| header.get_section_case_insensitive(name)) {

        match extract_zone_content(&content, info, zone_idx) {
            Ok(zone_content) => {
                let body = format!(
                    r#"{{"section":"{}","zone":{},"content":"{}"}}"#,
                    name, zone_idx, zone_content.replace('"', "\\\"").replace('\n', "\\n")
                );
                json_response(200, &body)
            }
            Err(e) => json_response(500, &format!(r#"{{"error":"{}"}}"#, e)),
        }
    } else {
        json_response(404, &format!(r#"{{"error":"Section '{}' not found"}}"#, name))
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

        for (i, line) in lines.iter().enumerate().skip(info.content_start).take(
            info.content_end.saturating_sub(info.content_start) + 1
        ) {
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

fn handle_types() -> Response<std::io::Cursor<Vec<u8>>> {
    use crate::typed_value::list_registry_types;
    let types: Vec<BTreeMap<String, String>> = list_registry_types().into_iter().map(|(name, desc)| {
        let mut m = BTreeMap::new();
        m.insert("name".to_string(), name.to_string());
        m.insert("description".to_string(), desc.to_string());
        m
    }).collect();
    json_response(200, &serde_json::to_string(&types).unwrap_or_default())
}

fn handle_wal(state: &ServerState) -> Response<std::io::Cursor<Vec<u8>>> {
    let status = WalStatus::check(&state.config.file_path);
    match status {
        Ok(s) => {
            let body = format!(
                r#"{{"has_wal":{},"is_committed":{},"entry_count":{},"wal_path":"{}"}}"#,
                s.has_wal, s.is_committed, s.entry_count, s.wal_path.display()
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

fn handle_query(mut request: Request, _state: &ServerState) {
    let mut content = String::new();
    let response = if request.as_reader().read_to_string(&mut content).is_ok() {
        // Parse simple JSON query: {"section":"X","operation":"and","patterns":["a","b"]}
        // For now, return a placeholder
        json_response(200, r#"{"status":"ok","note":"Boolean query endpoint - implement with request body parsing"}"#)
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
                params.insert(
                    url_decode(parts[0]),
                    url_decode(parts[1])
                );
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
            if let Ok(hex) = u8::from_str_radix(&s[i+1..i+3], 16) {
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
