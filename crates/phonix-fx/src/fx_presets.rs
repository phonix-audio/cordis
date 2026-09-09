//! FX chain presets organized by category
//!
//! Each preset defines a complete effect chain configuration:
//! which effects in which slots, with all parameter values.

use crate::fx_params::pid;

/// A single slot in an FX preset
pub struct FxSlotPreset {
    pub effect_type: usize, // EffectType index (1=Delay, 2=Reverb, etc.)
    pub mix: f32,
    pub params: &'static [(u8, f32)], // (pid constant, value)
}

/// A complete FX chain preset
pub struct FxPreset {
    pub name: &'static str,
    pub slots: &'static [FxSlotPreset],
}

/// A category of FX presets
pub struct FxPresetCategory {
    pub name: &'static str,
    pub presets: &'static [&'static str],
}

pub fn get_fx_preset_categories() -> &'static [FxPresetCategory] {
    &[
        FxPresetCategory { name: "Mastering", presets: &[
            "Loud Master", "Clean Master", "Warm Master", "Punchy Master",
            "Pro Master Smart", "Pro Master Club", "Pro Master Streaming", "Pro Master Reference",
        ]},
        FxPresetCategory { name: "Mixing", presets: &[
            "Vocal Chain", "Drum Bus", "Guitar Crunch", "Bass Tightener",
        ]},
        FxPresetCategory { name: "Creative", presets: &[
            "Ambient Wash", "Lo-Fi Tape", "Dub Echo", "Shimmer Verb",
        ]},
        FxPresetCategory { name: "Modulation", presets: &[
            "Lush Chorus", "Jet Flanger", "Phaser Sweep", "Detune Widener",
        ]},
        FxPresetCategory { name: "Space", presets: &[
            "Big Hall", "Small Room", "Plate Verb", "Cathedral",
        ]},
        FxPresetCategory { name: "Delay", presets: &[
            "Ping Pong", "Slapback", "Tape Echo", "Dotted Eighth",
        ]},
        FxPresetCategory { name: "Guitar", presets: &[
            "Classic Rock", "Metal Rhythm", "Clean Shimmer", "Blues Lead", "Shoegaze Wall",
        ]},
        FxPresetCategory { name: "Bass", presets: &[
            "Sub Enhancer", "Acid FX", "Dub Bass", "Bass Crunch",
        ]},
        FxPresetCategory { name: "JMJ / Ambient", presets: &[
            "Oxygene Space", "Equinoxe Wash", "Laser FX", "Analog Warmth",
        ]},
        FxPresetCategory { name: "Vintage FX", presets: &[
            "String Ensemble", "Synthi Spring", "Cinema Space", "Dimension Wide", "Full Solina",
        ]},
    ]
}

pub fn find_fx_preset(name: &str) -> Option<&'static FxPreset> {
    get_all_fx_presets().iter().find(|p| p.name == name)
}

/// Describe one of the curated rack presets as an `FxChainSpec`, so an
/// instrument preset can ask the HOST for that chain by name instead of
/// carrying hand-written parameter values (or embedding effects of its own).
///
/// Returns `None` for an unknown name, which a caller should treat as "no
/// chain" rather than a panic: names are data, and a typo must not take the
/// audio thread down.
pub fn chain_spec(name: &str) -> Option<crate::fx_chain::FxChainSpec> {
    let preset = find_fx_preset(name)?;
    Some(crate::fx_chain::FxChainSpec::new(
        preset.slots.iter().map(|s| crate::fx_chain::FxSlotSpec {
            effect_type: s.effect_type as u8,
            enabled: true,
            mix: s.mix,
            params: s.params.to_vec(),
            sidechain_src: None,
            nested: Vec::new(),
            rack: Default::default(),
        }).collect(),
    ))
}

pub fn get_all_fx_presets() -> &'static [FxPreset] {
    &ALL_FX_PRESETS
}

// ═══════════════════════════════════════════════════════════════════════
// Per-effect presets (individual effect type presets)
// ═══════════════════════════════════════════════════════════════════════

/// A single preset for one effect type
pub struct EffectPreset {
    pub name: &'static str,
    pub params: &'static [(u8, f32)],
}

/// Get presets for a specific effect type index
pub fn get_effect_presets(effect_type_index: usize) -> &'static [EffectPreset] {
    match effect_type_index {
        1 => &DELAY_PRESETS,
        2 => &REVERB_PRESETS,
        3 => &CHORUS_PRESETS,
        4 => &PHASER_PRESETS,
        5 => &FLANGER_PRESETS,
        6 => &DISTORTION_PRESETS,
        7 => &VOCODER_PRESETS,
        8 => &COMPRESSOR_PRESETS,
        9 => &LIMITER_PRESETS,
        10 => &EXPANDER_PRESETS,
        11 => &AUTOPANNER_PRESETS,
        12 => &STUTTER_PRESETS,
        13 => &FILTER_PRESETS,
        14 => &RINGMOD_PRESETS,
        15 => &LOFI_PRESETS,
        16 => &STEREO_IMAGER_PRESETS,
        17 => &CHOIR_PRESETS,
        18 => &MAXIMIZER_PRESETS,
        19 => &SPATIALIZER_PRESETS,
        20 => &PARAM_EQ_PRESETS,
        21 => &DOPPLER_PRESETS,
        22 => &TRANSIENT_SHAPER_PRESETS,
        23 => &GRANULAR_FREEZER_PRESETS,
        24 => &SPECTRAL_FREEZE_PRESETS,
        25 => &RESONATOR_BANK_PRESETS,
        26 => &AUTO_WAH_PRESETS,
        27 => &TAPE_STOP_PRESETS,
        // Mastering tier (Phases A-E). Meter-only effects (LufsMeter
        // idx 29, SpectrumAnalyzer idx 33) intentionally return an
        // empty list — they have no audible params worth presetting.
        28 => &BRICKWALL_LIMITER_PRESETS,
        30 => &DYNAMIC_EQ_PRESETS,
        31 => &MID_SIDE_EQ_PRESETS,
        32 => &MULTIBAND_COMP_PRESETS,
        34 => &MASTER_ASSISTANT_PRESETS,
        36 => &MULTIBAND_DUCKER_PRESETS,
        37 => &VELVET_DECORRELATOR_PRESETS,
        38 => &ANALOG_DRIFT_PRESETS,
        42 => &RETUNE_PRESETS,
        43 => &HARMONY_PRESETS,
        _ => &[],
    }
}

static RETUNE_PRESETS: [EffectPreset; 5] = [
    EffectPreset { name: "Natural Touch-Up", params: &[
        (pid::RETUNE_KEY, 0.0), (pid::RETUNE_SCALE, 0.0), (pid::RETUNE_SPEED, 0.25),
        (pid::RETUNE_AMOUNT, 0.8), (pid::RETUNE_MIX, 1.0)]},
    EffectPreset { name: "Pop Tight", params: &[
        (pid::RETUNE_KEY, 0.0), (pid::RETUNE_SCALE, 0.0), (pid::RETUNE_SPEED, 0.6),
        (pid::RETUNE_AMOUNT, 1.0), (pid::RETUNE_MIX, 1.0)]},
    EffectPreset { name: "Hard Tune", params: &[
        (pid::RETUNE_KEY, 0.0), (pid::RETUNE_SCALE, 0.0), (pid::RETUNE_SPEED, 1.0),
        (pid::RETUNE_AMOUNT, 1.0), (pid::RETUNE_MIX, 1.0)]},
    EffectPreset { name: "Minor Hard Tune", params: &[
        (pid::RETUNE_KEY, 9.0), (pid::RETUNE_SCALE, 1.0), (pid::RETUNE_SPEED, 1.0),
        (pid::RETUNE_AMOUNT, 1.0), (pid::RETUNE_MIX, 1.0)]},
    EffectPreset { name: "Subtle Blend", params: &[
        (pid::RETUNE_KEY, 0.0), (pid::RETUNE_SCALE, 0.0), (pid::RETUNE_SPEED, 0.3),
        (pid::RETUNE_AMOUNT, 0.6), (pid::RETUNE_MIX, 0.7)]},
];

static HARMONY_PRESETS: [EffectPreset; 5] = [
    EffectPreset { name: "Third Above", params: &[
        (pid::HARM_KEY, 0.0), (pid::HARM_SCALE, 0.0), (pid::HARM_V1, 2.0),
        (pid::HARM_V2, 0.0), (pid::HARM_V3, 0.0), (pid::HARM_SPREAD, 0.4),
        (pid::HARM_MIX, 0.5)]},
    EffectPreset { name: "Third + Fifth", params: &[
        (pid::HARM_KEY, 0.0), (pid::HARM_SCALE, 0.0), (pid::HARM_V1, 2.0),
        (pid::HARM_V2, 4.0), (pid::HARM_V3, 0.0), (pid::HARM_SPREAD, 0.6),
        (pid::HARM_MIX, 0.45)]},
    EffectPreset { name: "Low Fourth Doubler", params: &[
        (pid::HARM_KEY, 0.0), (pid::HARM_SCALE, 0.0), (pid::HARM_V1, -3.0),
        (pid::HARM_V2, 0.0), (pid::HARM_V3, 0.0), (pid::HARM_SPREAD, 0.3),
        (pid::HARM_MIX, 0.55)]},
    EffectPreset { name: "Choir Stack", params: &[
        (pid::HARM_KEY, 0.0), (pid::HARM_SCALE, 0.0), (pid::HARM_V1, 2.0),
        (pid::HARM_V2, -3.0), (pid::HARM_V3, 4.0), (pid::HARM_SPREAD, 0.8),
        (pid::HARM_MIX, 0.5)]},
    EffectPreset { name: "Octave Up Shine", params: &[
        (pid::HARM_KEY, 0.0), (pid::HARM_SCALE, 0.0), (pid::HARM_V1, 7.0),
        (pid::HARM_V2, 0.0), (pid::HARM_V3, 0.0), (pid::HARM_SPREAD, 0.2),
        (pid::HARM_MIX, 0.35)]},
];

static MULTIBAND_DUCKER_PRESETS: [EffectPreset; 4] = [
    // The canon "the bass owns the lows" master move (crossover 230, depth 0.85,
    // sensitivity 1.7, 5 ms / 130 ms) -- sidechain the bass track into this.
    EffectPreset { name: "Bass Owns Lows", params: &[
        (pid::MBDUCK_CROSSOVER, 230.0), (pid::MBDUCK_DEPTH, 0.85), (pid::MBDUCK_SENS, 1.7),
        (pid::MBDUCK_ATTACK, 0.005), (pid::MBDUCK_RELEASE, 0.13), (pid::MBDUCK_MIX, 1.0),
    ]},
    EffectPreset { name: "Tight Kick Slot", params: &[
        (pid::MBDUCK_CROSSOVER, 140.0), (pid::MBDUCK_DEPTH, 0.7), (pid::MBDUCK_SENS, 2.2),
        (pid::MBDUCK_ATTACK, 0.002), (pid::MBDUCK_RELEASE, 0.08), (pid::MBDUCK_MIX, 1.0),
    ]},
    EffectPreset { name: "Gentle Sub Carve", params: &[
        (pid::MBDUCK_CROSSOVER, 90.0), (pid::MBDUCK_DEPTH, 0.5), (pid::MBDUCK_SENS, 1.3),
        (pid::MBDUCK_ATTACK, 0.008), (pid::MBDUCK_RELEASE, 0.2), (pid::MBDUCK_MIX, 1.0),
    ]},
    EffectPreset { name: "Heavy Low Duck", params: &[
        (pid::MBDUCK_CROSSOVER, 320.0), (pid::MBDUCK_DEPTH, 1.0), (pid::MBDUCK_SENS, 2.5),
        (pid::MBDUCK_ATTACK, 0.003), (pid::MBDUCK_RELEASE, 0.12), (pid::MBDUCK_MIX, 1.0),
    ]},
];

static VELVET_DECORRELATOR_PRESETS: [EffectPreset; 4] = [
    EffectPreset { name: "String Section", params: &[
        (pid::VELVET_WIDTH, 0.6), (pid::VELVET_SIZE, 0.5), (pid::VELVET_MIX, 1.0)]},
    EffectPreset { name: "Huge Ensemble", params: &[
        (pid::VELVET_WIDTH, 0.85), (pid::VELVET_SIZE, 0.9), (pid::VELVET_MIX, 1.0)]},
    EffectPreset { name: "Subtle Widen", params: &[
        (pid::VELVET_WIDTH, 0.35), (pid::VELVET_SIZE, 0.3), (pid::VELVET_MIX, 1.0)]},
    EffectPreset { name: "Mono-Safe Spread", params: &[
        (pid::VELVET_WIDTH, 0.7), (pid::VELVET_SIZE, 0.65), (pid::VELVET_MIX, 1.0)]},
];

static ANALOG_DRIFT_PRESETS: [EffectPreset; 4] = [
    EffectPreset { name: "Tape Wow", params: &[
        (pid::DRIFT_DEPTH, 5.0), (pid::DRIFT_RATE, 0.3), (pid::DRIFT_MIX, 1.0)]},
    EffectPreset { name: "Double Track", params: &[
        (pid::DRIFT_DEPTH, 8.0), (pid::DRIFT_RATE, 0.5), (pid::DRIFT_MIX, 1.0)]},
    EffectPreset { name: "Gentle Detune", params: &[
        (pid::DRIFT_DEPTH, 3.0), (pid::DRIFT_RATE, 0.2), (pid::DRIFT_MIX, 1.0)]},
    EffectPreset { name: "Warped Tape", params: &[
        (pid::DRIFT_DEPTH, 14.0), (pid::DRIFT_RATE, 0.8), (pid::DRIFT_MIX, 1.0)]},
];

static DELAY_PRESETS: [EffectPreset; 8] = [
    EffectPreset { name: "Slapback", params: &[
        (pid::DELAY_TYPE, 0.0), (pid::DELAY_TIME, 0.08), (pid::DELAY_FEEDBACK, 0.1),
        (pid::DELAY_STEREO, 0.1), (pid::DELAY_HICUT, 10000.0), (pid::DELAY_LOCUT, 80.0),
    ]},
    EffectPreset { name: "Ping Pong", params: &[
        (pid::DELAY_TYPE, 1.0), (pid::DELAY_TIME, 0.375), (pid::DELAY_FEEDBACK, 0.5),
        (pid::DELAY_STEREO, 1.0), (pid::DELAY_HICUT, 8000.0), (pid::DELAY_LOCUT, 100.0),
    ]},
    EffectPreset { name: "Tape Echo", params: &[
        (pid::DELAY_TYPE, 2.0), (pid::DELAY_TIME, 0.3), (pid::DELAY_FEEDBACK, 0.55),
        (pid::DELAY_STEREO, 0.3), (pid::DELAY_HICUT, 4000.0), (pid::DELAY_LOCUT, 200.0),
    ]},
    EffectPreset { name: "Dub Echo", params: &[
        (pid::DELAY_TYPE, 1.0), (pid::DELAY_TIME, 0.375), (pid::DELAY_FEEDBACK, 0.7),
        (pid::DELAY_STEREO, 0.8), (pid::DELAY_HICUT, 3000.0), (pid::DELAY_LOCUT, 400.0),
    ]},
    EffectPreset { name: "Dotted Eighth", params: &[
        (pid::DELAY_TYPE, 0.0), (pid::DELAY_TIME, 0.281), (pid::DELAY_FEEDBACK, 0.4),
        (pid::DELAY_STEREO, 0.4), (pid::DELAY_HICUT, 6000.0), (pid::DELAY_LOCUT, 200.0),
    ]},
    EffectPreset { name: "Dark Delay", params: &[
        (pid::DELAY_TYPE, 2.0), (pid::DELAY_TIME, 0.4), (pid::DELAY_FEEDBACK, 0.6),
        (pid::DELAY_STEREO, 0.5), (pid::DELAY_HICUT, 2000.0), (pid::DELAY_LOCUT, 300.0),
    ]},
    EffectPreset { name: "Short Ambience", params: &[
        (pid::DELAY_TYPE, 3.0), (pid::DELAY_TIME, 0.04), (pid::DELAY_FEEDBACK, 0.3),
        (pid::DELAY_STEREO, 0.6), (pid::DELAY_HICUT, 8000.0), (pid::DELAY_LOCUT, 100.0),
    ]},
    EffectPreset { name: "Infinite Hold", params: &[
        (pid::DELAY_TYPE, 0.0), (pid::DELAY_TIME, 0.5), (pid::DELAY_FEEDBACK, 0.95),
        (pid::DELAY_STEREO, 0.3), (pid::DELAY_HICUT, 6000.0), (pid::DELAY_LOCUT, 100.0),
    ]},
];

static REVERB_PRESETS: [EffectPreset; 8] = [
    EffectPreset { name: "Small Room", params: &[
        (pid::REVERB_TYPE, 1.0), (pid::REVERB_SIZE, 0.2), (pid::REVERB_DECAY, 0.2),
        (pid::REVERB_DAMP, 0.6), (pid::REVERB_WIDTH, 0.8), (pid::REVERB_PREDELAY, 0.005),
    ]},
    EffectPreset { name: "Big Hall", params: &[
        (pid::REVERB_TYPE, 0.0), (pid::REVERB_SIZE, 0.85), (pid::REVERB_DECAY, 0.75),
        (pid::REVERB_DAMP, 0.4), (pid::REVERB_WIDTH, 1.5), (pid::REVERB_PREDELAY, 0.035),
    ]},
    EffectPreset { name: "Plate", params: &[
        (pid::REVERB_TYPE, 2.0), (pid::REVERB_SIZE, 0.6), (pid::REVERB_DECAY, 0.5),
        (pid::REVERB_DAMP, 0.45), (pid::REVERB_WIDTH, 1.2), (pid::REVERB_PREDELAY, 0.01),
    ]},
    EffectPreset { name: "Cathedral", params: &[
        (pid::REVERB_TYPE, 0.0), (pid::REVERB_SIZE, 0.98), (pid::REVERB_DECAY, 0.92),
        (pid::REVERB_DAMP, 0.25), (pid::REVERB_WIDTH, 2.0), (pid::REVERB_PREDELAY, 0.06),
    ]},
    EffectPreset { name: "Spring", params: &[
        (pid::REVERB_TYPE, 3.0), (pid::REVERB_SIZE, 0.4), (pid::REVERB_DECAY, 0.4),
        (pid::REVERB_DAMP, 0.5), (pid::REVERB_WIDTH, 1.0), (pid::REVERB_PREDELAY, 0.005),
    ]},
    EffectPreset { name: "Ambient Pad", params: &[
        (pid::REVERB_TYPE, 0.0), (pid::REVERB_SIZE, 0.9), (pid::REVERB_DECAY, 0.85),
        (pid::REVERB_DAMP, 0.3), (pid::REVERB_WIDTH, 1.8), (pid::REVERB_PREDELAY, 0.04),
    ]},
    EffectPreset { name: "Tight Drum", params: &[
        (pid::REVERB_TYPE, 1.0), (pid::REVERB_SIZE, 0.15), (pid::REVERB_DECAY, 0.12),
        (pid::REVERB_DAMP, 0.7), (pid::REVERB_WIDTH, 0.6), (pid::REVERB_PREDELAY, 0.002),
    ]},
    EffectPreset { name: "Shimmer", params: &[
        (pid::REVERB_TYPE, 0.0), (pid::REVERB_SIZE, 0.95), (pid::REVERB_DECAY, 0.9),
        (pid::REVERB_DAMP, 0.2), (pid::REVERB_WIDTH, 2.0), (pid::REVERB_PREDELAY, 0.05),
    ]},
];

static CHORUS_PRESETS: [EffectPreset; 6] = [
    EffectPreset { name: "Subtle Width", params: &[
        (pid::CHORUS_RATE, 0.5), (pid::CHORUS_DEPTH, 0.3), (pid::CHORUS_WIDTH, 1.0),
        (pid::CHORUS_DELAY, 0.008), (pid::CHORUS_VOICES, 2.0), (pid::CHORUS_MIX, 0.3),
    ]},
    EffectPreset { name: "Lush", params: &[
        (pid::CHORUS_RATE, 0.6), (pid::CHORUS_DEPTH, 0.7), (pid::CHORUS_WIDTH, 1.0),
        (pid::CHORUS_DELAY, 0.012), (pid::CHORUS_VOICES, 4.0), (pid::CHORUS_MIX, 0.5),
    ]},
    EffectPreset { name: "Detune", params: &[
        (pid::CHORUS_RATE, 0.1), (pid::CHORUS_DEPTH, 0.3), (pid::CHORUS_WIDTH, 1.0),
        (pid::CHORUS_DELAY, 0.025), (pid::CHORUS_VOICES, 6.0), (pid::CHORUS_MIX, 0.35),
    ]},
    EffectPreset { name: "Vibrato", params: &[
        (pid::CHORUS_RATE, 5.0), (pid::CHORUS_DEPTH, 0.8), (pid::CHORUS_WIDTH, 0.0),
        (pid::CHORUS_DELAY, 0.005), (pid::CHORUS_VOICES, 1.0), (pid::CHORUS_MIX, 1.0),
    ]},
    EffectPreset { name: "80s Ensemble", params: &[
        (pid::CHORUS_RATE, 0.8), (pid::CHORUS_DEPTH, 0.5), (pid::CHORUS_WIDTH, 1.0),
        (pid::CHORUS_DELAY, 0.015), (pid::CHORUS_VOICES, 3.0), (pid::CHORUS_MIX, 0.5),
    ]},
    EffectPreset { name: "Lo-Fi Warble", params: &[
        (pid::CHORUS_RATE, 0.4), (pid::CHORUS_DEPTH, 0.9), (pid::CHORUS_WIDTH, 0.6),
        (pid::CHORUS_DELAY, 0.02), (pid::CHORUS_VOICES, 2.0), (pid::CHORUS_MIX, 0.6),
    ]},
];

