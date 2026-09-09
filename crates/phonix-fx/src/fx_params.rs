//! Effect parameter identifiers and value formatting.
//!
//! The common language between the global FX rack, per-sound drum FX and
//! every plugin that exposes an effect slot: a flat `u8` PID space, the
//! accessor trait a UI reads and writes through, and the value formatters.
//! Pure data — the egui drawing that consumes it stays in
//! `phonix::gui::fx_controls`.

/// Universal effect parameter IDs (matches DrumFxParam u8 values)
/// Used as the common language between global FX and per-sound FX.
pub mod pid {
    // Delay
    pub const DELAY_TIME: u8 = 0;
    pub const DELAY_FEEDBACK: u8 = 1;
    pub const DELAY_MIX: u8 = 2;
    pub const DELAY_STEREO: u8 = 3;
    pub const DELAY_HICUT: u8 = 4;
    pub const DELAY_LOCUT: u8 = 5;
    pub const DELAY_SYNC: u8 = 6;
    pub const DELAY_SYNC_DIV: u8 = 7;
    pub const DELAY_TYPE: u8 = 8;
    // Reverb
    pub const REVERB_SIZE: u8 = 10;
    pub const REVERB_DECAY: u8 = 11;
    pub const REVERB_DAMP: u8 = 12;
    pub const REVERB_PREDELAY: u8 = 13;
    pub const REVERB_MIX: u8 = 14;
    pub const REVERB_WIDTH: u8 = 15;
    pub const REVERB_TYPE: u8 = 16;
    // Chorus
    pub const CHORUS_RATE: u8 = 20;
    pub const CHORUS_DEPTH: u8 = 21;
    pub const CHORUS_MIX: u8 = 22;
    pub const CHORUS_DELAY: u8 = 23;
    pub const CHORUS_VOICES: u8 = 24;
    pub const CHORUS_WIDTH: u8 = 25;
    pub const CHORUS_SYNC: u8 = 26;
    pub const CHORUS_SYNC_DIV: u8 = 27;
    // Ensemble (BBD) — shares the Chorus-family base_param slots. base_params is
    // per-slot and a slot is never both Chorus and Ensemble, so the reuse is
    // safe (same pattern as CONVREV_IR_KIND sharing REVERB_TYPE). Depth = BBD
    // modulation depth; Mix = dry/wet.
    pub const ENSEMBLE_DEPTH: u8 = 21;
    pub const ENSEMBLE_MIX: u8 = 22;
    // Spring Tank (VCS-3 faithful spring) — aliases the Reverb base_param slots
    // (per-slot, never both Reverb and SpringTank, so no conflict).
    pub const SPRINGTANK_DECAY: u8 = 11; // shares REVERB_DECAY
    pub const SPRINGTANK_MIX: u8 = 14;   // shares REVERB_MIX
    // Cinema Plate (Aurora faithful hall) — aliases the Reverb base_param slots.
    pub const CINEMAPLATE_SIZE: u8 = 10; // shares REVERB_SIZE
    pub const CINEMAPLATE_DAMP: u8 = 12; // shares REVERB_DAMP
    pub const CINEMAPLATE_MIX: u8 = 14;  // shares REVERB_MIX
    // Solina Verb (VP-330 faithful Schroeder) — aliases the Reverb slots.
    pub const SOLINAVERB_SIZE: u8 = 10; // shares REVERB_SIZE
    pub const SOLINAVERB_DAMP: u8 = 12; // shares REVERB_DAMP
    pub const SOLINAVERB_MIX: u8 = 14;  // shares REVERB_MIX
    // Dimension (Aurora faithful stereo BBD chorus) — aliases the Chorus slots.
    pub const DIMENSION_DELAY: u8 = 23; // shares CHORUS_DELAY (base delay ms)
    pub const DIMENSION_RATE: u8 = 20;  // shares CHORUS_RATE (Hz)
    pub const DIMENSION_MIX: u8 = 22;   // shares CHORUS_MIX
    // Phaser
    pub const PHASER_RATE: u8 = 30;
    pub const PHASER_DEPTH: u8 = 31;
    pub const PHASER_MIX: u8 = 32;
    pub const PHASER_STAGES: u8 = 33;
    pub const PHASER_FEEDBACK: u8 = 34;
    pub const PHASER_STEREO: u8 = 35;
    pub const PHASER_SYNC: u8 = 36;
    pub const PHASER_SYNC_DIV: u8 = 37;
    // Flanger
    pub const FLANGER_RATE: u8 = 40;
    pub const FLANGER_DEPTH: u8 = 41;
    pub const FLANGER_MIX: u8 = 42;
    pub const FLANGER_DELAY: u8 = 43;
    pub const FLANGER_FEEDBACK: u8 = 44;
    pub const FLANGER_STEREO: u8 = 45;
    pub const FLANGER_SYNC: u8 = 46;
    pub const FLANGER_SYNC_DIV: u8 = 47;
    // Distortion
    pub const DIST_DRIVE: u8 = 50;
    pub const DIST_TONE: u8 = 51;
    pub const DIST_MIX: u8 = 52;
    pub const DIST_OUTPUT: u8 = 53;
    pub const DIST_TYPE: u8 = 54;
    pub const DIST_BITS: u8 = 55;
    pub const DIST_SR_DIV: u8 = 56;
    // Compressor
    pub const COMP_THRESH: u8 = 60;
    pub const COMP_RATIO: u8 = 61;
    pub const COMP_ATTACK: u8 = 62;
    pub const COMP_RELEASE: u8 = 63;
    pub const COMP_MAKEUP: u8 = 64;
    pub const COMP_KNEE: u8 = 65;
    pub const COMP_MIX: u8 = 66;
    pub const COMP_MODE: u8 = 67;
    pub const COMP_SC_HPF: u8 = 68;       // shares PID 68 with APAN_TREM_DEPTH (safe, per-slot)
    pub const COMP_AUTO_MAKEUP: u8 = 69;  // shares PID 69 with STUT_SPEED (safe, per-slot)
    // Limiter
    pub const LIMITER_CEIL: u8 = 70;
    pub const LIMITER_RELEASE: u8 = 71;
    pub const LIMITER_MIX: u8 = 72;
    pub const LIMITER_MODE: u8 = 73;
    pub const LIMITER_GAIN: u8 = 74;
    // Expander
    pub const EXP_THRESH: u8 = 80;
    pub const EXP_RATIO: u8 = 81;
    pub const EXP_ATTACK: u8 = 82;
    pub const EXP_RELEASE: u8 = 83;
    pub const EXP_RANGE: u8 = 84;
    pub const EXP_KNEE: u8 = 85;
    pub const EXP_MIX: u8 = 86;
    pub const EXP_MODE: u8 = 87;
    // Vocoder
    pub const VOC_BANDS: u8 = 90;
    pub const VOC_ATTACK: u8 = 91;
    pub const VOC_RELEASE: u8 = 92;
    pub const VOC_MIX: u8 = 93;
    pub const VOC_CARRIER: u8 = 94;
    pub const VOC_MOD: u8 = 95;
    pub const VOC_FORMANT: u8 = 96;
    // AutoPanner
    pub const APAN_MODE: u8 = 17;
    pub const APAN_WAVE: u8 = 18;
    pub const APAN_RATE: u8 = 19;
    pub const APAN_DEPTH: u8 = 9;
    pub const APAN_MIX: u8 = 48;
    pub const APAN_WIDTH: u8 = 49;
    pub const APAN_PHASE: u8 = 57;
    pub const APAN_GLIDE: u8 = 58;
    pub const APAN_SPIN_SPEED: u8 = 59;
    pub const APAN_TREM_DEPTH: u8 = 68;
    pub const APAN_SYNC: u8 = 56;
    pub const APAN_SYNC_DIV: u8 = 67;
    // Stutter (per-slot, no PID conflicts)
    pub const STUT_MODE: u8 = 28;
    pub const STUT_RATE: u8 = 29;
    pub const STUT_GATE: u8 = 38;
    pub const STUT_MIX: u8 = 39;
    pub const STUT_SPEED: u8 = 69;
    pub const STUT_SYNC: u8 = 36;
    pub const STUT_SYNC_DIV: u8 = 37;
    // Filter
    pub const FILT_MODE: u8 = 75;
    pub const FILT_CUTOFF: u8 = 76;
    pub const FILT_RESO: u8 = 77;
    pub const FILT_LFO_RATE: u8 = 78;
    pub const FILT_LFO_DEPTH: u8 = 79;
    pub const FILT_ENV: u8 = 88;
    pub const FILT_MIX: u8 = 89;
    pub const FILT_SYNC: u8 = 86;
    pub const FILT_SYNC_DIV: u8 = 87;
    // Ring Mod
    pub const RMOD_FREQ: u8 = 97;
    pub const RMOD_WAVE: u8 = 98;
    pub const RMOD_DEPTH: u8 = 99;
    pub const RMOD_MIX: u8 = 69; // safe: different effect type than Stutter
    // Lo-Fi (reuse PIDs safe per-slot)
    pub const LOFI_SR_DIV: u8 = 28;
    pub const LOFI_BITS: u8 = 29;
    pub const LOFI_NOISE: u8 = 38;
    pub const LOFI_WOW: u8 = 39;
    pub const LOFI_FLUTTER: u8 = 75;
    pub const LOFI_ROLLOFF: u8 = 76;
    pub const LOFI_MIX: u8 = 77;
    pub const LOFI_DITHER: u8 = 48;
    pub const LOFI_MU_LAW: u8 = 49;
    pub const LOFI_JITTER: u8 = 57;
    // Stereo Imager
    pub const SIMG_WIDTH: u8 = 28;
    pub const SIMG_MONO_FREQ: u8 = 29;
    pub const SIMG_MIX: u8 = 38;
    // Choir (reuse PIDs safe per-slot). All < 64 so they round-trip through
    // `restore_slots_from_patch`'s [f32;64] store (the old 75/76 never did).
    pub const CHOIR_VOICES: u8 = 28;
    pub const CHOIR_DETUNE: u8 = 29; // was SPREAD — random-walk pitch depth
    pub const CHOIR_WIDTH: u8 = 38;
    pub const CHOIR_WANDER: u8 = 39; // was SHIMMER — random-walk rate
    pub const CHOIR_VIBRATO: u8 = 30;
    pub const CHOIR_TIMBRE: u8 = 31;
    pub const CHOIR_WARMTH: u8 = 32; // was 75 (>=64, never persisted)
    pub const CHOIR_MIX: u8 = 33;    // was 76 (>=64, never persisted)
    // Maximizer (reuse PIDs safe per-slot)
    pub const MAXI_INPUT: u8 = 28;
    pub const MAXI_CEIL: u8 = 29;
    pub const MAXI_RELEASE: u8 = 38;
    pub const MAXI_KNEE: u8 = 39;
    pub const MAXI_CHARACTER: u8 = 75;
    pub const MAXI_MIX: u8 = 76;
    // Spatializer3D (reuse PIDs safe per-slot)
    pub const SPAT_AZIMUTH: u8 = 28;
    pub const SPAT_ELEVATION: u8 = 29;
    pub const SPAT_DISTANCE: u8 = 38;
    pub const SPAT_ROOM: u8 = 39;
    pub const SPAT_AIR: u8 = 48;
    pub const SPAT_ER: u8 = 49;
    pub const SPAT_WIDTH: u8 = 57;
    pub const SPAT_MODE: u8 = 58;
    pub const SPAT_MIX: u8 = 59;
    // Parametric EQ (reuse PIDs safe per-slot)
    pub const EQ_B0_FREQ: u8 = 28;
    pub const EQ_B0_GAIN: u8 = 29;
    pub const EQ_B0_Q: u8 = 38;
    pub const EQ_B1_FREQ: u8 = 39;
    pub const EQ_B1_GAIN: u8 = 48;
    pub const EQ_B1_Q: u8 = 49;
    pub const EQ_B2_FREQ: u8 = 57;
    pub const EQ_B2_GAIN: u8 = 58;
    pub const EQ_B2_Q: u8 = 59;
    pub const EQ_B3_FREQ: u8 = 68;
    pub const EQ_B3_GAIN: u8 = 69;
    pub const EQ_B3_Q: u8 = 78;
    pub const EQ_MIX: u8 = 79;
    pub const EQ_ENABLE_MASK: u8 = 80; // bitmask 1..0xF for 4 bands
    // Per-band filter type (0=auto/position-default, 1=peak, 2=low-shelf, 3=high-shelf, 4=low-pass).
    // 0 keeps the historical layout so existing patches are byte-identical.
    pub const EQ_B0_TYPE: u8 = 81;
    pub const EQ_B1_TYPE: u8 = 82;
    pub const EQ_B2_TYPE: u8 = 83;
    pub const EQ_B3_TYPE: u8 = 84;
    // Doppler (reuse PIDs safe per-slot)
    pub const DOP_VELOCITY: u8 = 28;
    pub const DOP_ANGLE: u8 = 29;
    pub const DOP_DISTANCE: u8 = 38;
    pub const DOP_HPF: u8 = 39;
    pub const DOP_MIX: u8 = 48;
    // Transient Shaper (reuse PIDs safe per-slot)
    pub const TS_ATTACK: u8 = 28;
    pub const TS_SUSTAIN: u8 = 29;
    pub const TS_MIX: u8 = 38;
    // Granular Freezer (reuse PIDs safe per-slot)
    pub const GRAN_SIZE: u8 = 28;
    pub const GRAN_DENSITY: u8 = 29;
    pub const GRAN_PITCH: u8 = 38;
    pub const GRAN_SCATTER: u8 = 39;
    pub const GRAN_MIX: u8 = 48;
    pub const GRAN_CAPTURE: u8 = 49; // edge-triggered by GUI → engine clears
    // Spectral Freeze (reuse PIDs safe per-slot)
    pub const SPEC_FREEZE: u8 = 28;
    pub const SPEC_SMEAR: u8 = 29;
    pub const SPEC_TILT: u8 = 38;
    pub const SPEC_MIX: u8 = 39;
    // Resonator Bank (reuse PIDs safe per-slot)
    pub const RES_ROOT: u8 = 28;
    pub const RES_OCTAVE: u8 = 29;
    pub const RES_CHORD: u8 = 38;
    pub const RES_SPREAD: u8 = 39;
    pub const RES_DECAY: u8 = 48;
    pub const RES_DAMPING: u8 = 49;
    pub const RES_MIX: u8 = 57;
    pub const RES_SYMPATHY: u8 = 58;
    pub const RES_FOLLOW: u8 = 59;
    // Auto-Wah (reuse PIDs safe per-slot)
    pub const WAH_SENS: u8 = 28;
    pub const WAH_ATK: u8 = 29;
    pub const WAH_REL: u8 = 38;
    pub const WAH_RANGE: u8 = 39;
    pub const WAH_Q: u8 = 48;
    pub const WAH_DIR: u8 = 49;
    pub const WAH_BASE: u8 = 57;
    pub const WAH_MIX: u8 = 58;
    // Tape Stop (reuse PIDs safe per-slot)
    pub const TAPE_TRIGGER: u8 = 28;
    pub const TAPE_DURATION: u8 = 29;
    pub const TAPE_CURVE: u8 = 38;
    pub const TAPE_MIX: u8 = 39;
    // Brickwall Limiter (mastering Phase A). Per-slot PIDs.
    pub const BRICK_CEILING_DB: u8 = 28;
    pub const BRICK_RELEASE_MS: u8 = 29;
    pub const BRICK_MIX: u8 = 38;
    // LUFS Meter (mastering Phase A). Display-only; no knobs.
    pub const LUFS_MIX: u8 = 28;
    // Dynamic EQ (mastering Phase B). 4 bands × 8 params, single mix.
    // Band layout: [freq, q, threshold_db, ratio, range_db, attack_ms,
    //               release_ms, enabled_mode] starting at offset 20.
    // Band 0 fields span 20..28, band 1 28..36, band 2 36..44, band 3 44..52.
    pub const DYNEQ_BAND_STRIDE: u8 = 8;
    pub const DYNEQ_B0_FREQ:     u8 = 20;
    pub const DYNEQ_B0_Q:        u8 = 21;
    pub const DYNEQ_B0_THRESH:   u8 = 22;
    pub const DYNEQ_B0_RATIO:    u8 = 23;
    pub const DYNEQ_B0_RANGE:    u8 = 24;
    pub const DYNEQ_B0_ATK:      u8 = 25;
    pub const DYNEQ_B0_REL:      u8 = 26;
    pub const DYNEQ_B0_MODE:     u8 = 27;
    pub const DYNEQ_MIX:         u8 = 19;
    // Mid/Side EQ (mastering Phase B). 4 mid bands + 4 side bands,
    // each with (freq, gain_db, q) — 3 fields × 4 bands × 2 chains.
    // Mid bands offset 20..32; Side bands offset 32..44.
    pub const MSEQ_M_B0_FREQ: u8 = 20;
    pub const MSEQ_M_B0_GAIN: u8 = 21;
    pub const MSEQ_M_B0_Q:    u8 = 22;
    pub const MSEQ_S_B0_FREQ: u8 = 32;
    pub const MSEQ_S_B0_GAIN: u8 = 33;
    pub const MSEQ_S_B0_Q:    u8 = 34;
    pub const MSEQ_MIX:       u8 = 19;
    // Multiband Compressor 3-band (mastering Phase C).
    // Layout: 2 crossover knobs (low / high) + per-band 5 knobs
    // (thresh, ratio, attack, release, makeup) for 3 bands.
    pub const MBC_XOVER_LOW:  u8 = 20;
    pub const MBC_XOVER_HIGH: u8 = 21;
    // Per-band fields, stride 6 starting at offset 22 / 28 / 34.
    // Band 0 low: 22..28, band 1 mid: 28..34, band 2 high: 34..40.
    pub const MBC_BAND_STRIDE: u8 = 6;
    pub const MBC_B0_THRESH:   u8 = 22;
    pub const MBC_B0_RATIO:    u8 = 23;
    pub const MBC_B0_ATK:      u8 = 24;
    pub const MBC_B0_REL:      u8 = 25;
    pub const MBC_B0_MAKEUP:   u8 = 26;
    pub const MBC_B0_ENABLED:  u8 = 27;
    pub const MBC_MIX:         u8 = 19;
    // Spectrum Analyzer (display-only).
    pub const SPECTRUM_MIX: u8 = 28;
    // Master Assistant — auto-loudness + spectral matching.
    pub const MA_MODE:         u8 = 20;
    pub const MA_TARGET_LUFS:  u8 = 21;
    pub const MA_TARGET_CURVE: u8 = 22;
    pub const MA_MIX:          u8 = 19;
    // Convolution Reverb — IR-driven sibling of the algorithmic Reverb.
    // Reuses the Reverb PID slots (per-slot gating in apply_fx_param_raw
    // keeps the two effects from stepping on each other) so toggling
    // ReverbKind on the per-plugin parity wiring is a 1:1 slot mapping.
    pub const CONVREV_IR_KIND: u8 = 16; // shares REVERB_TYPE slot
    pub const CONVREV_MIX:     u8 = 14; // shares REVERB_MIX slot
    // MultibandDucker (all < 100 so they round-trip the [f32;100] base_params store)
    pub const MBDUCK_CROSSOVER: u8 = 28;
    pub const MBDUCK_DEPTH:     u8 = 29;
    pub const MBDUCK_SENS:      u8 = 30;
    pub const MBDUCK_ATTACK:    u8 = 31;
    pub const MBDUCK_RELEASE:   u8 = 32;
    pub const MBDUCK_MIX:       u8 = 33;
    // VelvetDecorrelator
    pub const VELVET_WIDTH: u8 = 34;
    pub const VELVET_SIZE:  u8 = 35;
    pub const VELVET_MIX:   u8 = 36;
    // AnalogDrift
    pub const DRIFT_DEPTH: u8 = 37;
    pub const DRIFT_RATE:  u8 = 38;
    pub const DRIFT_MIX:   u8 = 39;
    // EarlyReflections
    pub const ER_SEED: u8 = 40;
    pub const ER_MIX:  u8 = 41;
    // MidSideWiden
    pub const MSW_FREQ:  u8 = 42;
    pub const MSW_WIDTH: u8 = 43;
    pub const MSW_MIX:   u8 = 44;
    // HaasSplit (PIDs scoped to the HaasSplit effect; reused low numbers like MSW)
    pub const HAAS_FREQ:  u8 = 45;
    pub const HAAS_DELAY: u8 = 46;
    pub const HAAS_MIX:   u8 = 47;
    // Retune (real-time pitch correction)
    pub const RETUNE_KEY:    u8 = 48; // 0..11 (C..B)
    pub const RETUNE_SCALE:  u8 = 49; // harmonizer ScaleType::all() index
    pub const RETUNE_SPEED:  u8 = 50; // 0..1 (1 = hard-tune)
    pub const RETUNE_AMOUNT: u8 = 51; // 0..1 correction depth
    pub const RETUNE_MIX:    u8 = 52;
    // Harmony (diatonic harmonizer)
    pub const HARM_KEY:    u8 = 53; // 0..11
    pub const HARM_SCALE:  u8 = 54; // harmonizer ScaleType::all() index
    pub const HARM_V1:     u8 = 55; // scale-degree offset, 0 = off (-7..+7)
    pub const HARM_V2:     u8 = 56;
    pub const HARM_V3:     u8 = 57;
    pub const HARM_SPREAD: u8 = 58; // 0..1 stereo spread
    pub const HARM_MIX:    u8 = 59; // harmony-voices level
    // Follow the session chord-track key (0/1); manual key/scale apply when off.
    pub const RETUNE_FOLLOW: u8 = 60;
    pub const HARM_FOLLOW:   u8 = 61;
    // TranceGate (tempo-synced stereo step VCA)
    pub const TG_DIV:    u8 = 62; // MagmaLfo division index 0..7 (step length)
    /// Right-channel step mask (16 bits packed in an f32; bit 15 = step 0).
    pub const TG_BITS_R: u8 = 63;
    pub const TG_SMOOTH: u8 = 64; // per-edge smoothing, ms
    pub const TG_SWING:  u8 = 65; // 0..1
    pub const TG_LINK:   u8 = 66; // >0.5 = right mirrors left
    pub const TG_MIX:    u8 = 67;
    /// Left-channel step mask. Two packed masks rather than 32 step flags: the
    /// flags would have eaten the slot's whole 100-parameter budget and, being
    /// stored sparsely, an OFF step would be dropped on save and come back ON.
    /// A mask is a single value, so a hand-drawn pattern round-trips.
    pub const TG_BITS_L: u8 = 68;

