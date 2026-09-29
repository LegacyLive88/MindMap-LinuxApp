//! Radial layout and obstacle-avoiding lines.
//!
//! Rectangles hang off the centre circle in wedges sized by how much is inside
//! them. A link pulls its two rectangles toward the same direction without
//! letting anything overlap. Lines stay straight when the gap is clear, and
//! bend around circles and rectangles when it is not.

use crate::model::{Canvas, EdgeKind, Id};
use egui::{Pos2, Rect, Vec2, vec2};
use std::collections::{HashMap, HashSet};

const GAP: f32 = 18.0;
const RING_CLEARANCE: f32 = 28.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Circle,
    Rect,
}

#[derive(Clone, Debug)]
pub struct PlacedNode {
    pub id: Id,
    pub center: Pos2,
    pub size: Vec2,
    pub shape: Shape,
}

#[derive(Clone, Debug)]
pub struct RoutedEdge {
    pub id: Id,
    pub from: Id,
    pub to: Id,
    pub kind: EdgeKind,
    pub points: Vec<Pos2>,
}

#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub nodes: Vec<PlacedNode>,
    pub routes: Vec<RoutedEdge>,
}

impl PlacedNode {
    pub fn bounds(&self) -> Rect {
        Rect::from_center_size(self.center, self.size)
    }

    pub fn contains(&self, point: Pos2) -> bool {
        match self.shape {
            Shape::Circle => self.center.distance(point) <= self.radius() + 4.0,
            Shape::Rect => self.bounds().expand(4.0).contains(point),
        }
    }

    pub fn radius(&self) -> f32 {
        self.size.x.min(self.size.y) * 0.5
    }
}

pub fn layout_canvas(canvas: &Canvas, sizes: &HashMap<Id, Vec2>) -> Layout {
    if canvas.nodes.is_empty() {
        return Layout::default();
    }
    let children = child_map(canvas);
    let root_id = if canvas.nodes.iter().any(|node| node.id == canvas.center_id) {
        canvas.center_id.clone()
    } else {
        canvas.nodes[0].id.clone()
    };
    let reachable = reachable_from(&root_id, &children);
    let mut placed = place_component(canvas, &root_id, &children, sizes);

    let mut orphan_roots: Vec<&crate::model::Node> = canvas
        .nodes
        .iter()
        .filter(|node| is_orphan_root(node, &reachable, canvas))
        .collect();
    orphan_roots.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));

    if !orphan_roots.is_empty() {
        let main_bounds = union_bounds(&placed);
        let mut cursor_x = main_bounds.min.x;
        let base_y = main_bounds.max.y + 72.0;
        for orphan in orphan_roots {
            let local = place_component(canvas, &orphan.id, &children, sizes);
            let bounds = union_bounds(&local);
            let shift = vec2(cursor_x - bounds.min.x, base_y - bounds.min.y);
            let mut local = local;
            for node in &mut local {
                node.center += shift;
            }
            cursor_x += bounds.width() + 56.0;
            placed.extend(local);
        }
    }

    let links: Vec<(Id, Id)> = canvas
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Link)
        .map(|edge| (edge.from.clone(), edge.to.clone()))
        .collect();
    relax(&mut placed, &links, &root_id);
    let routes = route_edges(&placed, canvas);
    Layout {
        nodes: placed,
        routes,
    }
}

fn child_map(canvas: &Canvas) -> HashMap<Id, Vec<Id>> {
    let ids: HashSet<&str> = canvas.nodes.iter().map(|node| node.id.as_str()).collect();
    let mut ordered: Vec<&crate::model::Node> = canvas.nodes.iter().collect();
    ordered.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
    let mut map: HashMap<Id, Vec<Id>> = HashMap::new();
    for node in ordered {
        if let Some(parent) = &node.parent_id {
            if parent != &node.id && ids.contains(parent.as_str()) {
                map.entry(parent.clone()).or_default().push(node.id.clone());
            }
        }
    }
    map
}

fn reachable_from(root: &str, children: &HashMap<Id, Vec<Id>>) -> HashSet<Id> {
    let mut seen = HashSet::new();
    let mut stack = vec![root.to_string()];
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        if let Some(kids) = children.get(&id) {
            stack.extend(kids.iter().cloned());
        }
    }
    seen
}