static PHASER_PRESETS: [EffectPreset; 5] = [
    EffectPreset { name: "Slow Sweep", params: &[
        (pid::PHASER_RATE, 0.15), (pid::PHASER_DEPTH, 0.6), (pid::PHASER_FEEDBACK, 0.5),
        (pid::PHASER_STEREO, 0.5), (pid::PHASER_STAGES, 6.0), (pid::PHASER_MIX, 0.5),
    ]},
    EffectPreset { name: "Fast Swirl", params: &[
        (pid::PHASER_RATE, 2.0), (pid::PHASER_DEPTH, 0.7), (pid::PHASER_FEEDBACK, 0.6),
        (pid::PHASER_STEREO, 0.7), (pid::PHASER_STAGES, 4.0), (pid::PHASER_MIX, 0.5),
    ]},
    EffectPreset { name: "Deep Phase", params: &[
        (pid::PHASER_RATE, 0.3), (pid::PHASER_DEPTH, 0.9), (pid::PHASER_FEEDBACK, 0.8),
        (pid::PHASER_STEREO, 0.6), (pid::PHASER_STAGES, 8.0), (pid::PHASER_MIX, 0.5),
    ]},
    EffectPreset { name: "Gentle", params: &[
        (pid::PHASER_RATE, 0.4), (pid::PHASER_DEPTH, 0.3), (pid::PHASER_FEEDBACK, 0.3),
        (pid::PHASER_STEREO, 0.4), (pid::PHASER_STAGES, 4.0), (pid::PHASER_MIX, 0.35),
    ]},
    EffectPreset { name: "Resonant", params: &[
        (pid::PHASER_RATE, 0.5), (pid::PHASER_DEPTH, 0.5), (pid::PHASER_FEEDBACK, 0.9),
        (pid::PHASER_STEREO, 0.5), (pid::PHASER_STAGES, 12.0), (pid::PHASER_MIX, 0.5),
    ]},
];

static FLANGER_PRESETS: [EffectPreset; 5] = [
    EffectPreset { name: "Jet", params: &[
        (pid::FLANGER_RATE, 0.15), (pid::FLANGER_DEPTH, 0.8), (pid::FLANGER_DELAY, 0.003),
        (pid::FLANGER_FEEDBACK, 0.85), (pid::FLANGER_STEREO, 0.5), (pid::FLANGER_MIX, 0.5),
    ]},
    EffectPreset { name: "Metallic", params: &[
        (pid::FLANGER_RATE, 0.3), (pid::FLANGER_DEPTH, 0.6), (pid::FLANGER_DELAY, 0.001),
        (pid::FLANGER_FEEDBACK, 0.9), (pid::FLANGER_STEREO, 0.3), (pid::FLANGER_MIX, 0.5),
    ]},
    EffectPreset { name: "Gentle Sweep", params: &[
        (pid::FLANGER_RATE, 0.2), (pid::FLANGER_DEPTH, 0.4), (pid::FLANGER_DELAY, 0.005),
        (pid::FLANGER_FEEDBACK, 0.4), (pid::FLANGER_STEREO, 0.5), (pid::FLANGER_MIX, 0.4),
    ]},
    EffectPreset { name: "Through Zero", params: &[
        (pid::FLANGER_RATE, 0.1), (pid::FLANGER_DEPTH, 1.0), (pid::FLANGER_DELAY, 0.002),
        (pid::FLANGER_FEEDBACK, 0.7), (pid::FLANGER_STEREO, 0.8), (pid::FLANGER_MIX, 0.5),
    ]},
    EffectPreset { name: "Barber Pole", params: &[
        (pid::FLANGER_RATE, 0.5), (pid::FLANGER_DEPTH, 0.7), (pid::FLANGER_DELAY, 0.004),
        (pid::FLANGER_FEEDBACK, 0.6), (pid::FLANGER_STEREO, 1.0), (pid::FLANGER_MIX, 0.5),
    ]},
];

static DISTORTION_PRESETS: [EffectPreset; 6] = [
    EffectPreset { name: "Warm Saturation", params: &[
        (pid::DIST_TYPE, 2.0), (pid::DIST_DRIVE, 2.0), (pid::DIST_TONE, 4000.0),
        (pid::DIST_OUTPUT, 0.8), (pid::DIST_MIX, 0.5),
    ]},
    EffectPreset { name: "Tube Drive", params: &[
        (pid::DIST_TYPE, 1.0), (pid::DIST_DRIVE, 6.0), (pid::DIST_TONE, 3500.0),
        (pid::DIST_OUTPUT, 0.6), (pid::DIST_MIX, 0.7),
    ]},
    EffectPreset { name: "Hard Clip", params: &[
        (pid::DIST_TYPE, 0.0), (pid::DIST_DRIVE, 10.0), (pid::DIST_TONE, 5000.0),
        (pid::DIST_OUTPUT, 0.5), (pid::DIST_MIX, 0.8),
    ]},
    EffectPreset { name: "Bitcrush", params: &[
        (pid::DIST_TYPE, 4.0), (pid::DIST_DRIVE, 1.0), (pid::DIST_TONE, 3000.0),
        (pid::DIST_OUTPUT, 0.7), (pid::DIST_BITS, 8.0), (pid::DIST_SR_DIV, 4.0), (pid::DIST_MIX, 0.8),
    ]},
    EffectPreset { name: "Lo-Fi", params: &[
        (pid::DIST_TYPE, 4.0), (pid::DIST_DRIVE, 1.5), (pid::DIST_TONE, 2500.0),
        (pid::DIST_OUTPUT, 0.7), (pid::DIST_BITS, 12.0), (pid::DIST_SR_DIV, 3.0), (pid::DIST_MIX, 0.5),
    ]},
    EffectPreset { name: "Fuzz", params: &[
        (pid::DIST_TYPE, 3.0), (pid::DIST_DRIVE, 15.0), (pid::DIST_TONE, 2000.0),
        (pid::DIST_OUTPUT, 0.4), (pid::DIST_MIX, 1.0),
    ]},
];

static VOCODER_PRESETS: [EffectPreset; 8] = [
    EffectPreset { name: "Classic Robot", params: &[
        (pid::VOC_BANDS, 16.0), (pid::VOC_ATTACK, 0.005), (pid::VOC_RELEASE, 0.04),
        (pid::VOC_MIX, 1.0), (pid::VOC_CARRIER, 1.0), (pid::VOC_MOD, 1.0), (pid::VOC_FORMANT, 1.0),
    ]},
    EffectPreset { name: "Soft Voice", params: &[
        (pid::VOC_BANDS, 24.0), (pid::VOC_ATTACK, 0.02), (pid::VOC_RELEASE, 0.12),
        (pid::VOC_MIX, 0.8), (pid::VOC_CARRIER, 1.0), (pid::VOC_MOD, 1.0), (pid::VOC_FORMANT, 1.1),
    ]},
    EffectPreset { name: "Whisper", params: &[
        (pid::VOC_BANDS, 6.0), (pid::VOC_ATTACK, 0.003), (pid::VOC_RELEASE, 0.02),
        (pid::VOC_MIX, 0.7), (pid::VOC_CARRIER, 0.8), (pid::VOC_MOD, 1.5), (pid::VOC_FORMANT, 0.8),
    ]},
    EffectPreset { name: "Wide Bands", params: &[
        (pid::VOC_BANDS, 8.0), (pid::VOC_ATTACK, 0.01), (pid::VOC_RELEASE, 0.08),
        (pid::VOC_MIX, 1.0), (pid::VOC_CARRIER, 1.0), (pid::VOC_MOD, 1.0), (pid::VOC_FORMANT, 1.5),
    ]},
    EffectPreset { name: "Daft Punk", params: &[
        (pid::VOC_BANDS, 20.0), (pid::VOC_ATTACK, 0.008), (pid::VOC_RELEASE, 0.05),
        (pid::VOC_MIX, 1.0), (pid::VOC_CARRIER, 1.2), (pid::VOC_MOD, 1.0), (pid::VOC_FORMANT, 1.0),
    ]},
    EffectPreset { name: "Frozen Vowel", params: &[
        (pid::VOC_BANDS, 16.0), (pid::VOC_ATTACK, 0.01), (pid::VOC_RELEASE, 0.4),
        (pid::VOC_MIX, 0.9), (pid::VOC_CARRIER, 1.0), (pid::VOC_MOD, 1.0), (pid::VOC_FORMANT, 1.0),
    ]},
    EffectPreset { name: "Hi-Res", params: &[
        (pid::VOC_BANDS, 32.0), (pid::VOC_ATTACK, 0.003), (pid::VOC_RELEASE, 0.03),
        (pid::VOC_MIX, 1.0), (pid::VOC_CARRIER, 1.0), (pid::VOC_MOD, 1.0), (pid::VOC_FORMANT, 1.0),
    ]},
    EffectPreset { name: "Dark Machine", params: &[
        (pid::VOC_BANDS, 10.0), (pid::VOC_ATTACK, 0.008), (pid::VOC_RELEASE, 0.08),
        (pid::VOC_MIX, 1.0), (pid::VOC_CARRIER, 1.3), (pid::VOC_MOD, 1.0), (pid::VOC_FORMANT, 0.65),
    ]},
];

static COMPRESSOR_PRESETS: [EffectPreset; 6] = [
    EffectPreset { name: "Gentle Glue", params: &[
        (pid::COMP_MODE, 0.0), (pid::COMP_THRESH, -20.0), (pid::COMP_RATIO, 2.0),
        (pid::COMP_ATTACK, 0.01), (pid::COMP_RELEASE, 0.2), (pid::COMP_MAKEUP, 2.0), (pid::COMP_KNEE, 12.0),
    ]},
    EffectPreset { name: "Punchy", params: &[
        (pid::COMP_MODE, 2.0), (pid::COMP_THRESH, -15.0), (pid::COMP_RATIO, 6.0),
        (pid::COMP_ATTACK, 0.003), (pid::COMP_RELEASE, 0.1), (pid::COMP_MAKEUP, 5.0), (pid::COMP_KNEE, 3.0),
    ]},
    EffectPreset { name: "Vocal Smooth", params: &[
        (pid::COMP_MODE, 1.0), (pid::COMP_THRESH, -20.0), (pid::COMP_RATIO, 3.0),
        (pid::COMP_ATTACK, 0.005), (pid::COMP_RELEASE, 0.2), (pid::COMP_MAKEUP, 4.0), (pid::COMP_KNEE, 6.0),
    ]},
    EffectPreset { name: "Drum Smash", params: &[
        (pid::COMP_MODE, 2.0), (pid::COMP_THRESH, -12.0), (pid::COMP_RATIO, 8.0),
        (pid::COMP_ATTACK, 0.001), (pid::COMP_RELEASE, 0.06), (pid::COMP_MAKEUP, 8.0), (pid::COMP_KNEE, 2.0),
    ]},
    EffectPreset { name: "Bass Control", params: &[
        (pid::COMP_MODE, 2.0), (pid::COMP_THRESH, -15.0), (pid::COMP_RATIO, 5.0),
        (pid::COMP_ATTACK, 0.002), (pid::COMP_RELEASE, 0.1), (pid::COMP_MAKEUP, 3.0), (pid::COMP_KNEE, 3.0),
    ]},
    EffectPreset { name: "Transparent", params: &[
        (pid::COMP_MODE, 0.0), (pid::COMP_THRESH, -24.0), (pid::COMP_RATIO, 1.5),
        (pid::COMP_ATTACK, 0.015), (pid::COMP_RELEASE, 0.3), (pid::COMP_MAKEUP, 1.5), (pid::COMP_KNEE, 15.0),
    ]},
];

static LIMITER_PRESETS: [EffectPreset; 4] = [
    EffectPreset { name: "Brickwall", params: &[
        (pid::LIMITER_MODE, 0.0), (pid::LIMITER_CEIL, -0.3), (pid::LIMITER_RELEASE, 0.05),
        (pid::LIMITER_GAIN, 0.0),
    ]},
    EffectPreset { name: "Loud Master", params: &[
        (pid::LIMITER_MODE, 2.0), (pid::LIMITER_CEIL, -0.3), (pid::LIMITER_RELEASE, 0.05),
        (pid::LIMITER_GAIN, 12.0),
    ]},
    EffectPreset { name: "Soft Clip", params: &[
        (pid::LIMITER_MODE, 1.0), (pid::LIMITER_CEIL, -0.5), (pid::LIMITER_RELEASE, 0.08),
        (pid::LIMITER_GAIN, 6.0),
    ]},
    EffectPreset { name: "Safety Net", params: &[
        (pid::LIMITER_MODE, 0.0), (pid::LIMITER_CEIL, -0.1), (pid::LIMITER_RELEASE, 0.1),
        (pid::LIMITER_GAIN, 0.0),
    ]},
];

static EXPANDER_PRESETS: [EffectPreset; 4] = [
    EffectPreset { name: "Noise Gate", params: &[
        (pid::EXP_MODE, 0.0), (pid::EXP_THRESH, -40.0), (pid::EXP_RATIO, 10.0),
        (pid::EXP_ATTACK, 0.001), (pid::EXP_RELEASE, 0.05), (pid::EXP_RANGE, -60.0), (pid::EXP_KNEE, 1.0),
    ]},
    EffectPreset { name: "Gentle Gate", params: &[
        (pid::EXP_MODE, 0.0), (pid::EXP_THRESH, -35.0), (pid::EXP_RATIO, 4.0),
        (pid::EXP_ATTACK, 0.002), (pid::EXP_RELEASE, 0.1), (pid::EXP_RANGE, -30.0), (pid::EXP_KNEE, 6.0),
    ]},
    EffectPreset { name: "Transient Shaper", params: &[
        (pid::EXP_MODE, 1.0), (pid::EXP_THRESH, -25.0), (pid::EXP_RATIO, 3.0),
        (pid::EXP_ATTACK, 0.0005), (pid::EXP_RELEASE, 0.03), (pid::EXP_RANGE, -20.0), (pid::EXP_KNEE, 3.0),
    ]},
    EffectPreset { name: "Drum Tighten", params: &[
        (pid::EXP_MODE, 0.0), (pid::EXP_THRESH, -30.0), (pid::EXP_RATIO, 6.0),
        (pid::EXP_ATTACK, 0.001), (pid::EXP_RELEASE, 0.04), (pid::EXP_RANGE, -40.0), (pid::EXP_KNEE, 2.0),
    ]},
];

static AUTOPANNER_PRESETS: [EffectPreset; 10] = [
    EffectPreset { name: "Gentle Sine", params: &[
        (pid::APAN_MODE, 0.0), (pid::APAN_WAVE, 0.0), (pid::APAN_RATE, 0.5),
        (pid::APAN_DEPTH, 0.4), (pid::APAN_WIDTH, 0.7), (pid::APAN_MIX, 1.0),
    ]},
    EffectPreset { name: "Fast Triangle", params: &[
        (pid::APAN_MODE, 0.0), (pid::APAN_WAVE, 1.0), (pid::APAN_RATE, 4.0),
        (pid::APAN_DEPTH, 0.8), (pid::APAN_WIDTH, 1.0), (pid::APAN_MIX, 1.0),
    ]},
    EffectPreset { name: "Hard Chop", params: &[
        (pid::APAN_MODE, 0.0), (pid::APAN_WAVE, 2.0), (pid::APAN_RATE, 2.0),
        (pid::APAN_DEPTH, 1.0), (pid::APAN_WIDTH, 1.0), (pid::APAN_MIX, 1.0),
    ]},
    EffectPreset { name: "Leslie Slow", params: &[
        (pid::APAN_MODE, 1.0), (pid::APAN_RATE, 1.0), (pid::APAN_DEPTH, 0.7),
        (pid::APAN_WIDTH, 0.8), (pid::APAN_MIX, 1.0), (pid::APAN_SPIN_SPEED, 0.0),
    ]},
    EffectPreset { name: "Leslie Fast", params: &[
        (pid::APAN_MODE, 1.0), (pid::APAN_RATE, 1.0), (pid::APAN_DEPTH, 0.8),
        (pid::APAN_WIDTH, 1.0), (pid::APAN_MIX, 1.0), (pid::APAN_SPIN_SPEED, 1.0),
    ]},
    EffectPreset { name: "Ping Pong Ball", params: &[
        (pid::APAN_MODE, 2.0), (pid::APAN_RATE, 1.5), (pid::APAN_DEPTH, 1.0),
        (pid::APAN_WIDTH, 1.0), (pid::APAN_MIX, 1.0),
    ]},
    EffectPreset { name: "Random Drift", params: &[
        (pid::APAN_MODE, 3.0), (pid::APAN_RATE, 0.8), (pid::APAN_DEPTH, 0.6),
        (pid::APAN_WIDTH, 0.8), (pid::APAN_MIX, 1.0), (pid::APAN_GLIDE, 0.8),
    ]},
    EffectPreset { name: "Jumpy S&H", params: &[
        (pid::APAN_MODE, 3.0), (pid::APAN_RATE, 3.0), (pid::APAN_DEPTH, 1.0),
        (pid::APAN_WIDTH, 1.0), (pid::APAN_MIX, 1.0), (pid::APAN_GLIDE, 0.1),
    ]},
    EffectPreset { name: "Envelope Pan", params: &[
        (pid::APAN_MODE, 4.0), (pid::APAN_RATE, 2.0), (pid::APAN_DEPTH, 0.8),
        (pid::APAN_WIDTH, 1.0), (pid::APAN_MIX, 1.0), (pid::APAN_PHASE, 0.0),
    ]},
    EffectPreset { name: "Vintage Trem-Pan", params: &[
        (pid::APAN_MODE, 6.0), (pid::APAN_WAVE, 0.0), (pid::APAN_RATE, 3.0),
        (pid::APAN_DEPTH, 0.6), (pid::APAN_WIDTH, 0.5), (pid::APAN_MIX, 1.0),
        (pid::APAN_TREM_DEPTH, 0.7),
    ]},
];

// ═══════════════════════════════════════════════════════════════════════
// Drum-specific per-effect presets
// ═══════════════════════════════════════════════════════════════════════

/// Get drum-adapted presets for a specific effect type index
pub fn get_drum_effect_presets(effect_type_index: usize) -> &'static [EffectPreset] {
    match effect_type_index {
        1 => &DRUM_DELAY_PRESETS,
        2 => &DRUM_REVERB_PRESETS,
        3 => &DRUM_CHORUS_PRESETS,
        4 => &DRUM_PHASER_PRESETS,
        5 => &DRUM_FLANGER_PRESETS,
        6 => &DRUM_DISTORTION_PRESETS,
        8 => &DRUM_COMPRESSOR_PRESETS,
        9 => &DRUM_LIMITER_PRESETS,
        10 => &DRUM_EXPANDER_PRESETS,
        _ => get_effect_presets(effect_type_index),
    }
}

static DRUM_DELAY_PRESETS: [EffectPreset; 5] = [
    EffectPreset { name: "Ghost Slap", params: &[
        (pid::DELAY_TYPE, 0.0), (pid::DELAY_TIME, 0.06), (pid::DELAY_FEEDBACK, 0.05),
        (pid::DELAY_STEREO, 0.2), (pid::DELAY_HICUT, 6000.0), (pid::DELAY_LOCUT, 200.0),
    ]},
    EffectPreset { name: "Dub Snare", params: &[
        (pid::DELAY_TYPE, 1.0), (pid::DELAY_TIME, 0.375), (pid::DELAY_FEEDBACK, 0.5),
        (pid::DELAY_STEREO, 0.8), (pid::DELAY_HICUT, 3000.0), (pid::DELAY_LOCUT, 400.0),
    ]},
    EffectPreset { name: "HiHat Scatter", params: &[
        (pid::DELAY_TYPE, 1.0), (pid::DELAY_TIME, 0.125), (pid::DELAY_FEEDBACK, 0.4),
        (pid::DELAY_STEREO, 1.0), (pid::DELAY_HICUT, 10000.0), (pid::DELAY_LOCUT, 500.0),
    ]},
    EffectPreset { name: "Tape Bounce", params: &[
        (pid::DELAY_TYPE, 2.0), (pid::DELAY_TIME, 0.25), (pid::DELAY_FEEDBACK, 0.45),
        (pid::DELAY_STEREO, 0.5), (pid::DELAY_HICUT, 4000.0), (pid::DELAY_LOCUT, 150.0),
    ]},
    EffectPreset { name: "Kick Sub Echo", params: &[
        (pid::DELAY_TYPE, 0.0), (pid::DELAY_TIME, 0.15), (pid::DELAY_FEEDBACK, 0.3),
        (pid::DELAY_STEREO, 0.0), (pid::DELAY_HICUT, 200.0), (pid::DELAY_LOCUT, 20.0),
    ]},
];

static DRUM_REVERB_PRESETS: [EffectPreset; 6] = [
    EffectPreset { name: "Tight Snare", params: &[
        (pid::REVERB_TYPE, 1.0), (pid::REVERB_SIZE, 0.15), (pid::REVERB_DECAY, 0.1),
        (pid::REVERB_DAMP, 0.7), (pid::REVERB_WIDTH, 0.6), (pid::REVERB_PREDELAY, 0.002),
    ]},
    EffectPreset { name: "Gated Room", params: &[
        (pid::REVERB_TYPE, 1.0), (pid::REVERB_SIZE, 0.3), (pid::REVERB_DECAY, 0.15),
        (pid::REVERB_DAMP, 0.5), (pid::REVERB_WIDTH, 1.0), (pid::REVERB_PREDELAY, 0.005),
    ]},
    EffectPreset { name: "808 Room", params: &[
        (pid::REVERB_TYPE, 1.0), (pid::REVERB_SIZE, 0.25), (pid::REVERB_DECAY, 0.2),
        (pid::REVERB_DAMP, 0.6), (pid::REVERB_WIDTH, 0.8), (pid::REVERB_PREDELAY, 0.003),
    ]},
    EffectPreset { name: "Big Snare Hall", params: &[
        (pid::REVERB_TYPE, 0.0), (pid::REVERB_SIZE, 0.7), (pid::REVERB_DECAY, 0.5),
        (pid::REVERB_DAMP, 0.45), (pid::REVERB_WIDTH, 1.5), (pid::REVERB_PREDELAY, 0.02),
    ]},
    EffectPreset { name: "Plate Clap", params: &[
        (pid::REVERB_TYPE, 2.0), (pid::REVERB_SIZE, 0.5), (pid::REVERB_DECAY, 0.4),
        (pid::REVERB_DAMP, 0.5), (pid::REVERB_WIDTH, 1.2), (pid::REVERB_PREDELAY, 0.01),
    ]},
    EffectPreset { name: "Ambient Perc", params: &[
        (pid::REVERB_TYPE, 0.0), (pid::REVERB_SIZE, 0.85), (pid::REVERB_DECAY, 0.7),
        (pid::REVERB_DAMP, 0.35), (pid::REVERB_WIDTH, 1.8), (pid::REVERB_PREDELAY, 0.03),
    ]},
];

