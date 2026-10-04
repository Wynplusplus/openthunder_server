//! OpenThunder dedicated server.
//!
//! Usage:
//!
//! ```sh
//! cargo run --release -- --bind 0.0.0.0:7777 --map training
//! ```
//!
//! Maps live in `maps/*.map` and choose the gamemode and its rules.

mod crew;
mod gamemode;
mod map;
mod plane;
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
            eprintln!(
                "[server] could not read maps dir '{}': {err}",
                maps_dir.display()
            );
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

    // Pilot crew skills, shipped to every client that connects.
    let crew_path =
        PathBuf::from(arg_value(&args, "--crew").unwrap_or_else(|| "crew.conf".to_string()));
    let crew = crew::load(&crew_path);
    match crew::value(&crew, "g_tolerance") {
        Some(g) => println!(
            "[server] crew from '{}': pilot tolerates {g:.1} g",
            crew_path.display()
        ),
        None => println!(
            "[server] crew from '{}' (no g_tolerance set; clients keep defaults)",
            crew_path.display()
        ),
    }

    // Load the planes so clients can fetch them on connect.
    let planes_dir =
        PathBuf::from(arg_value(&args, "--planes").unwrap_or_else(|| "planes".to_string()));
    let planes = match plane::load_dir(&planes_dir) {
        Ok(planes) => planes,
        Err(err) => {
            eprintln!(
                "[server] could not read planes dir '{}': {err}",
                planes_dir.display()
            );
            std::process::exit(1);
        }
    };
    if planes.is_empty() {
        eprintln!("[server] no planes found in '{}'", planes_dir.display());
        std::process::exit(1);
    }
    println!(
        "[server] planes: {}",
        planes
            .iter()
            .map(|plane| plane.id.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    if let Err(err) = server::run(&bind, map, plane::as_pairs(&planes), crew) {
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
