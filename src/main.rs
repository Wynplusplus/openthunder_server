//! OpenThunder dedicated server.
//!
//! Usage:
//!
//! ```sh
//! cargo run --release -- --bind 0.0.0.0:7777 --map training
//! ```
//!
//! Maps live in `maps/*.map` and choose the gamemode and its rules.

mod gamemode;
mod map;
mod protocol;
mod server;

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bind = arg_value(&args, "--bind").unwrap_or_else(|| "0.0.0.0:7777".to_string());
    let maps_dir = PathBuf::from(arg_value(&args, "--maps").unwrap_or_else(|| "maps".to_string()));
    let requested_map = arg_value(&args, "--map");

    let maps = match map::load_dir(&maps_dir) {
        Ok(maps) => maps,
        Err(err) => {
            eprintln!("[server] could not read maps dir '{}': {err}", maps_dir.display());
            std::process::exit(1);
        }
    };
    if maps.is_empty() {
        eprintln!("[server] no *.map files found in '{}'", maps_dir.display());
        std::process::exit(1);
    }

    let selected = match &requested_map {
        Some(wanted) => maps
            .iter()
            .find(|map| &map.id == wanted || &map.name == wanted)
            .cloned(),
        None => Some(maps[0].clone()),
    };
    let Some(map) = selected else {
        let available = maps
            .iter()
            .map(|map| map.id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!("[server] map '{requested_map:?}' not found (available: {available})");
        std::process::exit(1);
    };

    println!("OpenThunder dedicated server");
    if let Err(err) = server::run(&bind, map) {
        eprintln!("[server] fatal: {err}");
        std::process::exit(1);
    }
}

/// Returns the value following `flag`, if present.
fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1).cloned())
}
