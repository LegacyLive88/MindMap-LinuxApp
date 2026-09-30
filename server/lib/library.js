// Edits the same map document the desktop program stores.
// Functions return a new library and leave the input alone.

export function emptyLibrary() {
  return { version: 1, root_order: [], last_open: null, canvases: {} };
}

export function newId() {
  return globalThis.crypto.randomUUID();
}

export function nowIso() {
  return new Date().toISOString();
}

export function labelFor(text) {
  const trimmed = String(text || "").trim();
  return trimmed || "New idea";
}

export function compactLibrary(library) {
  const copy = structuredClone(library);
  for (const canvas of Object.values(copy.canvases || {})) {
    for (const edge of canvas.edges || []) {
      if (Array.isArray(edge.tasks)) {
        for (const task of edge.tasks) {
          if (!task.deadline) delete task.deadline;
        }
        if (edge.tasks.length === 0) delete edge.tasks;
      }
      if (Array.isArray(edge.constraints) && edge.constraints.length === 0) {
        delete edge.constraints;
      }
      if (edge.constrained_percent == null) delete edge.constrained_percent;
    }
  }
  return copy;
}

export function createRoot(library, text, now = nowIso()) {
  const next = clone(library);
  const centerId = newId();
  const canvas = {
    id: newId(),
    parent_canvas_id: null,
    parent_node_id: null,
    center_id: centerId,
    nodes: [
      {
        id: centerId,
        text: String(text ?? ""),
        created_at: now,
        modified_at: now,
        closed: false,
        parent_id: null,
        child_canvas_id: null,
      },
    ],
    edges: [],
  };
  next.canvases[canvas.id] = canvas;
  next.root_order = [...(next.root_order || []), canvas.id];
  next.last_open = canvas.id;
  return { library: next, canvasId: canvas.id };
}

export function addChild(library, canvasId, parentId, now = nowIso()) {
  const canvas = library.canvases?.[canvasId];
  if (!canvas || !node(canvas, parentId) || effectivelyClosed(canvas, parentId)) return null;
  const next = clone(library);
  const target = next.canvases[canvasId];
  const childId = newId();
  target.nodes.push({
    id: childId,
    text: "",
    created_at: now,
    modified_at: now,
    closed: false,
    parent_id: parentId,
    child_canvas_id: null,
  });
  target.edges.push(edge(parentId, childId, "tree"));
  const parent = node(target, parentId);
  parent.modified_at = now;
  return { library: next, nodeId: childId };
}

export function addLink(library, canvasId, a, b, now = nowIso()) {
  if (!a || !b || a === b) return null;
  const canvas = library.canvases?.[canvasId];
  if (!canvas || canvas.center_id === a || canvas.center_id === b) return null;
  if (!node(canvas, a) || !node(canvas, b)) return null;
  if (effectivelyClosed(canvas, a) || effectivelyClosed(canvas, b)) return null;
  if (canvas.edges.some((edge) => (edge.from === a && edge.to === b) || (edge.from === b && edge.to === a))) {
    return null;
  }
  const next = clone(library);
  const target = next.canvases[canvasId];
  target.edges.push(edge(a, b, "link"));
  node(target, a).modified_at = now;
  node(target, b).modified_at = now;
  return { library: next };
}

export function setText(library, canvasId, nodeId, text, now = nowIso()) {
  const canvas = library.canvases?.[canvasId];
  const current = canvas && node(canvas, nodeId);
  if (!current) return null;
  if (canvas.center_id !== nodeId && effectivelyClosed(canvas, nodeId)) return null;
  if (current.text === text) return { library };
  const next = clone(library);
  const target = node(next.canvases[canvasId], nodeId);
  target.text = text;
  target.modified_at = now;
  return { library: next };
}

export function setClosed(library, canvasId, nodeId, closed, now = nowIso()) {
  const canvas = library.canvases?.[canvasId];
  if (!canvas || canvas.center_id === nodeId || !node(canvas, nodeId)) return null;
  const next = clone(library);
  const target = node(next.canvases[canvasId], nodeId);
  target.closed = Boolean(closed);
  target.modified_at = now;
  return { library: next };
}

export function deleteNode(library, canvasId, nodeId, now = nowIso()) {
  const canvas = library.canvases?.[canvasId];
  const current = canvas && node(canvas, nodeId);
  if (!current || canvas.center_id === nodeId) return null;
  const next = clone(library);
  if (current.child_canvas_id && next.canvases[current.child_canvas_id]) {
    promote(next, current.child_canvas_id);
  }
  const target = next.canvases[canvasId];
  for (const item of target.nodes) {
    if (item.parent_id === nodeId) item.parent_id = null;
  }
  const parentId = current.parent_id;
  target.nodes = target.nodes.filter((item) => item.id !== nodeId);
  target.edges = target.edges.filter((item) => item.from !== nodeId && item.to !== nodeId);
  if (parentId) {
    const parent = node(target, parentId);
    if (parent) parent.modified_at = now;
  }
  return { library: next };
}