static DRUM_CHORUS_PRESETS: [EffectPreset; 3] = [
    EffectPreset { name: "HiHat Width", params: &[
        (pid::CHORUS_RATE, 0.8), (pid::CHORUS_DEPTH, 0.3), (pid::CHORUS_WIDTH, 1.0),
        (pid::CHORUS_DELAY, 0.005), (pid::CHORUS_VOICES, 2.0), (pid::CHORUS_MIX, 0.3),
    ]},
    EffectPreset { name: "Snare Thicken", params: &[
        (pid::CHORUS_RATE, 0.4), (pid::CHORUS_DEPTH, 0.4), (pid::CHORUS_WIDTH, 0.8),
        (pid::CHORUS_DELAY, 0.01), (pid::CHORUS_VOICES, 3.0), (pid::CHORUS_MIX, 0.4),
    ]},
    EffectPreset { name: "Tom Ensemble", params: &[
        (pid::CHORUS_RATE, 0.6), (pid::CHORUS_DEPTH, 0.5), (pid::CHORUS_WIDTH, 1.0),
        (pid::CHORUS_DELAY, 0.015), (pid::CHORUS_VOICES, 4.0), (pid::CHORUS_MIX, 0.4),
    ]},
];

static DRUM_PHASER_PRESETS: [EffectPreset; 3] = [
    EffectPreset { name: "HiHat Shimmer", params: &[
        (pid::PHASER_RATE, 1.5), (pid::PHASER_DEPTH, 0.5), (pid::PHASER_FEEDBACK, 0.4),
        (pid::PHASER_STEREO, 0.6), (pid::PHASER_STAGES, 4.0), (pid::PHASER_MIX, 0.4),
    ]},
    EffectPreset { name: "Snare Phase", params: &[
        (pid::PHASER_RATE, 0.3), (pid::PHASER_DEPTH, 0.7), (pid::PHASER_FEEDBACK, 0.6),
        (pid::PHASER_STEREO, 0.5), (pid::PHASER_STAGES, 6.0), (pid::PHASER_MIX, 0.5),
    ]},
    EffectPreset { name: "Cymbal Sweep", params: &[
        (pid::PHASER_RATE, 0.15), (pid::PHASER_DEPTH, 0.8), (pid::PHASER_FEEDBACK, 0.7),
        (pid::PHASER_STEREO, 0.7), (pid::PHASER_STAGES, 8.0), (pid::PHASER_MIX, 0.5),
    ]},
];

static DRUM_FLANGER_PRESETS: [EffectPreset; 3] = [
    EffectPreset { name: "Metallic HH", params: &[
        (pid::FLANGER_RATE, 0.3), (pid::FLANGER_DEPTH, 0.5), (pid::FLANGER_DELAY, 0.001),
        (pid::FLANGER_FEEDBACK, 0.8), (pid::FLANGER_STEREO, 0.4), (pid::FLANGER_MIX, 0.5),
    ]},
    EffectPreset { name: "Snare Jet", params: &[
        (pid::FLANGER_RATE, 0.1), (pid::FLANGER_DEPTH, 0.7), (pid::FLANGER_DELAY, 0.003),
        (pid::FLANGER_FEEDBACK, 0.7), (pid::FLANGER_STEREO, 0.5), (pid::FLANGER_MIX, 0.4),
    ]},
    EffectPreset { name: "Tom Sweep", params: &[
        (pid::FLANGER_RATE, 0.2), (pid::FLANGER_DEPTH, 0.6), (pid::FLANGER_DELAY, 0.004),
        (pid::FLANGER_FEEDBACK, 0.5), (pid::FLANGER_STEREO, 0.6), (pid::FLANGER_MIX, 0.4),
    ]},
];

static DRUM_DISTORTION_PRESETS: [EffectPreset; 4] = [
    EffectPreset { name: "Kick Grit", params: &[
        (pid::DIST_TYPE, 2.0), (pid::DIST_DRIVE, 3.0), (pid::DIST_TONE, 2000.0),
        (pid::DIST_OUTPUT, 0.8), (pid::DIST_MIX, 0.4),
    ]},
    EffectPreset { name: "Snare Crunch", params: &[
        (pid::DIST_TYPE, 1.0), (pid::DIST_DRIVE, 5.0), (pid::DIST_TONE, 4000.0),
        (pid::DIST_OUTPUT, 0.6), (pid::DIST_MIX, 0.5),
    ]},
    EffectPreset { name: "Lo-Fi Drums", params: &[
        (pid::DIST_TYPE, 4.0), (pid::DIST_DRIVE, 1.0), (pid::DIST_TONE, 3000.0),
        (pid::DIST_OUTPUT, 0.7), (pid::DIST_BITS, 10.0), (pid::DIST_SR_DIV, 2.0), (pid::DIST_MIX, 0.6),
    ]},
    EffectPreset { name: "Tape Warmth", params: &[
        (pid::DIST_TYPE, 2.0), (pid::DIST_DRIVE, 2.0), (pid::DIST_TONE, 5000.0),
        (pid::DIST_OUTPUT, 0.85), (pid::DIST_MIX, 0.3),
    ]},
];

static DRUM_COMPRESSOR_PRESETS: [EffectPreset; 4] = [
    EffectPreset { name: "Kick Punch", params: &[
        (pid::COMP_MODE, 2.0), (pid::COMP_THRESH, -12.0), (pid::COMP_RATIO, 6.0),
        (pid::COMP_ATTACK, 0.005), (pid::COMP_RELEASE, 0.08), (pid::COMP_MAKEUP, 4.0), (pid::COMP_KNEE, 3.0),
    ]},
    EffectPreset { name: "Snare Snap", params: &[
        (pid::COMP_MODE, 2.0), (pid::COMP_THRESH, -15.0), (pid::COMP_RATIO, 5.0),
        (pid::COMP_ATTACK, 0.001), (pid::COMP_RELEASE, 0.06), (pid::COMP_MAKEUP, 5.0), (pid::COMP_KNEE, 2.0),
    ]},
    EffectPreset { name: "Parallel Crush", params: &[
        (pid::COMP_MODE, 2.0), (pid::COMP_THRESH, -8.0), (pid::COMP_RATIO, 10.0),
        (pid::COMP_ATTACK, 0.001), (pid::COMP_RELEASE, 0.04), (pid::COMP_MAKEUP, 10.0), (pid::COMP_KNEE, 1.0),
    ]},
    EffectPreset { name: "Tom Control", params: &[
        (pid::COMP_MODE, 0.0), (pid::COMP_THRESH, -18.0), (pid::COMP_RATIO, 3.0),
        (pid::COMP_ATTACK, 0.003), (pid::COMP_RELEASE, 0.12), (pid::COMP_MAKEUP, 3.0), (pid::COMP_KNEE, 6.0),
    ]},
];

static DRUM_LIMITER_PRESETS: [EffectPreset; 3] = [
    EffectPreset { name: "Drum Ceiling", params: &[
        (pid::LIMITER_MODE, 0.0), (pid::LIMITER_CEIL, -0.5), (pid::LIMITER_RELEASE, 0.03),
        (pid::LIMITER_GAIN, 0.0),
    ]},
    EffectPreset { name: "Loud Drums", params: &[
        (pid::LIMITER_MODE, 2.0), (pid::LIMITER_CEIL, -0.3), (pid::LIMITER_RELEASE, 0.04),
        (pid::LIMITER_GAIN, 8.0),
    ]},
    EffectPreset { name: "Soft Clip", params: &[
        (pid::LIMITER_MODE, 1.0), (pid::LIMITER_CEIL, -1.0), (pid::LIMITER_RELEASE, 0.06),
        (pid::LIMITER_GAIN, 4.0),
    ]},
];

static DRUM_EXPANDER_PRESETS: [EffectPreset; 3] = [
    EffectPreset { name: "Kick Gate", params: &[
        (pid::EXP_MODE, 0.0), (pid::EXP_THRESH, -35.0), (pid::EXP_RATIO, 8.0),
        (pid::EXP_ATTACK, 0.0005), (pid::EXP_RELEASE, 0.03), (pid::EXP_RANGE, -50.0), (pid::EXP_KNEE, 2.0),
    ]},
    EffectPreset { name: "Snare Gate", params: &[
        (pid::EXP_MODE, 0.0), (pid::EXP_THRESH, -30.0), (pid::EXP_RATIO, 6.0),
        (pid::EXP_ATTACK, 0.001), (pid::EXP_RELEASE, 0.04), (pid::EXP_RANGE, -40.0), (pid::EXP_KNEE, 3.0),
    ]},
    EffectPreset { name: "Tom Tighten", params: &[
        (pid::EXP_MODE, 0.0), (pid::EXP_THRESH, -28.0), (pid::EXP_RATIO, 5.0),
        (pid::EXP_ATTACK, 0.001), (pid::EXP_RELEASE, 0.05), (pid::EXP_RANGE, -35.0), (pid::EXP_KNEE, 3.0),
    ]},
];

// ═══════════════════════════════════════════════════════════════════════
// Drum Bus FX chain presets
// ═══════════════════════════════════════════════════════════════════════

pub fn get_drum_bus_preset_categories() -> &'static [FxPresetCategory] {
    &[
        FxPresetCategory { name: "Bus Processing", presets: &[
            "Drum Bus Glue", "Punchy Bus", "Warm Bus", "Lo-Fi Bus",
        ]},
        FxPresetCategory { name: "Character", presets: &[
            "Tape Machine", "NYC Crush", "Room Ambience", "Dub Drums",
        ]},
    ]
}

pub fn find_drum_bus_preset(name: &str) -> Option<&'static FxPreset> {
    DRUM_BUS_PRESETS.iter().find(|p| p.name == name)
}

static DRUM_BUS_PRESETS: [FxPreset; 8] = [
    FxPreset { name: "Drum Bus Glue", slots: &[
        FxSlotPreset { effect_type: 8, mix: 1.0, params: &[
            (pid::COMP_MODE, 0.0), (pid::COMP_THRESH, -18.0), (pid::COMP_RATIO, 2.5),
            (pid::COMP_ATTACK, 0.01), (pid::COMP_RELEASE, 0.15), (pid::COMP_MAKEUP, 3.0), (pid::COMP_KNEE, 10.0),
        ]},
    ]},
    FxPreset { name: "Punchy Bus", slots: &[
        FxSlotPreset { effect_type: 8, mix: 1.0, params: &[
            (pid::COMP_MODE, 2.0), (pid::COMP_THRESH, -14.0), (pid::COMP_RATIO, 5.0),
            (pid::COMP_ATTACK, 0.003), (pid::COMP_RELEASE, 0.08), (pid::COMP_MAKEUP, 5.0), (pid::COMP_KNEE, 3.0),
        ]},
        FxSlotPreset { effect_type: 9, mix: 1.0, params: &[
            (pid::LIMITER_MODE, 0.0), (pid::LIMITER_CEIL, -0.5), (pid::LIMITER_RELEASE, 0.03), (pid::LIMITER_GAIN, 3.0),
        ]},
    ]},
    FxPreset { name: "Warm Bus", slots: &[
        FxSlotPreset { effect_type: 6, mix: 0.2, params: &[
            (pid::DIST_TYPE, 2.0), (pid::DIST_DRIVE, 2.0), (pid::DIST_TONE, 4500.0), (pid::DIST_OUTPUT, 0.85),
        ]},
        FxSlotPreset { effect_type: 8, mix: 1.0, params: &[
            (pid::COMP_MODE, 1.0), (pid::COMP_THRESH, -16.0), (pid::COMP_RATIO, 3.0),
            (pid::COMP_ATTACK, 0.008), (pid::COMP_RELEASE, 0.2), (pid::COMP_MAKEUP, 3.0), (pid::COMP_KNEE, 8.0),
        ]},
    ]},
    FxPreset { name: "Lo-Fi Bus", slots: &[
        FxSlotPreset { effect_type: 6, mix: 0.4, params: &[
            (pid::DIST_TYPE, 4.0), (pid::DIST_DRIVE, 1.0), (pid::DIST_TONE, 3000.0),
            (pid::DIST_OUTPUT, 0.7), (pid::DIST_BITS, 12.0), (pid::DIST_SR_DIV, 2.0),
        ]},
        FxSlotPreset { effect_type: 8, mix: 1.0, params: &[
            (pid::COMP_MODE, 2.0), (pid::COMP_THRESH, -12.0), (pid::COMP_RATIO, 4.0),
            (pid::COMP_ATTACK, 0.005), (pid::COMP_RELEASE, 0.1), (pid::COMP_MAKEUP, 4.0), (pid::COMP_KNEE, 3.0),
        ]},
    ]},
    FxPreset { name: "Tape Machine", slots: &[
        FxSlotPreset { effect_type: 6, mix: 0.3, params: &[
            (pid::DIST_TYPE, 2.0), (pid::DIST_DRIVE, 3.5), (pid::DIST_TONE, 3500.0), (pid::DIST_OUTPUT, 0.75),
        ]},
        FxSlotPreset { effect_type: 3, mix: 0.15, params: &[
            (pid::CHORUS_RATE, 0.3), (pid::CHORUS_DEPTH, 0.2), (pid::CHORUS_WIDTH, 0.5),
            (pid::CHORUS_DELAY, 0.008), (pid::CHORUS_VOICES, 2.0),
        ]},
    ]},
    FxPreset { name: "NYC Crush", slots: &[
        FxSlotPreset { effect_type: 8, mix: 1.0, params: &[
            (pid::COMP_MODE, 2.0), (pid::COMP_THRESH, -8.0), (pid::COMP_RATIO, 10.0),
            (pid::COMP_ATTACK, 0.001), (pid::COMP_RELEASE, 0.04), (pid::COMP_MAKEUP, 10.0), (pid::COMP_KNEE, 1.0),
        ]},
        FxSlotPreset { effect_type: 6, mix: 0.15, params: &[
            (pid::DIST_TYPE, 2.0), (pid::DIST_DRIVE, 4.0), (pid::DIST_TONE, 5000.0), (pid::DIST_OUTPUT, 0.7),
        ]},
    ]},
    FxPreset { name: "Room Ambience", slots: &[
        FxSlotPreset { effect_type: 2, mix: 0.25, params: &[
            (pid::REVERB_TYPE, 1.0), (pid::REVERB_SIZE, 0.3), (pid::REVERB_DECAY, 0.2),
            (pid::REVERB_DAMP, 0.6), (pid::REVERB_WIDTH, 1.0), (pid::REVERB_PREDELAY, 0.005),
        ]},
        FxSlotPreset { effect_type: 8, mix: 1.0, params: &[
            (pid::COMP_MODE, 0.0), (pid::COMP_THRESH, -20.0), (pid::COMP_RATIO, 2.0),
            (pid::COMP_ATTACK, 0.01), (pid::COMP_RELEASE, 0.2), (pid::COMP_MAKEUP, 2.0), (pid::COMP_KNEE, 10.0),
        ]},
    ]},
    FxPreset { name: "Dub Drums", slots: &[
        FxSlotPreset { effect_type: 1, mix: 0.3, params: &[
            (pid::DELAY_TYPE, 1.0), (pid::DELAY_TIME, 0.375), (pid::DELAY_FEEDBACK, 0.55),
            (pid::DELAY_STEREO, 0.8), (pid::DELAY_HICUT, 3000.0), (pid::DELAY_LOCUT, 400.0),
        ]},
        FxSlotPreset { effect_type: 2, mix: 0.3, params: &[
            (pid::REVERB_TYPE, 3.0), (pid::REVERB_SIZE, 0.5), (pid::REVERB_DECAY, 0.4),
            (pid::REVERB_DAMP, 0.5), (pid::REVERB_WIDTH, 1.2), (pid::REVERB_PREDELAY, 0.01),
        ]},
    ]},
];

// ═══════════════════════════════════════════════════════════════════════
// All FX presets
// ═══════════════════════════════════════════════════════════════════════

