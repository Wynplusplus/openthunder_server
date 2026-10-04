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
    /// Team index (0/1), or [`crate::protocol::NO_TEAM`].
    pub team: u8,
    pub kills: u32,
    pub deaths: u32,
}

/// Context passed to [`GameMode::on_tick`].
pub struct TickContext<'a> {
    pub map: &'a MapConfig,
    pub players: &'a [PlayerView],
    pub elapsed_secs: f32,
    pub tick: u64,
}

/// Match state a scoring gamemode wants broadcast.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchState {
    /// Kills per team.
    pub scores: Vec<u32>,
    /// Kills needed to win.
    pub score_limit: u32,
    /// Seconds left in the round.
    pub time_left: f32,
}

/// What a gamemode wants the server to do after a tick.
#[derive(Default, Debug)]
pub struct TickOutcome {
    /// End the round (the server resets scores and the mode restarts).
    pub round_over: bool,
    /// Match state to broadcast, if this mode scores.
    pub match_state: Option<MatchState>,
}

/// A configurable rule set.
pub trait GameMode: Send + Sync {
    /// Stable id used in map files.
    fn id(&self) -> &'static str;
    /// Human-readable name.
    fn name(&self) -> &'static str;
    /// Rules this mode understands (for docs / validation).
    fn known_rules(&self) -> &'static [&'static str];
    /// Whether the server should split players into teams (TDM).
    fn uses_teams(&self) -> bool {
        false
    }
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

/// Team deathmatch: two teams race to a kill target, or the clock runs out.
pub struct TeamDeathmatch {
    score_limit: u32,
    time_limit: f32,
    /// `elapsed_secs` at which the current round started.
    round_start: f32,
}

impl Default for TeamDeathmatch {
    fn default() -> Self {
        Self {
            score_limit: 50,
            time_limit: 900.0,
            round_start: 0.0,
        }
    }
}

impl GameMode for TeamDeathmatch {
    fn id(&self) -> &'static str {
        "team_deathmatch"
    }
    fn name(&self) -> &'static str {
        "Team Deathmatch"
    }
    fn known_rules(&self) -> &'static [&'static str] {
        &["max_players", "score_limit", "time_limit"]
    }
    fn uses_teams(&self) -> bool {
        true
    }
    fn on_start(&mut self, map: &MapConfig) {
        self.score_limit = map.rule_u32("score_limit", 50);
        self.time_limit = map.rule_f32("time_limit", 900.0);
        self.round_start = 0.0;
        println!(
            "[gamemode] team_deathmatch on '{}' (score_limit={}, time_limit={:.0}s, 2 teams)",
            map.name, self.score_limit, self.time_limit
        );
    }
    fn on_tick(&mut self, ctx: &TickContext) -> TickOutcome {
        let mut scores = vec![0u32; 2];
        for player in ctx.players {
            if (player.team as usize) < scores.len() {
                scores[player.team as usize] += player.kills;
            }
        }
        let elapsed = (ctx.elapsed_secs - self.round_start).max(0.0);
        let time_left = (self.time_limit - elapsed).max(0.0);
        let round_over = time_left <= 0.0 || scores.iter().any(|score| *score >= self.score_limit);
        if round_over {
            // Restart the clock; the server clears the scores.
            self.round_start = ctx.elapsed_secs;
        }
        TickOutcome {
            round_over,
            match_state: Some(MatchState {
                scores,
                score_limit: self.score_limit,
                time_left,
            }),
        }
    }
}

/// Construct a gamemode by id, or `None` if it is not registered.
pub fn create(id: &str) -> Option<Box<dyn GameMode>> {
    match id {
        "free_flight" => Some(Box::new(FreeFlight)),
        "team_deathmatch" => Some(Box::new(TeamDeathmatch::default())),
        _ => None,
    }
}

/// Every registered gamemode id.
pub fn registered_ids() -> Vec<&'static str> {
    vec!["free_flight", "team_deathmatch"]
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
        let outcome = mode.on_tick(&ctx);
        assert!(!outcome.round_over);
        assert!(outcome.match_state.is_none(), "free flight does not score");
    }

    fn player(id: u64, team: u8, kills: u32) -> PlayerView {
        PlayerView {
            id,
            name: format!("P{id}"),
            plane: "Test".into(),
            position: [0.0; 3],
            team,
            kills,
            deaths: 0,
        }
    }

    #[test]
    fn team_deathmatch_scores_by_team() {
        let map = MapConfig::parse(
            "t",
            "score_limit = 3\ntime_limit = 600\n",
            PathBuf::from("t.map"),
        );
        let mut mode = create("team_deathmatch").unwrap();
        mode.on_start(&map);
        let players = vec![player(1, 0, 2), player(2, 1, 1), player(3, 0, 1)];
        let ctx = TickContext {
            map: &map,
            players: &players,
            elapsed_secs: 1.0,
            tick: 1,
        };
        let outcome = mode.on_tick(&ctx);
        let state = outcome.match_state.expect("tdm scores");
        assert_eq!(state.scores, vec![3, 1]);
        assert!(
            outcome.round_over,
            "reaching the score limit ends the round"
        );
    }

    #[test]
    fn team_deathmatch_ends_when_time_runs_out() {
        let map = MapConfig::parse(
            "t",
            "score_limit = 50\ntime_limit = 10\n",
            PathBuf::from("t.map"),
        );
        let mut mode = create("team_deathmatch").unwrap();
        mode.on_start(&map);
        let players = Vec::new();
        let ctx = TickContext {
            map: &map,
            players: &players,
            elapsed_secs: 11.0,
            tick: 1,
        };
        let outcome = mode.on_tick(&ctx);
        assert!(outcome.round_over);
        assert_eq!(outcome.match_state.unwrap().time_left, 0.0);
    }
}
