//! Signal extraction.
//!
//! Turns a stream of parsed `Entry`s (one session, in order) into `Chunk`s
//! worth remembering, applying the CLAUDE.md signal rules. The goal is recall
//! of *meaningful* moments — decisions, file writes, error fixes, summaries,
//! and substantive user questions — while dropping noise.

use crate::model::{Chunk, ChunkType, Entry};
use crate::util::{hash_text, normalize, truncate};

/// Keywords that mark an assistant message as a "decision".
const DECISION_KEYWORDS: &[&str] = &[
    "decided",
    "because",
    "instead of",
    "approach",
    "architecture",
    "will use",
    "won't use",
    "wont use",
];

/// Tool names whose use we treat as a file write.
const WRITE_TOOLS: &[&str] = &["Write", "Edit", "MultiEdit", "NotebookEdit"];

/// Max characters kept per chunk (keeps chunks ~300 tokens).
const MAX_CHUNK_CHARS: usize = 1200;

/// Bash output longer than this is considered noise and skipped.
const MAX_ERROR_CHARS: usize = 600;

/// Extract chunks from one session's entries (already in chronological order).
///
/// `project` is the project slug; `default_ts` is used when an entry has no
/// timestamp of its own (e.g. an import-time fallback).
pub fn extract_session(entries: &[Entry], project: &str, default_ts: &str) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut seen_hashes = std::collections::HashSet::new();

    // For pairing an error with the assistant text that resolves it.
    let mut pending_error: Option<String> = None;
    // Guard against the same error repeating 3+ times in a row.
    let mut last_error_text = String::new();
    let mut repeat_count = 0usize;

    for entry in entries {
        let ts = entry.timestamp().unwrap_or(default_ts).to_string();
        let session = entry.session_id().unwrap_or("unknown").to_string();

        match entry {
            Entry::Summary { text, .. } => {
                push_chunk(
                    &mut chunks,
                    &mut seen_hashes,
                    project,
                    &session,
                    &ts,
                    ChunkType::Summary,
                    text,
                );
            }

            Entry::User {
                text,
                tool_results,
                ..
            } => {
                // Substantive user questions → context.
                if is_question(text) {
                    push_chunk(
                        &mut chunks,
                        &mut seen_hashes,
                        project,
                        &session,
                        &ts,
                        ChunkType::Context,
                        text,
                    );
                }

                // A failing tool result arms an error-resolution pairing.
                for result in tool_results {
                    if !result.is_error {
                        continue;
                    }
                    let err = normalize(&result.content);
                    if err.is_empty() || err.len() > MAX_ERROR_CHARS {
                        continue; // skip empty / huge log dumps
                    }
                    // Suppress error loops (same error 3+ times running).
                    if err == last_error_text {
                        repeat_count += 1;
                        if repeat_count >= 2 {
                            continue;
                        }
                    } else {
                        last_error_text = err.clone();
                        repeat_count = 0;
                    }
                    pending_error = Some(err);
                }
            }

            Entry::Assistant {
                text, tool_uses, ..
            } => {
                // File writes/edits.
                for tu in tool_uses {
                    if WRITE_TOOLS.contains(&tu.name.as_str()) {
                        if let Some(path) = tu.input.get("file_path").and_then(|v| v.as_str()) {
                            let note = first_sentence(text);
                            let body = if note.is_empty() {
                                format!("{} `{}`", tu.name, path)
                            } else {
                                format!("{} `{}` — {}", tu.name, path, note)
                            };
                            push_chunk(
                                &mut chunks,
                                &mut seen_hashes,
                                project,
                                &session,
                                &ts,
                                ChunkType::FileWrite,
                                &body,
                            );
                        }
                    }
                }

                // Error resolution: the first assistant text after a failing result.
                if let Some(err) = pending_error.take() {
                    if !text.trim().is_empty() {
                        let body = format!(
                            "Fixed: {}\nResolution: {}",
                            truncate(&err, 300),
                            first_sentence(text)
                        );
                        push_chunk(
                            &mut chunks,
                            &mut seen_hashes,
                            project,
                            &session,
                            &ts,
                            ChunkType::ErrorResolution,
                            &body,
                        );
                    }
                }

                // Decisions.
                if contains_keyword(text, DECISION_KEYWORDS) {
                    push_chunk(
                        &mut chunks,
                        &mut seen_hashes,
                        project,
                        &session,
                        &ts,
                        ChunkType::Decision,
                        text,
                    );
                }
            }

            Entry::Other => {}
        }
    }

    chunks
}

