interface Props {
  onBack: () => void;
}

const SECTIONS = [
  { id: "what", label: "What it is" },
  { id: "principles", label: "Principles" },
  { id: "start", label: "Quick start" },
  { id: "mcp", label: "MCP toolkit" },
  { id: "how", label: "How it works" },
  { id: "retrieval", label: "Retrieval (RAG)" },
  { id: "hood", label: "Under the hood" },
  { id: "privacy", label: "Data & privacy" },
  { id: "roadmap", label: "Roadmap" },
];

const PIPELINE = [
  {
    k: "Capture",
    d: "Grasp reads the JSONL transcripts Claude Code already writes to ~/.claude/projects — no plugin, no hook, no agent cooperation; the agent is never in the write path. Capture happens three ways: `grasp import` catches up on all existing history, while `grasp watch` and the desktop app ingest new sessions live (a debounced file watcher, so a burst of writes to one file collapses into a single pass). (Codex & other agents: planned.)",
  },
  {
    k: "Extract",
    d: "Each transcript is parsed into typed entries, then filtered to signal only. Decisions keep just the decision sentence and its rationale — triggered by words like 'decided', 'because', 'instead of' — not the whole rambling message; file writes keep the path plus the 'why'; an error is paired with the first reply that fixed it; summaries and substantive questions are kept. Plumbing — compaction summaries, IDE events, search steps, repeated error loops, and huge log dumps — is dropped, and every memory is SHA-256 deduped.",
  },
  {
    k: "Redact",
    d: "Before anything is stored, a scrubber strips secrets: private keys, JWTs, provider API keys (sk-…, ghp_…, AKIA…), bearer tokens, and KEY=value assignments, replaced with [REDACTED] labels.",
  },
  {
    k: "Store",
    d: "Each memory is written twice — human-readable Markdown (the source of truth, one file per project per day) and SQLite (for fast machine search) — deduplicated by a SHA-256 hash of its content.",
  },
  {
    k: "Embed",
    d: "A local candle model (all-MiniLM-L6-v2, 384-dim) turns each memory into a vector: tokenize → BERT → mean-pool → normalize. Fully on-device; the ~90MB model downloads once.",
  },
  {
    k: "Search",
    d: "A query runs two ways at once — BM25 keyword match (SQLite FTS5) and cosine similarity over the vectors — and the two rankings are fused with Reciprocal Rank Fusion (k=60).",
  },
  {
    k: "Return",
    d: "The top memories are handed back to Claude through the MCP tools (or to this app), scoped to the current project so one repo's history never leaks into another's.",
  },
  {
    k: "Display",
    d: "In the app, results appear as searchable cards and as the 3D memory graph you can fly through — nodes are memories, edges are sessions, shared files, and semantic similarity.",
  },
];