fn is_orphan_root(node: &crate::model::Node, reachable: &HashSet<Id>, canvas: &Canvas) -> bool {
    if reachable.contains(&node.id) {
        return false;
    }
    match &node.parent_id {
        None => true,
        Some(parent) if !canvas.nodes.iter().any(|other| &other.id == parent) => true,
        Some(parent) if reachable.contains(parent) => true,
        Some(_) => false,
    }
}

fn place_component(
    canvas: &Canvas,
    root_id: &str,
    children: &HashMap<Id, Vec<Id>>,
    sizes: &HashMap<Id, Vec2>,
) -> Vec<PlacedNode> {
    let mut angles: HashMap<Id, f32> = HashMap::new();
    let mut depths: HashMap<Id, usize> = HashMap::new();
    angles.insert(root_id.to_string(), 0.0);
    depths.insert(root_id.to_string(), 0);

    let weights = subtree_weights(root_id, children);
    let root_children = children.get(root_id).cloned().unwrap_or_default();
    if !root_children.is_empty() {
        let total = root_children
            .iter()
            .map(|id| weights.get(id).copied().unwrap_or(1.0))
            .sum::<f32>()
            .max(1.0);
        let mut cursor = -std::f32::consts::PI;
        let mut seen = HashSet::new();
        seen.insert(root_id.to_string());
        for child in &root_children {
            let span = std::f32::consts::TAU * weights.get(child).copied().unwrap_or(1.0) / total;
            assign(
                child,
                cursor,
                cursor + span,
                1,
                children,
                &weights,
                &mut angles,
                &mut depths,
                &mut seen,
            );
            cursor += span;
        }
    }

    let max_depth = depths.values().copied().max().unwrap_or(0);
    let mut radius_of_depth = vec![0.0; max_depth + 1];
    for depth in 1..=max_depth {
        let mut radius = radius_of_depth[depth - 1]
            + max_radial_extent(canvas, &depths, depth - 1, &angles, sizes)
            + max_radial_extent(canvas, &depths, depth, &angles, sizes)
            + RING_CLEARANCE;
        let mut group: Vec<(f32, Id)> = depths
            .iter()
            .filter(|(_, node_depth)| **node_depth == depth)
            .map(|(id, _)| (angles[id], id.clone()))
            .collect();
        group.sort_by(|a, b| a.0.total_cmp(&b.0));
        if group.len() >= 2 {
            for index in 0..group.len() {
                let (angle_a, id_a) = &group[index];
                let (angle_b, id_b) = &group[(index + 1) % group.len()];
                let mut delta = angle_b - angle_a;
                if delta <= 0.0 {
                    delta += std::f32::consts::TAU;
                }
                delta = delta.max(0.08);
                let size_a = node_size(canvas, id_a, sizes);
                let size_b = node_size(canvas, id_b, sizes);
                let needed = (size_a.x + size_b.x) * 0.5 + GAP;
                radius = radius.max(needed / delta);
            }
        }
        radius_of_depth[depth] = radius;
    }

    let mut placed = Vec::new();
    for (id, depth) in &depths {
        let shape = if canvas.is_center(id) {
            Shape::Circle
        } else {
            Shape::Rect
        };
        let size = node_size(canvas, id, sizes);
        let center = if *depth == 0 {
            Pos2::ZERO
        } else {
            let angle = angles[id];
            let radius = radius_of_depth[*depth];
            Pos2::new(angle.cos() * radius, angle.sin() * radius)
        };
        placed.push(PlacedNode {
            id: id.clone(),
            center,
            size,
            shape,
        });
    }
    placed
}

fn assign(
    id: &str,
    start: f32,
    end: f32,
    depth: usize,
    children: &HashMap<Id, Vec<Id>>,
    weights: &HashMap<Id, f32>,
    angles: &mut HashMap<Id, f32>,
    depths: &mut HashMap<Id, usize>,
    seen: &mut HashSet<Id>,
) {
    if !seen.insert(id.to_string()) {
        return;
    }
    angles.insert(id.to_string(), (start + end) * 0.5);
    depths.insert(id.to_string(), depth);
    let kids = children.get(id).cloned().unwrap_or_default();
    if kids.is_empty() {
        return;
    }
    let total = kids
        .iter()
        .map(|child| weights.get(child).copied().unwrap_or(1.0))
        .sum::<f32>()
        .max(1.0);
    let mut cursor = start;
    let span_all = end - start;
    for child in &kids {
        let span = span_all * weights.get(child).copied().unwrap_or(1.0) / total;
        assign(
            child,
            cursor,
            cursor + span,
            depth + 1,
            children,
            weights,
            angles,
            depths,
            seen,
        );
        cursor += span;
    }
}