static ALL_FX_PRESETS: [FxPreset; 46] = [
    // ═══════ MASTERING ═══════
    FxPreset {
        name: "Loud Master",
        slots: &[
            FxSlotPreset {
                effect_type: 8, // Compressor
                mix: 1.0,
                params: &[
                    (pid::COMP_MODE, 0.0),      // VCA
                    (pid::COMP_THRESH, -12.0),
                    (pid::COMP_RATIO, 4.0),
                    (pid::COMP_ATTACK, 0.01),
                    (pid::COMP_RELEASE, 0.15),
                    (pid::COMP_MAKEUP, 6.0),
                    (pid::COMP_KNEE, 6.0),
                ],
            },
            FxSlotPreset {
                effect_type: 9, // Limiter
                mix: 1.0,
                params: &[
                    (pid::LIMITER_MODE, 2.0),    // Maximizer
                    (pid::LIMITER_CEIL, -0.3),
                    (pid::LIMITER_RELEASE, 0.05),
                    (pid::LIMITER_GAIN, 12.0),
                ],
            },
        ],
    },
    FxPreset {
        name: "Clean Master",
        slots: &[
            FxSlotPreset {
                effect_type: 8, // Compressor
                mix: 1.0,
                params: &[
                    (pid::COMP_MODE, 0.0),      // VCA
                    (pid::COMP_THRESH, -20.0),
                    (pid::COMP_RATIO, 2.0),
                    (pid::COMP_ATTACK, 0.01),
                    (pid::COMP_RELEASE, 0.2),
                    (pid::COMP_MAKEUP, 2.0),
                    (pid::COMP_KNEE, 12.0),
                ],
            },
            FxSlotPreset {
                effect_type: 9, // Limiter
                mix: 1.0,
                params: &[
                    (pid::LIMITER_MODE, 0.0),    // Brickwall
                    (pid::LIMITER_CEIL, -0.3),
                    (pid::LIMITER_RELEASE, 0.1),
                    (pid::LIMITER_GAIN, 3.0),
                ],
            },
        ],
    },
    FxPreset {
        name: "Warm Master",
        slots: &[
            FxSlotPreset {
                effect_type: 6, // Distortion (for saturation)
                mix: 0.15,
                params: &[
                    (pid::DIST_TYPE, 2.0),       // Tape
                    (pid::DIST_DRIVE, 2.0),
                    (pid::DIST_TONE, 4000.0),
                    (pid::DIST_OUTPUT, 0.8),
                ],
            },
            FxSlotPreset {
                effect_type: 8, // Compressor
                mix: 1.0,
                params: &[
                    (pid::COMP_MODE, 1.0),      // Opto
                    (pid::COMP_THRESH, -18.0),
                    (pid::COMP_RATIO, 3.0),
                    (pid::COMP_ATTACK, 0.015),
                    (pid::COMP_RELEASE, 0.3),
                    (pid::COMP_MAKEUP, 4.0),
                    (pid::COMP_KNEE, 10.0),
                ],
            },
            FxSlotPreset {
                effect_type: 9, // Limiter
                mix: 1.0,
                params: &[
                    (pid::LIMITER_MODE, 1.0),    // SoftClip
                    (pid::LIMITER_CEIL, -0.5),
                    (pid::LIMITER_RELEASE, 0.08),
                    (pid::LIMITER_GAIN, 6.0),
                ],
            },
        ],
    },
    FxPreset {
        name: "Punchy Master",
        slots: &[
            FxSlotPreset {
                effect_type: 8, // Compressor
                mix: 1.0,
                params: &[
                    (pid::COMP_MODE, 2.0),      // FET
                    (pid::COMP_THRESH, -15.0),
                    (pid::COMP_RATIO, 6.0),
                    (pid::COMP_ATTACK, 0.003),
                    (pid::COMP_RELEASE, 0.1),
                    (pid::COMP_MAKEUP, 5.0),
                    (pid::COMP_KNEE, 3.0),
                ],
            },
            FxSlotPreset {
                effect_type: 9, // Limiter
                mix: 1.0,
                params: &[
                    (pid::LIMITER_MODE, 0.0),    // Brickwall
                    (pid::LIMITER_CEIL, -0.3),
                    (pid::LIMITER_RELEASE, 0.03),
                    (pid::LIMITER_GAIN, 8.0),
                ],
            },
        ],
    },

    // ─── Pro suite presets ───────────────────────────────────────────
    // Use the new mastering-tier effects (DynamicEq, MidSideEq,
    // MultibandComp3, MasterAssistant, BrickwallLimiter) instead of
    // the legacy single-band Compressor + soft Limiter chain.
    //
    // EffectType indices used below — must match `EffectType::all()`:
    //   30 = DynamicEq        31 = MidSideEq      32 = MultibandComp3
    //   33 = SpectrumAnalyzer 29 = LufsMeter      28 = BrickwallLimiter
    //   34 = MasterAssistant

    FxPreset {
        name: "Pro Master Smart",
        // One-button mastering: the assistant proposes a -14 LUFS
        // target on a Balanced curve while the multiband glues and
        // the brick-wall enforces the ceiling. Best on a finished mix
        // that needs minimal intervention.
        slots: &[
            FxSlotPreset {
                effect_type: 31, // MidSideEq — tighten low
                mix: 1.0,
                params: &[
                    // M band 0: low-shelf gentle +1 dB at 80 Hz to
                    // glue the sub centre.
                    (pid::MSEQ_M_B0_FREQ, 80.0),
                    (pid::MSEQ_M_B0_GAIN, 1.0),
                    (pid::MSEQ_M_B0_Q, 0.7),
                    (pid::MSEQ_MIX, 1.0),
                ],
            },
            FxSlotPreset {
                effect_type: 32, // MultibandComp3 — gentle glue
                mix: 1.0,
                params: &[
                    (pid::MBC_XOVER_LOW, 200.0),
                    (pid::MBC_XOVER_HIGH, 2000.0),
                    (pid::MBC_B0_THRESH, -14.0), (pid::MBC_B0_RATIO, 1.5),
                    (pid::MBC_B0_ATK, 0.015), (pid::MBC_B0_REL, 0.200),
                    (pid::MBC_B0_MAKEUP, 0.0), (pid::MBC_B0_ENABLED, 1.0),
                    (pid::MBC_B0_THRESH + pid::MBC_BAND_STRIDE,     -16.0),
                    (pid::MBC_B0_RATIO + pid::MBC_BAND_STRIDE,       1.5),
                    (pid::MBC_B0_ATK + pid::MBC_BAND_STRIDE,         0.010),
                    (pid::MBC_B0_REL + pid::MBC_BAND_STRIDE,         0.150),
                    (pid::MBC_B0_MAKEUP + pid::MBC_BAND_STRIDE,      0.0),
                    (pid::MBC_B0_ENABLED + pid::MBC_BAND_STRIDE,     1.0),
                    (pid::MBC_B0_THRESH + 2 * pid::MBC_BAND_STRIDE, -18.0),
                    (pid::MBC_B0_RATIO + 2 * pid::MBC_BAND_STRIDE,   1.5),
                    (pid::MBC_B0_ATK + 2 * pid::MBC_BAND_STRIDE,     0.005),
                    (pid::MBC_B0_REL + 2 * pid::MBC_BAND_STRIDE,     0.100),
                    (pid::MBC_B0_MAKEUP + 2 * pid::MBC_BAND_STRIDE,  0.0),
                    (pid::MBC_B0_ENABLED + 2 * pid::MBC_BAND_STRIDE, 1.0),
                    (pid::MBC_MIX, 1.0),
                ],
            },
            FxSlotPreset {
                effect_type: 34, // MasterAssistant — Apply, -14 LUFS, Balanced
                mix: 1.0,
                params: &[
                    (pid::MA_MODE,         2.0),   // Apply
                    (pid::MA_TARGET_LUFS, -14.0),
                    (pid::MA_TARGET_CURVE, 2.0),   // Balanced
                    (pid::MA_MIX, 1.0),
                ],
            },
            FxSlotPreset {
                effect_type: 28, // BrickwallLimiter — ceiling guard
                mix: 1.0,
                params: &[
                    (pid::BRICK_CEILING_DB, -0.3),
                    (pid::BRICK_RELEASE_MS, 50.0),
                    (pid::BRICK_MIX, 1.0),
                ],
            },
        ],
    },
    FxPreset {
        name: "Pro Master Club",
        // Loudness-first; the assistant aims at -8 LUFS with the
        // Modern Techno curve and the multiband works harder on the
        // low end. Brick-wall has a fast release for transient punch.
        slots: &[
            FxSlotPreset {
                effect_type: 31, // MidSideEq — mono-ize sub, widen highs
                mix: 1.0,
                params: &[
                    (pid::MSEQ_M_B0_FREQ, 60.0),
                    (pid::MSEQ_M_B0_GAIN, 2.0),
                    (pid::MSEQ_M_B0_Q, 0.7),
                    (pid::MSEQ_S_B0_FREQ, 10000.0),
                    (pid::MSEQ_S_B0_GAIN, 1.5),
                    (pid::MSEQ_S_B0_Q, 0.7),
                    (pid::MSEQ_MIX, 1.0),
                ],
            },
            FxSlotPreset {
                effect_type: 32, // MultibandComp3 — harder low
                mix: 1.0,
                params: &[
                    (pid::MBC_XOVER_LOW, 200.0),
                    (pid::MBC_XOVER_HIGH, 2500.0),
                    (pid::MBC_B0_THRESH, -10.0), (pid::MBC_B0_RATIO, 3.0),
                    (pid::MBC_B0_ATK, 0.010), (pid::MBC_B0_REL, 0.150),
                    (pid::MBC_B0_MAKEUP, 2.0), (pid::MBC_B0_ENABLED, 1.0),
                    (pid::MBC_B0_THRESH + pid::MBC_BAND_STRIDE,     -14.0),
                    (pid::MBC_B0_RATIO + pid::MBC_BAND_STRIDE,       2.0),
                    (pid::MBC_B0_ATK + pid::MBC_BAND_STRIDE,         0.008),
                    (pid::MBC_B0_REL + pid::MBC_BAND_STRIDE,         0.120),
                    (pid::MBC_B0_MAKEUP + pid::MBC_BAND_STRIDE,      1.0),
                    (pid::MBC_B0_ENABLED + pid::MBC_BAND_STRIDE,     1.0),
                    (pid::MBC_B0_THRESH + 2 * pid::MBC_BAND_STRIDE, -16.0),
                    (pid::MBC_B0_RATIO + 2 * pid::MBC_BAND_STRIDE,   2.0),
                    (pid::MBC_B0_ATK + 2 * pid::MBC_BAND_STRIDE,     0.004),
                    (pid::MBC_B0_REL + 2 * pid::MBC_BAND_STRIDE,     0.080),
                    (pid::MBC_B0_MAKEUP + 2 * pid::MBC_BAND_STRIDE,  1.0),
                    (pid::MBC_B0_ENABLED + 2 * pid::MBC_BAND_STRIDE, 1.0),
                    (pid::MBC_MIX, 1.0),
                ],
            },
            FxSlotPreset {
                effect_type: 34, // MasterAssistant — Apply, -8 LUFS, Modern Techno
                mix: 1.0,
                params: &[
                    (pid::MA_MODE,         2.0),    // Apply
                    (pid::MA_TARGET_LUFS, -8.0),    // Club-loud
                    (pid::MA_TARGET_CURVE, 0.0),    // Modern Techno
                    (pid::MA_MIX, 1.0),
                ],
            },
            FxSlotPreset {
                effect_type: 28, // BrickwallLimiter — fast release
                mix: 1.0,
                params: &[
                    (pid::BRICK_CEILING_DB, -0.3),
                    (pid::BRICK_RELEASE_MS, 20.0),
                    (pid::BRICK_MIX, 1.0),
                ],
            },
        ],
    },
    FxPreset {
        name: "Pro Master Streaming",
        // Conservative chain calibrated for streaming platforms
        // (Spotify / Apple Music normalise to ~-14 LUFS). Wider safety
        // ceiling at -1 dB so re-encoding doesn't clip.
        slots: &[
            FxSlotPreset {
                effect_type: 30, // DynamicEq — tame 2-5 kHz harshness
                mix: 1.0,
                params: &[
                    (pid::DYNEQ_B0_FREQ, 120.0),    (pid::DYNEQ_B0_Q, 1.0),
                    (pid::DYNEQ_B0_THRESH, -18.0),  (pid::DYNEQ_B0_RATIO, 1.5),
                    (pid::DYNEQ_B0_RANGE, 3.0),
                    (pid::DYNEQ_B0_ATK, 0.020),     (pid::DYNEQ_B0_REL, 0.200),
                    (pid::DYNEQ_B0_MODE, 0.0),  // disabled
                    (pid::DYNEQ_B0_FREQ + pid::DYNEQ_BAND_STRIDE,     400.0),
                    (pid::DYNEQ_B0_Q + pid::DYNEQ_BAND_STRIDE,        1.0),
                    (pid::DYNEQ_B0_THRESH + pid::DYNEQ_BAND_STRIDE,  -20.0),
                    (pid::DYNEQ_B0_RATIO + pid::DYNEQ_BAND_STRIDE,    2.0),
                    (pid::DYNEQ_B0_RANGE + pid::DYNEQ_BAND_STRIDE,    3.0),
                    (pid::DYNEQ_B0_ATK + pid::DYNEQ_BAND_STRIDE,      0.020),
                    (pid::DYNEQ_B0_REL + pid::DYNEQ_BAND_STRIDE,      0.200),
                    (pid::DYNEQ_B0_MODE + pid::DYNEQ_BAND_STRIDE,     0.0),
                    (pid::DYNEQ_B0_FREQ + 2 * pid::DYNEQ_BAND_STRIDE,   3000.0),
                    (pid::DYNEQ_B0_Q + 2 * pid::DYNEQ_BAND_STRIDE,      1.2),
                    (pid::DYNEQ_B0_THRESH + 2 * pid::DYNEQ_BAND_STRIDE,-15.0),
                    (pid::DYNEQ_B0_RATIO + 2 * pid::DYNEQ_BAND_STRIDE,  2.5),
                    (pid::DYNEQ_B0_RANGE + 2 * pid::DYNEQ_BAND_STRIDE,  4.0),
                    (pid::DYNEQ_B0_ATK + 2 * pid::DYNEQ_BAND_STRIDE,    0.015),
                    (pid::DYNEQ_B0_REL + 2 * pid::DYNEQ_BAND_STRIDE,    0.150),
                    (pid::DYNEQ_B0_MODE + 2 * pid::DYNEQ_BAND_STRIDE,   1.0),
                    (pid::DYNEQ_B0_FREQ + 3 * pid::DYNEQ_BAND_STRIDE,   8000.0),
                    (pid::DYNEQ_B0_Q + 3 * pid::DYNEQ_BAND_STRIDE,      1.0),
                    (pid::DYNEQ_B0_THRESH + 3 * pid::DYNEQ_BAND_STRIDE,-18.0),
                    (pid::DYNEQ_B0_RATIO + 3 * pid::DYNEQ_BAND_STRIDE,  2.0),
                    (pid::DYNEQ_B0_RANGE + 3 * pid::DYNEQ_BAND_STRIDE,  3.0),
                    (pid::DYNEQ_B0_ATK + 3 * pid::DYNEQ_BAND_STRIDE,    0.010),
                    (pid::DYNEQ_B0_REL + 3 * pid::DYNEQ_BAND_STRIDE,    0.150),
                    (pid::DYNEQ_B0_MODE + 3 * pid::DYNEQ_BAND_STRIDE,   0.0),
                    (pid::DYNEQ_MIX, 1.0),
                ],
            },
            FxSlotPreset {
                effect_type: 32, // MultibandComp3 — gentle 1.5:1
                mix: 1.0,
                params: &[
                    (pid::MBC_XOVER_LOW, 200.0),
                    (pid::MBC_XOVER_HIGH, 2000.0),
                    (pid::MBC_B0_THRESH, -16.0), (pid::MBC_B0_RATIO, 1.5),
                    (pid::MBC_B0_ATK, 0.020), (pid::MBC_B0_REL, 0.250),
                    (pid::MBC_B0_MAKEUP, 0.0), (pid::MBC_B0_ENABLED, 1.0),
                    (pid::MBC_B0_THRESH + pid::MBC_BAND_STRIDE,     -18.0),
                    (pid::MBC_B0_RATIO + pid::MBC_BAND_STRIDE,       1.5),
                    (pid::MBC_B0_ATK + pid::MBC_BAND_STRIDE,         0.015),
                    (pid::MBC_B0_REL + pid::MBC_BAND_STRIDE,         0.200),
                    (pid::MBC_B0_MAKEUP + pid::MBC_BAND_STRIDE,      0.0),
                    (pid::MBC_B0_ENABLED + pid::MBC_BAND_STRIDE,     1.0),
                    (pid::MBC_B0_THRESH + 2 * pid::MBC_BAND_STRIDE, -20.0),
                    (pid::MBC_B0_RATIO + 2 * pid::MBC_BAND_STRIDE,   1.5),
                    (pid::MBC_B0_ATK + 2 * pid::MBC_BAND_STRIDE,     0.010),
                    (pid::MBC_B0_REL + 2 * pid::MBC_BAND_STRIDE,     0.150),
                    (pid::MBC_B0_MAKEUP + 2 * pid::MBC_BAND_STRIDE,  0.0),
                    (pid::MBC_B0_ENABLED + 2 * pid::MBC_BAND_STRIDE, 1.0),
                    (pid::MBC_MIX, 1.0),
                ],
            },
            FxSlotPreset {
                effect_type: 34, // MasterAssistant — Apply, -14 LUFS, Modern Techno
                mix: 1.0,
                params: &[
                    (pid::MA_MODE,         2.0),
                    (pid::MA_TARGET_LUFS, -14.0),  // Streaming-safe
                    (pid::MA_TARGET_CURVE, 0.0),
                    (pid::MA_MIX, 1.0),
                ],
            },
            FxSlotPreset {
                effect_type: 28, // BrickwallLimiter — wider safety
                mix: 1.0,
                params: &[
                    (pid::BRICK_CEILING_DB, -1.0),  // Re-encode headroom
                    (pid::BRICK_RELEASE_MS, 80.0),
                    (pid::BRICK_MIX, 1.0),
                ],
            },
        ],
    },
    FxPreset {
        name: "Pro Master Reference",
        // Analytic / metering-only chain. No DSP modifies the audio.
        // Drop this on the master bus while mixing to read LUFS,
        // spectrum, and the assistant's proposed corrections without
        // ever touching the sound.
        slots: &[
            FxSlotPreset {
                effect_type: 29, // LufsMeter
                mix: 1.0,
                params: &[(pid::LUFS_MIX, 1.0)],
            },
            FxSlotPreset {
                effect_type: 33, // SpectrumAnalyzer
                mix: 1.0,
                params: &[(pid::SPECTRUM_MIX, 1.0)],
            },
            FxSlotPreset {
                effect_type: 34, // MasterAssistant — Listen only
                mix: 1.0,
                params: &[
                    (pid::MA_MODE,         1.0),   // Listen
                    (pid::MA_TARGET_LUFS, -14.0),
                    (pid::MA_TARGET_CURVE, 2.0),   // Balanced reference
                    (pid::MA_MIX, 1.0),
                ],
            },
        ],
    },

    // ═══════ MIXING ═══════
    FxPreset {
        name: "Vocal Chain",
        slots: &[
            FxSlotPreset {
                effect_type: 8, // Compressor
                mix: 1.0,
                params: &[
                    (pid::COMP_MODE, 1.0),      // Opto
                    (pid::COMP_THRESH, -20.0),
                    (pid::COMP_RATIO, 3.0),
                    (pid::COMP_ATTACK, 0.005),
                    (pid::COMP_RELEASE, 0.2),
                    (pid::COMP_MAKEUP, 4.0),
                    (pid::COMP_KNEE, 6.0),
                ],
            },
            FxSlotPreset {
                effect_type: 1, // Delay
                mix: 0.15,
                params: &[
                    (pid::DELAY_TYPE, 0.0),      // Simple
                    (pid::DELAY_TIME, 0.08),
                    (pid::DELAY_FEEDBACK, 0.1),
                    (pid::DELAY_HICUT, 6000.0),
                    (pid::DELAY_LOCUT, 300.0),
                ],
            },
            FxSlotPreset {
                effect_type: 2, // Reverb
                mix: 0.25,
                params: &[
                    (pid::REVERB_TYPE, 2.0),     // Plate
                    (pid::REVERB_SIZE, 0.4),
                    (pid::REVERB_DECAY, 0.35),
                    (pid::REVERB_DAMP, 0.6),
                    (pid::REVERB_WIDTH, 1.0),
                    (pid::REVERB_PREDELAY, 0.03),
                ],
            },
        ],
    },
    FxPreset {
        name: "Drum Bus",
        slots: &[
            FxSlotPreset {
                effect_type: 8, // Compressor
                mix: 1.0,
                params: &[
                    (pid::COMP_MODE, 2.0),      // FET
                    (pid::COMP_THRESH, -18.0),
                    (pid::COMP_RATIO, 4.0),
                    (pid::COMP_ATTACK, 0.001),
                    (pid::COMP_RELEASE, 0.08),
                    (pid::COMP_MAKEUP, 4.0),
                    (pid::COMP_KNEE, 3.0),
                ],
            },
            FxSlotPreset {
                effect_type: 6, // Distortion (saturation)
                mix: 0.2,
                params: &[
                    (pid::DIST_TYPE, 2.0),       // Tape
                    (pid::DIST_DRIVE, 3.0),
                    (pid::DIST_TONE, 6000.0),
                    (pid::DIST_OUTPUT, 0.7),
                ],
            },
        ],
    },
    FxPreset {
        name: "Guitar Crunch",
        slots: &[
            FxSlotPreset {
                effect_type: 6, // Distortion
                mix: 0.7,
                params: &[
                    (pid::DIST_TYPE, 1.0),       // Tube
                    (pid::DIST_DRIVE, 8.0),
                    (pid::DIST_TONE, 3500.0),
                    (pid::DIST_OUTPUT, 0.6),
                ],
            },
            FxSlotPreset {
                effect_type: 3, // Chorus
                mix: 0.2,
                params: &[
                    (pid::CHORUS_RATE, 0.8),
                    (pid::CHORUS_DEPTH, 0.3),
                    (pid::CHORUS_WIDTH, 0.8),
                    (pid::CHORUS_DELAY, 0.008),
                    (pid::CHORUS_VOICES, 2.0),
                ],
            },
            FxSlotPreset {
                effect_type: 2, // Reverb
                mix: 0.2,
                params: &[
                    (pid::REVERB_TYPE, 1.0),     // Room
                    (pid::REVERB_SIZE, 0.3),
                    (pid::REVERB_DECAY, 0.3),
                    (pid::REVERB_DAMP, 0.5),
                    (pid::REVERB_WIDTH, 0.8),
                ],
            },
        ],
    },
    FxPreset {
        name: "Bass Tightener",
        slots: &[
            FxSlotPreset {
                effect_type: 8, // Compressor
                mix: 1.0,
                params: &[
                    (pid::COMP_MODE, 2.0),      // FET
                    (pid::COMP_THRESH, -15.0),
                    (pid::COMP_RATIO, 5.0),
                    (pid::COMP_ATTACK, 0.002),
                    (pid::COMP_RELEASE, 0.1),
                    (pid::COMP_MAKEUP, 3.0),
                    (pid::COMP_KNEE, 3.0),
                ],
            },
            FxSlotPreset {
                effect_type: 6, // Distortion (warmth)
                mix: 0.1,
                params: &[
                    (pid::DIST_TYPE, 2.0),       // Tape
                    (pid::DIST_DRIVE, 2.5),
                    (pid::DIST_TONE, 2000.0),
                    (pid::DIST_OUTPUT, 0.9),
                ],
            },
        ],
    },

    // ═══════ CREATIVE ═══════
    FxPreset {
        name: "Ambient Wash",
        slots: &[
            FxSlotPreset {
                effect_type: 3, // Chorus
                mix: 0.4,
                params: &[
                    (pid::CHORUS_RATE, 0.3),
                    (pid::CHORUS_DEPTH, 0.6),
                    (pid::CHORUS_WIDTH, 1.0),
                    (pid::CHORUS_DELAY, 0.015),
                    (pid::CHORUS_VOICES, 4.0),
                ],
            },
            FxSlotPreset {
                effect_type: 1, // Delay
                mix: 0.35,
                params: &[
                    (pid::DELAY_TYPE, 2.0),      // Tape
                    (pid::DELAY_TIME, 0.5),
                    (pid::DELAY_FEEDBACK, 0.6),
                    (pid::DELAY_STEREO, 0.5),
                    (pid::DELAY_HICUT, 5000.0),
                    (pid::DELAY_LOCUT, 200.0),
                ],
            },
            FxSlotPreset {
                effect_type: 2, // Reverb
                mix: 0.5,
                params: &[
                    (pid::REVERB_TYPE, 0.0),     // Hall
                    (pid::REVERB_SIZE, 0.9),
                    (pid::REVERB_DECAY, 0.8),
                    (pid::REVERB_DAMP, 0.4),
                    (pid::REVERB_WIDTH, 1.8),
                    (pid::REVERB_PREDELAY, 0.04),
                ],
            },
        ],
    },
    FxPreset {
        name: "Lo-Fi Tape",
        slots: &[
            FxSlotPreset {
                effect_type: 6, // Distortion
                mix: 0.3,
                params: &[
                    (pid::DIST_TYPE, 4.0),       // Bitcrush
                    (pid::DIST_DRIVE, 1.5),
                    (pid::DIST_TONE, 3000.0),
                    (pid::DIST_OUTPUT, 0.7),
                    (pid::DIST_BITS, 12.0),
                    (pid::DIST_SR_DIV, 3.0),
                ],
            },
            FxSlotPreset {
                effect_type: 3, // Chorus
                mix: 0.25,
                params: &[
                    (pid::CHORUS_RATE, 0.4),
                    (pid::CHORUS_DEPTH, 0.4),
                    (pid::CHORUS_WIDTH, 0.6),
                    (pid::CHORUS_DELAY, 0.012),
                    (pid::CHORUS_VOICES, 2.0),
                ],
            },
            FxSlotPreset {
                effect_type: 2, // Reverb
                mix: 0.2,
                params: &[
                    (pid::REVERB_TYPE, 1.0),     // Room
                    (pid::REVERB_SIZE, 0.3),
                    (pid::REVERB_DECAY, 0.25),
                    (pid::REVERB_DAMP, 0.7),
                    (pid::REVERB_WIDTH, 0.6),
                ],
            },
        ],
    },
    FxPreset {
        name: "Dub Echo",
        slots: &[
            FxSlotPreset {
                effect_type: 1, // Delay
                mix: 0.45,
                params: &[
                    (pid::DELAY_TYPE, 1.0),      // PingPong
                    (pid::DELAY_TIME, 0.375),
                    (pid::DELAY_FEEDBACK, 0.7),
                    (pid::DELAY_STEREO, 0.8),
                    (pid::DELAY_HICUT, 3000.0),
                    (pid::DELAY_LOCUT, 400.0),
                ],
            },
            FxSlotPreset {
                effect_type: 6, // Distortion
                mix: 0.15,
                params: &[
                    (pid::DIST_TYPE, 2.0),       // Tape
                    (pid::DIST_DRIVE, 3.0),
                    (pid::DIST_TONE, 2500.0),
                    (pid::DIST_OUTPUT, 0.7),
                ],
            },
            FxSlotPreset {
                effect_type: 2, // Reverb
                mix: 0.3,
                params: &[
                    (pid::REVERB_TYPE, 3.0),     // Spring
                    (pid::REVERB_SIZE, 0.5),
                    (pid::REVERB_DECAY, 0.5),
                    (pid::REVERB_DAMP, 0.5),
                    (pid::REVERB_WIDTH, 1.2),
                ],
            },
        ],
    },
    FxPreset {
        name: "Shimmer Verb",
        slots: &[
            FxSlotPreset {
                effect_type: 4, // Phaser
                mix: 0.2,
                params: &[
                    (pid::PHASER_RATE, 0.15),
                    (pid::PHASER_DEPTH, 0.4),
                    (pid::PHASER_FEEDBACK, 0.3),
                    (pid::PHASER_STEREO, 0.8),
                    (pid::PHASER_STAGES, 6.0),
                ],
            },
            FxSlotPreset {
                effect_type: 2, // Reverb
                mix: 0.6,
                params: &[
                    (pid::REVERB_TYPE, 0.0),     // Hall
                    (pid::REVERB_SIZE, 0.95),
                    (pid::REVERB_DECAY, 0.9),
                    (pid::REVERB_DAMP, 0.3),
                    (pid::REVERB_WIDTH, 2.0),
                    (pid::REVERB_PREDELAY, 0.05),
                ],
            },
            FxSlotPreset {
                effect_type: 3, // Chorus
                mix: 0.3,
                params: &[
                    (pid::CHORUS_RATE, 0.2),
                    (pid::CHORUS_DEPTH, 0.5),
                    (pid::CHORUS_WIDTH, 1.0),
                    (pid::CHORUS_DELAY, 0.02),
                    (pid::CHORUS_VOICES, 3.0),
                ],
            },
        ],
    },

    // ═══════ MODULATION ═══════
    FxPreset {
        name: "Lush Chorus",
        slots: &[
            FxSlotPreset {
                effect_type: 3, // Chorus
                mix: 0.5,
                params: &[
                    (pid::CHORUS_RATE, 0.6),
                    (pid::CHORUS_DEPTH, 0.7),
                    (pid::CHORUS_WIDTH, 1.0),
                    (pid::CHORUS_DELAY, 0.012),
                    (pid::CHORUS_VOICES, 4.0),
                ],
            },
        ],
    },
    FxPreset {
        name: "Jet Flanger",
        slots: &[
            FxSlotPreset {
                effect_type: 5, // Flanger
                mix: 0.5,
                params: &[
                    (pid::FLANGER_RATE, 0.15),
                    (pid::FLANGER_DEPTH, 0.8),
                    (pid::FLANGER_DELAY, 0.003),
                    (pid::FLANGER_FEEDBACK, 0.85),
                    (pid::FLANGER_STEREO, 0.5),
                ],
            },
        ],
    },
    FxPreset {
        name: "Phaser Sweep",
        slots: &[
            FxSlotPreset {
                effect_type: 4, // Phaser
                mix: 0.5,
                params: &[
                    (pid::PHASER_RATE, 0.3),
                    (pid::PHASER_DEPTH, 0.7),
                    (pid::PHASER_FEEDBACK, 0.7),
                    (pid::PHASER_STEREO, 0.6),
                    (pid::PHASER_STAGES, 8.0),
                ],
            },
        ],
    },
    FxPreset {
        name: "Detune Widener",
        slots: &[
            FxSlotPreset {
                effect_type: 3, // Chorus
                mix: 0.35,
                params: &[
                    (pid::CHORUS_RATE, 0.1),
                    (pid::CHORUS_DEPTH, 0.3),
                    (pid::CHORUS_WIDTH, 1.0),
                    (pid::CHORUS_DELAY, 0.025),
                    (pid::CHORUS_VOICES, 6.0),
                ],
            },
            FxSlotPreset {
                effect_type: 2, // Reverb (tiny)
                mix: 0.1,
                params: &[
                    (pid::REVERB_TYPE, 1.0),     // Room
                    (pid::REVERB_SIZE, 0.15),
                    (pid::REVERB_DECAY, 0.1),
                    (pid::REVERB_DAMP, 0.7),
                    (pid::REVERB_WIDTH, 1.5),
                ],
            },
        ],
    },

    // ═══════ SPACE ═══════
    FxPreset {
        name: "Big Hall",
        slots: &[
            FxSlotPreset {
                effect_type: 2, // Reverb
                mix: 0.45,
                params: &[
                    (pid::REVERB_TYPE, 0.0),     // Hall
                    (pid::REVERB_SIZE, 0.85),
                    (pid::REVERB_DECAY, 0.75),
                    (pid::REVERB_DAMP, 0.4),
                    (pid::REVERB_WIDTH, 1.5),
                    (pid::REVERB_PREDELAY, 0.035),
                ],
            },
        ],
    },
    FxPreset {
        name: "Small Room",
        slots: &[
            FxSlotPreset {
                effect_type: 2, // Reverb
                mix: 0.3,
                params: &[
                    (pid::REVERB_TYPE, 1.0),     // Room
                    (pid::REVERB_SIZE, 0.2),
                    (pid::REVERB_DECAY, 0.2),
                    (pid::REVERB_DAMP, 0.6),
                    (pid::REVERB_WIDTH, 0.8),
                    (pid::REVERB_PREDELAY, 0.005),
                ],
            },
        ],
    },
    FxPreset {
        name: "Plate Verb",
        slots: &[
            FxSlotPreset {
                effect_type: 2, // Reverb
                mix: 0.35,
                params: &[
                    (pid::REVERB_TYPE, 2.0),     // Plate
                    (pid::REVERB_SIZE, 0.6),
                    (pid::REVERB_DECAY, 0.5),
                    (pid::REVERB_DAMP, 0.45),
                    (pid::REVERB_WIDTH, 1.2),
                    (pid::REVERB_PREDELAY, 0.01),
                ],
            },
        ],
    },
    FxPreset {
        name: "Cathedral",
        slots: &[
            FxSlotPreset {
                effect_type: 2, // Reverb
                mix: 0.55,
                params: &[
                    (pid::REVERB_TYPE, 0.0),     // Hall
                    (pid::REVERB_SIZE, 0.98),
                    (pid::REVERB_DECAY, 0.92),
                    (pid::REVERB_DAMP, 0.25),
                    (pid::REVERB_WIDTH, 2.0),
                    (pid::REVERB_PREDELAY, 0.06),
                ],
            },
            FxSlotPreset {
                effect_type: 1, // Delay (early reflections feel)
                mix: 0.1,
                params: &[
                    (pid::DELAY_TYPE, 0.0),      // Simple
                    (pid::DELAY_TIME, 0.12),
                    (pid::DELAY_FEEDBACK, 0.2),
                    (pid::DELAY_HICUT, 4000.0),
                    (pid::DELAY_LOCUT, 300.0),
                    (pid::DELAY_STEREO, 0.6),
                ],
            },
        ],
    },

    // ═══════ DELAY ═══════
    FxPreset {
        name: "Ping Pong",
        slots: &[
            FxSlotPreset {
                effect_type: 1, // Delay
                mix: 0.4,
                params: &[
                    (pid::DELAY_TYPE, 1.0),      // PingPong
                    (pid::DELAY_TIME, 0.375),
                    (pid::DELAY_FEEDBACK, 0.5),
                    (pid::DELAY_STEREO, 1.0),
                    (pid::DELAY_HICUT, 8000.0),
                    (pid::DELAY_LOCUT, 100.0),
                ],
            },
        ],
    },
    FxPreset {
        name: "Slapback",
        slots: &[
            FxSlotPreset {
                effect_type: 1, // Delay
                mix: 0.35,
                params: &[
                    (pid::DELAY_TYPE, 4.0),      // Slapback
                    (pid::DELAY_TIME, 0.08),
                    (pid::DELAY_FEEDBACK, 0.1),
                    (pid::DELAY_HICUT, 10000.0),
                    (pid::DELAY_LOCUT, 80.0),
                ],
            },
        ],
    },
    FxPreset {
        name: "Tape Echo",
        slots: &[
            FxSlotPreset {
                effect_type: 1, // Delay
                mix: 0.4,
                params: &[
                    (pid::DELAY_TYPE, 2.0),      // Tape
                    (pid::DELAY_TIME, 0.3),
                    (pid::DELAY_FEEDBACK, 0.55),
                    (pid::DELAY_STEREO, 0.3),
                    (pid::DELAY_HICUT, 4000.0),
                    (pid::DELAY_LOCUT, 200.0),
                ],
            },
            FxSlotPreset {
                effect_type: 6, // Distortion (subtle tape warmth)
                mix: 0.1,
                params: &[
                    (pid::DIST_TYPE, 2.0),       // Tape
                    (pid::DIST_DRIVE, 1.5),
                    (pid::DIST_TONE, 3000.0),
                    (pid::DIST_OUTPUT, 0.9),
                ],
            },
        ],
    },
    FxPreset {
        name: "Dotted Eighth",
        slots: &[
            FxSlotPreset {
                effect_type: 1, // Delay
                mix: 0.35,
                params: &[
                    (pid::DELAY_TYPE, 0.0),      // Simple
                    (pid::DELAY_TIME, 0.281),    // ~dotted 8th at 120bpm
                    (pid::DELAY_FEEDBACK, 0.4),
                    (pid::DELAY_STEREO, 0.4),
                    (pid::DELAY_HICUT, 6000.0),
                    (pid::DELAY_LOCUT, 200.0),
                ],
            },
        ],
    },

    // ═══════ GUITAR ═══════
    FxPreset { name: "Classic Rock", slots: &[
        FxSlotPreset { effect_type: 6, mix: 0.8, params: &[
            (pid::DIST_TYPE, 2.0), (pid::DIST_DRIVE, 5.0), (pid::DIST_TONE, 4000.0), (pid::DIST_OUTPUT, 0.7),
        ]},
        FxSlotPreset { effect_type: 3, mix: 0.15, params: &[
            (pid::CHORUS_RATE, 0.8), (pid::CHORUS_DEPTH, 0.2),
        ]},
        FxSlotPreset { effect_type: 2, mix: 0.2, params: &[
            (pid::REVERB_TYPE, 2.0), (pid::REVERB_SIZE, 0.4), (pid::REVERB_DECAY, 0.3),
        ]},
    ]},
    FxPreset { name: "Metal Rhythm", slots: &[
        FxSlotPreset { effect_type: 6, mix: 1.0, params: &[
            (pid::DIST_TYPE, 3.0), (pid::DIST_DRIVE, 8.0), (pid::DIST_TONE, 3000.0), (pid::DIST_OUTPUT, 0.6),
        ]},
        FxSlotPreset { effect_type: 8, mix: 1.0, params: &[
            (pid::COMP_THRESH, -12.0), (pid::COMP_RATIO, 4.0), (pid::COMP_ATTACK, 0.005), (pid::COMP_RELEASE, 0.1),
        ]},
    ]},
    FxPreset { name: "Clean Shimmer", slots: &[
        FxSlotPreset { effect_type: 3, mix: 0.4, params: &[
            (pid::CHORUS_RATE, 1.2), (pid::CHORUS_DEPTH, 0.5),
        ]},
        FxSlotPreset { effect_type: 1, mix: 0.3, params: &[
            (pid::DELAY_TIME, 0.281), (pid::DELAY_FEEDBACK, 0.4), (pid::DELAY_STEREO, 0.8),
        ]},
        FxSlotPreset { effect_type: 2, mix: 0.4, params: &[
            (pid::REVERB_TYPE, 3.0), (pid::REVERB_SIZE, 0.7), (pid::REVERB_DECAY, 0.6),
        ]},
    ]},
    FxPreset { name: "Blues Lead", slots: &[
        FxSlotPreset { effect_type: 6, mix: 0.6, params: &[
            (pid::DIST_TYPE, 1.0), (pid::DIST_DRIVE, 3.5), (pid::DIST_TONE, 5000.0), (pid::DIST_OUTPUT, 0.7),
        ]},
        FxSlotPreset { effect_type: 1, mix: 0.25, params: &[
            (pid::DELAY_TIME, 0.375), (pid::DELAY_FEEDBACK, 0.3),
        ]},
        FxSlotPreset { effect_type: 2, mix: 0.2, params: &[
            (pid::REVERB_TYPE, 0.0), (pid::REVERB_SIZE, 0.3), (pid::REVERB_DECAY, 0.25),
        ]},
    ]},
    FxPreset { name: "Shoegaze Wall", slots: &[
        FxSlotPreset { effect_type: 6, mix: 1.0, params: &[
            (pid::DIST_TYPE, 3.0), (pid::DIST_DRIVE, 9.0), (pid::DIST_TONE, 3500.0), (pid::DIST_OUTPUT, 0.5),
        ]},
        FxSlotPreset { effect_type: 3, mix: 0.5, params: &[
            (pid::CHORUS_RATE, 0.6), (pid::CHORUS_DEPTH, 0.7),
        ]},
        FxSlotPreset { effect_type: 2, mix: 0.8, params: &[
            (pid::REVERB_TYPE, 4.0), (pid::REVERB_SIZE, 0.9), (pid::REVERB_DECAY, 0.8),
        ]},
        FxSlotPreset { effect_type: 1, mix: 0.5, params: &[
            (pid::DELAY_TIME, 0.375), (pid::DELAY_FEEDBACK, 0.5), (pid::DELAY_STEREO, 0.9),
        ]},
    ]},

    // ═══════ BASS ═══════
    FxPreset { name: "Sub Enhancer", slots: &[
        FxSlotPreset { effect_type: 8, mix: 1.0, params: &[
            (pid::COMP_THRESH, -15.0), (pid::COMP_RATIO, 4.0), (pid::COMP_ATTACK, 0.003), (pid::COMP_RELEASE, 0.15), (pid::COMP_MAKEUP, 4.0),
        ]},
        FxSlotPreset { effect_type: 6, mix: 0.15, params: &[
            (pid::DIST_TYPE, 1.0), (pid::DIST_DRIVE, 2.0), (pid::DIST_TONE, 5000.0), (pid::DIST_OUTPUT, 0.8),
        ]},
    ]},
    FxPreset { name: "Acid FX", slots: &[
        FxSlotPreset { effect_type: 6, mix: 0.5, params: &[
            (pid::DIST_TYPE, 2.0), (pid::DIST_DRIVE, 4.0), (pid::DIST_TONE, 4000.0), (pid::DIST_OUTPUT, 0.7),
        ]},
        FxSlotPreset { effect_type: 1, mix: 0.2, params: &[
            (pid::DELAY_TIME, 0.125), (pid::DELAY_FEEDBACK, 0.3), (pid::DELAY_HICUT, 5000.0),
        ]},
    ]},
    FxPreset { name: "Dub Bass", slots: &[
        FxSlotPreset { effect_type: 1, mix: 0.3, params: &[
            (pid::DELAY_TYPE, 1.0), (pid::DELAY_TIME, 0.281), (pid::DELAY_FEEDBACK, 0.45), (pid::DELAY_HICUT, 3000.0),
        ]},
        FxSlotPreset { effect_type: 2, mix: 0.25, params: &[
            (pid::REVERB_TYPE, 1.0), (pid::REVERB_SIZE, 0.3), (pid::REVERB_DECAY, 0.3),
        ]},
    ]},
    FxPreset { name: "Bass Crunch", slots: &[
        FxSlotPreset { effect_type: 6, mix: 0.8, params: &[
            (pid::DIST_TYPE, 3.0), (pid::DIST_DRIVE, 5.0), (pid::DIST_TONE, 3000.0), (pid::DIST_OUTPUT, 0.6),
        ]},
        FxSlotPreset { effect_type: 8, mix: 1.0, params: &[
            (pid::COMP_THRESH, -10.0), (pid::COMP_RATIO, 6.0), (pid::COMP_ATTACK, 0.002), (pid::COMP_RELEASE, 0.1), (pid::COMP_MAKEUP, 3.0),
        ]},
    ]},

    // ═══════ JMJ / AMBIENT ═══════
    FxPreset { name: "Oxygene Space", slots: &[
        FxSlotPreset { effect_type: 4, mix: 0.5, params: &[ // Phaser
            (pid::PHASER_RATE, 0.15), (pid::PHASER_DEPTH, 0.8), (pid::PHASER_FEEDBACK, 0.6),
        ]},
        FxSlotPreset { effect_type: 1, mix: 0.45, params: &[ // Delay (dotted 1/4)
            (pid::DELAY_TYPE, 1.0), (pid::DELAY_TIME, 0.562), (pid::DELAY_FEEDBACK, 0.5),
            (pid::DELAY_STEREO, 0.8), (pid::DELAY_HICUT, 5000.0),
        ]},
        FxSlotPreset { effect_type: 2, mix: 0.55, params: &[ // Reverb (hall)
            (pid::REVERB_TYPE, 3.0), (pid::REVERB_SIZE, 0.7), (pid::REVERB_DECAY, 0.6),
            (pid::REVERB_DAMP, 0.4), (pid::REVERB_WIDTH, 1.5),
        ]},
    ]},
    FxPreset { name: "Equinoxe Wash", slots: &[
        FxSlotPreset { effect_type: 3, mix: 0.4, params: &[ // Chorus (lush)
            (pid::CHORUS_RATE, 0.5), (pid::CHORUS_DEPTH, 0.6),
        ]},
        FxSlotPreset { effect_type: 2, mix: 0.75, params: &[ // Reverb (cathedral)
            (pid::REVERB_TYPE, 4.0), (pid::REVERB_SIZE, 0.9), (pid::REVERB_DECAY, 0.8),
            (pid::REVERB_DAMP, 0.3), (pid::REVERB_WIDTH, 2.0),
        ]},
        FxSlotPreset { effect_type: 16, mix: 1.0, params: &[ // StereoImager (wide)
            (pid::SIMG_WIDTH, 1.8), (pid::SIMG_MONO_FREQ, 150.0),
        ]},
    ]},
    FxPreset { name: "Laser FX", slots: &[
        FxSlotPreset { effect_type: 4, mix: 0.6, params: &[ // Phaser (fast)
            (pid::PHASER_RATE, 3.0), (pid::PHASER_DEPTH, 0.7), (pid::PHASER_FEEDBACK, 0.5),
        ]},
        FxSlotPreset { effect_type: 1, mix: 0.35, params: &[ // Delay (1/8)
            (pid::DELAY_TYPE, 0.0), (pid::DELAY_TIME, 0.188), (pid::DELAY_FEEDBACK, 0.4),
            (pid::DELAY_STEREO, 0.5),
        ]},
    ]},
    FxPreset { name: "Analog Warmth", slots: &[
        FxSlotPreset { effect_type: 6, mix: 0.15, params: &[ // Distortion (tape, subtle)
            (pid::DIST_TYPE, 2.0), (pid::DIST_DRIVE, 1.5), (pid::DIST_TONE, 4000.0), (pid::DIST_OUTPUT, 0.85),
        ]},
        FxSlotPreset { effect_type: 3, mix: 0.2, params: &[ // Chorus (slow)
            (pid::CHORUS_RATE, 0.3), (pid::CHORUS_DEPTH, 0.25),
        ]},
        FxSlotPreset { effect_type: 15, mix: 0.3, params: &[ // LoFi (mild)
            (pid::LOFI_BITS, 14.0), (pid::LOFI_ROLLOFF, 12000.0),
        ]},
    ]},

    // Vintage FX — showcase the faithful engine-extracted effects (each reuses a
    // bespoke instrument's exact DSP) on any track. Indices 46..50 = Ensemble,
    // SpringTank, CinemaPlate, SolinaVerb, Dimension (appended last in all()).
    FxPreset { name: "String Ensemble", slots: &[
        FxSlotPreset { effect_type: 46, mix: 1.0,  params: &[(pid::ENSEMBLE_DEPTH, 0.7)] },
        FxSlotPreset { effect_type: 49, mix: 0.28, params: &[(pid::SOLINAVERB_SIZE, 0.55), (pid::SOLINAVERB_DAMP, 0.4)] },
    ]},
    FxPreset { name: "Synthi Spring", slots: &[
        FxSlotPreset { effect_type: 47, mix: 0.35, params: &[(pid::SPRINGTANK_DECAY, 0.6)] },
    ]},
    FxPreset { name: "Cinema Space", slots: &[
        FxSlotPreset { effect_type: 48, mix: 0.35, params: &[(pid::CINEMAPLATE_SIZE, 0.7), (pid::CINEMAPLATE_DAMP, 0.3)] },
    ]},
    FxPreset { name: "Dimension Wide", slots: &[
        FxSlotPreset { effect_type: 50, mix: 0.5,  params: &[(pid::DIMENSION_DELAY, 9.0), (pid::DIMENSION_RATE, 0.4)] },
        FxSlotPreset { effect_type: 48, mix: 0.22, params: &[(pid::CINEMAPLATE_SIZE, 0.6), (pid::CINEMAPLATE_DAMP, 0.35)] },
    ]},
    FxPreset { name: "Full Solina", slots: &[
        FxSlotPreset { effect_type: 46, mix: 1.0,  params: &[(pid::ENSEMBLE_DEPTH, 0.8)] },
        FxSlotPreset { effect_type: 50, mix: 0.3,  params: &[(pid::DIMENSION_DELAY, 7.0), (pid::DIMENSION_RATE, 0.55)] },
        FxSlotPreset { effect_type: 49, mix: 0.3,  params: &[(pid::SOLINAVERB_SIZE, 0.6), (pid::SOLINAVERB_DAMP, 0.45)] },
    ]},
];

