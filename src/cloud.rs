//! Talks to the MindMap cloud: sign in, push maps, pull maps, and keep backups.
//!
//! The desktop program keeps its own copy in `mindmap-data`. When the cloud
//! address is set and this computer can reach it, changes go up and changes
//! made in the browser (or on another device) come back down. If the network
//! is down, edits stay on this computer and are sent the next time a sync
//! succeeds.

use crate::model::Library;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

const BODY_LIMIT: u64 = 20_000_000;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncState {
    pub client_id: String,
    pub base_revision: u64,
    /// Fingerprint of the library that last matched the cloud. Empty until the
    /// first successful sync, so a brand new install does not upload an empty
    /// map over a cloud that already has one.
    #[serde(default)]
    pub local_hash: String,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            client_id: Uuid::new_v4().to_string(),
            base_revision: 0,
            local_hash: String::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoteDecision {
    /// The cloud and this computer already match.
    Idle,
    /// Take the cloud copy.
    Replace,
    /// Send this computer's maps.
    Push,
    /// Both sides changed. The person has to choose.
    Conflict,
    /// The cloud is empty and so is this computer.
    MarkEmptySynced,
    /// A download was asked for, but the cloud has no map yet.
    NoRemote,
}

/// What to do after a pull. `prefer_remote` is the explicit "download the cloud
/// copy" action. Automatic sync never overwrites a computer that already has
/// maps of its own before the first successful sync.
pub fn decide_after_pull(
    state: &SyncState,
    dirty: bool,
    has_maps: bool,
    server_revision: u64,
    server_has_library: bool,
    prefer_remote: bool,
) -> RemoteDecision {
    if prefer_remote {
        return if server_has_library {
            RemoteDecision::Replace
        } else {
            RemoteDecision::NoRemote
        };
    }
    if !server_has_library {
        return if has_maps {
            RemoteDecision::Push
        } else {
            RemoteDecision::MarkEmptySynced
        };
    }
    if state.local_hash.is_empty() {
        return if has_maps {
            RemoteDecision::Conflict
        } else {
            RemoteDecision::Replace
        };
    }
    if server_revision == state.base_revision {
        return if dirty {
            RemoteDecision::Push
        } else {
            RemoteDecision::Idle
        };
    }
    if dirty {
        RemoteDecision::Conflict
    } else {
        RemoteDecision::Replace
    }
}

pub fn normalize_endpoint(input: &str) -> Result<String, String> {
    let trimmed = input.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("Enter the public address of the cloud.".into());
    }
    let rest = if let Some(rest) = trimmed.strip_prefix("https://") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        rest
    } else {
        return Err("The address should start with https://.".into());
    };
    let missing_dot = !rest.contains('.') && !is_local_host(rest);
    if rest.is_empty() || rest.contains(' ') || rest.contains('@') || missing_dot {
        return Err("That address does not look like a public cloud address.".into());
    }
    Ok(trimmed.to_string())
}

fn is_local_host(rest: &str) -> bool {
    let host = rest.split('/').next().unwrap_or(rest);
    let host = host.split(':').next().unwrap_or(host);
    host == "localhost" || host == "127.0.0.1"
}

pub fn library_json(library: &Library) -> String {
    serde_json::to_string(library).unwrap_or_else(|_| "{}".to_string())
}

pub fn library_fingerprint(library: &Library) -> String {
    sha256_hex(library_json(library).as_bytes())
}

pub fn parse_library(json: &str) -> Result<Library, String> {
    let mut library: Library = serde_json::from_str(json)
        .map_err(|error| format!("The cloud sent a map this program could not read ({error})."))?;
    library.repair();
    Ok(library)
}