#[allow(clippy::too_many_arguments)]
fn push_chunk(
    chunks: &mut Vec<Chunk>,
    seen: &mut std::collections::HashSet<String>,
    project: &str,
    session: &str,
    ts: &str,
    chunk_type: ChunkType,
    raw_text: &str,
) {
    // Scrub secrets before anything is stored or hashed (issue #1). Done before
    // truncation so a redacted block can't be sliced in half.
    let scrubbed = crate::redact::scrub(raw_text.trim());
    let text = truncate(scrubbed.trim(), MAX_CHUNK_CHARS);
    if text.is_empty() {
        return;
    }
    // Dedup key = type + normalized text, so identical content isn't stored twice.
    let hash = hash_text(&format!("{}|{}", chunk_type.as_str(), normalize(&text)));
    if !seen.insert(hash.clone()) {
        return; // already produced in this session
    }
    chunks.push(Chunk {
        project: project.to_string(),
        session_id: session.to_string(),
        hash,
        text,
        timestamp: ts.to_string(),
        chunk_type,
    });
}

fn contains_keyword(text: &str, keywords: &[&str]) -> bool {
    let lower = text.to_lowercase();
    keywords.iter().any(|k| lower.contains(k))
}

/// A user message counts as a "question" if it asks why/how/should or ends with '?'.
fn is_question(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() || t.len() < 8 {
        return false;
    }
    if t.ends_with('?') {
        return true;
    }
    let lower = t.to_lowercase();
    ["why ", "how ", "should ", "what ", "when should"]
        .iter()
        .any(|k| lower.contains(k))
}

/// First sentence (or first line) of a block of text, for compact summaries.
fn first_sentence(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let end = trimmed
        .find(['.', '\n'])
        .map(|i| i + 1)
        .unwrap_or(trimmed.len());
    truncate(trimmed[..end].trim(), 200)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ToolResult, ToolUse};
    use serde_json::json;

    fn assistant(text: &str, tools: Vec<ToolUse>) -> Entry {
        Entry::Assistant {
            text: text.to_string(),
            thinking: String::new(),
            tool_uses: tools,
            timestamp: Some("2026-01-01T00:00:00Z".into()),
            session_id: Some("s".into()),
        }
    }

    #[test]
    fn extracts_decision_and_file_write() {
        let entries = vec![assistant(
            "We decided to use GKE because Minikube was too slow.",
            vec![ToolUse {
                name: "Write".into(),
                input: json!({"file_path": "src/main.rs"}),
            }],
        )];
        let chunks = extract_session(&entries, "proj", "now");
        let types: Vec<_> = chunks.iter().map(|c| c.chunk_type).collect();
        assert!(types.contains(&ChunkType::Decision));
        assert!(types.contains(&ChunkType::FileWrite));
    }

    #[test]
    fn pairs_error_with_resolution() {
        let entries = vec![
            Entry::User {
                text: String::new(),
                tool_results: vec![ToolResult {
                    tool_name: None,
                    content: "context deadline exceeded".into(),
                    is_error: true,
                }],
                timestamp: Some("t".into()),
                session_id: Some("s".into()),
                cwd: None,
            },
            assistant("Added a 30s timeout to the REST client.", vec![]),
        ];
        let chunks = extract_session(&entries, "proj", "now");
        assert!(chunks.iter().any(|c| c.chunk_type == ChunkType::ErrorResolution));
    }

    #[test]
    fn extracts_user_question_as_context() {
        let entries = vec![Entry::User {
            text: "why is the auth token expiring early?".into(),
            tool_results: vec![],
            timestamp: Some("t".into()),
            session_id: Some("s".into()),
            cwd: None,
        }];
        let chunks = extract_session(&entries, "proj", "now");
        assert!(chunks.iter().any(|c| c.chunk_type == ChunkType::Context));
    }
}