// ═══════════════════════════════════════════════════════════════════════
// Per-drum-element FX chain presets (rack presets for individual sounds)
// ═══════════════════════════════════════════════════════════════════════

/// Categories of per-sound FX presets, organized by drum element
pub fn get_drum_sound_preset_categories() -> &'static [FxPresetCategory] {
    &[
        FxPresetCategory { name: "Kick", presets: &[
            "Kick: Punchy", "Kick: Sub Boom", "Kick: Lo-Fi",
        ]},
        FxPresetCategory { name: "Snare", presets: &[
            "Snare: Snap & Verb", "Snare: Fat Crunch", "Snare: Gated Room",
        ]},
        FxPresetCategory { name: "Hi-Hat", presets: &[
            "HH: Stereo Width", "HH: Shimmer", "HH: Lo-Fi Tape",
        ]},
        FxPresetCategory { name: "Clap / Perc", presets: &[
            "Clap: Plate Verb", "Perc: Ambient", "Perc: Dub Echo",
        ]},
        FxPresetCategory { name: "Toms", presets: &[
            "Tom: Tight Gate", "Tom: Big Room", "Tom: Flanged",
        ]},
    ]
}

pub fn find_drum_sound_preset(name: &str) -> Option<&'static FxPreset> {
    DRUM_SOUND_PRESETS.iter().find(|p| p.name == name)
}