export function deleteCanvas(library, canvasId) {
  if (!library.canvases?.[canvasId]) return null;
  const next = clone(library);
  const ids = descendants(next, canvasId);
  const idSet = new Set(ids);
  for (const canvas of Object.values(next.canvases)) {
    for (const item of canvas.nodes) {
      if (item.child_canvas_id && idSet.has(item.child_canvas_id)) item.child_canvas_id = null;
    }
  }
  for (const id of ids) delete next.canvases[id];
  next.root_order = (next.root_order || []).filter((id) => !idSet.has(id));
  if (idSet.has(next.last_open)) next.last_open = next.root_order[0] || null;
  return { library: next };
}

export function openNested(library, canvasId, nodeId, now = nowIso()) {
  const canvas = library.canvases?.[canvasId];
  const current = canvas && node(canvas, nodeId);
  if (!current) return null;
  if (current.child_canvas_id && library.canvases[current.child_canvas_id]) {
    const next = clone(library);
    next.last_open = current.child_canvas_id;
    return { library: next, canvasId: current.child_canvas_id };
  }
  if (canvas.center_id !== nodeId && effectivelyClosed(canvas, nodeId)) return null;
  const created = createRoot(library, String(current.text || "").trim(), now);
  const next = created.library;
  const child = next.canvases[created.canvasId];
  child.parent_canvas_id = canvasId;
  child.parent_node_id = nodeId;
  next.root_order = next.root_order.filter((id) => id !== created.canvasId);
  const parent = node(next.canvases[canvasId], nodeId);
  parent.child_canvas_id = created.canvasId;
  parent.modified_at = now;
  next.last_open = created.canvasId;
  return { library: next, canvasId: created.canvasId };
}

export function addTask(library, canvasId, edgeId, now = nowIso()) {
  if (edgeClosed(library, canvasId, edgeId)) return null;
  const next = clone(library);
  const line = findEdge(next, canvasId, edgeId);
  if (!line) return null;
  const taskId = newId();
  line.tasks = line.tasks || [];
  line.tasks.push({ id: taskId, text: "", done: false });
  stampEdge(next.canvases[canvasId], line, now);
  return { library: next, taskId };
}

export function setTaskText(library, canvasId, edgeId, taskId, text, now = nowIso()) {
  return changeTask(library, canvasId, edgeId, taskId, now, (task) => {
    if (task.text === text) return false;
    task.text = text;
    return true;
  });
}

export function setTaskDone(library, canvasId, edgeId, taskId, done, now = nowIso()) {
  return changeTask(library, canvasId, edgeId, taskId, now, (task) => {
    if (task.done === done) return false;
    task.done = done;
    return true;
  });
}

export function setTaskDeadline(library, canvasId, edgeId, taskId, deadline, now = nowIso()) {
  return changeTask(library, canvasId, edgeId, taskId, now, (task) => {
    const nextDeadline = deadline || null;
    if ((task.deadline || null) === nextDeadline) return false;
    if (nextDeadline) task.deadline = nextDeadline;
    else delete task.deadline;
    return true;
  });
}

export function removeTask(library, canvasId, edgeId, taskId, now = nowIso()) {
  if (edgeClosed(library, canvasId, edgeId)) return null;
  const canvas = library.canvases?.[canvasId];
  const line = canvas && (canvas.edges || []).find((item) => item.id === edgeId);
  if (!line || !(line.tasks || []).some((task) => task.id === taskId)) return null;
  const next = clone(library);
  const target = findEdge(next, canvasId, edgeId);
  target.tasks = (target.tasks || []).filter((task) => task.id !== taskId);
  stampEdge(next.canvases[canvasId], target, now);
  return { library: next };
}

export function addConstraint(library, canvasId, edgeId, now = nowIso()) {
  if (edgeClosed(library, canvasId, edgeId)) return null;
  const next = clone(library);
  const line = findEdge(next, canvasId, edgeId);
  if (!line) return null;
  const constraintId = newId();
  line.constraints = line.constraints || [];
  line.constraints.push({ id: constraintId, text: "" });
  stampEdge(next.canvases[canvasId], line, now);
  return { library: next, constraintId };
}