    // Multi-FX (a nested chain in one slot). Only its wet/dry lives here: the
    // sub-effects keep their own params inside the nested chain, not in this
    // slot's flat array. 19 is where several other effects put their mix.
    pub const MULTIFX_MIX: u8 = 19;
}

/// Trait for reading/writing effect parameters.
/// Implemented differently for global FX (ParameterSet) and per-sound drum FX (DrumFxUiState).
pub trait FxParamAccess {
    fn get(&self, param_id: u8) -> f32;
    fn set(&mut self, param_id: u8, value: f32);
    /// Live meter/analyzer readout for this slot (LUFS, peak band dB), if the
    /// effect publishes one. `None` for a plain effect.
    fn readout(&self) -> Option<f32> { None }
}

pub const TEMPO_DIVISIONS: [&str; 14] = [
    "1/1", "1/2", "1/4", "1/8", "1/16", "1/32",
    "1/2D", "1/4D", "1/8D", "1/16D",
    "1/2T", "1/4T", "1/8T", "1/16T",
];

// ── Shared knob helpers ─────────────────────────────────────────────

/// Value display format for knobs
#[derive(Clone, Copy)]
pub enum ValFmt {
    Pct,    // 0-100%
    Db,     // dB
    Ratio,  // N:1
    Ms,     // milliseconds (value is in seconds)
    Hz,     // Hertz
    Plain,  // raw numeric — used for Q, etc.
}

pub fn format_val(raw: f32, fmt: ValFmt) -> String {
    match fmt {
        ValFmt::Pct => format!("{:.0}%", raw * 100.0),
        ValFmt::Db => {
            if raw >= 0.0 { format!("{:.1} dB", raw) }
            else { format!("{:.1}dB", raw) }
        }
        ValFmt::Ratio => format!("{:.1}:1", raw),
        ValFmt::Ms => {
            let ms = raw * 1000.0;
            if ms < 1.0 { format!("{:.1}ms", ms) }
            else if ms < 100.0 { format!("{:.1}ms", ms) }
            else { format!("{:.0}ms", ms) }
        }
        ValFmt::Hz => {
            if raw >= 1000.0 { format!("{:.1}k", raw / 1000.0) }
            else { format!("{:.0}Hz", raw) }
        }
        ValFmt::Plain => format!("{:.2}", raw),
    }
}
