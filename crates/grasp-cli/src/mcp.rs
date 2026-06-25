//! Minimal MCP server over stdio (JSON-RPC 2.0, newline-delimited).
//!
//! Exposes three tools to Claude Code: `query_memory`, `save_context`,
//! `list_projects`. The transport is just JSON objects separated by newlines on
//! stdin/stdout — so logging MUST go to stderr (configured in main.rs) to avoid
//! corrupting the protocol stream.

use std::io::{BufRead, Write};
use std::path::Path;

use anyhow::Result;
use grasp_core::Grasp;
use serde_json::{json, Value};

const DEFAULT_PROTOCOL: &str = "2024-11-05";

/// Run the MCP server loop until stdin closes.
pub fn run(grasp: Grasp) -> Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("ignoring invalid JSON-RPC line: {e}");
                continue;
            }
        };

        if let Some(resp) = handle(&grasp, &req) {
            writeln!(out, "{}", serde_json::to_string(&resp)?)?;
            out.flush()?;
        }
    }
    Ok(())
}

/// Returns `Some(response)` for requests, `None` for notifications.
fn handle(grasp: &Grasp, req: &Value) -> Option<Value> {
    let method = req.get("method").and_then(Value::as_str).unwrap_or("");
    let id = req.get("id").cloned();
    let params = req.get("params").cloned().unwrap_or(Value::Null);

    // Notifications have no id and expect no response.
    if id.is_none() {
        return None;
    }
    let id = id.unwrap();

    match method {
        "initialize" => {
            let protocol = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_PROTOCOL)
                .to_string();
            Some(ok(
                id,
                json!({
                    "protocolVersion": protocol,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "grasp", "version": env!("CARGO_PKG_VERSION") }
                }),
            ))
        }
        "ping" => Some(ok(id, json!({}))),
        "tools/list" => Some(ok(id, json!({ "tools": tool_definitions() }))),
        "tools/call" => Some(call_tool(grasp, id, &params)),
        other => Some(err(id, -32601, &format!("method not found: {other}"))),
    }
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": "query_memory",
            "description": "Recall what was decided, built, or fixed in PAST sessions on this project. \
                Call this proactively at the start of a task, or whenever the user refers to earlier \
                work ('what did we decide', 'how did we fix', 'last time', 'why did we'). \
                Defaults to the current project automatically; pass `project` only to search a different one.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "What to search for" },
                    "project": { "type": "string", "description": "Optional project slug filter" },
                    "limit": { "type": "integer", "description": "Max results (default 5)" }
                },
                "required": ["query"]
            }
        },
        {
            "name": "save_context",
            "description": "Explicitly save a note to long-term memory.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": { "type": "string" },
                    "project": { "type": "string" },
                    "type": { "type": "string", "enum": ["decision", "context", "note"] }
                },
                "required": ["text"]
            }
        },
        {
            "name": "list_projects",
            "description": "List all indexed projects with memory counts.",
            "inputSchema": { "type": "object", "properties": {} }
        }
    ])
}

fn call_tool(grasp: &Grasp, id: Value, params: &Value) -> Value {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(Value::Null);

    let result: Result<String> = match name {
        "query_memory" => tool_query_memory(grasp, &args),
        "save_context" => tool_save_context(grasp, &args),
        "list_projects" => tool_list_projects(grasp),
        other => return err(id, -32602, &format!("unknown tool: {other}")),
    };

    match result {
        Ok(text) => ok(id, json!({ "content": [{ "type": "text", "text": text }] })),
        Err(e) => ok(
            id,
            json!({
                "content": [{ "type": "text", "text": format!("error: {e}") }],
                "isError": true
            }),
        ),
    }
}

fn tool_query_memory(grasp: &Grasp, args: &Value) -> Result<String> {
    let query = args.get("query").and_then(Value::as_str).unwrap_or("");
    if query.trim().is_empty() {
        anyhow::bail!("query is required");
    }
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .unwrap_or(5);

    // Explicit project arg wins; otherwise default to the project this server
    // was launched in, so a query from one repo doesn't surface another's memory.
    let explicit = args.get("project").and_then(Value::as_str);
    let inferred = if explicit.is_none() {
        current_project_slug(grasp)
    } else {
        None
    };
    let project = explicit.or(inferred.as_deref());

    let hits = grasp.search(query, project, limit)?;
    if hits.is_empty() {
        return Ok(format!("No memories found for {query:?}."));
    }
    let mut out = String::new();
    for h in hits {
        let date = h.timestamp.split('T').next().unwrap_or(&h.timestamp);
        out.push_str(&format!(
            "- [{}] ({}, {}) {}\n",
            date, h.chunk_type, h.project, h.text
        ));
    }
    Ok(out)
}

/// Encode a working directory the way Claude Code names its transcript folders:
/// every non-alphanumeric character becomes `-` (e.g. `C:\projects\Grasp`
/// -> `C--projects-Grasp`). Case is normalized away by the caller's compare.
fn encode_claude_slug(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Best-effort: which indexed project is the server's current directory? Returns
/// `Some(slug)` only if the encoded cwd matches a real project (case-insensitive),
/// so a wrong guess safely falls back to searching all projects.
fn current_project_slug(grasp: &Grasp) -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    let encoded = encode_claude_slug(&cwd);
    // Pick the richest match: drive-letter case differences can leave two rows
    // for one project (e.g. `C--…` empty, `c--…` full) — prefer the full one.
    grasp
        .projects()
        .ok()?
        .into_iter()
        .filter(|p| p.slug.eq_ignore_ascii_case(&encoded))
        .max_by_key(|p| p.chunk_count)
        .map(|p| p.slug)
}

fn tool_save_context(grasp: &Grasp, args: &Value) -> Result<String> {
    let text = args.get("text").and_then(Value::as_str).unwrap_or("");
    if text.trim().is_empty() {
        anyhow::bail!("text is required");
    }
    let project = args.get("project").and_then(Value::as_str);
    let type_ = args.get("type").and_then(Value::as_str);
    let added = grasp.save_context(text, project, type_)?;
    Ok(if added {
        "Saved to memory.".to_string()
    } else {
        "Already in memory (duplicate).".to_string()
    })
}

fn tool_list_projects(grasp: &Grasp) -> Result<String> {
    let projects = grasp.projects()?;
    if projects.is_empty() {
        return Ok("No projects indexed yet.".to_string());
    }
    let mut out = String::new();
    for p in projects {
        out.push_str(&format!("- {} ({} memories)\n", p.slug, p.chunk_count));
    }
    Ok(out)
}

fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn err(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}
