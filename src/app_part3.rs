enum InspectorAction {
    Edit,
    Dive,
    Toggle,
    Delete,
}

fn apply_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals.panel_fill = PAPER;
    style.visuals.window_fill = CARD;
    style.visuals.extreme_bg_color = PAPER;
    style.visuals.faint_bg_color = Color32::from_rgb(236, 230, 218);
    style.visuals.override_text_color = Some(INK);
    style.visuals.widgets.noninteractive.fg_stroke.color = INK;
    let button_fill = Color32::from_rgb(255, 250, 244);
    let button_hover = Color32::from_rgb(244, 232, 220);
    for visuals in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        visuals.bg_fill = button_fill;
        visuals.weak_bg_fill = Color32::from_rgb(236, 230, 218);
        visuals.bg_stroke = Stroke::new(1.0, BORDER);
        visuals.fg_stroke.color = INK;
    }
    style.visuals.widgets.hovered.bg_fill = button_hover;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, TERRACOTTA);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(232, 214, 196);
    style.visuals.selection.bg_fill = Color32::from_rgb(232, 214, 196);
    style.visuals.selection.stroke.color = TERRACOTTA;
    style.visuals.window_rounding = Rounding::same(12.0);
    style.spacing.button_padding = vec2(10.0, 6.0);
    style.spacing.item_spacing = vec2(8.0, 6.0);
    style.text_styles.insert(egui::TextStyle::Body, FontId::proportional(15.0));
    style.text_styles.insert(egui::TextStyle::Button, FontId::proportional(14.0));
    style.text_styles.insert(egui::TextStyle::Small, FontId::proportional(12.0));
    style.text_styles.insert(egui::TextStyle::Heading, FontId::proportional(22.0));
    ctx.set_style(style);
}

fn measure(ctx: &egui::Context, text: &str, circle: bool) -> Vec2 {
    let wrap = if circle { CIRCLE_WRAP } else { RECT_WRAP };
    let galley = ctx.fonts(|fonts| {
        fonts.layout(label_for(text), FontId::proportional(BODY), INK, wrap)
    });
    if circle {
        let diameter = (galley.size().x.max(galley.size().y) + 40.0).max(112.0);
        vec2(diameter, diameter)
    } else {
        vec2((galley.size().x + 36.0).max(124.0), (galley.size().y + 22.0).max(46.0))
    }
}

fn paint_label(
    painter: &egui::Painter,
    ctx: &egui::Context,
    rect: Rect,
    text: &str,
    color: Color32,
    circle: bool,
    zoom: f32,
) {
    let zoom = zoom.clamp(0.55, 2.2);
    let wrap = if circle { CIRCLE_WRAP } else { RECT_WRAP };
    let galley = ctx.fonts(|fonts| {
        fonts.layout(
            label_for(text),
            FontId::proportional(BODY * zoom),
            color,
            wrap * zoom,
        )
    });
    let pos = rect.center() - galley.size() * 0.5;
    painter.galley(pos, galley, color);
}

fn paint_dots(painter: &egui::Painter, camera: &Camera, viewport: Rect) {
    let spacing = 48.0;
    let top_left = camera.screen_to_world(viewport.min, viewport);
    let bottom_right = camera.screen_to_world(viewport.max, viewport);
    let mut x = (top_left.x / spacing).floor() * spacing;
    let mut count = 0;
    while x < bottom_right.x && count < 4000 {
        let mut y = (top_left.y / spacing).floor() * spacing;
        while y < bottom_right.y && count < 4000 {
            let screen = camera.world_to_screen(Pos2::new(x, y), viewport);
            painter.circle_filled(screen, 1.15, DOT);
            y += spacing;
            count += 1;
        }
        x += spacing;
        count += 1;
    }
}

fn node_screen_rect(camera: &Camera, node: &PlacedNode, viewport: Rect) -> Rect {
    Rect::from_center_size(
        camera.world_to_screen(node.center, viewport),
        node.size * camera.zoom,
    )
}

fn heat_color(heat: DeadlineHeat) -> Color32 {
    match heat {
        DeadlineHeat::Red => RED,
        DeadlineHeat::Orange => ORANGE,
        DeadlineHeat::Yellow => YELLOW,
    }
}

fn parse_deadline(text: &str) -> Option<NaiveDate> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    for format in ["%Y-%m-%d", "%d/%m/%Y", "%d %b %Y", "%d %B %Y"] {
        if let Ok(date) = NaiveDate::parse_from_str(text, format) {
            return Some(date);
        }
    }
    None
}

