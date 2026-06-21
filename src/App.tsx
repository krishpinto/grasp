import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, type MemoryDetail, type ProjectRow, type Stats } from "./lib/api";
import { Overview } from "./components/Overview";
import { Archive } from "./components/Archive";
import { ProjectView } from "./components/ProjectView";
import { DocsView } from "./components/DocsView";
import { NoteView } from "./components/NoteView";
import { LoadingScreen } from "./components/LoadingScreen";

type View = "overview" | "archive" | "project" | "docs";
const SPOTLIGHT_KEY = "engram_spotlight_seen";

export default function App() {
  const [stats, setStats] = useState<Stats | null>(null);
  const [projects, setProjects] = useState<ProjectRow[]>([]);
  const [view, setView] = useState<View>("overview");
  const [project, setProject] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const [memory, setMemory] = useState<MemoryDetail | null>(null);
  const [reloadKey, setReloadKey] = useState(0);

  // Splash on launch.
  const [booting, setBooting] = useState(true);
  const [bootLeaving, setBootLeaving] = useState(false);
  // First-time guided spotlight.
  const [spotlightSeen, setSpotlightSeen] = useState(
    () => localStorage.getItem(SPOTLIGHT_KEY) === "1"
  );

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

  // Splash timing: fade out at 1.3s, unmount after the 420ms fade.
  useEffect(() => {
    const leave = setTimeout(() => setBootLeaving(true), 1300);
    const done = setTimeout(() => setBooting(false), 1720);
    return () => {
      clearTimeout(leave);
      clearTimeout(done);
    };
  }, []);

  // Live capture: backend ingests new transcripts and emits this event.
  useEffect(() => {
    const unlisten = listen<number>("memories-updated", () => {
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
      setReloadKey((k) => k + 1);
    } finally {
      setImporting(false);
    }
  }

  async function openMemory(id: number) {
    setMemory(await api.getMemory(id));
  }

  function dismissSpotlight() {
    setSpotlightSeen(true);
    localStorage.setItem(SPOTLIGHT_KEY, "1");
  }

  function enterArchive() {
    dismissSpotlight();
    setView("archive");
  }

  function selectProject(slug: string) {
    setProject(slug);
    setView("project");
  }

  const showSpotlight = view === "overview" && !spotlightSeen && !booting;

  return (
    <div className="app">
      {booting && <LoadingScreen leaving={bootLeaving} />}

      {view === "overview" && (
        <Overview
          stats={stats}
          reloadKey={reloadKey}
          importing={importing}
          showSpotlight={showSpotlight}
          onOpenMemory={openMemory}
          onEnterArchive={enterArchive}
          onDismissSpotlight={dismissSpotlight}
          onImport={handleImport}
          onOpenDocs={() => setView("docs")}
        />
      )}

      {view === "docs" && <DocsView onBack={() => setView("overview")} />}

      {view === "archive" && (
        <Archive
          projects={projects}
          importing={importing}
          onSelect={selectProject}
          onBack={() => setView("overview")}
          onImport={handleImport}
        />
      )}

      {view === "project" && project && (
        <ProjectView
          project={project}
          reloadKey={reloadKey}
          onOpenMemory={openMemory}
          onBack={() => setView("archive")}
        />
      )}

      <NoteView memory={memory} onClose={() => setMemory(null)} />
    </div>
  );
}
