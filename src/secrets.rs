//! Encrypted cloud settings stored beside the maps.
//!
//! ```text
//! mindmap-data/
//!   cloud.key     random key created on this computer
//!   cloud.enc     endpoint, username, and password
//! ```
//!
//! The password is not written as plain text. The key file stays in the data
//! folder so the program can open the settings on the next launch. Copy both
//! files together with the maps if you move the folder to another computer.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use hkdf::Hkdf;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::fs;
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 4] = b"MMCS";
const VERSION: u8 = 1;
const INFO: &[u8] = b"mindmap-cloud-settings-v1";

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CloudSettings {
    pub endpoint: String,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub sync_enabled: bool,
}

impl Default for CloudSettings {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            username: String::new(),
            password: String::new(),
            sync_enabled: true,
        }
    }
}

impl std::fmt::Debug for CloudSettings {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CloudSettings")
            .field("endpoint", &self.endpoint)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .field("sync_enabled", &self.sync_enabled)
            .finish()
    }
}

impl CloudSettings {
    pub fn ready(&self) -> bool {
        !self.endpoint.is_empty() && !self.username.is_empty() && !self.password.is_empty()
    }
}

pub fn settings_path(dir: &Path) -> PathBuf {
    dir.join("cloud.enc")
}

pub fn load_settings(dir: &Path) -> Result<Option<CloudSettings>, String> {
    let path = settings_path(dir);
    if !path.exists() {
        return Ok(None);
    }
    let blob = fs::read(&path).map_err(|error| error.to_string())?;
    let key = read_key(dir)?;
    let plain = open_blob(&key, &blob)?;
    let settings = serde_json::from_slice::<CloudSettings>(&plain)
        .map_err(|error| format!("Could not read the cloud settings ({error})."))?;
    Ok(Some(settings))
}

pub fn save_settings(dir: &Path, settings: &CloudSettings) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let key = install_key(dir)?;
    let plain = serde_json::to_vec(settings).map_err(|error| error.to_string())?;
    let blob = seal_blob(&key, &plain)?;
    let path = settings_path(dir);
    let tmp = dir.join("cloud.enc.tmp");
    fs::write(&tmp, &blob).map_err(|error| error.to_string())?;
    fs::rename(&tmp, &path).map_err(|error| error.to_string())?;
    set_private(&path);
    Ok(())
}

fn key_path(dir: &Path) -> PathBuf {
    dir.join("cloud.key")
}

fn read_key(dir: &Path) -> Result<Vec<u8>, String> {
    let path = key_path(dir);
    let bytes = fs::read(&path).map_err(|_| {
        "Could not read cloud.key, so the saved cloud settings cannot be opened.".to_string()
    })?;
    if bytes.len() < 32 {
        return Err("cloud.key is incomplete, so the saved cloud settings cannot be opened.".into());
    }
    Ok(bytes)
}

fn install_key(dir: &Path) -> Result<Vec<u8>, String> {
    let path = key_path(dir);
    if path.exists() {
        return read_key(dir);
    }
    let mut bytes = vec![0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    fs::write(&path, &bytes).map_err(|error| error.to_string())?;
    set_private(&path);
    Ok(bytes)
}

fn seal_blob(key_file: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let mut salt = [0u8; 16];
    let mut nonce_bytes = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut salt);
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let key = derive_key(key_file, &salt);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|error| error.to_string())?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext)
        .map_err(|_| "Could not encrypt the cloud settings.".to_string())?;
    let mut blob = Vec::with_capacity(4 + 1 + 16 + 12 + ciphertext.len());
    blob.extend_from_slice(MAGIC);
    blob.push(VERSION);
    blob.extend_from_slice(&salt);
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ciphertext);
    Ok(blob)
}

fn open_blob(key_file: &[u8], blob: &[u8]) -> Result<Vec<u8>, String> {
    if blob.len() < 4 + 1 + 16 + 12 + 16 || &blob[0..4] != MAGIC {
        return Err("The cloud settings file is not a MindMap settings file.".into());
    }
    if blob[4] != VERSION {
        return Err("The cloud settings file is from a newer MindMap.".into());
    }
    let salt = &blob[5..21];
    let nonce = &blob[21..33];
    let ciphertext = &blob[33..];
    let key = derive_key(key_file, salt);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|error| error.to_string())?;
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| "Could not decrypt the cloud settings.".to_string())
}

fn derive_key(key_file: &[u8], salt: &[u8]) -> [u8; 32] {
    let hkdf = Hkdf::<Sha256>::new(Some(salt), key_file);
    let mut key = [0u8; 32];
    hkdf.expand(INFO, &mut key)
        .expect("cloud settings key length is valid");
    key
}

fn set_private(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_the_file_hides_the_password() {
        let dir = std::env::temp_dir().join(format!("mindmap-secrets-{}", uuid::Uuid::new_v4()));
        let _ = fs::remove_dir_all(&dir);
        let settings = CloudSettings {
            endpoint: "https://maps.example.com".into(),
            username: "ada".into(),
            password: "correct horse battery".into(),
            sync_enabled: true,
        };
        save_settings(&dir, &settings).unwrap();
        let stored = fs::read(settings_path(&dir)).unwrap();
        let as_text = String::from_utf8_lossy(&stored);
        assert!(!as_text.contains("correct horse battery"));
        assert!(!as_text.contains("ada"));
        assert_eq!(load_settings(&dir).unwrap().as_ref(), Some(&settings));

        let mut tampered = stored.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x5a;
        fs::write(settings_path(&dir), tampered).unwrap();
        assert!(load_settings(&dir).is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
