import { useCallback, useEffect, useState } from "react";
import { api } from "../../api";
import type { FileNode } from "../../types";

interface Props {
  /** Active project root. The tree's "root" entries come from listing this dir. */
  projectPath: string;
  onOpenFile: (path: string) => void;
}

interface NodeState {
  expanded: boolean;
  children?: FileNode[];
  loading?: boolean;
}

function ChevronIcon({ open }: { open: boolean }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width="10"
      height="10"
      fill="none"
      stroke="currentColor"
      strokeWidth="2.2"
      strokeLinecap="round"
      strokeLinejoin="round"
      style={{ transform: open ? "rotate(90deg)" : "rotate(0deg)", transition: "transform 120ms" }}
    >
      <path d="M9 6l6 6-6 6" />
    </svg>
  );
}

function FolderIcon({ open }: { open: boolean }) {
  return (
    <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" className="text-violet-300/85">
      {open ? (
        <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2l-1.5 7a2 2 0 0 1-2 1.5H5a2 2 0 0 1-2-2z" />
      ) : (
        <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
      )}
    </svg>
  );
}

function FileIcon() {
  return (
    <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="text-white/45">
      <path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z" />
      <path d="M14 3v6h6" />
    </svg>
  );
}

interface RowProps {
  node: FileNode;
  depth: number;
  states: Record<string, NodeState>;
  setState: (path: string, state: NodeState) => void;
  onOpenFile: (path: string) => void;
}

function Row({ node, depth, states, setState, onOpenFile }: RowProps) {
  const state = states[node.path];
  const expanded = !!state?.expanded;

  const toggle = useCallback(async () => {
    if (node.kind === "file") {
      onOpenFile(node.path);
      return;
    }
    if (expanded) {
      setState(node.path, { ...state, expanded: false });
      return;
    }
    if (state?.children) {
      setState(node.path, { ...state, expanded: true });
      return;
    }
    setState(node.path, { expanded: true, loading: true });
    try {
      const children = await api.listProjectFiles(node.path);
      setState(node.path, { expanded: true, children });
    } catch (err) {
      console.warn("[vibn] list_project_files failed", node.path, err);
      setState(node.path, { expanded: false });
    }
  }, [node, state, expanded, setState, onOpenFile]);

  return (
    <>
      <button
        type="button"
        onClick={toggle}
        className="w-full text-left flex items-center gap-1.5 px-2 py-[3px] text-[12px] text-white/80 hover:bg-white/[0.04] transition-colors"
        style={{ paddingLeft: 8 + depth * 12 }}
        title={node.path}
      >
        <span className="w-3 grid place-items-center text-white/40">
          {node.kind === "dir" ? <ChevronIcon open={expanded} /> : null}
        </span>
        {node.kind === "dir" ? <FolderIcon open={expanded} /> : <FileIcon />}
        <span className="truncate">{node.name}</span>
      </button>
      {expanded && state?.children && (
        <>
          {state.children.map((child) => (
            <Row
              key={child.path}
              node={child}
              depth={depth + 1}
              states={states}
              setState={setState}
              onOpenFile={onOpenFile}
            />
          ))}
        </>
      )}
    </>
  );
}

export function FileTree({ projectPath, onOpenFile }: Props) {
  const [roots, setRoots] = useState<FileNode[] | null>(null);
  const [states, setStates] = useState<Record<string, NodeState>>({});
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setRoots(null);
    setStates({});
    setError(null);
    (async () => {
      try {
        const data = await api.listProjectFiles(projectPath);
        if (!cancelled) setRoots(data);
      } catch (err) {
        if (!cancelled) setError(String(err));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [projectPath]);

  const setState = useCallback((path: string, state: NodeState) => {
    setStates((prev) => ({ ...prev, [path]: state }));
  }, []);

  if (error) {
    return (
      <div className="p-3 text-[11px] text-red-300/80">Failed to load files: {error}</div>
    );
  }
  if (!roots) {
    return <div className="p-3 text-[11px] text-white/35">Loading files…</div>;
  }
  if (roots.length === 0) {
    return <div className="p-3 text-[11px] text-white/35">Empty directory</div>;
  }

  return (
    <div className="py-1 select-none">
      {roots.map((node) => (
        <Row
          key={node.path}
          node={node}
          depth={0}
          states={states}
          setState={setState}
          onOpenFile={onOpenFile}
        />
      ))}
    </div>
  );
}
