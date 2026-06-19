//! Human-readable markdown output — the "source of truth".
//!
//! Each chunk is appended to `memory/<slug>/YYYY-MM-DD.md` in the CLAUDE.md
//! format, with an HTML metadata comment carrying the hash/type/session.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};

use crate::config::Config;
use crate::model::Chunk;

/// Append a chunk to its project's daily markdown file. Returns the file path
/// (relative-friendly, stored alongside the chunk in SQLite).
pub fn append_chunk(config: &Config, chunk: &Chunk) -> Result<PathBuf> {
    let (date, time) = date_and_time(&chunk.timestamp);

    let dir = config.memory_project_dir(&chunk.project);
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("creating memory dir {}", dir.display()))?;
    let path = dir.join(format!("{date}.md"));

    // Write a title header the first time we touch a day's file.
    let needs_header = !path.exists() || file_is_empty(&path)?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("opening {}", path.display()))?;

    if needs_header {
        writeln!(file, "# Session Memory — {date}\n")?;
    }

    writeln!(file, "## [{time}] {}", chunk.chunk_type.heading())?;
    writeln!(file, "{}", chunk.text)?;
    writeln!(
        file,
        "<!-- engram:hash:{} type:{} session:{} -->\n",
        chunk.hash,
        chunk.chunk_type.as_str(),
        chunk.session_id
    )?;

    Ok(path)
}

fn file_is_empty(path: &PathBuf) -> Result<bool> {
    let mut s = String::new();
    File::open(path)?.read_to_string(&mut s).ok();
    Ok(s.trim().is_empty())
}

/// Parse an ISO-8601 timestamp into (`YYYY-MM-DD`, `HH:MM`). Falls back to
/// today's date / `??:??` if the timestamp can't be parsed.
fn date_and_time(timestamp: &str) -> (String, String) {
    if let Ok(dt) = timestamp.parse::<DateTime<Utc>>() {
        return (
            dt.format("%Y-%m-%d").to_string(),
            dt.format("%H:%M").to_string(),
        );
    }
    let now = Utc::now();
    (now.format("%Y-%m-%d").to_string(), "??:??".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ChunkType;

    #[test]
    fn parses_timestamp() {
        let (d, t) = date_and_time("2026-01-15T10:32:00Z");
        assert_eq!(d, "2026-01-15");
        assert_eq!(t, "10:32");
    }

    #[test]
    fn bad_timestamp_falls_back() {
        let (_d, t) = date_and_time("not-a-time");
        assert_eq!(t, "??:??");
    }

    #[test]
    fn writes_file_with_header_and_metadata() {
        let tmp = std::env::temp_dir().join(format!("engram-md-test-{}", std::process::id()));
        let config = Config {
            claude_projects_dir: tmp.clone(),
            data_dir: tmp.clone(),
        };
        let chunk = Chunk {
            project: "proj".into(),
            session_id: "sess".into(),
            hash: "abc123".into(),
            text: "Switched to GKE.".into(),
            timestamp: "2026-01-15T10:32:00Z".into(),
            chunk_type: ChunkType::Decision,
        };
        let path = append_chunk(&config, &chunk).unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("# Session Memory — 2026-01-15"));
        assert!(body.contains("## [10:32] Decision"));
        assert!(body.contains("engram:hash:abc123"));
        std::fs::remove_dir_all(&tmp).ok();
    }
}
