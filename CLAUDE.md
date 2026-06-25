# Grasp — Build Overview
> Passive, local, zero-API memory for Claude Code. Single Rust binary.

---

## ⚠️ Implementation note (what was actually built)

This document is the original spec. The shipped project follows it closely with
two deliberate, agreed changes:

1. **It's also a desktop app.** On top of the Rust engine there's a **Tauri 2 +
   React/Vite** UI (search, note viewer, force-directed brain graph). This
   relaxes the "single static binary / Rust-only" rule — the engine stays pure
   Rust (`crates/grasp-core`), reused by the CLI, the app, and the MCP server.
2. **Windows-first, GNU toolchain.** Built/tested on Windows with a portable
   MinGW (`x86_64-pc-windows-gnu`) toolchain so no admin/Visual Studio is needed.
   Paths use the `directories` crate, not the Linux paths below.

Also note vs. this spec: real transcripts live flat at `<slug>/<uuid>.jsonl`
(no `sessions/` subdir), and `tool_use`/`tool_result` are nested inside
`message.content` arrays. Embeddings (Phase 2) are **deferred**: the ONNX-based
`fastembed`/`ort` ships no GNU prebuilt binaries; planned via pure-Rust `candle`.
See `README.md` for current status and build steps.

---

## What This Is

A Rust binary that runs two threads permanently in the background:

1. **Daemon thread** — watches `~/.claude/projects/` for new/updated JSONL session files, parses them, extracts signal, embeds chunks, writes to markdown + SQLite.
2. **MCP server thread** — exposes tools to Claude Code for RAG query and optional manual save.

Zero user action required after install. No API keys. No external processes. One `.db` file, one binary.

---

## Project Name

**grasp** — a neurological term for a memory trace stored in the brain. Fits perfectly.

---

## Constraints (non-negotiable)

- Rust only
- No external API calls ever (no OpenAI, no Anthropic, no OpenRouter)
- No separate server processes (no Qdrant, no Postgres, no Neo4j)
- Single static binary, zero runtime dependencies
- SQLite as sole storage backend (`rusqlite` + `sqlite-vec` + FTS5)
- Bundled embedding model (nomic-embed or similar, ~100-440MB one-time download)
- Markdown as human-readable source of truth
- MCP server built into same binary
- Works on macOS + Linux (Windows stretch goal)

---

## Repository Structure

```
grasp/
├── Cargo.toml
├── README.md
├── install.sh
├── src/
│   ├── main.rs               # entry point, spawns daemon + MCP threads
│   ├── daemon/
│   │   ├── mod.rs            # watcher loop
│   │   ├── watcher.rs        # fs watch on ~/.claude/projects/
│   │   ├── parser.rs         # JSONL transcript parser
│   │   ├── extractor.rs      # signal extraction from parsed entries
│   │   └── writer.rs         # writes markdown + triggers embed+index
│   ├── embed/
│   │   ├── mod.rs
│   │   └── model.rs          # bundled embedding model wrapper
│   ├── store/
│   │   ├── mod.rs
│   │   ├── db.rs             # SQLite init, migrations
│   │   ├── index.rs          # FTS5 + sqlite-vec write/query
│   │   └── markdown.rs       # markdown file read/write
│   ├── mcp/
│   │   ├── mod.rs
│   │   ├── server.rs         # JSON-RPC 2.0 over stdio
│   │   └── tools.rs          # query_memory, save_context tool handlers
│   └── config.rs             # paths, settings, env vars
├── models/
│   └── .gitkeep              # embedding model downloaded here on first run
└── tests/
    ├── parser_tests.rs
    └── rag_tests.rs
```

---

## Core Data Flow

### Write Path (Daemon)

