"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useMemo, useState } from "react";
import {
  addChild,
  addConstraint,
  addLink,
  addTask,
  deleteCanvas,
  deleteNode,
  labelFor,
  openNested,
  removeConstraint,
  removeTask,
  setClosed,
  setConstraintText,
  setConstrainedPercent,
  setTaskDeadline,
  setTaskDone,
  setTaskText,
  setText,
} from "../../lib/library";
import { api, saveLibrary } from "./api";
import Shell from "./Shell";

export default function MapEditor({ canvasId }) {
  const router = useRouter();
  const [sync, setSync] = useState(null);
  const [library, setLibrary] = useState(null);
  const [revision, setRevision] = useState(0);
  const [selected, setSelected] = useState(null);
  const [draft, setDraft] = useState("");
  const [notice, setNotice] = useState("");
  const [conflict, setConflict] = useState(null);
  const [linkFrom, setLinkFrom] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);

  useEffect(() => {
    let gone = false;
    api("/api/v1/sync")
      .then((data) => {
        if (gone) return;
        setSync(data);
        setLibrary(data.library);
        setRevision(data.revision || 0);
      })
      .catch((error) => {
        if (!gone) setNotice(error.message);
      });
    return () => {
      gone = true;
    };
  }, [canvasId]);

  const canvas = library?.canvases?.[canvasId] || null;
  const positions = useMemo(() => (canvas ? layoutOf(canvas) : new Map()), [canvas]);
  const selectedNode = canvas && selected?.type === "node"
    ? canvas.nodes.find((node) => node.id === selected.id)
    : null;
  const selectedEdge = canvas && selected?.type === "edge"
    ? canvas.edges.find((edge) => edge.id === selected.id)
    : null;

  useEffect(() => {
    setDraft(selectedNode ? selectedNode.text : "");
  }, [selectedNode?.id, selectedNode?.text]);

  async function commit(mutator) {
    if (!library) return null;
    const result = mutator(library);
    if (!result) {
      setNotice("That edit is not available. A closed idea cannot be changed.");
      return null;
    }
    try {
      const saved = await saveLibrary(result.library, revision);
      setLibrary(result.library);
      setRevision(saved.revision);
      setConflict(null);
      setNotice("");
      return result;
    } catch (error) {
      if (error.status === 409) {
        setConflict(error.data);
        setNotice("The cloud copy changed somewhere else. Reload before editing, or replace the cloud with this page.");
      } else {
        setNotice(error.message);
      }
      return null;
    }
  }

  async function replaceCloud() {
    if (!library) return;
    try {
      const saved = await saveLibrary(library, revision, true);
      setRevision(saved.revision);
      setConflict(null);
      setNotice("This page replaced the cloud copy. The previous copy is kept as a backup.");
    } catch (error) {
      setNotice(error.message);
    }
  }

  function chooseNode(id) {
    if (linkFrom && linkFrom !== id) {
      const from = linkFrom;
      setLinkFrom("");
      commit((current) => addLink(current, canvasId, from, id));
      return;
    }
    setSelected({ type: "node", id });
    setConfirmDelete(false);
  }

  async function saveDraft() {
    if (!selectedNode) return;
    await commit((current) => setText(current, canvasId, selectedNode.id, draft));
  }

  async function branch() {
    if (!selectedNode) return;
    const result = await commit((current) => addChild(current, canvasId, selectedNode.id));
    if (result?.nodeId) setSelected({ type: "node", id: result.nodeId });
  }

  async function nest() {
    if (!selectedNode) return;
    const result = await commit((current) => openNested(current, canvasId, selectedNode.id));
    if (result?.canvasId && result.canvasId !== canvasId) router.push(`/map/${result.canvasId}`);
  }

  async function removeNode() {
    if (!selectedNode) return;
    const result = await commit((current) => deleteNode(current, canvasId, selectedNode.id));
    if (result) setSelected(null);
  }

  async function removeMap() {
    const result = await commit((current) => deleteCanvas(current, canvasId));
    if (result) router.push("/");
  }

  if (!sync) {
    return (
      <Shell>
        <h1>Map</h1>
        <p className="lede">{notice || "Loading the cloud copy…"}</p>
      </Shell>
    );
  }
  if (!canvas) {
    return (
      <Shell>
        <h1>Map</h1>
        <p className="lede">That canvas is not on the cloud.</p>
        <Link href="/">Back to canvases</Link>
      </Shell>
    );
  }

  const bounds = boundsOf(positions, canvas);
  const crumbs = breadcrumb(library, canvasId);
  return (
    <Shell>
      <div className="row">
        <h1 style={{ marginRight: "auto" }}>{labelFor(centerText(canvas))}</h1>
        <button className="quiet" type="button" onClick={() => router.push("/")}>All canvases</button>
        {confirmDelete ? (
          <button className="danger" type="button" onClick={removeMap}>Confirm delete</button>
        ) : (
          <button className="quiet" type="button" onClick={() => setConfirmDelete(true)}>Delete map</button>
        )}
      </div>
      <p className="lede">
        {crumbs.map((crumb, index) => (
          <span key={crumb.id}>
            {index > 0 ? " / " : ""}
            {crumb.id === canvasId ? crumb.text : <Link href={`/map/${crumb.id}`}>{crumb.text}</Link>}
          </span>
        ))}
        {" · "}revision {revision}
      </p>
      {notice ? <p className="notice">{notice}</p> : null}
      {conflict ? (
        <p className="row">
          <button className="quiet" type="button" onClick={() => window.location.reload()}>Reload cloud copy</button>
          <button className="primary" type="button" onClick={replaceCloud}>Replace the cloud with this page</button>
        </p>
      ) : null}
      <div className="map-layout">
        <div className="map-board">
          <svg viewBox={`${bounds.x} ${bounds.y} ${bounds.w} ${bounds.h}`}>
            {(canvas.edges || []).map((edge) => {
              const from = positions.get(edge.from);
              const to = positions.get(edge.to);
              if (!from || !to) return null;
              const active = selectedEdge?.id === edge.id;
              return (
                <line
                  key={edge.id}
                  x1={from.x}
                  y1={from.y}
                  x2={to.x}
                  y2={to.y}
                  stroke={active ? "#b8542a" : "#5c4e40"}
                  strokeWidth={active ? 4 : 2.5}
                  strokeDasharray={edge.kind === "link" ? "7 6" : undefined}
                  style={{ cursor: "pointer" }}
                  onClick={(event) => {
                    event.stopPropagation();
                    setSelected({ type: "edge", id: edge.id });
                    setLinkFrom("");
                  }}
                />
              );
            })}
            {canvas.nodes.map((node) => {
              const at = positions.get(node.id);
              if (!at) return null;
              const center = node.id === canvas.center_id;
              const active = selectedNode?.id === node.id;
              const fill = node.closed ? "#e2dcd2" : "#fffcf7";
              const width = Math.max(112, Math.min(210, labelFor(node.text).length * 8 + 36));
              return (
                <g
                  key={node.id}
                  transform={`translate(${at.x} ${at.y})`}
                  style={{ cursor: "pointer" }}
                  onClick={(event) => {
                    event.stopPropagation();
                    chooseNode(node.id);
                  }}
                >
                  {center ? (
                    <circle r="58" fill={fill} stroke={active ? "#b8542a" : "#d6cdbe"} strokeWidth={active ? 3 : 1.5} />
                  ) : (
                    <rect
                      x={-width / 2}
                      y="-26"
                      width={width}
                      height="52"
                      rx="8"
                      fill={fill}
                      stroke={active ? "#b8542a" : "#d6cdbe"}
                      strokeWidth={active ? 3 : 1.5}
                    />
                  )}
                  <text className="node-label" textAnchor="middle" dominantBaseline="middle">
                    {trimLabel(labelFor(node.text), center ? 16 : 24)}
                  </text>
                </g>
              );
            })}
          </svg>
        </div>
        <aside className="card inspector">
          {selectedNode ? (
            <NodePanel
              node={selectedNode}
              center={selectedNode.id === canvas.center_id}
              draft={draft}
              setDraft={setDraft}
              onSave={saveDraft}
              onBranch={branch}
              onToggle={() => commit((current) => setClosed(current, canvasId, selectedNode.id, !selectedNode.closed))}
              onDelete={removeNode}
              onNest={nest}
              linking={linkFrom === selectedNode.id}
              onLink={() => setLinkFrom(linkFrom === selectedNode.id ? "" : selectedNode.id)}
            />
          ) : null}
          {selectedEdge ? (
            <EdgePanel
              edge={selectedEdge}
              canvas={canvas}
              onTask={(taskId, change) => commit((current) => change(current, canvasId, selectedEdge.id, taskId))}
              onEdge={(change) => commit((current) => change(current, canvasId, selectedEdge.id))}
            />
          ) : null}
          {!selectedNode && !selectedEdge ? (
            <p className="meta">Click a circle, a rectangle, or a line.</p>
          ) : null}
        </aside>
      </div>
    </Shell>
  );
}

