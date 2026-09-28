impl MindMapApp {
    fn sidebar(&mut self, ctx: &egui::Context) {
        let mut create = false;
        let mut open: Option<NodeId> = None;
        let mut review = false;
        let roots: Vec<(NodeId, String, Staleness)> = self
            .library
            .roots()
            .into_iter()
            .map(|canvas| {
                let modified = self
                    .library
                    .collective_modified(&canvas.id, &canvas.center_id)
                    .unwrap_or_else(Utc::now);
                (
                    canvas.id.clone(),
                    label_for(&canvas.center_text()),
                    staleness(modified, self.now()),
                )
            })
            .collect();
        let stale = self.library.stale_circles(self.now());
        let worst = stale.first().map(|item| item.staleness);
        let selected = match &self.screen {
            Screen::Map(id) => Some(id.clone()),
            _ => None,
        };
        let status = self.status.clone();
        let data_dir = self.data_dir.display().to_string();
        let review_open = matches!(self.screen, Screen::Review);

        egui::SidePanel::left("nav")
            .exact_width(276.0)
            .resizable(false)
            .frame(
                Frame::none()
                    .fill(SIDEBAR)
                    .inner_margin(Margin::same(16.0)),
            )
            .show(ctx, |ui| {
                ui.style_mut().visuals.override_text_color = Some(CREAM);
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                    ui.painter().circle_filled(rect.center(), 8.0, TEAL);
                    ui.label(RichText::new("MindMap").size(20.0).strong().color(CREAM));
                });
                ui.label(
                    RichText::new("A circle at the centre. Maps inside maps.")
                        .size(12.0)
                        .color(CREAM_DIM),
                );
                ui.add_space(12.0);
                let new_canvas = Button::new(RichText::new("New canvas").color(CREAM).size(15.0))
                    .fill(TERRACOTTA)
                    .min_size(vec2(ui.available_width(), 36.0));
                if ui.add(new_canvas).clicked() {
                    create = true;
                }
                ui.add_space(14.0);
                ui.label(RichText::new("CANVASES").size(11.0).color(CREAM_DIM));
                ui.add_space(4.0);
                let footer = 248.0;
                let list_h = (ui.available_height() - footer).max(72.0);
                ScrollArea::vertical()
                    .max_height(list_h)
                    .show(ui, |ui| {
                        if roots.is_empty() {
                            ui.label(RichText::new("No canvases yet").color(CREAM_DIM).italics());
                        }
                        for (id, label, freshness) in &roots {
                            let is_selected = selected.as_deref() == Some(id.as_str());
                            if canvas_row(ui, label, *freshness, is_selected) {
                                open = Some(id.clone());
                            }
                        }
                    });
                ui.add_space(8.0);
                let attention = if stale.is_empty() {
                    "Needs attention".to_string()
                } else {
                    format!("Needs attention  ·  {}", stale.len())
                };
                let button = Button::new(RichText::new(attention).color(CREAM).size(14.0))
                    .fill(if review_open { SIDEBAR_SELECTED } else { SIDEBAR_RAISED })
                    .min_size(vec2(ui.available_width(), 36.0));
                ui.horizontal(|ui| {
                    if let Some(worst) = worst {
                        let (rect, _) = ui.allocate_exact_size(vec2(14.0, 36.0), Sense::hover());
                        ui.painter()
                            .circle_filled(rect.center(), 5.0, staleness_color(worst));
                    }
                    if ui.add(button).clicked() {
                        review = true;
                    }
                });
                ui.add_space(12.0);
                for line in [
                    "Ctrl+click    add a branch",
                    "Ctrl+drag     connect rectangles",
                    "Shift+click   nested map",
                    "Double-click  edit",
                    "Arrows        pan",
                    "Space         centre the circle",
                    "+  and  −     zoom",
                    "=             fit everything",
                    "Ctrl+Z        undo",
                ] {
                    ui.label(RichText::new(line).size(11.5).color(CREAM_DIM).monospace());
                }
                ui.add_space(8.0);
                if let Some(status) = status {
                    ui.label(RichText::new(status).size(11.0).color(YELLOW));
                }
                ui.label(
                    RichText::new(format!("Saved in {data_dir}"))
                        .size(10.5)
                        .color(CREAM_DIM),
                );
            });

        if create {
            self.create_canvas();
        } else if let Some(id) = open {
            self.open_root(id);
        } else if review {
            self.open_review();
        }
    }

    fn welcome(&mut self, ctx: &egui::Context) {
        let mut create = false;
        egui::CentralPanel::default()
            .frame(Frame::none().fill(PAPER).inner_margin(Margin::same(24.0)))
            .show(ctx, |ui| {
                ui.add_space((ui.available_height() * 0.22).min(180.0));
                let width = 480.0;
                let spare = (ui.available_width() - width).max(0.0) * 0.5;
                ui.horizontal(|ui| {
                    ui.add_space(spare);
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        ui.label(RichText::new("Start a canvas").size(32.0).strong().color(INK));
                        ui.add_space(8.0);
                        ui.label(RichText::new("Each canvas begins as one circle. Ctrl+click it to branch out. Shift+click any idea to open a map of its own, as deep as you like.").size(16.0).color(MUTED));
                        ui.add_space(18.0);
                        let button = Button::new(RichText::new("New canvas").color(CREAM).size(16.0))
                            .fill(TERRACOTTA)
                            .min_size(vec2(160.0, 40.0));
                        if ui.add(button).clicked() {
                            create = true;
                        }
                    });
                });
            });
        if create {
            self.create_canvas();
        }
    }

    fn review(&mut self, ctx: &egui::Context) {
        let items = self.library.stale_circles(self.now());
        let now = self.now();
        let mut open: Option<NodeId> = None;
        let mut back = false;
        let can_back = self.can_go_back();
        egui::CentralPanel::default()
            .frame(Frame::none().fill(PAPER).inner_margin(Margin::symmetric(28.0, 18.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if can_back && ui.button("←  Back").clicked() {
                        back = true;
                    }
                    ui.label(RichText::new("Needs attention").size(26.0).strong().color(INK));
                });
                ui.label(
                    RichText::new("Circles turn yellow after a week, orange after a month, and red after six months without a change to that idea or anything inside it.")
                        .color(MUTED),
                );
                ui.add_space(8.0);
                legend(ui);
                ui.add_space(12.0);
                if items.is_empty() {
                    ui.add_space(48.0);
                    ui.label(RichText::new("Everything has been touched in the last week.").size(18.0).color(INK));
                    return;
                }
                ScrollArea::vertical().show(ui, |ui| {
                    ui.set_max_width(760.0);
                    let mut current: Option<Staleness> = None;
                    for item in &items {
                        if current != Some(item.staleness) {
                            current = Some(item.staleness);
                            ui.add_space(10.0);
                            ui.label(
                                RichText::new(staleness_heading(item.staleness))
                                    .strong()
                                    .color(INK),
                            );
                        }
                        let title = label_for(&item.text);
                        let response = ui.add(
                            Button::new(
                                RichText::new(format!(
                                    "    {title}\n    {}   ·   {}",
                                    item.path,
                                    idle_phrase(item.modified, now)
                                ))
                                .color(INK),
                            )
                            .fill(CARD)
                            .stroke(Stroke::new(1.0, BORDER))
                            .min_size(vec2(ui.available_width(), 52.0)),
                        );
                        let dot = ui.painter().clip_rect();
                        let _ = dot;
                        if response.clicked() {
                            open = Some(item.canvas_id.clone());
                        }
                        paint_dot_on_button(ui, &response, staleness_color(item.staleness));
                    }
                });
            });
        if back {
            self.go_back();
        } else if let Some(id) = open {
            self.finish_edit();
            self.open_map_raw(id, None, HistoryMode::Push);
            self.save();
        }
    }

    fn map_screen(&mut self, ctx: &egui::Context, canvas_id: &str) {
        if self.library.canvas(canvas_id).is_none() {
            self.ensure_screen();
            return;
        }
        if self.editing.as_ref().is_some_and(|edit| edit.armed) {
            let release = ctx.input(|input| {
                input.events.iter().find_map(|event| match event {
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        ..
                    } => Some(*pos),
                    _ => None,
                })
            });
            if let Some(pos) = release {
                let rect = self.editing.as_ref().unwrap().rect;
                if !rect.contains(pos) {
                    self.finish_edit();
                }
            }
        }

        let mut navigated = false;
        let canvas_id = canvas_id.to_string();
        egui::CentralPanel::default()
            .frame(Frame::none().fill(PAPER).inner_margin(Margin::symmetric(12.0, 8.0)))
            .show(ctx, |ui| {
                navigated = self.toolbar(ui, &canvas_id);
                if navigated {
                    return;
                }
                ui.add_space(2.0);
                let hint = self.hint(ctx);
                ui.label(RichText::new(hint).size(12.0).color(MUTED));
                ui.add_space(4.0);
                let canvas_rect = ui.available_rect_before_wrap();
                let response = ui.allocate_rect(canvas_rect, Sense::click_and_drag());
                let mut layout = self.layout_for(ctx, &canvas_id);
                self.apply_pending(&layout, canvas_rect);
                self.apply_camera_cmd(&layout, canvas_rect);
                if self.editing.is_none() && !ctx.wants_keyboard_input() {
                    self.canvas_keys(ctx, &layout, canvas_rect, &canvas_id);
                }
                let screen_before = self.screen.clone();
                self.handle_pointer(ctx, &response, &layout, canvas_rect, &canvas_id);
                if self.screen != screen_before {
                    navigated = true;
                    return;
                }
                layout = self.layout_for(ctx, &canvas_id);
                self.apply_pending(&layout, canvas_rect);
                self.paint(ui, &layout, canvas_rect, &canvas_id);
                self.editor(ctx, &layout, canvas_rect);
                self.inspector(ctx, &canvas_id);
            });
        let _ = navigated;
    }

    fn hint(&self, ctx: &egui::Context) -> &'static str {
        let ctrl = ctx.input(|input| input.modifiers.ctrl);
        let shift = ctx.input(|input| input.modifiers.shift);
        if matches!(self.gesture, Gesture::Connecting { .. }) {
            "Release on another rectangle to connect it. A short drag in empty space adds a branch instead."
        } else if ctrl {
            "Ctrl+click the circle or a rectangle to add a branch."
        } else if shift {
            "Shift+click an idea to open a map with that wording in the centre."
        } else {
            "Ctrl+click adds a branch. Ctrl+drag connects two rectangles. Shift+click opens a nested map. Double-click edits."
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, canvas_id: &str) -> bool {
        let mut back = false;
        let mut jump: Option<usize> = None;
        let mut delete = false;
        let crumbs = self.library.breadcrumb(canvas_id);
        let can_back = self.can_go_back();
        if let Some(started) = self.confirm_delete {
            if started.elapsed() > Duration::from_secs(4) {
                self.confirm_delete = None;
            } else {
                ui.ctx().request_repaint_after(Duration::from_millis(200));
            }
        }
        let confirm = self.confirm_delete.is_some();
        ui.horizontal(|ui| {
            // Place the controls first so a long title cannot push them away.
            let toolbar_width = ui.available_width();
            ui.allocate_ui_with_layout(
                vec2(toolbar_width, 36.0),
                Layout::right_to_left(Align::Center),
                |ui| {
                    let delete_label = if confirm { "Confirm delete" } else { "Delete map" };
                    let delete_button = if confirm {
                        Button::new(RichText::new(delete_label).color(CREAM)).fill(RED)
                    } else {
                        Button::new(delete_label)
                    };
                    if ui.add(delete_button).clicked() {
                        if confirm {
                            delete = true;
                        } else {
                            self.confirm_delete = Some(Instant::now());
                        }
                    }
                    if ui.button("Fit").clicked() {
                        self.camera_cmd = Some(CamCmd::Fit);
                    }
                    if ui.button("+").clicked() {
                        self.camera_cmd = Some(CamCmd::In);
                    }
                    ui.label(
                        RichText::new(format!("{:.0}%", self.camera.zoom * 100.0))
                            .color(MUTED)
                            .size(13.0),
                    );
                    if ui.button("−").clicked() {
                        self.camera_cmd = Some(CamCmd::Out);
                    }
                    if ui.button("Center").clicked() {
                        self.camera_cmd = Some(CamCmd::Center);
                    }
                    let crumbs_width = ui.available_width();
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        ui.set_max_width(crumbs_width);
                        if can_back && ui.button("←  Back").clicked() {
                            back = true;
                        }
                        for (index, (_, text)) in crumbs.iter().enumerate() {
                            let label = ellipsize(&label_for(text), 22);
                            if index + 1 == crumbs.len() {
                                ui.label(RichText::new(label).strong().color(INK));
                            } else if ui.button(RichText::new(label).color(MUTED)).clicked() {
                                jump = Some(index);
                            }
                            if index + 1 != crumbs.len() {
                                ui.label(RichText::new("/").color(MUTED));
                            }
                        }
                    });
                },
            );
        });
        if back {
            self.go_back();
            return true;
        }
        if let Some(index) = jump {
            self.jump_to_crumb(index);
            return true;
        }
        if delete {
            self.delete_active_canvas();
            return true;
        }
        false
    }

    fn paint(&self, ui: &egui::Ui, layout: &MapLayout, viewport: Rect, canvas_id: &str) {
        let painter = ui.painter_at(viewport);
        paint_dots(&painter, &self.camera, viewport);
        let Some(canvas) = self.library.canvas(canvas_id) else {
            return;
        };
        let ctx = ui.ctx().clone();
        for route in &layout.routes {
            let closed = canvas.effectively_closed(&route.from) || canvas.effectively_closed(&route.to);
            let color = if closed {
                Color32::from_rgba_unmultiplied(120, 110, 98, 90)
            } else if route.kind == EdgeKind::Link {
                Color32::from_rgba_unmultiplied(92, 78, 64, 160)
            } else {
                LINE
            };
            let width = if route.kind == EdgeKind::Link { 1.35 } else { 1.8 };
            let points: Vec<Pos2> = route
                .points
                .iter()
                .map(|point| self.camera.world_to_screen(*point, viewport))
                .collect();
            if points.len() >= 2 {
                painter.add(egui::Shape::line(points, Stroke::new(width, color)));
            }
        }
        if let Gesture::Connecting { from, .. } = &self.gesture {
            if let Some(node) = layout.nodes.iter().find(|node| &node.id == from) {
                if let Some(pointer) = ui.input(|input| input.pointer.hover_pos()) {
                    let world = self.camera.screen_to_world(pointer, viewport);
                    let anchor = self
                        .camera
                        .world_to_screen(geom::anchor_point(node, world), viewport);
                    painter.line_segment([anchor, pointer], Stroke::new(1.8, TERRACOTTA));
                    painter.circle_stroke(pointer, 5.0, Stroke::new(1.5, TERRACOTTA));
                }
            }
        }
        let circle_first: Vec<&PlacedNode> = layout
            .nodes
            .iter()
            .filter(|node| node.shape == NodeShape::Circle)
            .collect();
        let rects: Vec<&PlacedNode> = layout
            .nodes
            .iter()
            .filter(|node| node.shape == NodeShape::Rect)
            .collect();
        for node in circle_first.into_iter().chain(rects) {
            self.paint_node(&painter, &ctx, canvas, node, viewport);
        }
        if canvas.nodes.len() == 1 {
            if let Some(circle) = layout.nodes.iter().find(|node| node.shape == NodeShape::Circle) {
                let pos = self.camera.world_to_screen(
                    circle.center + vec2(0.0, circle.radius() + 28.0),
                    viewport,
                );
                painter.text(
                    pos,
                    Align2::CENTER_TOP,
                    "Ctrl+click the circle to add a branch",
                    FontId::proportional(13.0),
                    MUTED,
                );
            }
        }
    }

    fn paint_node(
        &self,
        painter: &egui::Painter,
        ctx: &egui::Context,
        canvas: &mindmap::model::Canvas,
        node: &PlacedNode,
        viewport: Rect,
    ) {
        let rect = node_screen_rect(&self.camera, node, viewport);
        let selected = self.selected.as_deref() == Some(node.id.as_str());
        let hovered = self.hover.as_deref() == Some(node.id.as_str());
        let model = canvas.node(&node.id);
        let text = model.map(|item| item.text.as_str()).unwrap_or("");
        let closed = !canvas.is_center(&node.id) && canvas.effectively_closed(&node.id);
        let placeholder = text.trim().is_empty();
        match node.shape {
            NodeShape::Circle => {
                let modified = self
                    .library
                    .collective_modified(&canvas.id, &node.id)
                    .unwrap_or_else(Utc::now);
                let freshness = staleness(modified, self.now());
                let fill = staleness_color(freshness);
                let text_color = on_fill(freshness);
                let radius = rect.width() * 0.5;
                painter.circle_filled(rect.center() + vec2(0.0, 3.0), radius, Color32::from_black_alpha(24));
                painter.circle_filled(rect.center(), radius, fill);
                painter.circle_stroke(
                    rect.center(),
                    radius + if selected { 5.0 } else { 0.0 },
                    Stroke::new(if selected { 2.2 } else { 1.2 }, if selected { TERRACOTTA } else { Color32::from_white_alpha(50) }),
                );
                paint_label(painter, ctx, rect, text, text_color, true, self.camera.zoom);
            }
            NodeShape::Rect => {
                let fill = if closed { CLOSED_FILL } else { CARD };
                let text_color = if closed {
                    CLOSED_TEXT
                } else if placeholder {
                    MUTED
                } else {
                    INK
                };
                let rounding = Rounding::same((10.0 * self.camera.zoom).clamp(6.0, 14.0));
                painter.rect_filled(
                    rect.translate(vec2(0.0, 3.0)),
                    rounding,
                    Color32::from_black_alpha(if closed { 10 } else { 18 }),
                );
                painter.rect_filled(rect, rounding, fill);
                let stroke = if selected {
                    Stroke::new(2.0, TERRACOTTA)
                } else if hovered {
                    Stroke::new(1.4, Color32::from_rgb(168, 142, 112))
                } else {
                    Stroke::new(1.0, BORDER)
                };
                painter.rect_stroke(rect, rounding, stroke);
                paint_label(painter, ctx, rect, text, text_color, false, self.camera.zoom);
                if closed {
                    painter.text(
                        rect.right_top() + vec2(-8.0, 8.0),
                        Align2::RIGHT_TOP,
                        "closed",
                        FontId::proportional(10.0),
                        CLOSED_TEXT,
                    );
                }
            }
        }
        let ctrl = ctx.input(|input| input.modifiers.ctrl);
        if ctrl && hovered && !closed && !matches!(self.gesture, Gesture::Connecting { .. }) {
            let badge = rect.right_top() + vec2(2.0, -2.0);
            painter.circle_filled(badge, 9.0, TERRACOTTA);
            painter.text(badge, Align2::CENTER_CENTER, "+", FontId::proportional(14.0), CREAM);
        }
    }

    fn editor(&mut self, ctx: &egui::Context, layout: &MapLayout, viewport: Rect) {
        let Some(edit) = self.editing.as_ref() else {
            return;
        };
        if edit.canvas_id != match &self.screen {
            Screen::Map(id) => id.clone(),
            _ => return,
        } {
            return;
        }
        let Some(node) = layout.nodes.iter().find(|node| node.id == edit.node_id) else {
            return;
        };
        let screen = node_screen_rect(&self.camera, node, viewport).expand(6.0);
        let width = screen.width().max(200.0);
        let pos = Pos2::new(screen.center().x - width * 0.5, screen.top());
        let mut buffer = edit.buffer.clone();
        let node_id = edit.node_id.clone();
        let canvas_id = edit.canvas_id.clone();
        let edit_id = Id::new(("node-editor", node_id.clone()));
        // The area's first frame is only a size measurement, so a one-shot
        // focus request is easy to miss. Keep asking until the field has it.
        if !ctx.memory(|memory| memory.has_focus(edit_id)) {
            ctx.memory_mut(|memory| memory.request_focus(edit_id));
        }
        let mut card_rect = Rect::NOTHING;
        let area = egui::Area::new(Id::new("editor"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                let card = Frame::none()
                    .fill(CARD)
                    .rounding(Rounding::same(12.0))
                    .stroke(Stroke::new(2.0, TERRACOTTA))
                    .inner_margin(Margin::same(8.0))
                    .show(ui, |ui| {
                        ui.set_min_width(width);
                        ui.set_max_width(width + 24.0);
                        TextEdit::multiline(&mut buffer)
                            .id(edit_id)
                            .hint_text("New idea")
                            .desired_width(width)
                            .desired_rows(2)
                            .font(FontId::proportional(15.0))
                            .show(ui);
                    });
                card_rect = card.response.rect;
            });
        let changed = self
            .library
            .canvas(&canvas_id)
            .and_then(|canvas| canvas.node(&node_id))
            .is_some_and(|node| node.text != buffer);
        if changed {
            let _ = self.library.set_text(&canvas_id, &node_id, buffer.clone(), self.now());
            self.save();
        }
        if let Some(edit) = &mut self.editing {
            if edit.node_id == node_id {
                edit.buffer = buffer;
                let hit = if card_rect.width() > 1.0 && card_rect.height() > 1.0 {
                    card_rect
                } else {
                    area.response.rect
                };
                edit.rect = hit.expand(8.0);
                edit.armed = true;
            }
        }
    }

    fn inspector(&mut self, ctx: &egui::Context, canvas_id: &str) {
        if self.editing.is_some() {
            return;
        }
        let Some(node_id) = self.selected.clone() else {
            return;
        };
        let Some(canvas) = self.library.canvas(canvas_id) else {
            return;
        };
        let Some(node) = canvas.node(&node_id) else {
            return;
        };
        let is_center = canvas.is_center(&node_id);
        let own_closed = node.closed;
        let closed = !is_center && canvas.effectively_closed(&node_id);
        let blocked_by_parent = closed && !own_closed;
        let created = node.created_at;
        let own_modified = node.modified_at;
        let collective = self
            .library
            .collective_modified(canvas_id, &node_id)
            .unwrap_or(own_modified);
        let has_nested = node
            .child_canvas_id
            .as_ref()
            .is_some_and(|id| self.library.canvases.contains_key(id));
        let title = label_for(&node.text);
        let freshness = staleness(collective, self.now());
        let canvas_id = canvas_id.to_string();
        let mut action: Option<InspectorAction> = None;

        egui::Area::new(Id::new("inspector"))
            .order(egui::Order::Foreground)
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -14.0))
            .show(ctx, |ui| {
                Frame::popup(ui.style())
                    .fill(CARD)
                    .rounding(Rounding::same(14.0))
                    .inner_margin(Margin::same(14.0))
                    .show(ui, |ui| {
                        ui.set_max_width(420.0);
                        ui.set_min_width(320.0);
                        ui.label(RichText::new(ellipsize(&title, 80)).strong().size(16.0).color(INK));
                        ui.add_space(6.0);
                        egui::Grid::new("dates").num_columns(2).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
                            ui.label(RichText::new("Created").color(MUTED).size(12.0));
                            ui.label(RichText::new(format_time(created)).color(INK));
                            ui.end_row();
                            ui.label(RichText::new("Modified").color(MUTED).size(12.0));
                            ui.label(RichText::new(format_time(collective)).color(INK));
                            ui.end_row();
                        });
                        if collective != own_modified {
                            ui.label(
                                RichText::new("Includes changes inside this idea and its nested maps")
                                    .size(11.0)
                                    .color(MUTED)
                                    .italics(),
                            );
                        }
                        if is_center {
                            ui.horizontal(|ui| {
                                let (rect, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                                ui.painter().circle_filled(rect.center(), 5.5, staleness_color(freshness));
                                ui.label(RichText::new(idle_phrase(collective, self.now())).color(INK));
                            });
                        }
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if !closed && ui.button("Edit").clicked() {
                                action = Some(InspectorAction::Edit);
                            }
                            let nested_label = if has_nested { "Open nested map" } else { "Open as its own map" };
                            if (has_nested || !closed) && ui.button(nested_label).clicked() {
                                action = Some(InspectorAction::Dive);
                            }
                            if !is_center && !blocked_by_parent {
                                let label = if own_closed { "Open" } else { "Close" };
                                if ui.button(label).clicked() {
                                    action = Some(InspectorAction::Toggle);
                                }
                            }
                            if !is_center {
                                let delete = Button::new(RichText::new("Delete").color(CREAM)).fill(RED);
                                if ui.add(delete).clicked() {
                                    action = Some(InspectorAction::Delete);
                                }
                            }
                        });
                        if blocked_by_parent {
                            ui.label(RichText::new("Closed because a parent is closed").size(12.0).color(MUTED).italics());
                        }
                        ui.label(
                            RichText::new("Double-click to edit  ·  Ctrl+click to branch  ·  Shift+click to open inside")
                                .size(11.0)
                                .color(MUTED),
                        );
                    });
            });

        match action {
            Some(InspectorAction::Edit) => self.begin_edit(&canvas_id, &node_id, true),
            Some(InspectorAction::Dive) => self.dive(&canvas_id, &node_id),
            Some(InspectorAction::Toggle) => self.toggle_closed(&canvas_id, &node_id),
            Some(InspectorAction::Delete) => self.delete_node(&canvas_id, &node_id),
            None => {}
        }
    }
}

