//! Tolerant JSONL transcript parser.
//!
//! Real Claude Code transcripts contain many entry types beyond the documented
//! set (`queue-operation`, `attachment`, `file-history-snapshot`, `ai-title`,
//! `mode`, `last-prompt`, `system`, ...), and `tool_use`/`tool_result` blocks
//! are *nested* inside the `message.content` arrays of `user`/`assistant`
//! entries — not top-level. We parse each line into a `serde_json::Value` and
//! navigate defensively so an unfamiliar shape can never panic: anything we
//! don't recognize becomes `Entry::Other`.

use serde_json::Value;

use crate::model::{Entry, ToolResult, ToolUse};

/// Parse one JSONL line. Returns `None` only for blank lines or invalid JSON.
pub fn parse_line(line: &str) -> Option<Entry> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let v: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!("skipping unparseable JSONL line: {e}");
            return None;
        }
    };
    Some(parse_value(&v))
}

fn parse_value(v: &Value) -> Entry {
    let entry_type = v.get("type").and_then(Value::as_str).unwrap_or("");
    let timestamp = string_field(v, "timestamp");
    let session_id = string_field(v, "sessionId");
    let cwd = string_field(v, "cwd");

    match entry_type {
        "user" => {
            let (text, tool_results) = parse_user_content(v.get("message"));
            Entry::User {
                text,
                tool_results,
                timestamp,
                session_id,
                cwd,
            }
        }
        "assistant" => {
            let (text, thinking, tool_uses) = parse_assistant_content(v.get("message"));
            Entry::Assistant {
                text,
                thinking,
                tool_uses,
                timestamp,
                session_id,
            }
        }
        "summary" => {
            // Compaction summaries store text under "summary".
            let text = v
                .get("summary")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_default();
            Entry::Summary {
                text,
                timestamp,
                session_id,
            }
        }
        _ => Entry::Other,
    }
}

/// User `message.content` is either a plain string (the prompt) or an array of
/// blocks (text + nested `tool_result`s).
fn parse_user_content(message: Option<&Value>) -> (String, Vec<ToolResult>) {
    let mut text = String::new();
    let mut results = Vec::new();

    let Some(content) = message.and_then(|m| m.get("content")) else {
        return (text, results);
    };

    match content {
        Value::String(s) => text.push_str(s),
        Value::Array(blocks) => {
            for block in blocks {
                match block.get("type").and_then(Value::as_str) {
                    Some("text") => push_text(&mut text, block.get("text")),
                    Some("tool_result") => {
                        results.push(ToolResult {
                            tool_name: None,
                            content: extract_block_text(block.get("content")),
                            is_error: block
                                .get("is_error")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                        });
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    (text, results)
}

/// Assistant `message.content` is an array of `text` / `thinking` / `tool_use` blocks.
fn parse_assistant_content(message: Option<&Value>) -> (String, String, Vec<ToolUse>) {
    let mut text = String::new();
    let mut thinking = String::new();
    let mut tool_uses = Vec::new();

    let Some(content) = message.and_then(|m| m.get("content")) else {
        return (text, thinking, tool_uses);
    };

    match content {
        Value::String(s) => text.push_str(s),
        Value::Array(blocks) => {
            for block in blocks {
                match block.get("type").and_then(Value::as_str) {
                    Some("text") => push_text(&mut text, block.get("text")),
                    Some("thinking") => push_text(&mut thinking, block.get("thinking")),
                    Some("tool_use") => {
                        if let Some(name) = block.get("name").and_then(Value::as_str) {
                            tool_uses.push(ToolUse {
                                name: name.to_string(),
                                input: block.get("input").cloned().unwrap_or(Value::Null),
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    (text, thinking, tool_uses)
}

/// A `tool_result`/content field can be a string or an array of `{type,text}` blocks.
fn extract_block_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(blocks)) => {
            let mut out = String::new();
            for block in blocks {
                push_text(&mut out, block.get("text"));
            }
            out
        }
        _ => String::new(),
    }
}

fn push_text(buf: &mut String, field: Option<&Value>) {
    if let Some(s) = field.and_then(Value::as_str) {
        if !buf.is_empty() {
            buf.push('\n');
        }
        buf.push_str(s);
    }
}

fn string_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Entry;

    #[test]
    fn parses_plain_user_prompt() {
        let line = r#"{"type":"user","timestamp":"t","sessionId":"s","cwd":"c","message":{"role":"user","content":"fix the auth bug"}}"#;
        match parse_line(line).unwrap() {
            Entry::User { text, .. } => assert_eq!(text, "fix the auth bug"),
            other => panic!("expected User, got {other:?}"),
        }
    }

    #[test]
    fn parses_assistant_text_and_tool_use() {
        let line = r#"{"type":"assistant","sessionId":"s","message":{"role":"assistant","content":[{"type":"text","text":"Let me check"},{"type":"tool_use","name":"Write","input":{"file_path":"a.rs"}}]}}"#;
        match parse_line(line).unwrap() {
            Entry::Assistant { text, tool_uses, .. } => {
                assert_eq!(text, "Let me check");
                assert_eq!(tool_uses.len(), 1);
                assert_eq!(tool_uses[0].name, "Write");
            }
            other => panic!("expected Assistant, got {other:?}"),
        }
    }

    #[test]
    fn nested_tool_result_in_user_message() {
        let line = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","is_error":true,"content":"boom"}]}}"#;
        match parse_line(line).unwrap() {
            Entry::User { tool_results, .. } => {
                assert_eq!(tool_results.len(), 1);
                assert!(tool_results[0].is_error);
                assert_eq!(tool_results[0].content, "boom");
            }
            other => panic!("expected User, got {other:?}"),
        }
    }

    #[test]
    fn unknown_types_never_panic() {
        for line in [
            r#"{"type":"queue-operation","operation":"enqueue"}"#,
            r#"{"type":"file-history-snapshot"}"#,
            r#"{"type":"ai-title","title":"x"}"#,
            r#"{"weird":"no type at all"}"#,
            r#"not json at all"#,
            r#""#,
        ] {
            // Should be Some(Other) or None, but never panic.
            let _ = parse_line(line);
        }
    }
}
