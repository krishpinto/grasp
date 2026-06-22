# Engram

[![CI](https://github.com/krishpinto/engram/actions/workflows/ci.yml/badge.svg)](https://github.com/krishpinto/engram/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

> **Passive, local, zero-API memory for AI coding agents — and a 3D app to *see* it.**

Claude Code already writes a transcript of every session to disk. **Engram reads
those transcripts, keeps only the meaningful moments** — decisions, file changes,
error fixes, summaries — stores them as a searchable database *and* human-readable
Markdown, embeds them locally for semantic search, and gives you a desktop app to
fly through your project's history as a 3D **memory graph**.

The agent never participates in the write path. No hooks, no API keys, no cloud,
no separate database server. If the agent ran, Engram captured it.

> *"I never tell it to remember anything, and I can open a map of every decision
> my project ever made."*

---

## Why it's different

The "AI memory" space is crowded, so Engram is deliberate about its niche:

| | Engram | Typical memory tools |
|---|---|---|
| **Capture** | **Passive** — watches transcript files | Agent must call a tool / you prompt "remember this" |
| **What it stores** | Your **decisions & conversation history** | Often code structure, or verbatim everything |
| **Surface** | **3D decision graph + docs, in a desktop app** | No UI, or a raw database browser |
| **Embeddings** | **On-device** (candle) — no API, ever | Cloud embedding APIs |
| **Footprint** | One Rust engine, SQLite, Markdown | Python + vector DB + cloud, usually |

The wedge: **zero-friction passive capture + a visual decision graph**, entirely local.

---

## How it works

```
  Claude Code ──auto-writes──▶ transcripts (~/.claude/projects/*.jsonl)
                                   │
                                   ▼
   ENGINE (Rust, engram-core)
     parse → extract signal → redact secrets → store BOTH
          SQLite + FTS5 (fast search)   and   Markdown (human source of truth)
       → embed locally (candle)  → hybrid search (BM25 + cosine, RRF-fused)
                                   │
              ┌────────────────────┼────────────────────┐
              ▼                     ▼                     ▼
          engram CLI          MCP server            Tauri + React app
        (import/search)   (agent recalls memory)   (search · 3D graph · docs)
```

1. **Capture** — a `notify` watcher reads transcripts the moment they're written. Passive; the agent does nothing.
2. **Extract** — tolerant JSONL parser → keeps decisions (with rationale), file writes (what & why), error fixes, summaries, questions; drops plumbing (compaction summaries, IDE events, search noise).
3. **Redact** — secrets (private keys, JWTs, `sk-…`/`ghp_…`/`AKIA…`, bearer tokens, `KEY=value`) are scrubbed *before* storage.
4. **Store** — SQLite (`chunks` + FTS5) + per-day Markdown, SHA-256 deduped.
5. **Embed** — candle runs `all-MiniLM-L6-v2` (384-dim) on-device; ~90 MB model, downloaded once.
6. **Search** — BM25 keyword **and** cosine semantic, fused with Reciprocal Rank Fusion (k=60); falls back to BM25 if no vectors yet.

---

## The app

> Runs from source today (`pnpm tauri dev` — see [Quick start](#quick-start-windows));
> a packaged installer is on the roadmap. The one-line install above sets up the
> engine + Claude Code memory without it.

- **Loading splash** → **Overview**: the whole-system 3D "brain" of every project.
- **Guided spotlight** the first time, pointing you to your memories.
- **Archive**: a clean card per project.
- **Project view**: that project's **3D graph** (orbit / zoom / pan — "spectator mode") + a scoped search.
- **In-app docs**: an illustrated "How it works", with an animated pipeline.

Graph nodes are memories (colored by type) and the **files** they touched; edges are
**session** order, **shared file**, and **semantic similarity** — so related decisions
connect across days, not just within a session.

<!-- TODO: add docs/graph.gif — a 15s screen recording of flying through the graph -->

---

## Features

- ✅ Live **passive capture** (`engram watch` and the in-app watcher)
- ✅ **Hybrid search** — BM25 (FTS5) + on-device semantic embeddings, RRF-fused
- ✅ **3D memory graph** — orbitable, with file nodes + semantic edges
- ✅ **Secret redaction** before anything is stored
- ✅ **MCP server** — `query_memory` (auto-scoped to the current project) / `save_context` / `list_projects`
- ✅ **Markdown source of truth** — readable, git-committable, survives the DB
- ✅ **Retrieval eval harness** (`engram eval`) — BM25 vs hybrid hit-rate

---

## Quick start (Windows)

**Install the engine in one line** — downloads a prebuilt build with the
embedding model **bundled in** (no Rust, no build, no separate download),
registers it with Claude Code, and imports your history:

```powershell
irm https://github.com/krishpinto/engram/releases/latest/download/install.ps1 | iex
```

Then open a Claude Code session and ask about your past work — it'll use Engram
automatically. That's it.

> **What the installer gives you:** the memory **engine** — passive capture,
> hybrid search, and the **MCP server** so Claude Code can recall your history.
> The **desktop app** (the 3D memory graph) isn't packaged yet — for now it runs
> from source (`pnpm tauri dev`, see below). A packaged app installer is on the
> roadmap.

<details><summary>Manual steps</summary>

Engram needs **Rust** and a **C compiler** (for the bundled SQLite). On Windows
without Visual Studio, a portable GNU/MinGW toolchain works with no admin:

```powershell
# 1. Rust — GNU toolchain, no MSVC needed (https://rustup.rs)
rustup default stable-x86_64-pc-windows-gnu
# 2. A portable MinGW gcc on PATH (e.g. WinLibs UCRT), so bundled SQLite compiles.
# 3. Node + pnpm for the UI
pnpm install
```

Run it:

```powershell
pnpm tauri dev                          # desktop app (Vite + Tauri, hot reload)
# or drive the engine headless:
cargo run -p engram-cli -- import       # ingest ~/.claude/projects
cargo run -p engram-cli -- embed        # generate vectors (semantic search)
cargo run -p engram-cli -- search candle toolchain
```

Register it with Claude Code (auto-writes the MCP config):

```powershell
cargo run -p engram-cli -- setup
```

> macOS/Linux ship a C compiler by default, so only Rust + Node/pnpm are needed.

</details>

---

## CLI

```
engram import [--path DIR]     ingest transcripts (default ~/.claude/projects)
engram embed                   generate embeddings (enables hybrid search)
engram search <query…>         hybrid (or keyword) search
engram watch [--path DIR]      ingest live as sessions are written
engram eval                    run the retrieval eval set
engram graph [--project SLUG]  print the memory graph as JSON
engram projects | stats        registry / totals
engram setup                   auto-register the MCP server with Claude Code
engram mcp                     run the MCP server over stdio
engram redact                  re-scrub stored memories with current secret patterns
engram forget --project SLUG | engram reset --yes
```

## Use with Claude Code (MCP)

```powershell
engram setup     # auto-registers the MCP server with Claude Code
```

(or manually: `claude mcp add engram -s user -- "C:\path\to\engram.exe" mcp`.)
Then, inside a session, just ask — *"what did we decide about X?"* — and Claude
calls `query_memory` automatically.

---

## Data & privacy

Engram captures real transcripts, which can contain secrets — so a **redaction
pass runs before anything is stored**: private keys, JWTs, provider API keys
(`sk-…`, `ghp_…`, `AKIA…`), bearer tokens, and `KEY=value` assignments are
replaced with `[REDACTED]` labels. Everything stays on your machine — the SQLite
database and Markdown files live in your local data directory, and the embedding
model runs on-device. Captured something before redaction improved? Run
`engram redact` to re-scrub the existing database and Markdown in place.

## Platform support

Built and tested on **Windows** (GNU/MinGW toolchain). The engine is written to
be cross-platform — paths are resolved via the `directories` crate, not hardcoded
— so **macOS/Linux should work** with only Rust + Node/pnpm, but they aren't
verified yet (CI builds the engine on Linux). The one-command installer is
Windows-only for now; a `install.sh` and prebuilt release binaries are on the
roadmap.

## Tech stack

Rust · Tauri 2 · React + Vite + TypeScript · SQLite (`rusqlite` + FTS5) ·
`candle` (local embeddings) · `notify` · `react-force-graph-3d` + three.js.

```
crates/engram-core   the engine: parser · extractor · redactor · store · embeddings · search · graph
crates/engram-cli    command-line driver + MCP server
src-tauri            Tauri desktop shell + live watcher
src                  React UI (overview · archive · project graph · docs · note viewer)
```

## Roadmap

- **More agents** — capture is Claude Code-first; Codex & others planned (the MCP read side already works with any MCP client).
- **One-click installer** — so "try it" doesn't mean "compile it".
- **Auto-recall** — a SessionStart hook that injects memory before you ask.
- **Sharper retrieval** — honest eval set + tuned extraction.

## License

MIT — see [LICENSE](LICENSE).
