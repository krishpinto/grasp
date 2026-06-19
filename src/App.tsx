import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, type MemoryDetail, type ProjectRow, type Stats } from "./lib/api";
import { Sidebar } from "./components/Sidebar";
import { SearchView } from "./components/SearchView";
import { GraphView } from "./components/GraphView";
import { NoteView } from "./components/NoteView";

type Tab = "search" | "graph";

export default function App() {
  const [stats, setStats] = useState<Stats | null>(null);
  const [projects, setProjects] = useState<ProjectRow[]>([]);
  const [activeProject, setActiveProject] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("search");
  const [importing, setImporting] = useState(false);
  const [memory, setMemory] = useState<MemoryDetail | null>(null);
  // Bumped whenever memories change, so the graph reloads.
  const [reloadKey, setReloadKey] = useState(0);
  const [liveCount, setLiveCount] = useState(0);

  async function refresh() {
    try {
      setStats(await api.stats());
      setProjects(await api.listProjects());
    } catch {
      /* engine not ready yet */
    }
  }

  useEffect(() => {
    refresh();
  }, []);

  // Live capture: the backend ingests new transcripts and emits this event.
  useEffect(() => {
    const unlisten = listen<number>("memories-updated", (e) => {
      setLiveCount((c) => c + (e.payload ?? 0));
      refresh();
      setReloadKey((k) => k + 1);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  async function handleImport() {
    setImporting(true);
    try {
      await api.import();
      await refresh();
    } finally {
      setImporting(false);
    }
  }

  async function openMemory(id: number) {
    setMemory(await api.getMemory(id));
  }

  return (
    <div className="app">
      <Sidebar
        stats={stats}
        projects={projects}
        activeProject={activeProject}
        onSelectProject={setActiveProject}
        onImport={handleImport}
        importing={importing}
      />

      <main className="main">
        <div className="tabs">
          <button
            className={tab === "search" ? "active" : ""}
            onClick={() => setTab("search")}
          >
            Search
          </button>
          <button
            className={tab === "graph" ? "active" : ""}
            onClick={() => setTab("graph")}
          >
            Graph
          </button>
          {liveCount > 0 && (
            <span className="live-badge" title="Captured live while the app was open">
              ● {liveCount} captured live
            </span>
          )}
        </div>

        {tab === "search" && (
          <SearchView project={activeProject} onOpen={openMemory} />
        )}
        {tab === "graph" && (
          <GraphView
            project={activeProject}
            onOpen={openMemory}
            reloadKey={reloadKey}
          />
        )}
      </main>

      <NoteView memory={memory} onClose={() => setMemory(null)} />
    </div>
  );
}
