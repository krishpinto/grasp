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

- [x] Tolerant JSONL transcript parser (handles the real, messy entry types)
- [x] Signal extractor (decisions / file writes / error resolutions / summaries / context)
- [x] SQLite schema + FTS5 keyword search · markdown writer (source of truth)
- [x] CLI: `import` / `search` / `projects` / `stats` / `graph` / `watch` / `mcp`
- [x] Tauri + React desktop app: search, note viewer, force-directed brain graph
- [x] Live file watcher — passive capture, in both the app and `engram watch`
- [x] MCP server (`engram mcp`) — `query_memory` / `save_context` / `list_projects`
- [ ] Embeddings + hybrid semantic search (next) · installer + demo

## Use it with Claude Code (MCP)

Add Engram as an MCP server so Claude Code can query your memory. In your
`~/.claude/settings.json` (or a project `.mcp.json`):

```json
{
  "mcpServers": {
    "engram": {
      "command": "C:\\path\\to\\engram.exe",
      "args": ["mcp"]
    }
  }
}
```

Tools exposed: `query_memory(query, project?, limit?)`,
`save_context(text, project?, type?)`, `list_projects()`.

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
