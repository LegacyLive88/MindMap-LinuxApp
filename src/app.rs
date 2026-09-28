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
