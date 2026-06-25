# Grasp — How It Actually Works (file-by-file)

Written so you can defend it cold. Read it next to the code. Every claim points
at the file that proves it.

---

## The one-sentence truth (memorize this, you keep getting it wrong)

Grasp does **not** vectorize your source code. It reads **Claude Code session
transcripts** (`~/.claude/projects/*.jsonl`) and remembers the *meaningful
moments of your work* — decisions, file-writes, error fixes, summaries. That's
the wedge: other tools store code structure; you store **the story of how the
project was built**.

---

## The pipeline, stage by stage, with the file that does it

```
transcripts (*.jsonl)
   │  watch.rs        ← passive capture: notices changed files
   ▼
parser.rs            ← each JSONL line → a normalized Entry
   ▼
extractor.rs         ← Entry stream → Chunks worth remembering
   ▼
redact.rs            ← scrub secrets BEFORE anything is stored
   ▼
store/markdown.rs    ← write Markdown (the source of truth)
store/index.rs       ← insert into SQLite + FTS5 (the fast index)
   ▼   ── embedding is a SEPARATE step, not on this path ──
embed.rs             ← candle MiniLM turns text → 384-dim vectors
store/vectors.rs     ← store the vectors
   ▼
lib.rs :: search()   ← BM25 + cosine, fused with RRF → ranked results
store/graph.rs       ← build the 3D node/edge graph for the app
```

The orchestration lives in `lib.rs`. The CLI (`grasp-cli/src/main.rs`) and the
MCP server (`mcp.rs`) are thin drivers on top of it.

---

## Your daemon question, answered (this is a GOOD answer — learn it)

**Q: When Claude writes the transcript 50 times in 2 seconds, does the watcher
fire the model every time? Isn't that wasteful?**

No, and here's exactly why:

1. `watch.rs` uses a **debouncer** (`notify_debouncer_mini`). It collapses a
   burst of writes to one file into a **single** event emitted only after the
   file goes quiet for `debounce` duration. 50 writes in 2s = 1 ingest. (See
   `watch(dir, debounce)`.)
2. When it does fire, it only emits the **changed file path**. The consumer calls
   `import_file` in `import.rs`, which is: read file → `parser::parse_line` →
   `extractor::extract_session` → redact → write Markdown + SQLite insert.
3. **None of that loads the embedding model.** Importing is pure string work +
   SQLite. The 90 MB MiniLM model in `embed.rs` is only loaded when you actually
   **embed** (a separate step) or **search**. So the hot path — the thing that
   runs constantly in the background — is cheap by design.
4. Bonus cheapness: `import_file` hashes the file content and checks
   `file_already_processed` — if nothing changed since last time, it skips
   entirely (`import.rs:123`).

So the strong interview line: *"Passive capture is cheap because the watcher is
debounced and the ingest path never touches the model — embedding is a separate,
batched step. The expensive thing only runs when you search or explicitly embed."*

---

## The concepts you flubbed — corrected, simply

### "Candle" — what it actually is
Candle is a **machine-learning framework written in Rust**, made by Hugging Face.
It's the *engine that runs the neural network* — the PyTorch-equivalent for Rust.
You do **not** "get the model from candle." You get the **model weights** from the
Hugging Face hub (or a copy bundled next to the .exe), and candle **runs** them.
(See `embed.rs::resolve_model_files` — it downloads `config.json`,
`tokenizer.json`, `model.safetensors`, then candle loads them.)

### The model: all-MiniLM-L6-v2
- **~22 million parameters** (NOT 1.5 billion — that error kills you).
- **6 transformer layers**, outputs a **384-dimensional** vector per text.
- ~90 MB on disk. Runs on **CPU** (`Device::Cpu` in `embed.rs`).
- It's a *sentence-embedding* model: text in → one vector that captures meaning.

### Why candle and not ort/fastembed (the real reason)
`ort` (the ONNX runtime) **ships no prebuilt binaries for the GNU/MinGW
toolchain** you build with on Windows. candle is pure Rust, compiles cleanly, no
ONNX dependency. That's it — it's a toolchain/portability decision, stated right
in the `embed.rs` doc comment.

### Why no API
The entire pitch is **zero-API, fully local, private**. Your transcripts contain
your work and secrets. Sending them to an embedding API would break the one
promise that makes Grasp different. Local is the product, not a constraint.