function NodePanel({ node, center, draft, setDraft, onSave, onBranch, onToggle, onDelete, onNest, linking, onLink }) {
  return (
    <>
      <strong>{center ? "Centre circle" : "Rectangle"}</strong>
      <label className="field">
        Wording
        <textarea value={draft} onChange={(event) => setDraft(event.target.value)} />
      </label>
      <div className="row">
        <button className="primary" type="button" onClick={onSave}>Save text</button>
        <button className="quiet" type="button" onClick={onBranch}>Add branch</button>
      </div>
      <div className="row">
        <button className="quiet" type="button" onClick={onNest}>
          {node.child_canvas_id ? "Open nested map" : "Make nested map"}
        </button>
        {center ? null : (
          <button className="quiet" type="button" onClick={onToggle}>{node.closed ? "Open" : "Close"}</button>
        )}
      </div>
      {center ? null : (
        <div className="row">
          <button className="quiet" type="button" onClick={onLink}>{linking ? "Linking… click another rectangle" : "Link to another"}</button>
          <button className="danger" type="button" onClick={onDelete}>Delete</button>
        </div>
      )}
      <p className="meta">Created {formatWhen(node.created_at)}. Changed {formatWhen(node.modified_at)}.</p>
    </>
  );
}

function EdgePanel({ edge, canvas, onTask, onEdge }) {
  const from = canvas.nodes.find((node) => node.id === edge.from);
  const to = canvas.nodes.find((node) => node.id === edge.to);
  return (
    <>
      <strong>{edge.kind === "link" ? "Link" : "Branch"}</strong>
      <p className="meta">{labelFor(from?.text)} → {labelFor(to?.text)}</p>
      <div className="row">
        <button className="quiet" type="button" onClick={() => onEdge((library, canvasId, edgeId) => addTask(library, canvasId, edgeId))}>
          Add task
        </button>
        <button className="quiet" type="button" onClick={() => onEdge((library, canvasId, edgeId) => addConstraint(library, canvasId, edgeId))}>
          Add constraint
        </button>
      </div>
      {(edge.tasks || []).map((task) => (
        <TaskRow key={task.id} task={task} onTask={onTask} />
      ))}
      {(edge.constraints || []).map((constraint) => (
        <ConstraintRow key={constraint.id} constraint={constraint} onTask={onTask} />
      ))}
      <label className="field">
        Percent constrained
        <input
          key={edge.id}
          type="number"
          min="0"
          max="100"
          defaultValue={edge.constrained_percent ?? ""}
          onBlur={(event) => {
            const raw = event.target.value;
            onEdge((library, canvasId, edgeId) =>
              setConstrainedPercent(library, canvasId, edgeId, raw === "" ? null : Number(raw)),
            );
          }}
        />
      </label>
    </>
  );
}

