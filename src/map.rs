//! Maps.
//!
//! A map is a small `name = value` file under `maps/`. It names the map and the
//! gamemode to run, and carries that gamemode's **rules** — so the same gamemode
//! can be configured differently per map, or a map can pick a different mode.
//!
//! Example `maps/training.map`:
//!
//! ```text
//! name = Training Islands
//! gamemode = free_flight
//! max_players = 16
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A parsed map definition.
#[derive(Clone, Debug)]
pub struct MapConfig {
    /// File stem, e.g. `training`.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Gamemode id (see `gamemode.rs`).
    pub gamemode: String,
    /// Gamemode rules, as raw key/value pairs the gamemode interprets.
    pub rules: BTreeMap<String, String>,
    /// Where it was loaded from.
    pub source: PathBuf,
}

impl MapConfig {
    pub fn parse(id: &str, text: &str, source: PathBuf) -> Self {
        let mut name = id.to_string();
        let mut gamemode = "free_flight".to_string();
        let mut rules = BTreeMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim();
            match key {
                "name" => name = value.to_string(),
                "gamemode" => gamemode = value.to_string(),
                other => {
                    rules.insert(other.to_string(), value.to_string());
                }
            }
        }
        Self {
            id: id.to_string(),
            name,
            gamemode,
            rules,
            source,
        }
    }

    pub fn load(path: &Path) -> std::io::Result<Self> {
        let text = fs::read_to_string(path)?;
        let id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("map")
            .to_string();
        Ok(Self::parse(&id, &text, path.to_path_buf()))
    }

    pub fn rule(&self, key: &str) -> Option<&str> {
        self.rules.get(key).map(String::as_str)
    }

    pub fn rule_u32(&self, key: &str, default: u32) -> u32 {
        self.rule(key)
            .and_then(|value| value.parse().ok())
            .unwrap_or(default)
    }

    pub fn rule_f32(&self, key: &str, default: f32) -> f32 {
        self.rule(key)
            .and_then(|value| value.parse().ok())
            .unwrap_or(default)
    }
}

/// Load every `*.map` file in `dir`, sorted by filename.
pub fn load_dir(dir: &Path) -> std::io::Result<Vec<MapConfig>> {
    let mut maps = Vec::new();
    if !dir.is_dir() {
        return Ok(maps);
    }
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().map(|ext| ext == "map").unwrap_or(false))
        .collect();
    entries.sort();
    for path in entries {
        maps.push(MapConfig::load(&path)?);
    }
    Ok(maps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_name_gamemode_and_rules() {
        let text = "# comment\nname = Training Islands\ngamemode = free_flight\nmax_players = 24\ntime_limit = 600\n";
        let map = MapConfig::parse("training", text, PathBuf::from("training.map"));
        assert_eq!(map.name, "Training Islands");
        assert_eq!(map.gamemode, "free_flight");
        assert_eq!(map.rule_u32("max_players", 0), 24);
        assert_eq!(map.rule_f32("time_limit", 0.0), 600.0);
    }

    #[test]
    fn defaults_when_keys_missing() {
        let map = MapConfig::parse("blank", "", PathBuf::from("blank.map"));
        assert_eq!(map.name, "blank");
        assert_eq!(map.gamemode, "free_flight");
        assert_eq!(map.rule_u32("max_players", 8), 8);
    }
}
