//! Canvases, nodes, links, staleness, and the edits the canvas can make.
//!
//! A canvas is a tree grown from a centre circle. Ctrl+click adds a child
//! rectangle. Ctrl+drag adds a non-hierarchical link. Shift+click opens that
//! idea as its own canvas. Closing a rectangle closes its descendants.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

pub type Id = String;

pub const LIBRARY_VERSION: u32 = 1;
pub const YELLOW_AFTER: Duration = Duration::days(7);
pub const ORANGE_AFTER: Duration = Duration::days(30);
pub const RED_AFTER: Duration = Duration::days(182);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeKind {
    /// Parent to child, created by Ctrl+click.
    Tree,
    /// Association between two rectangles, created by Ctrl+drag.
    Link,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Staleness {
    Red,
    Orange,
    Yellow,
    Fresh,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub id: Id,
    pub text: String,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    #[serde(default)]
    pub closed: bool,
    #[serde(default)]
    pub parent_id: Option<Id>,
    #[serde(default)]
    pub child_canvas_id: Option<Id>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub id: Id,
    pub from: Id,
    pub to: Id,
    pub kind: EdgeKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Canvas {
    pub id: Id,
    #[serde(default)]
    pub parent_canvas_id: Option<Id>,
    #[serde(default)]
    pub parent_node_id: Option<Id>,
    pub center_id: Id,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaleCircle {
    pub canvas_id: Id,
    pub text: String,
    pub path: String,
    pub modified: DateTime<Utc>,
    pub staleness: Staleness,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Library {
    pub version: u32,
    pub root_order: Vec<Id>,
    #[serde(default)]
    pub last_open: Option<Id>,
    pub canvases: BTreeMap<Id, Canvas>,
}

pub fn new_id() -> Id {
    uuid::Uuid::new_v4().to_string()
}

/// Text shown on a node when its stored wording is blank.
pub fn label_for(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        "New idea".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn staleness(modified: DateTime<Utc>, now: DateTime<Utc>) -> Staleness {
    let age = now.signed_duration_since(modified);
    if age >= RED_AFTER {
        Staleness::Red
    } else if age >= ORANGE_AFTER {
        Staleness::Orange
    } else if age >= YELLOW_AFTER {
        Staleness::Yellow
    } else {
        Staleness::Fresh
    }
}

impl Canvas {
    pub fn new_root(text: &str, now: DateTime<Utc>) -> Self {
        let center_id = new_id();
        Self {
            id: new_id(),
            parent_canvas_id: None,
            parent_node_id: None,
            center_id: center_id.clone(),
            nodes: vec![Node {
                id: center_id,
                text: text.to_string(),
                created_at: now,
                modified_at: now,
                closed: false,
                parent_id: None,
                child_canvas_id: None,
            }],
            edges: Vec::new(),
        }
    }

    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn node_mut(&mut self, id: &str) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|node| node.id == id)
    }

    pub fn is_center(&self, id: &str) -> bool {
        self.center_id == id
    }

    pub fn center_text(&self) -> String {
        self.node(&self.center_id)
            .map(|node| node.text.clone())
            .unwrap_or_default()
    }

    /// A node is closed when it, or any ancestor, has been closed.
    pub fn effectively_closed(&self, node_id: &str) -> bool {
        let mut current = Some(node_id.to_string());
        let mut seen = HashSet::new();
        while let Some(id) = current {
            if !seen.insert(id.clone()) {
                return false;
            }
            let Some(node) = self.node(&id) else {
                return false;
            };
            if node.closed {
                return true;
            }
            current = node.parent_id.clone();
        }
        false
    }
}

impl Default for Library {
    fn default() -> Self {
        Self::new()
    }
}

impl Library {
    pub fn new() -> Self {
        Self {
            version: LIBRARY_VERSION,
            root_order: Vec::new(),
            last_open: None,
            canvases: BTreeMap::new(),
        }
    }

    pub fn canvas(&self, id: &str) -> Option<&Canvas> {
        self.canvases.get(id)
    }

    pub fn roots(&self) -> Vec<&Canvas> {
        self.root_order
            .iter()
            .filter_map(|id| self.canvases.get(id))
            .filter(|canvas| canvas.parent_canvas_id.is_none())
            .collect()
    }

    pub fn create_root(&mut self, text: &str, now: DateTime<Utc>) -> Id {
        let canvas = Canvas::new_root(text, now);
        let id = canvas.id.clone();
        self.canvases.insert(id.clone(), canvas);
        self.root_order.push(id.clone());
        self.last_open = Some(id.clone());
        id
    }

    pub fn add_child_node(&mut self, canvas_id: &str, parent_id: &str, now: DateTime<Utc>) -> Option<Id> {
        {
            let canvas = self.canvases.get(canvas_id)?;
            if canvas.node(parent_id).is_none() || canvas.effectively_closed(parent_id) {
                return None;
            }
        }
        let node_id = new_id();
        let canvas = self.canvases.get_mut(canvas_id)?;
        canvas.nodes.push(Node {
            id: node_id.clone(),
            text: String::new(),
            created_at: now,
            modified_at: now,
            closed: false,
            parent_id: Some(parent_id.to_string()),
            child_canvas_id: None,
        });
        canvas.edges.push(Edge {
            id: new_id(),
            from: parent_id.to_string(),
            to: node_id.clone(),
            kind: EdgeKind::Tree,
        });
        if let Some(parent) = canvas.node_mut(parent_id) {
            parent.modified_at = now;
        }
        Some(node_id)
    }

    pub fn add_link(&mut self, canvas_id: &str, a: &str, b: &str, now: DateTime<Utc>) -> bool {
        if a == b {
            return false;
        }
        {
            let Some(canvas) = self.canvases.get(canvas_id) else {
                return false;
            };
            if canvas.is_center(a) || canvas.is_center(b) {
                return false;
            }
            if canvas.node(a).is_none() || canvas.node(b).is_none() {
                return false;
            }
            if canvas.effectively_closed(a) || canvas.effectively_closed(b) {
                return false;
            }
            let already = canvas.edges.iter().any(|edge| {
                (edge.from == a && edge.to == b) || (edge.from == b && edge.to == a)
            });
            if already {
                return false;
            }
        }
        let Some(canvas) = self.canvases.get_mut(canvas_id) else {
            return false;
        };
        canvas.edges.push(Edge {
            id: new_id(),
            from: a.to_string(),
            to: b.to_string(),
            kind: EdgeKind::Link,
        });
        if let Some(node) = canvas.node_mut(a) {
            node.modified_at = now;
        }
        if let Some(node) = canvas.node_mut(b) {
            node.modified_at = now;
        }
        true
    }

    pub fn set_text(&mut self, canvas_id: &str, node_id: &str, text: String, now: DateTime<Utc>) -> bool {
        let blocked = {
            let Some(canvas) = self.canvases.get(canvas_id) else {
                return false;
            };
            if canvas.node(node_id).is_none() {
                return false;
            }
            !canvas.is_center(node_id) && canvas.effectively_closed(node_id)
        };
        if blocked {
            return false;
        }
        let Some(canvas) = self.canvases.get_mut(canvas_id) else {
            return false;
        };
        let Some(node) = canvas.node_mut(node_id) else {
            return false;
        };
        if node.text == text {
            return true;
        }
        node.text = text;
        node.modified_at = now;
        true
    }

    /// Close or open a rectangle. The centre circle cannot be closed.
    /// Opening clears this node's own flag; a closed ancestor still keeps it shut.
    pub fn set_closed(&mut self, canvas_id: &str, node_id: &str, closed: bool, now: DateTime<Utc>) -> bool {
        {
            let Some(canvas) = self.canvases.get(canvas_id) else {
                return false;
            };
            if canvas.is_center(node_id) || canvas.node(node_id).is_none() {
                return false;
            }
        }
        let Some(canvas) = self.canvases.get_mut(canvas_id) else {
            return false;
        };
        let Some(node) = canvas.node_mut(node_id) else {
            return false;
        };
        node.closed = closed;
        node.modified_at = now;
        true
    }

    /// Remove a rectangle and every line that touches it.
    /// Its child rectangles stay, disconnected, and are laid out on their own.
    /// A nested canvas that lived on the deleted rectangle becomes a root canvas
    /// so the work is not stranded.
    pub fn delete_node(&mut self, canvas_id: &str, node_id: &str, now: DateTime<Utc>) -> bool {
        let (parent_id, child_canvas) = {
            let Some(canvas) = self.canvases.get(canvas_id) else {
                return false;
            };
            if canvas.is_center(node_id) {
                return false;
            }
            let Some(node) = canvas.node(node_id) else {
                return false;
            };
            (node.parent_id.clone(), node.child_canvas_id.clone())
        };
        if let Some(child_id) = child_canvas {
            self.promote_to_root(&child_id);
        }
        let Some(canvas) = self.canvases.get_mut(canvas_id) else {
            return false;
        };
        for node in &mut canvas.nodes {
            if node.parent_id.as_deref() == Some(node_id) {
                node.parent_id = None;
            }
        }
        canvas.nodes.retain(|node| node.id != node_id);
        canvas.edges.retain(|edge| edge.from != node_id && edge.to != node_id);
        if let Some(parent_id) = parent_id {
            if let Some(parent) = canvas.node_mut(&parent_id) {
                parent.modified_at = now;
            }
        }
        true
    }

    pub fn delete_canvas(&mut self, canvas_id: &str) -> bool {
        if !self.canvases.contains_key(canvas_id) {
            return false;
        }
        let ids = self.canvas_and_descendants(canvas_id);
        let id_set: HashSet<&str> = ids.iter().map(String::as_str).collect();
        for canvas in self.canvases.values_mut() {
            for node in &mut canvas.nodes {
                if node
                    .child_canvas_id
                    .as_ref()
                    .is_some_and(|id| id_set.contains(id.as_str()))
                {
                    node.child_canvas_id = None;
                }
            }
        }
        for id in &ids {
            self.canvases.remove(id);
            self.root_order.retain(|root| root != id);
        }
        if self
            .last_open
            .as_ref()
            .is_some_and(|id| id_set.contains(id.as_str()))
        {
            self.last_open = self.root_order.first().cloned();
        }
        true
    }

    /// Open the nested canvas for an element, creating it with that wording the first time.
    pub fn open_or_create_child_canvas(
        &mut self,
        canvas_id: &str,
        node_id: &str,
        now: DateTime<Utc>,
    ) -> Option<Id> {
        let (existing, text, closed) = {
            let canvas = self.canvases.get(canvas_id)?;
            let node = canvas.node(node_id)?;
            (
                node.child_canvas_id.clone(),
                node.text.clone(),
                !canvas.is_center(node_id) && canvas.effectively_closed(node_id),
            )
        };
        if let Some(child_id) = existing {
            if self.canvases.contains_key(&child_id) {
                return Some(child_id);
            }
        }
        if closed {
            return None;
        }
        let mut child = Canvas::new_root(text.trim(), now);
        let child_id = child.id.clone();
        child.parent_canvas_id = Some(canvas_id.to_string());
        child.parent_node_id = Some(node_id.to_string());
        self.canvases.insert(child_id.clone(), child);
        let canvas = self.canvases.get_mut(canvas_id)?;
        let node = canvas.node_mut(node_id)?;
        node.child_canvas_id = Some(child_id.clone());
        node.modified_at = now;
        Some(child_id)
    }

    /// Modified date of a node together with every descendant, including nested canvases.
    pub fn collective_modified(&self, canvas_id: &str, node_id: &str) -> Option<DateTime<Utc>> {
        let mut seen = HashSet::new();
        self.collective_modified_inner(canvas_id, node_id, &mut seen)
    }

    fn collective_modified_inner(
        &self,
        canvas_id: &str,
        node_id: &str,
        seen: &mut HashSet<(Id, Id)>,
    ) -> Option<DateTime<Utc>> {
        if !seen.insert((canvas_id.to_string(), node_id.to_string())) {
            return None;
        }
        let canvas = self.canvases.get(canvas_id)?;
        let node = canvas.node(node_id)?;
        let mut latest = node.modified_at;
        let child_ids: Vec<Id> = canvas
            .nodes
            .iter()
            .filter(|child| child.parent_id.as_deref() == Some(node_id))
            .map(|child| child.id.clone())
            .collect();
        for child_id in child_ids {
            if let Some(time) = self.collective_modified_inner(canvas_id, &child_id, seen) {
                if time > latest {
                    latest = time;
                }
            }
        }
        if let Some(nested_id) = node.child_canvas_id.clone() {
            if let Some(nested) = self.canvases.get(&nested_id) {
                let center_id = nested.center_id.clone();
                if let Some(time) = self.collective_modified_inner(&nested_id, &center_id, seen) {
                    if time > latest {
                        latest = time;
                    }
                }
            }
        }
        Some(latest)
    }

    pub fn breadcrumb(&self, canvas_id: &str) -> Vec<(Id, String)> {
        let mut chain = Vec::new();
        let mut current = Some(canvas_id.to_string());
        let mut seen = HashSet::new();
        while let Some(id) = current {
            if !seen.insert(id.clone()) {
                break;
            }
            let Some(canvas) = self.canvases.get(&id) else {
                break;
            };
            chain.push((canvas.id.clone(), canvas.center_text()));
            current = canvas.parent_canvas_id.clone();
        }
        chain.reverse();
        chain
    }

    pub fn stale_circles(&self, now: DateTime<Utc>) -> Vec<StaleCircle> {
        let mut items = Vec::new();
        for canvas in self.canvases.values() {
            let Some(modified) = self.collective_modified(&canvas.id, &canvas.center_id) else {
                continue;
            };
            let staleness = staleness(modified, now);
            if staleness == Staleness::Fresh {
                continue;
            }
            let path = self
                .breadcrumb(&canvas.id)
                .iter()
                .map(|(_, text)| label_for(text))
                .collect::<Vec<_>>()
                .join("  /  ");
            items.push(StaleCircle {
                canvas_id: canvas.id.clone(),
                text: canvas.center_text(),
                path,
                modified,
                staleness,
            });
        }
        items.sort_by(|a, b| a.staleness.cmp(&b.staleness).then(a.modified.cmp(&b.modified)));
        items
    }

    pub fn repair(&mut self) {
        let ids: Vec<Id> = self.canvases.keys().cloned().collect();
        for id in ids {
            self.repair_canvas(&id);
        }
        let promote: Vec<Id> = self
            .canvases
            .values()
            .filter(|canvas| {
                canvas
                    .parent_canvas_id
                    .as_ref()
                    .is_some_and(|parent| !self.canvases.contains_key(parent))
            })
            .map(|canvas| canvas.id.clone())
            .collect();
        for id in promote {
            self.promote_to_root(&id);
        }
        self.root_order.retain(|id| {
            self.canvases
                .get(id)
                .is_some_and(|canvas| canvas.parent_canvas_id.is_none())
        });
        let extras: Vec<Id> = self
            .canvases
            .values()
            .filter(|canvas| canvas.parent_canvas_id.is_none() && !self.root_order.contains(&canvas.id))
            .map(|canvas| canvas.id.clone())
            .collect();
        self.root_order.extend(extras);
        if self
            .last_open
            .as_ref()
            .is_some_and(|id| !self.canvases.contains_key(id))
        {
            self.last_open = None;
        }
    }

    fn promote_to_root(&mut self, canvas_id: &str) {
        let Some(canvas) = self.canvases.get_mut(canvas_id) else {
            return;
        };
        canvas.parent_canvas_id = None;
        canvas.parent_node_id = None;
        let id = canvas.id.clone();
        if !self.root_order.contains(&id) {
            self.root_order.push(id);
        }
    }

    fn canvas_and_descendants(&self, root: &str) -> Vec<Id> {
        let mut out = vec![root.to_string()];
        loop {
            let before = out.len();
            for canvas in self.canvases.values() {
                if out.iter().any(|id| id == &canvas.id) {
                    continue;
                }
                if canvas
                    .parent_canvas_id
                    .as_ref()
                    .is_some_and(|parent| out.iter().any(|id| id == parent))
                {
                    out.push(canvas.id.clone());
                }
            }
            if out.len() == before || out.len() > 10_000 {
                break;
            }
        }
        out
    }

    fn repair_canvas(&mut self, canvas_id: &str) {
        let known_canvases: HashSet<Id> = self.canvases.keys().cloned().collect();
        let Some(canvas) = self.canvases.get_mut(canvas_id) else {
            return;
        };
        if canvas.node(&canvas.center_id).is_none() {
            let now = Utc::now();
            let center_id = canvas.center_id.clone();
            canvas.nodes.push(Node {
                id: center_id,
                text: String::new(),
                created_at: now,
                modified_at: now,
                closed: false,
                parent_id: None,
                child_canvas_id: None,
            });
        }
        let center_id = canvas.center_id.clone();
        if let Some(center) = canvas.node_mut(&center_id) {
            center.parent_id = None;
            center.closed = false;
        }

        break_parent_cycles(canvas);

        let node_ids: HashSet<Id> = canvas.nodes.iter().map(|node| node.id.clone()).collect();
        for node in &mut canvas.nodes {
            if node.parent_id.as_ref().is_some_and(|parent| {
                parent == &node.id || !node_ids.contains(parent)
            }) {
                node.parent_id = None;
            }
            if node
                .child_canvas_id
                .as_ref()
                .is_some_and(|id| !known_canvases.contains(id))
            {
                node.child_canvas_id = None;
            }
        }

        canvas.edges.retain(|edge| {
            edge.from != edge.to && node_ids.contains(&edge.from) && node_ids.contains(&edge.to)
        });
        dedup_edges(canvas);

        let keep_edge: Vec<bool> = canvas
            .edges
            .iter()
            .map(|edge| {
                if edge.kind != EdgeKind::Tree {
                    return true;
                }
                canvas
                    .node(&edge.to)
                    .and_then(|node| node.parent_id.as_ref())
                    .is_some_and(|parent| parent == &edge.from)
            })
            .collect();
        let mut edge_index = 0;
        canvas.edges.retain(|_| {
            let keep = keep_edge[edge_index];
            edge_index += 1;
            keep
        });

        let missing: Vec<(Id, Id)> = canvas
            .nodes
            .iter()
            .filter_map(|node| {
                let parent = node.parent_id.clone()?;
                let exists = canvas.edges.iter().any(|edge| {
                    edge.kind == EdgeKind::Tree && edge.from == parent && edge.to == node.id
                });
                if exists {
                    None
                } else {
                    Some((parent, node.id.clone()))
                }
            })
            .collect();
        for (parent, child) in missing {
            canvas.edges.push(Edge {
                id: new_id(),
                from: parent,
                to: child,
                kind: EdgeKind::Tree,
            });
        }
    }
}

fn break_parent_cycles(canvas: &mut Canvas) {
    let ids: Vec<Id> = canvas.nodes.iter().map(|node| node.id.clone()).collect();
    for start in ids {
        let mut current = Some(start);
        let mut seen = HashSet::new();
        while let Some(id) = current {
            if !seen.insert(id.clone()) {
                if let Some(node) = canvas.node_mut(&id) {
                    node.parent_id = None;
                }
                break;
            }
            current = canvas.node(&id).and_then(|node| node.parent_id.clone());
        }
    }
}

fn dedup_edges(canvas: &mut Canvas) {
    let mut seen = HashSet::new();
    canvas.edges.retain(|edge| {
        let mut pair = [edge.from.clone(), edge.to.clone()];
        pair.sort();
        seen.insert((pair[0].clone(), pair[1].clone(), edge.kind))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at_days_before(now: DateTime<Utc>, days: i64) -> DateTime<Utc> {
        now - Duration::days(days)
    }

    fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 28, 12, 0, 0).unwrap()
    }

    #[test]
    fn staleness_thresholds() {
        let now = fixed_now();
        assert_eq!(staleness(at_days_before(now, 6), now), Staleness::Fresh);
        assert_eq!(staleness(at_days_before(now, 7), now), Staleness::Yellow);
        assert_eq!(staleness(at_days_before(now, 29), now), Staleness::Yellow);
        assert_eq!(staleness(at_days_before(now, 30), now), Staleness::Orange);
        assert_eq!(staleness(at_days_before(now, 181), now), Staleness::Orange);
        assert_eq!(staleness(at_days_before(now, 182), now), Staleness::Red);
        assert_eq!(staleness(now + Duration::hours(1), now), Staleness::Fresh);
    }

    #[test]
    fn child_edits_and_nested_canvases_refresh_ancestors() {
        let t0 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let t1 = t0 + Duration::days(3);
        let t2 = t0 + Duration::days(9);
        let mut library = Library::new();
        let root = library.create_root("Plan", t0);
        let center = library.canvas(&root).unwrap().center_id.clone();
        let child = library.add_child_node(&root, &center, t1).unwrap();
        assert!(library.set_text(&root, &child, "Write the brief".into(), t2));
        let center = library.canvas(&root).unwrap().center_id.clone();
        assert_eq!(library.collective_modified(&root, &center), Some(t2));
        assert_eq!(library.collective_modified(&root, &child), Some(t2));

        let nested = library.open_or_create_child_canvas(&root, &child, t1).unwrap();
        assert_eq!(
            library.canvas(&nested).unwrap().center_text(),
            "Write the brief"
        );
        let again = library.open_or_create_child_canvas(&root, &child, t2).unwrap();
        assert_eq!(nested, again);

        let nested_center = library.canvas(&nested).unwrap().center_id.clone();
        let t3 = t0 + Duration::days(12);
        assert!(library.set_text(&nested, &nested_center, "Write the brief".into(), t3));
        // Same text does not move the clock; force a real edit.
        assert!(library.set_text(&nested, &nested_center, "Brief v2".into(), t3));
        assert_eq!(library.collective_modified(&root, &center), Some(t3));
        assert_eq!(library.collective_modified(&root, &child), Some(t3));
    }

    #[test]
    fn closing_blocks_edits_until_the_closed_ancestor_opens() {
        let now = fixed_now();
        let mut library = Library::new();
        let root = library.create_root("Map", now);
        let center = library.canvas(&root).unwrap().center_id.clone();
        let parent = library.add_child_node(&root, &center, now).unwrap();
        let child = library.add_child_node(&root, &parent, now).unwrap();
        assert!(library.set_closed(&root, &parent, true, now));
        let canvas = library.canvas(&root).unwrap();
        assert!(canvas.effectively_closed(&parent));
        assert!(canvas.effectively_closed(&child));
        assert!(!canvas.effectively_closed(&center));
        assert!(!library.set_text(&root, &child, "nope".into(), now));
        assert!(library.add_child_node(&root, &child, now).is_none());
        assert!(!library.add_link(&root, &parent, &child, now));
        assert!(library.set_closed(&root, &parent, false, now));
        assert!(!library.canvas(&root).unwrap().effectively_closed(&child));
        assert!(library.set_text(&root, &child, "yes".into(), now));
    }

    #[test]
    fn delete_removes_the_rectangle_and_its_lines_only() {
        let now = fixed_now();
        let mut library = Library::new();
        let root = library.create_root("Map", now);
        let center = library.canvas(&root).unwrap().center_id.clone();
        let parent = library.add_child_node(&root, &center, now).unwrap();
        let child = library.add_child_node(&root, &parent, now).unwrap();
        let sibling = library.add_child_node(&root, &center, now).unwrap();
        assert!(library.add_link(&root, &parent, &sibling, now));
        let nested = library.open_or_create_child_canvas(&root, &parent, now).unwrap();

        assert!(!library.delete_node(&root, &center, now));
        assert!(library.delete_node(&root, &parent, now));

        let canvas = library.canvas(&root).unwrap();
        assert!(canvas.node(&parent).is_none());
        assert!(canvas.node(&child).is_some());
        assert_eq!(canvas.node(&child).unwrap().parent_id, None);
        assert!(canvas.edges.iter().all(|edge| edge.from != parent && edge.to != parent));
        assert!(library.canvas(&nested).unwrap().parent_canvas_id.is_none());
        assert!(library.root_order.iter().any(|id| id == &nested));
        assert!(library.roots().iter().any(|canvas| canvas.id == nested));
    }

    #[test]
    fn links_are_undirected_and_refuse_the_centre() {
        let now = fixed_now();
        let mut library = Library::new();
        let root = library.create_root("Map", now);
        let center = library.canvas(&root).unwrap().center_id.clone();
        let a = library.add_child_node(&root, &center, now).unwrap();
        let b = library.add_child_node(&root, &center, now).unwrap();
        assert!(!library.add_link(&root, &center, &a, now));
        assert!(!library.add_link(&root, &a, &a, now));
        assert!(library.add_link(&root, &a, &b, now));
        assert!(!library.add_link(&root, &b, &a, now));
    }

    #[test]
    fn delete_canvas_removes_nested_maps_and_the_link_to_them() {
        let now = fixed_now();
        let mut library = Library::new();
        let root = library.create_root("Map", now);
        let center = library.canvas(&root).unwrap().center_id.clone();
        let child = library.add_child_node(&root, &center, now).unwrap();
        let nested = library.open_or_create_child_canvas(&root, &child, now).unwrap();
        let nested_center = library.canvas(&nested).unwrap().center_id.clone();
        let deep = library
            .open_or_create_child_canvas(&nested, &nested_center, now)
            .unwrap();
        assert!(library.delete_canvas(&root));
        assert!(library.canvas(&root).is_none());
        assert!(library.canvas(&nested).is_none());
        assert!(library.canvas(&deep).is_none());
    }

    #[test]
    fn stale_list_contains_only_quiet_circles_oldest_first() {
        let now = fixed_now();
        let mut library = Library::new();
        let fresh = library.create_root("Fresh", now);
        let yellow = library.create_root("Yellow", at_days_before(now, 10));
        let red = library.create_root("Red", at_days_before(now, 200));
        let orange = library.create_root("Orange", at_days_before(now, 40));
        let _ = fresh;
        let stale = library.stale_circles(now);
        let ids: Vec<_> = stale.iter().map(|item| item.canvas_id.as_str()).collect();
        assert_eq!(ids, vec![red.as_str(), orange.as_str(), yellow.as_str()]);
        assert_eq!(stale[0].staleness, Staleness::Red);
        assert_eq!(stale[1].staleness, Staleness::Orange);
        assert_eq!(stale[2].staleness, Staleness::Yellow);
    }
}