static DRUM_SOUND_PRESETS: [FxPreset; 15] = [
    // ═══ KICK ═══
    FxPreset { name: "Kick: Punchy", slots: &[
        FxSlotPreset { effect_type: 8, mix: 1.0, params: &[
            (pid::COMP_MODE, 2.0), (pid::COMP_THRESH, -12.0), (pid::COMP_RATIO, 6.0),
            (pid::COMP_ATTACK, 0.005), (pid::COMP_RELEASE, 0.08), (pid::COMP_MAKEUP, 4.0), (pid::COMP_KNEE, 3.0),
        ]},
        FxSlotPreset { effect_type: 9, mix: 1.0, params: &[
            (pid::LIMITER_MODE, 0.0), (pid::LIMITER_CEIL, -0.5), (pid::LIMITER_RELEASE, 0.03),
        ]},
    ]},
    FxPreset { name: "Kick: Sub Boom", slots: &[
        FxSlotPreset { effect_type: 6, mix: 0.2, params: &[
            (pid::DIST_TYPE, 2.0), (pid::DIST_DRIVE, 2.0), (pid::DIST_TONE, 800.0), (pid::DIST_OUTPUT, 0.9),
        ]},
        FxSlotPreset { effect_type: 8, mix: 1.0, params: &[
            (pid::COMP_MODE, 1.0), (pid::COMP_THRESH, -15.0), (pid::COMP_RATIO, 4.0),
            (pid::COMP_ATTACK, 0.008), (pid::COMP_RELEASE, 0.12), (pid::COMP_MAKEUP, 3.0), (pid::COMP_KNEE, 6.0),
        ]},
    ]},
    FxPreset { name: "Kick: Lo-Fi", slots: &[
        FxSlotPreset { effect_type: 6, mix: 0.5, params: &[
            (pid::DIST_TYPE, 4.0), (pid::DIST_DRIVE, 1.5), (pid::DIST_TONE, 2000.0),
            (pid::DIST_OUTPUT, 0.7), (pid::DIST_BITS, 10.0), (pid::DIST_SR_DIV, 2.0),
        ]},
        FxSlotPreset { effect_type: 8, mix: 1.0, params: &[
            (pid::COMP_MODE, 2.0), (pid::COMP_THRESH, -10.0), (pid::COMP_RATIO, 8.0),
            (pid::COMP_ATTACK, 0.001), (pid::COMP_RELEASE, 0.05), (pid::COMP_MAKEUP, 6.0), (pid::COMP_KNEE, 2.0),
        ]},
    ]},

    // ═══ SNARE ═══
    FxPreset { name: "Snare: Snap & Verb", slots: &[
        FxSlotPreset { effect_type: 8, mix: 1.0, params: &[
            (pid::COMP_MODE, 2.0), (pid::COMP_THRESH, -15.0), (pid::COMP_RATIO, 5.0),
            (pid::COMP_ATTACK, 0.001), (pid::COMP_RELEASE, 0.06), (pid::COMP_MAKEUP, 5.0), (pid::COMP_KNEE, 2.0),
        ]},
        FxSlotPreset { effect_type: 2, mix: 0.25, params: &[
            (pid::REVERB_TYPE, 1.0), (pid::REVERB_SIZE, 0.2), (pid::REVERB_DECAY, 0.15),
            (pid::REVERB_DAMP, 0.65), (pid::REVERB_WIDTH, 0.8), (pid::REVERB_PREDELAY, 0.003),
        ]},
    ]},
    FxPreset { name: "Snare: Fat Crunch", slots: &[
        FxSlotPreset { effect_type: 6, mix: 0.35, params: &[
            (pid::DIST_TYPE, 1.0), (pid::DIST_DRIVE, 5.0), (pid::DIST_TONE, 4000.0), (pid::DIST_OUTPUT, 0.6),
        ]},
        FxSlotPreset { effect_type: 8, mix: 1.0, params: &[
            (pid::COMP_MODE, 2.0), (pid::COMP_THRESH, -10.0), (pid::COMP_RATIO, 8.0),
            (pid::COMP_ATTACK, 0.001), (pid::COMP_RELEASE, 0.04), (pid::COMP_MAKEUP, 8.0), (pid::COMP_KNEE, 1.0),
        ]},
    ]},
    FxPreset { name: "Snare: Gated Room", slots: &[
        FxSlotPreset { effect_type: 2, mix: 0.4, params: &[
            (pid::REVERB_TYPE, 1.0), (pid::REVERB_SIZE, 0.3), (pid::REVERB_DECAY, 0.15),
            (pid::REVERB_DAMP, 0.5), (pid::REVERB_WIDTH, 1.0), (pid::REVERB_PREDELAY, 0.005),
        ]},
        FxSlotPreset { effect_type: 10, mix: 1.0, params: &[
            (pid::EXP_MODE, 0.0), (pid::EXP_THRESH, -30.0), (pid::EXP_RATIO, 6.0),
            (pid::EXP_ATTACK, 0.001), (pid::EXP_RELEASE, 0.04), (pid::EXP_RANGE, -40.0), (pid::EXP_KNEE, 3.0),
        ]},
    ]},

    // ═══ HI-HAT ═══
    FxPreset { name: "HH: Stereo Width", slots: &[
        FxSlotPreset { effect_type: 3, mix: 0.3, params: &[
            (pid::CHORUS_RATE, 0.8), (pid::CHORUS_DEPTH, 0.3), (pid::CHORUS_WIDTH, 1.0),
            (pid::CHORUS_DELAY, 0.005), (pid::CHORUS_VOICES, 2.0),
        ]},
    ]},
    FxPreset { name: "HH: Shimmer", slots: &[
        FxSlotPreset { effect_type: 4, mix: 0.4, params: &[
            (pid::PHASER_RATE, 1.5), (pid::PHASER_DEPTH, 0.5), (pid::PHASER_FEEDBACK, 0.4),
            (pid::PHASER_STEREO, 0.6), (pid::PHASER_STAGES, 4.0),
        ]},
        FxSlotPreset { effect_type: 1, mix: 0.15, params: &[
            (pid::DELAY_TYPE, 1.0), (pid::DELAY_TIME, 0.125), (pid::DELAY_FEEDBACK, 0.3),
            (pid::DELAY_STEREO, 1.0), (pid::DELAY_HICUT, 10000.0), (pid::DELAY_LOCUT, 500.0),
        ]},
    ]},
    FxPreset { name: "HH: Lo-Fi Tape", slots: &[
        FxSlotPreset { effect_type: 6, mix: 0.3, params: &[
            (pid::DIST_TYPE, 2.0), (pid::DIST_DRIVE, 1.5), (pid::DIST_TONE, 6000.0),
            (pid::DIST_OUTPUT, 0.8), (pid::DIST_BITS, 12.0),
        ]},
    ]},

    // ═══ CLAP / PERC ═══
    FxPreset { name: "Clap: Plate Verb", slots: &[
        FxSlotPreset { effect_type: 2, mix: 0.35, params: &[
            (pid::REVERB_TYPE, 2.0), (pid::REVERB_SIZE, 0.5), (pid::REVERB_DECAY, 0.4),
            (pid::REVERB_DAMP, 0.5), (pid::REVERB_WIDTH, 1.2), (pid::REVERB_PREDELAY, 0.01),
        ]},
    ]},
    FxPreset { name: "Perc: Ambient", slots: &[
        FxSlotPreset { effect_type: 2, mix: 0.4, params: &[
            (pid::REVERB_TYPE, 0.0), (pid::REVERB_SIZE, 0.85), (pid::REVERB_DECAY, 0.7),
            (pid::REVERB_DAMP, 0.35), (pid::REVERB_WIDTH, 1.8), (pid::REVERB_PREDELAY, 0.03),
        ]},
        FxSlotPreset { effect_type: 1, mix: 0.2, params: &[
            (pid::DELAY_TYPE, 1.0), (pid::DELAY_TIME, 0.25), (pid::DELAY_FEEDBACK, 0.3),
            (pid::DELAY_STEREO, 0.7), (pid::DELAY_HICUT, 5000.0), (pid::DELAY_LOCUT, 300.0),
        ]},
    ]},
    FxPreset { name: "Perc: Dub Echo", slots: &[
        FxSlotPreset { effect_type: 1, mix: 0.35, params: &[
            (pid::DELAY_TYPE, 1.0), (pid::DELAY_TIME, 0.375), (pid::DELAY_FEEDBACK, 0.5),
            (pid::DELAY_STEREO, 0.8), (pid::DELAY_HICUT, 3000.0), (pid::DELAY_LOCUT, 400.0),
        ]},
        FxSlotPreset { effect_type: 2, mix: 0.25, params: &[
            (pid::REVERB_TYPE, 3.0), (pid::REVERB_SIZE, 0.5), (pid::REVERB_DECAY, 0.4),
            (pid::REVERB_DAMP, 0.5), (pid::REVERB_WIDTH, 1.2), (pid::REVERB_PREDELAY, 0.01),
        ]},
    ]},

    // ═══ TOMS ═══
    FxPreset { name: "Tom: Tight Gate", slots: &[
        FxSlotPreset { effect_type: 10, mix: 1.0, params: &[
            (pid::EXP_MODE, 0.0), (pid::EXP_THRESH, -28.0), (pid::EXP_RATIO, 5.0),
            (pid::EXP_ATTACK, 0.001), (pid::EXP_RELEASE, 0.05), (pid::EXP_RANGE, -35.0), (pid::EXP_KNEE, 3.0),
        ]},
        FxSlotPreset { effect_type: 8, mix: 1.0, params: &[
            (pid::COMP_MODE, 0.0), (pid::COMP_THRESH, -18.0), (pid::COMP_RATIO, 3.0),
            (pid::COMP_ATTACK, 0.003), (pid::COMP_RELEASE, 0.12), (pid::COMP_MAKEUP, 3.0), (pid::COMP_KNEE, 6.0),
        ]},
    ]},
    FxPreset { name: "Tom: Big Room", slots: &[
        FxSlotPreset { effect_type: 2, mix: 0.3, params: &[
            (pid::REVERB_TYPE, 1.0), (pid::REVERB_SIZE, 0.4), (pid::REVERB_DECAY, 0.25),
            (pid::REVERB_DAMP, 0.55), (pid::REVERB_WIDTH, 1.0), (pid::REVERB_PREDELAY, 0.005),
        ]},
        FxSlotPreset { effect_type: 8, mix: 1.0, params: &[
            (pid::COMP_MODE, 0.0), (pid::COMP_THRESH, -16.0), (pid::COMP_RATIO, 3.5),
            (pid::COMP_ATTACK, 0.005), (pid::COMP_RELEASE, 0.15), (pid::COMP_MAKEUP, 3.0), (pid::COMP_KNEE, 5.0),
        ]},
    ]},
    FxPreset { name: "Tom: Flanged", slots: &[
        FxSlotPreset { effect_type: 5, mix: 0.4, params: &[
            (pid::FLANGER_RATE, 0.2), (pid::FLANGER_DEPTH, 0.6), (pid::FLANGER_DELAY, 0.004),
            (pid::FLANGER_FEEDBACK, 0.5), (pid::FLANGER_STEREO, 0.6),
        ]},
        FxSlotPreset { effect_type: 2, mix: 0.2, params: &[
            (pid::REVERB_TYPE, 1.0), (pid::REVERB_SIZE, 0.25), (pid::REVERB_DECAY, 0.15),
            (pid::REVERB_DAMP, 0.6), (pid::REVERB_WIDTH, 0.8), (pid::REVERB_PREDELAY, 0.003),
        ]},
    ]},
];

//==============================================================================
// STUTTER PRESETS
//==============================================================================

static STUTTER_PRESETS: [EffectPreset; 8] = [
    EffectPreset { name: "Classic Stutter", params: &[
        (pid::STUT_MODE, 0.0), (pid::STUT_RATE, 4.0), (pid::STUT_GATE, 0.5),
        (pid::STUT_SPEED, 0.5), (pid::STUT_MIX, 1.0),
    ]},
    EffectPreset { name: "Fast Chop", params: &[
        (pid::STUT_MODE, 0.0), (pid::STUT_RATE, 16.0), (pid::STUT_GATE, 0.3),
        (pid::STUT_SPEED, 0.5), (pid::STUT_MIX, 1.0),
    ]},
    EffectPreset { name: "Reverse Hit", params: &[
        (pid::STUT_MODE, 1.0), (pid::STUT_RATE, 2.0), (pid::STUT_GATE, 0.8),
        (pid::STUT_SPEED, 0.5), (pid::STUT_MIX, 1.0),
    ]},
    EffectPreset { name: "Half Speed", params: &[
        (pid::STUT_MODE, 2.0), (pid::STUT_RATE, 4.0), (pid::STUT_GATE, 0.7),
        (pid::STUT_SPEED, 0.5), (pid::STUT_MIX, 1.0),
    ]},
    EffectPreset { name: "Vinyl Scratch", params: &[
        (pid::STUT_MODE, 3.0), (pid::STUT_RATE, 3.0), (pid::STUT_GATE, 0.6),
        (pid::STUT_SPEED, 0.8), (pid::STUT_MIX, 1.0),
    ]},
    EffectPreset { name: "Glitch Out", params: &[
        (pid::STUT_MODE, 4.0), (pid::STUT_RATE, 8.0), (pid::STUT_GATE, 0.4),
        (pid::STUT_SPEED, 0.5), (pid::STUT_MIX, 1.0),
    ]},
    EffectPreset { name: "Tape Stop", params: &[
        (pid::STUT_MODE, 2.0), (pid::STUT_RATE, 1.0), (pid::STUT_GATE, 0.9),
        (pid::STUT_SPEED, 0.3), (pid::STUT_MIX, 1.0),
    ]},
    EffectPreset { name: "Subtle Stutter", params: &[
        (pid::STUT_MODE, 0.0), (pid::STUT_RATE, 2.0), (pid::STUT_GATE, 0.7),
        (pid::STUT_SPEED, 0.5), (pid::STUT_MIX, 0.6),
    ]},
];

//==============================================================================
// FILTER PRESETS
//==============================================================================

static FILTER_PRESETS: [EffectPreset; 8] = [
    EffectPreset { name: "Acid Squelch", params: &[
        (pid::FILT_MODE, 0.0), (pid::FILT_CUTOFF, 800.0), (pid::FILT_RESO, 0.8),
        (pid::FILT_LFO_RATE, 2.0), (pid::FILT_LFO_DEPTH, 0.6), (pid::FILT_MIX, 1.0),
    ]},
    EffectPreset { name: "Auto Wah", params: &[
        (pid::FILT_MODE, 1.0), (pid::FILT_CUTOFF, 500.0), (pid::FILT_RESO, 0.6),
        (pid::FILT_ENV, 0.8), (pid::FILT_MIX, 1.0),
    ]},
    EffectPreset { name: "Slow Sweep", params: &[
        (pid::FILT_MODE, 0.0), (pid::FILT_CUTOFF, 2000.0), (pid::FILT_RESO, 0.3),
        (pid::FILT_LFO_RATE, 0.1), (pid::FILT_LFO_DEPTH, 0.7), (pid::FILT_MIX, 1.0),
    ]},
    EffectPreset { name: "Bandpass Vowel", params: &[
        (pid::FILT_MODE, 2.0), (pid::FILT_CUTOFF, 1200.0), (pid::FILT_RESO, 0.7),
        (pid::FILT_LFO_RATE, 1.5), (pid::FILT_LFO_DEPTH, 0.5), (pid::FILT_MIX, 1.0),
    ]},
    EffectPreset { name: "Notch Phaser", params: &[
        (pid::FILT_MODE, 3.0), (pid::FILT_CUTOFF, 3000.0), (pid::FILT_RESO, 0.5),
        (pid::FILT_LFO_RATE, 0.5), (pid::FILT_LFO_DEPTH, 0.4), (pid::FILT_MIX, 1.0),
    ]},
    EffectPreset { name: "Dark Muffled", params: &[
        (pid::FILT_MODE, 0.0), (pid::FILT_CUTOFF, 400.0), (pid::FILT_RESO, 0.1),
        (pid::FILT_MIX, 1.0),
    ]},
    EffectPreset { name: "Telephone", params: &[
        (pid::FILT_MODE, 2.0), (pid::FILT_CUTOFF, 2500.0), (pid::FILT_RESO, 0.4),
        (pid::FILT_MIX, 1.0),
    ]},
    EffectPreset { name: "Resonant Bounce", params: &[
        (pid::FILT_MODE, 0.0), (pid::FILT_CUTOFF, 1500.0), (pid::FILT_RESO, 0.9),
        (pid::FILT_LFO_RATE, 4.0), (pid::FILT_LFO_DEPTH, 0.8), (pid::FILT_MIX, 1.0),
    ]},
];

//==============================================================================
// RING MOD PRESETS
//==============================================================================

static RINGMOD_PRESETS: [EffectPreset; 6] = [
    EffectPreset { name: "Robot Voice", params: &[
        (pid::RMOD_FREQ, 200.0), (pid::RMOD_WAVE, 0.0), (pid::RMOD_DEPTH, 1.0),
        (pid::RMOD_MIX, 0.7),
    ]},
    EffectPreset { name: "Bell Tone", params: &[
        (pid::RMOD_FREQ, 880.0), (pid::RMOD_WAVE, 0.0), (pid::RMOD_DEPTH, 0.5),
        (pid::RMOD_MIX, 0.5),
    ]},
    EffectPreset { name: "Harsh Metallic", params: &[
        (pid::RMOD_FREQ, 1500.0), (pid::RMOD_WAVE, 1.0), (pid::RMOD_DEPTH, 1.0),
        (pid::RMOD_MIX, 1.0),
    ]},
    EffectPreset { name: "Subtle Shimmer", params: &[
        (pid::RMOD_FREQ, 3000.0), (pid::RMOD_WAVE, 0.0), (pid::RMOD_DEPTH, 0.3),
        (pid::RMOD_MIX, 0.3),
    ]},
    EffectPreset { name: "Saw Grind", params: &[
        (pid::RMOD_FREQ, 60.0), (pid::RMOD_WAVE, 2.0), (pid::RMOD_DEPTH, 0.8),
        (pid::RMOD_MIX, 0.8),
    ]},
    EffectPreset { name: "Tremolo", params: &[
        (pid::RMOD_FREQ, 5.0), (pid::RMOD_WAVE, 0.0), (pid::RMOD_DEPTH, 0.6),
        (pid::RMOD_MIX, 1.0),
    ]},
];

//==============================================================================
// LO-FI PRESETS
//==============================================================================

static LOFI_PRESETS: [EffectPreset; 8] = [
    EffectPreset { name: "VHS Tape", params: &[
        (pid::LOFI_SR_DIV, 2.0), (pid::LOFI_BITS, 12.0), (pid::LOFI_NOISE, 0.05),
        (pid::LOFI_WOW, 0.3), (pid::LOFI_FLUTTER, 0.2), (pid::LOFI_ROLLOFF, 6000.0), (pid::LOFI_MIX, 1.0),
    ]},
    EffectPreset { name: "8-Bit", params: &[
        (pid::LOFI_SR_DIV, 8.0), (pid::LOFI_BITS, 8.0), (pid::LOFI_NOISE, 0.0),
        (pid::LOFI_ROLLOFF, 4000.0), (pid::LOFI_MIX, 1.0),
    ]},
    EffectPreset { name: "Cassette", params: &[
        (pid::LOFI_SR_DIV, 1.0), (pid::LOFI_BITS, 14.0), (pid::LOFI_NOISE, 0.08),
        (pid::LOFI_WOW, 0.15), (pid::LOFI_FLUTTER, 0.1), (pid::LOFI_ROLLOFF, 8000.0), (pid::LOFI_MIX, 1.0),
    ]},
    EffectPreset { name: "Telephone", params: &[
        (pid::LOFI_SR_DIV, 6.0), (pid::LOFI_BITS, 10.0), (pid::LOFI_NOISE, 0.02),
        (pid::LOFI_ROLLOFF, 3000.0), (pid::LOFI_MIX, 1.0),
    ]},
    EffectPreset { name: "Vinyl Crackle", params: &[
        (pid::LOFI_SR_DIV, 1.0), (pid::LOFI_BITS, 16.0), (pid::LOFI_NOISE, 0.15),
        (pid::LOFI_WOW, 0.4), (pid::LOFI_ROLLOFF, 10000.0), (pid::LOFI_MIX, 1.0),
    ]},
    EffectPreset { name: "Gameboy", params: &[
        (pid::LOFI_SR_DIV, 16.0), (pid::LOFI_BITS, 4.0), (pid::LOFI_NOISE, 0.0),
        (pid::LOFI_ROLLOFF, 2000.0), (pid::LOFI_MIX, 1.0),
    ]},
    EffectPreset { name: "Warbly Tape", params: &[
        (pid::LOFI_SR_DIV, 1.0), (pid::LOFI_BITS, 16.0), (pid::LOFI_NOISE, 0.03),
        (pid::LOFI_WOW, 0.7), (pid::LOFI_FLUTTER, 0.5), (pid::LOFI_ROLLOFF, 6000.0), (pid::LOFI_MIX, 1.0),
    ]},
    EffectPreset { name: "Radio Static", params: &[
        (pid::LOFI_SR_DIV, 4.0), (pid::LOFI_BITS, 10.0), (pid::LOFI_NOISE, 0.12),
        (pid::LOFI_ROLLOFF, 3500.0), (pid::LOFI_MIX, 0.8),
    ]},
];

//==============================================================================
// STEREO IMAGER PRESETS
//==============================================================================

static STEREO_IMAGER_PRESETS: [EffectPreset; 5] = [
    EffectPreset { name: "Wide Stereo", params: &[
        (pid::SIMG_WIDTH, 1.5), (pid::SIMG_MONO_FREQ, 100.0), (pid::SIMG_MIX, 1.0),
    ]},
    EffectPreset { name: "Extra Wide", params: &[
        (pid::SIMG_WIDTH, 2.0), (pid::SIMG_MONO_FREQ, 150.0), (pid::SIMG_MIX, 1.0),
    ]},
    EffectPreset { name: "Mono Bass", params: &[
        (pid::SIMG_WIDTH, 1.0), (pid::SIMG_MONO_FREQ, 250.0), (pid::SIMG_MIX, 1.0),
    ]},
    EffectPreset { name: "Narrow Focus", params: &[
        (pid::SIMG_WIDTH, 0.5), (pid::SIMG_MONO_FREQ, 80.0), (pid::SIMG_MIX, 1.0),
    ]},
    EffectPreset { name: "Mono Collapse", params: &[
        (pid::SIMG_WIDTH, 0.0), (pid::SIMG_MONO_FREQ, 100.0), (pid::SIMG_MIX, 1.0),
    ]},
];

// ═══════════════════════════════════════════════════════════════════════
// Choir & Maximizer — were missing presets
// ═══════════════════════════════════════════════════════════════════════

static CHOIR_PRESETS: [EffectPreset; 6] = [
    EffectPreset { name: "Small Ensemble", params: &[
        (pid::CHOIR_VOICES, 4.0), (pid::CHOIR_DETUNE, 0.3), (pid::CHOIR_WIDTH, 0.6),
        (pid::CHOIR_WANDER, 0.3), (pid::CHOIR_VIBRATO, 0.25), (pid::CHOIR_TIMBRE, 0.5),
        (pid::CHOIR_WARMTH, 0.5), (pid::CHOIR_MIX, 0.4),
    ]},
    EffectPreset { name: "Cathedral Choir", params: &[
        (pid::CHOIR_VOICES, 12.0), (pid::CHOIR_DETUNE, 0.6), (pid::CHOIR_WIDTH, 1.0),
        (pid::CHOIR_WANDER, 0.35), (pid::CHOIR_VIBRATO, 0.4), (pid::CHOIR_TIMBRE, 0.6),
        (pid::CHOIR_WARMTH, 0.7), (pid::CHOIR_MIX, 0.6),
    ]},
    EffectPreset { name: "Angelic Shimmer", params: &[
        (pid::CHOIR_VOICES, 8.0), (pid::CHOIR_DETUNE, 0.5), (pid::CHOIR_WIDTH, 0.9),
        (pid::CHOIR_WANDER, 0.6), (pid::CHOIR_VIBRATO, 0.5), (pid::CHOIR_TIMBRE, 0.45),
        (pid::CHOIR_WARMTH, 0.3), (pid::CHOIR_MIX, 0.5),
    ]},
    EffectPreset { name: "Lush Pad Double", params: &[
        (pid::CHOIR_VOICES, 6.0), (pid::CHOIR_DETUNE, 0.4), (pid::CHOIR_WIDTH, 1.0),
        (pid::CHOIR_WANDER, 0.3), (pid::CHOIR_VIBRATO, 0.2), (pid::CHOIR_TIMBRE, 0.4),
        (pid::CHOIR_WARMTH, 0.6), (pid::CHOIR_MIX, 0.45),
    ]},
    EffectPreset { name: "Warm Unison", params: &[
        (pid::CHOIR_VOICES, 10.0), (pid::CHOIR_DETUNE, 0.25), (pid::CHOIR_WIDTH, 0.5),
        (pid::CHOIR_WANDER, 0.2), (pid::CHOIR_VIBRATO, 0.15), (pid::CHOIR_TIMBRE, 0.3),
        (pid::CHOIR_WARMTH, 0.85), (pid::CHOIR_MIX, 0.4),
    ]},
    EffectPreset { name: "Ghost Choir", params: &[
        (pid::CHOIR_VOICES, 12.0), (pid::CHOIR_DETUNE, 0.8), (pid::CHOIR_WIDTH, 1.0),
        (pid::CHOIR_WANDER, 0.7), (pid::CHOIR_VIBRATO, 0.6), (pid::CHOIR_TIMBRE, 0.55),
        (pid::CHOIR_WARMTH, 0.2), (pid::CHOIR_MIX, 0.7),
    ]},
];