pub fn load_sync_state(dir: &Path) -> SyncState {
    let path = sync_state_path(dir);
    let Ok(text) = fs::read_to_string(path) else {
        return SyncState::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save_sync_state(dir: &Path, state: &SyncState) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec_pretty(state).map_err(|error| error.to_string())?;
    let tmp = dir.join("sync-state.json.tmp");
    let path = sync_state_path(dir);
    fs::write(&tmp, bytes).map_err(|error| error.to_string())?;
    fs::rename(&tmp, path).map_err(|error| error.to_string())?;
    Ok(())
}

pub fn sync_state_path(dir: &Path) -> PathBuf {
    dir.join("sync-state.json")
}

/// Keep a readable copy before a cloud download replaces the maps on disk.
pub fn archive_library(dir: &Path, library: &Library) -> Result<PathBuf, String> {
    let folder = dir.join("conflicts");
    fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let name = format!(
        "before-download-{}-{}.json",
        Utc::now().format("%Y%m%dT%H%M%SZ"),
        Utc::now().timestamp_millis()
    );
    let path = folder.join(name);
    let bytes = serde_json::to_vec_pretty(library).map_err(|error| error.to_string())?;
    fs::write(&path, bytes).map_err(|error| error.to_string())?;
    Ok(path)
}

#[derive(Clone, Debug)]
pub enum JobKind {
    Test,
    Pull { prefer_remote: bool },
    Push {
        library_json: String,
        base_revision: u64,
        force: bool,
    },
    Backup,
}

#[derive(Clone, Debug)]
pub struct CloudJob {
    pub endpoint: String,
    pub username: String,
    pub password: String,
    pub client_id: String,
    pub kind: JobKind,
}

#[derive(Clone, Debug)]
pub enum CloudEvent {
    TestOk { detail: String },
    Synced {
        revision: u64,
        pushed_hash: String,
        detail: String,
    },
    Remote {
        revision: u64,
        library_json: Option<String>,
        prefer_remote: bool,
    },
    Conflict {
        revision: u64,
        library_json: String,
    },
    BackupOk { detail: String },
    Offline { detail: String },
    Failed { detail: String },
}

pub struct CloudSession {
    agent: ureq::Agent,
    token: Option<String>,
    token_key: String,
}

impl CloudSession {
    pub fn new() -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(5))
            .timeout(Duration::from_secs(25))
            .build();
        Self {
            agent,
            token: None,
            token_key: String::new(),
        }
    }

    pub fn perform(&mut self, job: &CloudJob) -> CloudEvent {
        if let Err(error) = self.ensure_token(job) {
            return fail_event(error);
        }
        match &job.kind {
            JobKind::Test => match self.pull(job) {
                Ok(remote) => CloudEvent::TestOk {
                    detail: match remote.library_json {
                        Some(_) => format!(
                            "Signed in as {}. The cloud is at revision {}.",
                            job.username, remote.revision
                        ),
                        None => format!(
                            "Signed in as {}. The cloud does not have a map yet.",
                            job.username
                        ),
                    },
                },
                Err(error) => fail_event(error),
            },
            JobKind::Pull { prefer_remote } => match self.pull(job) {
                Ok(remote) => CloudEvent::Remote {
                    revision: remote.revision,
                    library_json: remote.library_json,
                    prefer_remote: *prefer_remote,
                },
                Err(error) => fail_event(error),
            },
            JobKind::Push {
                library_json,
                base_revision,
                force,
            } => {
                let pushed_hash = sha256_hex(library_json.as_bytes());
                match self.push(job, library_json, *base_revision, *force) {
                    Ok(PushResult::Accepted { revision }) => CloudEvent::Synced {
                        revision,
                        pushed_hash,
                        detail: if *force {
                            format!("Uploaded this computer's maps. The cloud kept the previous copy. Revision {revision}.")
                        } else {
                            format!("Synced with the cloud. Revision {revision}.")
                        },
                    },
                    Ok(PushResult::Conflict {
                        revision,
                        library_json,
                    }) => CloudEvent::Conflict {
                        revision,
                        library_json,
                    },
                    Err(error) => fail_event(error),
                }
            }
            JobKind::Backup => match self.backup(job) {
                Ok(detail) => CloudEvent::BackupOk { detail },
                Err(error) => fail_event(error),
            },
        }
    }

    fn ensure_token(&mut self, job: &CloudJob) -> Result<(), CallError> {
        let key = format!("{}\n{}\n{}", job.endpoint, job.username, job.password);
        if self.token_key != key {
            self.token = None;
            self.token_key = key;
        }
        self.probe(&job.endpoint)?;
        if self.token.is_some() {
            return Ok(());
        }
        self.login(job)
    }

    fn probe(&self, endpoint: &str) -> Result<(), CallError> {
        match self.request("GET", &api(endpoint, "/api/v1/health"), None, None) {
            Ok((status, _)) if (200..300).contains(&status) => Ok(()),
            Ok((status, body)) => Err(CallError::Failed(format!(
                "The cloud answered {status}. {}",
                error_text(&body)
            ))),
            Err(error) => Err(error),
        }
    }

    fn login(&mut self, job: &CloudJob) -> Result<(), CallError> {
        let body = serde_json::json!({
            "username": job.username,
            "password": job.password,
        })
        .to_string();
        let (status, text) = self.request(
            "POST",
            &api(&job.endpoint, "/api/v1/login"),
            None,
            Some(body),
        )?;
        if status == 401 || status == 403 {
            return Err(CallError::Failed(
                "The username or password was not accepted.".into(),
            ));
        }
        if !(200..300).contains(&status) {
            return Err(CallError::Failed(format!(
                "Could not sign in ({status}). {}",
                error_text(&text)
            )));
        }
        let parsed: Value = serde_json::from_str(&text)
            .map_err(|_| CallError::Failed("The cloud sent a sign-in response that was not valid.".into()))?;
        let token = parsed
            .get("token")
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty())
            .ok_or_else(|| CallError::Failed("The cloud did not return a sign-in token.".into()))?;
        self.token = Some(token.to_string());
        Ok(())
    }

    fn pull(&mut self, job: &CloudJob) -> Result<RemoteBody, CallError> {
        let (status, text) = self.authed(job, "GET", "/api/v1/sync", None)?;
        if !(200..300).contains(&status) {
            return Err(CallError::Failed(format!(
                "Could not download the map ({status}). {}",
                error_text(&text)
            )));
        }
        parse_remote(&text).map_err(CallError::Failed)
    }

    fn push(
        &mut self,
        job: &CloudJob,
        library_json: &str,
        base_revision: u64,
        force: bool,
    ) -> Result<PushResult, CallError> {
        let library: Value = serde_json::from_str(library_json)
            .map_err(|_| CallError::Failed("Could not prepare the map for upload.".into()))?;
        let body = serde_json::json!({
            "base_revision": base_revision,
            "client_id": job.client_id,
            "force": force,
            "library": library,
        })
        .to_string();
        let (status, text) = self.authed(job, "POST", "/api/v1/sync", Some(body))?;
        if status == 409 {
            let remote = parse_remote(&text).map_err(CallError::Failed)?;
            let library_json = remote.library_json.ok_or_else(|| {
                CallError::Failed("The cloud reported a conflict but did not send its map.".into())
            })?;
            return Ok(PushResult::Conflict {
                revision: remote.revision,
                library_json,
            });
        }
        if !(200..300).contains(&status) {
            return Err(CallError::Failed(format!(
                "Could not upload the map ({status}). {}",
                error_text(&text)
            )));
        }
        let parsed: Value = serde_json::from_str(&text)
            .map_err(|_| CallError::Failed("The cloud sent an upload response that was not valid.".into()))?;
        let revision = parsed.get("revision").and_then(json_u64).ok_or_else(|| {
            CallError::Failed("The cloud did not say which revision it stored.".into())
        })?;
        Ok(PushResult::Accepted { revision })
    }

    fn backup(&mut self, job: &CloudJob) -> Result<String, CallError> {
        let body = serde_json::json!({
            "label": format!("From {}", job.client_id),
        })
        .to_string();
        let (status, text) = self.authed(job, "POST", "/api/v1/backups", Some(body))?;
        if status == 404 {
            return Err(CallError::Failed(
                "The cloud does not have a map to back up yet.".into(),
            ));
        }
        if !(200..300).contains(&status) {
            return Err(CallError::Failed(format!(
                "Could not store a backup ({status}). {}",
                error_text(&text)
            )));
        }
        let parsed: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        let id = parsed
            .get("id")
            .map(|value| value.to_string().trim_matches('"').to_string())
            .unwrap_or_else(|| "a new".into());
        Ok(format!("Backup {id} is stored on the cloud."))
    }

    fn authed(
        &mut self,
        job: &CloudJob,
        method: &str,
        path: &str,
        body: Option<String>,
    ) -> Result<(u16, String), CallError> {
        let token = self
            .token
            .clone()
            .ok_or_else(|| CallError::Failed("Not signed in.".into()))?;
        let url = api(&job.endpoint, path);
        match self.request(method, &url, Some(&token), body.clone()) {
            Ok((401, _)) => {
                self.token = None;
                self.login(job)?;
                let token = self.token.clone().unwrap_or_default();
                self.request(method, &url, Some(&token), body)
            }
            other => other,
        }
    }

    fn request(
        &self,
        method: &str,
        url: &str,
        token: Option<&str>,
        body: Option<String>,
    ) -> Result<(u16, String), CallError> {
        let mut request = self
            .agent
            .request(method, url)
            .set("Accept", "application/json")
            .set("User-Agent", "MindMap/1.0");
        if let Some(token) = token {
            request = request.set("Authorization", &format!("Bearer {token}"));
        }
        let result = if let Some(body) = body.as_ref() {
            request
                .set("Content-Type", "application/json")
                .send_string(body)
        } else {
            request.call()
        };
        match result {
            Ok(response) => {
                let status = response.status();
                Ok((status, read_limited(response.into_reader())?))
            }
            Err(ureq::Error::Status(status, response)) => {
                Ok((status, read_limited(response.into_reader())?))
            }
            Err(ureq::Error::Transport(error)) => Err(CallError::Offline(format!(
                "No connection to the cloud ({error})."
            ))),
        }
    }
}