fn subtree_weights(root: &str, children: &HashMap<Id, Vec<Id>>) -> HashMap<Id, f32> {
    let mut memo = HashMap::new();
    let mut stack = HashSet::new();
    weight_of(root, children, &mut memo, &mut stack);
    memo
}

fn weight_of(
    id: &str,
    children: &HashMap<Id, Vec<Id>>,
    memo: &mut HashMap<Id, f32>,
    stack: &mut HashSet<Id>,
) -> f32 {
    if let Some(weight) = memo.get(id) {
        return *weight;
    }
    if !stack.insert(id.to_string()) {
        return 1.0;
    }
    let mut weight = 1.0;
    if let Some(kids) = children.get(id) {
        for child in kids {
            weight += weight_of(child, children, memo, stack);
        }
    }
    stack.remove(id);
    memo.insert(id.to_string(), weight);
    weight
}

fn node_size(canvas: &Canvas, id: &str, sizes: &HashMap<Id, Vec2>) -> Vec2 {
    if let Some(size) = sizes.get(id) {
        if canvas.is_center(id) {
            let diameter = size.x.max(size.y).max(48.0);
            return vec2(diameter, diameter);
        }
        return vec2(size.x.max(48.0), size.y.max(32.0));
    }
    if canvas.is_center(id) {
        vec2(120.0, 120.0)
    } else {
        vec2(128.0, 48.0)
    }
}

fn max_radial_extent(
    canvas: &Canvas,
    depths: &HashMap<Id, usize>,
    depth: usize,
    angles: &HashMap<Id, f32>,
    sizes: &HashMap<Id, Vec2>,
) -> f32 {
    depths
        .iter()
        .filter(|(_, node_depth)| **node_depth == depth)
        .map(|(id, _)| {
            let size = node_size(canvas, id, sizes);
            let angle = angles.get(id).copied().unwrap_or(0.0);
            size.x * 0.5 * angle.cos().abs() + size.y * 0.5 * angle.sin().abs()
        })
        .fold(0.0, f32::max)
}

fn union_bounds(nodes: &[PlacedNode]) -> Rect {
    let mut bounds = nodes[0].bounds();
    for node in nodes.iter().skip(1) {
        bounds = bounds.union(node.bounds());
    }
    bounds
}

/// Nudge linked rectangles toward each other, then separate anything that overlaps.
fn relax(nodes: &mut [PlacedNode], links: &[(Id, Id)], root_id: &str) {
    for _ in 0..36 {
        pull_links(nodes, links, root_id);
        keep_outside_root(nodes, root_id);
        separate_overlaps(nodes, root_id);
    }
    separate_overlaps(nodes, root_id);
}

fn pull_links(nodes: &mut [PlacedNode], links: &[(Id, Id)], root_id: &str) {
    for (from_id, to_id) in links {
        let Some(from_index) = nodes.iter().position(|node| &node.id == from_id) else {
            continue;
        };
        let Some(to_index) = nodes.iter().position(|node| &node.id == to_id) else {
            continue;
        };
        nudge_angle(nodes, from_index, to_index, root_id);
        nudge_angle(nodes, to_index, from_index, root_id);
    }
}

fn nudge_angle(nodes: &mut [PlacedNode], index: usize, toward: usize, root_id: &str) {
    if nodes[index].id == root_id || nodes[index].center == Pos2::ZERO {
        return;
    }
    let here = nodes[index].center;
    let there = nodes[toward].center;
    if there == Pos2::ZERO {
        return;
    }
    let angle = here.y.atan2(here.x);
    let target = there.y.atan2(there.x);
    let delta = wrap_pi(target - angle);
    let step = delta.clamp(-0.045, 0.045);
    if step.abs() < 0.0001 {
        return;
    }
    let radius = here.distance(Pos2::ZERO);
    let next = angle + step;
    nodes[index].center = Pos2::new(next.cos() * radius, next.sin() * radius);
}

