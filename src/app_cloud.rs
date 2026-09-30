// Cloud settings window and the background sync with the public endpoint.

use mindmap::cloud::{self, CloudEvent, CloudJob, CloudSession, JobKind, RemoteDecision};
use mindmap::secrets::{self, CloudSettings};
use std::sync::mpsc::{self, TryRecvError};
use std::thread;

enum Intent {
    Pull,
    PreferRemote,
    Push,
    ForcePush,
    Test,
    Backup,
}

enum CloudEffect {
    Install {
        revision: u64,
        json: String,
        archive: bool,
    },
}

struct CloudPanel {
    data_dir: std::path::PathBuf,
    settings: CloudSettings,
    state: cloud::SyncState,
    open: bool,
    draft_endpoint: String,
    draft_username: String,
    draft_password: String,
    draft_sync: bool,
    show_password: bool,
    fingerprint: String,
    latest_json: String,
    has_maps: bool,
    dirty: bool,
    busy: bool,
    queued: Option<Intent>,
    debounce_until: Option<Instant>,
    next_pull: Instant,
    message: String,
    conflict_json: Option<String>,
    conflict_revision: u64,
    needs_choice: bool,
    offline: bool,
    effect: Option<CloudEffect>,
    job_tx: Option<mpsc::Sender<CloudJob>>,
    event_rx: Option<mpsc::Receiver<CloudEvent>>,
}

