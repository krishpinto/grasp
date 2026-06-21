import { useCallback, useEffect, useRef, useState } from "react";
import ForceGraph3D from "react-force-graph-3d";
import SpriteText from "three-spritetext";
import * as THREE from "three";
import { api } from "../lib/api";
import { TYPE_META, typeColor } from "../lib/format";
import { useElementSize } from "../lib/useElementSize";
import { BrainLoader } from "./BrainLoader";

interface Props {
  project: string | null;
  onOpen: (id: number) => void;
  /// Bumped by the parent when memories change, to force a reload.
  reloadKey?: number;
}

interface GNode {
  id: number;
  nodeType: string; // "memory" | "file"
  label: string;
  type: keyof typeof TYPE_META;
  project: string;
  deg: number;
  neighbors: GNode[];
  links: GLink[];
}
interface GLink {
  source: number | GNode;
  target: number | GNode;
  kind: string;
}

const FILE_NODE_COLOR = "#9aa0aa";
const DIM_NODE_COLOR = "#3a3a3c";
const LINK_BASE = "rgba(150,152,160,0.13)";
const LINK_HI = "#8b7cff";
const ACCENT = "#8b7cff";
const ACCENT_BRIGHT = "#b5acff";
/// Only label files + well-connected "hub" memories, so we don't build hundreds
/// of text sprites (perf) and the graph reads cleanly like Obsidian.
const LABEL_MIN_DEGREE = 3;

function baseColor(n: GNode): string {
  return n.nodeType === "file" ? FILE_NODE_COLOR : typeColor(n.type);
}

export function GraphView({ project, onOpen, reloadKey }: Props) {
  const { ref, width, height } = useElementSize<HTMLDivElement>();
  const fgRef = useRef<any>(null);
  const [data, setData] = useState<{ nodes: GNode[]; links: GLink[] }>({
    nodes: [],
    links: [],
  });
  const [loading, setLoading] = useState(true);
  // The force simulation keeps moving after data arrives; cover that "settling"
  // period with the loader so users see the animation, not a jittering graph.
  const [settling, setSettling] = useState(false);

  // Hover highlight: kept in refs so changing it doesn't rebuild node objects.
  const hoverNode = useRef<GNode | null>(null);
  const highlightNodes = useRef(new Set<GNode>());
  const highlightLinks = useRef(new Set<GLink>());
  // A cheap counter just to re-run the (cheap) color accessors on hover.
  const [, bumpColors] = useState(0);

  useEffect(() => {
    let alive = true;
    setLoading(true);
    setSettling(false);
    api
      .getGraph(project ?? undefined)
      .then((g) => {
        if (!alive) return;
        const byId = new Map<number, GNode>();
        for (const n of g.nodes) {
          byId.set(n.id, {
            id: n.id,
            nodeType: n.node_type,
            label: n.label,
            type: n.chunk_type,
            project: n.project,
            deg: 0,
            neighbors: [],
            links: [],
          });
        }
        const links: GLink[] = [];
        for (const e of g.edges) {
          const a = byId.get(e.source);
          const b = byId.get(e.target);
          if (!a || !b) continue;
          const link: GLink = { source: a.id, target: b.id, kind: e.kind };
          links.push(link);
          a.deg++;
          b.deg++;
          a.neighbors.push(b);
          b.neighbors.push(a);
          a.links.push(link);
          b.links.push(link);
        }
        hoverNode.current = null;
        highlightNodes.current.clear();
        highlightLinks.current.clear();
        const nodes = [...byId.values()];
        setData({ nodes, links });
        setLoading(false);
        setSettling(nodes.length > 0); // cleared on the graph's onEngineStop
      })
      .catch(() => {
        setLoading(false);
        setSettling(false);
      });
    return () => {
      alive = false;
    };
  }, [project, reloadKey]);

  // STABLE label builder: built once per node, never rebuilt on hover.
  const nodeThreeObject = useCallback((n: any) => {
    const group = new THREE.Group();
    // Label only files + well-connected hubs (perf + Obsidian-clean look).
    if (n.nodeType === "file" || n.deg >= LABEL_MIN_DEGREE) {
      const sprite = new SpriteText(n.label) as any;
      sprite.color = "#c8c9cc";
      sprite.textHeight = n.nodeType === "file" ? 3.5 : 2.6;
      sprite.fontFace = "Inter, sans-serif";
      sprite.material.depthWrite = false;
      sprite.position.set(0, n.nodeType === "file" ? 7 : 6, 0);
      group.add(sprite);
    }
    return group;
  }, []);

  const handleHover = useCallback((node: GNode | null) => {
    highlightNodes.current.clear();
    highlightLinks.current.clear();
    if (node) {
      highlightNodes.current.add(node);
      node.neighbors.forEach((nb) => highlightNodes.current.add(nb));
      node.links.forEach((l) => highlightLinks.current.add(l));
    }
    hoverNode.current = node;
    bumpColors((c) => c + 1); // re-run cheap color accessors only
  }, []);

  const anyHighlight = highlightNodes.current.size > 0;

  const busy = loading || settling;

  return (
    <div className="graph-wrap" ref={ref}>
      {!loading && data.nodes.length === 0 && (
        <div className="placeholder">No memories to graph yet — import first.</div>
      )}
      {data.nodes.length > 0 && width > 0 && (
        <>
          <ForceGraph3D
            ref={fgRef}
            width={width}
            height={height}
            graphData={data}
            backgroundColor="#1e1e1e"
            showNavInfo={false}
            onEngineStop={() => setSettling(false)}
            warmupTicks={40}
            nodeRelSize={4}
            nodeVal={(n: any) => 1 + Math.sqrt(n.deg)}
            nodeColor={(n: any) => {
              if (anyHighlight) {
                if (n === hoverNode.current) return ACCENT_BRIGHT;
                if (highlightNodes.current.has(n)) return ACCENT;
                return DIM_NODE_COLOR;
              }
              return baseColor(n);
            }}
            nodeOpacity={0.95}
            nodeThreeObjectExtend
            nodeThreeObject={nodeThreeObject}
            linkColor={(l: any) =>
              highlightLinks.current.has(l) ? LINK_HI : LINK_BASE
            }
            linkWidth={(l: any) => (highlightLinks.current.has(l) ? 1.4 : 0.4)}
            linkOpacity={0.5}
            onNodeHover={(n: any) => handleHover(n || null)}
            onNodeClick={(n: any) => {
              if (n.nodeType !== "file") onOpen(n.id);
            }}
          />
          {!busy && (
            <>
              <div className="graph-hint">drag to orbit · scroll to zoom · right-drag to pan</div>
              <Legend />
            </>
          )}
        </>
      )}
      {busy && (
        <div className="graph-loading">
          <BrainLoader caption="Building graph…" />
        </div>
      )}
    </div>
  );
}

function Legend() {
  return (
    <div className="legend">
      {(Object.keys(TYPE_META) as (keyof typeof TYPE_META)[]).map((t) => (
        <span key={t} className="legend-item">
          <span className="dot" style={{ background: TYPE_META[t].color }} />
          {TYPE_META[t].label}
        </span>
      ))}
      <span className="legend-item">
        <span className="dot" style={{ background: FILE_NODE_COLOR }} />
        File
      </span>
    </div>
  );
}
