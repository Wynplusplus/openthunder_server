//! Server-side pilot crew configuration.
//!
//! A `crew.conf` (`key = value`) sets the pilot model used by **every client on
//! this server**. It is shipped verbatim to clients when they connect, so a
//! server can run anything from forgiving arcade pilots to strict realistic
//! ones — e.g. `g_tolerance = 4.0` for a harsher server.
//!
//! Recognised keys (parsed by the game's `pilot.rs`): `g_tolerance`,
//! `negative_g_tolerance`, `blackout_rate`, `recovery_rate`, `stamina_drain`,
//! `stamina_recovery`. Unknown keys are ignored.

use std::fs;
use std::path::Path;

/// Shipped when `crew.conf` is missing; matches the game's built-in defaults.
pub const DEFAULT: &str = "\
# Pilot crew skills, shipped to every client on this server.
# Positive / negative g the pilot can hold before blacking / redding out.
g_tolerance = 6.5
negative_g_tolerance = -3.0
# Blackout gained per g above the tolerance, per second.
blackout_rate = 0.18
# Vision recovered per second once the g comes off.
recovery_rate = 0.4
# Stamina drained per g above 3, per second.
stamina_drain = 0.012
# Stamina recovered per second below 3 g.
stamina_recovery = 0.1
# Spotting: aircraft are drawn within render_distance; enemies get a marker
# within detection_range inside the view cone, or always within awareness_range.
render_distance = 9000
detection_range = 7000
awareness_range = 1500
view_cone_deg = 25
";

/// Load `crew.conf`, falling back to [`DEFAULT`] when it cannot be read.
pub fn load(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|_| DEFAULT.to_string())
}

/// Read a numeric value out of a crew config, for logging.
pub fn value(text: &str, key: &str) -> Option<f32> {
    text.lines().find_map(|line| {
        let (k, v) = line.split_once('=')?;
        if k.trim() != key {
            return None;
        }
        v.trim().parse::<f32>().ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_parses() {
        assert_eq!(value(DEFAULT, "g_tolerance"), Some(6.5));
        assert_eq!(value(DEFAULT, "negative_g_tolerance"), Some(-3.0));
    }

    #[test]
    fn reads_values_and_ignores_junk() {
        let text = "# a comment\ng_tolerance = 4.0\nnot a setting\n\n";
        assert_eq!(value(text, "g_tolerance"), Some(4.0));
        assert_eq!(value(text, "missing"), None);
    }
}