static MAXIMIZER_PRESETS: [EffectPreset; 6] = [
    EffectPreset { name: "Transparent Master", params: &[
        (pid::MAXI_INPUT, 3.0), (pid::MAXI_CEIL, -0.3), (pid::MAXI_RELEASE, 0.15),
        (pid::MAXI_KNEE, 8.0), (pid::MAXI_CHARACTER, 0.2), (pid::MAXI_MIX, 1.0),
    ]},
    EffectPreset { name: "Loud & Proud", params: &[
        (pid::MAXI_INPUT, 12.0), (pid::MAXI_CEIL, -0.3), (pid::MAXI_RELEASE, 0.08),
        (pid::MAXI_KNEE, 4.0), (pid::MAXI_CHARACTER, 0.7), (pid::MAXI_MIX, 1.0),
    ]},
    EffectPreset { name: "Streaming Safe", params: &[
        (pid::MAXI_INPUT, 6.0), (pid::MAXI_CEIL, -1.0), (pid::MAXI_RELEASE, 0.2),
        (pid::MAXI_KNEE, 6.0), (pid::MAXI_CHARACTER, 0.3), (pid::MAXI_MIX, 1.0),
    ]},
    EffectPreset { name: "Drum Bus Glue", params: &[
        (pid::MAXI_INPUT, 8.0), (pid::MAXI_CEIL, -0.5), (pid::MAXI_RELEASE, 0.05),
        (pid::MAXI_KNEE, 3.0), (pid::MAXI_CHARACTER, 0.55), (pid::MAXI_MIX, 0.8),
    ]},
    EffectPreset { name: "Radio Ready", params: &[
        (pid::MAXI_INPUT, 10.0), (pid::MAXI_CEIL, -0.1), (pid::MAXI_RELEASE, 0.1),
        (pid::MAXI_KNEE, 2.0), (pid::MAXI_CHARACTER, 0.8), (pid::MAXI_MIX, 1.0),
    ]},
    EffectPreset { name: "Dynamic Preserver", params: &[
        (pid::MAXI_INPUT, 2.0), (pid::MAXI_CEIL, -0.3), (pid::MAXI_RELEASE, 0.3),
        (pid::MAXI_KNEE, 10.0), (pid::MAXI_CHARACTER, 0.1), (pid::MAXI_MIX, 1.0),
    ]},
];

// ═══════════════════════════════════════════════════════════════════════
// Phase 1 new effects: Spatializer3D, Parametric EQ, Doppler
// ═══════════════════════════════════════════════════════════════════════

static SPATIALIZER_PRESETS: [EffectPreset; 7] = [
    EffectPreset { name: "Center Close", params: &[
        (pid::SPAT_AZIMUTH, 0.0), (pid::SPAT_ELEVATION, 0.0), (pid::SPAT_DISTANCE, 0.8),
        (pid::SPAT_ROOM, 0.2), (pid::SPAT_AIR, 0.2), (pid::SPAT_ER, 0.15),
        (pid::SPAT_WIDTH, 1.0), (pid::SPAT_MODE, 0.0), (pid::SPAT_MIX, 1.0),
    ]},
    EffectPreset { name: "Hard Left", params: &[
        (pid::SPAT_AZIMUTH, -80.0), (pid::SPAT_ELEVATION, 0.0), (pid::SPAT_DISTANCE, 1.5),
        (pid::SPAT_ROOM, 0.3), (pid::SPAT_AIR, 0.3), (pid::SPAT_ER, 0.2),
        (pid::SPAT_WIDTH, 1.0), (pid::SPAT_MODE, 0.0), (pid::SPAT_MIX, 1.0),
    ]},
    EffectPreset { name: "Hard Right", params: &[
        (pid::SPAT_AZIMUTH, 80.0), (pid::SPAT_ELEVATION, 0.0), (pid::SPAT_DISTANCE, 1.5),
        (pid::SPAT_ROOM, 0.3), (pid::SPAT_AIR, 0.3), (pid::SPAT_ER, 0.2),
        (pid::SPAT_WIDTH, 1.0), (pid::SPAT_MODE, 0.0), (pid::SPAT_MIX, 1.0),
    ]},
    EffectPreset { name: "Overhead", params: &[
        (pid::SPAT_AZIMUTH, 0.0), (pid::SPAT_ELEVATION, 35.0), (pid::SPAT_DISTANCE, 2.0),
        (pid::SPAT_ROOM, 0.5), (pid::SPAT_AIR, 0.4), (pid::SPAT_ER, 0.35),
        (pid::SPAT_WIDTH, 1.1), (pid::SPAT_MODE, 0.0), (pid::SPAT_MIX, 1.0),
    ]},
    EffectPreset { name: "Distant Echo", params: &[
        (pid::SPAT_AZIMUTH, 45.0), (pid::SPAT_ELEVATION, -10.0), (pid::SPAT_DISTANCE, 12.0),
        (pid::SPAT_ROOM, 0.9), (pid::SPAT_AIR, 0.85), (pid::SPAT_ER, 0.7),
        (pid::SPAT_WIDTH, 1.3), (pid::SPAT_MODE, 0.0), (pid::SPAT_MIX, 1.0),
    ]},
    EffectPreset { name: "Whispered Ear", params: &[
        (pid::SPAT_AZIMUTH, 70.0), (pid::SPAT_ELEVATION, 0.0), (pid::SPAT_DISTANCE, 0.3),
        (pid::SPAT_ROOM, 0.1), (pid::SPAT_AIR, 0.1), (pid::SPAT_ER, 0.05),
        (pid::SPAT_WIDTH, 0.8), (pid::SPAT_MODE, 0.0), (pid::SPAT_MIX, 1.0),
    ]},
    EffectPreset { name: "Speaker Room", params: &[
        (pid::SPAT_AZIMUTH, 20.0), (pid::SPAT_ELEVATION, 0.0), (pid::SPAT_DISTANCE, 3.0),
        (pid::SPAT_ROOM, 0.5), (pid::SPAT_AIR, 0.4), (pid::SPAT_ER, 0.4),
        (pid::SPAT_WIDTH, 1.0), (pid::SPAT_MODE, 1.0), (pid::SPAT_MIX, 1.0),
    ]},
];

static PARAM_EQ_PRESETS: [EffectPreset; 8] = [
    EffectPreset { name: "Smile Curve", params: &[
        (pid::EQ_B0_FREQ, 100.0), (pid::EQ_B0_GAIN, 4.0), (pid::EQ_B0_Q, 0.7),
        (pid::EQ_B1_FREQ, 500.0), (pid::EQ_B1_GAIN, -2.0), (pid::EQ_B1_Q, 1.0),
        (pid::EQ_B2_FREQ, 2000.0), (pid::EQ_B2_GAIN, -1.0), (pid::EQ_B2_Q, 1.0),
        (pid::EQ_B3_FREQ, 10000.0), (pid::EQ_B3_GAIN, 3.5), (pid::EQ_B3_Q, 0.7),
        (pid::EQ_MIX, 1.0), (pid::EQ_ENABLE_MASK, 15.0),
    ]},
    EffectPreset { name: "Vocal Air", params: &[
        (pid::EQ_B0_FREQ, 80.0), (pid::EQ_B0_GAIN, -3.0), (pid::EQ_B0_Q, 0.7),
        (pid::EQ_B1_FREQ, 300.0), (pid::EQ_B1_GAIN, -2.0), (pid::EQ_B1_Q, 1.0),
        (pid::EQ_B2_FREQ, 3000.0), (pid::EQ_B2_GAIN, 2.0), (pid::EQ_B2_Q, 1.5),
        (pid::EQ_B3_FREQ, 12000.0), (pid::EQ_B3_GAIN, 5.0), (pid::EQ_B3_Q, 0.6),
        (pid::EQ_MIX, 1.0), (pid::EQ_ENABLE_MASK, 15.0),
    ]},
    EffectPreset { name: "Kick Punch", params: &[
        (pid::EQ_B0_FREQ, 60.0), (pid::EQ_B0_GAIN, 5.0), (pid::EQ_B0_Q, 1.0),
        (pid::EQ_B1_FREQ, 250.0), (pid::EQ_B1_GAIN, -4.0), (pid::EQ_B1_Q, 2.0),
        (pid::EQ_B2_FREQ, 3500.0), (pid::EQ_B2_GAIN, 4.0), (pid::EQ_B2_Q, 2.5),
        (pid::EQ_B3_FREQ, 8000.0), (pid::EQ_B3_GAIN, 0.0), (pid::EQ_B3_Q, 0.7),
        (pid::EQ_MIX, 1.0), (pid::EQ_ENABLE_MASK, 15.0),
    ]},
    EffectPreset { name: "Warm Tilt", params: &[
        (pid::EQ_B0_FREQ, 200.0), (pid::EQ_B0_GAIN, 3.0), (pid::EQ_B0_Q, 0.5),
        (pid::EQ_B1_FREQ, 500.0), (pid::EQ_B1_GAIN, 0.0), (pid::EQ_B1_Q, 1.0),
        (pid::EQ_B2_FREQ, 2000.0), (pid::EQ_B2_GAIN, 0.0), (pid::EQ_B2_Q, 1.0),
        (pid::EQ_B3_FREQ, 10000.0), (pid::EQ_B3_GAIN, -3.0), (pid::EQ_B3_Q, 0.5),
        (pid::EQ_MIX, 1.0), (pid::EQ_ENABLE_MASK, 15.0),
    ]},
    EffectPreset { name: "Bright Tilt", params: &[
        (pid::EQ_B0_FREQ, 200.0), (pid::EQ_B0_GAIN, -3.0), (pid::EQ_B0_Q, 0.5),
        (pid::EQ_B1_FREQ, 500.0), (pid::EQ_B1_GAIN, 0.0), (pid::EQ_B1_Q, 1.0),
        (pid::EQ_B2_FREQ, 2000.0), (pid::EQ_B2_GAIN, 0.0), (pid::EQ_B2_Q, 1.0),
        (pid::EQ_B3_FREQ, 10000.0), (pid::EQ_B3_GAIN, 4.0), (pid::EQ_B3_Q, 0.5),
        (pid::EQ_MIX, 1.0), (pid::EQ_ENABLE_MASK, 15.0),
    ]},
    EffectPreset { name: "Mud Cut", params: &[
        (pid::EQ_B0_FREQ, 80.0), (pid::EQ_B0_GAIN, 0.0), (pid::EQ_B0_Q, 0.7),
        (pid::EQ_B1_FREQ, 250.0), (pid::EQ_B1_GAIN, -6.0), (pid::EQ_B1_Q, 2.0),
        (pid::EQ_B2_FREQ, 500.0), (pid::EQ_B2_GAIN, -3.0), (pid::EQ_B2_Q, 1.5),
        (pid::EQ_B3_FREQ, 8000.0), (pid::EQ_B3_GAIN, 1.0), (pid::EQ_B3_Q, 0.7),
        (pid::EQ_MIX, 1.0), (pid::EQ_ENABLE_MASK, 15.0),
    ]},
    EffectPreset { name: "Bass Tighten", params: &[
        (pid::EQ_B0_FREQ, 40.0), (pid::EQ_B0_GAIN, -6.0), (pid::EQ_B0_Q, 0.7),
        (pid::EQ_B1_FREQ, 80.0), (pid::EQ_B1_GAIN, 3.0), (pid::EQ_B1_Q, 1.5),
        (pid::EQ_B2_FREQ, 800.0), (pid::EQ_B2_GAIN, -2.0), (pid::EQ_B2_Q, 1.2),
        (pid::EQ_B3_FREQ, 5000.0), (pid::EQ_B3_GAIN, 0.0), (pid::EQ_B3_Q, 0.7),
        (pid::EQ_MIX, 1.0), (pid::EQ_ENABLE_MASK, 15.0),
    ]},
    EffectPreset { name: "Flat (Bypass)", params: &[
        (pid::EQ_B0_FREQ, 100.0), (pid::EQ_B0_GAIN, 0.0), (pid::EQ_B0_Q, 0.707),
        (pid::EQ_B1_FREQ, 500.0), (pid::EQ_B1_GAIN, 0.0), (pid::EQ_B1_Q, 1.0),
        (pid::EQ_B2_FREQ, 2000.0), (pid::EQ_B2_GAIN, 0.0), (pid::EQ_B2_Q, 1.0),
        (pid::EQ_B3_FREQ, 8000.0), (pid::EQ_B3_GAIN, 0.0), (pid::EQ_B3_Q, 0.707),
        (pid::EQ_MIX, 1.0), (pid::EQ_ENABLE_MASK, 15.0),
    ]},
];

static DOPPLER_PRESETS: [EffectPreset; 6] = [
    EffectPreset { name: "Passing Car", params: &[
        (pid::DOP_VELOCITY, 15.0), (pid::DOP_ANGLE, 30.0), (pid::DOP_DISTANCE, 4.0),
        (pid::DOP_HPF, 0.6), (pid::DOP_MIX, 1.0),
    ]},
    EffectPreset { name: "Approaching", params: &[
        (pid::DOP_VELOCITY, -8.0), (pid::DOP_ANGLE, 0.0), (pid::DOP_DISTANCE, 3.0),
        (pid::DOP_HPF, 0.3), (pid::DOP_MIX, 1.0),
    ]},
    EffectPreset { name: "Receding", params: &[
        (pid::DOP_VELOCITY, 8.0), (pid::DOP_ANGLE, 0.0), (pid::DOP_DISTANCE, 3.0),
        (pid::DOP_HPF, 0.2), (pid::DOP_MIX, 1.0),
    ]},
    EffectPreset { name: "Subtle Drift", params: &[
        (pid::DOP_VELOCITY, 2.0), (pid::DOP_ANGLE, 45.0), (pid::DOP_DISTANCE, 1.5),
        (pid::DOP_HPF, 0.1), (pid::DOP_MIX, 0.5),
    ]},
    EffectPreset { name: "Windrush", params: &[
        (pid::DOP_VELOCITY, 18.0), (pid::DOP_ANGLE, 90.0), (pid::DOP_DISTANCE, 2.0),
        (pid::DOP_HPF, 0.95), (pid::DOP_MIX, 1.0),
    ]},
    EffectPreset { name: "Space Flyby", params: &[
        (pid::DOP_VELOCITY, 20.0), (pid::DOP_ANGLE, 60.0), (pid::DOP_DISTANCE, 8.0),
        (pid::DOP_HPF, 0.7), (pid::DOP_MIX, 1.0),
    ]},
];

// ═══════════════════════════════════════════════════════════════════════
// Phase 2 new effect: Transient Shaper
// ═══════════════════════════════════════════════════════════════════════

static TRANSIENT_SHAPER_PRESETS: [EffectPreset; 6] = [
    EffectPreset { name: "Punchy Drums", params: &[
        (pid::TS_ATTACK, 0.7), (pid::TS_SUSTAIN, -0.2), (pid::TS_MIX, 1.0),
    ]},
    EffectPreset { name: "Softer Attack", params: &[
        (pid::TS_ATTACK, -0.6), (pid::TS_SUSTAIN, 0.0), (pid::TS_MIX, 1.0),
    ]},
    EffectPreset { name: "Sustain Boost", params: &[
        (pid::TS_ATTACK, 0.0), (pid::TS_SUSTAIN, 0.6), (pid::TS_MIX, 1.0),
    ]},
    EffectPreset { name: "Tight Gate-ish", params: &[
        (pid::TS_ATTACK, 0.4), (pid::TS_SUSTAIN, -0.7), (pid::TS_MIX, 1.0),
    ]},
    EffectPreset { name: "Full Punch", params: &[
        (pid::TS_ATTACK, 0.9), (pid::TS_SUSTAIN, 0.3), (pid::TS_MIX, 1.0),
    ]},
    EffectPreset { name: "Release Tail", params: &[
        (pid::TS_ATTACK, -0.3), (pid::TS_SUSTAIN, 0.9), (pid::TS_MIX, 1.0),
    ]},
];

// ═══════════════════════════════════════════════════════════════════════
// Phase 3 new effects
// ═══════════════════════════════════════════════════════════════════════

static GRANULAR_FREEZER_PRESETS: [EffectPreset; 6] = [
    EffectPreset { name: "Dense Cloud", params: &[
        (pid::GRAN_SIZE, 80.0), (pid::GRAN_DENSITY, 0.85), (pid::GRAN_PITCH, 0.0),
        (pid::GRAN_SCATTER, 0.3), (pid::GRAN_MIX, 0.7),
    ]},
    EffectPreset { name: "Sparse Drops", params: &[
        (pid::GRAN_SIZE, 200.0), (pid::GRAN_DENSITY, 0.2), (pid::GRAN_PITCH, 0.0),
        (pid::GRAN_SCATTER, 0.7), (pid::GRAN_MIX, 0.6),
    ]},
    EffectPreset { name: "Octave Up Pad", params: &[
        (pid::GRAN_SIZE, 150.0), (pid::GRAN_DENSITY, 0.7), (pid::GRAN_PITCH, 12.0),
        (pid::GRAN_SCATTER, 0.4), (pid::GRAN_MIX, 0.65),
    ]},
    EffectPreset { name: "Sub Drone", params: &[
        (pid::GRAN_SIZE, 300.0), (pid::GRAN_DENSITY, 0.6), (pid::GRAN_PITCH, -12.0),
        (pid::GRAN_SCATTER, 0.2), (pid::GRAN_MIX, 0.8),
    ]},
    EffectPreset { name: "Scatter Texture", params: &[
        (pid::GRAN_SIZE, 100.0), (pid::GRAN_DENSITY, 0.5), (pid::GRAN_PITCH, 7.0),
        (pid::GRAN_SCATTER, 0.95), (pid::GRAN_MIX, 0.7),
    ]},
    EffectPreset { name: "Glacial Wash", params: &[
        (pid::GRAN_SIZE, 400.0), (pid::GRAN_DENSITY, 0.9), (pid::GRAN_PITCH, 0.0),
        (pid::GRAN_SCATTER, 0.5), (pid::GRAN_MIX, 0.8),
    ]},
];

static SPECTRAL_FREEZE_PRESETS: [EffectPreset; 5] = [
    EffectPreset { name: "Clean Hold", params: &[
        (pid::SPEC_FREEZE, 1.0), (pid::SPEC_SMEAR, 0.0), (pid::SPEC_TILT, 0.0),
        (pid::SPEC_MIX, 1.0),
    ]},
    EffectPreset { name: "Smeared Drone", params: &[
        (pid::SPEC_FREEZE, 1.0), (pid::SPEC_SMEAR, 0.85), (pid::SPEC_TILT, 0.0),
        (pid::SPEC_MIX, 0.7),
    ]},
    EffectPreset { name: "Dark Freeze", params: &[
        (pid::SPEC_FREEZE, 1.0), (pid::SPEC_SMEAR, 0.4), (pid::SPEC_TILT, -0.7),
        (pid::SPEC_MIX, 0.8),
    ]},
    EffectPreset { name: "Shimmer Hold", params: &[
        (pid::SPEC_FREEZE, 1.0), (pid::SPEC_SMEAR, 0.3), (pid::SPEC_TILT, 0.8),
        (pid::SPEC_MIX, 0.7),
    ]},
    EffectPreset { name: "Blend Under", params: &[
        (pid::SPEC_FREEZE, 1.0), (pid::SPEC_SMEAR, 0.5), (pid::SPEC_TILT, -0.2),
        (pid::SPEC_MIX, 0.35),
    ]},
];

static RESONATOR_BANK_PRESETS: [EffectPreset; 8] = [
    EffectPreset { name: "C Major Bell", params: &[
        (pid::RES_ROOT, 0.0), (pid::RES_OCTAVE, 0.0), (pid::RES_CHORD, 0.0),
        (pid::RES_SPREAD, 0.3), (pid::RES_DECAY, 0.8), (pid::RES_DAMPING, 0.3),
        (pid::RES_MIX, 0.6),
    ]},
    EffectPreset { name: "A Minor Pad", params: &[
        (pid::RES_ROOT, 9.0), (pid::RES_OCTAVE, -1.0), (pid::RES_CHORD, 1.0),
        (pid::RES_SPREAD, 0.5), (pid::RES_DECAY, 0.85), (pid::RES_DAMPING, 0.4),
        (pid::RES_MIX, 0.55),
    ]},
    EffectPreset { name: "Cmaj7 Glow", params: &[
        (pid::RES_ROOT, 0.0), (pid::RES_OCTAVE, 0.0), (pid::RES_CHORD, 2.0),
        (pid::RES_SPREAD, 0.4), (pid::RES_DECAY, 0.9), (pid::RES_DAMPING, 0.25),
        (pid::RES_MIX, 0.65),
    ]},
    EffectPreset { name: "Dmin7 Dream", params: &[
        (pid::RES_ROOT, 2.0), (pid::RES_OCTAVE, 0.0), (pid::RES_CHORD, 3.0),
        (pid::RES_SPREAD, 0.6), (pid::RES_DECAY, 0.88), (pid::RES_DAMPING, 0.3),
        (pid::RES_MIX, 0.6),
    ]},
    EffectPreset { name: "Sus4 Ambient", params: &[
        (pid::RES_ROOT, 7.0), (pid::RES_OCTAVE, 0.0), (pid::RES_CHORD, 4.0),
        (pid::RES_SPREAD, 0.5), (pid::RES_DECAY, 0.92), (pid::RES_DAMPING, 0.35),
        (pid::RES_MIX, 0.55),
    ]},
    EffectPreset { name: "Dim Tension", params: &[
        (pid::RES_ROOT, 5.0), (pid::RES_OCTAVE, 0.0), (pid::RES_CHORD, 5.0),
        (pid::RES_SPREAD, 0.3), (pid::RES_DECAY, 0.78), (pid::RES_DAMPING, 0.5),
        (pid::RES_MIX, 0.5),
    ]},
    EffectPreset { name: "Octave Drone", params: &[
        (pid::RES_ROOT, 0.0), (pid::RES_OCTAVE, -2.0), (pid::RES_CHORD, 6.0),
        (pid::RES_SPREAD, 0.2), (pid::RES_DECAY, 0.95), (pid::RES_DAMPING, 0.2),
        (pid::RES_MIX, 0.7),
    ]},
    EffectPreset { name: "Metallic Ring", params: &[
        (pid::RES_ROOT, 4.0), (pid::RES_OCTAVE, 1.0), (pid::RES_CHORD, 0.0),
        (pid::RES_SPREAD, 0.8), (pid::RES_DECAY, 0.7), (pid::RES_DAMPING, 0.15),
        (pid::RES_MIX, 0.5),
    ]},
];

