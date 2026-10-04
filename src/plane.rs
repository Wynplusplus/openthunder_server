//! Plane definitions live in directories under `planes/`.
//!
//! Each plane is `planes/<id>/` containing at least a `plane.conf` (flight
//! model + model + armament). Any other files in the directory (models, notes,
//! ...) belong to that plane.
//!
//! Adding a plane to a server is just creating a new directory with a
//! `plane.conf`; the server ships the file to clients on connect and the client
//! parses it (see the game's `openthunder::plane_config`).

use std::fs;
use std::path::{Path, PathBuf};

/// A loaded plane: its directory id and the raw `plane.conf` text.
#[derive(Clone, Debug)]
pub struct PlaneFile {
    pub id: String,
    pub config: String,
    pub dir: PathBuf,
}

/// Load every plane directory under `dir` that contains a `plane.conf`,
/// sorted by id.
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
        planes.push(PlaneFile {
            id,
            config,
            dir: path,
        });
    }
    Ok(planes)
}

/// The `(id, plane.conf text)` pairs the protocol sends to clients.
pub fn as_pairs(planes: &[PlaneFile]) -> Vec<(String, String)> {
    planes
        .iter()
        .map(|plane| (plane.id.clone(), plane.config.clone()))
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
}