impl Default for CloudSession {
    fn default() -> Self {
        Self::new()
    }
}

enum PushResult {
    Accepted { revision: u64 },
    Conflict { revision: u64, library_json: String },
}

struct RemoteBody {
    revision: u64,
    library_json: Option<String>,
}

enum CallError {
    Offline(String),
    Failed(String),
}

fn fail_event(error: CallError) -> CloudEvent {
    match error {
        CallError::Offline(detail) => CloudEvent::Offline { detail },
        CallError::Failed(detail) => CloudEvent::Failed { detail },
    }
}

fn api(endpoint: &str, path: &str) -> String {
    format!("{}{path}", endpoint.trim_end_matches('/'))
}

fn parse_remote(text: &str) -> Result<RemoteBody, String> {
    let parsed: Value = serde_json::from_str(text)
        .map_err(|_| "The cloud sent a response that was not valid.".to_string())?;
    let revision = parsed
        .get("revision")
        .and_then(json_u64)
        .ok_or_else(|| "The cloud did not include a revision.".to_string())?;
    let library_json = match parsed.get("library") {
        None | Some(Value::Null) => None,
        Some(value) => Some(value.to_string()),
    };
    Ok(RemoteBody {
        revision,
        library_json,
    })
}

fn json_u64(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        value
            .as_str()
            .and_then(|text| text.parse().ok())
    })
}