static AUTO_WAH_PRESETS: [EffectPreset; 6] = [
    EffectPreset { name: "Funky Wah", params: &[
        (pid::WAH_SENS, 0.8), (pid::WAH_ATK, 3.0), (pid::WAH_REL, 80.0),
        (pid::WAH_RANGE, 2.5), (pid::WAH_Q, 5.0), (pid::WAH_DIR, 0.0),
        (pid::WAH_BASE, 300.0), (pid::WAH_MIX, 1.0),
    ]},
    EffectPreset { name: "Slow Sweep", params: &[
        (pid::WAH_SENS, 0.6), (pid::WAH_ATK, 30.0), (pid::WAH_REL, 400.0),
        (pid::WAH_RANGE, 3.5), (pid::WAH_Q, 3.5), (pid::WAH_DIR, 0.0),
        (pid::WAH_BASE, 200.0), (pid::WAH_MIX, 0.8),
    ]},
    EffectPreset { name: "Down Wah", params: &[
        (pid::WAH_SENS, 0.7), (pid::WAH_ATK, 8.0), (pid::WAH_REL, 150.0),
        (pid::WAH_RANGE, 3.0), (pid::WAH_Q, 4.0), (pid::WAH_DIR, 1.0),
        (pid::WAH_BASE, 1200.0), (pid::WAH_MIX, 1.0),
    ]},
    EffectPreset { name: "Tight Quack", params: &[
        (pid::WAH_SENS, 0.9), (pid::WAH_ATK, 1.0), (pid::WAH_REL, 40.0),
        (pid::WAH_RANGE, 1.5), (pid::WAH_Q, 8.0), (pid::WAH_DIR, 0.0),
        (pid::WAH_BASE, 500.0), (pid::WAH_MIX, 1.0),
    ]},
    EffectPreset { name: "Deep Bass Wah", params: &[
        (pid::WAH_SENS, 0.7), (pid::WAH_ATK, 5.0), (pid::WAH_REL, 200.0),
        (pid::WAH_RANGE, 2.5), (pid::WAH_Q, 3.0), (pid::WAH_DIR, 0.0),
        (pid::WAH_BASE, 120.0), (pid::WAH_MIX, 1.0),
    ]},
    EffectPreset { name: "Subtle Movement", params: &[
        (pid::WAH_SENS, 0.4), (pid::WAH_ATK, 15.0), (pid::WAH_REL, 250.0),
        (pid::WAH_RANGE, 1.2), (pid::WAH_Q, 2.0), (pid::WAH_DIR, 0.0),
        (pid::WAH_BASE, 400.0), (pid::WAH_MIX, 0.6),
    ]},
];

static TAPE_STOP_PRESETS: [EffectPreset; 5] = [
    EffectPreset { name: "Classic Slow", params: &[
        (pid::TAPE_TRIGGER, 0.0), (pid::TAPE_DURATION, 1.5),
        (pid::TAPE_CURVE, 1.0), (pid::TAPE_MIX, 1.0),
    ]},
    EffectPreset { name: "Fast Stutter", params: &[
        (pid::TAPE_TRIGGER, 0.0), (pid::TAPE_DURATION, 0.15),
        (pid::TAPE_CURVE, 0.0), (pid::TAPE_MIX, 1.0),
    ]},
    EffectPreset { name: "Long Drop", params: &[
        (pid::TAPE_TRIGGER, 0.0), (pid::TAPE_DURATION, 3.5),
        (pid::TAPE_CURVE, 1.0), (pid::TAPE_MIX, 1.0),
    ]},
    EffectPreset { name: "Linear Halt", params: &[
        (pid::TAPE_TRIGGER, 0.0), (pid::TAPE_DURATION, 1.0),
        (pid::TAPE_CURVE, 0.0), (pid::TAPE_MIX, 1.0),
    ]},
    EffectPreset { name: "DJ Brake", params: &[
        (pid::TAPE_TRIGGER, 0.0), (pid::TAPE_DURATION, 0.6),
        (pid::TAPE_CURVE, 1.0), (pid::TAPE_MIX, 1.0),
    ]},
];

// ─── Mastering tier per-effect presets ────────────────────────────────
// Each list is small (3-4 entries) and focused. The rack header's
// preset picker only shows when a non-empty list is returned, so the
// meter-only effects (LufsMeter, SpectrumAnalyzer) intentionally have
// no preset arrays — there's nothing useful to preset.

static BRICKWALL_LIMITER_PRESETS: [EffectPreset; 4] = [
    EffectPreset { name: "Streaming -1 dB", params: &[
        // -1 dB ceiling gives lossy re-encode headroom (Spotify/Apple
        // Music quantise the master). Conservative release rides
        // dynamics without pumping.
        (pid::BRICK_CEILING_DB, -1.0),
        (pid::BRICK_RELEASE_MS, 100.0),
        (pid::BRICK_MIX, 1.0),
    ]},
    EffectPreset { name: "Club -0.3 dB", params: &[
        // Loud-but-clean mastering target. Fast release for transient
        // punch.
        (pid::BRICK_CEILING_DB, -0.3),
        (pid::BRICK_RELEASE_MS, 30.0),
        (pid::BRICK_MIX, 1.0),
    ]},
    EffectPreset { name: "Hard 0 dB", params: &[
        // Push to the absolute ceiling — only safe when downstream
        // re-encoding is not in the pipeline.
        (pid::BRICK_CEILING_DB, 0.0),
        (pid::BRICK_RELEASE_MS, 20.0),
        (pid::BRICK_MIX, 1.0),
    ]},
    EffectPreset { name: "Gentle -0.5", params: &[
        // Long release stays out of the way on classical / jazz mixes
        // where transparency matters more than peak push.
        (pid::BRICK_CEILING_DB, -0.5),
        (pid::BRICK_RELEASE_MS, 250.0),
        (pid::BRICK_MIX, 1.0),
    ]},
];

static MASTER_ASSISTANT_PRESETS: [EffectPreset; 4] = [
    EffectPreset { name: "Streaming -14", params: &[
        (pid::MA_MODE, 2.0),         // Apply
        (pid::MA_TARGET_LUFS, -14.0),
        (pid::MA_TARGET_CURVE, 2.0), // Balanced
        (pid::MA_MIX, 1.0),
    ]},
    EffectPreset { name: "Club -8", params: &[
        (pid::MA_MODE, 2.0),
        (pid::MA_TARGET_LUFS, -8.0),
        (pid::MA_TARGET_CURVE, 0.0), // Modern Techno
        (pid::MA_MIX, 1.0),
    ]},
    EffectPreset { name: "Broadcast -16", params: &[
        (pid::MA_MODE, 2.0),
        (pid::MA_TARGET_LUFS, -16.0),
        (pid::MA_TARGET_CURVE, 3.0), // Vocal-Forward
        (pid::MA_MIX, 1.0),
    ]},
    EffectPreset { name: "Listen Only", params: &[
        // Pure metering — measures LUFS + spectrum but never modifies
        // audio. Drop on the master while mixing for objective feedback.
        (pid::MA_MODE, 1.0),         // Listen
        (pid::MA_TARGET_LUFS, -14.0),
        (pid::MA_TARGET_CURVE, 2.0),
        (pid::MA_MIX, 1.0),
    ]},
];

static DYNAMIC_EQ_PRESETS: [EffectPreset; 3] = [
    EffectPreset { name: "De-ess Vocal", params: &[
        // Band 2 (pid base 36) — narrow Q dip at 6 kHz that engages
        // only when sibilance peaks past -20 dB.
        (pid::DYNEQ_B0_FREQ + 2 * pid::DYNEQ_BAND_STRIDE, 6000.0),
        (pid::DYNEQ_B0_Q + 2 * pid::DYNEQ_BAND_STRIDE, 3.0),
        (pid::DYNEQ_B0_THRESH + 2 * pid::DYNEQ_BAND_STRIDE, -20.0),
        (pid::DYNEQ_B0_RATIO + 2 * pid::DYNEQ_BAND_STRIDE, 3.0),
        (pid::DYNEQ_B0_RANGE + 2 * pid::DYNEQ_BAND_STRIDE, 6.0),
        (pid::DYNEQ_B0_ATK + 2 * pid::DYNEQ_BAND_STRIDE, 0.005),
        (pid::DYNEQ_B0_REL + 2 * pid::DYNEQ_BAND_STRIDE, 0.050),
        (pid::DYNEQ_B0_MODE + 2 * pid::DYNEQ_BAND_STRIDE, 1.0), // enabled+downward
        (pid::DYNEQ_MIX, 1.0),
    ]},
    EffectPreset { name: "Tame Boomy", params: &[
        // Band 0 (pid base 20) — wider Q dip around 120 Hz to control
        // muddy low-mids when they build up.
        (pid::DYNEQ_B0_FREQ, 120.0),
        (pid::DYNEQ_B0_Q, 0.7),
        (pid::DYNEQ_B0_THRESH, -18.0),
        (pid::DYNEQ_B0_RATIO, 2.5),
        (pid::DYNEQ_B0_RANGE, 4.0),
        (pid::DYNEQ_B0_ATK, 0.015),
        (pid::DYNEQ_B0_REL, 0.200),
        (pid::DYNEQ_B0_MODE, 1.0),
        (pid::DYNEQ_MIX, 1.0),
    ]},
    EffectPreset { name: "Bright Lift", params: &[
        // Band 3 (pid base 44) — upward expansion at 10 kHz to add
        // air when the high end goes quiet.
        (pid::DYNEQ_B0_FREQ + 3 * pid::DYNEQ_BAND_STRIDE, 10000.0),
        (pid::DYNEQ_B0_Q + 3 * pid::DYNEQ_BAND_STRIDE, 0.8),
        (pid::DYNEQ_B0_THRESH + 3 * pid::DYNEQ_BAND_STRIDE, -24.0),
        (pid::DYNEQ_B0_RATIO + 3 * pid::DYNEQ_BAND_STRIDE, 2.0),
        (pid::DYNEQ_B0_RANGE + 3 * pid::DYNEQ_BAND_STRIDE, 3.0),
        (pid::DYNEQ_B0_ATK + 3 * pid::DYNEQ_BAND_STRIDE, 0.030),
        (pid::DYNEQ_B0_REL + 3 * pid::DYNEQ_BAND_STRIDE, 0.300),
        (pid::DYNEQ_B0_MODE + 3 * pid::DYNEQ_BAND_STRIDE, 2.0), // enabled+upward
        (pid::DYNEQ_MIX, 1.0),
    ]},
];

static MID_SIDE_EQ_PRESETS: [EffectPreset; 3] = [
    EffectPreset { name: "Wide Highs", params: &[
        // Side band 3 (pid base 41) — high shelf boost above 5 kHz
        // on the difference signal only, widens the stereo image.
        (pid::MSEQ_S_B0_FREQ + 9, 6000.0),
        (pid::MSEQ_S_B0_GAIN + 9, 3.0),
        (pid::MSEQ_S_B0_Q    + 9, 0.7),
        (pid::MSEQ_MIX, 1.0),
    ]},
    EffectPreset { name: "Tight Lows", params: &[
        // Side band 0 (pid base 32) — low shelf cut below 200 Hz on
        // the side channel; sub stays mono via the M path.
        (pid::MSEQ_S_B0_FREQ, 200.0),
        (pid::MSEQ_S_B0_GAIN, -6.0),
        (pid::MSEQ_S_B0_Q,    0.7),
        (pid::MSEQ_MIX, 1.0),
    ]},
    EffectPreset { name: "Centre Air", params: &[
        // Mid band 2 (pid base 26) — gentle peak around 12 kHz on the
        // sum signal for air without widening.
        (pid::MSEQ_M_B0_FREQ + 6, 12000.0),
        (pid::MSEQ_M_B0_GAIN + 6, 2.0),
        (pid::MSEQ_M_B0_Q    + 6, 0.7),
        (pid::MSEQ_MIX, 1.0),
    ]},
];

static MULTIBAND_COMP_PRESETS: [EffectPreset; 3] = [
    EffectPreset { name: "Glue", params: &[
        // Gentle bus-style compression — 1.5:1 across all 3 bands.
        (pid::MBC_XOVER_LOW,  200.0),
        (pid::MBC_XOVER_HIGH, 2000.0),
        // Low
        (pid::MBC_B0_THRESH, -16.0), (pid::MBC_B0_RATIO, 1.5),
        (pid::MBC_B0_ATK, 0.020),    (pid::MBC_B0_REL, 0.200),
        (pid::MBC_B0_MAKEUP, 0.0),   (pid::MBC_B0_ENABLED, 1.0),
        // Mid
        (pid::MBC_B0_THRESH + pid::MBC_BAND_STRIDE, -18.0),
        (pid::MBC_B0_RATIO + pid::MBC_BAND_STRIDE, 1.5),
        (pid::MBC_B0_ATK + pid::MBC_BAND_STRIDE, 0.015),
        (pid::MBC_B0_REL + pid::MBC_BAND_STRIDE, 0.150),
        (pid::MBC_B0_MAKEUP + pid::MBC_BAND_STRIDE, 0.0),
        (pid::MBC_B0_ENABLED + pid::MBC_BAND_STRIDE, 1.0),
        // Hi
        (pid::MBC_B0_THRESH + 2 * pid::MBC_BAND_STRIDE, -20.0),
        (pid::MBC_B0_RATIO + 2 * pid::MBC_BAND_STRIDE, 1.5),
        (pid::MBC_B0_ATK + 2 * pid::MBC_BAND_STRIDE, 0.008),
        (pid::MBC_B0_REL + 2 * pid::MBC_BAND_STRIDE, 0.100),
        (pid::MBC_B0_MAKEUP + 2 * pid::MBC_BAND_STRIDE, 0.0),
        (pid::MBC_B0_ENABLED + 2 * pid::MBC_BAND_STRIDE, 1.0),
        (pid::MBC_MIX, 1.0),
    ]},
    EffectPreset { name: "Master Punch", params: &[
        // Harder low-band compression for a deliberately tight kick
        // through the mid range.
        (pid::MBC_XOVER_LOW,  150.0),
        (pid::MBC_XOVER_HIGH, 2500.0),
        (pid::MBC_B0_THRESH, -10.0), (pid::MBC_B0_RATIO, 3.0),
        (pid::MBC_B0_ATK, 0.008),    (pid::MBC_B0_REL, 0.120),
        (pid::MBC_B0_MAKEUP, 1.0),   (pid::MBC_B0_ENABLED, 1.0),
        (pid::MBC_B0_THRESH + pid::MBC_BAND_STRIDE, -14.0),
        (pid::MBC_B0_RATIO + pid::MBC_BAND_STRIDE, 2.0),
        (pid::MBC_B0_ATK + pid::MBC_BAND_STRIDE, 0.012),
        (pid::MBC_B0_REL + pid::MBC_BAND_STRIDE, 0.120),
        (pid::MBC_B0_MAKEUP + pid::MBC_BAND_STRIDE, 1.0),
        (pid::MBC_B0_ENABLED + pid::MBC_BAND_STRIDE, 1.0),
        (pid::MBC_B0_THRESH + 2 * pid::MBC_BAND_STRIDE, -18.0),
        (pid::MBC_B0_RATIO + 2 * pid::MBC_BAND_STRIDE, 1.5),
        (pid::MBC_B0_ATK + 2 * pid::MBC_BAND_STRIDE, 0.004),
        (pid::MBC_B0_REL + 2 * pid::MBC_BAND_STRIDE, 0.080),
        (pid::MBC_B0_MAKEUP + 2 * pid::MBC_BAND_STRIDE, 0.5),
        (pid::MBC_B0_ENABLED + 2 * pid::MBC_BAND_STRIDE, 1.0),
        (pid::MBC_MIX, 1.0),
    ]},
    EffectPreset { name: "Bus Comp", params: &[
        // Vintage SSL-style mid-band-only compression. Low + high
        // pass through, mid does the work.
        (pid::MBC_XOVER_LOW,  200.0),
        (pid::MBC_XOVER_HIGH, 2000.0),
        (pid::MBC_B0_RATIO, 1.0), (pid::MBC_B0_ENABLED, 0.0),
        (pid::MBC_B0_THRESH + pid::MBC_BAND_STRIDE, -14.0),
        (pid::MBC_B0_RATIO + pid::MBC_BAND_STRIDE, 2.0),
        (pid::MBC_B0_ATK + pid::MBC_BAND_STRIDE, 0.030),
        (pid::MBC_B0_REL + pid::MBC_BAND_STRIDE, 0.300),
        (pid::MBC_B0_MAKEUP + pid::MBC_BAND_STRIDE, 1.5),
        (pid::MBC_B0_ENABLED + pid::MBC_BAND_STRIDE, 1.0),
        (pid::MBC_B0_RATIO + 2 * pid::MBC_BAND_STRIDE, 1.0),
        (pid::MBC_B0_ENABLED + 2 * pid::MBC_BAND_STRIDE, 0.0),
        (pid::MBC_MIX, 1.0),
    ]},
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_named_rack_preset_resolves() {
        // Every name listed in `get_fx_preset_categories()` must
        // resolve via `find_fx_preset()` — guards against typos and
        // ensures the category bar in the GUI doesn't show ghost
        // entries that fail to load.
        for cat in get_fx_preset_categories() {
            for name in cat.presets {
                assert!(find_fx_preset(name).is_some(),
                    "FX preset category '{}' references unknown preset '{}'",
                    cat.name, name);
            }
        }
    }

    #[test]
    fn every_rack_preset_uses_valid_effect_types() {
        // Each slot's `effect_type` must index a non-`None` EffectType.
        // Catches off-by-one errors when reordering the enum.
        use crate::effects::EffectType;
        let total_types = EffectType::all().len();
        for preset in get_all_fx_presets() {
            for (i, slot) in preset.slots.iter().enumerate() {
                assert!(slot.effect_type > 0 && slot.effect_type < total_types,
                    "preset '{}' slot {} has invalid effect_type {} (must be 1..{})",
                    preset.name, i, slot.effect_type, total_types);
                let et = EffectType::from_index(slot.effect_type);
                assert_ne!(et, EffectType::None,
                    "preset '{}' slot {} effect_type {} resolved to None",
                    preset.name, i, slot.effect_type);
            }
        }
    }

    #[test]
    fn pro_mastering_presets_use_pro_mastering_suite() {
        // The four "Pro Master *" presets must compose from the new
        // mastering tier (DynamicEq, MidSideEq, MultibandComp3,
        // BrickwallLimiter, LufsMeter, MasterAssistant). Defends
        // against future edits accidentally swapping them back to the
        // legacy Compressor + Limiter chain.
        use crate::effects::EffectType;
        const PRO_NAMES: &[&str] = &[
            "Pro Master Smart",
            "Pro Master Club",
            "Pro Master Streaming",
            "Pro Master Reference",
        ];
        for name in PRO_NAMES {
            let preset = find_fx_preset(name).expect("pro preset missing");
            let mut uses_pro_tool = false;
            for slot in preset.slots {
                let et = EffectType::from_index(slot.effect_type);
                if matches!(et,
                    EffectType::BrickwallLimiter |
                    EffectType::LufsMeter |
                    EffectType::DynamicEq |
                    EffectType::MidSideEq |
                    EffectType::MultibandComp3 |
                    EffectType::SpectrumAnalyzer |
                    EffectType::MasterAssistant) {
                    uses_pro_tool = true;
                }
                // Legacy single-band Compressor / Limiter must not
                // appear in the pro presets.
                assert!(
                    !matches!(et, EffectType::Compressor | EffectType::Limiter),
                    "preset '{name}' should not use legacy {et:?}");
            }
            assert!(uses_pro_tool,
                "preset '{name}' should use at least one pro mastering tool");
        }
    }

    #[test]
    fn audible_mastering_effects_have_per_effect_presets() {
        // Every audible mastering effect should expose presets via
        // get_effect_presets so the rack header's preset picker
        // actually surfaces when the user selects one. The two
        // meter-only effects (LufsMeter, SpectrumAnalyzer) must NOT
        // — they have nothing to preset.
        use crate::effects::EffectType;
        let audible = [
            EffectType::BrickwallLimiter,
            EffectType::DynamicEq,
            EffectType::MidSideEq,
            EffectType::MultibandComp3,
            EffectType::MasterAssistant,
        ];
        for et in audible {
            let presets = get_effect_presets(et.index());
            assert!(!presets.is_empty(),
                "{et:?} should have per-effect presets; rack header picker is hidden when empty");
        }
        let meter_only = [EffectType::LufsMeter, EffectType::SpectrumAnalyzer];
        for et in meter_only {
            let presets = get_effect_presets(et.index());
            assert!(presets.is_empty(),
                "{et:?} is meter-only; should NOT have per-effect presets");
        }
    }

    #[test]
    fn every_per_effect_preset_uses_known_pids() {
        // Sanity: every PID referenced in a mastering per-effect
        // preset must fit in the 100-entry base_params array. Catches
        // overflows where a preset references a PID outside the
        // storage range — saved value would be silently dropped on
        // session reload.
        use crate::effects::EffectType;
        let mastering = [
            EffectType::BrickwallLimiter,
            EffectType::DynamicEq,
            EffectType::MidSideEq,
            EffectType::MultibandComp3,
            EffectType::MasterAssistant,
        ];
        for et in mastering {
            for preset in get_effect_presets(et.index()) {
                for &(pid, _) in preset.params {
                    assert!((pid as usize) < 100,
                        "{:?} preset '{}' uses pid {} outside the 100-entry base_params range",
                        et, preset.name, pid);
                }
            }
        }
    }
}
