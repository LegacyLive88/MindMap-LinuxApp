//! Canvases, nodes, links, staleness, and the edits the canvas can make.
//!
//! A canvas is a tree grown from a centre circle. Ctrl+click adds a child
//! rectangle. Ctrl+drag adds a non-hierarchical link. Shift+click opens that
//! idea as its own canvas. Closing a rectangle closes its descendants.

use chrono::{DateTime, Duration, NaiveDate, Utc};
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
pub struct Task {
    pub id: Id,
    pub text: String,
    #[serde(default)]
    pub done: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline: Option<NaiveDate>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Constraint {
    pub id: Id,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub id: Id,
    pub from: Id,
    pub to: Id,
    pub kind: EdgeKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<Task>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<Constraint>,
    /// Share of this line that is constrained, from 0 to 100.
    /// Absent on older files, and until the user sets one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constrained_percent: Option<u8>,
}

/// How soon an open task is due. Red is the most urgent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DeadlineHeat {
    Red,
    Orange,
    Yellow,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskList {
    pub canvas_id: Id,
    pub edge_id: Id,
    pub from_id: Id,
    pub to_id: Id,
    pub from_text: String,
    pub to_text: String,
    pub tasks: Vec<Task>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstraintList {
    pub canvas_id: Id,
    pub edge_id: Id,
    pub from_id: Id,
    pub to_id: Id,
    pub from_text: String,
    pub to_text: String,
    pub constraints: Vec<Constraint>,
    pub constrained_percent: Option<u8>,
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

/// Colour for a deadline measured in calendar days from today.
/// Due today, overdue, or due within 3 days is red. Within a week is orange.
/// Within 3 weeks is yellow.
pub fn deadline_heat(deadline: NaiveDate, today: NaiveDate) -> Option<DeadlineHeat> {
    let days = (deadline - today).num_days();
    if days <= 3 {
        Some(DeadlineHeat::Red)
    } else if days <= 7 {
        Some(DeadlineHeat::Orange)
    } else if days <= 21 {
        Some(DeadlineHeat::Yellow)
    } else {
        None
    }
}

fn new_edge(from: Id, to: Id, kind: EdgeKind) -> Edge {
    Edge {
        id: new_id(),
        from,
        to,
        kind,
        tasks: Vec::new(),
        constraints: Vec::new(),
        constrained_percent: None,
    }
}

impl Edge {
    /// Completed tasks over the total, when at least one task is still open.
    pub fn open_task_ratio(&self) -> Option<(usize, usize)> {
        if self.tasks.is_empty() {
            return None;
        }
        let done = self.tasks.iter().filter(|task| task.done).count();
        let total = self.tasks.len();
        if done == total {
            return None;
        }
        Some((done, total))
    }

    /// The share of the line that is not constrained, once a percent is set
    /// and at least one constraint has been entered.
    pub fn free_percent(&self) -> Option<u8> {
        if self.constraints.is_empty() {
            return None;
        }
        self.constrained_percent
            .map(|percent| 100 - percent.min(100))
    }

    pub fn heat(&self, today: NaiveDate) -> Option<DeadlineHeat> {
        self.tasks
            .iter()
            .filter(|task| !task.done)
            .filter_map(|task| task.deadline)
            .filter_map(|deadline| deadline_heat(deadline, today))
            .min()
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
        canvas.edges.push(new_edge(
            parent_id.to_string(),
            node_id.clone(),
            EdgeKind::Tree,
        ));
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
        canvas.edges.push(new_edge(a.to_string(), b.to_string(), EdgeKind::Link));
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

    pub fn edge_closed(&self, canvas_id: &str, edge_id: &str) -> bool {
        let Some(canvas) = self.canvases.get(canvas_id) else {
            return true;
        };
        let Some(edge) = canvas.edges.iter().find(|edge| edge.id == edge_id) else {
            return true;
        };
        canvas.effectively_closed(&edge.from) || canvas.effectively_closed(&edge.to)
    }

    pub fn add_task(&mut self, canvas_id: &str, edge_id: &str, now: DateTime<Utc>) -> Option<Id> {
        if self.edge_closed(canvas_id, edge_id) {
            return None;
        }
        let task_id = new_id();
        {
            let canvas = self.canvases.get_mut(canvas_id)?;
            let edge = canvas.edges.iter_mut().find(|edge| edge.id == edge_id)?;
            edge.tasks.push(Task {
                id: task_id.clone(),
                text: String::new(),
                done: false,
                deadline: None,
            });
        }
        self.stamp_edge(canvas_id, edge_id, now);
        Some(task_id)
    }

    pub fn set_task_text(
        &mut self,
        canvas_id: &str,
        edge_id: &str,
        task_id: &str,
        text: String,
        now: DateTime<Utc>,
    ) -> bool {
        if self.edge_closed(canvas_id, edge_id) {
            return false;
        }
        let changed = {
            let Some(canvas) = self.canvases.get_mut(canvas_id) else {
                return false;
            };
            let Some(edge) = canvas.edges.iter_mut().find(|edge| edge.id == edge_id) else {
                return false;
            };
            let Some(task) = edge.tasks.iter_mut().find(|task| task.id == task_id) else {
                return false;
            };
            if task.text == text {
                return true;
            }
            task.text = text;
            true
        };
        if changed {
            self.stamp_edge(canvas_id, edge_id, now);
        }
        changed
    }

    pub fn set_task_done(
        &mut self,
        canvas_id: &str,
        edge_id: &str,
        task_id: &str,
        done: bool,
        now: DateTime<Utc>,
    ) -> bool {
        if self.edge_closed(canvas_id, edge_id) {
            return false;
        }
        let changed = {
            let Some(canvas) = self.canvases.get_mut(canvas_id) else {
                return false;
            };
            let Some(edge) = canvas.edges.iter_mut().find(|edge| edge.id == edge_id) else {
                return false;
            };
            let Some(task) = edge.tasks.iter_mut().find(|task| task.id == task_id) else {
                return false;
            };
            if task.done == done {
                return true;
            }
            task.done = done;
            true
        };
        if changed {
            self.stamp_edge(canvas_id, edge_id, now);
        }
        changed
    }

    pub fn set_task_deadline(
        &mut self,
        canvas_id: &str,
        edge_id: &str,
        task_id: &str,
        deadline: Option<NaiveDate>,
        now: DateTime<Utc>,
    ) -> bool {
        if self.edge_closed(canvas_id, edge_id) {
            return false;
        }
        let changed = {
            let Some(canvas) = self.canvases.get_mut(canvas_id) else {
                return false;
            };
            let Some(edge) = canvas.edges.iter_mut().find(|edge| edge.id == edge_id) else {
                return false;
            };
            let Some(task) = edge.tasks.iter_mut().find(|task| task.id == task_id) else {
                return false;
            };
            if task.deadline == deadline {
                return true;
            }
            task.deadline = deadline;
            true
        };
        if changed {
            self.stamp_edge(canvas_id, edge_id, now);
        }
        changed
    }

    pub fn remove_task(
        &mut self,
        canvas_id: &str,
        edge_id: &str,
        task_id: &str,
        now: DateTime<Utc>,
    ) -> bool {
        if self.edge_closed(canvas_id, edge_id) {
            return false;
        }
        let removed = {
            let Some(canvas) = self.canvases.get_mut(canvas_id) else {
                return false;
            };
            let Some(edge) = canvas.edges.iter_mut().find(|edge| edge.id == edge_id) else {
                return false;
            };
            let before = edge.tasks.len();
            edge.tasks.retain(|task| task.id != task_id);
            edge.tasks.len() != before
        };
        if removed {
            self.stamp_edge(canvas_id, edge_id, now);
        }
        removed
    }

    pub fn add_constraint(&mut self, canvas_id: &str, edge_id: &str, now: DateTime<Utc>) -> Option<Id> {
        if self.edge_closed(canvas_id, edge_id) {
            return None;
        }
        let constraint_id = new_id();
        {
            let canvas = self.canvases.get_mut(canvas_id)?;
            let edge = canvas.edges.iter_mut().find(|edge| edge.id == edge_id)?;
            edge.constraints.push(Constraint {
                id: constraint_id.clone(),
                text: String::new(),
            });
        }
        self.stamp_edge(canvas_id, edge_id, now);
        Some(constraint_id)
    }

    pub fn set_constraint_text(
        &mut self,
        canvas_id: &str,
        edge_id: &str,
        constraint_id: &str,
        text: String,
        now: DateTime<Utc>,
    ) -> bool {
        if self.edge_closed(canvas_id, edge_id) {
            return false;
        }
        let changed = {
            let Some(canvas) = self.canvases.get_mut(canvas_id) else {
                return false;
            };
            let Some(edge) = canvas.edges.iter_mut().find(|edge| edge.id == edge_id) else {
                return false;
            };
            let Some(constraint) = edge
                .constraints
                .iter_mut()
                .find(|constraint| constraint.id == constraint_id)
            else {
                return false;
            };
            if constraint.text == text {
                return true;
            }
            constraint.text = text;
            true
        };
        if changed {
            self.stamp_edge(canvas_id, edge_id, now);
        }
        changed
    }

    pub fn remove_constraint(
        &mut self,
        canvas_id: &str,
        edge_id: &str,
        constraint_id: &str,
        now: DateTime<Utc>,
    ) -> bool {
        if self.edge_closed(canvas_id, edge_id) {
            return false;
        }
        let removed = {
            let Some(canvas) = self.canvases.get_mut(canvas_id) else {
                return false;
            };
            let Some(edge) = canvas.edges.iter_mut().find(|edge| edge.id == edge_id) else {
                return false;
            };
            let before = edge.constraints.len();
            edge.constraints.retain(|constraint| constraint.id != constraint_id);
            edge.constraints.len() != before
        };
        if removed {
            self.stamp_edge(canvas_id, edge_id, now);
        }
        removed
    }

    pub fn set_constrained_percent(
        &mut self,
        canvas_id: &str,
        edge_id: &str,
        percent: Option<u8>,
        now: DateTime<Utc>,
    ) -> bool {
        if self.edge_closed(canvas_id, edge_id) {
            return false;
        }
        let percent = percent.map(|value| value.min(100));
        let changed = {
            let Some(canvas) = self.canvases.get_mut(canvas_id) else {
                return false;
            };
            let Some(edge) = canvas.edges.iter_mut().find(|edge| edge.id == edge_id) else {
                return false;
            };
            if edge.constraints.is_empty() {
                return false;
            }
            if edge.constrained_percent == percent {
                return true;
            }
            edge.constrained_percent = percent;
            true
        };
        if changed {
            self.stamp_edge(canvas_id, edge_id, now);
        }
        changed
    }

    /// One group per line that has tasks. Groups and the tasks inside them
    /// are ordered by the soonest open deadline.
    pub fn task_lists(&self) -> Vec<TaskList> {
        let mut lists = Vec::new();
        for canvas in self.canvases.values() {
            for edge in &canvas.edges {
                if edge.tasks.is_empty() {
                    continue;
                }
                let mut tasks = edge.tasks.clone();
                tasks.sort_by(cmp_tasks_by_deadline);
                lists.push(TaskList {
                    canvas_id: canvas.id.clone(),
                    edge_id: edge.id.clone(),
                    from_id: edge.from.clone(),
                    to_id: edge.to.clone(),
                    from_text: canvas
                        .node(&edge.from)
                        .map(|node| node.text.clone())
                        .unwrap_or_default(),
                    to_text: canvas
                        .node(&edge.to)
                        .map(|node| node.text.clone())
                        .unwrap_or_default(),
                    tasks,
                });
            }
        }
        lists.sort_by(|left, right| {
            let left_due = soonest_open_deadline(&left.tasks);
            let right_due = soonest_open_deadline(&right.tasks);
            match (left_due, right_due) {
                (Some(left_due), Some(right_due)) => left_due.cmp(&right_due),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => left
                    .from_text
                    .cmp(&right.from_text)
                    .then(left.to_text.cmp(&right.to_text))
                    .then(left.edge_id.cmp(&right.edge_id)),
            }
        });
        lists
    }

    /// One group per line that has constraints, soonest to the most constrained.
    pub fn constraint_lists(&self) -> Vec<ConstraintList> {
        let mut lists = Vec::new();
        for canvas in self.canvases.values() {
            for edge in &canvas.edges {
                if edge.constraints.is_empty() {
                    continue;
                }
                lists.push(ConstraintList {
                    canvas_id: canvas.id.clone(),
                    edge_id: edge.id.clone(),
                    from_id: edge.from.clone(),
                    to_id: edge.to.clone(),
                    from_text: canvas
                        .node(&edge.from)
                        .map(|node| node.text.clone())
                        .unwrap_or_default(),
                    to_text: canvas
                        .node(&edge.to)
                        .map(|node| node.text.clone())
                        .unwrap_or_default(),
                    constraints: edge.constraints.clone(),
                    constrained_percent: edge.constrained_percent,
                });
            }
        }
        lists.sort_by(|left, right| {
            match (left.constrained_percent, right.constrained_percent) {
                (Some(left_percent), Some(right_percent)) => right_percent
                    .cmp(&left_percent)
                    .then(left.to_text.cmp(&right.to_text))
                    .then(left.edge_id.cmp(&right.edge_id)),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => left
                    .to_text
                    .cmp(&right.to_text)
                    .then(left.edge_id.cmp(&right.edge_id)),
            }
        });
        lists
    }

    fn stamp_edge(&mut self, canvas_id: &str, edge_id: &str, now: DateTime<Utc>) {
        let Some(canvas) = self.canvases.get_mut(canvas_id) else {
            return;
        };
        let Some(edge) = canvas.edges.iter().find(|edge| edge.id == edge_id) else {
            return;
        };
        let from = edge.from.clone();
        let to = edge.to.clone();
        for node in &mut canvas.nodes {
            if node.id == from || node.id == to {
                node.modified_at = now;
            }
        }
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
            canvas.edges.push(new_edge(parent, child, EdgeKind::Tree));
        }

        for edge in &mut canvas.edges {
            if let Some(percent) = edge.constrained_percent {
                edge.constrained_percent = Some(percent.min(100));
            }
        }
    }
}

fn soonest_open_deadline(tasks: &[Task]) -> Option<NaiveDate> {
    tasks
        .iter()
        .filter(|task| !task.done)
        .filter_map(|task| task.deadline)
        .min()
}

fn cmp_tasks_by_deadline(left: &Task, right: &Task) -> std::cmp::Ordering {
    match (left.done, right.done) {
        (false, true) => std::cmp::Ordering::Less,
        (true, false) => std::cmp::Ordering::Greater,
        _ => match (left.deadline, right.deadline) {
            (Some(left_due), Some(right_due)) => left_due.cmp(&right_due),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left.id.cmp(&right.id),
        },
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

    #[test]
    fn older_canvas_json_without_tasks_still_loads() {
        let json = r#"{
            "id": "canvas-old",
            "center_id": "center-old",
            "nodes": [
                {
                    "id": "center-old",
                    "text": "Kept",
                    "created_at": "2024-03-01T10:00:00Z",
                    "modified_at": "2024-06-01T10:00:00Z"
                },
                {
                    "id": "child-old",
                    "text": "Branch",
                    "created_at": "2024-03-02T10:00:00Z",
                    "modified_at": "2024-06-02T10:00:00Z",
                    "parent_id": "center-old"
                }
            ],
            "edges": [
                {
                    "id": "edge-old",
                    "from": "center-old",
                    "to": "child-old",
                    "kind": "tree"
                }
            ]
        }"#;
        let canvas: Canvas = serde_json::from_str(json).unwrap();
        assert_eq!(canvas.center_text(), "Kept");
        assert!(canvas.edges[0].tasks.is_empty());
        assert!(canvas.edges[0].constraints.is_empty());
        assert_eq!(canvas.edges[0].constrained_percent, None);
        let mut library = Library::new();
        library.canvases.insert(canvas.id.clone(), canvas);
        library.repair();
        let canvas = library.canvas("canvas-old").unwrap();
        assert_eq!(canvas.node("child-old").unwrap().text, "Branch");
        assert_eq!(canvas.edges.len(), 1);
        assert!(canvas.edges[0].tasks.is_empty());
    }

    #[test]
    fn line_tasks_move_the_modified_clock_and_colour_by_deadline() {
        let now = fixed_now();
        let today = now.date_naive();
        let mut library = Library::new();
        let root = library.create_root("Plan", now);
        let center = library.canvas(&root).unwrap().center_id.clone();
        let _child = library.add_child_node(&root, &center, now).unwrap();
        let edge = library.canvas(&root).unwrap().edges[0].id.clone();
        let before = library.collective_modified(&root, &center).unwrap();
        let later = now + Duration::hours(3);
        let task = library.add_task(&root, &edge, later).unwrap();
        assert!(library.set_task_text(&root, &edge, &task, "Draft".into(), later));
        assert_eq!(library.collective_modified(&root, &center), Some(later));
        assert!(before < later);

        assert!(library.set_task_deadline(&root, &edge, &task, Some(today + Duration::days(3)), later));
        let edge_ref = library
            .canvas(&root)
            .unwrap()
            .edges
            .iter()
            .find(|item| item.id == edge)
            .unwrap();
        assert_eq!(edge_ref.heat(today), Some(DeadlineHeat::Red));
        assert_eq!(edge_ref.open_task_ratio(), Some((0, 1)));

        assert!(library.set_task_deadline(&root, &edge, &task, Some(today + Duration::days(4)), later));
        assert!(library.set_task_deadline(&root, &edge, &task, Some(today + Duration::days(7)), later));
        let edge_ref = library.canvas(&root).unwrap().edges.iter().find(|item| item.id == edge).unwrap();
        assert_eq!(edge_ref.heat(today), Some(DeadlineHeat::Orange));

        assert!(library.set_task_deadline(&root, &edge, &task, Some(today + Duration::days(8)), later));
        assert!(library.set_task_deadline(&root, &edge, &task, Some(today + Duration::days(21)), later));
        let edge_ref = library.canvas(&root).unwrap().edges.iter().find(|item| item.id == edge).unwrap();
        assert_eq!(edge_ref.heat(today), Some(DeadlineHeat::Yellow));

        assert!(library.set_task_deadline(&root, &edge, &task, Some(today + Duration::days(22)), later));
        let edge_ref = library.canvas(&root).unwrap().edges.iter().find(|item| item.id == edge).unwrap();
        assert_eq!(edge_ref.heat(today), None);

        assert!(library.set_task_deadline(&root, &edge, &task, Some(today - Duration::days(1)), later));
        let edge_ref = library.canvas(&root).unwrap().edges.iter().find(|item| item.id == edge).unwrap();
        assert_eq!(edge_ref.heat(today), Some(DeadlineHeat::Red));

        assert!(library.set_task_done(&root, &edge, &task, true, later));
        let edge_ref = library.canvas(&root).unwrap().edges.iter().find(|item| item.id == edge).unwrap();
        assert_eq!(edge_ref.heat(today), None);
        assert_eq!(edge_ref.open_task_ratio(), None);
    }

    #[test]
    fn constraint_percent_is_the_free_share_and_lists_follow_urgency() {
        let now = fixed_now();
        let today = now.date_naive();
        let mut library = Library::new();
        let root = library.create_root("Plan", now);
        let center = library.canvas(&root).unwrap().center_id.clone();
        let soon = library.add_child_node(&root, &center, now).unwrap();
        let later_node = library
            .add_child_node(&root, &center, now + Duration::seconds(1))
            .unwrap();
        assert!(library.set_text(&root, &soon, "Soon".into(), now));
        assert!(library.set_text(&root, &later_node, "Later".into(), now));
        let soon_edge = library
            .canvas(&root)
            .unwrap()
            .edges
            .iter()
            .find(|edge| edge.to == soon)
            .unwrap()
            .id
            .clone();
        let later_edge = library
            .canvas(&root)
            .unwrap()
            .edges
            .iter()
            .find(|edge| edge.to == later_node)
            .unwrap()
            .id
            .clone();

        let soon_task = library.add_task(&root, &soon_edge, now).unwrap();
        library
            .set_task_deadline(&root, &soon_edge, &soon_task, Some(today + Duration::days(2)), now);
        let later_task = library.add_task(&root, &later_edge, now).unwrap();
        library.set_task_deadline(
            &root,
            &later_edge,
            &later_task,
            Some(today + Duration::days(10)),
            now,
        );
        let lists = library.task_lists();
        assert_eq!(lists[0].edge_id, soon_edge);
        assert_eq!(lists[1].edge_id, later_edge);

        assert!(library.set_constrained_percent(&root, &soon_edge, Some(40), now) == false);
        let tight = library.add_constraint(&root, &soon_edge, now).unwrap();
        let loose = library.add_constraint(&root, &later_edge, now).unwrap();
        assert!(library.set_constraint_text(&root, &soon_edge, &tight, "Budget".into(), now));
        assert!(library.set_constraint_text(&root, &later_edge, &loose, "Time".into(), now));
        assert!(library.set_constrained_percent(&root, &soon_edge, Some(40), now));
        assert!(library.set_constrained_percent(&root, &later_edge, Some(85), now));
        let soon_edge_ref = library
            .canvas(&root)
            .unwrap()
            .edges
            .iter()
            .find(|edge| edge.id == soon_edge)
            .unwrap();
        assert_eq!(soon_edge_ref.free_percent(), Some(60));
        let ordered = library.constraint_lists();
        assert_eq!(ordered[0].edge_id, later_edge);
        assert_eq!(ordered[0].to_text, "Later");
        assert_eq!(ordered[1].constrained_percent, Some(40));
    }
}
