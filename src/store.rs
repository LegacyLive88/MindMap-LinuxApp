//! Plain JSON stored in a folder next to the executable.
//!
//! ```text
//! mindmap                  the program
//! mindmap-data/
//!   manifest.json          canvas order and the last map you had open
//!   canvases/<id>.json     one file per canvas
//! ```

use crate::model::{Canvas, Id, Library, LIBRARY_VERSION};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Loaded {
    pub library: Library,
    pub warning: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    version: u32,
    root_order: Vec<Id>,
    #[serde(default)]
    last_open: Option<Id>,
}

pub fn default_data_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|dir| dir.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("mindmap-data")
}

pub fn load_library(dir: &Path) -> Loaded {
    let mut warning = None;
    let mut library = Library::new();
    let manifest_path = dir.join("manifest.json");
    if manifest_path.exists() {
        match fs::read_to_string(&manifest_path) {
            Ok(text) => match serde_json::from_str::<Manifest>(&text) {
                Ok(manifest) => {
                    library.version = manifest.version;
                    library.root_order = manifest.root_order;
                    library.last_open = manifest.last_open;
                }
                Err(error) => {
                    warning = Some(format!("Could not read manifest.json ({error})."));
                }
            },
            Err(error) => {
                warning = Some(format!("Could not read manifest.json ({error})."));
            }
        }
    }

    let canvas_dir = dir.join("canvases");
    if canvas_dir.exists() {
        match fs::read_dir(&canvas_dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                        continue;
                    }
                    match fs::read_to_string(&path).and_then(|text| {
                        serde_json::from_str::<Canvas>(&text)
                            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
                    }) {
                        Ok(canvas) => {
                            library.canvases.insert(canvas.id.clone(), canvas);
                        }
                        Err(error) => {
                            let name = path.display().to_string();
                            let message = format!("Skipped {name} ({error}).");
                            warning = Some(match warning {
                                Some(previous) => format!("{previous} {message}"),
                                None => message,
                            });
                        }
                    }
                }
            }
            Err(error) => {
                warning = Some(format!("Could not read {} ({error}).", canvas_dir.display()));
            }
        }
    }

    library.repair();
    Loaded { library, warning }
}

pub fn save_library(dir: &Path, library: &Library) -> Result<(), String> {
    let canvas_dir = dir.join("canvases");
    fs::create_dir_all(&canvas_dir).map_err(|error| error.to_string())?;

    let manifest = Manifest {
        version: LIBRARY_VERSION.max(library.version),
        root_order: library.root_order.clone(),
        last_open: library.last_open.clone(),
    };
    atomic_write(
        &dir.join("manifest.json"),
        &serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?,
    )?;

    let mut keep = HashSet::new();
    for canvas in library.canvases.values() {
        let name = format!("{}.json", canvas.id);
        keep.insert(name.clone());
        atomic_write(
            &canvas_dir.join(name),
            &serde_json::to_vec_pretty(canvas).map_err(|error| error.to_string())?,
        )?;
    }

    if let Ok(entries) = fs::read_dir(&canvas_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".json") && !keep.contains(&name) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, bytes).map_err(|error| error.to_string())?;
    fs::rename(&tmp, path).map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Library;
    use chrono::{Duration, TimeZone, Utc};

    #[test]
    fn round_trip_keeps_canvases_links_and_nested_maps() {
        let now = Utc.with_ymd_and_hms(2026, 5, 1, 8, 30, 0).unwrap();
        let mut library = Library::new();
        let root = library.create_root("Garden", now);
        let center = library.canvas(&root).unwrap().center_id.clone();
        let bed = library.add_child_node(&root, &center, now).unwrap();
        let path = library
            .add_child_node(&root, &center, now + Duration::seconds(1))
            .unwrap();
        assert!(library.set_text(&root, &bed, "Herb bed".into(), now + Duration::minutes(5)));
        assert!(library.add_link(&root, &bed, &path, now + Duration::minutes(6)));
        let nested = library
            .open_or_create_child_canvas(&root, &bed, now + Duration::minutes(7))
            .unwrap();
        library.last_open = Some(nested.clone());
        library.repair();

        let dir = std::env::temp_dir().join(format!("mindmap-store-{}", crate::model::new_id()));
        let _ = fs::remove_dir_all(&dir);
        save_library(&dir, &library).unwrap();
        let loaded = load_library(&dir);
        assert!(loaded.warning.is_none(), "{:?}", loaded.warning);
        assert_eq!(loaded.library, library);

        assert!(library.delete_node(&root, &bed, now + Duration::minutes(8)));
        save_library(&dir, &library).unwrap();
        let loaded = load_library(&dir);
        assert!(loaded.library.canvas(&root).unwrap().node(&bed).is_none());
        assert!(loaded.library.canvas(&nested).is_some());
        assert!(loaded.library.root_order.iter().any(|id| id == &nested));
        assert!(dir.join("canvases").join(format!("{nested}.json")).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tasks_and_constraints_round_trip_and_old_files_still_open() {
        let now = Utc.with_ymd_and_hms(2026, 5, 1, 8, 30, 0).unwrap();
        let mut library = Library::new();
        let root = library.create_root("Garden", now);
        let center = library.canvas(&root).unwrap().center_id.clone();
        let bed = library.add_child_node(&root, &center, now).unwrap();
        assert!(library.set_text(&root, &bed, "Herb bed".into(), now));
        let edge = library.canvas(&root).unwrap().edges[0].id.clone();
        let task = library.add_task(&root, &edge, now).unwrap();
        assert!(library.set_task_text(&root, &edge, &task, "Water".into(), now));
        assert!(library.set_task_deadline(
            &root,
            &edge,
            &task,
            Some(chrono::NaiveDate::from_ymd_opt(2026, 5, 4).unwrap()),
            now,
        ));
        let constraint = library.add_constraint(&root, &edge, now).unwrap();
        assert!(library.set_constraint_text(&root, &edge, &constraint, "Hose".into(), now));
        assert!(library.set_constrained_percent(&root, &edge, Some(25), now));

        let dir = std::env::temp_dir().join(format!("mindmap-lines-{}", crate::model::new_id()));
        let _ = fs::remove_dir_all(&dir);
        save_library(&dir, &library).unwrap();
        let loaded = load_library(&dir);
        assert!(loaded.warning.is_none(), "{:?}", loaded.warning);
        let loaded_edge = loaded
            .library
            .canvas(&root)
            .unwrap()
            .edges
            .iter()
            .find(|item| item.id == edge)
            .unwrap();
        assert_eq!(loaded_edge.tasks[0].text, "Water");
        assert_eq!(loaded_edge.free_percent(), Some(75));

        let old = dir.join("canvases").join("legacy.json");
        fs::write(
            &old,
            r#"{
                "id": "legacy",
                "center_id": "legacy-center",
                "nodes": [{
                    "id": "legacy-center",
                    "text": "Already here",
                    "created_at": "2024-01-01T00:00:00Z",
                    "modified_at": "2024-02-01T00:00:00Z"
                }],
                "edges": []
            }"#,
        )
        .unwrap();
        let loaded = load_library(&dir);
        assert!(loaded.warning.is_none(), "{:?}", loaded.warning);
        assert_eq!(
            loaded.library.canvas("legacy").unwrap().center_text(),
            "Already here"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
