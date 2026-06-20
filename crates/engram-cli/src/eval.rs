//! Retrieval evaluation (issue #4).
//!
//! Runs a fixed set of questions against memory and reports whether the
//! expected answer appears in the top-K results — for BM25-only vs. the hybrid
//! (BM25 + semantic) ranker. Gives a repeatable, honest quality number instead
//! of one-off anecdotes.

use std::path::Path;

use anyhow::{Context, Result};
use engram_core::{store, Engram};
use serde::Deserialize;

/// One evaluation case: ask `query` (optionally scoped to `project`) and expect
/// some result in the top-K to contain `expect` (case-insensitive substring).
#[derive(Deserialize)]
struct Case {
    query: String,
    #[serde(default)]
    project: Option<String>,
    expect: String,
}

pub fn run(engram: &Engram, path: &Path, k: usize) -> Result<()> {
    let data = std::fs::read_to_string(path)
        .with_context(|| format!("reading eval cases from {}", path.display()))?;
    let cases: Vec<Case> = serde_json::from_str(&data).context("parsing eval cases JSON")?;
    if cases.is_empty() {
        println!("No eval cases in {}.", path.display());
        return Ok(());
    }

    let mut bm25_hits = 0usize;
    let mut hybrid_hits = 0usize;

    println!("{:<4} {:<6} {:<7} {}", "bm25", "hybrid", "", "query");
    println!("{}", "-".repeat(60));

    for c in &cases {
        let proj = c.project.as_deref();
        let needle = c.expect.to_lowercase();

        let bm = store::index::search(&engram.conn, &c.query, proj, k)?;
        let hy = engram.search(&c.query, proj, k)?;

        let bm_hit = bm.iter().any(|h| h.text.to_lowercase().contains(&needle));
        let hy_hit = hy.iter().any(|h| h.text.to_lowercase().contains(&needle));
        bm25_hits += bm_hit as usize;
        hybrid_hits += hy_hit as usize;

        println!(
            "{:<4} {:<6} {:<7} {}",
            if bm_hit { "✓" } else { "·" },
            if hy_hit { "✓" } else { "·" },
            "",
            c.query
        );
    }

    let n = cases.len();
    println!("{}", "-".repeat(60));
    println!(
        "BM25-only : {}/{} ({:.0}%)",
        bm25_hits,
        n,
        100.0 * bm25_hits as f64 / n as f64
    );
    println!(
        "Hybrid    : {}/{} ({:.0}%)  (top-{})",
        hybrid_hits,
        n,
        100.0 * hybrid_hits as f64 / n as f64,
        k
    );
    Ok(())
}
