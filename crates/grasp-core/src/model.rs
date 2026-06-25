//! Shared data types for the Grasp engine.

use serde::{Deserialize, Serialize};

/// A single tool invocation found inside an assistant message.
#[derive(Debug, Clone)]
pub struct ToolUse {
    pub name: String,
    /// Raw JSON input the tool was called with.
    pub input: serde_json::Value,
}

/// A tool result block found inside a user message.
#[derive(Debug, Clone)]
pub struct ToolResult {
    /// Name of the tool this result is for, if we could resolve it (often unknown).
    pub tool_name: Option<String>,
    pub content: String,
    pub is_error: bool,
}

/// A normalized transcript entry. Anything we don't understand becomes `Other`
/// so the parser can never crash on unfamiliar lines.
#[derive(Debug, Clone)]
pub enum Entry {
    User {
        text: String,
        tool_results: Vec<ToolResult>,
        timestamp: Option<String>,
        session_id: Option<String>,
        cwd: Option<String>,
    },
    Assistant {
        text: String,
        thinking: String,
        tool_uses: Vec<ToolUse>,
        timestamp: Option<String>,
        session_id: Option<String>,
    },
    Summary {
        text: String,
        timestamp: Option<String>,
        session_id: Option<String>,
    },
    /// Recognized-but-ignored, or entirely unknown line types.
    Other,
}

impl Entry {
    pub fn timestamp(&self) -> Option<&str> {
        match self {
            Entry::User { timestamp, .. }
            | Entry::Assistant { timestamp, .. }
            | Entry::Summary { timestamp, .. } => timestamp.as_deref(),
            Entry::Other => None,
        }
    }

    pub fn session_id(&self) -> Option<&str> {
        match self {
            Entry::User { session_id, .. }
            | Entry::Assistant { session_id, .. }
            | Entry::Summary { session_id, .. } => session_id.as_deref(),
            Entry::Other => None,
        }
    }
}

/// The kind of memory a chunk represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkType {
    Decision,
    FileWrite,
    ErrorResolution,
    Summary,
    Context,
}

impl ChunkType {
    pub fn as_str(self) -> &'static str {
        match self {
            ChunkType::Decision => "decision",
            ChunkType::FileWrite => "file_write",
            ChunkType::ErrorResolution => "error_resolution",
            ChunkType::Summary => "summary",
            ChunkType::Context => "context",
        }
    }

    /// Human-friendly heading used in the markdown output.
    pub fn heading(self) -> &'static str {
        match self {
            ChunkType::Decision => "Decision",
            ChunkType::FileWrite => "File Write",
            ChunkType::ErrorResolution => "Error Resolution",
            ChunkType::Summary => "Summary",
            ChunkType::Context => "Context",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "decision" => ChunkType::Decision,
            "file_write" => ChunkType::FileWrite,
            "error_resolution" => ChunkType::ErrorResolution,
            "summary" => ChunkType::Summary,
            "context" => ChunkType::Context,
            _ => return None,
        })
    }
}

/// An extracted memory, ready to be written to markdown + SQLite.
#[derive(Debug, Clone, Serialize)]
pub struct Chunk {
    pub project: String,
    pub session_id: String,
    /// SHA-256 of the normalized text; the dedup key.
    pub hash: String,
    pub text: String,
    /// ISO-8601 timestamp from the source entry (or import time if absent).
    pub timestamp: String,
    pub chunk_type: ChunkType,
}

/// A search hit returned from the store.
#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub id: i64,
    pub project: String,
    pub session_id: String,
    pub text: String,
    pub timestamp: String,
    pub chunk_type: String,
    pub md_path: String,
    /// Lower is better (raw bm25 score from SQLite).
    pub score: f64,
}