### Mean pooling (you said "no idea")
The model outputs **one vector per token** (per word-piece). You need **one
vector for the whole sentence**. Mean pooling = **average all the token vectors
together** (weighted by the attention mask so padding tokens don't count). See
`embed.rs:125-130` — that's the `summed / counts` math. Why mean and not the
[CLS] token? Because sentence-transformer models are *trained* to produce good
embeddings via mean pooling; the CLS token isn't meaningful for them.

### L2 normalization (you said "no idea")
"L2 normalize" = **scale every vector so its length is exactly 1** (divide by its
magnitude). Why? Because once both vectors have length 1, **cosine similarity
becomes identical to the dot product** — so the comparison is just multiply-and-
add, no division needed. That's why `cosine()` in `embed.rs:149` is a one-line
dot product: the normalization already did the hard part.

### Cosine vs Euclidean (you mixed these up)
- **Cosine similarity** = the *angle* between two vectors. Ignores length, cares
  about direction = meaning. **This is what Grasp uses.**
- **Euclidean distance** = straight-line distance between the points. Grasp does
  **not** use this. Don't say it.
- "Dot product" isn't a third thing here — on normalized vectors it *equals*
  cosine. That's the trick.

### BM25 (you got this roughly right)
BM25 is a classic **keyword-ranking** algorithm — like a smarter version of
"count how many query words appear, weighted by how rare they are." It's built
into **SQLite's FTS5** full-text extension; you call `bm25(chunks_fts)` and lower
score = more relevant (`store/index.rs:42`).

### Hybrid + RRF, k=60 (you said k=16 — it's 60)
Two separate searches run:
1. **BM25** → ranked list by keyword match.
2. **Cosine** → ranked list by meaning (brute-force, see below).

Then **Reciprocal Rank Fusion** merges them. For each result, score =
`1 / (k + rank)` from each list, summed. The list that ranks something higher
contributes more. **k=60** is a damping constant (from the original RRF paper) —
it stops the very top ranks from completely dominating. Big k = ranks matter
less; small k = top results dominate. The whole point of RRF: it fuses by **rank
position, not raw score**, so it doesn't matter that BM25 and cosine produce
scores on totally different, incompatible scales. (See `lib.rs:116-123`.)

### "Brute force" semantic search (you said "no idea")
"Brute force" = to find the closest vectors, Grasp compares your query vector
against **every single stored vector, one by one** (`lib.rs:109-114`). Simple and
exact. The downside: it's **O(n)** — fine for thousands of memories, too slow for
millions. The real-world fix (and a great "what would you improve" answer): an
**approximate nearest-neighbor index** like HNSW / sqlite-vec.

### "Inline rationale" exception in the extractor (you said "no idea")
When extracting a decision, Grasp grabs the decision sentence **plus the next
sentence** (the rationale). BUT if the decision sentence already contains a word
like "because", "since", "instead of" — the reason is *already in that sentence*,
so grabbing the next one would pull in **unrelated** text. So it skips the
next-sentence grab in that case. (See `extractor.rs::decision_spans` +
`INLINE_RATIONALE`, and the test that proves it doesn't swallow junk.)

### Redaction (you mostly had this)
Before any chunk is stored *or even hashed*, `redact.rs::scrub` replaces secrets
(private keys, JWTs, `sk-…`, `ghp_…`, `AKIA…`, bearer tokens, `KEY=value`) with
`[REDACTED]`. Done **before truncation** so a long secret can't get sliced in
half and half-leaked (`extractor.rs:198`).

### Why BOTH Markdown and SQLite (your weak answer: "looks cool")
The real reason: **Markdown is the git-committable, human-readable source of
truth that survives the database.** Wipe the DB, your memory still exists as plain
files. SQLite is the *disposable, rebuildable fast index* on top. `import.rs:141`
even writes Markdown **first**, then indexes — because Markdown is canonical.

### Why a "meaningfulness" test on embeddings (you said "no idea")
A shape check ("is it 384 numbers?") passes even if the model is loaded wrong and
outputs garbage. The test in `embed.rs` checks a *related* sentence scores higher
than an *unrelated* one — catching the class of bug where embeddings are
technically valid but **semantically meaningless**. That's mature testing
instinct; claim it.

---

## Which files actually matter (study order)

1. `lib.rs` — the orchestrator + `hybrid_search` (RRF). **Most important.**
2. `embed.rs` — the model, mean pooling, normalization, cosine.
3. `extractor.rs` — what gets remembered and why.
4. `store/index.rs` — SQLite + BM25.
5. `import.rs` + `watch.rs` — the passive-capture pipeline.
6. `redact.rs`, `store/markdown.rs`, `store/vectors.rs`, `store/graph.rs` — supporting.

If you can narrate files 1–5 from memory, you own Grasp.
```