```
~/.claude/projects/<slug>/sessions/<uuid>.jsonl
        │
        ▼ (file watcher detects new/modified)
   parser.rs
   — reads each JSONL line
   — deserializes into typed Entry enum
   — filters: skip tool_result noise, error loops, system prompts
        │
        ▼
   extractor.rs
   — extracts signal:
     · files written/modified (Write tool calls)
     · user decisions (assistant text with decision keywords)
     · errors hit + how resolved (Bash tool results with exit code != 0)
     · project name + working directory
     · timestamps
   — chunks into ~300 token segments
        │
        ▼
   writer.rs
   — appends to markdown file:
     ~/.local/share/grasp/memory/<project-slug>/YYYY-MM-DD.md
   — each chunk gets a SHA-256 hash (dedup key)
        │
        ▼
   embed/model.rs
   — embeds each new chunk (768-dim vectors)
        │
        ▼
   store/index.rs
   — writes to SQLite:
     · chunks table (id, project, hash, text, timestamp, md_path)
     · FTS5 virtual table (full-text search)
     · vec0 table (sqlite-vec, cosine similarity)
```

### Read Path (MCP)

```
Claude Code session starts
        │
        ▼ (SessionStart hook OR explicit tool call)
   mcp/tools.rs → query_memory(context: current project + recent files)
        │
        ▼
   store/index.rs
   — parallel search:
     · BM25 over FTS5 (keyword match)
     · cosine similarity over vec0 (semantic match)
   — RRF fusion (Reciprocal Rank Fusion, k=60)
   — top-K results (default: 5 chunks, ~400 tokens)
        │
        ▼
   returns chunks as MCP tool response
   Claude Code injects into context window
```

---

## JSONL Transcript Format

Claude Code writes entries like:

```json
{"type":"user","uuid":"...","parentUuid":"...","timestamp":"...","sessionId":"...","cwd":"/path","message":{"role":"user","content":"fix the auth bug"}}
{"type":"assistant","uuid":"...","parentUuid":"...","timestamp":"...","sessionId":"...","message":{"role":"assistant","content":[{"type":"text","text":"Let me check..."},{"type":"tool_use","name":"Read","input":{"file_path":"src/auth.ts"}}]}}
{"type":"tool_result","toolUseId":"...","content":"export function validateToken...","timestamp":"..."}
```

Key entry types to handle:
- `user` — user prompt text
- `assistant` — Claude response + tool_use blocks
- `tool_result` — output of tool calls
- `summary` — compaction summaries (high signal, always keep)

Use the `claude_code_transcripts` Rust crate (already exists on crates.io) as the typed parser — don't rewrite this from scratch.

---

## Signal Extraction Rules

Not all transcript content is worth storing. Apply these filters:

**High signal — always extract:**
- `summary` type entries (compaction summaries)
- Assistant text containing: "decided", "because", "instead of", "approach", "architecture", "will use", "won't use"
- Successful `Write`/`Edit` tool calls (file path + what changed)
- `Bash` tool results where exit code = 0 AND output is short (< 200 chars) — likely a meaningful result
- User messages that are questions or contain "why", "how", "should"

**Low signal — skip:**
- `Read` tool results (too noisy, just file contents)
- `Bash` results with long output (build logs, test output dumps)
- Error loops (same error 3+ times in a row)
- System prompt entries
- Tool calls to `Grep`/`Glob` (intermediate search steps)

**Dedup:**
- SHA-256 hash each chunk before inserting
- Skip if hash already exists in DB
- 5-minute window dedup to handle rapid re-saves

---

## SQLite Schema

```sql
-- Main chunks table
CREATE TABLE chunks (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    project     TEXT NOT NULL,
    session_id  TEXT NOT NULL,
    hash        TEXT UNIQUE NOT NULL,
    text        TEXT NOT NULL,
    timestamp   TEXT NOT NULL,
    md_path     TEXT NOT NULL,
    chunk_type  TEXT NOT NULL  -- 'decision', 'file_write', 'error_resolution', 'summary', 'context'
);

-- FTS5 for BM25 keyword search
CREATE VIRTUAL TABLE chunks_fts USING fts5(
    text,
    project UNINDEXED,
    content='chunks',
    content_rowid='id'
);

-- sqlite-vec for vector search (768-dim nomic-embed)
CREATE VIRTUAL TABLE chunks_vec USING vec0(
    chunk_id INTEGER,
    embedding FLOAT[768]
);

-- Project registry
CREATE TABLE projects (
    slug        TEXT PRIMARY KEY,
    path        TEXT NOT NULL,
    last_seen   TEXT NOT NULL,
    chunk_count INTEGER DEFAULT 0
);

-- Processed file tracking (avoid reprocessing)
CREATE TABLE processed_files (
    file_path   TEXT PRIMARY KEY,
    last_hash   TEXT NOT NULL,
    processed_at TEXT NOT NULL
);
```