impl CloudPanel {
    fn open(dir: &std::path::Path) -> Self {
        let (settings, message) = match secrets::load_settings(dir) {
            Ok(Some(settings)) => (settings, String::new()),
            Ok(None) => (CloudSettings::default(), String::new()),
            Err(error) => (CloudSettings::default(), error),
        };
        let state = cloud::load_sync_state(dir);
        let (job_tx, job_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let spawned = thread::Builder::new()
            .name("mindmap-cloud".into())
            .spawn(move || {
                let mut session = CloudSession::new();
                while let Ok(job) = job_rx.recv() {
                    let event = session.perform(&job);
                    if event_tx.send(event).is_err() {
                        break;
                    }
                }
            })
            .is_ok();
        Self {
            data_dir: dir.to_path_buf(),
            settings,
            state,
            open: false,
            draft_endpoint: String::new(),
            draft_username: String::new(),
            draft_password: String::new(),
            draft_sync: true,
            show_password: false,
            fingerprint: String::new(),
            latest_json: String::new(),
            has_maps: false,
            dirty: false,
            busy: false,
            queued: None,
            debounce_until: None,
            next_pull: Instant::now() + Duration::from_millis(900),
            message,
            conflict_json: None,
            conflict_revision: 0,
            needs_choice: false,
            offline: false,
            effect: None,
            job_tx: spawned.then_some(job_tx),
            event_rx: spawned.then_some(event_rx),
        }
    }

    fn observe_library(&mut self, library: &Library) {
        self.fingerprint = cloud::library_fingerprint(library);
        self.latest_json = cloud::library_json(library);
        self.has_maps = !library.canvases.is_empty();
        self.dirty = !self.state.local_hash.is_empty() && self.state.local_hash != self.fingerprint;
        if self.dirty && self.settings.sync_enabled && self.settings.ready() {
            self.debounce_until = Some(Instant::now() + Duration::from_millis(1200));
        }
    }

    fn begin_edit(&mut self) {
        if !self.open {
            self.draft_endpoint = self.settings.endpoint.clone();
            self.draft_username = self.settings.username.clone();
            self.draft_password.clear();
            self.draft_sync = self.settings.sync_enabled;
            self.show_password = false;
        }
        self.open = true;
    }

    fn mark_synced(&mut self, revision: u64, library: &Library) {
        self.state.base_revision = revision;
        self.state.local_hash = cloud::library_fingerprint(library);
        self.fingerprint = self.state.local_hash.clone();
        self.latest_json = cloud::library_json(library);
        self.has_maps = !library.canvases.is_empty();
        self.dirty = false;
        self.needs_choice = false;
        self.offline = false;
        self.conflict_json = None;
        self.debounce_until = None;
        self.message = format!("Up to date with the cloud. Revision {revision}.");
        self.persist_state();
    }

    fn tick(&mut self) {
        loop {
            let event = match self.event_rx.as_ref().map(|rx| rx.try_recv()) {
                Some(Ok(event)) => event,
                Some(Err(TryRecvError::Empty)) | None => break,
                Some(Err(TryRecvError::Disconnected)) => {
                    self.busy = false;
                    self.job_tx = None;
                    self.event_rx = None;
                    if self.message.is_empty() {
                        self.message = "Cloud sync stopped.".into();
                    }
                    break;
                }
            };
            self.busy = false;
            self.apply_event(event);
        }
        if self.busy {
            return;
        }
        if let Some(intent) = self.queued.take() {
            self.start(intent);
            return;
        }
        self.schedule_auto();
    }

    fn request_repaint(&self, ctx: &egui::Context) {
        if self.busy {
            ctx.request_repaint_after(Duration::from_millis(200));
            return;
        }
        if let Some(until) = self.debounce_until {
            let wait = until.saturating_duration_since(Instant::now());
            ctx.request_repaint_after(wait.max(Duration::from_millis(150)));
            return;
        }
        if self.settings.sync_enabled && self.settings.ready() && !self.needs_choice {
            let wait = self.next_pull.saturating_duration_since(Instant::now());
            ctx.request_repaint_after(wait.min(Duration::from_secs(5)).max(Duration::from_millis(250)));
        }
    }

    fn take_effect(&mut self) -> Option<CloudEffect> {
        self.effect.take()
    }

    fn status_line(&self) -> (String, Color32) {
        let (text, color) = if !self.settings.ready() {
            ("Cloud  ·  not set up", CREAM_DIM)
        } else if !self.settings.sync_enabled {
            ("Cloud  ·  sync is off", CREAM_DIM)
        } else if self.needs_choice {
            ("Cloud  ·  choose a copy", YELLOW)
        } else if self.busy {
            ("Cloud  ·  syncing", CREAM)
        } else if self.offline {
            ("Cloud  ·  offline", YELLOW)
        } else if self.dirty {
            ("Cloud  ·  waiting to sync", CREAM)
        } else if self.state.local_hash.is_empty() {
            ("Cloud  ·  not synced yet", CREAM_DIM)
        } else {
            ("Cloud  ·  up to date", CREAM_DIM)
        };
        (text.to_string(), color)
    }

    fn apply_event(&mut self, event: CloudEvent) {
        match event {
            CloudEvent::TestOk { detail } | CloudEvent::BackupOk { detail } => {
                self.offline = false;
                self.message = detail;
                self.next_pull = Instant::now() + Duration::from_secs(20);
            }
            CloudEvent::Synced {
                revision,
                pushed_hash,
                detail,
            } => {
                self.offline = false;
                self.state.base_revision = revision;
                self.state.local_hash = pushed_hash.clone();
                self.dirty = self.fingerprint != pushed_hash;
                self.needs_choice = false;
                self.conflict_json = None;
                self.message = detail;
                self.next_pull = Instant::now() + Duration::from_secs(20);
                self.persist_state();
                if self.dirty {
                    self.debounce_until = Some(Instant::now() + Duration::from_millis(800));
                }
            }
            CloudEvent::Remote {
                revision,
                library_json,
                prefer_remote,
            } => {
                self.offline = false;
                self.next_pull = Instant::now() + Duration::from_secs(20);
                let decision = cloud::decide_after_pull(
                    &self.state,
                    self.dirty,
                    self.has_maps,
                    revision,
                    library_json.is_some(),
                    prefer_remote,
                );
                self.follow_decision(decision, revision, library_json, prefer_remote);
            }
            CloudEvent::Conflict {
                revision,
                library_json,
            } => {
                self.offline = false;
                self.remember_conflict(revision, library_json);
                self.message = "The cloud changed before this computer could upload. Choose which copy to keep.".into();
                self.next_pull = Instant::now() + Duration::from_secs(20);
            }
            CloudEvent::Offline { detail } => {
                self.offline = true;
                self.message = format!("{detail} Changes stay on this computer until the connection returns.");
                self.next_pull = Instant::now() + Duration::from_secs(20);
            }
            CloudEvent::Failed { detail } => {
                self.offline = false;
                self.message = detail;
                self.next_pull = Instant::now() + Duration::from_secs(20);
            }
        }
    }

    fn follow_decision(
        &mut self,
        decision: RemoteDecision,
        revision: u64,
        library_json: Option<String>,
        prefer_remote: bool,
    ) {
        match decision {
            RemoteDecision::Idle => {
                self.needs_choice = false;
                self.message = format!("Up to date with the cloud. Revision {}.", self.state.base_revision);
            }
            RemoteDecision::MarkEmptySynced => {
                self.state.base_revision = revision;
                self.state.local_hash = self.fingerprint.clone();
                self.dirty = false;
                self.needs_choice = false;
                self.message = "Signed in. There is no map on the cloud yet.".into();
                self.persist_state();
            }
            RemoteDecision::Push => {
                self.needs_choice = false;
                self.message = "Sending this computer's maps to the cloud.".into();
                self.queued = Some(Intent::Push);
            }
            RemoteDecision::Replace => {
                if let Some(json) = library_json {
                    self.effect = Some(CloudEffect::Install {
                        revision,
                        json,
                        archive: prefer_remote && self.has_maps,
                    });
                }
            }
            RemoteDecision::Conflict => {
                if let Some(json) = library_json {
                    self.remember_conflict(revision, json);
                    self.message = "This computer and the cloud both have changes. Choose which copy to keep.".into();
                }
            }
            RemoteDecision::NoRemote => {
                self.message = "The cloud does not have a map to download yet.".into();
            }
        }
    }

    fn remember_conflict(&mut self, revision: u64, json: String) {
        self.conflict_revision = revision;
        self.conflict_json = Some(json);
        self.needs_choice = true;
        self.begin_edit();
    }

    fn schedule_auto(&mut self) {
        if self.needs_choice || !self.settings.sync_enabled || !self.settings.ready() {
            return;
        }
        let now = Instant::now();
        if self.dirty {
            if self.debounce_until.is_some_and(|until| now >= until) {
                self.debounce_until = None;
                self.start(Intent::Push);
            }
            return;
        }
        if now >= self.next_pull {
            self.start(Intent::Pull);
        }
    }

    fn request(&mut self, intent: Intent) {
        if !self.settings.ready() {
            self.message = "Save the cloud address, username, and password first.".into();
            self.open = true;
            return;
        }
        if self.busy {
            self.queued = Some(intent);
            return;
        }
        self.start(intent);
    }

    fn start(&mut self, intent: Intent) {
        let Some(tx) = self.job_tx.clone() else {
            self.message = "Cloud sync did not start on this computer.".into();
            return;
        };
        if !self.settings.ready() {
            self.message = "Save the cloud address, username, and password first.".into();
            return;
        }
        if matches!(intent, Intent::Push | Intent::ForcePush) && self.latest_json.is_empty() {
            self.message = "There is no map to upload yet.".into();
            return;
        }
        let kind = match intent {
            Intent::Pull => JobKind::Pull { prefer_remote: false },
            Intent::PreferRemote => JobKind::Pull { prefer_remote: true },
            Intent::Push => JobKind::Push {
                library_json: self.latest_json.clone(),
                base_revision: self.state.base_revision,
                force: false,
            },
            Intent::ForcePush => JobKind::Push {
                library_json: self.latest_json.clone(),
                base_revision: self.state.base_revision,
                force: true,
            },
            Intent::Test => JobKind::Test,
            Intent::Backup => JobKind::Backup,
        };
        let job = CloudJob {
            endpoint: self.settings.endpoint.clone(),
            username: self.settings.username.clone(),
            password: self.settings.password.clone(),
            client_id: self.state.client_id.clone(),
            kind,
        };
        if tx.send(job).is_err() {
            self.job_tx = None;
            self.message = "Cloud sync stopped.".into();
            return;
        }
        self.busy = true;
    }

    fn save_drafts(&mut self) -> bool {
        let endpoint = match cloud::normalize_endpoint(&self.draft_endpoint) {
            Ok(endpoint) => endpoint,
            Err(error) => {
                self.message = error;
                return false;
            }
        };
        let username = self.draft_username.trim().to_string();
        if username.is_empty() {
            self.message = "Enter the username.".into();
            return false;
        }
        let password = if self.draft_password.is_empty() {
            self.settings.password.clone()
        } else {
            self.draft_password.clone()
        };
        if password.is_empty() {
            self.message = "Enter the password.".into();
            return false;
        }
        let settings = CloudSettings {
            endpoint: endpoint.clone(),
            username,
            password,
            sync_enabled: self.draft_sync,
        };
        if let Err(error) = secrets::save_settings(&self.data_dir, &settings) {
            self.message = format!("Could not save the cloud settings ({error}).");
            return false;
        }
        let identity_changed = self.settings.endpoint != settings.endpoint
            || self.settings.username != settings.username
            || self.settings.password != settings.password;
        if identity_changed {
            self.needs_choice = false;
            self.conflict_json = None;
            self.offline = false;
            self.next_pull = Instant::now();
        } else if settings.sync_enabled && !self.settings.sync_enabled {
            self.next_pull = Instant::now();
        }
        self.settings = settings;
        self.draft_endpoint = endpoint;
        self.draft_password.clear();
        let insecure = self.settings.endpoint.starts_with("http://")
            && !self.settings.endpoint.starts_with("http://localhost")
            && !self.settings.endpoint.starts_with("http://127.0.0.1");
        self.message = if insecure {
            "Settings saved. This address is not encrypted in transit. Prefer https:// once the cloud has a certificate.".into()
        } else {
            "Settings saved on this computer, encrypted in the mindmap-data folder.".into()
        };
        true
    }

    fn persist_state(&mut self) {
        if let Err(error) = cloud::save_sync_state(&self.data_dir, &self.state) {
            self.message = format!("Could not record the sync state ({error}).");
        }
    }

    fn ui(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        let mut still_open = true;
        let mut save = false;
        let mut test = false;
        let mut sync_now = false;
        let mut backup = false;
        let mut upload = false;
        let mut download = false;
        egui::Window::new("Cloud")
            .open(&mut still_open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.set_min_width(420.0);
                ui.label(
                    RichText::new("Sign in to the MindMap cloud. Maps on this computer sync when there is a connection, and the cloud keeps a backup of the previous copy each time it accepts an upload.")
                        .size(13.5)
                        .color(MUTED),
                );
                ui.add_space(8.0);
                ui.label(RichText::new("Cloud address").strong());
                ui.add(
                    TextEdit::singleline(&mut self.draft_endpoint)
                        .hint_text("https://maps.example.com")
                        .desired_width(f32::INFINITY),
                );
                ui.label(RichText::new("Username").strong());
                ui.add(
                    TextEdit::singleline(&mut self.draft_username)
                        .desired_width(f32::INFINITY),
                );
                ui.label(RichText::new("Password").strong());
                ui.add(
                    TextEdit::singleline(&mut self.draft_password)
                        .password(!self.show_password)
                        .hint_text("Leave blank to keep the saved password")
                        .desired_width(f32::INFINITY),
                );
                ui.checkbox(&mut self.show_password, "Show password");
                ui.checkbox(&mut self.draft_sync, "Sync when this computer is online");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let save_button =
                        Button::new(RichText::new("Save settings").color(CREAM)).fill(TERRACOTTA);
                    if ui.add(save_button).clicked() {
                        save = true;
                    }
                    if ui.button("Test connection").clicked() {
                        test = true;
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Sync now").clicked() {
                        sync_now = true;
                    }
                    if ui.button("Back up on the cloud").clicked() {
                        backup = true;
                    }
                });
                if self.needs_choice {
                    ui.add_space(6.0);
                    Frame::none()
                        .fill(Color32::from_rgb(255, 244, 230))
                        .stroke(Stroke::new(1.0, ORANGE))
                        .rounding(Rounding::same(8.0))
                        .inner_margin(Margin::same(10.0))
                        .show(ui, |ui| {
                            ui.label(RichText::new("Both copies changed").strong().color(INK));
                            ui.label(
                                RichText::new("Upload keeps this computer's maps and stores the cloud copy as a backup. Download replaces the maps here. A copy of what is here now is written into mindmap-data/conflicts.")
                                    .size(13.0)
                                    .color(INK),
                            );
                        });
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button("Upload this computer").clicked() {
                        upload = true;
                    }
                    if ui.button("Download the cloud copy").clicked() {
                        download = true;
                    }
                });
                if !self.message.is_empty() {
                    ui.add_space(6.0);
                    let color = if self.offline || self.needs_choice { ORANGE } else { TEAL };
                    ui.label(RichText::new(&self.message).size(13.0).color(color));
                }
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!(
                        "Encrypted settings: {}",
                        self.data_dir.join("cloud.enc").display()
                    ))
                    .size(11.0)
                    .color(MUTED),
                );
            });
        self.open = still_open;
        if save {
            self.save_drafts();
        } else if test {
            self.save_drafts_if_needed_then(Intent::Test);
        } else if sync_now {
            self.save_drafts_if_needed_then(Intent::Pull);
        } else if backup {
            self.save_drafts_if_needed_then(Intent::Backup);
        } else if upload {
            self.save_drafts_if_needed_then(Intent::ForcePush);
        } else if download {
            if self.save_drafts_if_needed_then_ready() {
                self.download_cloud();
            }
        }
    }

    /// Buttons in the window use the address on screen, saving it first when
    /// it differs from what is already stored.
    fn save_drafts_if_needed_then(&mut self, intent: Intent) {
        if !self.drafts_match_saved() && !self.save_drafts() {
            return;
        }
        if !self.settings.ready() && !self.save_drafts() {
            return;
        }
        self.request(intent);
    }

    fn save_drafts_if_needed_then_ready(&mut self) -> bool {
        if !self.drafts_match_saved() && !self.save_drafts() {
            return false;
        }
        if !self.settings.ready() {
            self.message = "Save the cloud address, username, and password first.".into();
            return false;
        }
        true
    }

    fn drafts_match_saved(&self) -> bool {
        let endpoint_same = cloud::normalize_endpoint(&self.draft_endpoint)
            .ok()
            .as_deref()
            == Some(self.settings.endpoint.as_str());
        endpoint_same
            && self.draft_username.trim() == self.settings.username
            && self.draft_password.is_empty()
            && self.draft_sync == self.settings.sync_enabled
    }

    fn download_cloud(&mut self) {
        if let Some(json) = self.conflict_json.clone() {
            self.effect = Some(CloudEffect::Install {
                revision: self.conflict_revision,
                json,
                archive: self.has_maps,
            });
            return;
        }
        self.request(Intent::PreferRemote);
    }
}