fn separate_overlaps(nodes: &mut [PlacedNode], root_id: &str) {
    let count = nodes.len();
    let mut delta = vec![Vec2::ZERO; count];
    for i in 0..count {
        for j in (i + 1)..count {
            let expanded_i = nodes[i].bounds().expand(GAP * 0.5);
            let expanded_j = nodes[j].bounds().expand(GAP * 0.5);
            let Some(push_j) = translation_to_separate(expanded_i, expanded_j) else {
                continue;
            };
            let i_fixed = nodes[i].id == root_id;
            let j_fixed = nodes[j].id == root_id;
            if i_fixed && j_fixed {
                continue;
            } else if i_fixed {
                delta[j] += push_j;
            } else if j_fixed {
                delta[i] -= push_j;
            } else {
                delta[i] -= push_j * 0.5;
                delta[j] += push_j * 0.5;
            }
        }
    }
    for (node, push) in nodes.iter_mut().zip(delta) {
        if node.id == root_id {
            continue;
        }
        let push = clamp_length(push, 72.0);
        node.center += push;
    }
}

fn keep_outside_root(nodes: &mut [PlacedNode], root_id: &str) {
    let Some(root) = nodes.iter().find(|node| node.id == root_id) else {
        return;
    };
    let root_radius = match root.shape {
        Shape::Circle => root.radius(),
        Shape::Rect => root.size.x.max(root.size.y) * 0.5,
    };
    let root_center = root.center;
    for node in nodes.iter_mut() {
        if node.id == root_id {
            continue;
        }
        let extent = node.size.x.max(node.size.y) * 0.5;
        let minimum = root_radius + extent + GAP;
        let offset = node.center - root_center;
        let distance = offset.length();
        if distance < minimum {
            let direction = if distance < 1.0 {
                vec2(1.0, 0.0)
            } else {
                offset / distance
            };
            node.center = root_center + direction * minimum;
        }
    }
}

fn translation_to_separate(a: Rect, b: Rect) -> Option<Vec2> {
    let overlap_x = a.max.x.min(b.max.x) - a.min.x.max(b.min.x);
    let overlap_y = a.max.y.min(b.max.y) - a.min.y.max(b.min.y);
    if overlap_x <= 0.0 || overlap_y <= 0.0 {
        return None;
    }
    if overlap_x < overlap_y {
        let sign = if b.center().x >= a.center().x { 1.0 } else { -1.0 };
        Some(vec2(sign * (overlap_x + 1.0), 0.0))
    } else {
        let sign = if b.center().y >= a.center().y { 1.0 } else { -1.0 };
        Some(vec2(0.0, sign * (overlap_y + 1.0)))
    }
}

fn route_edges(nodes: &[PlacedNode], canvas: &Canvas) -> Vec<RoutedEdge> {
    let mut edges: Vec<&crate::model::Edge> = canvas.edges.iter().collect();
    edges.sort_by(|a, b| {
        let rank = |kind: EdgeKind| match kind {
            EdgeKind::Tree => 0,
            EdgeKind::Link => 1,
        };
        rank(a.kind).cmp(&rank(b.kind)).then_with(|| {
            let da = endpoint_distance(nodes, &a.from, &a.to);
            let db = endpoint_distance(nodes, &b.from, &b.to);
            da.total_cmp(&db)
        })
    });
    let mut routed = Vec::new();
    let mut existing: Vec<Vec<Pos2>> = Vec::new();
    for edge in edges {
        let Some(from) = nodes.iter().find(|node| node.id == edge.from) else {
            continue;
        };
        let Some(to) = nodes.iter().find(|node| node.id == edge.to) else {
            continue;
        };
        let points = route_pair(from, to, nodes, &existing);
        existing.push(points.clone());
        routed.push(RoutedEdge {
            id: edge.id.clone(),
            from: edge.from.clone(),
            to: edge.to.clone(),
            kind: edge.kind,
            points,
        });
    }
    routed
}

fn endpoint_distance(nodes: &[PlacedNode], a: &str, b: &str) -> f32 {
    match (
        nodes.iter().find(|node| node.id == a),
        nodes.iter().find(|node| node.id == b),
    ) {
        (Some(a), Some(b)) => a.center.distance(b.center),
        _ => f32::MAX,
    }
}

