import { useState } from "react";
import { GraphView } from "./GraphView";
import { SearchView } from "./SearchView";
import { prettyProject } from "../lib/format";

interface Props {
  project: string;
  reloadKey?: number;
  onOpenMemory: (id: number) => void;
  onBack: () => void;
}

/// A single project: its 3D graph by default, with a search tab scoped to it.
export function ProjectView({ project, reloadKey, onOpenMemory, onBack }: Props) {
  const [tab, setTab] = useState<"graph" | "search">("graph");

  return (
    <div className="screen">
      <div className="topbar">
        <button className="back-btn" onClick={onBack}>
          ← Memories
        </button>
        <div className="topbar-title">{prettyProject(project)}</div>
        <div className="tabs-inline">
          <button
            className={tab === "graph" ? "active" : ""}
            onClick={() => setTab("graph")}
          >
            Graph
          </button>
          <button
            className={tab === "search" ? "active" : ""}
            onClick={() => setTab("search")}
          >
            Search
          </button>
        </div>
      </div>

      {tab === "graph" ? (
        <GraphView project={project} onOpen={onOpenMemory} reloadKey={reloadKey} />
      ) : (
        <SearchView project={project} onOpen={onOpenMemory} />
      )}
    </div>
  );
}