function TaskRow({ task, onTask }) {
  const [text, setText] = useState(task.text);
  useEffect(() => setText(task.text), [task.text]);
  return (
    <div className="stack">
      <div className="task">
        <input
          type="checkbox"
          checked={task.done}
          onChange={(event) => onTask(task.id, (library, canvasId, edgeId, taskId) =>
            setTaskDone(library, canvasId, edgeId, taskId, event.target.checked))}
          aria-label="Done"
        />
        <input type="text" value={text} onChange={(event) => setText(event.target.value)} aria-label="Task" />
        <button className="quiet" type="button" onClick={() => onTask(task.id, (library, canvasId, edgeId, taskId) => removeTask(library, canvasId, edgeId, taskId))}>
          Remove
        </button>
      </div>
      <div className="row">
        <button className="quiet" type="button" onClick={() => onTask(task.id, (library, canvasId, edgeId, taskId) => setTaskText(library, canvasId, edgeId, taskId, text))}>
          Save task
        </button>
        <input
          type="date"
          value={task.deadline || ""}
          aria-label="Deadline"
          onChange={(event) => onTask(task.id, (library, canvasId, edgeId, taskId) =>
            setTaskDeadline(library, canvasId, edgeId, taskId, event.target.value || null))}
        />
      </div>
    </div>
  );
}

