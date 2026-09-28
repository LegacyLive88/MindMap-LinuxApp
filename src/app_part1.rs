use chrono::{DateTime, Local, Utc};
use eframe::egui::{
    self, Align, Align2, Button, Color32, CursorIcon, FontId, Frame, Id, Key, Layout, Margin,
    Modifiers, Pos2, Rect, RichText, Rounding, ScrollArea, Sense, Stroke, TextEdit, Vec2, vec2,
};
use mindmap::geom::{self, Layout as MapLayout, PlacedNode, Shape as NodeShape};
use mindmap::model::{label_for, staleness, EdgeKind, Id as NodeId, Library, Staleness};
use mindmap::store::{self, default_data_dir};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

const MIN_ZOOM: f32 = 0.28;
const MAX_ZOOM: f32 = 2.6;
const BODY: f32 = 15.0;
const RECT_WRAP: f32 = 200.0;
const CIRCLE_WRAP: f32 = 132.0;

const PAPER: Color32 = Color32::from_rgb(243, 238, 228);
const DOT: Color32 = Color32::from_rgb(214, 206, 190);
const INK: Color32 = Color32::from_rgb(36, 32, 29);
const MUTED: Color32 = Color32::from_rgb(120, 110, 98);
const SIDEBAR: Color32 = Color32::from_rgb(28, 25, 23);
const SIDEBAR_RAISED: Color32 = Color32::from_rgb(48, 42, 38);
const SIDEBAR_SELECTED: Color32 = Color32::from_rgb(64, 54, 46);
const CREAM: Color32 = Color32::from_rgb(247, 242, 234);
const CREAM_DIM: Color32 = Color32::from_rgb(186, 176, 162);
const CARD: Color32 = Color32::from_rgb(255, 252, 247);
const BORDER: Color32 = Color32::from_rgb(214, 205, 190);
const TERRACOTTA: Color32 = Color32::from_rgb(184, 84, 42);
const TEAL: Color32 = Color32::from_rgb(32, 96, 88);
const LINE: Color32 = Color32::from_rgb(92, 78, 64);
const CLOSED_FILL: Color32 = Color32::from_rgb(226, 220, 210);
const CLOSED_TEXT: Color32 = Color32::from_rgb(132, 124, 114);
const YELLOW: Color32 = Color32::from_rgb(228, 176, 46);
const ORANGE: Color32 = Color32::from_rgb(210, 116, 36);
const RED: Color32 = Color32::from_rgb(168, 52, 42);

#[derive(Clone, Debug, PartialEq, Eq)]
enum Screen {
    Welcome,
    Map(NodeId),
    Review,
}

#[derive(Clone, Copy)]
enum HistoryMode {
    Replace,
    Push,
    Keep,
}

#[derive(Clone, Copy)]
enum CamCmd {
    In,
    Out,
    Fit,
    Center,
}

#[derive(Clone)]
enum Gesture {
    None,
    Panning,
    Connecting { from: NodeId, origin: Pos2 },
}

struct EditState {
    canvas_id: NodeId,
    node_id: NodeId,
    buffer: String,
    original: String,
    owns_undo: bool,
    armed: bool,
    rect: Rect,
}

struct Camera {
    pan: Pos2,
    zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            pan: Pos2::ZERO,
            zoom: 1.0,
        }
    }
}

impl Camera {
    fn world_to_screen(&self, world: Pos2, viewport: Rect) -> Pos2 {
        viewport.center() + (world - self.pan) * self.zoom
    }

    fn screen_to_world(&self, screen: Pos2, viewport: Rect) -> Pos2 {
        self.pan + (screen - viewport.center()) / self.zoom
    }
}

struct Cache {
    canvas_id: NodeId,
    fingerprint: u64,
    layout: MapLayout,
}

pub struct MindMapApp {
    data_dir: std::path::PathBuf,
    library: Library,
    undo: Vec<Library>,
    screen: Screen,
    history: Vec<Screen>,
    selected: Option<NodeId>,
    editing: Option<EditState>,
    camera: Camera,
    pending_fit: bool,
    pending_focus: Option<NodeId>,
    gesture: Gesture,
    hover: Option<NodeId>,
    cache: Option<Cache>,
    status: Option<String>,
    confirm_delete: Option<Instant>,
    camera_cmd: Option<CamCmd>,
    block_shift_until: Option<Instant>,
}