fn breadcrumb_bar(ui: &mut egui::Ui, parts: &[(String, PlaceJump)]) -> Option<PlaceJump> {
    let mut chosen = None;
    ui.horizontal_wrapped(|ui| {
        for (index, (label, jump)) in parts.iter().enumerate() {
            if index > 0 {
                ui.label(RichText::new("/").color(MUTED));
            }
            let button = Button::new(RichText::new(ellipsize(label, 32)).color(TEAL))
                .fill(Color32::TRANSPARENT);
            if ui.add(button).clicked() {
                chosen = Some(jump.clone());
            }
        }
    });
    chosen
}

fn hit_test(layout: &MapLayout, world: Pos2) -> Option<NodeId> {
    for node in layout.nodes.iter().rev() {
        if node.shape == NodeShape::Rect && node.contains(world) {
            return Some(node.id.clone());
        }
    }
    layout.nodes.iter().find_map(|node| {
        (node.shape == NodeShape::Circle && node.contains(world)).then(|| node.id.clone())
    })
}

fn hit_test_edge(layout: &MapLayout, world: Pos2, zoom: f32) -> Option<NodeId> {
    let threshold = (10.0 / zoom).clamp(7.0, 22.0);
    let mut best: Option<(f32, NodeId)> = None;
    for route in &layout.routes {
        let Some(distance) = distance_to_polyline(world, &route.points) else {
            continue;
        };
        if distance > threshold {
            continue;
        }
        if best
            .as_ref()
            .is_none_or(|(best_distance, _)| distance < *best_distance)
        {
            best = Some((distance, route.id.clone()));
        }
    }
    best.map(|(_, id)| id)
}

fn distance_to_polyline(point: Pos2, points: &[Pos2]) -> Option<f32> {
    if points.len() < 2 {
        return None;
    }
    let mut best = f32::MAX;
    for pair in points.windows(2) {
        best = best.min(distance_to_segment(point, pair[0], pair[1]));
    }
    Some(best)
}

fn distance_to_segment(point: Pos2, start: Pos2, end: Pos2) -> f32 {
    let delta = end - start;
    let len2 = delta.length_sq();
    if len2 <= f32::EPSILON {
        return point.distance(start);
    }
    let t = ((point - start).dot(delta) / len2).clamp(0.0, 1.0);
    point.distance(start + delta * t)
}

fn polyline_midpoint(points: &[Pos2]) -> Option<Pos2> {
    if points.len() < 2 {
        return None;
    }
    let total = points
        .windows(2)
        .map(|pair| pair[0].distance(pair[1]))
        .sum::<f32>();
    if total <= f32::EPSILON {
        return Some(points[0]);
    }
    let mut walked = 0.0;
    let half = total * 0.5;
    for pair in points.windows(2) {
        let span = pair[0].distance(pair[1]);
        if walked + span >= half {
            let t = if span <= f32::EPSILON {
                0.0
            } else {
                (half - walked) / span
            };
            return Some(pair[0] + (pair[1] - pair[0]) * t);
        }
        walked += span;
    }
    points.last().copied()
}

fn paint_line_badges(
    painter: &egui::Painter,
    ctx: &egui::Context,
    center: Pos2,
    ratio: Option<(usize, usize)>,
    free: Option<u8>,
    zoom: f32,
) {
    let font = FontId::proportional((13.0 * zoom).clamp(11.0, 16.0));
    let mut labels = Vec::new();
    if let Some((done, total)) = ratio {
        labels.push(format!("{done}/{total}"));
    }
    if let Some(free) = free {
        labels.push(format!("{free}%"));
    }
    if labels.is_empty() {
        return;
    }
    let galleys: Vec<_> = labels
        .into_iter()
        .map(|text| ctx.fonts(|fonts| fonts.layout_no_wrap(text, font.clone(), INK)))
        .collect();
    let gap = 6.0;
    let pad = vec2(7.0, 3.0);
    let widths: Vec<f32> = galleys.iter().map(|galley| galley.size().x + pad.x * 2.0).collect();
    let height = galleys
        .iter()
        .map(|galley| galley.size().y + pad.y * 2.0)
        .fold(0.0, f32::max);
    let total_width = widths.iter().sum::<f32>() + gap * widths.len().saturating_sub(1) as f32;
    let mut x = center.x - total_width * 0.5;
    for (galley, width) in galleys.into_iter().zip(widths) {
        let rect = Rect::from_min_size(Pos2::new(x, center.y - height * 0.5), vec2(width, height));
        painter.rect_filled(rect, Rounding::same(5.0), line_badge());
        painter.rect_stroke(rect, Rounding::same(5.0), Stroke::new(1.0, BORDER));
        painter.galley(rect.min + pad, galley, INK);
        x += width + gap;
    }
}