fn error_text(body: &str) -> String {
    let Ok(parsed) = serde_json::from_str::<Value>(body) else {
        return String::new();
    };
    parsed
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn read_limited(reader: impl Read) -> Result<String, CallError> {
    let mut buffer = Vec::new();
    reader
        .take(BODY_LIMIT)
        .read_to_end(&mut buffer)
        .map_err(|error| CallError::Failed(format!("Could not read the cloud response ({error}).")))?;
    String::from_utf8(buffer)
        .map_err(|_| CallError::Failed("The cloud response was not text.".into()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Library;

    fn state(revision: u64, hash: &str) -> SyncState {
        SyncState {
            client_id: "device".into(),
            base_revision: revision,
            local_hash: hash.into(),
        }
    }

    #[test]
    fn endpoint_accepts_https_and_rejects_a_password_in_the_address() {
        assert_eq!(
            normalize_endpoint(" https://maps.example.com/ ").unwrap(),
            "https://maps.example.com"
        );
        assert!(normalize_endpoint("https://user:secret@maps.example.com").is_err());
        assert!(normalize_endpoint("ftp://maps.example.com").is_err());
        assert_eq!(
            normalize_endpoint("http://127.0.0.1:3000").unwrap(),
            "http://127.0.0.1:3000"
        );
    }

    #[test]
    fn first_sync_does_not_upload_empty_maps_over_a_cloud_copy() {
        let fresh = state(0, "");
        assert_eq!(
            decide_after_pull(&fresh, false, false, 4, true, false),
            RemoteDecision::Replace
        );
        assert_eq!(
            decide_after_pull(&fresh, false, true, 4, true, false),
            RemoteDecision::Conflict
        );
        assert_eq!(
            decide_after_pull(&fresh, false, true, 0, false, false),
            RemoteDecision::Push
        );
        assert_eq!(
            decide_after_pull(&fresh, false, false, 0, false, false),
            RemoteDecision::MarkEmptySynced
        );
    }

    #[test]
    fn later_sync_pushes_local_edits_and_pulls_cloud_edits() {
        let synced = state(3, "abc");
        assert_eq!(
            decide_after_pull(&synced, true, true, 3, true, false),
            RemoteDecision::Push
        );
        assert_eq!(
            decide_after_pull(&synced, false, true, 4, true, false),
            RemoteDecision::Replace
        );
        assert_eq!(
            decide_after_pull(&synced, true, true, 4, true, false),
            RemoteDecision::Conflict
        );
        assert_eq!(
            decide_after_pull(&synced, false, true, 3, true, false),
            RemoteDecision::Idle
        );
        assert_eq!(
            decide_after_pull(&synced, true, true, 9, true, true),
            RemoteDecision::Replace
        );
    }

    #[test]
    fn fingerprint_follows_the_map_and_sync_state_round_trips() {
        let library = Library::new();
        let hash = library_fingerprint(&library);
        assert_eq!(hash, library_fingerprint(&Library::new()));
        assert_eq!(hash.len(), 64);
        let dir = std::env::temp_dir().join(format!("mindmap-cloud-{}", Uuid::new_v4()));
        let state = SyncState {
            client_id: "device-1".into(),
            base_revision: 7,
            local_hash: hash,
        };
        save_sync_state(&dir, &state).unwrap();
        assert_eq!(load_sync_state(&dir), state);
        let _ = fs::remove_dir_all(&dir);
    }
}
