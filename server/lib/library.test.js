import test from "node:test";
import assert from "node:assert/strict";
import {
  addChild,
  compactLibrary,
  createRoot,
  deleteNode,
  emptyLibrary,
  openNested,
  setClosed,
  setText,
} from "./library.js";

test("a new canvas is a centre circle and a child branches from it", () => {
  const created = createRoot(emptyLibrary(), "Garden", "2026-05-01T08:30:00.000Z");
  const canvas = created.library.canvases[created.canvasId];
  assert.equal(canvas.nodes.length, 1);
  assert.equal(canvas.nodes[0].text, "Garden");
  const branched = addChild(created.library, created.canvasId, canvas.center_id, "2026-05-01T08:31:00.000Z");
  assert.ok(branched);
  const next = branched.library.canvases[created.canvasId];
  assert.equal(next.nodes.length, 2);
  assert.equal(next.edges[0].kind, "tree");
  assert.equal(next.edges[0].from, canvas.center_id);
});

test("closing a rectangle blocks edits until it is opened", () => {
  const created = createRoot(emptyLibrary(), "Garden", "2026-05-01T08:30:00.000Z");
  const canvas = created.library.canvases[created.canvasId];
  const branched = addChild(created.library, created.canvasId, canvas.center_id);
  const closed = setClosed(branched.library, created.canvasId, branched.nodeId, true);
  assert.equal(setText(closed.library, created.canvasId, branched.nodeId, "Herb bed"), null);
  const opened = setClosed(closed.library, created.canvasId, branched.nodeId, false);
  const named = setText(opened.library, created.canvasId, branched.nodeId, "Herb bed");
  assert.equal(named.library.canvases[created.canvasId].nodes.find((node) => node.id === branched.nodeId).text, "Herb bed");
});

test("deleting a rectangle keeps a nested map as its own canvas", () => {
  const created = createRoot(emptyLibrary(), "Garden", "2026-05-01T08:30:00.000Z");
  const canvas = created.library.canvases[created.canvasId];
  const branched = addChild(created.library, created.canvasId, canvas.center_id);
  const named = setText(branched.library, created.canvasId, branched.nodeId, "Herb bed");
  const nested = openNested(named.library, created.canvasId, branched.nodeId);
  assert.ok(nested.library.canvases[nested.canvasId]);
  assert.equal(nested.library.root_order.includes(nested.canvasId), false);
  const removed = deleteNode(nested.library, created.canvasId, branched.nodeId);
  assert.equal(removed.library.canvases[created.canvasId].nodes.some((node) => node.id === branched.nodeId), false);
  assert.ok(removed.library.canvases[nested.canvasId]);
  assert.equal(removed.library.root_order.includes(nested.canvasId), true);
  assert.equal(removed.library.canvases[nested.canvasId].parent_canvas_id, null);
});

test("compact drops empty task lists the desktop program omits", () => {
  const library = emptyLibrary();
  library.canvases.demo = {
    id: "demo",
    center_id: "c",
    nodes: [],
    edges: [{ id: "e", from: "a", to: "b", kind: "link", tasks: [], constraints: [], constrained_percent: null }],
  };
  const compact = compactLibrary(library);
  assert.equal(compact.canvases.demo.edges[0].tasks, undefined);
  assert.equal(compact.canvases.demo.edges[0].constraints, undefined);
  assert.equal("constrained_percent" in compact.canvases.demo.edges[0], false);
});
