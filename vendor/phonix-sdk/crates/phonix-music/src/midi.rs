//! Notes and ticks: the units every pattern, clip and export agree on.

use serde::{Deserialize, Serialize};

pub const PPQN: u32 = 480;  // pulses per quarter note

/// Ticks in one bar of the given metre.
///
/// `PPQN` counts ticks per QUARTER note, the standard MIDI convention, so a bar
/// is `numerator` notes of `1/denominator` each: 4/4 and 12/8 are 1920 and 2880
/// ticks, 3/4 is 1440, 9/8 is 2160. Everything that used to hardcode
/// `4 * PPQN` for a bar goes through this, so a session in a compound or odd
/// metre has clips, the arrangement grid and the export all agreeing.
pub fn bar_ticks_of(time_sig: (u8, u8)) -> u64 {
    let num = time_sig.0.max(1) as u64;
    let den = time_sig.1.max(1) as u64;
    ((num * 4 * PPQN as u64) / den).max(1)
}

/// Ticks in one bar of 4/4 — the length of a whole note. Only for the places
/// that genuinely mean "a whole note" rather than "a bar".
pub const WHOLE_NOTE_TICKS: u64 = 4 * PPQN as u64;

// ── MIDI note ─────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, Copy, PartialEq)]
pub struct MidiNote {
    pub tick:     u64,   // start tick (absolute within pattern)
    pub duration: u32,   // ticks (>0)
    pub pitch:    u8,    // 0..127
    pub velocity: u8,    // 1..127
    /// Per-note probability (1–100). Default 100 = always fire.
    #[serde(default = "default_probability")]
    pub probability: u8,
}

fn default_probability() -> u8 { 100 }

pub const NOTE_NAMES: [&str; 12] =
    ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