pub fn route_pair(from: &PlacedNode, to: &PlacedNode, obstacles: &[PlacedNode], existing: &[Vec<Pos2>]) -> Vec<Pos2> {
    let start = boundary_point(from, to.center);
    let end = boundary_point(to, from.center);
    let direction = end - start;
    let length = direction.length().max(1.0);
    let outward = direction / length;
    let start = start + outward * 3.0;
    let end = end - outward * 3.0;

    let mut best: Option<(i32, f32, Vec<Pos2>)> = None;
    for candidate in candidates(start, end) {
        let hits = obstacle_hits(&candidate, obstacles, &from.id, &to.id);
        let crossings = line_crossings(&candidate, existing);
        let score = hits * 1_000_000 + crossings * 1_000;
        let length = polyline_length(&candidate);
        let better = match &best {
            None => true,
            Some((best_score, best_length, _)) => (score, length) < (*best_score, *best_length),
        };
        if better {
            best = Some((score, length, candidate));
        }
    }
    best.map(|(_, _, points)| points).unwrap_or_else(|| vec![start, end])
}

fn candidates(start: Pos2, end: Pos2) -> Vec<Vec<Pos2>> {
    let mut paths = vec![vec![start, end]];
    let offsets = [36.0, 64.0, 100.0, 150.0, 220.0, 320.0, 460.0];
    for offset in offsets {
        paths.push(quadratic(start, end, offset, 22));
        paths.push(quadratic(start, end, -offset, 22));
        paths.push(s_curve(start, end, offset, 22));
        paths.push(s_curve(start, end, -offset, 22));
    }
    paths
}

fn quadratic(start: Pos2, end: Pos2, offset: f32, steps: usize) -> Vec<Pos2> {
    let direction = end - start;
    let length = direction.length().max(1.0);
    let perp = vec2(-direction.y, direction.x) / length;
    let control = lerp(start, end, 0.5) + perp * offset;
    (0..=steps)
        .map(|step| {
            let t = step as f32 / steps as f32;
            let u = 1.0 - t;
            Pos2::new(
                u * u * start.x + 2.0 * u * t * control.x + t * t * end.x,
                u * u * start.y + 2.0 * u * t * control.y + t * t * end.y,
            )
        })
        .collect()
}

fn s_curve(start: Pos2, end: Pos2, offset: f32, steps: usize) -> Vec<Pos2> {
    let direction = end - start;
    let length = direction.length().max(1.0);
    let perp = vec2(-direction.y, direction.x) / length;
    let control_a = lerp(start, end, 0.33) + perp * offset;
    let control_b = lerp(start, end, 0.66) - perp * offset;
    (0..=steps)
        .map(|step| {
            let t = step as f32 / steps as f32;
            let u = 1.0 - t;
            Pos2::new(
                u * u * u * start.x
                    + 3.0 * u * u * t * control_a.x
                    + 3.0 * u * t * t * control_b.x
                    + t * t * t * end.x,
                u * u * u * start.y
                    + 3.0 * u * u * t * control_a.y
                    + 3.0 * u * t * t * control_b.y
                    + t * t * t * end.y,
            )
        })
        .collect()
}

fn obstacle_hits(points: &[Pos2], obstacles: &[PlacedNode], from: &str, to: &str) -> i32 {
    let mut hits = 0;
    for obstacle in obstacles {
        if obstacle.id == from || obstacle.id == to {
            continue;
        }
        for pair in points.windows(2) {
            if segment_hits(pair[0], pair[1], obstacle, 5.0) {
                hits += 1;
                break;
            }
        }
    }
    hits
}

fn segment_hits(a: Pos2, b: Pos2, obstacle: &PlacedNode, pad: f32) -> bool {
    match obstacle.shape {
        Shape::Circle => distance_to_segment(obstacle.center, a, b) <= obstacle.radius() + pad,
        Shape::Rect => segment_hits_rect(a, b, obstacle.bounds().expand(pad)),
    }
}

fn segment_hits_rect(a: Pos2, b: Pos2, rect: Rect) -> bool {
    if rect.contains(a) || rect.contains(b) {
        return true;
    }
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    (0..4).any(|index| segments_cross(a, b, corners[index], corners[(index + 1) % 4]))
}

fn line_crossings(points: &[Pos2], existing: &[Vec<Pos2>]) -> i32 {
    let mut crossings = 0;
    for other in existing {
        for left in points.windows(2) {
            for right in other.windows(2) {
                if segments_cross(left[0], left[1], right[0], right[1])
                    && !near_end(left[0], left[1], right[0], right[1])
                {
                    crossings += 1;
                }
            }
        }
    }
    crossings
}

