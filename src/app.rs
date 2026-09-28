//! Windowed canvas: pan, zoom, branch, link, close, and nested maps.
#![allow(float_literal_f32_fallback)]

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
