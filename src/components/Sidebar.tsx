import type { ProjectRow, Stats } from "../lib/api";

interface Props {
  stats: Stats | null;
  projects: ProjectRow[];
  activeProject: string | null;
  onSelectProject: (slug: string | null) => void;
  onImport: () => void;
  importing: boolean;
}

export function Sidebar({
  stats,
  projects,
  activeProject,
  onSelectProject,
  onImport,
  importing,
}: Props) {
  return (
    <aside className="sidebar">
      <div className="brand">🧠 Grasp</div>
      <div className="stat">
        {stats
          ? `${stats.total_chunks} memories · ${stats.total_projects} project(s)`
          : "…"}
      </div>

      <button className="import-btn" onClick={onImport} disabled={importing}>
        {importing ? "Importing…" : "Import transcripts"}
      </button>

      <div className="proj-head">Projects</div>
      <ul className="proj-list">
        <li
          className={activeProject === null ? "active" : ""}
          onClick={() => onSelectProject(null)}
        >
          <span>All projects</span>
        </li>
        {projects.map((p) => (
          <li
            key={p.slug}
            className={activeProject === p.slug ? "active" : ""}
            onClick={() => onSelectProject(p.slug)}
            title={p.path}
          >
            <span className="proj-name">{p.slug}</span>
            <span className="proj-count">{p.chunk_count}</span>
          </li>
        ))}
      </ul>
    </aside>
  );
}