---

## Markdown Output Format

Each project gets daily markdown files:

```
~/.local/share/grasp/memory/
└── -home-user-myproject/
    ├── 2026-01-15.md
    ├── 2026-01-16.md
    └── index.md          ← project summary, auto-updated
```

Each `.md` file looks like:

```markdown
# Session Memory — 2026-01-15

## [10:32] Decision
Switched from Minikube to GKE for Artemis. Local cluster was too slow
for testing operator reconciliation loops. GKE gives real node behavior.
<!-- grasp:hash:a3f8c2... type:decision session:abc-123 -->

## [10:45] File Write
Modified `operator/reconciler.go` — added exponential backoff to
the deployment reconciliation loop (was causing thundering herd).
<!-- grasp:hash:b9d1e4... type:file_write session:abc-123 -->

## [11:20] Error Resolution
Fixed: `context deadline exceeded` in kubeconfig loader. Root cause was
missing timeout on the REST client config. Added 30s timeout.
<!-- grasp:hash:c2f7a1... type:error_resolution session:abc-123 -->
```

Human-readable, editable, git-committable. The HTML comment carries metadata for the index layer.

---

## Embedding Model

**Recommended:** `nomic-embed-text-v1.5` (quantized GGUF, ~100MB)
- 768 dimensions
- 8192 token context
- Runs fully local via `candle` (Rust ML framework by HuggingFace)
- No Python, no ONNX runtime, no external process

**Alternative if candle is too heavy to start:** `fastembed-rs`
- Pure Rust, wraps ONNX models
- Slightly easier to integrate initially
- Same result

**First-run behavior:**
```
grasp start
→ checks if model exists at ~/.local/share/grasp/models/
→ if not: downloads model (~100MB, one time, shows progress bar)
→ loads model into memory
→ starts daemon + MCP server
```

---

## MCP Server

Implements JSON-RPC 2.0 over stdio (how Claude Code communicates with MCP servers).

### Tools Exposed

**`query_memory`**
```json
{
  "name": "query_memory",
  "description": "Search past session memory for context relevant to current work",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": { "type": "string" },
      "project": { "type": "string" },
      "limit": { "type": "integer", "default": 5 }
    },
    "required": ["query"]
  }
}
```

**`save_context`** (optional manual save)
```json
{
  "name": "save_context",
  "description": "Explicitly save a note to memory",
  "inputSchema": {
    "type": "object",
    "properties": {
      "text": { "type": "string" },
      "project": { "type": "string" },
      "type": { "type": "string", "enum": ["decision", "context", "note"] }
    },
    "required": ["text"]
  }
}
```

**`list_projects`**
```json
{
  "name": "list_projects",
  "description": "List all indexed projects with chunk counts"
}
```

### MCP Config (user adds to `~/.claude/settings.json`)

```json
{
  "mcpServers": {
    "grasp": {
      "command": "/path/to/grasp",
      "args": ["mcp"]
    }
  }
}
```

### Session Start Hook (auto-inject)

Add to `~/.claude/settings.json`:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "",
        "hooks": [{
          "type": "command",
          "command": "grasp inject --project $(basename $PWD)"
        }]
      }
    ]
  }
}
```

The `inject` command queries memory and writes results to a temp file that Claude Code reads as context.

---

## Cargo Dependencies

```toml
[dependencies]
# MCP / JSON-RPC
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }

# JSONL transcript parsing
claude_code_transcripts = "0.1"   # already exists on crates.io

# SQLite
rusqlite = { version = "0.31", features = ["bundled", "vtab", "session"] }
sqlite-vec = "0.1"                # vector search extension

# Embeddings
fastembed = "3"                   # or candle if you want full control

# File watching
notify = "6"