fn near_end(a: Pos2, b: Pos2, c: Pos2, d: Pos2) -> bool {
    let ends = [a, b];
    let other = [c, d];
    ends.iter().any(|end| other.iter().any(|point| end.distance(*point) < 10.0))
}

fn segments_cross(a: Pos2, b: Pos2, c: Pos2, d: Pos2) -> bool {
    let ab = b - a;
    let cd = d - c;
    let d1 = cross(ab, c - a);
    let d2 = cross(ab, d - a);
    let d3 = cross(cd, a - c);
    let d4 = cross(cd, b - c);
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

fn distance_to_segment(point: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_sq();
    if len_sq < 1e-6 {
        return point.distance(a);
    }
    let t = ((point - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    point.distance(a + ab * t)
}

pub fn anchor_point(node: &PlacedNode, toward: Pos2) -> Pos2 {
    boundary_point(node, toward)
}

fn boundary_point(node: &PlacedNode, toward: Pos2) -> Pos2 {
    let direction = toward - node.center;
    let length = direction.length();
    if length < 1e-4 {
        return node.center;
    }
    let unit = direction / length;
    match node.shape {
        Shape::Circle => node.center + unit * node.radius(),
        Shape::Rect => {
            let half = node.size * 0.5;
            let tx = if unit.x.abs() < 1e-6 {
                f32::INFINITY
            } else {
                half.x / unit.x.abs()
            };
            let ty = if unit.y.abs() < 1e-6 {
                f32::INFINITY
            } else {
                half.y / unit.y.abs()
            };
            node.center + unit * tx.min(ty)
        }
    }
}

fn polyline_length(points: &[Pos2]) -> f32 {
    points.windows(2).map(|pair| pair[0].distance(pair[1])).sum()
}

fn lerp(a: Pos2, b: Pos2, t: f32) -> Pos2 {
    Pos2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

fn wrap_pi(mut angle: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let pi = std::f32::consts::PI;
    while angle > pi {
        angle -= tau;
    }
    while angle < -pi {
        angle += tau;
    }
    angle
}

fn clamp_length(value: Vec2, max: f32) -> Vec2 {
    let length = value.length();
    if length > max && length > 0.0 {
        value * (max / length)
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Library;
    use chrono::{Duration, TimeZone, Utc};

    fn overlaps(a: Rect, b: Rect) -> bool {
        let overlap_x = a.max.x.min(b.max.x) - a.min.x.max(b.min.x);
        let overlap_y = a.max.y.min(b.max.y) - a.min.y.max(b.min.y);
        overlap_x > 1.0 && overlap_y > 1.0
    }

    fn assert_separated(layout: &Layout) {
        for i in 0..layout.nodes.len() {
            for j in (i + 1)..layout.nodes.len() {
                let a = &layout.nodes[i];
                let b = &layout.nodes[j];
                assert!(
                    !overlaps(a.bounds(), b.bounds()),
                    "{} overlaps {} at {:?} and {:?}",
                    a.id,
                    b.id,
                    a.center,
                    b.center
                );
            }
        }
    }

    fn sized(canvas: &Canvas) -> HashMap<Id, Vec2> {
        canvas
            .nodes
            .iter()
            .map(|node| {
                let size = if canvas.is_center(&node.id) {
                    vec2(110.0, 110.0)
                } else {
                    vec2(130.0, 48.0)
                };
                (node.id.clone(), size)
            })
            .collect()
    }

    #[test]
    fn star_chain_and_unbalanced_trees_do_not_overlap() {
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let mut library = Library::new();
        let root = library.create_root("Centre", now);
        let center = library.canvas(&root).unwrap().center_id.clone();
        for _ in 0..8 {
            library.add_child_node(&root, &center, now).unwrap();
        }
        assert_separated(&layout_canvas(library.canvas(&root).unwrap(), &sized(library.canvas(&root).unwrap())));

        let mut library = Library::new();
        let root = library.create_root("Chain", now);
        let mut parent = library.canvas(&root).unwrap().center_id.clone();
        for step in 0..6 {
            parent = library
                .add_child_node(&root, &parent, now + Duration::seconds(step))
                .unwrap();
        }
        assert_separated(&layout_canvas(library.canvas(&root).unwrap(), &sized(library.canvas(&root).unwrap())));

        let mut library = Library::new();
        let root = library.create_root("Uneven", now);
        let center = library.canvas(&root).unwrap().center_id.clone();
        for branch in 0..4 {
            let child = library
                .add_child_node(&root, &center, now + Duration::seconds(branch))
                .unwrap();
            if branch == 0 {
                for leaf in 0..8 {
                    library
                        .add_child_node(&root, &child, now + Duration::seconds(10 + leaf))
                        .unwrap();
                }
            }
        }
        assert_separated(&layout_canvas(library.canvas(&root).unwrap(), &sized(library.canvas(&root).unwrap())));
    }

    #[test]
    fn orphans_sit_apart_from_the_main_tree() {
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let mut library = Library::new();
        let root = library.create_root("Map", now);
        let center = library.canvas(&root).unwrap().center_id.clone();
        let parent = library.add_child_node(&root, &center, now).unwrap();
        let child = library.add_child_node(&root, &parent, now).unwrap();
        assert!(library.delete_node(&root, &parent, now));
        let layout = layout_canvas(library.canvas(&root).unwrap(), &sized(library.canvas(&root).unwrap()));
        assert_separated(&layout);
        let child_pos = layout.nodes.iter().find(|node| node.id == child).unwrap().center;
        let center_pos = layout.nodes.iter().find(|node| node.id == center).unwrap().center;
        assert!(child_pos.y > center_pos.y + 40.0);
    }

    #[test]
    fn a_clear_line_stays_straight_and_a_blocked_line_goes_around() {
        let left = PlacedNode {
            id: "a".into(),
            center: Pos2::new(-220.0, 0.0),
            size: vec2(80.0, 40.0),
            shape: Shape::Rect,
        };
        let middle = PlacedNode {
            id: "b".into(),
            center: Pos2::ZERO,
            size: vec2(100.0, 70.0),
            shape: Shape::Rect,
        };
        let right = PlacedNode {
            id: "c".into(),
            center: Pos2::new(220.0, 0.0),
            size: vec2(80.0, 40.0),
            shape: Shape::Rect,
        };
        let around = route_pair(&left, &right, &[left.clone(), middle.clone(), right.clone()], &[]);
        for pair in around.windows(2) {
            assert!(
                !segment_hits(pair[0], pair[1], &middle, 2.0),
                "route crossed the middle node: {around:?}"
            );
        }
        assert!(polyline_length(&around) > left.center.distance(right.center) * 0.5);

        let clear = route_pair(&left, &right, &[left.clone(), right.clone()], &[]);
        assert_eq!(clear.len(), 2, "an open gap should stay a straight line");
    }

    #[test]
    fn tree_lines_do_not_cut_through_unrelated_rectangles() {
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let mut library = Library::new();
        let root = library.create_root("Map", now);
        let center = library.canvas(&root).unwrap().center_id.clone();
        for index in 0..6 {
            let child = library
                .add_child_node(&root, &center, now + Duration::seconds(index))
                .unwrap();
            library
                .add_child_node(&root, &child, now + Duration::seconds(20 + index))
                .unwrap();
        }
        let canvas = library.canvas(&root).unwrap();
        let layout = layout_canvas(canvas, &sized(canvas));
        for route in &layout.routes {
            for node in &layout.nodes {
                if node.id == route.from || node.id == route.to {
                    continue;
                }
                for pair in route.points.windows(2) {
                    assert!(
                        !segment_hits(pair[0], pair[1], node, 1.0),
                        "edge {} -> {} cut through {}",
                        route.from,
                        route.to,
                        node.id
                    );
                }
            }
        }
    }

    #[test]
    fn links_keep_rectangles_apart() {
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let mut library = Library::new();
        let root = library.create_root("Linked", now);
        let center = library.canvas(&root).unwrap().center_id.clone();
        let mut nodes = Vec::new();
        for index in 0..6 {
            nodes.push(
                library
                    .add_child_node(&root, &center, now + Duration::seconds(index))
                    .unwrap(),
            );
        }
        assert!(library.add_link(&root, &nodes[0], &nodes[3], now));
        assert!(library.add_link(&root, &nodes[1], &nodes[4], now));
        let canvas = library.canvas(&root).unwrap();
        assert_separated(&layout_canvas(canvas, &sized(canvas)));
    }
}
