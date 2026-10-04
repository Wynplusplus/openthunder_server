//! Gamemodes.
//!
//! A gamemode is a configurable rule set the server ticks. The default is
//! [`FreeFlight`] (no objectives). Adding a new one is two steps:
//!
//! 1. Implement [`GameMode`] for a new type.
//! 2. Register it in [`create`] and [`registered_ids`].
//!
//! Maps choose a gamemode by id and supply its rules (see `map.rs`), so the same
//! mode can behave differently on different maps.

use crate::map::MapConfig;

/// Read-only view of a player, handed to gamemodes.
#[derive(Clone, Debug)]
pub struct PlayerView {
    pub id: u64,
    pub name: String,
    pub plane: String,
    pub position: [f32; 3],
}

/// Context passed to [`GameMode::on_tick`].
pub struct TickContext<'a> {
    pub map: &'a MapConfig,
    pub players: &'a [PlayerView],
    pub elapsed_secs: f32,
    pub tick: u64,
}

/// What a gamemode wants the server to do after a tick.
#[derive(Default, Debug)]
pub struct TickOutcome {
    /// End the round (the server may reset scores, restart, ...).
    pub round_over: bool,
}

/// A configurable rule set.
pub trait GameMode: Send + Sync {
    /// Stable id used in map files.
    fn id(&self) -> &'static str;
    /// Human-readable name.
    fn name(&self) -> &'static str;
    /// Rules this mode understands (for docs / validation).
    fn known_rules(&self) -> &'static [&'static str];
    /// Called once when the mode starts, with the map's rules.
    fn on_start(&mut self, map: &MapConfig);
    /// Called every server tick.
    fn on_tick(&mut self, ctx: &TickContext) -> TickOutcome;
}

/// The default mode: no objectives, just fly around.
#[derive(Default)]
pub struct FreeFlight;

impl GameMode for FreeFlight {
    fn id(&self) -> &'static str {
        "free_flight"
    }
    fn name(&self) -> &'static str {
        "Free Flight"
    }
    fn known_rules(&self) -> &'static [&'static str] {
        &["max_players"]
    }
    fn on_start(&mut self, map: &MapConfig) {
        println!(
            "[gamemode] free_flight on '{}' (max_players={})",
            map.name,
            map.rule_u32("max_players", 16)
        );
    }
    fn on_tick(&mut self, _ctx: &TickContext) -> TickOutcome {
        TickOutcome::default()
    }
}

/// Construct a gamemode by id, or `None` if it is not registered.
pub fn create(id: &str) -> Option<Box<dyn GameMode>> {
    match id {
        "free_flight" => Some(Box::new(FreeFlight)),
        _ => None,
    }
}

/// Every registered gamemode id.
pub fn registered_ids() -> Vec<&'static str> {
    vec!["free_flight"]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn free_flight_is_registered() {
        assert!(create("free_flight").is_some());
        assert!(create("does_not_exist").is_none());
        assert!(registered_ids().contains(&"free_flight"));
    }

    #[test]
    fn tick_does_not_end_the_round() {
        let map = MapConfig::parse("t", "name = T", PathBuf::from("t.map"));
        let mut mode = create("free_flight").unwrap();
        mode.on_start(&map);
        let players = Vec::new();
        let ctx = TickContext {
            map: &map,
            players: &players,
            elapsed_secs: 0.0,
            tick: 0,
        };
        assert!(!mode.on_tick(&ctx).round_over);
    }
}