export function setConstraintText(library, canvasId, edgeId, constraintId, text, now = nowIso()) {
  if (edgeClosed(library, canvasId, edgeId)) return null;
  const canvas = library.canvases?.[canvasId];
  const line = canvas && (canvas.edges || []).find((item) => item.id === edgeId);
  const current = line && (line.constraints || []).find((item) => item.id === constraintId);
  if (!current || current.text === text) return current ? { library } : null;
  const next = clone(library);
  const target = findEdge(next, canvasId, edgeId);
  target.constraints.find((item) => item.id === constraintId).text = text;
  stampEdge(next.canvases[canvasId], target, now);
  return { library: next };
}

export function removeConstraint(library, canvasId, edgeId, constraintId, now = nowIso()) {
  if (edgeClosed(library, canvasId, edgeId)) return null;
  const canvas = library.canvases?.[canvasId];
  const line = canvas && (canvas.edges || []).find((item) => item.id === edgeId);
  if (!line || !(line.constraints || []).some((item) => item.id === constraintId)) return null;
  const next = clone(library);
  const target = findEdge(next, canvasId, edgeId);
  target.constraints = (target.constraints || []).filter((item) => item.id !== constraintId);
  stampEdge(next.canvases[canvasId], target, now);
  return { library: next };
}

export function setConstrainedPercent(library, canvasId, edgeId, percent, now = nowIso()) {
  if (edgeClosed(library, canvasId, edgeId)) return null;
  const canvas = library.canvases?.[canvasId];
  const line = canvas && (canvas.edges || []).find((item) => item.id === edgeId);
  if (!line) return null;
  const value = percent == null || percent === "" ? null : Math.max(0, Math.min(100, Number(percent)));
  if (!Number.isFinite(value) && value !== null) return null;
  if ((line.constrained_percent ?? null) === value) return { library };
  const next = clone(library);
  const target = findEdge(next, canvasId, edgeId);
  if (value == null) delete target.constrained_percent;
  else target.constrained_percent = value;
  stampEdge(next.canvases[canvasId], target, now);
  return { library: next };
}

export function rootCanvases(library) {
  const order = library?.root_order || [];
  return order
    .map((id) => library.canvases?.[id])
    .filter((canvas) => canvas && !canvas.parent_canvas_id);
}

function clone(library) {
  return structuredClone(library);
}

function node(canvas, id) {
  return (canvas.nodes || []).find((item) => item.id === id) || null;
}

function edge(from, to, kind) {
  return { id: newId(), from, to, kind };
}

function effectivelyClosed(canvas, nodeId) {
  let current = nodeId;
  const seen = new Set();
  while (current) {
    if (seen.has(current)) return false;
    seen.add(current);
    const item = node(canvas, current);
    if (!item) return false;
    if (item.closed) return true;
    current = item.parent_id;
  }
  return false;
}

function edgeClosed(library, canvasId, edgeId) {
  const canvas = library.canvases?.[canvasId];
  const line = canvas && (canvas.edges || []).find((item) => item.id === edgeId);
  if (!line) return true;
  return effectivelyClosed(canvas, line.from) || effectivelyClosed(canvas, line.to);
}

function findEdge(library, canvasId, edgeId) {
  return library.canvases[canvasId].edges.find((item) => item.id === edgeId);
}

function stampEdge(canvas, line, now) {
  for (const item of canvas.nodes) {
    if (item.id === line.from || item.id === line.to) item.modified_at = now;
  }
}

function changeTask(library, canvasId, edgeId, taskId, now, change) {
  if (edgeClosed(library, canvasId, edgeId)) return null;
  const canvas = library.canvases?.[canvasId];
  const line = canvas && (canvas.edges || []).find((item) => item.id === edgeId);
  const task = line && (line.tasks || []).find((item) => item.id === taskId);
  if (!task) return null;
  const next = clone(library);
  const target = findEdge(next, canvasId, edgeId);
  const copy = target.tasks.find((item) => item.id === taskId);
  if (!change(copy)) return { library };
  stampEdge(next.canvases[canvasId], target, now);
  return { library: next };
}

function promote(library, canvasId) {
  const canvas = library.canvases[canvasId];
  if (!canvas) return;
  canvas.parent_canvas_id = null;
  canvas.parent_node_id = null;
  if (!(library.root_order || []).includes(canvasId)) {
    library.root_order = [...(library.root_order || []), canvasId];
  }
}

function descendants(library, rootId) {
  const out = [rootId];
  let grew = true;
  while (grew && out.length < 10000) {
    grew = false;
    for (const canvas of Object.values(library.canvases)) {
      if (out.includes(canvas.id)) continue;
      if (canvas.parent_canvas_id && out.includes(canvas.parent_canvas_id)) {
        out.push(canvas.id);
        grew = true;
      }
    }
  }
  return out;
}