fn fingerprint(canvas: &mindmap::model::Canvas) -> u64 {
    let mut hasher = DefaultHasher::new();
    canvas.id.hash(&mut hasher);
    for node in &canvas.nodes {
        node.id.hash(&mut hasher);
        node.text.hash(&mut hasher);
        node.parent_id.hash(&mut hasher);
    }
    for edge in &canvas.edges {
        edge.from.hash(&mut hasher);
        edge.to.hash(&mut hasher);
        edge.kind.hash(&mut hasher);
    }
    hasher.finish()
}

fn canvas_row(ui: &mut egui::Ui, label: &str, freshness: Staleness, selected: bool) -> bool {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 36.0), Sense::click());
    let fill = if selected {
        SIDEBAR_SELECTED
    } else if response.hovered() {
        SIDEBAR_RAISED
    } else {
        Color32::TRANSPARENT
    };
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, Rounding::same(8.0), fill);
    }
    ui.painter().circle_filled(
        rect.left_center() + vec2(14.0, 0.0),
        5.0,
        staleness_color(freshness),
    );
    ui.painter().text(
        rect.left_center() + vec2(28.0, 0.0),
        Align2::LEFT_CENTER,
        ellipsize(label, 26),
        FontId::proportional(14.0),
        CREAM,
    );
    response.clicked()
}

fn legend(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        for (color, label) in [
            (YELLOW, "1 week"),
            (ORANGE, "1 month"),
            (RED, "6 months"),
        ] {
            let (rect, _) = ui.allocate_exact_size(vec2(12.0, 16.0), Sense::hover());
            ui.painter().circle_filled(rect.center(), 5.0, color);
            ui.label(RichText::new(label).color(MUTED).size(12.0));
            ui.add_space(8.0);
        }
    });
}

fn paint_dot_on_button(ui: &egui::Ui, response: &egui::Response, color: Color32) {
    ui.painter().circle_filled(
        response.rect.left_center() + vec2(14.0, 0.0),
        5.0,
        color,
    );
}

fn staleness_color(freshness: Staleness) -> Color32 {
    match freshness {
        Staleness::Fresh => TEAL,
        Staleness::Yellow => YELLOW,
        Staleness::Orange => ORANGE,
        Staleness::Red => RED,
    }
}

fn on_fill(freshness: Staleness) -> Color32 {
    match freshness {
        Staleness::Fresh | Staleness::Red => CREAM,
        Staleness::Yellow | Staleness::Orange => INK,
    }
}

fn staleness_heading(freshness: Staleness) -> &'static str {
    match freshness {
        Staleness::Red => "Red · quiet for six months or more",
        Staleness::Orange => "Orange · quiet for a month or more",
        Staleness::Yellow => "Yellow · quiet for a week or more",
        Staleness::Fresh => "",
    }
}

fn idle_phrase(modified: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let age = now.signed_duration_since(modified);
    if age.num_seconds() < 60 {
        return "Active just now".to_string();
    }
    let days = age.num_days();
    if days >= 365 {
        let years = days / 365;
        return format!("Quiet for {years} year{}", plural(years));
    }
    if days >= 30 {
        let months = days / 30;
        return format!("Quiet for {months} month{}", plural(months));
    }
    if days >= 7 {
        let weeks = days / 7;
        return format!("Quiet for {weeks} week{}", plural(weeks));
    }
    if days >= 1 {
        return format!("Quiet for {days} day{}", plural(days));
    }
    let hours = age.num_hours().max(1);
    format!("Active {hours} hour{} ago", plural(hours))
}

fn plural(value: i64) -> &'static str {
    if value == 1 { "" } else { "s" }
}

fn format_time(time: DateTime<Utc>) -> String {
    time.with_timezone(&Local).format("%d %b %Y, %H:%M").to_string()
}

fn ellipsize(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (index, ch) in text.chars().enumerate() {
        if index + 1 == max_chars && text.chars().nth(index + 1).is_some() {
            out.push('…');
            break;
        }
        if index >= max_chars {
            break;
        }
        out.push(ch);
    }
    out
}
