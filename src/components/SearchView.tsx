import { useState } from "react";
import { api, type SearchHit } from "../lib/api";
import { TYPE_META, shortDate } from "../lib/format";

interface Props {
  project: string | null;
  onOpen: (id: number) => void;
}

export function SearchView({ project, onOpen }: Props) {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [searched, setSearched] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function run() {
    const q = query.trim();
    if (!q) return;
    setLoading(true);
    setError(null);
    try {
      const results = await api.search(q, project ?? undefined, 40);
      setHits(results);
      setSearched(true);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="search-view">
      <div className="search-bar">
        <input
          autoFocus
          value={query}
          placeholder="Search your memory — e.g. email send, auth token, timeout"
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && run()}
        />
        <button onClick={run}>Search</button>
      </div>

      {error && <div className="placeholder error">{error}</div>}
      {loading && <div className="placeholder">Searching…</div>}

      {!loading && searched && hits.length === 0 && (
        <div className="placeholder">No matches{project ? " in this project" : ""}.</div>
      )}
      {!loading && !searched && (
        <div className="placeholder">
          Type a query to search across {project ?? "all"} memories.
        </div>
      )}

      <div className="results">
        {hits.map((h) => {
          const meta = TYPE_META[h.chunk_type];
          return (
            <div key={h.id} className="card" onClick={() => onOpen(h.id)}>
              <div className="card-meta">
                <span
                  className="tag"
                  style={{ color: meta.color, borderColor: meta.color }}
                >
                  {meta.label}
                </span>
                <span>{shortDate(h.timestamp)}</span>
                <span className="dim">{h.project}</span>
              </div>
              <div className="card-text">{h.text}</div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