function ConstraintRow({ constraint, onTask }) {
  const [text, setText] = useState(constraint.text);
  useEffect(() => setText(constraint.text), [constraint.text]);
  return (
    <div className="row">
      <input type="text" value={text} onChange={(event) => setText(event.target.value)} aria-label="Constraint" style={{ flex: 1 }} />
      <button className="quiet" type="button" onClick={() => onTask(constraint.id, (library, canvasId, edgeId, id) => setConstraintText(library, canvasId, edgeId, id, text))}>
        Save
      </button>
      <button className="quiet" type="button" onClick={() => onTask(constraint.id, (library, canvasId, edgeId, id) => removeConstraint(library, canvasId, edgeId, id))}>
        Remove
      </button>
    </div>
  );
}

function layoutOf(canvas) {
  const children = new Map();
  for (const item of canvas.nodes || []) {
    if (!item.parent_id) continue;
    const list = children.get(item.parent_id) || [];
    list.push(item.id);
    children.set(item.parent_id, list);
  }
  const pos = new Map();
  function place(id, depth, angle) {
    if (depth === 0) pos.set(id, { x: 0, y: 0 });
    else {
      const radius = 170 + (depth - 1) * 130;
      pos.set(id, { x: Math.cos(angle) * radius, y: Math.sin(angle) * radius });
    }
    const kids = children.get(id) || [];
    kids.forEach((kid, index) => {
      const spread = depth === 0 ? Math.PI * 2 : Math.PI * 0.85;
      const start = depth === 0 ? -Math.PI : angle - spread / 2;
      const step = kids.length === 0 ? 0 : spread / kids.length;
      place(kid, depth + 1, start + step * index + step / 2);
    });
  }
  if (canvas.center_id) place(canvas.center_id, 0, 0);
  let extra = 0;
  for (const item of canvas.nodes || []) {
    if (!pos.has(item.id)) {
      pos.set(item.id, { x: extra * 170 - 80, y: 360 });
      extra += 1;
    }
  }
  return pos;
}

function boundsOf(positions, canvas) {
  let minX = -80;
  let minY = -80;
  let maxX = 80;
  let maxY = 80;
  for (const item of canvas.nodes || []) {
    const at = positions.get(item.id);
    if (!at) continue;
    const pad = item.id === canvas.center_id ? 80 : 120;
    minX = Math.min(minX, at.x - pad);
    minY = Math.min(minY, at.y - pad);
    maxX = Math.max(maxX, at.x + pad);
    maxY = Math.max(maxY, at.y + pad);
  }
  return { x: minX, y: minY, w: maxX - minX, h: maxY - minY };
}

function breadcrumb(library, canvasId) {
  const chain = [];
  const seen = new Set();
  let current = canvasId;
  while (current && library.canvases?.[current] && !seen.has(current)) {
    seen.add(current);
    chain.push({ id: current, text: labelFor(centerText(library.canvases[current])) });
    current = library.canvases[current].parent_canvas_id;
  }
  return chain.reverse();
}

function centerText(canvas) {
  return canvas.nodes.find((node) => node.id === canvas.center_id)?.text || "";
}

function trimLabel(text, max) {
  return text.length > max ? `${text.slice(0, max - 1)}…` : text;
}

function formatWhen(value) {
  if (!value) return "unknown";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}
