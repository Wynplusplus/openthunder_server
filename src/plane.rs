//! Plane definitions live in directories under `planes/`, **or** embedded in a
//! mission file (`[plane <id>]` blocks) so a single mission is self-contained.
//!
//! A plane directory is `planes/<id>/` containing at least a `plane.conf`. Any
//! other files in the directory belong to that plane; a `*.glb`/`*.gltf` model
//! next to the config is embedded and shipped to clients.

use std::fs;
use std::path::{Path, PathBuf};

use crate::map::MapConfig;
use crate::protocol::PlaneWire;

/// A loaded plane: its id, `plane.conf` text and (optionally) its model.
#[derive(Clone, Debug)]
pub struct PlaneFile {
    pub id: String,
    pub config: String,
    /// Model file name (kept for the extension).
    pub model_name: Option<String>,
    /// Base64-encoded glTF/GLB bytes.
    pub model_data: Option<String>,
    pub dir: PathBuf,
}

/// Load every plane directory under `dir` that contains a `plane.conf`,
/// sorted by id. A glTF/GLB file in the directory is embedded.
pub fn load_dir(dir: &Path) -> std::io::Result<Vec<PlaneFile>> {
    let mut planes = Vec::new();
    if !dir.is_dir() {
        return Ok(planes);
    }
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .collect();
    entries.sort();
    for path in entries {
        let config_path = path.join("plane.conf");
        if !config_path.is_file() {
            continue;
        }
        let config = fs::read_to_string(&config_path)?;
        let id = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("plane")
            .to_string();
        let (model_name, model_data) = read_model(&path);
        planes.push(PlaneFile {
            id,
            config,
            model_name,
            model_data,
            dir: path,
        });
    }
    Ok(planes)
}

/// Find the first `*.glb`/`*.gltf` in `dir` and Base64-encode it.
fn read_model(dir: &Path) -> (Option<String>, Option<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return (None, None);
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    paths.sort();
    for path in paths {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();
        if extension != "glb" && extension != "gltf" {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("model.glb")
            .to_string();
        return (Some(name), Some(crate::base64::encode(&bytes)));
    }
    (None, None)
}

/// Planes embedded in a mission, so the server needs no `planes/` directory.
pub fn from_map(map: &MapConfig) -> Vec<PlaneFile> {
    map.planes
        .iter()
        .map(|plane| PlaneFile {
            id: plane.id.clone(),
            config: plane.config.clone(),
            model_name: plane.model_name.clone(),
            model_data: plane.model_data.clone(),
            dir: PathBuf::new(),
        })
        .collect()
}

/// The wire form of the plane list (config + optional model).
pub fn as_wire(planes: &[PlaneFile]) -> Vec<PlaneWire> {
    planes
        .iter()
        .map(|plane| PlaneWire {
            id: plane.id.clone(),
            config: plane.config.clone(),
            model_name: plane.model_name.clone(),
            model_data: plane.model_data.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_plane_directories() {
        // The repo ships three planes.
        let planes = load_dir(Path::new("planes")).expect("planes dir");
        assert!(
            planes.len() >= 3,
            "expected the built-in planes, found {}",
            planes.len()
        );
        for plane in &planes {
            assert!(plane.config.contains("name ="), "{} has no name", plane.id);
        }
    }

    #[test]
    fn builds_planes_from_a_mission() {
        let text = "name = M\n[plane custom]\nname = Custom\nmass = 1000\n[gun 0]\nname = G\n";
        let map = MapConfig::parse("m", text, PathBuf::from("m.map"));
        let planes = from_map(&map);
        assert_eq!(planes.len(), 1);
        assert_eq!(planes[0].id, "custom");
        assert!(planes[0].config.contains("mass = 1000"));
        assert!(planes[0].config.contains("[gun 0]"));
        let wire = as_wire(&planes);
        assert_eq!(wire[0].id, "custom");
    }
}
