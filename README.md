# Engram

> Passive, local, zero-API memory for Claude Code.

Claude Code already writes a transcript of every session to disk. **Engram reads
those transcripts, keeps only the meaningful moments** — decisions, file changes,
error fixes, summaries — and stores them as a searchable database plus
human-readable markdown. A visual desktop app (coming in Stage 2+) lets you
explore your project's decision history as a graph.

The agent never participates in the write path: if Claude Code is running,
Engram is capturing. No hooks, no API keys, no external services.

## Status

Stage 1 (engine) — in progress:

- [x] Tolerant JSONL transcript parser (handles the real, messy entry types)
- [x] Signal extractor (decisions / file writes / error resolutions / summaries / context)
- [x] SQLite schema + FTS5 keyword search
- [x] Markdown writer (source of truth)
- [x] `engram import` / `search` / `projects` / `stats`
- [ ] Tauri + React app · embeddings · MCP server · live watcher (later stages)

## Build (Windows, no admin)

Requires Rust and a C compiler (for the bundled SQLite). On Windows we use a
portable MinGW toolchain — see the build notes; no Visual Studio needed.

```sh
cargo run -p engram-cli -- import
cargo run -p engram-cli -- search auth bug
cargo run -p engram-cli -- projects
```

## Layout

```
crates/engram-core   # the engine (parser, extractor, store) — reused everywhere
crates/engram-cli    # command-line driver
```