impl MindMapApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_style(&cc.egui_ctx);
        let data_dir = default_data_dir();
        let loaded = store::load_library(&data_dir);
        let mut app = Self {
            data_dir,
            library: loaded.library,
            undo: Vec::new(),
            screen: Screen::Welcome,
            history: Vec::new(),
            selected: None,
            editing: None,
            camera: Camera::default(),
            pending_fit: false,
            pending_focus: None,
            gesture: Gesture::None,
            hover: None,
            cache: None,
            status: loaded.warning,
            confirm_delete: None,
            camera_cmd: None,
            block_shift_until: None,
        };
        app.restore_session();
        app.save();
        app
    }

    fn restore_session(&mut self) {
        let id = self
            .library
            .last_open
            .clone()
            .filter(|id| self.library.canvases.contains_key(id))
            .or_else(|| self.library.root_order.first().cloned());
        if let Some(id) = id {
            self.screen = Screen::Map(id.clone());
            self.history = vec![Screen::Map(id)];
            self.pending_fit = true;
        } else {
            self.screen = Screen::Welcome;
            self.history = vec![Screen::Welcome];
        }
    }

    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn save(&mut self) {
        match store::save_library(&self.data_dir, &self.library) {
            Ok(()) => {
                if self
                    .status
                    .as_ref()
                    .is_some_and(|status| status.starts_with("Could not save"))
                {
                    self.status = None;
                }
            }
            Err(error) => self.status = Some(format!("Could not save ({error})")),
        }
    }

    fn push_undo(&mut self) {
        self.undo.push(self.library.clone());
        if self.undo.len() > 40 {
            self.undo.remove(0);
        }
    }

    fn finish_edit(&mut self) {
        let Some(edit) = self.editing.take() else {
            return;
        };
        if edit.buffer == edit.original && edit.owns_undo {
            self.undo.pop();
        }
        self.save();
    }

    fn cancel_edit(&mut self) {
        self.editing = None;
        if let Some(previous) = self.undo.pop() {
            self.library = previous;
            self.ensure_screen();
            self.save();
        }
    }

    fn begin_edit(&mut self, canvas_id: &str, node_id: &str, owns_undo: bool) {
        if self
            .editing
            .as_ref()
            .is_some_and(|edit| edit.canvas_id == canvas_id && edit.node_id == node_id)
        {
            return;
        }
        self.finish_edit();
        let Some(canvas) = self.library.canvas(canvas_id) else {
            return;
        };
        if !canvas.is_center(node_id) && canvas.effectively_closed(node_id) {
            return;
        }
        let Some(node) = canvas.node(node_id) else {
            return;
        };
        let original = node.text.clone();
        if owns_undo {
            self.push_undo();
        }
        self.selected = Some(node_id.to_string());
        self.editing = Some(EditState {
            canvas_id: canvas_id.to_string(),
            node_id: node_id.to_string(),
            buffer: original.clone(),
            original,
            owns_undo,
            armed: false,
            rect: Rect::NOTHING,
        });
    }

    fn undo_action(&mut self) {
        if self.editing.is_some() {
            self.cancel_edit();
            return;
        }
        let Some(previous) = self.undo.pop() else {
            return;
        };
        self.library = previous;
        self.ensure_screen();
        self.save();
    }

    fn ensure_screen(&mut self) {
        self.history.retain(|item| match item {
            Screen::Map(id) => self.library.canvases.contains_key(id),
            _ => true,
        });
        let valid = match &self.screen {
            Screen::Map(id) => self.library.canvases.contains_key(id),
            _ => true,
        };
        if !valid {
            if let Some(id) = self
                .library
                .root_order
                .iter()
                .find(|id| self.library.canvases.contains_key(*id))
                .cloned()
            {
                self.screen = Screen::Map(id.clone());
                self.history = vec![Screen::Map(id)];
                self.pending_fit = true;
                self.pending_focus = None;
            } else {
                self.screen = Screen::Welcome;
                self.history = vec![Screen::Welcome];
            }
        } else if self.history.last() != Some(&self.screen) && self.history.is_empty() {
            self.history.push(self.screen.clone());
        }
        if let Some(selected) = &self.selected {
            let exists = match &self.screen {
                Screen::Map(canvas_id) => self
                    .library
                    .canvas(canvas_id)
                    .and_then(|canvas| canvas.node(selected))
                    .is_some(),
                _ => false,
            };
            if !exists {
                self.selected = None;
            }
        }
    }

    fn open_map_raw(&mut self, id: NodeId, focus: Option<NodeId>, mode: HistoryMode) {
        self.editing = None;
        self.confirm_delete = None;
        self.gesture = Gesture::None;
        self.selected = focus.clone();
        self.pending_focus = focus;
        self.pending_fit = self.pending_focus.is_none();
        self.library.last_open = Some(id.clone());
        self.screen = Screen::Map(id.clone());
        match mode {
            HistoryMode::Replace => self.history = vec![Screen::Map(id)],
            HistoryMode::Push => self.history.push(Screen::Map(id)),
            HistoryMode::Keep => {}
        }
    }

    fn open_root(&mut self, id: NodeId) {
        self.finish_edit();
        self.open_map_raw(id, None, HistoryMode::Replace);
        self.save();
    }

    fn create_canvas(&mut self) {
        self.finish_edit();
        self.push_undo();
        let now = self.now();
        let id = self.library.create_root("", now);
        let center = self.library.canvas(&id).unwrap().center_id.clone();
        self.open_map_raw(id.clone(), Some(center.clone()), HistoryMode::Replace);
        self.pending_fit = true;
        self.pending_focus = None;
        self.begin_edit(&id, &center, false);
        self.save();
    }

    fn dive(&mut self, canvas_id: &str, node_id: &str) {
        if self
            .block_shift_until
            .is_some_and(|until| Instant::now() < until)
        {
            return;
        }
        self.finish_edit();
        let existed = self
            .library
            .canvas(canvas_id)
            .and_then(|canvas| canvas.node(node_id))
            .and_then(|node| node.child_canvas_id.clone())
            .is_some_and(|id| self.library.canvases.contains_key(&id));
        if !existed {
            self.push_undo();
        }
        let now = self.now();
        let Some(child) = self.library.open_or_create_child_canvas(canvas_id, node_id, now) else {
            if !existed {
                self.undo.pop();
            }
            return;
        };
        self.block_shift_until = Some(Instant::now() + Duration::from_millis(450));
        self.open_map_raw(child, None, HistoryMode::Push);
        self.save();
    }

    fn add_child(&mut self, canvas_id: &str, parent_id: &str) {
        self.finish_edit();
        self.push_undo();
        let Some(id) = self.library.add_child_node(canvas_id, parent_id, self.now()) else {
            self.undo.pop();
            return;
        };
        self.pending_focus = Some(id.clone());
        self.pending_fit = false;
        self.begin_edit(canvas_id, &id, false);
        self.save();
    }

    fn add_link(&mut self, canvas_id: &str, a: &str, b: &str) {
        self.finish_edit();
        self.push_undo();
        if !self.library.add_link(canvas_id, a, b, self.now()) {
            self.undo.pop();
            return;
        }
        self.save();
    }

    fn toggle_closed(&mut self, canvas_id: &str, node_id: &str) {
        let Some(closed) = self
            .library
            .canvas(canvas_id)
            .and_then(|canvas| canvas.node(node_id))
            .map(|node| node.closed)
        else {
            return;
        };
        self.finish_edit();
        self.push_undo();
        if !self.library.set_closed(canvas_id, node_id, !closed, self.now()) {
            self.undo.pop();
            return;
        }
        self.save();
    }

    fn delete_node(&mut self, canvas_id: &str, node_id: &str) {
        self.finish_edit();
        self.push_undo();
        if !self.library.delete_node(canvas_id, node_id, self.now()) {
            self.undo.pop();
            return;
        }
        if self.selected.as_deref() == Some(node_id) {
            self.selected = None;
        }
        self.pending_fit = true;
        self.pending_focus = None;
        self.save();
    }

    fn delete_active_canvas(&mut self) {
        let Screen::Map(canvas_id) = self.screen.clone() else {
            return;
        };
        self.finish_edit();
        let parent = self
            .library
            .canvas(&canvas_id)
            .and_then(|canvas| canvas.parent_canvas_id.clone());
        let focus = self
            .library
            .canvas(&canvas_id)
            .and_then(|canvas| canvas.parent_node_id.clone());
        self.push_undo();
        if !self.library.delete_canvas(&canvas_id) {
            self.undo.pop();
            return;
        }
        self.confirm_delete = None;
        self.selected = None;
        if let Some(parent) = parent.filter(|id| self.library.canvases.contains_key(id)) {
            self.history = vec![Screen::Map(parent.clone())];
            self.open_map_raw(parent, focus, HistoryMode::Keep);
        } else if let Some(root) = self.library.root_order.first().cloned() {
            self.open_map_raw(root, None, HistoryMode::Replace);
        } else {
            self.screen = Screen::Welcome;
            self.history = vec![Screen::Welcome];
            self.library.last_open = None;
        }
        self.save();
    }

    fn go_back(&mut self) {
        self.finish_edit();
        if self.history.len() >= 2 {
            let leaving = self.history.pop().unwrap();
            match self.history.last().cloned() {
                Some(Screen::Map(id)) => {
                    let focus = match &leaving {
                        Screen::Map(child) => self.library.canvas(child).and_then(|canvas| {
                            if canvas.parent_canvas_id.as_deref() == Some(id.as_str()) {
                                canvas.parent_node_id.clone()
                            } else {
                                None
                            }
                        }),
                        _ => None,
                    };
                    self.open_map_raw(id, focus, HistoryMode::Keep);
                    self.save();
                }
                Some(Screen::Review) => {
                    self.screen = Screen::Review;
                    self.selected = None;
                    self.editing = None;
                }
                Some(Screen::Welcome) => {
                    self.screen = Screen::Welcome;
                    self.editing = None;
                }
                None => {}
            }
            return;
        }
        if let Screen::Map(id) = self.screen.clone() {
            if let Some(canvas) = self.library.canvas(&id) {
                if let Some(parent) = canvas.parent_canvas_id.clone() {
                    let focus = canvas.parent_node_id.clone();
                    self.history = vec![Screen::Map(parent.clone())];
                    self.open_map_raw(parent, focus, HistoryMode::Keep);
                    self.save();
                }
            }
        }
    }

    fn jump_to_crumb(&mut self, index: usize) {
        let Screen::Map(current) = self.screen.clone() else {
            return;
        };
        let crumbs = self.library.breadcrumb(&current);
        if index >= crumbs.len() {
            return;
        }
        self.finish_edit();
        let target = crumbs[index].0.clone();
        let focus = crumbs.get(index + 1).and_then(|(id, _)| {
            self.library
                .canvas(id)
                .and_then(|canvas| canvas.parent_node_id.clone())
        });
        self.history = crumbs[..=index]
            .iter()
            .map(|(id, _)| Screen::Map(id.clone()))
            .collect();
        self.open_map_raw(target, focus, HistoryMode::Keep);
        self.save();
    }

    fn open_review(&mut self) {
        if matches!(self.screen, Screen::Review) {
            return;
        }
        self.finish_edit();
        self.screen = Screen::Review;
        self.history.push(Screen::Review);
        self.selected = None;
    }

    fn can_go_back(&self) -> bool {
        if self.history.len() >= 2 {
            return true;
        }
        if let Screen::Map(id) = &self.screen {
            return self
                .library
                .canvas(id)
                .and_then(|canvas| canvas.parent_canvas_id.as_ref())
                .is_some();
        }
        false
    }

    fn layout_for(&mut self, ctx: &egui::Context, canvas_id: &str) -> MapLayout {
        let Some(canvas) = self.library.canvas(canvas_id).cloned() else {
            return MapLayout::default();
        };
        let fingerprint = fingerprint(&canvas);
        if let Some(cache) = &self.cache {
            if cache.canvas_id == canvas_id && cache.fingerprint == fingerprint {
                return cache.layout.clone();
            }
        }
        let mut sizes = std::collections::HashMap::new();
        for node in &canvas.nodes {
            sizes.insert(node.id.clone(), measure(ctx, &node.text, canvas.is_center(&node.id)));
        }
        let layout = geom::layout_canvas(&canvas, &sizes);
        self.cache = Some(Cache {
            canvas_id: canvas_id.to_string(),
            fingerprint,
            layout: layout.clone(),
        });
        layout
    }

    fn apply_pending(&mut self, layout: &MapLayout, viewport: Rect) {
        if let Some(id) = self.pending_focus.clone() {
            if layout.nodes.iter().any(|node| node.id == id) {
                self.focus_on(layout, &id);
                self.pending_focus = None;
                self.pending_fit = false;
                return;
            }
        }
        if self.pending_fit {
            self.fit(layout, viewport);
            self.pending_fit = false;
        }
    }

    fn apply_camera_cmd(&mut self, layout: &MapLayout, viewport: Rect) {
        let Some(cmd) = self.camera_cmd.take() else {
            return;
        };
        match cmd {
            CamCmd::In => self.zoom_at(viewport.center(), viewport, 1.12),
            CamCmd::Out => self.zoom_at(viewport.center(), viewport, 1.0 / 1.12),
            CamCmd::Fit => self.fit(layout, viewport),
            CamCmd::Center => self.center_circle(layout),
        }
    }

    fn zoom_at(&mut self, screen: Pos2, viewport: Rect, factor: f32) {
        let world = self.camera.screen_to_world(screen, viewport);
        let zoom = (self.camera.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.camera.zoom = zoom;
        self.camera.pan = world - (screen - viewport.center()) / zoom;
    }

    fn fit(&mut self, layout: &MapLayout, viewport: Rect) {
        if layout.nodes.is_empty() || viewport.width() < 2.0 || viewport.height() < 2.0 {
            return;
        }
        let mut bounds = layout.nodes[0].bounds();
        for node in &layout.nodes[1..] {
            bounds = bounds.union(node.bounds());
        }
        bounds = bounds.expand(56.0);
        let zoom = (viewport.width() / bounds.width().max(1.0))
            .min(viewport.height() / bounds.height().max(1.0))
            .clamp(MIN_ZOOM, MAX_ZOOM);
        self.camera.zoom = zoom;
        self.camera.pan = bounds.center();
    }

    fn focus_on(&mut self, layout: &MapLayout, id: &str) {
        if let Some(node) = layout.nodes.iter().find(|node| node.id == id) {
            self.camera.pan = node.center;
            if self.camera.zoom < 1.0 {
                self.camera.zoom = 1.0;
            }
            self.selected = Some(id.to_string());
        }
    }

    fn center_circle(&mut self, layout: &MapLayout) {
        if let Some(node) = layout.nodes.iter().find(|node| node.shape == NodeShape::Circle) {
            self.camera.pan = node.center;
        }
    }

    fn canvas_keys(&mut self, ctx: &egui::Context, layout: &MapLayout, viewport: Rect, canvas_id: &str) {
        let (zoom_in, zoom_out, fit, center, delete_key) = ctx.input_mut(|input| {
            (
                input.consume_key(Modifiers::NONE, Key::Plus)
                    || input.consume_key(Modifiers::SHIFT, Key::Equals),
                input.consume_key(Modifiers::NONE, Key::Minus),
                input.consume_key(Modifiers::NONE, Key::Equals),
                input.consume_key(Modifiers::NONE, Key::Space),
                input.consume_key(Modifiers::NONE, Key::Delete),
            )
        });
        ctx.input_mut(|input| {
            let _ = input.consume_key(Modifiers::NONE, Key::ArrowLeft);
            let _ = input.consume_key(Modifiers::NONE, Key::ArrowRight);
            let _ = input.consume_key(Modifiers::NONE, Key::ArrowUp);
            let _ = input.consume_key(Modifiers::NONE, Key::ArrowDown);
        });
        if zoom_in {
            self.zoom_at(viewport.center(), viewport, 1.12);
        }
        if zoom_out {
            self.zoom_at(viewport.center(), viewport, 1.0 / 1.12);
        }
        if fit {
            self.fit(layout, viewport);
        }
        if center {
            self.center_circle(layout);
        }
        if delete_key {
            if let Some(id) = self.selected.clone() {
                let is_center = self
                    .library
                    .canvas(canvas_id)
                    .is_some_and(|canvas| canvas.is_center(&id));
                if !is_center {
                    self.delete_node(canvas_id, &id);
                }
            }
        }
        let (left, right, up, down, dt) = ctx.input(|input| {
            (
                input.key_down(Key::ArrowLeft),
                input.key_down(Key::ArrowRight),
                input.key_down(Key::ArrowUp),
                input.key_down(Key::ArrowDown),
                input.stable_dt.min(0.05),
            )
        });
        if left || right || up || down {
            let step = 680.0 * dt / self.camera.zoom;
            if left {
                self.camera.pan.x -= step;
            }
            if right {
                self.camera.pan.x += step;
            }
            if up {
                self.camera.pan.y -= step;
            }
            if down {
                self.camera.pan.y += step;
            }
            ctx.request_repaint();
        }
    }

    fn handle_pointer(
        &mut self,
        ctx: &egui::Context,
        response: &egui::Response,
        layout: &MapLayout,
        viewport: Rect,
        canvas_id: &str,
    ) {
        let ctrl = ctx.input(|input| input.modifiers.ctrl);
        let shift = ctx.input(|input| input.modifiers.shift);
        self.hover = response.hover_pos().and_then(|pos| {
            hit_test(layout, self.camera.screen_to_world(pos, viewport))
        });

        if response.drag_started() {
            // A fast drag can cross the threshold in the same frame as the press.
            // The live pointer is already somewhere else; the press is where it began.
            let origin = ctx
                .input(|input| input.pointer.press_origin())
                .or_else(|| response.interact_pointer_pos())
                .unwrap_or(viewport.center());
            let world = self.camera.screen_to_world(origin, viewport);
            let hit = hit_test(layout, world);
            let can_connect = ctrl
                && hit.as_ref().is_some_and(|id| {
                    self.library.canvas(canvas_id).is_some_and(|canvas| {
                        !canvas.is_center(id) && !canvas.effectively_closed(id)
                    })
                });
            if can_connect {
                self.gesture = Gesture::Connecting {
                    from: hit.unwrap(),
                    origin,
                };
            } else {
                self.gesture = Gesture::Panning;
            }
        }

        if response.dragged() && matches!(self.gesture, Gesture::Panning) {
            self.camera.pan -= response.drag_delta() / self.camera.zoom;
        }

        let mut handled_drag = false;
        if response.drag_stopped() {
            if let Gesture::Connecting { from, origin } = self.gesture.clone() {
                let pointer = ctx.input(|input| input.pointer.interact_pos()).unwrap_or(origin);
                let hit = hit_test(layout, self.camera.screen_to_world(pointer, viewport));
                if let Some(target) = hit {
                    if target != from {
                        self.add_link(canvas_id, &from, &target);
                    }
                } else if origin.distance(pointer) < 16.0 {
                    self.add_child(canvas_id, &from);
                }
            }
            self.gesture = Gesture::None;
            handled_drag = true;
        }

        if response.clicked() && !handled_drag {
            let hit = response.interact_pointer_pos().and_then(|pos| {
                hit_test(layout, self.camera.screen_to_world(pos, viewport))
            });
            if ctrl {
                if let Some(id) = hit {
                    self.add_child(canvas_id, &id);
                }
            } else if shift {
                if let Some(id) = hit {
                    if !response.double_clicked() {
                        self.dive(canvas_id, &id);
                    }
                }
            } else if response.double_clicked() {
                if let Some(id) = hit {
                    self.selected = Some(id.clone());
                    self.begin_edit(canvas_id, &id, true);
                }
            } else {
                self.selected = hit;
            }
        }

        if response.hovered() {
            let scroll = ctx.input(|input| input.raw_scroll_delta.y);
            if scroll.abs() > 0.0 {
                if let Some(pointer) = ctx.input(|input| input.pointer.hover_pos()) {
                    self.zoom_at(pointer, viewport, (scroll * 0.00115).exp());
                }
            }
        }

        let cursor = if matches!(self.gesture, Gesture::Panning) && response.dragged() {
            CursorIcon::Grabbing
        } else if matches!(self.gesture, Gesture::Connecting { .. }) {
            CursorIcon::Crosshair
        } else if ctrl && self.hover.is_some() {
            CursorIcon::Copy
        } else if self.hover.is_some() {
            CursorIcon::PointingHand
        } else {
            CursorIcon::Grab
        };
        response.clone().on_hover_cursor(cursor);
    }
}

impl eframe::App for MindMapApp {
    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Match modifiers on the key event. The live modifier flags can already
        // be clear when the key-up arrives in the same frame.
        let undo = ctx.input_mut(|input| {
            let hit = input.events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::Key {
                        key: Key::Z,
                        pressed: true,
                        repeat: false,
                        modifiers,
                        ..
                    } if (modifiers.command || modifiers.ctrl) && !modifiers.shift
                )
            });
            if hit {
                let _ = input.consume_key(Modifiers::COMMAND, Key::Z);
            }
            hit
        });
        if undo {
            self.undo_action();
        }
        if self.editing.is_some() {
            let (escape, commit) = ctx.input_mut(|input| {
                (
                    input.consume_key(Modifiers::NONE, Key::Escape),
                    input.consume_key(Modifiers::COMMAND, Key::Enter)
                        || input.consume_key(Modifiers::CTRL, Key::Enter),
                )
            });
            if escape {
                self.cancel_edit();
            } else if commit {
                self.finish_edit();
            }
        }

        self.sidebar(ctx);
        match self.screen.clone() {
            Screen::Welcome => self.welcome(ctx),
            Screen::Review => self.review(ctx),
            Screen::Map(id) => self.map_screen(ctx, &id),
        }
    }
}

