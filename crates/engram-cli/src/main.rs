//! Engram CLI — headless driver for the engine (Stage 1).
//!
//! Subcommands:
//!   engram import [--path DIR]   ingest transcripts (default: ~/.claude/projects)
//!   engram search <QUERY> [--project SLUG] [--limit N]
//!   engram projects              list indexed projects + chunk counts
//!   engram stats                 totals

mod mcp;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use engram_core::Engram;

#[derive(Parser)]
#[command(name = "engram", version, about = "Local, passive memory for Claude Code")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Import transcripts into memory.
    Import {
        /// Directory to import (defaults to ~/.claude/projects).
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Search memory by keyword.
    Search {
        /// Words to search for.
        query: Vec<String>,
        /// Restrict to one project slug.
        #[arg(long)]
        project: Option<String>,
        /// Max results.
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },
    /// List indexed projects.
    Projects,
    /// Show aggregate stats.
    Stats,
    /// Print the memory graph (nodes + edges) as JSON.
    Graph {
        /// Restrict to one project slug.
        #[arg(long)]
        project: Option<String>,
    },
    /// Watch transcripts and ingest changes live (passive capture).
    Watch {
        /// Directory to watch (defaults to ~/.claude/projects).
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Run the MCP server over stdio (for Claude Code).
    Mcp,
    /// Generate embeddings for memories that lack them (enables semantic search).
    Embed,
    /// Forget all memories for one project.
    Forget {
        /// Project slug to forget.
        #[arg(long)]
        project: String,
    },
    /// Wipe all memory (every project).
    Reset {
        /// Confirm the wipe (required).
        #[arg(long)]
        yes: bool,
    },
}

fn main() -> Result<()> {
    // Logs go to stderr: the MCP server uses stdout for the JSON-RPC stream.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let engram = Engram::open()?;

    match cli.command {
        Command::Import { path } => {
            let report = engram.import(path.as_deref())?;
            println!(
                "Imported {} file(s), skipped {} unchanged, added {} new memories.",
                report.files_processed, report.files_skipped, report.chunks_added
            );
        }
        Command::Search {
            query,
            project,
            limit,
        } => {
            let q = query.join(" ");
            if q.trim().is_empty() {
                eprintln!("Provide something to search for, e.g. engram search auth bug");
                std::process::exit(2);
            }
            let hits = engram.search(&q, project.as_deref(), limit)?;
            if hits.is_empty() {
                println!("No matches for {q:?}.");
            }
            for hit in hits {
                let when = hit.timestamp.split('T').next().unwrap_or(&hit.timestamp);
                println!(
                    "\n[{}] {} · {}\n{}",
                    when, hit.chunk_type, hit.project, hit.text
                );
            }
        }
        Command::Projects => {
            let projects = engram.projects()?;
            if projects.is_empty() {
                println!("No projects indexed yet. Run `engram import` first.");
            }
            for p in projects {
                println!("{:<40} {:>6} memories", p.slug, p.chunk_count);
            }
        }
        Command::Stats => {
            let stats = engram.stats()?;
            println!("Projects: {}", stats.total_projects);
            println!("Memories: {}", stats.total_chunks);
            println!("Database: {}", engram.config.db_path().display());
        }
        Command::Graph { project } => {
            let graph = engram.graph(project.as_deref())?;
            println!("{}", serde_json::to_string_pretty(&graph)?);
        }
        Command::Watch { path } => {
            let dir = path.unwrap_or_else(|| engram.config.claude_projects_dir.clone());
            // Capture anything already on disk, then watch for changes.
            let report = engram.import(Some(&dir))?;
            println!(
                "Initial import: {} file(s), +{} memories. Now watching {} … (Ctrl+C to stop)",
                report.files_processed,
                report.chunks_added,
                dir.display()
            );
            let watcher =
                engram_core::watch::watch(&dir, std::time::Duration::from_millis(800))?;
            for changed in watcher.changes {
                match engram.ingest_file(&changed) {
                    Ok(0) => {}
                    Ok(n) => println!("+{n} memories from {}", changed.display()),
                    Err(e) => eprintln!("ingest error for {}: {e}", changed.display()),
                }
            }
        }
        Command::Mcp => {
            mcp::run(engram)?;
        }
        Command::Embed => {
            println!("Generating embeddings (first run downloads ~90MB model)…");
            let n = engram.embed_backfill()?;
            println!("Embedded {n} memories. Search is now hybrid (keyword + semantic).");
        }
        Command::Forget { project } => {
            let removed = engram.forget(&project)?;
            println!("Forgot {removed} memories from {project}.");
        }
        Command::Reset { yes } => {
            if !yes {
                eprintln!("This wipes ALL memory. Re-run with --yes to confirm.");
                std::process::exit(2);
            }
            engram.reset()?;
            println!("All memory wiped.");
        }
    }
    Ok(())
}