# Hashing
sha2 = "0.10"

# CLI
clap = { version = "4", features = ["derive"] }

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"
```

---

## CLI Interface

```bash
# Start daemon + MCP server (normal usage)
grasp start

# MCP server only (Claude Code calls this)
grasp mcp

# Inject relevant memories to stdout (used by SessionStart hook)
grasp inject --project <name> --query <optional>

# Import existing transcripts retroactively
grasp import --path ~/.claude/projects/

# Show stats
grasp stats

# List projects in memory
grasp projects

# Search memory directly (debugging)
grasp search "auth bug kubeconfig"

# Wipe memory for a project
grasp forget --project <name>

# Wipe all memory
grasp reset
```

---

## Phase Plan

### Phase 1 — Core (build this first)
- [ ] JSONL parser using `claude_code_transcripts` crate
- [ ] Signal extractor (decision/file_write/error_resolution rules)
- [ ] SQLite schema + migrations
- [ ] Markdown writer
- [ ] `grasp import` command (retroactive ingest of existing transcripts)
- [ ] BM25 search via FTS5 (no embeddings yet)
- [ ] `grasp search` CLI command
- Validate: run on your own `~/.claude/projects/`, see if output makes sense

### Phase 2 — Embeddings + RAG
- [ ] Integrate `fastembed-rs`
- [ ] First-run model download with progress bar
- [ ] Embed chunks on write, store in `sqlite-vec`
- [ ] Hybrid search (BM25 + cosine, RRF fusion)
- Validate: compare BM25-only vs hybrid retrieval quality on your own history

### Phase 3 — MCP Server
- [ ] JSON-RPC 2.0 stdio server
- [ ] `query_memory` tool
- [ ] `save_context` tool
- [ ] `list_projects` tool
- [ ] Wire into Claude Code via `~/.claude/settings.json`
- Validate: open Claude Code, ask it what you worked on last week

### Phase 4 — Daemon + File Watcher
- [ ] `notify` watcher on `~/.claude/projects/`
- [ ] Real-time ingest as sessions are written
- [ ] Dedup logic (SHA-256 + 5min window)
- [ ] `grasp start` as persistent background process

### Phase 5 — Polish
- [ ] `install.sh` (download binary, configure Claude Code automatically)
- [ ] `grasp stats` output
- [ ] Config file support (`~/.config/grasp/config.toml`)
- [ ] README + demo

---

## Key Files to Read Before Starting

- `~/.claude/projects/<slug>/sessions/<uuid>.jsonl` — understand the actual format
- [claude_code_transcripts crate docs](https://docs.rs/claude-code-transcripts) — typed parser, use this
- [sqlite-vec docs](https://github.com/asg017/sqlite-vec) — vector search in SQLite
- [fastembed-rs docs](https://docs.rs/fastembed) — local embeddings in Rust
- [notify crate docs](https://docs.rs/notify) — file system watching

---

## What Makes This Different From Existing Tools

| Feature | agentmemory | memsearch | grasp |
|---|---|---|---|
| Language | Node.js | Go/Node | Rust |
| LLM in write path | Optional (off by default) | Yes (haiku) | Never |
| Capture mechanism | PostToolUse hooks | Stop hooks | Raw file watcher |
| Requires agent participation | Yes | Yes | No |
| Markdown source of truth | No | Yes | Yes |
| Single binary | No | No | Yes |
| External processes | No | Milvus | No |
| API keys ever | Optional | Optional | Never |

The core differentiation: **the agent never participates in the write path.** grasp reads files Claude Code already writes to disk regardless. If Claude Code is running, grasp is capturing. No hooks, no decisions, no friction.

---

## Notes for Claude Code Worker Instance

When building this, prioritize in order:
1. Get the parser + extractor working first — validate signal quality on real transcripts
2. SQLite schema before embeddings — BM25 alone is useful and ships faster
3. Don't add embeddings until Phase 1 is proven working
4. The MCP server is simpler than it sounds — it's just JSON over stdin/stdout
5. Use `claude_code_transcripts` crate, don't rewrite the parser
6. Test against your own `~/.claude/projects/` data from day one