export function DocsView({ onBack }: Props) {
  function jump(id: string) {
    document
      .getElementById(`doc-${id}`)
      ?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  return (
    <div className="screen">
      <div className="topbar">
        <button className="back-btn" onClick={onBack}>
          ← Overview
        </button>
        <div className="topbar-title">How Grasp works</div>
      </div>

      <div className="docs-body">
        <nav className="docs-nav">
          {SECTIONS.map((s) => (
            <button key={s.id} onClick={() => jump(s.id)}>
              {s.label}
            </button>
          ))}
        </nav>

        <div className="docs-content">
          {/* What it is */}
          <section id="doc-what" className="docs-section">
            <h1>Grasp</h1>
            <p className="docs-lead">
              Passive, local, zero-API memory for AI coding agents. Grasp reads the
              sessions your agent already writes, keeps the meaningful moments, and
              lets you search them — or lets the agent recall them itself.
            </p>
            <p>
              When you work with an AI coding agent, every decision, fix, and dead end
              lives in a transcript that's forgotten the moment the session ends. Start
              a new session next week and the agent has no idea what you already decided
              or why. Grasp fixes that: it turns those transcripts into a durable,
              searchable memory that the agent can pull from automatically.
            </p>
            <p>
              Two things make it different from a notes plugin or a vector-DB wrapper:
              capture is <b>completely passive</b> (the agent never has to choose to
              remember — Grasp reads the files on disk regardless), and the result is a
              <b> visual map of every decision your project ever made</b>, not just a
              search box.
            </p>
          </section>

          {/* Principles */}
          <section id="doc-principles" className="docs-section">
            <h2>Design principles</h2>
            <ul className="docs-list">
              <li>
                <b>Local-only.</b> Everything runs on your machine. No servers, no
                accounts.
              </li>
              <li>
                <b>No API calls, ever.</b> Embeddings run on-device with candle — no
                OpenAI, no Anthropic API, no keys.
              </li>
              <li>
                <b>Passive capture.</b> The agent isn't in the write path. Grasp reads
                transcripts that already exist.
              </li>
              <li>
                <b>Markdown is the source of truth.</b> SQLite is a fast index you can
                rebuild; your memories live as plain, git-friendly text.
              </li>
              <li>
                <b>One engine, many front doors.</b> The same Rust core powers the CLI,
                the MCP server, and this desktop app.
              </li>
            </ul>
          </section>

          {/* Quick start */}
          <section id="doc-start" className="docs-section">
            <h2>Quick start</h2>
            <h3>Install the engine (one line, Windows)</h3>
            <p>
              The fastest path. Downloads a prebuilt build with the embedding model
              bundled in (no Rust, no compile, no separate download), registers it with
              Claude Code, and imports your history:
            </p>
            <pre className="code-block">irm https://github.com/krishpinto/grasp/releases/latest/download/install.ps1 | iex</pre>
            <p>
              This gives you the <b>engine + MCP server</b> — passive capture and recall
              inside Claude Code. The desktop app (this 3D graph) runs from source for
              now; a packaged app installer is on the roadmap.
            </p>
            <h3>Run the app from source</h3>
            <p>
              Prerequisites: Rust (via rustup) and Node + pnpm. On Windows, Grasp builds
              with the GNU toolchain so no Visual Studio is required. From the project
              root:</p>
            <pre className="code-block">pnpm tauri dev</pre>
            <h3>Build the CLI &amp; import your history</h3>
            <p>
              The CLI binary is also what the MCP server runs. Build it once, then
              ingest existing transcripts (or just click <b>Import</b> in the app):
            </p>
            <pre className="code-block">cargo build --release{"\n"}grasp import{"\n"}grasp embed   # generate vectors for semantic search</pre>
            <h3>Connect it to Claude Code</h3>
            <p>Register the MCP server (one time):</p>
            <pre className="code-block">claude mcp add grasp -- "C:\path\to\grasp.exe" mcp</pre>
            <p>
              Add <code>-s user</code> to make it available in every project. Verify it
              connected inside a session with <code>/mcp</code>, then just ask
              naturally — "what did we decide about X?" — and Claude will call it.
            </p>
          </section>

          {/* MCP toolkit */}
          <section id="doc-mcp" className="docs-section">
            <h2>MCP toolkit</h2>
            <p>
              Grasp exposes a <b>Model Context Protocol</b> server (JSON-RPC 2.0 over
              stdio). Claude Code launches it as a child process and talks to it over a
              pipe — no network. Three tools are exposed:
            </p>
            <div className="tool-grid">
              <div className="tool">
                <div className="tool-name">query_memory</div>
                <div className="tool-desc">
                  Recall past decisions/fixes relevant to the current task. Defaults to
                  the current project (derived from the working directory); accepts an
                  optional query, project, and limit.
                </div>
              </div>
              <div className="tool">
                <div className="tool-name">save_context</div>
                <div className="tool-desc">
                  Manually save a note to long-term memory, tagged as a decision,
                  context, or note.
                </div>
              </div>
              <div className="tool">
                <div className="tool-name">list_projects</div>
                <div className="tool-desc">
                  List every indexed project with its memory count.
                </div>
              </div>
            </div>
            <p className="callout">
              The handshake: Claude sends <code>initialize</code> → Grasp replies with
              its tools → Claude calls <code>tools/call</code> when it needs memory →
              Grasp searches and returns text → Claude folds it into context. Logs go
              to stderr so they never corrupt the stdout JSON stream.
            </p>
          </section>

          {/* How it works */}
          <section id="doc-how" className="docs-section">
            <h2>How it works</h2>
            <p>
              A memory flows through the pipeline below, from the moment a session is
              written to the moment the agent recalls it:
            </p>
            <div className="pipeline">
              {PIPELINE.map((p, i) => (
                <div className="stage" key={p.k}>
                  <div className="stage-spine">
                    <span
                      className="stage-dot"
                      style={{ animationDelay: `${i * 0.45}s` }}
                    />
                    {i < PIPELINE.length - 1 && <span className="stage-line" />}
                  </div>
                  <div className="stage-body">
                    <div className="stage-name">{p.k}</div>
                    <div className="stage-desc">{p.d}</div>
                  </div>
                </div>
              ))}
            </div>
          </section>

          {/* Retrieval deep dive */}
          <section id="doc-retrieval" className="docs-section">
            <h2>Retrieval (RAG)</h2>
            <p>
              "Good search" is the whole game, so Grasp doesn't rely on one method. It
              combines two complementary ones:
            </p>
            <h3>Keyword — BM25</h3>
            <p>
              An FTS5 full-text index ranks memories by exact term overlap. It's precise
              when you remember the words, useless when you don't ("what did we choose"
              never says "candle").
            </p>
            <h3>Semantic — cosine similarity</h3>
            <p>
              Each memory and the query are embedded into 384-dim vectors; a cosine
              comparison finds memories that <i>mean</i> the same thing even with no
              shared words. At Grasp's scale a direct in-memory scan is plenty fast and
              avoids a native vector extension.
            </p>
            <h3>Fusion — Reciprocal Rank Fusion</h3>
            <p>
              The two ranked lists are merged with RRF (k=60): each memory scores{" "}
              <code>1 / (k + rank)</code> from each list, summed. A memory that ranks
              well in <i>either</i> method surfaces — keyword precision and semantic
              recall, without one drowning the other. If no embeddings exist yet, search
              falls back to BM25 alone.
            </p>
          </section>

          {/* Under the hood */}
          <section id="doc-hood" className="docs-section">
            <h2>Under the hood</h2>
            <h3>Crates</h3>
            <ul className="docs-list">
              <li>
                <b>grasp-core</b> (Rust) — the engine: transcript parser, signal
                extractor + redactor, SQLite/FTS5 store, Markdown writer, candle
                embeddings, hybrid search, and the graph builder.
              </li>
              <li>
                <b>grasp-cli</b> (Rust) — the terminal driver (<code>import</code>,{" "}
                <code>search</code>, <code>embed</code>, <code>watch</code>,{" "}
                <code>eval</code>) and the MCP server (<code>grasp mcp</code>).
              </li>
              <li>
                <b>src-tauri + React</b> — the desktop shell, the live file watcher, and
                this UI (search, note viewer, 3D graph, these docs).
              </li>
            </ul>
            <h3>Storage schema</h3>
            <ul className="docs-list">
              <li>
                <code>chunks</code> — the memories (project, text, timestamp, type, hash).
              </li>
              <li>
                <code>chunks_fts</code> — FTS5 mirror kept in sync by triggers (keyword
                search).
              </li>
              <li>
                <code>embeddings</code> — one 384-dim vector per chunk (semantic search).
              </li>
              <li>
                <code>projects</code> / <code>processed_files</code> — registry and
                re-import tracking. WAL + a busy-timeout let the watcher and MCP server
                write concurrently.
              </li>
            </ul>
          </section>

          {/* Privacy */}
          <section id="doc-privacy" className="docs-section">
            <h2>Data &amp; privacy</h2>
            <p>
              Grasp captures real transcripts, which can contain secrets — so a
              redaction pass runs before <i>anything</i> is stored, replacing private
              keys, JWTs, provider API keys, bearer tokens, and KEY=value assignments
              with labelled placeholders.
            </p>
            <p>
              Everything stays on your machine: the SQLite database and Markdown files
              live in your local data directory, the embedding model runs on-device, and
              no request ever leaves the computer. Memory is per-machine and per-person
              — your history is yours.
            </p>
          </section>

          {/* Roadmap */}
          <section id="doc-roadmap" className="docs-section">
            <h2>Roadmap</h2>
            <ul className="docs-list">
              <li>
                <b>More agents.</b> Capture is Claude Code-first; Codex and other agents
                are planned (the MCP read side already works with any MCP client).
              </li>
              <li>
                <b>One-click install.</b> A packaged installer so "try it" doesn't mean
                "compile it."
              </li>
              <li>
                <b>Auto-recall.</b> A SessionStart hook that injects relevant memory
                before you even ask.
              </li>
              <li>
                <b>Sharper retrieval.</b> An honest eval set and tuned extraction so
                recall keeps improving.
              </li>
            </ul>
            <p className="docs-foot">One Rust engine, three front doors — all local.</p>
          </section>
        </div>
      </div>
    </div>
  );
}
