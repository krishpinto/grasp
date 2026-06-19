# 🧠 Engram

> **Passive, local, zero-API memory for Claude Code — and a desktop app to *see* it.**

Claude Code already writes a transcript of every session to disk. **Engram reads
those transcripts, keeps only the meaningful moments** — decisions, file changes,
error fixes, summaries — stores them as a searchable database *and* human-readable
markdown, and gives you a desktop app to explore your project's history as a
force-directed **memory graph**.

The agent never participates in the write path. No hooks, no API keys, no cloud,
no separate database server. If Claude Code is running, Engram is capturing.

> *"I never tell it to remember anything, and I can open a map of every decision
> my project ever made."*

---

## Why it's different

The "AI memory" space is crowded, so Engram is deliberate about its niche:

| | Engram | Typical memory tools |
|---|---|---|
| **Capture** | **Passive** — watches transcript files | Agent must call MCP tools / you prompt "remember this" |
| **What it stores** | Your **decisions & conversation history** | Often code structure, or verbatim everything |
| **Surface** | **Purpose-built brain-graph desktop app** | No UI, or a raw database browser |
| **Footprint** | Single Rust engine, SQLite, markdown | Python + vector DB + cloud, usually |

Local-only and semantic search are table stakes now; the wedge here is
**zero-friction passive capture + a visual decision graph**.

---

## How it works

```
  Claude Code ──auto-writes──▶ raw transcripts (~/.claude/projects/*.jsonl)
                                   │
                                   ▼
   ENGINE (Rust): parse → extract signal → store BOTH
        SQLite + FTS5 (fast search)   and   markdown (human source of truth)
                                   │  Tauri (IPC)
                                   ▼
   APP (React): search • note viewer • force-directed brain graph
```

- **Parser** — tolerant JSONL reader that ignores the many noisy entry types real
  transcripts contain instead of crashing on them.
- **Extractor** — keeps decisions / file writes / error fixes / summaries /
  questions; SHA-256 dedup.
- **Store** — SQLite (`chunks` + FTS5 keyword index) + per-day markdown files.
- **Watcher** — `notify`-based; ingests new transcripts live.
- **MCP server** — lets Claude Code query its own memory.

---

## Features

- ✅ Live **passive capture** of new sessions (watcher in the app and `engram watch`)
- ✅ **Keyword search** (BM25 over SQLite FTS5)
- ✅ **Brain graph** — memories as nodes, colored by type, chained by session
- ✅ **Note viewer** for full memory text
- ✅ **MCP server** — `query_memory` / `save_context` / `list_projects`
- ✅ **Markdown source of truth** — readable, git-committable, survives the DB
- ⏳ Embeddings / hybrid semantic search — *deferred* (see Roadmap)

---

## Build from source (Windows)

Engram needs **Rust** and a **C compiler** (for the bundled SQLite). On Windows
without Visual Studio, a portable GNU/MinGW toolchain works with no admin:

```powershell
# 1. Rust (https://rustup.rs)
rustup default stable-x86_64-pc-windows-gnu     # GNU toolchain, no MSVC needed

# 2. A portable MinGW gcc on PATH (e.g. WinLibs UCRT build), so the bundled
#    SQLite C code compiles. Put its \mingw64\bin on your PATH.

# 3. Node + pnpm for the UI
pnpm install
```

Then:

```powershell
# Run the desktop app (Vite + Tauri, hot reload)
pnpm tauri dev

# Or just the engine via the CLI
cargo run -p engram-cli -- import
cargo run -p engram-cli -- search email send
```

**Release build** — a standalone, optimized desktop binary that embeds the UI
(no dev server needed):

```powershell
pnpm build                                   # build the React UI into dist/
cargo build -p engram-app --release          # -> target/release/engram-app.exe (~15 MB)
```

> On macOS/Linux the default toolchain already includes a C compiler, so only
> Rust + Node/pnpm are needed.

---

## CLI

```
engram import [--path DIR]     ingest transcripts (default ~/.claude/projects)
engram search <query…>         keyword search
engram watch [--path DIR]      ingest live as sessions are written
engram graph [--project SLUG]  print the memory graph as JSON
engram projects                list indexed projects + counts
engram stats                   totals
engram mcp                     run the MCP server over stdio
engram forget --project SLUG   forget one project
engram reset --yes             wipe all memory
```

## Use with Claude Code (MCP)

Add to `~/.claude/settings.json` (or a project `.mcp.json`):

```json
{
  "mcpServers": {
    "engram": { "command": "C:\\path\\to\\engram.exe", "args": ["mcp"] }
  }
}
```

---

## Tech stack

Rust · Tauri 2 · React + Vite + TypeScript · SQLite (rusqlite + FTS5) ·
`notify` · `react-force-graph`. Workspace layout:

```
crates/engram-core   the engine (parser, extractor, store, watcher) — reused everywhere
crates/engram-cli    command-line driver + MCP server
src-tauri            Tauri desktop shell (commands over the engine)
src                  React UI (search, note viewer, brain graph)
```

## Roadmap

- Embeddings + hybrid (BM25 + vector) semantic search, and *meaning-based* graph
  edges. Deferred because the common Rust embedding lib (ONNX-based) ships no
  prebuilt binaries for the GNU toolchain; planned via pure-Rust `candle`.
- Packaged installers, demo GIF.
