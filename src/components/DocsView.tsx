interface Props {
  onBack: () => void;
}

const SECTIONS = [
  { id: "what", label: "What it is" },
  { id: "start", label: "Quick start" },
  { id: "mcp", label: "MCP toolkit" },
  { id: "how", label: "How it works" },
  { id: "hood", label: "Under the hood" },
];

const PIPELINE = [
  { k: "Capture", d: "Reads the transcripts Claude Code already writes to disk. No plugin, no agent action. (Codex & others: planned.)" },
  { k: "Extract", d: "Keeps only signal — decisions, file changes, error fixes, summaries — and redacts secrets first." },
  { k: "Store", d: "Each memory written twice: human-readable Markdown (source of truth) + SQLite, deduped by SHA-256." },
  { k: "Embed", d: "A local candle model (all-MiniLM-L6-v2, 384-dim) turns each memory into a vector — fully on-device." },
  { k: "Search", d: "Keyword (BM25 / FTS5) and semantic (cosine) run together, fused with Reciprocal Rank Fusion." },
  { k: "Return", d: "Top memories go back to Claude through the MCP tools, or to this app." },
  { k: "Display", d: "Shown as search results and the 3D memory graph you can fly through." },
];

export function DocsView({ onBack }: Props) {
  function jump(id: string) {
    document.getElementById(`doc-${id}`)?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  return (
    <div className="screen">
      <div className="topbar">
        <button className="back-btn" onClick={onBack}>
          ← Overview
        </button>
        <div className="topbar-title">How Engram works</div>
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
          <section id="doc-what" className="docs-section">
            <h1>Engram</h1>
            <p className="docs-lead">
              Passive, local, zero-API memory for AI coding agents. Engram reads the
              sessions your agent already writes, keeps the meaningful moments, and
              lets you search them — or lets the agent recall them itself.
            </p>
            <p>
              Nothing leaves your machine. No API keys, ever. The agent never has to
              "decide" to remember — Engram captures from the transcripts on disk.
            </p>
          </section>

          <section id="doc-start" className="docs-section">
            <h2>Quick start</h2>
            <p>Run the desktop app from the project root:</p>
            <pre className="code-block">pnpm tauri dev</pre>
            <p>Build the CLI (used by the MCP server), then import your history:</p>
            <pre className="code-block">cargo build --release{"\n"}engram import</pre>
            <p>Or just click <b>Import</b> in the app.</p>
          </section>

          <section id="doc-mcp" className="docs-section">
            <h2>MCP toolkit</h2>
            <p>
              Engram exposes a <b>Model Context Protocol</b> server so Claude Code can
              query its own memory. Register it once:
            </p>
            <pre className="code-block">
              claude mcp add engram -- "C:\path\to\engram.exe" mcp
            </pre>
            <div className="tool-grid">
              <div className="tool">
                <div className="tool-name">query_memory</div>
                <div className="tool-desc">Recall past decisions/fixes relevant to the current task.</div>
              </div>
              <div className="tool">
                <div className="tool-name">save_context</div>
                <div className="tool-desc">Manually save a note to long-term memory.</div>
              </div>
              <div className="tool">
                <div className="tool-name">list_projects</div>
                <div className="tool-desc">List every indexed project and its memory count.</div>
              </div>
            </div>
          </section>

          <section id="doc-how" className="docs-section">
            <h2>How it works</h2>
            <p>A memory flows through the pipeline below — capture to recall:</p>
            <div className="pipeline">
              {PIPELINE.map((p, i) => (
                <div className="stage" key={p.k}>
                  <div className="stage-spine">
                    <span className="stage-dot" style={{ animationDelay: `${i * 0.5}s` }} />
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

          <section id="doc-hood" className="docs-section">
            <h2>Under the hood</h2>
            <ul className="docs-list">
              <li>
                <b>engram-core</b> (Rust) — the engine: transcript parser, signal
                extractor, SQLite + FTS5 store, Markdown writer, candle embeddings,
                hybrid search.
              </li>
              <li>
                <b>engram-cli</b> (Rust) — the terminal driver and the MCP server
                (<code>engram mcp</code>) that Claude talks to.
              </li>
              <li>
                <b>src-tauri + React</b> — the desktop shell and this UI: search, the
                note viewer, and the 3D memory graph.
              </li>
            </ul>
            <p className="docs-foot">
              One Rust engine, three front doors (CLI, MCP, app) — all local.
            </p>
          </section>
        </div>
      </div>
    </div>
  );
}