impl MindMapApp {
    fn apply_cloud_effect(&mut self, effect: CloudEffect) {
        let CloudEffect::Install {
            revision,
            json,
            archive,
        } = effect;
        if archive {
            if let Err(error) = cloud::archive_library(&self.data_dir, &self.library) {
                let message = format!("Could not keep a local copy ({error}).");
                self.cloud.message = message.clone();
                self.status = Some(message);
                return;
            }
        }
        match cloud::parse_library(&json) {
            Ok(library) => {
                self.library = library;
                self.undo.clear();
                self.cache = None;
                self.editing = None;
                self.selected = None;
                self.gesture = Gesture::None;
                self.confirm_delete = None;
                self.line_undo_key = None;
                self.deadline_draft = None;
                self.pending_edge = None;
                self.hover = None;
                self.hover_edge = None;
                if let Err(error) = store::save_library(&self.data_dir, &self.library) {
                    let message = format!("Could not save the cloud copy ({error}).");
                    self.cloud.message = message.clone();
                    self.status = Some(message);
                    return;
                }
                self.cloud.mark_synced(revision, &self.library);
                self.ensure_screen();
                self.pending_fit = true;
                self.status = Some("The cloud copy is now on this computer.".into());
            }
            Err(error) => {
                self.cloud.message = error.clone();
                self.status = Some(error);
            }
        }
    }
}

fn gear_button(ui: &mut egui::Ui) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let color = if response.hovered() { CREAM } else { CREAM_DIM };
    let center = rect.center();
    let painter = ui.painter_at(rect);
    for tooth in 0..8 {
        let angle = tooth as f32 * std::f32::consts::TAU / 8.0;
        let direction = vec2(angle.cos(), angle.sin());
        painter.line_segment(
            [center + direction * 5.4, center + direction * 9.2],
            Stroke::new(2.3, color),
        );
    }
    painter.circle_stroke(center, 6.0, Stroke::new(1.7, color));
    painter.circle_filled(center, 2.0, color);
    response
}
