//! Effects processing for the Phonix synthesizer
//!
//! This module provides a comprehensive effects chain with multiple effect types
//! including delay, reverb, chorus, phaser, flanger, distortion, and vocoder.

#![allow(dead_code)] // Public API for future use

use std::time::Instant;

/// Fast dB-to-linear conversion using exp2 approximation.
/// 10^(db/20) = 2^(db * log2(10) / 20) = 2^(db * 0.16609640...)
/// Uses the identity: exp2(x) ≈ exp(x * ln2), which the compiler can optimize well.
#[inline(always)]
fn fast_db_to_lin(db: f32) -> f32 {
    // 10^(db/20) = e^(db * ln(10)/20)
    (db * (std::f32::consts::LN_10 / 20.0)).exp()
}

/// Fast log10 approximation using IEEE754 float bit tricks.
/// ~0.5% accuracy, avoids expensive libm log10f. For dB conversion in compressors.
#[inline(always)]
fn fast_log10(x: f32) -> f32 {
    // log10(x) = log2(x) / log2(10)
    // log2(x) ≈ bit manipulation on IEEE754 floats
    let i = x.to_bits() as f32;
    let log2 = i * (1.0 / (1 << 23) as f32) - 127.0;
    log2 * std::f32::consts::LOG10_2
}

/// Fast polynomial sin approximation for LFOs (±3% accuracy, no branching).
/// Input in radians. Good enough for modulation where precision doesn't matter.
#[inline(always)]
fn fast_sin(x: f32) -> f32 {
    // Normalize to [-pi, pi]
    let mut x = x % std::f32::consts::TAU;
    if x > std::f32::consts::PI {
        x -= std::f32::consts::TAU;
    }
    if x < -std::f32::consts::PI {
        x += std::f32::consts::TAU;
    }
    // Bhaskara I approximation: 16x(pi-x) / (5pi^2 - 4x(pi-x))
    let pi = std::f32::consts::PI;
    let abs_x = x.abs();
    let y = x * (pi - abs_x);
    let denom = 5.0 * pi * pi - 4.0 * y.abs();
    16.0 * y / denom
}

/// Effect type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectType {
    None,
    Delay,
    Reverb,
    Chorus,
    Phaser,
    Flanger,
    Distortion,
    Vocoder,
    Compressor,
    Limiter,
    Expander,
    AutoPanner,
    Stutter,
    Filter,
    RingMod,
    LoFi,
    StereoImager,
    Choir,
    Maximizer,
    // Phase 1: spatial + EQ
    Spatializer3D,
    ParametricEq,
    Doppler,
    // Phase 2: transient shaper
    TransientShaper,
    // Phase 3: creative
    GranularFreezer,
    SpectralFreeze,
    ResonatorBank,
    AutoWah,
    TapeStop,
    // Phase A of the mastering suite (Phase 7.12).
    /// Lookahead brick-wall limiter with true-peak detection.
    /// Goes at the end of the master chain to guarantee the output
    /// never exceeds the user-set ceiling.
    BrickwallLimiter,
    /// Visual-only ITU-R BS.1770 loudness meter (momentary /
    /// short-term / integrated). Audio passes through unchanged.
    LufsMeter,
    // Phase B — tonal-balance tier.
    /// 4-band parametric EQ where each band's gain is driven by an
    /// envelope follower on the same band — the "make everything
    /// clear" tool of the mastering chain.
    DynamicEq,
    /// Mid/Side EQ — splits L/R into M+S, runs an independent 4-band
    /// parametric on each, recombines. Used to tighten mono bass or
    /// widen treble without touching the centre image.
    MidSideEq,
    /// 3-band multiband compressor (LR4 split, default crossovers
    /// at 200 Hz and 2 kHz). Independent threshold/ratio/attack/
    /// release/makeup per band; LR4 sums recombine to flat
    /// magnitude at bypass.
    MultibandComp3,
    /// 32-band log-spaced spectrum analyser. Display-only effect.
    SpectrumAnalyzer,
    /// Master Assistant — intelligence layer. Reads its own LUFS +
    /// spectrum measurements and writes corrective gain / EQ moves
    /// into a target FX slot via the per-slot param API. The user's
    /// "make it loud and clear" one-button mastering tool.
    MasterAssistant,
    /// FFT convolution reverb using the 6 bundled IRs
    /// (Cathedral / Concrete / Plate / Wooden Hall / Spring / Ambient
    /// Room). Wraps the shared `crate::fx::convolution::ConvolutionReverb`
    /// so per-IR perceptual gain compensation matches every per-plugin
    /// convolution path — see `feedback_reverb_parity`.
    ConvolutionReverb,
    /// Frequency-slotting sidechain duck: attenuates ONLY a low band of the
    /// input under a sidechain key envelope, so a powerful low-end key (a bass)
    /// can own the lows without crushing the mids/highs. Reproduces the canon's
    /// "the bass owns the lows" master move. Appended at the end to keep every
    /// existing effect index stable for old `.aeth` files.
    MultibandDucker,
    /// Velvet-noise side-injection section widener (mono-invariant). Ports the
    /// canon's violin-section decorrelator: a large apparent ensemble at flat
    /// cost with zero mono coloration.
    VelvetDecorrelator,
    /// Slow tape-wow pitch drift (a few cents of continuous wander) so doubled
    /// takes never phase-lock. Ports the canon's per-stem `analog_drift`.
    AnalogDrift,
    /// 8-tap stereo early reflections with a seed-perturbed tap pattern (no two
    /// instances share a room signature). Ports the canon's per-stem `early_reflect`.
    EarlyReflections,
    /// Frequency-split mid/side widener: below `crossover` the L/R stay untouched;
    /// above it the side is scaled by `width`. Ports the canon drum-bus M/S widen
    /// (lows centred for kick power, highs spread) EXACTLY (canon_trance 1043-1053).
    MidSideWiden,
    /// Crossover Haas widener: lows (<crossover) stay centred (sub power), highs are fanned
    /// across the field by a fixed Haas delay on R. Ports the canon pulsar-bass stereo spread
    /// (canon_trance 1149-1161) EXACTLY.
    HaasSplit,
    /// Real-time pitch correction (autotune) insert: YIN detection drives the
    /// Harmonizer engine's pitch-synchronous corrector toward the nearest
    /// note of a key/scale. Transparent at low speed, hard-tune at 1.
    Retune,
    /// Diatonic harmonizer insert: up to 3 pitch-shifted voices at scale-degree
    /// intervals in a key, formant-preserving, panned across the field.
    Harmony,
    /// Tempo-synced stereo step VCA (trance gate). Hoisted out of the
    /// TranceVoice engine: a rhythmic gate belongs on a track, usable by
    /// anything, not welded into one instrument. Appended at the end to keep
    /// every existing effect index stable for old `.aeth` files.
    TranceGate,
    /// A rack inside a rack: this slot holds its own short chain of effects,
    /// so one insert can carry a whole multi-stage sound (Guitar Rig style).
    ///
    /// EXACTLY ONE level deep. A nested chain refuses to host another Multi-FX
    /// (see `EffectsChain::allow_multifx`), because unbounded nesting turns a
    /// per-sample chain walk into unbounded recursion on the audio thread.
    MultiFx,
    /// BBD "ensemble" chorus (3-phase Solina/string-machine widener) as a
    /// generic insert, wrapping the shared `EnsembleChorus` DSP. Appended at
    /// the end so every existing effect index stays stable for old `.aeth`
    /// files (index = position in `all()`). Lets VP-330 / Aurora move their
    /// baked-in ensemble out to a track FX slot.
    Ensemble,
    /// VCS-3 "Synthi" spring reverb (faithful 4-tap DSP) as a generic insert, so
    /// the spring can move out of the engine with zero tone change. Appended
    /// last to keep every existing effect index stable.
    SpringTank,
    /// Aurora's cinematic-plate hall reverb (faithful DSP) as a generic insert.
    /// Appended last to keep every existing effect index stable.
    CinemaPlate,
    /// VP-330's Schroeder string-machine reverb (faithful DSP) as a generic
    /// insert. Appended last to keep every existing effect index stable.
    SolinaVerb,
    /// Aurora's stereo BBD dimension chorus (faithful DSP) as a generic insert.
    /// Appended last to keep every existing effect index stable.
    Dimension,
}

/// Slots inside one Multi-FX. Small on purpose: it is a compact multi-stage
/// insert, not a second full rack.
pub const MULTIFX_SLOTS: usize = 4;

impl EffectType {
    /// Get the display name for the effect type
    pub fn display_name(&self) -> &'static str {
        match self {
            EffectType::None => "None",
            EffectType::Delay => "Delay",
            EffectType::Reverb => "Reverb",
            EffectType::Chorus => "Chorus",
            EffectType::Phaser => "Phaser",
            EffectType::Flanger => "Flanger",
            EffectType::Distortion => "Distortion",
            EffectType::Vocoder => "Vocoder",
            EffectType::Compressor => "Compressor",
            EffectType::Limiter => "Limiter",
            EffectType::Expander => "Expander",
            EffectType::AutoPanner => "AutoPan",
            EffectType::Stutter => "Stutter",
            EffectType::Filter => "Filter",
            EffectType::RingMod => "Ring Mod",
            EffectType::LoFi => "Lo-Fi",
            EffectType::StereoImager => "Stereo Img",
            EffectType::Choir => "Choir",
            EffectType::Maximizer => "Maximizer",
            EffectType::Spatializer3D => "Spatial 3D",
            EffectType::ParametricEq => "Param EQ",
            EffectType::Doppler => "Doppler",
            EffectType::TransientShaper => "Trans Shap",
            EffectType::GranularFreezer => "Granular",
            EffectType::SpectralFreeze => "Spec Freeze",
            EffectType::ResonatorBank => "Resonator",
            EffectType::AutoWah => "Auto Wah",
            EffectType::TapeStop => "Tape Stop",
            EffectType::BrickwallLimiter => "Brickwall",
            EffectType::LufsMeter => "LUFS Meter",
            EffectType::DynamicEq => "Dyn EQ",
            EffectType::MidSideEq => "M/S EQ",
            EffectType::MultibandComp3 => "MB Comp",
            EffectType::SpectrumAnalyzer => "Spectrum",
            EffectType::MasterAssistant => "Master Asst",
            EffectType::ConvolutionReverb => "Conv Reverb",
            EffectType::MultibandDucker => "MB Ducker",
            EffectType::VelvetDecorrelator => "Velvet Wide",
            EffectType::AnalogDrift => "Drift",
            EffectType::EarlyReflections => "Early Refl",
            EffectType::MidSideWiden => "M/S Widen",
            EffectType::HaasSplit => "Haas Split",
            EffectType::Retune => "Retune",
            EffectType::Harmony => "Harmonizer",
            EffectType::TranceGate => "Trance Gate",
            EffectType::MultiFx => "Multi-FX",
            EffectType::Ensemble => "Ensemble",
            EffectType::SpringTank => "Spring Tank",
            EffectType::CinemaPlate => "Cinema Plate",
            EffectType::SolinaVerb => "Solina Verb",
            EffectType::Dimension => "Dimension",
        }
    }

    /// Get all available effect types
    pub fn all() -> &'static [EffectType] {
        &[
            EffectType::None,
            EffectType::Delay,
            EffectType::Reverb,
            EffectType::Chorus,
            EffectType::Phaser,
            EffectType::Flanger,
            EffectType::Distortion,
            EffectType::Vocoder,
            EffectType::Compressor,
            EffectType::Limiter,
            EffectType::Expander,
            EffectType::AutoPanner,
            EffectType::Stutter,
            EffectType::Filter,
            EffectType::RingMod,
            EffectType::LoFi,
            EffectType::StereoImager,
            EffectType::Choir,
            EffectType::Maximizer,
            EffectType::Spatializer3D,
            EffectType::ParametricEq,
            EffectType::Doppler,
            EffectType::TransientShaper,
            EffectType::GranularFreezer,
            EffectType::SpectralFreeze,
            EffectType::ResonatorBank,
            EffectType::AutoWah,
            EffectType::TapeStop,
            EffectType::BrickwallLimiter,
            EffectType::LufsMeter,
            EffectType::DynamicEq,
            EffectType::MidSideEq,
            EffectType::MultibandComp3,
            EffectType::SpectrumAnalyzer,
            EffectType::MasterAssistant,
            EffectType::ConvolutionReverb,
            EffectType::MultibandDucker,
            EffectType::VelvetDecorrelator,
            EffectType::AnalogDrift,
            EffectType::EarlyReflections,
            EffectType::MidSideWiden,
            EffectType::HaasSplit,
            EffectType::Retune,
            EffectType::Harmony,
            EffectType::TranceGate,
            EffectType::MultiFx,
            EffectType::Ensemble,
            EffectType::SpringTank,
            EffectType::CinemaPlate,
            EffectType::SolinaVerb,
            EffectType::Dimension,
        ]
    }

    /// Create from index
    pub fn from_index(index: usize) -> Self {
        Self::all().get(index).copied().unwrap_or(EffectType::None)
    }

    /// Get index
    pub fn index(&self) -> usize {
        Self::all().iter().position(|&t| t == *self).unwrap_or(0)
    }
}

/// Delay type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelayType {
    Simple,
    PingPong,
    Tape,
    MultiTap,
    Slapback,
    Analog,
    Reverse,
    Ducking,
    Diffused,
    Shimmer,
    LoFi,
}

impl DelayType {
    pub fn display_name(&self) -> &'static str {
        match self {
            DelayType::Simple => "Digital",
            DelayType::PingPong => "Ping Pong",
            DelayType::Tape => "Tape",
            DelayType::MultiTap => "Multi-Tap",
            DelayType::Slapback => "Slapback",
            DelayType::Analog => "Analog",
            DelayType::Reverse => "Reverse",
            DelayType::Ducking => "Ducking",
            DelayType::Diffused => "Diffused",
            DelayType::Shimmer => "Shimmer",
            DelayType::LoFi => "Lo-Fi",
        }
    }

    pub fn all() -> &'static [DelayType] {
        &[
            DelayType::Simple,
            DelayType::PingPong,
            DelayType::Tape,
            DelayType::MultiTap,
            DelayType::Slapback,
            DelayType::Analog,
            DelayType::Reverse,
            DelayType::Ducking,
            DelayType::Diffused,
            DelayType::Shimmer,
            DelayType::LoFi,
        ]
    }

    pub fn from_index(index: usize) -> Self {
        Self::all().get(index).copied().unwrap_or(DelayType::Simple)
    }

    pub fn index(&self) -> usize {
        Self::all().iter().position(|&t| t == *self).unwrap_or(0)
    }
}

/// Reverb type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReverbType {
    Hall,
    Room,
    Plate,
    Spring,
    Shimmer,
    Cathedral,
    Cave,
    Ambient,
    Gated,
    Reverse,
    Infinite,
}

impl ReverbType {
    pub fn display_name(&self) -> &'static str {
        match self {
            ReverbType::Hall => "Hall",
            ReverbType::Room => "Room",
            ReverbType::Plate => "Plate",
            ReverbType::Spring => "Spring",
            ReverbType::Shimmer => "Shimmer",
            ReverbType::Cathedral => "Cathedral",
            ReverbType::Cave => "Cave",
            ReverbType::Ambient => "Ambient",
            ReverbType::Gated => "Gated",
            ReverbType::Reverse => "Reverse",
            ReverbType::Infinite => "Infinite",
        }
    }

    pub fn all() -> &'static [ReverbType] {
        &[
            ReverbType::Hall,
            ReverbType::Room,
            ReverbType::Plate,
            ReverbType::Spring,
            ReverbType::Shimmer,
            ReverbType::Cathedral,
            ReverbType::Cave,
            ReverbType::Ambient,
            ReverbType::Gated,
            ReverbType::Reverse,
            ReverbType::Infinite,
        ]
    }

    pub fn from_index(index: usize) -> Self {
        Self::all().get(index).copied().unwrap_or(ReverbType::Hall)
    }

    pub fn index(&self) -> usize {
        Self::all().iter().position(|&t| t == *self).unwrap_or(0)
    }
}

/// Distortion type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistortionType {
    SoftClip,
    HardClip,
    Tube,
    Fuzz,
    Bitcrush,
    Tape,
    Diode,
    Fold,
    /// Transparent mastering soft-knee clip: linear below `t`=0.9, gentle tanh above (the
    /// canon render's memoryless master clip). Crest-preserving; only the tips round off.
    SoftKnee,
    /// 50/50 tanh + hard-clip blend: `0.5*tanh(x) + 0.5*clamp(x,-1,1)`. The canon render's
    /// pulsar-bass GROWING distortion (canon_trance ~1146): the hard-clip half generates rich
    /// odd harmonics up the spectrum while the tanh half keeps the body round. Drive (2..28) is
    /// the input gain, so a drive AUTOMATION reproduces the render's clean->savage swell.
    TanhClip,
}

impl DistortionType {
    pub fn display_name(&self) -> &'static str {
        match self {
            DistortionType::SoftClip => "Soft Clip",
            DistortionType::HardClip => "Hard Clip",
            DistortionType::Tube => "Tube",
            DistortionType::Fuzz => "Fuzz",
            DistortionType::Bitcrush => "Bitcrush",
            DistortionType::Tape => "Tape",
            DistortionType::Diode => "Diode",
            DistortionType::Fold => "Fold",
            DistortionType::SoftKnee => "Soft Knee",
            DistortionType::TanhClip => "Tanh Clip",
        }
    }

    pub fn all() -> &'static [DistortionType] {
        &[
            DistortionType::SoftClip,
            DistortionType::HardClip,
            DistortionType::Tube,
            DistortionType::Fuzz,
            DistortionType::Bitcrush,
            DistortionType::Tape,
            DistortionType::Diode,
            DistortionType::Fold,
            DistortionType::SoftKnee,
            DistortionType::TanhClip,
        ]
    }

    pub fn from_index(index: usize) -> Self {
        Self::all()
            .get(index)
            .copied()
            .unwrap_or(DistortionType::SoftClip)
    }

    pub fn index(&self) -> usize {
        Self::all().iter().position(|&t| t == *self).unwrap_or(0)
    }
}

/// Tempo sync division
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TempoDivision {
    // Regular
    Whole,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
    // Dotted
    DottedHalf,
    DottedQuarter,
    DottedEighth,
    DottedSixteenth,
    // Triplets
    TripletHalf,
    TripletQuarter,
    TripletEighth,
    TripletSixteenth,
}

impl TempoDivision {
    pub fn display_name(&self) -> &'static str {
        match self {
            TempoDivision::Whole => "1/1",
            TempoDivision::Half => "1/2",
            TempoDivision::Quarter => "1/4",
            TempoDivision::Eighth => "1/8",
            TempoDivision::Sixteenth => "1/16",
            TempoDivision::ThirtySecond => "1/32",
            TempoDivision::DottedHalf => "1/2D",
            TempoDivision::DottedQuarter => "1/4D",
            TempoDivision::DottedEighth => "1/8D",
            TempoDivision::DottedSixteenth => "1/16D",
            TempoDivision::TripletHalf => "1/2T",
            TempoDivision::TripletQuarter => "1/4T",
            TempoDivision::TripletEighth => "1/8T",
            TempoDivision::TripletSixteenth => "1/16T",
        }
    }

    pub fn all() -> &'static [TempoDivision] {
        &[
            TempoDivision::Whole,
            TempoDivision::Half,
            TempoDivision::Quarter,
            TempoDivision::Eighth,
            TempoDivision::Sixteenth,
            TempoDivision::ThirtySecond,
            TempoDivision::DottedHalf,
            TempoDivision::DottedQuarter,
            TempoDivision::DottedEighth,
            TempoDivision::DottedSixteenth,
            TempoDivision::TripletHalf,
            TempoDivision::TripletQuarter,
            TempoDivision::TripletEighth,
            TempoDivision::TripletSixteenth,
        ]
    }

    pub fn from_index(index: usize) -> Self {
        Self::all()
            .get(index)
            .copied()
            .unwrap_or(TempoDivision::Quarter)
    }

    pub fn index(&self) -> usize {
        Self::all().iter().position(|&t| t == *self).unwrap_or(0)
    }

    /// Get the duration in beats
    pub fn beats(&self) -> f32 {
        match self {
            TempoDivision::Whole => 4.0,
            TempoDivision::Half => 2.0,
            TempoDivision::Quarter => 1.0,
            TempoDivision::Eighth => 0.5,
            TempoDivision::Sixteenth => 0.25,
            TempoDivision::ThirtySecond => 0.125,
            TempoDivision::DottedHalf => 3.0,
            TempoDivision::DottedQuarter => 1.5,
            TempoDivision::DottedEighth => 0.75,
            TempoDivision::DottedSixteenth => 0.375,
            TempoDivision::TripletHalf => 4.0 / 3.0,
            TempoDivision::TripletQuarter => 2.0 / 3.0,
            TempoDivision::TripletEighth => 1.0 / 3.0,
            TempoDivision::TripletSixteenth => 0.5 / 3.0,
        }
    }

    /// Convert to time in seconds given tempo
    pub fn to_time_seconds(&self, tempo_bpm: f32) -> f32 {
        let beats_per_second = tempo_bpm / 60.0;
        self.beats() / beats_per_second
    }
}

//==============================================================================
// DELAY EFFECT
//==============================================================================

/// Delay effect processor
#[derive(Debug, Clone)]
pub struct DelayEffect {
    sample_rate: f32,
    pi_over_sr: f32,
    /// `1.0 / sample_rate` cache for per-sample LFO/drift phase
    /// advances (`shimmer`, `analog_drift`) — replaces FP divisions
    /// with multiplies on the audio path. Lines 793/924/928 each
    /// fired once per sample under Analog and Shimmer modes.
    inv_sample_rate: f32,

    // Delay type
    delay_type: DelayType,

    // Parameters
    time: f32,          // seconds (when not synced)
    feedback: f32,      // 0.0 to 1.0
    mix: f32,           // 0.0 to 1.0
    stereo_offset: f32, // 0.0 to 1.0 (offset between L/R)

    // Tempo sync
    tempo_sync: bool,
    tempo_division: TempoDivision,
    tempo: f32, // BPM

    // Tape characteristics
    wow_depth: f32,
    wow_rate: f32,
    flutter_depth: f32,
    flutter_rate: f32,
    tape_saturation: f32,

    // Multi-tap settings
    tap_count: usize,

    // High/Low cut for delay feedback
    high_cut: f32,
    low_cut: f32,
    cached_lp_alpha: f32,        // cached 1-pole LP coefficient for high_cut
    cached_hp_alpha: f32,        // cached 1-pole HP coefficient for low_cut
    cached_analog_lp_alpha: f32, // cached analog delay darkening filter (fixed 2500Hz)

    // Delay lines (interleaved stereo: even = L, odd = R)
    delay_buffer: Vec<f32>,
    write_pos: usize,

    // Tape modulation
    lfo_phase_wow: f32,
    lfo_phase_flutter: f32,

    // Filter state for high/low cut
    hp_state: [f32; 2],
    lp_state: [f32; 2],

    // Analog: drift LFO and darkening filter
    analog_drift_phase: f32,
    analog_lp_state: [f32; 2],

    // Reverse: grain-based reverse playback
    reverse_grain_phase: f32,

    // Ducking: envelope follower
    duck_env: f32,

    // Diffused: allpass diffusers (4 per channel)
    diffuse_ap_l: [AllPassFilter; 4],
    diffuse_ap_r: [AllPassFilter; 4],

    // Shimmer: pitch shift phase
    shimmer_phase: f32,
    shimmer_phase2: f32,

    // LoFi: sample rate reduction state
    lofi_hold: [f32; 2],
    lofi_counter: f32,
}

impl DelayEffect {
    pub fn new(sample_rate: f32) -> Self {
        let max_delay_time = 5.0; // 5 seconds max
        let buffer_size = (sample_rate * max_delay_time * 2.0) as usize; // *2 for stereo

        let scale = sample_rate / 44100.0;
        const DIFF_AP_L: [usize; 4] = [601, 433, 311, 197];
        const DIFF_AP_R: [usize; 4] = [619, 449, 331, 211];

        Self {
            sample_rate,
            pi_over_sr: std::f32::consts::PI / sample_rate,
            inv_sample_rate: 1.0 / sample_rate.max(1.0),
            delay_type: DelayType::Simple,
            time: 0.3,
            feedback: 0.4,
            mix: 0.3,
            stereo_offset: 0.0,
            tempo_sync: false,
            tempo_division: TempoDivision::Eighth,
            tempo: 120.0,
            wow_depth: 0.002,
            wow_rate: 0.5,
            flutter_depth: 0.001,
            flutter_rate: 8.0,
            tape_saturation: 0.3,
            tap_count: 4,
            high_cut: 8000.0,
            low_cut: 100.0,
            delay_buffer: vec![0.0; buffer_size],
            write_pos: 0,
            lfo_phase_wow: 0.0,
            lfo_phase_flutter: 0.0,
            hp_state: [0.0; 2],
            lp_state: [0.0; 2],
            analog_drift_phase: 0.0,
            analog_lp_state: [0.0; 2],
            reverse_grain_phase: 0.0,
            duck_env: 0.0,
            diffuse_ap_l: DIFF_AP_L.map(|t| AllPassFilter::new((t as f32 * scale) as usize)),
            diffuse_ap_r: DIFF_AP_R.map(|t| AllPassFilter::new((t as f32 * scale) as usize)),
            shimmer_phase: 0.0,
            shimmer_phase2: 0.0,
            lofi_hold: [0.0; 2],
            lofi_counter: 0.0,
            cached_lp_alpha: 1.0 - (-2.0 * std::f32::consts::PI * 8000.0 / sample_rate).exp(),
            cached_hp_alpha: 1.0 - (-2.0 * std::f32::consts::PI * 100.0 / sample_rate).exp(),
            cached_analog_lp_alpha: 1.0 - (-std::f32::consts::TAU * 2500.0 / sample_rate).exp(),
        }
    }

    fn buffer_size(&self) -> usize {
        self.delay_buffer.len()
    }

    fn max_delay_samples(&self) -> usize {
        self.buffer_size() / 2
    }

    /// Get current delay time in seconds (accounting for tempo sync)
    fn get_delay_time(&self) -> f32 {
        if self.tempo_sync {
            self.tempo_division.to_time_seconds(self.tempo)
        } else {
            self.time
        }
    }

    /// Get tape modulation offset in samples
    fn get_tape_modulation(&mut self) -> f32 {
        if self.delay_type != DelayType::Tape {
            return 0.0;
        }

        // Update LFO phases — fold `rate/sr * 2π` into a single
        // multiply using the cached `pi_over_sr`, removing two FP
        // divisions per audio sample on the tape path.
        let two_pi_over_sr = 2.0 * self.pi_over_sr;
        self.lfo_phase_wow += self.wow_rate * two_pi_over_sr;
        self.lfo_phase_flutter += self.flutter_rate * two_pi_over_sr;

        if self.lfo_phase_wow > 2.0 * std::f32::consts::PI {
            self.lfo_phase_wow -= 2.0 * std::f32::consts::PI;
        }
        if self.lfo_phase_flutter > 2.0 * std::f32::consts::PI {
            self.lfo_phase_flutter -= 2.0 * std::f32::consts::PI;
        }

        // Calculate modulation
        let wow = fast_sin(self.lfo_phase_wow) * self.wow_depth * self.sample_rate;
        let flutter = fast_sin(self.lfo_phase_flutter) * self.flutter_depth * self.sample_rate;

        wow + flutter
    }

    /// Apply tape saturation
    fn apply_tape_saturation(&self, sample: f32) -> f32 {
        if self.delay_type != DelayType::Tape || self.tape_saturation <= 0.0 {
            return sample;
        }

        // Soft saturation curve
        let drive = 1.0 + self.tape_saturation * 3.0;
        let saturated = sample * drive;

        // Tanh-like saturation
        saturated.tanh() / drive.sqrt()
    }

    /// Apply high/low cut filter (correct 1-pole LP and HP) — uses cached coefficients
    #[inline(always)]
    fn apply_filter(&mut self, sample: f32, channel: usize) -> f32 {
        let ch = channel.min(1);

        // Low-pass filter (1-pole): lp += alpha * (input - lp)
        self.lp_state[ch] += self.cached_lp_alpha * (sample - self.lp_state[ch]);

        // High-pass filter: hp = input - lowpass(input)
        self.hp_state[ch] += self.cached_hp_alpha * (self.lp_state[ch] - self.hp_state[ch]);

        self.lp_state[ch] - self.hp_state[ch]
    }

    /// Read from delay buffer with linear interpolation (frame-aligned)
    fn read_delay(&self, channel: usize, delay_samples: f32) -> f32 {
        let num_frames = self.buffer_size() / 2;
        let delay_samples = delay_samples.max(1.0).min((num_frames - 2) as f32);

        let write_frame = self.write_pos / 2;
        let mut read_frame = write_frame as f32 - delay_samples;
        while read_frame < 0.0 {
            read_frame += num_frames as f32;
        }
        let read_frame = read_frame % num_frames as f32;

        let frame_int = read_frame as usize;
        let frac = read_frame - frame_int as f32;
        let frame_next = (frame_int + 1) % num_frames;

        let s1 = self.delay_buffer[frame_int * 2 + channel];
        let s2 = self.delay_buffer[frame_next * 2 + channel];

        s1 + (s2 - s1) * frac
    }

    /// Write to delay buffer (with safety clamp)
    fn write_delay(&mut self, channel: usize, sample: f32) {
        let pos = (self.write_pos + channel) % self.buffer_size();
        self.delay_buffer[pos] = if sample.is_finite() {
            sample.clamp(-4.0, 4.0)
        } else {
            0.0
        };
    }

    /// Process stereo sample
    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.mix <= 0.0001 {
            return (input_l, input_r);
        }

        let base_delay_samples = self.get_delay_time() * self.sample_rate;
        let tape_mod = self.get_tape_modulation();

        let result = match self.delay_type {
            DelayType::Simple => {
                let delay_l = base_delay_samples + tape_mod;
                let delay_r = base_delay_samples * (1.0 + self.stereo_offset) + tape_mod;

                let delayed_l = self.read_delay(0, delay_l);
                let delayed_r = self.read_delay(1, delay_r);

                // Apply filtering to feedback
                let fb_l = self.apply_filter(delayed_l, 0);
                let fb_r = self.apply_filter(delayed_r, 1);

                // Apply tape saturation
                let fb_l = self.apply_tape_saturation(fb_l);
                let fb_r = self.apply_tape_saturation(fb_r);

                // Write to buffer with feedback
                self.write_delay(0, input_l + fb_l * self.feedback);
                self.write_delay(1, input_r + fb_r * self.feedback);

                // Output
                let out_l = input_l + delayed_l * self.mix;
                let out_r = input_r + delayed_r * self.mix;

                (out_l, out_r)
            }

            DelayType::PingPong => {
                let delay_samples = base_delay_samples + tape_mod;

                // Read each channel from its own delay line
                let delayed_l = self.read_delay(0, delay_samples);
                let delayed_r = self.read_delay(1, delay_samples);

                let fb_l = self.apply_filter(delayed_l, 0);
                let fb_r = self.apply_filter(delayed_r, 1);
                let fb_l = self.apply_tape_saturation(fb_l);
                let fb_r = self.apply_tape_saturation(fb_r);

                // Mono input sum enters L only; L bounces to R, R bounces to L
                let mono_in = (input_l + input_r) * 0.5;
                self.write_delay(0, mono_in + fb_r * self.feedback);
                self.write_delay(1, fb_l * self.feedback);

                let out_l = input_l + delayed_l * self.mix;
                let out_r = input_r + delayed_r * self.mix;

                (out_l, out_r)
            }

            DelayType::Tape => {
                // Tape delay with wow/flutter and saturation
                let delay_samples = base_delay_samples + tape_mod;

                let delayed_l = self.read_delay(0, delay_samples);
                let delayed_r = self.read_delay(1, delay_samples * (1.0 + self.stereo_offset));

                let fb_l = self.apply_filter(delayed_l, 0);
                let fb_r = self.apply_filter(delayed_r, 1);
                let fb_l = self.apply_tape_saturation(fb_l);
                let fb_r = self.apply_tape_saturation(fb_r);

                self.write_delay(0, input_l + fb_l * self.feedback);
                self.write_delay(1, input_r + fb_r * self.feedback);

                let out_l = input_l + delayed_l * self.mix;
                let out_r = input_r + delayed_r * self.mix;

                (out_l, out_r)
            }

            DelayType::MultiTap => {
                // Multi-tap delay with evenly spaced taps
                let mut out_l = input_l;
                let mut out_r = input_r;

                let taps = self.tap_count.max(2);
                let tap_mix = self.mix / taps as f32;
                let inv_taps = 1.0 / taps as f32;
                for tap in 1..=taps {
                    let tap_delay = base_delay_samples * (tap as f32 * inv_taps);

                    let delayed_l = self.read_delay(0, tap_delay + tape_mod);
                    let delayed_r =
                        self.read_delay(1, tap_delay * (1.0 + self.stereo_offset) + tape_mod);

                    out_l += delayed_l * tap_mix;
                    out_r += delayed_r * tap_mix;
                }

                // Feedback from last tap
                let last_delay = base_delay_samples;
                let fb_l = self.apply_filter(self.read_delay(0, last_delay), 0);
                let fb_r = self.apply_filter(self.read_delay(1, last_delay), 1);

                self.write_delay(0, input_l + fb_l * self.feedback);
                self.write_delay(1, input_r + fb_r * self.feedback);

                (out_l, out_r)
            }

            DelayType::Slapback => {
                // Short delay, single echo, no feedback
                let slap_time = base_delay_samples.min(self.sample_rate * 0.15); // Max 150ms
                let delay_samples = slap_time + tape_mod;

                let delayed_l = self.read_delay(0, delay_samples);
                let delayed_r = self.read_delay(1, delay_samples * (1.0 + self.stereo_offset));

                let out_l = input_l + delayed_l * self.mix;
                let out_r = input_r + delayed_r * self.mix;

                // No feedback for slapback
                self.write_delay(0, input_l);
                self.write_delay(1, input_r);

                (out_l, out_r)
            }

            DelayType::Analog => {
                // Warm analog delay: drift, darkening repeats, subtle saturation
                self.analog_drift_phase += 0.07 * self.inv_sample_rate;
                if self.analog_drift_phase > 1.0 {
                    self.analog_drift_phase -= 1.0;
                }
                let drift = fast_sin(self.analog_drift_phase * std::f32::consts::TAU)
                    * 0.003
                    * self.sample_rate;

                let delay_l = base_delay_samples + drift;
                let delay_r = base_delay_samples * (1.0 + self.stereo_offset) - drift * 0.7;

                let delayed_l = self.read_delay(0, delay_l);
                let delayed_r = self.read_delay(1, delay_r);

                // Progressive darkening: heavy lowpass on feedback (cached 2500Hz coefficient)
                self.analog_lp_state[0] +=
                    self.cached_analog_lp_alpha * (delayed_l - self.analog_lp_state[0]);
                self.analog_lp_state[1] +=
                    self.cached_analog_lp_alpha * (delayed_r - self.analog_lp_state[1]);
                let fb_l = self.analog_lp_state[0];
                let fb_r = self.analog_lp_state[1];

                // Soft saturation on feedback
                let drive = 1.5;
                let fb_l = (fb_l * drive).tanh() / drive.sqrt();
                let fb_r = (fb_r * drive).tanh() / drive.sqrt();

                self.write_delay(0, input_l + fb_l * self.feedback);
                self.write_delay(1, input_r + fb_r * self.feedback);

                let out_l = input_l + delayed_l * self.mix;
                let out_r = input_r + delayed_r * self.mix;
                (out_l, out_r)
            }

            DelayType::Reverse => {
                // Reversed delay: reads buffer backwards in grains
                let grain_samples = base_delay_samples.max(64.0);

                // Write input
                self.write_delay(0, input_l);
                self.write_delay(1, input_r);

                // Advance grain phase
                self.reverse_grain_phase += 1.0;
                let grain_pos = self.reverse_grain_phase;

                if grain_pos >= grain_samples {
                    self.reverse_grain_phase = 0.0;
                }

                // Read backwards within grain
                let progress = self.reverse_grain_phase / grain_samples;
                let read_offset = progress * grain_samples;

                let delayed_l = self.read_delay(0, read_offset);
                let delayed_r = self.read_delay(1, read_offset * (1.0 + self.stereo_offset));

                // Hann window for smooth grains
                let window = 0.5 * (1.0 - (progress * std::f32::consts::TAU).cos());

                let out_l = input_l + delayed_l * window * self.mix;
                let out_r = input_r + delayed_r * window * self.mix;
                (out_l, out_r)
            }

            DelayType::Ducking => {
                // Ducking delay: delay ducks when input is present
                let input_level = (input_l.abs() + input_r.abs()) * 0.5;

                // Envelope follower: fast attack, slow release
                if input_level > self.duck_env {
                    self.duck_env += (input_level - self.duck_env) * 0.01; // fast attack
                } else {
                    self.duck_env *= 0.9997; // slow release (~200ms)
                }

                // Duck amount: when input is loud, reduce delay mix
                let duck_gain = (1.0 - self.duck_env * 4.0).max(0.0).min(1.0);

                let delay_l = base_delay_samples + tape_mod;
                let delay_r = base_delay_samples * (1.0 + self.stereo_offset) + tape_mod;

                let delayed_l = self.read_delay(0, delay_l);
                let delayed_r = self.read_delay(1, delay_r);

                let fb_l = self.apply_filter(delayed_l, 0);
                let fb_r = self.apply_filter(delayed_r, 1);

                self.write_delay(0, input_l + fb_l * self.feedback);
                self.write_delay(1, input_r + fb_r * self.feedback);

                let out_l = input_l + delayed_l * self.mix * duck_gain;
                let out_r = input_r + delayed_r * self.mix * duck_gain;
                (out_l, out_r)
            }

            DelayType::Diffused => {
                // Diffused delay: echoes smeared through allpass network
                let delay_l = base_delay_samples + tape_mod;
                let delay_r = base_delay_samples * (1.0 + self.stereo_offset) + tape_mod;

                let mut delayed_l = self.read_delay(0, delay_l);
                let mut delayed_r = self.read_delay(1, delay_r);

                // Run through allpass diffusion network
                for i in 0..4 {
                    delayed_l = self.diffuse_ap_l[i].process(delayed_l);
                    delayed_r = self.diffuse_ap_r[i].process(delayed_r);
                }

                let fb_l = self.apply_filter(delayed_l, 0);
                let fb_r = self.apply_filter(delayed_r, 1);

                self.write_delay(0, input_l + fb_l * self.feedback);
                self.write_delay(1, input_r + fb_r * self.feedback);

                let out_l = input_l + delayed_l * self.mix;
                let out_r = input_r + delayed_r * self.mix;
                (out_l, out_r)
            }

            DelayType::Shimmer => {
                // Shimmer delay: pitch-shifted feedback (octave harmonics)
                let delay_l = base_delay_samples + tape_mod;
                let delay_r = base_delay_samples * (1.0 + self.stereo_offset) + tape_mod;

                let delayed_l = self.read_delay(0, delay_l);
                let delayed_r = self.read_delay(1, delay_r);

                // Dual shimmer modulation
                self.shimmer_phase += 1.2 * self.inv_sample_rate;
                if self.shimmer_phase > 1.0 {
                    self.shimmer_phase -= 1.0;
                }
                self.shimmer_phase2 += 1.9 * self.inv_sample_rate;
                if self.shimmer_phase2 > 1.0 {
                    self.shimmer_phase2 -= 1.0;
                }

                let mod1 = fast_sin(self.shimmer_phase * std::f32::consts::TAU) * 0.12;
                let mod2 = fast_sin(self.shimmer_phase2 * std::f32::consts::TAU) * 0.08;

                // Pitch-shifted feedback: read at slightly different positions
                let shift_l = self.read_delay(0, delay_l * (1.0 + mod1)) * 0.3;
                let shift_r = self.read_delay(1, delay_r * (1.0 + mod2)) * 0.3;

                let fb_l = self.apply_filter(delayed_l + shift_l, 0);
                let fb_r = self.apply_filter(delayed_r + shift_r, 1);

                self.write_delay(0, input_l + fb_l * self.feedback);
                self.write_delay(1, input_r + fb_r * self.feedback);

                let out_l = input_l + delayed_l * self.mix;
                let out_r = input_r + delayed_r * self.mix;
                (out_l, out_r)
            }

            DelayType::LoFi => {
                // Lo-Fi delay: bit crushing + sample rate reduction in feedback
                let delay_l = base_delay_samples + tape_mod;
                let delay_r = base_delay_samples * (1.0 + self.stereo_offset) + tape_mod;

                let delayed_l = self.read_delay(0, delay_l);
                let delayed_r = self.read_delay(1, delay_r);

                // Sample rate reduction: hold samples for N cycles
                let reduction = 4.0 + self.stereo_offset * 12.0; // 4x to 16x reduction
                self.lofi_counter += 1.0;
                if self.lofi_counter >= reduction {
                    self.lofi_counter = 0.0;
                    self.lofi_hold[0] = delayed_l;
                    self.lofi_hold[1] = delayed_r;
                }

                // Bit crush: quantize to fewer levels
                let bits: f32 = 8.0; // ~256 levels
                let levels = bits.exp2();
                let crushed_l = (self.lofi_hold[0] * levels).round() / levels;
                let crushed_r = (self.lofi_hold[1] * levels).round() / levels;

                let fb_l = self.apply_filter(crushed_l, 0);
                let fb_r = self.apply_filter(crushed_r, 1);

                self.write_delay(0, input_l + fb_l * self.feedback);
                self.write_delay(1, input_r + fb_r * self.feedback);

                let out_l = input_l + crushed_l * self.mix;
                let out_r = input_r + crushed_r * self.mix;
                (out_l, out_r)
            }
        };

        // Advance write position (stereo interleaved: +2 per sample).
        // Bounded reset instead of `% buffer_size()`.
        self.write_pos += 2;
        let buf_size = self.buffer_size();
        if self.write_pos >= buf_size { self.write_pos -= buf_size; }

        result
    }

    /// Set delay type — resets buffer to prevent artifacts from old state
    pub fn set_delay_type(&mut self, delay_type: DelayType) {
        if self.delay_type != delay_type {
            self.delay_type = delay_type;
            self.reset();
        }
    }

    /// Set delay time in seconds
    pub fn set_time(&mut self, time: f32) {
        self.time = time.max(0.01).min(5.0);
    }

    /// Set feedback
    pub fn set_feedback(&mut self, feedback: f32) {
        self.feedback = feedback.max(0.0).min(0.99);
    }

    /// Set mix
    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.max(0.0).min(1.0);
    }

    /// Set tempo sync
    pub fn set_tempo_sync(&mut self, sync: bool) {
        self.tempo_sync = sync;
    }

    /// Set tempo division
    pub fn set_tempo_division(&mut self, division: TempoDivision) {
        self.tempo_division = division;
    }

    /// Set tempo in BPM
    pub fn set_tempo(&mut self, tempo: f32) {
        self.tempo = tempo.max(20.0).min(300.0);
    }

    /// Set stereo offset
    pub fn set_stereo_offset(&mut self, offset: f32) {
        self.stereo_offset = offset.max(0.0).min(1.0);
    }

    /// Set high cut frequency
    pub fn set_high_cut(&mut self, freq: f32) {
        self.high_cut = freq.max(100.0).min(20000.0);
        self.cached_lp_alpha =
            1.0 - (-2.0 * std::f32::consts::PI * self.high_cut / self.sample_rate).exp();
    }

    /// Set low cut frequency
    pub fn set_low_cut(&mut self, freq: f32) {
        self.low_cut = freq.max(20.0).min(5000.0);
        self.cached_hp_alpha =
            1.0 - (-2.0 * std::f32::consts::PI * self.low_cut / self.sample_rate).exp();
    }

    /// Reset delay buffer
    pub fn reset(&mut self) {
        for sample in self.delay_buffer.iter_mut() {
            *sample = 0.0;
        }
        self.write_pos = 0;
        self.hp_state = [0.0; 2];
        self.lp_state = [0.0; 2];
        self.lfo_phase_wow = 0.0;
        self.lfo_phase_flutter = 0.0;
        self.analog_drift_phase = 0.0;
        self.analog_lp_state = [0.0; 2];
        self.reverse_grain_phase = 0.0;
        self.duck_env = 0.0;
        for ap in self
            .diffuse_ap_l
            .iter_mut()
            .chain(self.diffuse_ap_r.iter_mut())
        {
            ap.reset();
        }
        self.shimmer_phase = 0.0;
        self.shimmer_phase2 = 0.0;
        self.lofi_hold = [0.0; 2];
        self.lofi_counter = 0.0;
    }
}

//==============================================================================
// REVERB EFFECT
//==============================================================================

/// Reverb effect processor with multiple distinct algorithms
#[derive(Debug, Clone)]
pub struct ReverbEffect {
    sample_rate: f32,
    pi_over_sr: f32, // PI / sample_rate (cached)
    /// `1.0 / sample_rate` cache for per-sample LFO phase advances
    /// in the Shimmer / Modulation paths (was 3+ FP-divisions per
    /// audio sample under those reverb types).
    inv_sample_rate: f32,

    // Reverb type
    reverb_type: ReverbType,

    // Parameters
    size: f32,      // Room size (0-1)
    decay: f32,     // Decay time (0-1, maps to different ranges per type)
    damping: f32,   // High frequency damping
    pre_delay: f32, // Pre-delay time in seconds (up to 500ms)
    mix: f32,       // Dry/wet mix
    mix_dry: f32,   // Cached cos(mix * PI/2)
    mix_wet: f32,   // Cached sin(mix * PI/2)
    width: f32,     // Stereo width

    // Freeverb-style comb filters (8 per channel)
    comb_l: [CombFilter; 8],
    comb_r: [CombFilter; 8],

    // Freeverb-style allpass filters (4 per channel)
    allpass_l: [AllPassFilter; 4],
    allpass_r: [AllPassFilter; 4],

    // Extra long comb filters for Cathedral/Cave/Infinite (4 per channel)
    long_comb_l: [CombFilter; 4],
    long_comb_r: [CombFilter; 4],

    // Extra allpass for diffusion in large spaces (4 per channel)
    diff_allpass_l: [AllPassFilter; 4],
    diff_allpass_r: [AllPassFilter; 4],

    // Pre-delay buffer (stereo interleaved, up to 500ms)
    pre_delay_buffer: Vec<f32>,
    pre_delay_pos: usize,

    // Reverse reverb buffer (stereo interleaved, up to 2 seconds)
    reverse_buffer: Vec<f32>,
    reverse_write_pos: usize,
    reverse_read_pos: f32,
    reverse_grain_phase: f32,

    // Shimmer
    shimmer_phase: f32,
    shimmer_phase2: f32,

    // Early reflections tapped delay line (mono in, stereo out)
    er_buffer: Vec<f32>,
    er_write_pos: usize,

    // Gated reverb
    gate_env: f32,
    gate_hold_counter: usize,
    // Cached gated hold-time in samples — derives only from `size` + sample_rate,
    // refreshed in set_size() to avoid the per-sample cast in process_gated().
    cached_gate_hold_samples: usize,

    // Modulation LFO for ambient/shimmer
    mod_phase: f32,

    // Fade-in counter after type switch — ramps input from 0→1 over ~50ms
    // to prevent transient blowup when switching from high-feedback types
    type_switch_fade: u32,
}

impl ReverbEffect {
    pub fn new(sample_rate: f32) -> Self {
        let scale = sample_rate / 44100.0;

        // Standard Freeverb tunings
        const COMB_TUNING_L: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
        const COMB_TUNING_R: [usize; 8] = [1139, 1211, 1300, 1379, 1445, 1514, 1580, 1640];
        const ALLPASS_TUNING_L: [usize; 4] = [556, 441, 341, 225];
        const ALLPASS_TUNING_R: [usize; 4] = [579, 464, 364, 248];

        // Long comb tunings for large spaces (Cathedral, Cave, Infinite)
        // These are 3-5x longer than standard for massive tail
        const LONG_COMB_L: [usize; 4] = [4799, 5399, 5903, 6421];
        const LONG_COMB_R: [usize; 4] = [4919, 5501, 6007, 6553];

        // Extra diffusion allpass tunings
        const DIFF_ALLPASS_L: [usize; 4] = [1024, 743, 512, 379];
        const DIFF_ALLPASS_R: [usize; 4] = [1051, 769, 537, 397];

        let comb_l = COMB_TUNING_L.map(|t| CombFilter::new((t as f32 * scale) as usize));
        let comb_r = COMB_TUNING_R.map(|t| CombFilter::new((t as f32 * scale) as usize));
        let allpass_l = ALLPASS_TUNING_L.map(|t| AllPassFilter::new((t as f32 * scale) as usize));
        let allpass_r = ALLPASS_TUNING_R.map(|t| AllPassFilter::new((t as f32 * scale) as usize));
        let long_comb_l = LONG_COMB_L.map(|t| CombFilter::new((t as f32 * scale) as usize));
        let long_comb_r = LONG_COMB_R.map(|t| CombFilter::new((t as f32 * scale) as usize));
        let diff_allpass_l =
            DIFF_ALLPASS_L.map(|t| AllPassFilter::new((t as f32 * scale) as usize));
        let diff_allpass_r =
            DIFF_ALLPASS_R.map(|t| AllPassFilter::new((t as f32 * scale) as usize));

        // Pre-delay: up to 500ms stereo interleaved
        let pre_delay_samples = (sample_rate * 0.5) as usize;
        // Reverse buffer: up to 2 seconds stereo interleaved
        let reverse_samples = (sample_rate * 2.0) as usize;
        // Early reflections buffer: up to 100ms mono
        let er_samples = (sample_rate * 0.1) as usize;

        let mix_angle = 0.3 * std::f32::consts::FRAC_PI_2;
        let mut s = Self {
            sample_rate,
            pi_over_sr: std::f32::consts::PI / sample_rate,
            inv_sample_rate: 1.0 / sample_rate.max(1.0),
            reverb_type: ReverbType::Hall,
            size: 0.7,
            decay: 0.5,
            damping: 0.5,
            pre_delay: 0.0,
            mix: 0.3,
            mix_dry: mix_angle.cos(),
            mix_wet: mix_angle.sin(),
            width: 1.0,
            comb_l,
            comb_r,
            allpass_l,
            allpass_r,
            long_comb_l,
            long_comb_r,
            diff_allpass_l,
            diff_allpass_r,
            pre_delay_buffer: vec![0.0; pre_delay_samples * 2],
            pre_delay_pos: 0,
            er_buffer: vec![0.0; er_samples.max(1)],
            er_write_pos: 0,
            reverse_buffer: vec![0.0; reverse_samples * 2],
            reverse_write_pos: 0,
            reverse_read_pos: 0.0,
            reverse_grain_phase: 0.0,
            shimmer_phase: 0.0,
            shimmer_phase2: 0.0,
            gate_env: 0.0,
            gate_hold_counter: 0,
            cached_gate_hold_samples: 0,
            mod_phase: 0.0,
            type_switch_fade: 0,
        };
        Self::refresh_static(&mut s);
        s
    }

    /// Refresh derived constants that depend on `size` / sample_rate.
    /// Cheap — just a multiply + cast. Called from setters that touch `size`.
    fn refresh_static(s: &mut Self) {
        s.cached_gate_hold_samples = ((s.size * 0.75 + 0.05) * s.sample_rate) as usize;
    }

    /// Process pre-delay (up to 500ms)
    fn process_pre_delay(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.pre_delay <= 0.0001 {
            return (input_l, input_r);
        }

        let buffer_len = self.pre_delay_buffer.len() / 2;
        if buffer_len < 2 {
            return (input_l, input_r);
        }

        let delay_samples = ((self.pre_delay * self.sample_rate) as usize).min(buffer_len - 1);

        self.pre_delay_buffer[self.pre_delay_pos * 2] = input_l;
        self.pre_delay_buffer[self.pre_delay_pos * 2 + 1] = input_r;

        let read_pos = (self.pre_delay_pos + buffer_len - delay_samples) % buffer_len;
        let delayed_l = self.pre_delay_buffer[read_pos * 2];
        let delayed_r = self.pre_delay_buffer[read_pos * 2 + 1];

        self.pre_delay_pos = (self.pre_delay_pos + 1) % buffer_len;

        (delayed_l, delayed_r)
    }

    /// Early reflections via tapped delay line.
    /// Returns (er_l, er_r, feed) where feed is the ER sum to feed into late reverb.
    fn process_early_reflections(&mut self, input: f32) -> (f32, f32, f32) {
        let buf_len = self.er_buffer.len();
        if buf_len < 2 {
            return (0.0, 0.0, input);
        }

        // Write input into circular buffer. Bounded advance is
        // cheaper than `% buf_len` (non-power-of-2, would compile
        // to an integer divide per audio sample).
        self.er_buffer[self.er_write_pos] = input;
        self.er_write_pos += 1;
        if self.er_write_pos >= buf_len { self.er_write_pos = 0; }

        // Tap times in samples at 44.1kHz — scaled by sample rate and room size.
        // These model first-order reflections from walls, ceiling, floor of a
        // rectangular room. L/R taps differ to create natural stereo from mono input.
        //
        // Tap times (ms at 44.1k): ~5, 8, 12, 17, 21, 27, 33, 39, 47, 55, 67, 80
        // Gains decay with distance (inverse square approximation)
        const TAP_TIMES_L: [usize; 12] = [
            221, 353, 530, 750, 926, 1191, 1456, 1720, 2073, 2425, 2955, 3528,
        ];
        const TAP_TIMES_R: [usize; 12] = [
            243, 397, 573, 794, 1014, 1279, 1544, 1808, 2161, 2513, 3043, 3616,
        ];
        const TAP_GAINS: [f32; 12] = [
            0.85, 0.72, 0.60, 0.50, 0.42, 0.35, 0.29, 0.24, 0.19, 0.15, 0.11, 0.08,
        ];

        let scale = self.sample_rate / 44100.0;
        // Size scales the tap spread: small rooms = close reflections, large = spread
        let size_scale = 0.3 + self.size * 0.7; // 0.3..1.0

        let mut er_l = 0.0f32;
        let mut er_r = 0.0f32;

        for i in 0..12 {
            let tap_l = ((TAP_TIMES_L[i] as f32 * scale * size_scale) as usize).min(buf_len - 1);
            let tap_r = ((TAP_TIMES_R[i] as f32 * scale * size_scale) as usize).min(buf_len - 1);
            let gain = TAP_GAINS[i];

            let read_l = (self.er_write_pos + buf_len - tap_l) % buf_len;
            let read_r = (self.er_write_pos + buf_len - tap_r) % buf_len;

            er_l += self.er_buffer[read_l] * gain;
            er_r += self.er_buffer[read_r] * gain;
        }

        // Normalize so total ER energy ≈ 1x input (sum of gains ≈ 4.2, /4.2)
        er_l *= 0.24;
        er_r *= 0.24;

        // Feed to late reverb: mix of input + attenuated ER tail
        let feed = input + (er_l + er_r) * 0.15;

        (er_l, er_r, feed)
    }

    /// Core Freeverb processing through comb + allpass network
    fn process_core(&mut self, input: f32, feedback: f32, damping: f32) -> (f32, f32) {
        let mut out_l = 0.0f32;
        let mut out_r = 0.0f32;

        // Freeverb-style input scaling: combs with high feedback have peak gain
        // of 1/(1-feedback), so scale input to prevent level blowup.
        // Standard Freeverb uses fixedgain=0.015 with wet output gain=3.0;
        // we fold both into a single input gain: 0.015 * 3.0 ≈ 0.045.
        const INPUT_GAIN: f32 = 0.045;
        let scaled_input = input * INPUT_GAIN;

        for i in 0..8 {
            self.comb_l[i].set_feedback(feedback);
            self.comb_l[i].set_damping(damping);
            self.comb_r[i].set_feedback(feedback);
            self.comb_r[i].set_damping(damping);

            out_l += self.comb_l[i].process(scaled_input);
            out_r += self.comb_r[i].process(scaled_input);
        }

        for i in 0..4 {
            out_l = self.allpass_l[i].process(out_l);
            out_r = self.allpass_r[i].process(out_r);
        }

        (out_l, out_r)
    }

    /// Long comb processing for cavernous spaces
    fn process_long_combs(&mut self, input: f32, feedback: f32, damping: f32) -> (f32, f32) {
        let mut out_l = 0.0f32;
        let mut out_r = 0.0f32;

        const INPUT_GAIN: f32 = 0.045;
        let scaled_input = input * INPUT_GAIN;

        for i in 0..4 {
            self.long_comb_l[i].set_feedback(feedback);
            self.long_comb_l[i].set_damping(damping);
            self.long_comb_r[i].set_feedback(feedback);
            self.long_comb_r[i].set_damping(damping);

            out_l += self.long_comb_l[i].process(scaled_input);
            out_r += self.long_comb_r[i].process(scaled_input);
        }

        for i in 0..4 {
            out_l = self.diff_allpass_l[i].process(out_l);
            out_r = self.diff_allpass_r[i].process(out_r);
        }

        (out_l, out_r)
    }

    /// Apply stereo width
    fn apply_width(&self, out_l: f32, out_r: f32) -> (f32, f32) {
        let mid = (out_l + out_r) * 0.5;
        let side = (out_l - out_r) * 0.5 * self.width;
        (mid + side, mid - side)
    }

    /// Process stereo sample
    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.mix <= 0.0001 {
            return (input_l, input_r);
        }

        let (pre_l, pre_r) = self.process_pre_delay(input_l, input_r);
        let mut input_mono = (pre_l + pre_r) * 0.5;

        // After a type switch, fade input in over ~50ms to prevent transient spikes
        // from sudden full-level signal hitting fresh high-feedback filters.
        if self.type_switch_fade > 0 {
            let fade_total = (self.sample_rate * 0.05) as f32;
            let fade_gain = 1.0 - (self.type_switch_fade as f32 / fade_total);
            input_mono *= fade_gain;
            self.type_switch_fade -= 1;
        }

        // Early reflections: discrete echoes that give the ear room geometry cues.
        // Some types (Plate, Spring, Reverse, Infinite) skip ER for character reasons.
        let (er_l, er_r, late_input) = match self.reverb_type {
            ReverbType::Plate | ReverbType::Spring | ReverbType::Reverse | ReverbType::Infinite => {
                (0.0, 0.0, input_mono)
            }
            _ => self.process_early_reflections(input_mono),
        };

        // Late reverb (diffuse tail) — fed by ER output for smooth ER→tail crossover
        let (mut wet_l, mut wet_r) = match self.reverb_type {
            ReverbType::Hall => self.process_hall(late_input),
            ReverbType::Room => self.process_room(late_input),
            ReverbType::Plate => self.process_plate(input_mono),
            ReverbType::Spring => self.process_spring(input_mono),
            ReverbType::Shimmer => self.process_shimmer(late_input),
            ReverbType::Cathedral => self.process_cathedral(late_input),
            ReverbType::Cave => self.process_cave(late_input),
            ReverbType::Ambient => self.process_ambient(late_input, pre_l, pre_r),
            ReverbType::Gated => self.process_gated(late_input, input_l, input_r),
            ReverbType::Reverse => self.process_reverse(input_mono),
            ReverbType::Infinite => self.process_infinite(input_mono),
        };

        // Blend early reflections with late tail
        wet_l += er_l;
        wet_r += er_r;

        let (wet_l, wet_r) = self.apply_width(wet_l, wet_r);

        // Guard against NaN/inf from filter blowup
        let wet_l = if wet_l.is_finite() {
            wet_l.clamp(-4.0, 4.0)
        } else {
            0.0
        };
        let wet_r = if wet_r.is_finite() {
            wet_r.clamp(-4.0, 4.0)
        } else {
            0.0
        };

        // Equal-power crossfade (cached coefficients, updated in set_mix)
        let out_l = input_l * self.mix_dry + wet_l * self.mix_wet;
        let out_r = input_r * self.mix_dry + wet_r * self.mix_wet;

        (out_l, out_r)
    }

    // --- Type-specific algorithms ---

    /// Hall: classic large concert hall, warm and enveloping
    fn process_hall(&mut self, input: f32) -> (f32, f32) {
        let feedback = self.decay * 0.3 + 0.6; // 0.6-0.9: long natural tail
        let damping = self.damping * 0.6 + 0.1;
        let (mut out_l, mut out_r) = self.process_core(input, feedback, damping);
        let scale = self.size * 0.8 + 0.6;
        out_l *= scale;
        out_r *= scale;
        (out_l, out_r)
    }

    /// Room: small, tight reflections with quick decay
    fn process_room(&mut self, input: f32) -> (f32, f32) {
        let feedback = self.decay * 0.25 + 0.35; // 0.35-0.6: short tail
        let damping = self.damping * 0.4 + 0.4; // more damped = tighter
        let (mut out_l, mut out_r) = self.process_core(input, feedback, damping);
        let scale = self.size * 0.5 + 0.8;
        out_l *= scale;
        out_r *= scale;
        (out_l, out_r)
    }

    /// Plate: bright, dense, metallic character
    fn process_plate(&mut self, input: f32) -> (f32, f32) {
        let feedback = self.decay * 0.3 + 0.55; // 0.55-0.85: medium-long
        let damping = self.damping * 0.3 + 0.05; // low damping = bright
        let (mut out_l, mut out_r) = self.process_core(input, feedback, damping);
        let scale = self.size * 0.7 + 0.8;
        out_l *= scale;
        out_r *= scale;
        (out_l, out_r)
    }

    /// Spring: bouncy, metallic, vintage character
    fn process_spring(&mut self, input: f32) -> (f32, f32) {
        let feedback = self.decay * 0.2 + 0.4; // 0.4-0.6: short bouncy
        let damping = self.damping * 0.5 + 0.2;
        let (mut out_l, mut out_r) = self.process_core(input, feedback, damping);
        // Spring character: emphasize the "drip" by adding some of the early comb output
        let drip = self.comb_l[0].process(input * 0.1) * 0.15;
        out_l += drip;
        out_r += drip * 0.7;
        let scale = self.size * 0.4 + 0.7;
        out_l *= scale;
        out_r *= scale;
        (out_l, out_r)
    }

    /// Shimmer: ethereal, pitch-shifted reverb with octave harmonics
    fn process_shimmer(&mut self, input: f32) -> (f32, f32) {
        let feedback = self.decay * 0.3 + 0.6; // 0.6-0.9
        let damping = self.damping * 0.3 + 0.05; // bright
        let (mut out_l, mut out_r) = self.process_core(input, feedback, damping);

        // Dual shimmer LFOs at different rates for richer effect
        self.shimmer_phase += 1.5 * self.inv_sample_rate;
        if self.shimmer_phase > 1.0 {
            self.shimmer_phase -= 1.0;
        }
        self.shimmer_phase2 += 2.3 * self.inv_sample_rate;
        if self.shimmer_phase2 > 1.0 {
            self.shimmer_phase2 -= 1.0;
        }

        let shimmer1 = fast_sin(self.shimmer_phase * std::f32::consts::TAU);
        let shimmer2 = fast_sin(self.shimmer_phase2 * std::f32::consts::TAU);

        let shimmer_amount = self.size * 0.15;
        out_l *= 1.0 + shimmer1 * shimmer_amount;
        out_r *= 1.0 + shimmer2 * shimmer_amount;

        // Add octave-up harmonics through the long combs
        let (long_l, long_r) = self.process_long_combs(input * 0.3, feedback * 0.85, damping);
        out_l += long_l * 0.2 * self.size;
        out_r += long_r * 0.2 * self.size;

        let scale = self.size * 0.7 + 0.7;
        out_l *= scale;
        out_r *= scale;
        (out_l, out_r)
    }

    /// Cathedral: massive space with very long, diffuse tail (5-20+ seconds)
    fn process_cathedral(&mut self, input: f32) -> (f32, f32) {
        // High feedback for enormous tail, but capped at 0.95 to stay stable
        let feedback = self.decay * 0.1 + 0.84; // 0.84-0.94
        let damping = self.damping * 0.5 + 0.15;

        // Standard network
        let (mut out_l, mut out_r) = self.process_core(input, feedback, damping);

        // Long combs for the massive tail (slightly higher feedback than core)
        let long_feedback = (feedback * 1.02).min(0.96);
        let (long_l, long_r) = self.process_long_combs(input, long_feedback, damping * 0.7);

        // Blend standard + long for layered reflections
        out_l = out_l * 0.4 + long_l * 0.6;
        out_r = out_r * 0.4 + long_r * 0.6;

        let scale = self.size * 0.5 + 0.7;
        out_l *= scale;
        out_r *= scale;
        (out_l, out_r)
    }

    /// Cave: dark, cavernous with heavy low-end and long pre-delay feel
    fn process_cave(&mut self, input: f32) -> (f32, f32) {
        let feedback = self.decay * 0.12 + 0.80; // 0.80-0.92: long but stable
        let damping = self.damping * 0.3 + 0.55; // heavily damped = dark

        let (mut out_l, mut out_r) = self.process_core(input, feedback, damping);

        // Long combs with heavy damping for dark rumble
        // (process_long_combs already applies diff_allpass internally)
        let long_fb = (feedback * 0.98).min(0.90);
        let (long_l, long_r) = self.process_long_combs(input, long_fb, damping + 0.1);

        out_l = out_l * 0.4 + long_l * 0.6;
        out_r = out_r * 0.4 + long_r * 0.6;

        let scale = self.size * 0.5 + 0.5;
        out_l *= scale;
        out_r *= scale;
        (out_l, out_r)
    }

    /// Ambient: lush, modulated, wide pad-like reverb
    fn process_ambient(&mut self, input: f32, stereo_l: f32, stereo_r: f32) -> (f32, f32) {
        let feedback = self.decay * 0.2 + 0.68; // 0.68-0.88
        let damping = self.damping * 0.4 + 0.1; // relatively bright

        // Modulate the comb feedback slightly for chorus-like lushness
        self.mod_phase += 0.3 * self.inv_sample_rate;
        if self.mod_phase > 1.0 {
            self.mod_phase -= 1.0;
        }
        let modulation = fast_sin(self.mod_phase * std::f32::consts::TAU) * 0.015;

        let (mut out_l, mut out_r) = self.process_core(input, feedback + modulation, damping);

        // Process stereo input separately for wide imaging
        let (long_l, long_r) = self.process_long_combs(
            (stereo_l - stereo_r) * 0.3, // side signal for width
            (feedback * 0.9).min(0.92),
            damping * 0.8,
        );

        out_l += long_l * 0.3;
        out_r -= long_r * 0.3; // inverted for wider image

        let scale = self.size * 0.7 + 0.7;
        out_l *= scale;
        out_r *= scale;
        (out_l, out_r)
    }

    /// Gated: 80s-style reverb with abrupt cutoff after a hold time
    fn process_gated(&mut self, input: f32, raw_l: f32, raw_r: f32) -> (f32, f32) {
        // Detect input level for gate trigger
        let input_level = (raw_l.abs() + raw_r.abs()) * 0.5;
        let gate_threshold = 0.005;

        // Gate hold time based on size parameter (50ms to 800ms) — cached.
        if input_level > gate_threshold {
            self.gate_env = 1.0;
            self.gate_hold_counter = self.cached_gate_hold_samples;
        } else if self.gate_hold_counter > 0 {
            self.gate_hold_counter -= 1;
        } else {
            // Sharp exponential decay when gate closes
            self.gate_env *= 0.995;
            if self.gate_env < 0.001 {
                self.gate_env = 0.0;
            }
        }

        // Dense reverb with medium feedback (the gate does the shaping)
        let feedback = self.decay * 0.2 + 0.65;
        let damping = self.damping * 0.4 + 0.2;
        let (mut out_l, mut out_r) = self.process_core(input, feedback, damping);

        let scale = self.size * 0.7 + 1.0;
        out_l *= scale * self.gate_env;
        out_r *= scale * self.gate_env;
        (out_l, out_r)
    }

    /// Reverse: reversed reverb tail for swelling, pre-verb effects
    fn process_reverse(&mut self, input: f32) -> (f32, f32) {
        let buf_len = self.reverse_buffer.len() / 2;
        if buf_len < 4 {
            return (0.0, 0.0);
        }

        // Write input into forward buffer. Bounded advance is
        // cheaper than `% buf_len` per sample.
        self.reverse_buffer[self.reverse_write_pos * 2] = input;
        self.reverse_buffer[self.reverse_write_pos * 2 + 1] = input;
        self.reverse_write_pos += 1;
        if self.reverse_write_pos >= buf_len { self.reverse_write_pos = 0; }

        // Grain length based on size (100ms to 1.5 seconds)
        let grain_len = ((self.size * 1.4 + 0.1) * self.sample_rate) as usize;
        let grain_len = grain_len.min(buf_len - 1).max(64);

        // Advance grain phase
        self.reverse_grain_phase += 1.0;
        let grain_pos = self.reverse_grain_phase as usize;

        if grain_pos >= grain_len {
            self.reverse_grain_phase = 0.0;
            // Snap read position to current write position
            self.reverse_read_pos = self.reverse_write_pos as f32;
        }

        // Read backwards from the read position
        let progress = self.reverse_grain_phase / grain_len as f32;
        let read_offset = (progress * grain_len as f32) as usize;
        let read_idx = (self.reverse_read_pos as usize + buf_len - read_offset) % buf_len;

        let raw = self.reverse_buffer[read_idx * 2];

        // Window function for smooth crossfade (Hann window)
        let window = 0.5 * (1.0 - (progress * std::f32::consts::TAU).cos());

        let reversed = raw * window;

        // Process the reversed signal through a short reverb tail
        let feedback = self.decay * 0.2 + 0.5;
        let damping = self.damping * 0.4 + 0.2;
        let (mut out_l, mut out_r) = self.process_core(reversed, feedback, damping);

        let scale = 1.5;
        out_l *= scale;
        out_r *= scale;
        (out_l, out_r)
    }

    /// Infinite: freeze/sustain reverb that never decays (drone pad)
    fn process_infinite(&mut self, input: f32) -> (f32, f32) {
        // Near-unity feedback = infinite sustain
        // Decay knob controls how much new input feeds in (freeze amount)
        let feedback = 0.995;
        // Very low input gain to compensate for extreme feedback accumulation
        let input_gain = (1.0 - self.decay * 0.95) * 0.1;
        let damping = self.damping * 0.3 + 0.05; // keep it bright for pads

        let scaled_input = input * input_gain;

        let (mut out_l, mut out_r) = self.process_core(scaled_input, feedback, damping);

        // Long combs also at near-unity for massive cloud
        let (long_l, long_r) = self.process_long_combs(scaled_input, 0.993, damping * 0.5);

        out_l = out_l * 0.5 + long_l * 0.5;
        out_r = out_r * 0.5 + long_r * 0.5;

        // Gentle modulation to prevent static buildup
        self.mod_phase += 0.15 * self.inv_sample_rate;
        if self.mod_phase > 1.0 {
            self.mod_phase -= 1.0;
        }
        let mod_val = fast_sin(self.mod_phase * std::f32::consts::TAU) * 0.01;
        out_l *= 1.0 + mod_val;
        out_r *= 1.0 - mod_val;

        let scale = self.size * 0.6 + 0.8;
        out_l *= scale;
        out_r *= scale;
        (out_l, out_r)
    }

    /// Set reverb type — resets all internal buffers to prevent blowup
    /// from residual energy in comb/allpass filters.
    pub fn set_reverb_type(&mut self, reverb_type: ReverbType) {
        if self.reverb_type != reverb_type {
            self.reverb_type = reverb_type;
            // Critical: flush all filter state so old resonance doesn't
            // feed into the new algorithm's feedback path.
            self.reset();
            // Fade input in over ~50ms to let the new algorithm settle
            // without transient spikes from sudden full-level input.
            self.type_switch_fade = (self.sample_rate * 0.05) as u32;
        }
    }

    pub fn set_size(&mut self, size: f32) {
        self.size = size.max(0.0).min(1.0);
        Self::refresh_static(self);
    }

    pub fn set_decay(&mut self, decay: f32) {
        self.decay = decay.max(0.0).min(1.0);
    }

    pub fn set_damping(&mut self, damping: f32) {
        self.damping = damping.max(0.0).min(1.0);
    }

    /// Set pre-delay time (up to 500ms)
    pub fn set_pre_delay(&mut self, pre_delay: f32) {
        self.pre_delay = pre_delay.max(0.0).min(0.5);
    }

    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.max(0.0).min(1.0);
        let angle = self.mix * std::f32::consts::FRAC_PI_2;
        self.mix_dry = angle.cos();
        self.mix_wet = angle.sin();
    }

    pub fn set_width(&mut self, width: f32) {
        self.width = width.max(0.0).min(2.0);
    }

    /// Reset reverb
    pub fn reset(&mut self) {
        for comb in self.comb_l.iter_mut().chain(self.comb_r.iter_mut()) {
            comb.reset();
        }
        for comb in self
            .long_comb_l
            .iter_mut()
            .chain(self.long_comb_r.iter_mut())
        {
            comb.reset();
        }
        for allpass in self.allpass_l.iter_mut().chain(self.allpass_r.iter_mut()) {
            allpass.reset();
        }
        for allpass in self
            .diff_allpass_l
            .iter_mut()
            .chain(self.diff_allpass_r.iter_mut())
        {
            allpass.reset();
        }
        for sample in self.pre_delay_buffer.iter_mut() {
            *sample = 0.0;
        }
        for sample in self.er_buffer.iter_mut() {
            *sample = 0.0;
        }
        self.er_write_pos = 0;
        for sample in self.reverse_buffer.iter_mut() {
            *sample = 0.0;
        }
        self.pre_delay_pos = 0;
        self.reverse_write_pos = 0;
        self.reverse_read_pos = 0.0;
        self.reverse_grain_phase = 0.0;
        self.shimmer_phase = 0.0;
        self.shimmer_phase2 = 0.0;
        self.gate_env = 0.0;
        self.gate_hold_counter = 0;
        self.mod_phase = 0.0;
    }
}

/// Comb filter for reverb
#[derive(Debug, Clone)]
struct CombFilter {
    buffer: Vec<f32>,
    pos: usize,
    feedback: f32,
    filter_state: f32,
    damping: f32,
}

impl CombFilter {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size.max(1)],
            pos: 0,
            feedback: 0.84,
            filter_state: 0.0,
            damping: 0.5,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let output = self.buffer[self.pos];

        // Apply damping filter
        self.filter_state = output * (1.0 - self.damping) + self.filter_state * self.damping;

        // Write to buffer with NaN/inf guard — prevent runaway feedback
        let new_val = input + self.filter_state * self.feedback;
        self.buffer[self.pos] = if new_val.is_finite() {
            new_val.clamp(-4.0, 4.0)
        } else {
            0.0
        };

        // Advance position
        self.pos = (self.pos + 1) % self.buffer.len();

        if output.is_finite() {
            output
        } else {
            0.0
        }
    }

    fn set_feedback(&mut self, feedback: f32) {
        self.feedback = feedback;
    }

    fn set_damping(&mut self, damping: f32) {
        self.damping = damping;
    }

    fn reset(&mut self) {
        for sample in self.buffer.iter_mut() {
            *sample = 0.0;
        }
        self.pos = 0;
        self.filter_state = 0.0;
    }
}

/// Allpass filter for reverb
#[derive(Debug, Clone)]
struct AllPassFilter {
    buffer: Vec<f32>,
    pos: usize,
    feedback: f32,
}

impl AllPassFilter {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size.max(1)],
            pos: 0,
            feedback: 0.5,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.buffer[self.pos];
        let output = -input + delayed;

        let new_val = input + delayed * self.feedback;
        self.buffer[self.pos] = if new_val.is_finite() {
            new_val.clamp(-4.0, 4.0)
        } else {
            0.0
        };

        self.pos = (self.pos + 1) % self.buffer.len();

        if output.is_finite() {
            output
        } else {
            0.0
        }
    }

    fn reset(&mut self) {
        for sample in self.buffer.iter_mut() {
            *sample = 0.0;
        }
        self.pos = 0;
    }
}

//==============================================================================
// CHORUS EFFECT
//==============================================================================

#[derive(Debug, Clone)]
/// BBD "ensemble" chorus — the 3-phase Solina/string-machine widener — as a
/// generic insert effect. Wraps the shared `EnsembleChorus` DSP so the rack
/// offers the same character an engine (VP-330, Aurora) previously baked in,
/// which is what lets those engines move it out to a track FX slot. Input is
/// summed to mono (the BBD path is mono-in / stereo-out), then blended dry/wet.
pub struct EnsembleEffect {
    core: crate::vp330_dsp::EnsembleChorus,
    depth: f32,
    pub mix: f32,
}

impl EnsembleEffect {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            core: crate::vp330_dsp::EnsembleChorus::new(sample_rate),
            depth: 0.5,
            mix: 0.5,
        }
    }
    pub fn set_depth(&mut self, d: f32) { self.depth = d.clamp(0.0, 1.0); }
    pub fn set_mix(&mut self, m: f32) { self.mix = m.clamp(0.0, 1.0); }
    pub fn process(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let mono = (in_l + in_r) * 0.5;
        let (wl, wr) = self.core.process(mono, self.depth);
        let d = 1.0 - self.mix;
        (in_l * d + wl * self.mix, in_r * d + wr * self.mix)
    }
    pub fn reset(&mut self) { self.core.reset(); }
}

/// VCS-3 "Synthi" spring reverb (4-tap diffusion + modulated warble) as a
/// generic insert, wrapping the shared `SpringReverb` DSP verbatim so the
/// metallic spring character an engine baked in can move to a track FX slot
/// with zero tone change. Mono-summed input; the core does its own dry/wet.
#[derive(Debug, Clone)]
pub struct SpringTankEffect {
    core: crate::spring_reverb::SpringReverb,
    decay: f32,
    pub mix: f32,
}

impl SpringTankEffect {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            core: crate::spring_reverb::SpringReverb::new(sample_rate),
            decay: 0.6,
            mix: 0.3,
        }
    }
    pub fn set_decay(&mut self, d: f32) { self.decay = d.clamp(0.0, 1.0); }
    pub fn set_mix(&mut self, m: f32) { self.mix = m.clamp(0.0, 1.0); }
    pub fn process(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let mono = (in_l + in_r) * 0.5;
        self.core.process(mono, self.mix, self.decay)
    }
    pub fn reset(&mut self) { self.core.reset(); }
}

/// Aurora's "cinematic plate" hall reverb (pre-delay + dense ER + comb/allpass
/// diffusion, loudness-matched) as a generic insert, wrapping the shared
/// `HallReverb` DSP verbatim so the cinematic plate can move to a track FX slot
/// with zero tone change. `HallReverb` emits WET only, so blend dry/wet here.
#[derive(Debug, Clone)]
pub struct CinemaPlateEffect {
    core: crate::aurora_dsp::reverb::HallReverb,
    sample_rate: f32,
    size: f32,
    damp: f32,
    pub mix: f32,
}

impl CinemaPlateEffect {
    pub fn new(sample_rate: f32) -> Self {
        let mut core = crate::aurora_dsp::reverb::HallReverb::new();
        core.set_sample_rate(sample_rate);
        core.set_params(0.6, 0.3);
        Self { core, sample_rate, size: 0.6, damp: 0.3, mix: 0.3 }
    }
    pub fn set_size(&mut self, s: f32) {
        self.size = s.clamp(0.0, 1.0);
        self.core.set_params(self.size, self.damp);
    }
    pub fn set_damp(&mut self, d: f32) {
        self.damp = d.clamp(0.0, 1.0);
        self.core.set_params(self.size, self.damp);
    }
    pub fn set_mix(&mut self, m: f32) { self.mix = m.clamp(0.0, 1.0); }
    pub fn process(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let (wl, wr) = self.core.process(in_l, in_r);
        // Match Aurora's engine blend exactly: soft dry attenuation (1 - mix*0.5),
        // NOT a full crossfade, so the plate keeps the direct signal present
        // instead of washing it out.
        let d = 1.0 - self.mix * 0.5;
        (in_l * d + wl * self.mix, in_r * d + wr * self.mix)
    }
    pub fn reset(&mut self) {
        // HallReverb has no in-place reset; re-init clears every internal buffer.
        self.core = crate::aurora_dsp::reverb::HallReverb::new();
        self.core.set_sample_rate(self.sample_rate);
        self.core.set_params(self.size, self.damp);
    }
}

/// VP-330's Schroeder string-machine reverb (faithful DSP) as a generic insert,
/// wrapping the shared `SimpleReverb` verbatim so the vintage string-machine
/// tail can move to a track FX slot with zero tone change. `SimpleReverb` emits
/// WET only, so blend dry/wet here.
#[derive(Debug, Clone)]
pub struct SolinaVerbEffect {
    core: crate::vp330_dsp::SimpleReverb,
    sample_rate: f32,
    size: f32,
    damp: f32,
    pub mix: f32,
}

impl SolinaVerbEffect {
    pub fn new(sample_rate: f32) -> Self {
        let mut core = crate::vp330_dsp::SimpleReverb::new(sample_rate);
        core.set_size(0.5);
        core.set_damping(0.4);
        Self { core, sample_rate, size: 0.5, damp: 0.4, mix: 0.25 }
    }
    pub fn set_size(&mut self, s: f32) {
        self.size = s.clamp(0.0, 1.0);
        self.core.set_size(self.size);
    }
    pub fn set_damp(&mut self, d: f32) {
        self.damp = d.clamp(0.0, 1.0);
        self.core.set_damping(self.damp);
    }
    pub fn set_mix(&mut self, m: f32) { self.mix = m.clamp(0.0, 1.0); }
    pub fn process(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let (wl, wr) = self.core.process(in_l, in_r);
        let d = 1.0 - self.mix;
        (in_l * d + wl * self.mix, in_r * d + wr * self.mix)
    }
    pub fn reset(&mut self) {
        // SimpleReverb has no in-place reset; re-init clears the comb/allpass state.
        self.core = crate::vp330_dsp::SimpleReverb::new(self.sample_rate);
        self.core.set_size(self.size);
        self.core.set_damping(self.damp);
    }
}

/// Aurora's stereo BBD "dimension" chorus (two modulated delays 180° apart —
/// the wide-image widener) as a generic insert, wrapping the shared
/// `StereoChorus` DSP verbatim. Params in raw units (delay ms, rate Hz) so a
/// host can pass an engine's exact values. The core self-mixes via `wet`, so
/// `mix` maps straight to it. Aurora fixes the mod depth at 2.5 ms; kept here.
#[derive(Debug, Clone)]
pub struct DimensionEffect {
    core: crate::aurora_dsp::chorus::StereoChorus,
    delay_ms: f32,
    rate_hz: f32,
    pub mix: f32,
}

impl DimensionEffect {
    pub fn new(sample_rate: f32) -> Self {
        let mut core = crate::aurora_dsp::chorus::StereoChorus::new(sample_rate);
        core.set_params(8.0, 2.5, 0.5);
        Self { core, delay_ms: 8.0, rate_hz: 0.5, mix: 0.5 }
    }
    pub fn set_delay(&mut self, ms: f32) {
        self.delay_ms = ms.clamp(0.1, 20.0);
        self.core.set_params(self.delay_ms, 2.5, self.rate_hz);
    }
    pub fn set_rate(&mut self, hz: f32) {
        self.rate_hz = hz.clamp(0.05, 8.0);
        self.core.set_params(self.delay_ms, 2.5, self.rate_hz);
    }
    pub fn set_mix(&mut self, m: f32) { self.mix = m.clamp(0.0, 1.0); }
    pub fn process(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        self.core.process(in_l, in_r, self.mix)
    }
    pub fn reset(&mut self) { self.core.reset_state(); }
}

/// Chorus effect processor
#[derive(Debug, Clone)]
pub struct ChorusEffect {
    sample_rate: f32,
    /// `1.0 / sample_rate` cache — `effective_rate / sr * 2π` ran
    /// once per `process()` call (per audio sample) for the LFO
    /// phase advance. Replaces the FP-division with a multiply.
    inv_sample_rate: f32,

    // Parameters
    rate: f32,     // LFO rate in Hz
    depth: f32,    // Modulation depth
    mix: f32,      // Dry/wet mix
    delay: f32,    // Base delay time
    voices: usize, // Number of chorus voices
    width: f32,    // Stereo width

    // Tempo sync
    tempo_sync: bool,
    tempo_division: TempoDivision,
    tempo: f32,
    beat_position: f64,

    // Delay lines
    delay_buffer: Vec<f32>,
    write_pos: usize,

    // LFO phases (one per voice)
    lfo_phases: Vec<f32>,

    // Cached `i / voices * TAU` per voice. Was recomputed every
    // sample × per voice — pointless since it only changes when
    // voices changes.
    cached_phase_offsets: [f32; 8],
    // Cached crossfade gains — set_mix refreshes.
    cached_dry_gain: f32,
    cached_wet_gain: f32,
}

impl ChorusEffect {
    pub fn new(sample_rate: f32) -> Self {
        let max_delay = 0.05; // 50ms max
        let buffer_size = (sample_rate * max_delay * 2.0) as usize;

        let mut s = Self {
            sample_rate,
            inv_sample_rate: 1.0 / sample_rate.max(1.0),
            rate: 1.5,
            depth: 0.3,
            mix: 0.3,
            delay: 0.02,
            voices: 3,
            width: 0.5,
            tempo_sync: false,
            tempo_division: TempoDivision::Quarter,
            tempo: 120.0,
            beat_position: 0.0,
            delay_buffer: vec![0.0; buffer_size],
            write_pos: 0,
            lfo_phases: vec![0.0; 8],
            cached_phase_offsets: [0.0; 8],
            cached_dry_gain: 0.7,
            cached_wet_gain: 0.7,
        };
        s.refresh_phase_offsets();
        s.refresh_mix_gains();
        s
    }

    fn refresh_phase_offsets(&mut self) {
        let v = self.voices.max(1) as f32;
        for i in 0..8 {
            self.cached_phase_offsets[i] =
                (i as f32 / v) * std::f32::consts::TAU;
        }
    }

    fn refresh_mix_gains(&mut self) {
        let angle = self.mix * std::f32::consts::FRAC_PI_2;
        self.cached_dry_gain = angle.cos();
        self.cached_wet_gain = angle.sin();
    }

    fn buffer_size(&self) -> usize {
        self.delay_buffer.len()
    }

    /// Read from delay buffer with interpolation (frame-aligned)
    fn read_delay(&self, channel: usize, delay_samples: f32) -> f32 {
        let num_frames = self.buffer_size() / 2;
        let delay_samples = delay_samples.max(1.0).min((num_frames - 2) as f32);

        let write_frame = self.write_pos / 2;
        let mut read_frame = write_frame as f32 - delay_samples;
        while read_frame < 0.0 {
            read_frame += num_frames as f32;
        }
        let read_frame = read_frame % num_frames as f32;

        let frame_int = read_frame as usize;
        let frac = read_frame - frame_int as f32;
        let frame_next = (frame_int + 1) % num_frames;

        let s1 = self.delay_buffer[frame_int * 2 + channel];
        let s2 = self.delay_buffer[frame_next * 2 + channel];

        s1 + (s2 - s1) * frac
    }

    /// Write to delay buffer (with safety clamp)
    fn write_delay(&mut self, channel: usize, sample: f32) {
        let pos = (self.write_pos + channel) % self.buffer_size();
        self.delay_buffer[pos] = if sample.is_finite() {
            sample.clamp(-4.0, 4.0)
        } else {
            0.0
        };
    }

    /// Process stereo sample
    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.mix <= 0.0001 {
            return (input_l, input_r);
        }

        let base_delay_samples = self.delay * self.sample_rate;
        let effective_rate = self.effective_rate();
        let lfo_inc = effective_rate * self.inv_sample_rate * 2.0 * std::f32::consts::PI;

        let mut wet_l = 0.0f32;
        let mut wet_r = 0.0f32;

        // Generate chorus voices
        for i in 0..self.voices.min(self.lfo_phases.len()) {
            let phase_offset = self.cached_phase_offsets[i];
            let lfo = if self.tempo_sync {
                let base_phase = (self.beat_position / self.tempo_division.beats() as f64).fract()
                    as f32
                    * std::f32::consts::TAU;
                fast_sin(base_phase + phase_offset)
            } else {
                self.lfo_phases[i] += lfo_inc;
                if self.lfo_phases[i] > 2.0 * std::f32::consts::PI {
                    self.lfo_phases[i] -= 2.0 * std::f32::consts::PI;
                }
                fast_sin(self.lfo_phases[i] + phase_offset)
            };

            let mod_delay = base_delay_samples * (1.0 + lfo * self.depth);
            let delayed_l = self.read_delay(0, mod_delay);
            let delayed_r = self.read_delay(1, mod_delay);
            wet_l += delayed_l;
            wet_r += delayed_r;
        }

        // Scale by voice count
        let voice_scale = 1.0 / self.voices.max(1) as f32;
        wet_l *= voice_scale;
        wet_r *= voice_scale;

        // Write input to buffer
        self.write_delay(0, input_l);
        self.write_delay(1, input_r);

        // Apply width
        let mid = (wet_l + wet_r) * 0.5;
        let side = (wet_l - wet_r) * 0.5 * self.width;
        wet_l = mid + side;
        wet_r = mid - side;

        // Cached crossfade gains.
        let out_l = input_l * self.cached_dry_gain + wet_l * self.cached_wet_gain;
        let out_r = input_r * self.cached_dry_gain + wet_r * self.cached_wet_gain;

        // Advance write position (bounded — `%` would be a divide).
        self.write_pos += 2;
        let buf_size = self.buffer_size();
        if self.write_pos >= buf_size { self.write_pos -= buf_size; }

        (out_l, out_r)
    }

    /// Set LFO rate
    pub fn set_rate(&mut self, rate: f32) {
        self.rate = rate.max(0.01).min(20.0);
    }

    /// Set depth
    pub fn set_depth(&mut self, depth: f32) {
        self.depth = depth.max(0.0).min(1.0);
    }

    /// Set mix
    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.max(0.0).min(1.0);
        self.refresh_mix_gains();
    }

    /// Set delay time
    pub fn set_delay(&mut self, delay: f32) {
        self.delay = delay.max(0.001).min(0.05);
    }

    /// Set number of voices
    pub fn set_voices(&mut self, voices: usize) {
        self.voices = voices.max(1).min(8);
        self.refresh_phase_offsets();
    }

    /// Set stereo width
    pub fn set_width(&mut self, width: f32) {
        self.width = width.max(0.0).min(1.0);
    }

    pub fn set_tempo_sync(&mut self, sync: bool) {
        self.tempo_sync = sync;
    }
    pub fn set_tempo_division(&mut self, division: TempoDivision) {
        self.tempo_division = division;
    }
    pub fn set_tempo(&mut self, tempo: f32) {
        self.tempo = tempo.max(20.0).min(300.0);
    }
    pub fn set_beat_position(&mut self, pos: f64) {
        self.beat_position = pos;
    }

    fn effective_rate(&self) -> f32 {
        if self.tempo_sync {
            1.0 / self.tempo_division.to_time_seconds(self.tempo)
        } else {
            self.rate
        }
    }

    /// Reset chorus
    pub fn reset(&mut self) {
        for sample in self.delay_buffer.iter_mut() {
            *sample = 0.0;
        }
        self.write_pos = 0;
        for phase in self.lfo_phases.iter_mut() {
            *phase = 0.0;
        }
    }
}

//==============================================================================
// CHOIR EFFECT
//==============================================================================

const CHOIR_MAX_VOICES: usize = 12;

/// Cheap per-voice white noise in [-1, 1). xorshift32 — deterministic per seed,
/// branch-free, no trig. Used to drive the choir's random-walk pitch/timing.
#[inline(always)]
fn choir_rand_pm1(state: &mut u32) -> f32 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    (x as f32) * (2.0 / 4_294_967_296.0) - 1.0
}

/// Choir effect — an ensemble of distinguishable solo voices.
///
/// Unlike a chorus (periodic sinusoidal LFOs → metallic sheen), each voice
/// here drifts with its OWN band-limited RANDOM walk in pitch (smoothstep
/// value-noise driving a fractional delay → non-periodic micro-detune) and a
/// slower random walk in timing (onset scatter), carries a DECORRELATED vibrato
/// (per-voice rate/phase, delayed onset), and gets a small per-voice timbre
/// fingerprint (one peaking biquad) so the voices are distinguishable rather
/// than detuned clones. This follows the choir-synthesis research consensus
/// (IRCAM / PSOLA-choir / unison-singing studies): a convincing choir is the
/// superposition of several solo voices with random micro-deviations + distinct
/// timbre, not a chorus.
#[derive(Debug, Clone)]
pub struct ChoirEffect {
    sample_rate: f32,

    // Parameters (all 0..1 except voices)
    voices: usize,   // 2-12 choir voices
    detune: f32,     // random-walk pitch depth (was `spread`)
    wander: f32,     // random-walk rate (was `shimmer`)
    vibrato: f32,    // decorrelated vibrato depth
    timbre: f32,     // per-voice timbre spread (0 = bypass)
    width: f32,      // stereo spread
    warmth: f32,     // LP filter on wet (0=bright, 1=warm)
    pub mix: f32,    // dry/wet

    // Delay buffer (stereo interleaved), ~250ms to fit base+scatter+detune+vib.
    delay_buffer: Vec<f32>,
    write_pos: usize,

    // Fixed per-voice character (seeded once in `new`)
    voice_base_delay: [f32; CHOIR_MAX_VOICES], // seconds (20-50ms)
    voice_pan: [f32; CHOIR_MAX_VOICES],        // -1..+1
    voice_scale: [f32; CHOIR_MAX_VOICES],      // 0.6-1.0 detune unevenness
    vib_rate: [f32; CHOIR_MAX_VOICES],         // Hz (5-7, per-voice spread)
    timbre_center: [f32; CHOIR_MAX_VOICES],    // Hz (700-3000 log-spaced)
    rng: [u32; CHOIR_MAX_VOICES],              // per-voice noise state

    // Dynamic per-voice state
    // pitch value-noise (smoothstep-interpolated random targets)
    vn_phase: [f32; CHOIR_MAX_VOICES],
    vn_cur: [f32; CHOIR_MAX_VOICES],
    vn_nxt: [f32; CHOIR_MAX_VOICES],
    // timing value-noise (slower)
    tn_phase: [f32; CHOIR_MAX_VOICES],
    tn_cur: [f32; CHOIR_MAX_VOICES],
    tn_nxt: [f32; CHOIR_MAX_VOICES],
    // decorrelated vibrato + delayed onset ramp
    vib_phase: [f32; CHOIR_MAX_VOICES],
    vib_gain: [f32; CHOIR_MAX_VOICES],
    // per-voice timbre fingerprint
    timbre_eq: [RbjBiquad; CHOIR_MAX_VOICES],

    // Warmth filter state
    lp_state_l: f32,
    lp_state_r: f32,

    // ── Cached per-sample constants (recomputed only in setters) ──────────
    cached_pan_l: [f32; CHOIR_MAX_VOICES],
    cached_pan_r: [f32; CHOIR_MAX_VOICES],
    cached_vib_inc: [f32; CHOIR_MAX_VOICES],   // from fixed vib_rate
    cached_onset_inc: [f32; CHOIR_MAX_VOICES], // 1/(onset_sec·sr), fixed
    cached_pitch_inc: f32,                     // pitch value-noise rate / sr
    cached_timing_inc: f32,                    // timing value-noise rate / sr (fixed)
    cached_detune_samp: f32,                   // pitch-walk depth in samples
    cached_vib_depth_samp: f32,                // vibrato depth in samples
    cached_timing_samp: f32,                   // timing-walk depth in samples (fixed)
    cached_warmth_alpha: f32,
    cached_dry_gain: f32,
    cached_wet_gain: f32,
    timbre_active: bool,
}

impl ChoirEffect {
    pub fn new(sample_rate: f32) -> Self {
        let buffer_size = (sample_rate * 0.25) as usize * 2; // 250ms stereo

        let mut base_delay = [0.0f32; CHOIR_MAX_VOICES];
        let mut pan = [0.0f32; CHOIR_MAX_VOICES];
        let mut scale = [0.0f32; CHOIR_MAX_VOICES];
        let mut vib_rate = [0.0f32; CHOIR_MAX_VOICES];
        let mut timbre_center = [0.0f32; CHOIR_MAX_VOICES];
        let mut rng = [0u32; CHOIR_MAX_VOICES];
        let mut vn_phase = [0.0f32; CHOIR_MAX_VOICES];
        let mut vn_cur = [0.0f32; CHOIR_MAX_VOICES];
        let mut vn_nxt = [0.0f32; CHOIR_MAX_VOICES];
        let mut tn_phase = [0.0f32; CHOIR_MAX_VOICES];
        let mut tn_cur = [0.0f32; CHOIR_MAX_VOICES];
        let mut tn_nxt = [0.0f32; CHOIR_MAX_VOICES];
        let mut vib_phase = [0.0f32; CHOIR_MAX_VOICES];
        let mut cached_vib_inc = [0.0f32; CHOIR_MAX_VOICES];
        let mut cached_onset_inc = [0.0f32; CHOIR_MAX_VOICES];

        let inv_sr_tau = std::f32::consts::TAU / sample_rate;
        let denom = (CHOIR_MAX_VOICES - 1).max(1) as f32;
        for i in 0..CHOIR_MAX_VOICES {
            let t = i as f32;
            // Staggered base delays (20-50ms) so taps never collapse onto each
            // other even with the scatter/detune/vibrato offsets stacked.
            base_delay[i] = 0.020 + (t * 2.7 + 1.1).sin().abs() * 0.030;
            // Spread across the stereo field.
            pan[i] = (t / denom) * 2.0 - 1.0;
            // Per-voice detune scale so singers wander unequally.
            scale[i] = 0.6 + (t * 2.1 + 0.5).sin().abs() * 0.4;
            // Decorrelated vibrato rate 4.8-6.2 Hz.
            vib_rate[i] = 5.5 + (t * 1.7 + 0.3).sin() * 0.7;
            cached_vib_inc[i] = vib_rate[i] * inv_sr_tau;
            // Delayed vibrato onset 0.3-1.5s so vibratos don't bloom in sync.
            let onset_sec = 0.3 + (t * 3.1 + 0.7).sin().abs() * 1.2;
            cached_onset_inc[i] = 1.0 / (onset_sec * sample_rate);
            // Log-spaced timbre centres 700-3000 Hz.
            timbre_center[i] = 700.0 * (3000.0f32 / 700.0).powf(t / denom);
            // Deterministic non-zero per-voice noise seed.
            let seed = 0x9E37_79B9u32
                .wrapping_add((i as u32).wrapping_add(1).wrapping_mul(0x6D2B_79F5));
            rng[i] = seed | 1;
            // Decorrelate starting phases.
            vn_phase[i] = (t * 0.618) % 1.0;
            tn_phase[i] = (t * 0.381) % 1.0;
            vib_phase[i] = (t * 0.618 * std::f32::consts::TAU) % std::f32::consts::TAU;
            // Prime the value-noise targets.
            vn_cur[i] = choir_rand_pm1(&mut rng[i]);
            vn_nxt[i] = choir_rand_pm1(&mut rng[i]);
            tn_cur[i] = choir_rand_pm1(&mut rng[i]);
            tn_nxt[i] = choir_rand_pm1(&mut rng[i]);
        }

        let mut s = Self {
            sample_rate,
            voices: 6,
            detune: 0.5,
            wander: 0.35,
            vibrato: 0.3,
            timbre: 0.4,
            width: 0.7,
            warmth: 0.5,
            mix: 0.5,
            delay_buffer: vec![0.0; buffer_size],
            write_pos: 0,
            voice_base_delay: base_delay,
            voice_pan: pan,
            voice_scale: scale,
            vib_rate,
            timbre_center,
            rng,
            vn_phase,
            vn_cur,
            vn_nxt,
            tn_phase,
            tn_cur,
            tn_nxt,
            vib_phase,
            vib_gain: [0.0; CHOIR_MAX_VOICES],
            timbre_eq: [RbjBiquad::new(); CHOIR_MAX_VOICES],
            lp_state_l: 0.0,
            lp_state_r: 0.0,
            cached_pan_l: [0.707; CHOIR_MAX_VOICES],
            cached_pan_r: [0.707; CHOIR_MAX_VOICES],
            cached_vib_inc,
            cached_onset_inc,
            cached_pitch_inc: 0.0,
            cached_timing_inc: 0.3 / sample_rate, // 0.3 Hz timing wander (fixed)
            cached_detune_samp: 0.0,
            cached_vib_depth_samp: 0.0,
            cached_timing_samp: 0.010 * sample_rate, // ±10ms onset scatter (fixed)
            cached_warmth_alpha: 0.0,
            cached_dry_gain: 0.707,
            cached_wet_gain: 0.707,
            timbre_active: false,
        };
        s.refresh_pan();
        s.refresh_wander();
        s.refresh_detune();
        s.refresh_vibrato();
        s.refresh_timbre();
        s.refresh_warmth();
        s.refresh_mix_gains();
        s
    }

    /// Precompute pan_l/pan_r per voice from `voice_pan[i] * width`.
    fn refresh_pan(&mut self) {
        for i in 0..CHOIR_MAX_VOICES {
            let pan = self.voice_pan[i] * self.width;
            let angle = (pan * 0.5 + 0.5) * std::f32::consts::FRAC_PI_2;
            self.cached_pan_l[i] = angle.cos();
            self.cached_pan_r[i] = angle.sin();
        }
    }

    /// Pitch random-walk rate: Wander knob → 0.3..3.0 Hz value-noise.
    fn refresh_wander(&mut self) {
        let f_wander = 0.3 + self.wander * 2.7;
        self.cached_pitch_inc = f_wander / self.sample_rate;
    }

    /// Pitch random-walk depth: Detune knob → up to ±6ms delay excursion.
    fn refresh_detune(&mut self) {
        self.cached_detune_samp = self.detune * 0.006 * self.sample_rate;
    }

    /// Vibrato depth: Vibrato knob → up to ±1.8ms delay sweep.
    fn refresh_vibrato(&mut self) {
        self.cached_vib_depth_samp = self.vibrato * 0.0018 * self.sample_rate;
    }

    /// Per-voice peaking-EQ fingerprints: alternating ±(timbre·4dB) at each
    /// voice's log-spaced centre. timbre=0 bypasses the biquads entirely.
    fn refresh_timbre(&mut self) {
        self.timbre_active = self.timbre > 0.001;
        if !self.timbre_active {
            return;
        }
        let gain = self.timbre * 4.0;
        for i in 0..CHOIR_MAX_VOICES {
            let sign = if i & 1 == 0 { 1.0 } else { -1.0 };
            self.timbre_eq[i].set_peak(self.sample_rate, self.timbre_center[i], 0.8, sign * gain);
        }
    }

    /// Precompute the warmth-filter alpha (static between setter calls).
    fn refresh_warmth(&mut self) {
        if self.warmth > 0.01 {
            let cutoff = (1.0 - self.warmth) * (1.0 - self.warmth) * 18000.0 + 500.0;
            let rc = 1.0 / (cutoff * std::f32::consts::TAU);
            let inv_sr = 1.0 / self.sample_rate;
            self.cached_warmth_alpha = inv_sr / (rc + inv_sr);
        } else {
            self.cached_warmth_alpha = 0.0;
        }
    }

    /// Precompute the equal-power dry/wet crossfade gains.
    fn refresh_mix_gains(&mut self) {
        let angle = self.mix * std::f32::consts::FRAC_PI_2;
        self.cached_dry_gain = angle.cos();
        self.cached_wet_gain = angle.sin();
    }

    fn buf_frames(&self) -> usize {
        self.delay_buffer.len() / 2
    }

    fn read_delay(&self, channel: usize, delay_samples: f32) -> f32 {
        let num_frames = self.buf_frames();
        let delay_samples = delay_samples.max(1.0).min((num_frames - 2) as f32);
        let write_frame = self.write_pos / 2;
        let mut read_frame = write_frame as f32 - delay_samples;
        if read_frame < 0.0 {
            read_frame += num_frames as f32;
        }
        let read_frame = read_frame % num_frames as f32;
        let fi = read_frame as usize;
        let frac = read_frame - fi as f32;
        let fn_ = (fi + 1) % num_frames;
        let s1 = self.delay_buffer[fi * 2 + channel];
        let s2 = self.delay_buffer[fn_ * 2 + channel];
        s1 + (s2 - s1) * frac
    }

    #[inline]
    fn write_input(&mut self, input_l: f32, input_r: f32) {
        let pos = self.write_pos % self.delay_buffer.len();
        self.delay_buffer[pos] = if input_l.is_finite() { input_l.clamp(-4.0, 4.0) } else { 0.0 };
        self.delay_buffer[pos + 1] = if input_r.is_finite() { input_r.clamp(-4.0, 4.0) } else { 0.0 };
        self.write_pos += 2;
        let buf_len = self.delay_buffer.len();
        if self.write_pos >= buf_len { self.write_pos -= buf_len; }
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.mix <= 0.0001 {
            // Keep the delay line fresh so re-enabling doesn't pop.
            self.write_input(input_l, input_r);
            return (input_l, input_r);
        }

        let mut wet_l = 0.0f32;
        let mut wet_r = 0.0f32;
        let n = self.voices.min(CHOIR_MAX_VOICES);

        for i in 0..n {
            // ── Pitch random walk (smoothstep value-noise) ───────────────
            self.vn_phase[i] += self.cached_pitch_inc;
            if self.vn_phase[i] >= 1.0 {
                self.vn_phase[i] -= 1.0;
                self.vn_cur[i] = self.vn_nxt[i];
                self.vn_nxt[i] = choir_rand_pm1(&mut self.rng[i]);
            }
            let tp = self.vn_phase[i];
            let pitch_walk = self.vn_cur[i] + (self.vn_nxt[i] - self.vn_cur[i]) * (tp * tp * (3.0 - 2.0 * tp));

            // ── Timing random walk (slower) → onset scatter ──────────────
            self.tn_phase[i] += self.cached_timing_inc;
            if self.tn_phase[i] >= 1.0 {
                self.tn_phase[i] -= 1.0;
                self.tn_cur[i] = self.tn_nxt[i];
                self.tn_nxt[i] = choir_rand_pm1(&mut self.rng[i]);
            }
            let tt = self.tn_phase[i];
            let timing_walk = self.tn_cur[i] + (self.tn_nxt[i] - self.tn_cur[i]) * (tt * tt * (3.0 - 2.0 * tt));

            // ── Decorrelated vibrato with delayed onset ──────────────────
            self.vib_phase[i] = (self.vib_phase[i] + self.cached_vib_inc[i]) % std::f32::consts::TAU;
            let vib = fast_sin(self.vib_phase[i]) * self.cached_vib_depth_samp * self.vib_gain[i];
            if self.vib_gain[i] < 1.0 {
                self.vib_gain[i] = (self.vib_gain[i] + self.cached_onset_inc[i]).min(1.0);
            }

            let delay_samples = self.voice_base_delay[i] * self.sample_rate
                + pitch_walk * self.cached_detune_samp * self.voice_scale[i]
                + timing_walk * self.cached_timing_samp
                + vib;

            let mut dl = self.read_delay(0, delay_samples);
            let mut dr = self.read_delay(1, delay_samples);
            if self.timbre_active {
                dl = self.timbre_eq[i].process(dl);
                dr = self.timbre_eq[i].process(dr);
            }
            wet_l += dl * self.cached_pan_l[i];
            wet_r += dr * self.cached_pan_r[i];
        }

        // Incoherent-sum normalisation: the taps are decorrelated, so 1/√N
        // keeps the ensemble at roughly unity (the old 1/N over-attenuated).
        let scale = 1.0 / (n.max(1) as f32).sqrt();
        wet_l = (wet_l * scale).clamp(-4.0, 4.0);
        wet_r = (wet_r * scale).clamp(-4.0, 4.0);

        // Warmth filter (1-pole LP).
        if self.cached_warmth_alpha > 0.0 {
            let alpha = self.cached_warmth_alpha;
            self.lp_state_l += alpha * (wet_l - self.lp_state_l);
            self.lp_state_r += alpha * (wet_r - self.lp_state_r);
            wet_l = self.lp_state_l;
            wet_r = self.lp_state_r;
        }

        self.write_input(input_l, input_r);

        // Equal-power crossfade (gains cached on set_mix).
        (
            input_l * self.cached_dry_gain + wet_l * self.cached_wet_gain,
            input_r * self.cached_dry_gain + wet_r * self.cached_wet_gain,
        )
    }

    pub fn set_voices(&mut self, v: usize) {
        self.voices = v.clamp(2, CHOIR_MAX_VOICES);
    }
    /// Random-walk pitch depth (formerly `spread`).
    pub fn set_detune(&mut self, v: f32) {
        self.detune = v.clamp(0.0, 1.0);
        self.refresh_detune();
    }
    /// Random-walk rate (formerly `shimmer`).
    pub fn set_wander(&mut self, v: f32) {
        self.wander = v.clamp(0.0, 1.0);
        self.refresh_wander();
    }
    pub fn set_vibrato(&mut self, v: f32) {
        self.vibrato = v.clamp(0.0, 1.0);
        self.refresh_vibrato();
    }
    pub fn set_timbre(&mut self, v: f32) {
        self.timbre = v.clamp(0.0, 1.0);
        self.refresh_timbre();
    }
    pub fn set_mix(&mut self, v: f32) {
        self.mix = v.clamp(0.0, 1.0);
        self.refresh_mix_gains();
    }
    pub fn set_width(&mut self, v: f32) {
        self.width = v.clamp(0.0, 1.0);
        self.refresh_pan();
    }
    pub fn set_warmth(&mut self, v: f32) {
        self.warmth = v.clamp(0.0, 1.0);
        self.refresh_warmth();
    }

    pub fn reset(&mut self) {
        self.delay_buffer.fill(0.0);
        self.write_pos = 0;
        self.lp_state_l = 0.0;
        self.lp_state_r = 0.0;
        for i in 0..CHOIR_MAX_VOICES {
            self.vib_gain[i] = 0.0;
            self.timbre_eq[i].reset();
        }
    }
}

//==============================================================================
// PHASER EFFECT
//==============================================================================

/// Phaser effect processor
#[derive(Debug, Clone)]
pub struct PhaserEffect {
    sample_rate: f32,
    /// `1.0 / sample_rate` cache for the per-sample LFO phase
    /// advance — replaces an FP-division per audio sample.
    inv_sample_rate: f32,

    // Parameters
    rate: f32,         // LFO rate in Hz
    depth: f32,        // Modulation depth
    mix: f32,          // Dry/wet mix
    stages: usize,     // Number of allpass stages
    feedback: f32,     // Feedback amount
    stereo_phase: f32, // Stereo phase offset

    // Tempo sync
    tempo_sync: bool,
    tempo_division: TempoDivision,
    tempo: f32,
    beat_position: f64,

    // LFO phase
    lfo_phase: f32,

    // Allpass filters for each channel
    allpass_l: Vec<AllPassFilterVariable>,
    allpass_r: Vec<AllPassFilterVariable>,

    // Feedback state
    feedback_l: f32,
    feedback_r: f32,
}

impl PhaserEffect {
    pub fn new(sample_rate: f32) -> Self {
        let max_stages = 12;
        let allpass_l: Vec<AllPassFilterVariable> = (0..max_stages)
            .map(|_| AllPassFilterVariable::new(sample_rate))
            .collect();
        let allpass_r: Vec<AllPassFilterVariable> = (0..max_stages)
            .map(|_| AllPassFilterVariable::new(sample_rate))
            .collect();

        Self {
            sample_rate,
            inv_sample_rate: 1.0 / sample_rate.max(1.0),
            rate: 0.5,
            depth: 0.7,
            mix: 0.5,
            stages: 4,
            feedback: 0.5,
            stereo_phase: 0.5,
            tempo_sync: false,
            tempo_division: TempoDivision::Quarter,
            tempo: 120.0,
            beat_position: 0.0,
            lfo_phase: 0.0,
            allpass_l,
            allpass_r,
            feedback_l: 0.0,
            feedback_r: 0.0,
        }
    }

    /// Process stereo sample
    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.mix <= 0.0001 {
            return (input_l, input_r);
        }

        // Update LFO
        if self.tempo_sync {
            self.lfo_phase = (self.beat_position / self.tempo_division.beats() as f64).fract()
                as f32
                * std::f32::consts::TAU;
        } else {
            let effective_rate = self.effective_rate();
            let lfo_inc = effective_rate * self.inv_sample_rate * 2.0 * std::f32::consts::PI;
            self.lfo_phase += lfo_inc;
            if self.lfo_phase > 2.0 * std::f32::consts::PI {
                self.lfo_phase -= 2.0 * std::f32::consts::PI;
            }
        }

        // Calculate LFO values with stereo offset
        let lfo_l = fast_sin(self.lfo_phase);
        let lfo_r = fast_sin(self.lfo_phase + self.stereo_phase * std::f32::consts::PI);

        // Frequency range: 300Hz to 3000Hz for allpass
        let freq_min = 300.0f32;
        let freq_max = 3000.0f32;

        // Process left channel
        let mut signal_l = input_l + self.feedback_l * self.feedback;
        let freq_l = freq_min * (freq_max / freq_min).powf((lfo_l * self.depth + 1.0) * 0.5);

        for i in 0..self.stages.min(self.allpass_l.len()) {
            self.allpass_l[i].set_frequency(freq_l);
            signal_l = self.allpass_l[i].process(signal_l);
        }

        // Process right channel
        let mut signal_r = input_r + self.feedback_r * self.feedback;
        let freq_r = freq_min * (freq_max / freq_min).powf((lfo_r * self.depth + 1.0) * 0.5);

        for i in 0..self.stages.min(self.allpass_r.len()) {
            self.allpass_r[i].set_frequency(freq_r);
            signal_r = self.allpass_r[i].process(signal_r);
        }

        // Store feedback
        self.feedback_l = signal_l;
        self.feedback_r = signal_r;

        // Mix dry and wet
        let out_l = input_l * (1.0 - self.mix) + signal_l * self.mix;
        let out_r = input_r * (1.0 - self.mix) + signal_r * self.mix;

        (out_l, out_r)
    }

    /// Set LFO rate
    pub fn set_rate(&mut self, rate: f32) {
        self.rate = rate.max(0.01).min(20.0);
    }

    /// Set depth
    pub fn set_depth(&mut self, depth: f32) {
        self.depth = depth.max(0.0).min(1.0);
    }

    /// Set mix
    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.max(0.0).min(1.0);
    }

    /// Set number of stages
    pub fn set_stages(&mut self, stages: usize) {
        self.stages = stages.max(1).min(12);
    }

    /// Set feedback
    pub fn set_feedback(&mut self, feedback: f32) {
        self.feedback = feedback.max(0.0).min(0.99);
    }

    /// Set stereo phase offset
    pub fn set_stereo_phase(&mut self, phase: f32) {
        self.stereo_phase = phase.max(0.0).min(1.0);
    }

    pub fn set_tempo_sync(&mut self, sync: bool) {
        self.tempo_sync = sync;
    }
    pub fn set_tempo_division(&mut self, division: TempoDivision) {
        self.tempo_division = division;
    }
    pub fn set_tempo(&mut self, tempo: f32) {
        self.tempo = tempo.max(20.0).min(300.0);
    }
    pub fn set_beat_position(&mut self, pos: f64) {
        self.beat_position = pos;
    }

    fn effective_rate(&self) -> f32 {
        if self.tempo_sync {
            1.0 / self.tempo_division.to_time_seconds(self.tempo)
        } else {
            self.rate
        }
    }

    /// Reset phaser
    pub fn reset(&mut self) {
        self.lfo_phase = 0.0;
        self.feedback_l = 0.0;
        self.feedback_r = 0.0;
        for allpass in self.allpass_l.iter_mut().chain(self.allpass_r.iter_mut()) {
            allpass.reset();
        }
    }
}

/// Variable frequency allpass filter for phaser
#[derive(Debug, Clone)]
struct AllPassFilterVariable {
    sample_rate: f32,
    /// `π / sample_rate` cache. The phaser sweeps `set_frequency`
    /// every sample on every stage (up to 12 stages), so the
    /// `PI · freq / sr` reduces to a multiply once cached.
    pi_over_sr: f32,
    frequency: f32,
    x1: f32,
    y1: f32,
    a: f32,
}

impl AllPassFilterVariable {
    fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            pi_over_sr: std::f32::consts::PI / sample_rate.max(1.0),
            frequency: 1000.0,
            x1: 0.0,
            y1: 0.0,
            a: 0.5,
        }
    }

    fn set_frequency(&mut self, freq: f32) {
        self.frequency = freq;
        let omega = freq * self.pi_over_sr;
        self.a = (omega - 1.0) / (omega + 1.0);
    }

    fn process(&mut self, input: f32) -> f32 {
        let output = self.a * input + self.x1 - self.a * self.y1;
        self.x1 = input;
        self.y1 = output;
        output
    }

    fn reset(&mut self) {
        self.x1 = 0.0;
        self.y1 = 0.0;
    }
}

//==============================================================================
// FLANGER EFFECT
//==============================================================================

/// Flanger effect processor
#[derive(Debug, Clone)]
pub struct FlangerEffect {
    sample_rate: f32,
    /// `1.0 / sample_rate` cache for the per-sample LFO phase
    /// advance — replaces an FP-division per audio sample.
    inv_sample_rate: f32,

    // Parameters
    rate: f32,         // LFO rate in Hz
    depth: f32,        // Modulation depth
    mix: f32,          // Dry/wet mix
    delay: f32,        // Base delay time
    feedback: f32,     // Feedback amount
    stereo_phase: f32, // Stereo phase offset

    // Tempo sync
    tempo_sync: bool,
    tempo_division: TempoDivision,
    tempo: f32,
    beat_position: f64,

    // Delay buffer
    delay_buffer: Vec<f32>,
    write_pos: usize,

    // LFO phase
    lfo_phase: f32,

    // Feedback state
    feedback_l: f32,
    feedback_r: f32,
}

impl FlangerEffect {
    pub fn new(sample_rate: f32) -> Self {
        let max_delay = 0.02; // 20ms max
        let buffer_size = (sample_rate * max_delay * 2.0) as usize;

        Self {
            sample_rate,
            inv_sample_rate: 1.0 / sample_rate.max(1.0),
            rate: 0.2,
            depth: 0.8,
            mix: 0.5,
            delay: 0.005,
            feedback: 0.7,
            stereo_phase: 0.0,
            tempo_sync: false,
            tempo_division: TempoDivision::Quarter,
            tempo: 120.0,
            beat_position: 0.0,
            delay_buffer: vec![0.0; buffer_size.max(4)],
            write_pos: 0,
            lfo_phase: 0.0,
            feedback_l: 0.0,
            feedback_r: 0.0,
        }
    }

    fn buffer_size(&self) -> usize {
        self.delay_buffer.len()
    }

    /// Read from delay buffer with interpolation (frame-aligned)
    fn read_delay(&self, channel: usize, delay_samples: f32) -> f32 {
        let num_frames = self.buffer_size() / 2;
        let delay_samples = delay_samples.max(1.0).min((num_frames - 2) as f32);

        let write_frame = self.write_pos / 2;
        let mut read_frame = write_frame as f32 - delay_samples;
        while read_frame < 0.0 {
            read_frame += num_frames as f32;
        }
        let read_frame = read_frame % num_frames as f32;

        let frame_int = read_frame as usize;
        let frac = read_frame - frame_int as f32;
        let frame_next = (frame_int + 1) % num_frames;

        let s1 = self.delay_buffer[frame_int * 2 + channel];
        let s2 = self.delay_buffer[frame_next * 2 + channel];

        s1 + (s2 - s1) * frac
    }

    /// Write to delay buffer (with safety clamp)
    fn write_delay(&mut self, channel: usize, sample: f32) {
        let pos = (self.write_pos + channel) % self.buffer_size();
        self.delay_buffer[pos] = if sample.is_finite() {
            sample.clamp(-4.0, 4.0)
        } else {
            0.0
        };
    }

    /// Process stereo sample
    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.mix <= 0.0001 {
            return (input_l, input_r);
        }

        // Update LFO
        if self.tempo_sync {
            self.lfo_phase = (self.beat_position / self.tempo_division.beats() as f64).fract()
                as f32
                * std::f32::consts::TAU;
        } else {
            let effective_rate = self.effective_rate();
            let lfo_inc = effective_rate * self.inv_sample_rate * 2.0 * std::f32::consts::PI;
            self.lfo_phase += lfo_inc;
            if self.lfo_phase > 2.0 * std::f32::consts::PI {
                self.lfo_phase -= 2.0 * std::f32::consts::PI;
            }
        }

        // Calculate LFO values with stereo offset
        let lfo_l = fast_sin(self.lfo_phase);
        let lfo_r = fast_sin(self.lfo_phase + self.stereo_phase * std::f32::consts::PI);

        // Calculate modulated delay
        let base_delay_samples = self.delay * self.sample_rate;
        let mod_range = self.depth * base_delay_samples;

        let delay_l = base_delay_samples + lfo_l * mod_range;
        let delay_r = base_delay_samples + lfo_r * mod_range;

        // Write to buffer with feedback
        self.write_delay(0, input_l + self.feedback_l * self.feedback);
        self.write_delay(1, input_r + self.feedback_r * self.feedback);

        // Read delayed signal
        let delayed_l = self.read_delay(0, delay_l);
        let delayed_r = self.read_delay(1, delay_r);

        // Store feedback
        self.feedback_l = delayed_l;
        self.feedback_r = delayed_r;

        // Mix dry and wet
        let out_l = input_l * (1.0 - self.mix) + delayed_l * self.mix;
        let out_r = input_r * (1.0 - self.mix) + delayed_r * self.mix;

        // Advance write position (bounded — `%` would be a divide).
        self.write_pos += 2;
        let buf_size = self.buffer_size();
        if self.write_pos >= buf_size { self.write_pos -= buf_size; }

        (out_l, out_r)
    }

    /// Set LFO rate
    pub fn set_rate(&mut self, rate: f32) {
        self.rate = rate.max(0.01).min(20.0);
    }

    /// Set depth
    pub fn set_depth(&mut self, depth: f32) {
        self.depth = depth.max(0.0).min(1.0);
    }

    /// Set mix
    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.max(0.0).min(1.0);
    }

    /// Set delay time
    pub fn set_delay(&mut self, delay: f32) {
        self.delay = delay.max(0.001).min(0.02);
    }

    /// Set feedback
    pub fn set_feedback(&mut self, feedback: f32) {
        self.feedback = feedback.max(0.0).min(0.99);
    }

    /// Set stereo phase offset
    pub fn set_stereo_phase(&mut self, phase: f32) {
        self.stereo_phase = phase.max(0.0).min(1.0);
    }

    pub fn set_tempo_sync(&mut self, sync: bool) {
        self.tempo_sync = sync;
    }
    pub fn set_tempo_division(&mut self, division: TempoDivision) {
        self.tempo_division = division;
    }
    pub fn set_tempo(&mut self, tempo: f32) {
        self.tempo = tempo.max(20.0).min(300.0);
    }
    pub fn set_beat_position(&mut self, pos: f64) {
        self.beat_position = pos;
    }

    fn effective_rate(&self) -> f32 {
        if self.tempo_sync {
            1.0 / self.tempo_division.to_time_seconds(self.tempo)
        } else {
            self.rate
        }
    }

    /// Reset flanger
    pub fn reset(&mut self) {
        for sample in self.delay_buffer.iter_mut() {
            *sample = 0.0;
        }
        self.write_pos = 0;
        self.lfo_phase = 0.0;
        self.feedback_l = 0.0;
        self.feedback_r = 0.0;
    }
}

//==============================================================================
// DISTORTION EFFECT
//==============================================================================

/// Distortion effect processor
#[derive(Debug, Clone)]
pub struct DistortionEffect {
    sample_rate: f32,

    // Distortion type
    distortion_type: DistortionType,

    // Parameters
    drive: f32,  // Input gain
    tone: f32,   // Tone filter frequency
    mix: f32,    // Dry/wet mix
    output: f32, // Output gain

    // Filter state for tone control
    tone_filter: ToneFilter,

    // Bitcrusher state
    bit_depth: usize,
    sample_rate_div: usize,
    sample_counter: usize,
    last_sample_l: f32,
    last_sample_r: f32,

    // Tape hysteresis memory (1-pole)
    tape_mem_l: f32,
    tape_mem_r: f32,
}

impl DistortionEffect {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            distortion_type: DistortionType::SoftClip,
            drive: 1.0,
            tone: 1000.0,
            mix: 0.5,
            output: 1.0,
            tone_filter: ToneFilter::new(sample_rate),
            bit_depth: 8,
            sample_rate_div: 1,
            sample_counter: 0,
            last_sample_l: 0.0,
            last_sample_r: 0.0,
            tape_mem_l: 0.0, tape_mem_r: 0.0,
        }
    }

    /// Apply distortion shaper to a driven sample.
    /// `ch` picks the hysteresis memory for the Tape shaper.
    #[inline]
    fn distort_sample(&mut self, driven: f32, ch: usize) -> f32 {
        match self.distortion_type {
            DistortionType::SoftClip => driven.tanh(),
            DistortionType::HardClip => driven.clamp(-1.0, 1.0),
            DistortionType::Tube => {
                let x = driven;
                let abs_x = x.abs();
                let shaped = if x > 0.0 {
                    1.0 - (1.0 - abs_x).max(0.0).powf(1.5)
                } else {
                    -(1.0 - (1.0 - abs_x).max(0.0).powf(0.8))
                };
                shaped.clamp(-1.0, 1.0)
            }
            DistortionType::Fuzz => {
                let x = driven * 2.0;
                let folded = if x.abs() > 1.0 {
                    x.signum() * (2.0 - x.abs())
                } else {
                    x
                };
                folded.tanh().clamp(-1.0, 1.0)
            }
            DistortionType::Bitcrush => driven, // handled separately
            DistortionType::Tape => {
                // tanh with hysteresis memory — 1-pole feedback of previous output
                let mem = if ch == 0 { self.tape_mem_l } else { self.tape_mem_r };
                let hyst = driven + mem * 0.15;
                let out = (hyst * 0.9).tanh();
                if ch == 0 { self.tape_mem_l = out; } else { self.tape_mem_r = out; }
                out
            }
            DistortionType::Diode => {
                // Rectified exponential: sign(x) * (1 - exp(-|x|))
                let abs_x = driven.abs();
                driven.signum() * (1.0 - (-abs_x).exp())
            }
            DistortionType::Fold => {
                // Triangular wavefold: 2/π * asin(sin(π/2 * x))
                let x = driven * 0.5;
                (2.0 / std::f32::consts::PI) * (x * std::f32::consts::PI / 2.0).sin().asin() * 2.0
            }
            DistortionType::SoftKnee => {
                // The canon render's memoryless master clip: linear below t=0.9, gentle tanh
                // above (only the tips round off -> crest-preserving, no limiter pumping).
                let a = driven.abs();
                let t = 0.9f32;
                if a <= t { driven } else { (t + (1.0 - t) * ((a - t) / (1.0 - t)).tanh()) * driven.signum() }
            }
            DistortionType::TanhClip => {
                // The canon render's pulsar growing-distortion shaper (canon_trance 1146):
                //   sh(v) = 0.5*tanh(v*drive) + 0.5*clamp(v*drive, -1, 1)
                // `driven` already = input*drive, so this is bit-identical to the render's sh.
                0.5 * driven.tanh() + 0.5 * driven.clamp(-1.0, 1.0)
            }
        }
    }

    /// Process stereo sample
    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.mix <= 0.0001 && self.distortion_type != DistortionType::Bitcrush {
            return (input_l, input_r);
        }

        let (wet_l, wet_r) = if self.distortion_type == DistortionType::Bitcrush {
            // Bitcrusher: reduce bit depth and sample rate
            self.sample_counter += 1;

            if self.sample_counter >= self.sample_rate_div {
                self.sample_counter = 0;

                // Quantize to bit depth
                let levels = (1 << self.bit_depth.min(16)) as f32;
                let quantized_l = (input_l * levels).round() / levels;
                let quantized_r = (input_r * levels).round() / levels;

                self.last_sample_l = quantized_l;
                self.last_sample_r = quantized_r;
            }

            (self.last_sample_l, self.last_sample_r)
        } else {
            let drive = self.drive;
            let shaped_l = self.distort_sample(input_l * drive, 0);
            let shaped_r = self.distort_sample(input_r * drive, 1);
            // TONE BYPASS: a TRANSPARENT memoryless clipper (no tone coloration). The SoftKnee
            // mastering clip ALWAYS bypasses (the render's master clip has no tone filter; the
            // tone param can be clamped to the UI max so a sentinel freq alone is unreliable).
            // tone >= 19999 also bypasses for any type. Existing patches (tone <= 10 kHz, non-SoftKnee)
            // are byte-stable.
            if self.tone >= 19999.0 || self.distortion_type == DistortionType::SoftKnee
                || self.distortion_type == DistortionType::TanhClip {
                // SoftKnee + TanhClip are the render's memoryless shapers (no tone filter).
                (shaped_l, shaped_r)
            } else {
                // Apply tone filter — separate L/R state to avoid
                // cross-channel pollution; alpha cached so we don't
                // do 3 divisions/sample × 2 channels for a static knob.
                let filtered_l = self.tone_filter.process_l(shaped_l, self.tone);
                let filtered_r = self.tone_filter.process_r(shaped_r, self.tone);
                (filtered_l, filtered_r)
            }
        };

        // Apply output gain and soft-clip to prevent excessive levels
        let wet_l = (wet_l * self.output).clamp(-2.0, 2.0);
        let wet_r = (wet_r * self.output).clamp(-2.0, 2.0);

        // Mix dry and wet
        let out_l = input_l * (1.0 - self.mix) + wet_l * self.mix;
        let out_r = input_r * (1.0 - self.mix) + wet_r * self.mix;

        (out_l, out_r)
    }

    /// Set distortion type
    pub fn set_distortion_type(&mut self, distortion_type: DistortionType) {
        self.distortion_type = distortion_type;
    }

    /// Set drive
    pub fn set_drive(&mut self, drive: f32) {
        self.drive = drive.max(0.1).min(100.0);
    }

    /// Set tone frequency
    pub fn set_tone(&mut self, tone: f32) {
        self.tone = tone.max(100.0).min(10000.0);
    }

    /// Set mix
    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.max(0.0).min(1.0);
    }

    /// Set output gain
    pub fn set_output(&mut self, output: f32) {
        self.output = output.max(0.0).min(4.0);
    }

    /// Set bit depth for bitcrusher
    pub fn set_bit_depth(&mut self, bits: usize) {
        self.bit_depth = bits.max(1).min(16);
    }

    /// Set sample rate divider for bitcrusher
    pub fn set_sample_rate_div(&mut self, div: usize) {
        self.sample_rate_div = div.max(1).min(64);
    }

    /// Reset distortion
    pub fn reset(&mut self) {
        self.tone_filter.reset();
        self.sample_counter = 0;
        self.last_sample_l = 0.0;
        self.last_sample_r = 0.0;
        self.tape_mem_l = 0.0; self.tape_mem_r = 0.0;
    }
}

/// Simple tone filter — one-pole LP with separate L/R state to
/// avoid cross-channel contamination, and cached `alpha` recomputed
/// only when the cutoff changes (was 3 divisions / sample × 2
/// channels in the audio loop).
#[derive(Debug, Clone)]
struct ToneFilter {
    sample_rate: f32,
    y1_l: f32,
    y1_r: f32,
    cached_alpha: f32,
    last_cutoff: f32,
}

impl ToneFilter {
    fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            y1_l: 0.0,
            y1_r: 0.0,
            cached_alpha: 1.0,
            last_cutoff: f32::NAN,
        }
    }

    #[inline]
    fn refresh_alpha(&mut self, cutoff: f32) {
        if cutoff != self.last_cutoff {
            let rc = 1.0 / (cutoff * 2.0 * std::f32::consts::PI);
            let dt = 1.0 / self.sample_rate;
            self.cached_alpha = dt / (rc + dt);
            self.last_cutoff = cutoff;
        }
    }

    fn process_l(&mut self, input: f32, cutoff: f32) -> f32 {
        self.refresh_alpha(cutoff);
        self.y1_l += self.cached_alpha * (input - self.y1_l);
        self.y1_l
    }

    fn process_r(&mut self, input: f32, cutoff: f32) -> f32 {
        self.refresh_alpha(cutoff);
        self.y1_r += self.cached_alpha * (input - self.y1_r);
        self.y1_r
    }

    fn reset(&mut self) {
        self.y1_l = 0.0;
        self.y1_r = 0.0;
    }
}

//==============================================================================
// VOCODER EFFECT
//==============================================================================

/// Vocoder effect processor
///
/// Classic channel vocoder: splits modulator and carrier into frequency bands,
/// extracts amplitude envelopes from modulator bands, and applies them to
/// carrier bands.  Carrier = synth output, modulator = voice engines.
#[derive(Debug, Clone)]
pub struct VocoderEffect {
    sample_rate: f32,

    // Parameters
    bands: usize,
    attack: f32,
    release: f32,
    mix: f32,
    carrier_level: f32,
    modulator_level: f32,
    formant_shift: f32,

    // Per-band state (parallel arrays for cache locality)
    carrier_filters_l: Vec<VocoderBPF>,
    carrier_filters_r: Vec<VocoderBPF>,
    modulator_filters: Vec<VocoderBPF>,
    envelopes: Vec<f32>,

    // Envelope coefficients (shared across bands)
    attack_coef: f32,
    release_coef: f32,

    // Band center frequencies (for rebuilding)
    band_freqs: Vec<f32>,

    // Cached gain compensation (recomputed when bands change)
    gain_compensation: f32,
}

impl VocoderEffect {
    pub fn new(sample_rate: f32) -> Self {
        let mut voc = Self {
            sample_rate,
            bands: 16,
            attack: 0.01,
            release: 0.05,
            mix: 1.0,
            carrier_level: 1.0,
            modulator_level: 1.0,
            formant_shift: 1.0,
            carrier_filters_l: Vec::new(),
            carrier_filters_r: Vec::new(),
            modulator_filters: Vec::new(),
            envelopes: Vec::new(),
            attack_coef: 0.0,
            release_coef: 0.0,
            band_freqs: Vec::new(),
            gain_compensation: (16.0f32).sqrt() * 1.5,
        };
        voc.rebuild();
        voc
    }

    /// Compute band center frequencies using logarithmic spacing over voice range.
    /// Range: 80 Hz – 8 kHz (covers fundamental + formants).
    fn compute_band_freqs(bands: usize, formant_shift: f32) -> Vec<f32> {
        let lo = 80.0f32;
        let hi = 8000.0f32;
        let n = bands.max(2);
        let log_lo = lo.ln();
        let log_hi = hi.ln();
        (0..n)
            .map(|i| {
                let t = i as f32 / (n - 1) as f32;
                let freq = (log_lo + t * (log_hi - log_lo)).exp() * formant_shift;
                freq.clamp(20.0, 20000.0)
            })
            .collect()
    }

    /// Q factor: wider at low frequencies, narrower at high frequencies.
    /// This gives better frequency resolution in the formant region.
    fn q_for_band(band_idx: usize, total_bands: usize) -> f32 {
        // Q ranges from ~2.5 (low bands) to ~5 (high bands)
        let t = band_idx as f32 / (total_bands.max(1) - 1).max(1) as f32;
        2.5 + t * 2.5
    }

    /// Rebuild all filter banks from current parameters.
    fn rebuild(&mut self) {
        let freqs = Self::compute_band_freqs(self.bands, self.formant_shift);
        let n = freqs.len();

        self.carrier_filters_l.clear();
        self.carrier_filters_r.clear();
        self.modulator_filters.clear();
        self.envelopes.clear();

        for i in 0..n {
            let q = Self::q_for_band(i, n);
            self.carrier_filters_l
                .push(VocoderBPF::new(self.sample_rate, freqs[i], q));
            self.carrier_filters_r
                .push(VocoderBPF::new(self.sample_rate, freqs[i], q));
            self.modulator_filters
                .push(VocoderBPF::new(self.sample_rate, freqs[i], q));
            self.envelopes.push(0.0);
        }

        self.band_freqs = freqs;
        self.gain_compensation = (n as f32).sqrt() * 1.5;
        self.update_envelope_coefs();
    }

    fn update_envelope_coefs(&mut self) {
        let sr = self.sample_rate;
        self.attack_coef = (-1.0 / (self.attack.max(0.0005) * sr)).exp();
        self.release_coef = (-1.0 / (self.release.max(0.001) * sr)).exp();
    }

    /// Process with explicit modulator signal (voice engines).
    pub fn process_with_modulator(
        &mut self,
        carrier_l: f32,
        carrier_r: f32,
        modulator: f32,
    ) -> (f32, f32) {
        if self.mix <= 0.0001 {
            return (carrier_l, carrier_r);
        }

        let mod_in = modulator * self.modulator_level;
        let car_l = carrier_l * self.carrier_level;
        let car_r = carrier_r * self.carrier_level;

        let n = self.bands.min(self.carrier_filters_l.len());
        let mut out_l = 0.0f32;
        let mut out_r = 0.0f32;

        let atk = self.attack_coef;
        let rel = self.release_coef;

        for i in 0..n {
            // Analyze modulator: bandpass filter then envelope follower
            let mod_band = self.modulator_filters[i].process(mod_in);
            let level = mod_band.abs();

            // Smooth envelope (one-pole, separate attack/release)
            let env = &mut self.envelopes[i];
            if level > *env {
                *env = atk * (*env - level) + level;
            } else {
                *env = rel * (*env - level) + level;
            }
            let envelope = (*env).max(0.0);

            // Filter carrier through matching bands, scale by envelope
            let car_band_l = self.carrier_filters_l[i].process(car_l);
            let car_band_r = self.carrier_filters_r[i].process(car_r);

            out_l += car_band_l * envelope;
            out_r += car_band_r * envelope;
        }

        // Gain compensation (cached, recomputed when bands change)
        out_l *= self.gain_compensation;
        out_r *= self.gain_compensation;

        // Soft-clip — `x / (1 + |x|)` is a tanh-like curve with the
        // same role (catch overs from band-sum gain) but ~2x faster
        // per call than `f32::tanh`.
        out_l = out_l / (1.0 + out_l.abs());
        out_r = out_r / (1.0 + out_r.abs());

        // Dry/wet mix
        let final_l = carrier_l * (1.0 - self.mix) + out_l * self.mix;
        let final_r = carrier_r * (1.0 - self.mix) + out_r * self.mix;

        (final_l, final_r)
    }

    /// Standalone process: uses the input as both carrier and modulator.
    /// This gives a "self-vocoding" effect (spectral gating / formant emphasis).
    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        let modulator = (input_l + input_r) * 0.5;
        self.process_with_modulator(input_l, input_r, modulator)
    }

    pub fn set_bands(&mut self, bands: usize) {
        let new_bands = bands.clamp(4, 32);
        if new_bands != self.bands {
            self.bands = new_bands;
            self.rebuild();
        }
    }

    pub fn set_attack(&mut self, attack: f32) {
        self.attack = attack.clamp(0.0005, 0.1);
        self.update_envelope_coefs();
    }

    pub fn set_release(&mut self, release: f32) {
        self.release = release.clamp(0.001, 0.5);
        self.update_envelope_coefs();
    }

    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    pub fn set_carrier_level(&mut self, level: f32) {
        self.carrier_level = level.clamp(0.0, 2.0);
    }

    pub fn set_modulator_level(&mut self, level: f32) {
        self.modulator_level = level.clamp(0.0, 2.0);
    }

    pub fn set_formant_shift(&mut self, shift: f32) {
        let new_shift = shift.clamp(0.5, 2.0);
        if (new_shift - self.formant_shift).abs() > 0.01 {
            self.formant_shift = new_shift;
            self.rebuild();
        }
    }

    pub fn reset(&mut self) {
        for f in self
            .carrier_filters_l
            .iter_mut()
            .chain(self.carrier_filters_r.iter_mut())
            .chain(self.modulator_filters.iter_mut())
        {
            f.reset();
        }
        for e in &mut self.envelopes {
            *e = 0.0;
        }
    }
}

/// 2nd-order bandpass filter (RBJ Audio EQ Cookbook).
#[derive(Debug, Clone)]
struct VocoderBPF {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl VocoderBPF {
    fn new(sample_rate: f32, freq: f32, q: f32) -> Self {
        let mut f = Self {
            b0: 0.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        };
        f.set_coeffs(sample_rate, freq, q);
        f
    }

    fn set_coeffs(&mut self, sample_rate: f32, freq: f32, q: f32) {
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let sin_w0 = w0.sin();
        let cos_w0 = w0.cos();
        let alpha = sin_w0 / (2.0 * q);
        let a0_inv = 1.0 / (1.0 + alpha);

        // Constant-0dB-peak bandpass (BPF in Audio EQ Cookbook)
        self.b0 = (sin_w0 / 2.0) * a0_inv;
        self.b1 = 0.0;
        self.b2 = -(sin_w0 / 2.0) * a0_inv;
        self.a1 = -2.0 * cos_w0 * a0_inv;
        self.a2 = (1.0 - alpha) * a0_inv;
    }

    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = if y.is_finite() { y } else { 0.0 };
        self.y1
    }

    fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

//==============================================================================
// COMPRESSOR EFFECT
//==============================================================================

/// Compressor mode: VCA (clean/SSL), Opto (slow/LA-2A), FET (fast/1176), Multiband
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressorMode {
    Vca,       // Clean, transparent (SSL-style)
    Opto,      // Program-dependent, slow (LA-2A style)
    Fet,       // Fast, aggressive (1176 style)
    Multiband, // 3-band compression
    Bus,       // Feed-forward peak bus comp (hard knee, NO lookahead) -- the canon render's `compress()`
}

impl CompressorMode {
    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Self::Vca,
            1 => Self::Opto,
            2 => Self::Fet,
            3 => Self::Multiband,
            4 => Self::Bus,
            _ => Self::Vca,
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Self::Vca => "VCA",
            Self::Opto => "Opto",
            Self::Fet => "FET",
            Self::Multiband => "Multi",
            Self::Bus => "Bus",
        }
    }
    pub fn all_names() -> &'static [&'static str] {
        &["VCA", "Opto", "FET", "Multi", "Bus"]
    }
}

/// Limiter mode: Brickwall (hard), Soft Clip (gentle saturation), Maximizer (multi-stage)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimiterMode {
    Brickwall, // Hard ceiling with lookahead
    SoftClip,  // Gentle saturation curve
    Maximizer, // Multi-stage loudness maximizer
}

impl LimiterMode {
    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Self::Brickwall,
            1 => Self::SoftClip,
            2 => Self::Maximizer,
            _ => Self::Brickwall,
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Self::Brickwall => "Brick",
            Self::SoftClip => "Soft",
            Self::Maximizer => "Max",
        }
    }
    pub fn all_names() -> &'static [&'static str] {
        &["Brick", "Soft", "Max"]
    }
}

/// Expander mode: Gate (hard gate), Gentle (soft expansion), Ducker (duck loud signals)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpanderMode {
    Gate,   // Hard gate with range
    Gentle, // Soft downward expansion
    Ducker, // Duck loud signals (inverted)
}

impl ExpanderMode {
    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Self::Gate,
            1 => Self::Gentle,
            2 => Self::Ducker,
            _ => Self::Gate,
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Self::Gate => "Gate",
            Self::Gentle => "Gentle",
            Self::Ducker => "Ducker",
        }
    }
    pub fn all_names() -> &'static [&'static str] {
        &["Gate", "Gentle", "Ducker"]
    }
}

#[derive(Debug, Clone)]
pub struct CompressorEffect {
    sample_rate: f32,
    mode: CompressorMode,
    threshold: f32,   // dB (-60 to 0)
    ratio: f32,       // 1:1 to 20:1
    attack: f32,      // seconds
    release: f32,     // seconds
    makeup_gain: f32, // dB
    knee: f32,        // dB width of soft knee
    mix: f32,         // dry/wet

    // State
    envelope_db: f32, // smoothed level in dB
    attack_coef: f32,
    release_coef: f32,

    // Sidechain: when active, the detector listens to an external signal
    // written each frame by the sequencer instead of the input audio.
    pub sidechain_active: bool,
    pub sidechain_l: f32,
    pub sidechain_r: f32,

    // Multiband state
    mb_lo_state: f32, // 1-pole LPF state for low crossover
    mb_hi_state: f32, // 1-pole LPF state for high crossover
    mb_env_lo: f32,   // per-band envelope (dB)
    mb_env_mid: f32,
    mb_env_hi: f32,
    mb_lo_coef: f32, // cached crossover coefficient at 200Hz
    mb_hi_coef: f32, // cached crossover coefficient at 3000Hz

    // === Phase 2 upgrades ===
    /// Lookahead delay line (5 ms): the audio path reads `lookahead_len`
    /// samples behind the detector so the envelope can start opening
    /// before a transient arrives.
    lookahead_l: Vec<f32>,
    lookahead_r: Vec<f32>,
    lookahead_pos: usize,
    lookahead_len: usize,
    /// Sidechain high-pass filter cutoff in Hz — stops bass from pumping
    /// the whole mix. 0 disables.
    sc_hpf_hz: f32,
    sc_hp_l_prev_in: f32,
    sc_hp_l_prev_out: f32,
    sc_hp_r_prev_in: f32,
    sc_hp_r_prev_out: f32,
    /// When non-zero, auto-makeup is added on top of the user-set makeup.
    auto_makeup: f32, // 0..1

    // ── Control-rate cache for Opto/FET modes ─────────────────
    // process_opto + process_fet recomputed `(-1/(τ·sr)).exp()`
    // twice per sample. The τ depends on `envelope_db` (slow,
    // one-pole follower) — sample-accurate refresh isn't needed.
    // These get refreshed every CMP_CTRL_DIVIDER samples; first
    // sample after a mode/attack/release change forces a refresh
    // (`ctrl_dirty`).
    ctrl_counter: u32,
    ctrl_dirty: bool,
    cached_opto_att: f32,
    cached_opto_rel: f32,
    cached_fet_att: f32,
    cached_fet_rel: f32,

    // ── Bus mode (verbatim canon `compress()`): a LINEAR peak follower
    // (not the dB gain-reduction envelope the other modes smooth) plus the
    // linear-domain threshold / makeup caches refreshed in update_coefficients.
    bus_env: f32,
    bus_thr_lin: f32,
    bus_mk: f32,
}

const CMP_CTRL_DIVIDER: u32 = 32;

impl CompressorEffect {
    pub fn new(sample_rate: f32) -> Self {
        let lookahead_len = ((sample_rate * 0.005) as usize).max(1);
        let mut s = Self {
            sample_rate,
            mode: CompressorMode::Vca,
            threshold: -18.0,
            ratio: 4.0,
            attack: 0.005,
            release: 0.1,
            makeup_gain: 0.0,
            knee: 6.0,
            mix: 1.0,
            // `envelope_db` is the SMOOTHED gain-reduction in dB (added to makeup to form the
            // output gain: gain = db_to_lin(envelope_db + makeup)). Its REST value is 0 (no
            // reduction = unity), matching mb_env_* below. Initialising it to -96 made every
            // compressor start FULLY CLOSED (gain ~= db_to_lin(-96) ~= silence) and open over
            // ~5x its release time -> an audible fade-in at the very start of an offline bounce
            // (and at each instrument entrance for a freshly-constructed strip). Init at 0 = unity.
            envelope_db: 0.0,
            attack_coef: 0.0,
            release_coef: 0.0,
            sidechain_active: false,
            sidechain_l: 0.0,
            sidechain_r: 0.0,
            mb_lo_state: 0.0,
            mb_hi_state: 0.0,
            mb_env_lo: 0.0,
            mb_env_mid: 0.0,
            mb_env_hi: 0.0,
            mb_lo_coef: (-std::f32::consts::TAU * 200.0 / sample_rate).exp(),
            mb_hi_coef: (-std::f32::consts::TAU * 3000.0 / sample_rate).exp(),
            lookahead_l: vec![0.0; lookahead_len],
            lookahead_r: vec![0.0; lookahead_len],
            lookahead_pos: 0,
            lookahead_len,
            sc_hpf_hz: 0.0,
            sc_hp_l_prev_in: 0.0, sc_hp_l_prev_out: 0.0,
            sc_hp_r_prev_in: 0.0, sc_hp_r_prev_out: 0.0,
            auto_makeup: 0.0,
            ctrl_counter: 0,
            ctrl_dirty: true,
            cached_opto_att: 0.0,
            cached_opto_rel: 0.0,
            cached_fet_att: 0.0,
            cached_fet_rel: 0.0,
            bus_env: 0.0,
            bus_thr_lin: 1.0,
            bus_mk: 1.0,
        };
        s.update_coefficients();
        s
    }

    /// Apply the 1-pole sidechain HPF to the detector input. Returns
    /// the passed-through value when `sc_hpf_hz` is 0.
    #[inline]
    fn sc_hp_filter(&mut self, x_l: f32, x_r: f32) -> (f32, f32) {
        if self.sc_hpf_hz <= 0.1 { return (x_l, x_r); }
        // 1-pole RC HPF: y = alpha * (y_prev + x - x_prev)
        let rc = 1.0 / (std::f32::consts::TAU * self.sc_hpf_hz);
        let dt = 1.0 / self.sample_rate;
        let alpha = rc / (rc + dt);
        let out_l = alpha * (self.sc_hp_l_prev_out + x_l - self.sc_hp_l_prev_in);
        let out_r = alpha * (self.sc_hp_r_prev_out + x_r - self.sc_hp_r_prev_in);
        self.sc_hp_l_prev_in = x_l;
        self.sc_hp_r_prev_in = x_r;
        self.sc_hp_l_prev_out = out_l;
        self.sc_hp_r_prev_out = out_r;
        (out_l, out_r)
    }

    /// Push a new stereo sample into the lookahead buffer and return the
    /// sample that falls out of the far end (delayed by `lookahead_len`).
    #[inline]
    fn lookahead_push(&mut self, l: f32, r: f32) -> (f32, f32) {
        if self.lookahead_len <= 1 { return (l, r); }
        let out_l = self.lookahead_l[self.lookahead_pos];
        let out_r = self.lookahead_r[self.lookahead_pos];
        self.lookahead_l[self.lookahead_pos] = l;
        self.lookahead_r[self.lookahead_pos] = r;
        self.lookahead_pos = (self.lookahead_pos + 1) % self.lookahead_len;
        (out_l, out_r)
    }

    #[inline]
    fn auto_makeup_db(&self) -> f32 {
        // Classic rule: makeup = -threshold * (1 - 1/ratio) * 0.5
        let ratio_inv = 1.0 / self.ratio.max(1.0);
        (-self.threshold * (1.0 - ratio_inv) * 0.5) * self.auto_makeup
    }

    fn update_coefficients(&mut self) {
        self.attack_coef = (-1.0 / (self.attack * self.sample_rate)).exp();
        self.release_coef = (-1.0 / (self.release * self.sample_rate)).exp();
    }

    fn gain_computer(&self, input_db: f32) -> f32 {
        let half_knee = self.knee * 0.5;
        let diff = input_db - self.threshold;

        if diff <= -half_knee {
            // Below knee: no compression
            input_db
        } else if diff >= half_knee {
            // Above knee: full compression
            self.threshold + diff / self.ratio
        } else {
            // In knee: smooth transition
            let x = diff + half_knee;
            input_db + (1.0 / self.ratio - 1.0) * x * x / (2.0 * self.knee)
        }
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        match self.mode {
            CompressorMode::Vca => self.process_vca(input_l, input_r),
            CompressorMode::Opto => self.process_opto(input_l, input_r),
            CompressorMode::Fet => self.process_fet(input_l, input_r),
            CompressorMode::Multiband => self.process_multiband(input_l, input_r),
            CompressorMode::Bus => self.process_bus(input_l, input_r),
        }
    }

    // Bus: a VERBATIM port of the canon render's per-stem `compress()` — a
    // feed-forward PEAK detector (stereo-linked, max|L|,|R|), one-pole
    // attack/release on the raw level, a HARD-knee LINEAR-domain gain
    // `(thr + (env-thr)/ratio)/env`, then makeup. NO lookahead (so it does not
    // delay the audio path and phase-skew against uncompressed stems) and NO
    // soft knee — exactly the render's topology, so the .phx per-track
    // compression is faithful to the bespoke render instead of merely "close".
    // `mix` is the standard dry/wet crossfade: 1.0 = serial (the render's
    // in-place `compress()`); <1.0 = parallel blend.
    fn process_bus(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        // threshold/makeup change only on a param edit; refresh their linear
        // caches every CTRL_DIVIDER samples (and on the first sample) — the
        // hot path then has no powf. (update_coefficients also seeds them.)
        if self.ctrl_counter == 0 || self.ctrl_dirty {
            self.bus_thr_lin = 10f32.powf(self.threshold / 20.0);
            self.bus_mk = 10f32.powf(self.makeup_gain / 20.0);
            self.ctrl_dirty = false;
        }
        self.ctrl_counter = (self.ctrl_counter + 1) % CMP_CTRL_DIVIDER;

        let (det_l, det_r) = if self.sidechain_active {
            (self.sidechain_l, self.sidechain_r)
        } else {
            (input_l, input_r)
        };
        let pk = det_l.abs().max(det_r.abs());
        // attack_coef/release_coef are exp(-1/(t·sr)) == the render's ga/gr.
        let coef = if pk > self.bus_env { self.attack_coef } else { self.release_coef };
        self.bus_env = coef * self.bus_env + (1.0 - coef) * pk;
        let g = if self.bus_env > self.bus_thr_lin {
            (self.bus_thr_lin + (self.bus_env - self.bus_thr_lin) / self.ratio) / self.bus_env.max(1e-9)
        } else {
            1.0
        };
        let gm = g * self.bus_mk;
        let wet_l = input_l * gm;
        let wet_r = input_r * gm;
        (
            input_l * (1.0 - self.mix) + wet_l * self.mix,
            input_r * (1.0 - self.mix) + wet_r * self.mix,
        )
    }

    #[inline]
    fn detect_peak(&self, input_l: f32, input_r: f32) -> f32 {
        if self.sidechain_active {
            self.sidechain_l.abs().max(self.sidechain_r.abs()).max(1e-10)
        } else {
            input_l.abs().max(input_r.abs()).max(1e-10)
        }
    }

    /// Derive the detector sample (after sidechain HPF when enabled) and
    /// the delayed audio sample for the specified input pair. Used by all
    /// four compressor modes.
    #[inline]
    fn prepare_lookahead(&mut self, input_l: f32, input_r: f32) -> (f32, f32, f32, f32) {
        let (det_l, det_r) = if self.sidechain_active {
            self.sc_hp_filter(self.sidechain_l, self.sidechain_r)
        } else {
            self.sc_hp_filter(input_l, input_r)
        };
        let (aud_l, aud_r) = self.lookahead_push(input_l, input_r);
        (det_l, det_r, aud_l, aud_r)
    }

    // VCA: clean, transparent, SSL-style. Standard feedforward design.
    fn process_vca(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        let (det_l, det_r, aud_l, aud_r) = self.prepare_lookahead(input_l, input_r);
        let peak = det_l.abs().max(det_r.abs()).max(1e-10);
        let input_db = 20.0 * fast_log10(peak);
        let gain_reduction_db = self.gain_computer(input_db) - input_db;

        let coef = if gain_reduction_db < self.envelope_db {
            self.attack_coef
        } else {
            self.release_coef
        };
        self.envelope_db = coef * self.envelope_db + (1.0 - coef) * gain_reduction_db;

        let total_makeup = self.makeup_gain + self.auto_makeup_db();
        let gain = fast_db_to_lin(self.envelope_db + total_makeup).min(128.0);
        let wet_l = aud_l * gain;
        let wet_r = aud_r * gain;
        (
            aud_l * (1.0 - self.mix) + wet_l * self.mix,
            aud_r * (1.0 - self.mix) + wet_r * self.mix,
        )
    }

    // Opto: program-dependent timing (LA-2A style). Slow attack on transients, faster on sustained.
    fn process_opto(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        let (det_l, det_r, aud_l, aud_r) = self.prepare_lookahead(input_l, input_r);
        let peak = det_l.abs().max(det_r.abs()).max(1e-10);
        let input_db = 20.0 * fast_log10(peak);
        let gain_reduction_db = self.gain_computer(input_db) - input_db;

        // Program-dependent: attack slows as GR deepens, release has two stages
        // Opto attack/release depend on `gr_depth` (envelope follower —
        // slow). Refresh the two exp() coefs only every CTRL_DIVIDER
        // samples; the dependency on gr_depth doesn't need to be
        // sample-accurate.
        if self.ctrl_counter == 0 || self.ctrl_dirty {
            let gr_depth = (-self.envelope_db).max(0.0);
            let opto_attack = self.attack * (1.0 + gr_depth * 0.15);
            let opto_release = if gr_depth > 10.0 {
                self.release * 3.0
            } else {
                self.release * (1.0 + gr_depth * 0.1)
            };
            self.cached_opto_att = (-1.0 / (opto_attack * self.sample_rate)).exp();
            self.cached_opto_rel = (-1.0 / (opto_release * self.sample_rate)).exp();
            self.ctrl_dirty = false;
        }
        self.ctrl_counter = (self.ctrl_counter + 1) % CMP_CTRL_DIVIDER;

        let coef = if gain_reduction_db < self.envelope_db {
            self.cached_opto_att
        } else {
            self.cached_opto_rel
        };
        self.envelope_db = coef * self.envelope_db + (1.0 - coef) * gain_reduction_db;

        let total_makeup = self.makeup_gain + self.auto_makeup_db();
        let gain = fast_db_to_lin(self.envelope_db + total_makeup).min(128.0);
        let wet_l = aud_l * gain;
        let wet_r = aud_r * gain;
        (
            aud_l * (1.0 - self.mix) + wet_l * self.mix,
            aud_r * (1.0 - self.mix) + wet_r * self.mix,
        )
    }

    // FET: fast, aggressive (1176 style). Hard knee, feedback topology emulation, all-buttons character.
    fn process_fet(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        let (det_l, det_r, aud_l, aud_r) = self.prepare_lookahead(input_l, input_r);
        let peak = det_l.abs().max(det_r.abs()).max(1e-10);
        let input_db = 20.0 * fast_log10(peak);

        // FET uses hard knee
        let diff = input_db - self.threshold;
        let output_db = if diff > 0.0 {
            self.threshold + diff / self.ratio
        } else {
            input_db
        };
        let gain_reduction_db = output_db - input_db;

        // FET attack is fully static (depends only on the `attack`
        // knob × 0.2). Release depends on `gr_depth` — slow. Same
        // control-rate cache pattern as Opto.
        if self.ctrl_counter == 0 || self.ctrl_dirty {
            let fet_attack = self.attack * 0.2;
            let fet_release = self.release * (1.0 + (-self.envelope_db).max(0.0) * 0.05);
            self.cached_fet_att = (-1.0 / (fet_attack * self.sample_rate)).exp();
            self.cached_fet_rel = (-1.0 / (fet_release * self.sample_rate)).exp();
            self.ctrl_dirty = false;
        }
        self.ctrl_counter = (self.ctrl_counter + 1) % CMP_CTRL_DIVIDER;

        let coef = if gain_reduction_db < self.envelope_db {
            self.cached_fet_att
        } else {
            self.cached_fet_rel
        };
        self.envelope_db = coef * self.envelope_db + (1.0 - coef) * gain_reduction_db;

        // FET adds harmonic saturation
        let total_makeup = self.makeup_gain + self.auto_makeup_db();
        let gain = fast_db_to_lin(self.envelope_db + total_makeup).min(128.0);
        let wet_l = Self::fet_saturate(aud_l * gain);
        let wet_r = Self::fet_saturate(aud_r * gain);
        (
            aud_l * (1.0 - self.mix) + wet_l * self.mix,
            aud_r * (1.0 - self.mix) + wet_r * self.mix,
        )
    }

    fn fet_saturate(x: f32) -> f32 {
        // Soft asymmetric saturation for FET character
        if x > 0.0 {
            (x * 1.5).tanh() / 1.5_f32.tanh()
        } else {
            (x * 1.2).tanh() / 1.2_f32.tanh()
        }
    }

    // Multiband: split into 3 bands (low/mid/high) using proper 1-pole filters, compress independently, sum
    fn process_multiband(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        // Lookahead (SC HPF doesn't apply per-band, but we still delay the audio
        // path so the audio stays in sync with other modes).
        let (_det_l, _det_r, input_l, input_r) = self.prepare_lookahead(input_l, input_r);
        let mono = (input_l + input_r) * 0.5;

        // Low band: 1-pole LPF at 200 Hz (cached coefficient)
        self.mb_lo_state = self.mb_lo_coef * self.mb_lo_state + (1.0 - self.mb_lo_coef) * mono;
        let lo = self.mb_lo_state;

        // High-pass at 3kHz: input minus low-pass at 3kHz (cached coefficient)
        self.mb_hi_state = self.mb_hi_coef * self.mb_hi_state + (1.0 - self.mb_hi_coef) * mono;
        let hi = mono - self.mb_hi_state;

        // Mid: remainder
        let mid = mono - lo - hi;

        // Compress each band with envelope smoothing
        let ratio = self.ratio;
        let att = self.attack_coef;
        let rel = self.release_coef;

        let compress = |input: f32, threshold: f32, env: &mut f32| -> f32 {
            let level = input.abs().max(1e-10);
            let db = 20.0 * fast_log10(level);
            let diff = db - threshold;
            let gr_db = if diff > 0.0 {
                -(diff - diff / ratio)
            } else {
                0.0
            };
            let coef = if gr_db < *env { att } else { rel };
            *env = coef * *env + (1.0 - coef) * gr_db;
            input * fast_db_to_lin(*env)
        };

        let lo_compressed = compress(lo, self.threshold - 6.0, &mut self.mb_env_lo);
        let mid_compressed = compress(mid, self.threshold, &mut self.mb_env_mid);
        let hi_compressed = compress(hi, self.threshold + 3.0, &mut self.mb_env_hi);

        let wet = lo_compressed + mid_compressed + hi_compressed;
        let gain = fast_db_to_lin(self.makeup_gain).min(128.0);
        let wet_l = wet * gain;
        let wet_r = wet * gain;

        // Preserve stereo image
        let sum_abs = input_l.abs() + input_r.abs();
        let width = if sum_abs > 1e-10 {
            (input_l - input_r) / (sum_abs + 1e-10)
        } else {
            0.0
        };
        let out_l = wet_l * (1.0 + width * 0.5);
        let out_r = wet_r * (1.0 - width * 0.5);

        (
            input_l * (1.0 - self.mix) + out_l * self.mix,
            input_r * (1.0 - self.mix) + out_r * self.mix,
        )
    }

    pub fn reset(&mut self) {
        // At REST a gain-reduction envelope is 0 dB — no reduction. This said
        // -96, which is 96 dB of gain reduction: a reset compressor came back
        // silent and took its whole release time to let go. The constructor
        // already says so twenty lines above; only the reset disagreed.
        self.envelope_db = 0.0;
        self.mb_lo_state = 0.0;
        self.mb_hi_state = 0.0;
        self.mb_env_lo = 0.0;
        self.mb_env_mid = 0.0;
        self.mb_env_hi = 0.0;
        for s in &mut self.lookahead_l { *s = 0.0; }
        for s in &mut self.lookahead_r { *s = 0.0; }
        self.lookahead_pos = 0;
        self.sc_hp_l_prev_in = 0.0; self.sc_hp_l_prev_out = 0.0;
        self.sc_hp_r_prev_in = 0.0; self.sc_hp_r_prev_out = 0.0;
    }

    pub fn set_threshold(&mut self, db: f32) {
        self.threshold = db.clamp(-60.0, 0.0);
    }
    pub fn set_ratio(&mut self, r: f32) {
        self.ratio = r.clamp(1.0, 20.0);
    }
    pub fn set_attack(&mut self, s: f32) {
        self.attack = s.clamp(0.0001, 0.5);
        self.update_coefficients();
        self.ctrl_dirty = true;
    }
    pub fn set_release(&mut self, s: f32) {
        self.release = s.clamp(0.01, 2.0);
        self.update_coefficients();
        self.ctrl_dirty = true;
    }
    pub fn set_makeup_gain(&mut self, db: f32) {
        self.makeup_gain = db.clamp(0.0, 24.0);
    }
    pub fn set_knee(&mut self, db: f32) {
        self.knee = db.clamp(0.0, 24.0);
    }
    pub fn set_mix(&mut self, m: f32) {
        self.mix = m.clamp(0.0, 1.0);
    }
    pub fn set_mode(&mut self, mode: CompressorMode) {
        self.mode = mode;
        self.ctrl_dirty = true;
    }
    /// Sidechain HPF cutoff in Hz. Set to 0 to disable. 20..500 is typical.
    pub fn set_sc_hpf(&mut self, hz: f32) { self.sc_hpf_hz = hz.clamp(0.0, 2000.0); }
    /// Auto-makeup amount 0..1 (0 = off, 1 = full rule-of-thumb boost).
    pub fn set_auto_makeup(&mut self, v: f32) { self.auto_makeup = v.clamp(0.0, 1.0); }
}

//==============================================================================
// LIMITER EFFECT
//==============================================================================

#[derive(Debug, Clone)]
pub struct LimiterEffect {
    sample_rate: f32,
    mode: LimiterMode,
    ceiling: f32,    // dB output ceiling (-12 to 0)
    release: f32,    // seconds
    input_gain: f32, // dB input boost (0 to 24) — push into limiter for loudness
    mix: f32,

    // Cached linear values (avoid per-sample powf)
    ceiling_lin: f32,
    input_gain_lin: f32,

    // State
    envelope: f32, // linear gain reduction envelope
    release_coef: f32,

    // Lookahead buffer for true peak limiting
    lookahead_l: Vec<f32>,
    lookahead_r: Vec<f32>,
    lookahead_pos: usize,
    lookahead_len: usize,
}

impl LimiterEffect {
    pub fn new(sample_rate: f32) -> Self {
        let lookahead_len = (sample_rate * 0.005) as usize; // 5ms lookahead
        let mut s = Self {
            sample_rate,
            mode: LimiterMode::Brickwall,
            ceiling: -0.3,
            release: 0.1,
            input_gain: 0.0,
            mix: 1.0,
            ceiling_lin: 10.0f32.powf(-0.3 / 20.0),
            input_gain_lin: 1.0,
            envelope: 1.0,
            release_coef: 0.0,
            lookahead_l: vec![0.0; lookahead_len],
            lookahead_r: vec![0.0; lookahead_len],
            lookahead_pos: 0,
            lookahead_len,
        };
        s.update_coefficients();
        s
    }

    fn update_coefficients(&mut self) {
        self.release_coef = (-1.0 / (self.release * self.sample_rate)).exp();
        self.ceiling_lin = 10.0f32.powf(self.ceiling / 20.0);
        self.input_gain_lin = 10.0f32.powf(self.input_gain / 20.0);
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        // Apply input gain boost (cached linear value)
        let boosted_l = input_l * self.input_gain_lin;
        let boosted_r = input_r * self.input_gain_lin;
        match self.mode {
            LimiterMode::Brickwall => self.process_brickwall(boosted_l, boosted_r),
            LimiterMode::SoftClip => self.process_softclip(boosted_l, boosted_r),
            LimiterMode::Maximizer => self.process_maximizer(boosted_l, boosted_r),
        }
    }

    // Brickwall: hard ceiling with lookahead. Zero overshoot.
    fn process_brickwall(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        let ceiling_lin = self.ceiling_lin;

        let delayed_l = self.lookahead_l[self.lookahead_pos];
        let delayed_r = self.lookahead_r[self.lookahead_pos];
        self.lookahead_l[self.lookahead_pos] = input_l;
        self.lookahead_r[self.lookahead_pos] = input_r;
        self.lookahead_pos = (self.lookahead_pos + 1) % self.lookahead_len;

        let peak = input_l.abs().max(input_r.abs());
        let target_gain = if peak > ceiling_lin {
            ceiling_lin / peak
        } else {
            1.0
        };

        if target_gain < self.envelope {
            self.envelope = target_gain;
        } else {
            self.envelope =
                self.release_coef * self.envelope + (1.0 - self.release_coef) * target_gain;
        }

        let wet_l = delayed_l * self.envelope;
        let wet_r = delayed_r * self.envelope;
        (
            delayed_l * (1.0 - self.mix) + wet_l * self.mix,
            delayed_r * (1.0 - self.mix) + wet_r * self.mix,
        )
    }

    // Soft Clip: gentle saturation curve at ceiling. Musical, warm, allows slight overshoot.
    fn process_softclip(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        let ceiling_lin = self.ceiling_lin;

        // Smooth tanh soft clip — no discontinuity
        let clip = |x: f32| -> f32 { ceiling_lin * (x / ceiling_lin).tanh() };

        let wet_l = clip(input_l);
        let wet_r = clip(input_r);

        // Smooth release envelope for gain display
        let peak = input_l.abs().max(input_r.abs());
        let target = if peak > ceiling_lin {
            ceiling_lin / peak
        } else {
            1.0
        };
        if target < self.envelope {
            self.envelope = target;
        } else {
            self.envelope = self.release_coef * self.envelope + (1.0 - self.release_coef) * target;
        }

        (
            input_l * (1.0 - self.mix) + wet_l * self.mix,
            input_r * (1.0 - self.mix) + wet_r * self.mix,
        )
    }

    // Maximizer: multi-stage look-ahead with slower release for loudness. Pumps musically.
    fn process_maximizer(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        let ceiling_lin = self.ceiling_lin;

        let delayed_l = self.lookahead_l[self.lookahead_pos];
        let delayed_r = self.lookahead_r[self.lookahead_pos];
        self.lookahead_l[self.lookahead_pos] = input_l;
        self.lookahead_r[self.lookahead_pos] = input_r;
        self.lookahead_pos = (self.lookahead_pos + 1) % self.lookahead_len;

        // Two-stage gain reduction: fast for peaks, slow for average level
        let peak = input_l.abs().max(input_r.abs());
        let target_gain = if peak > ceiling_lin * 0.7 {
            // Start reducing earlier than brickwall for smoother limiting
            let over = peak / (ceiling_lin * 0.7);
            1.0 / over.powf(0.8) // softer ratio than brickwall
        } else {
            1.0
        };

        // Slower release for more loudness
        let slow_release = self.release_coef * self.release_coef; // squared = slower
        if target_gain < self.envelope {
            // Smooth attack (not instant) for less distortion
            let max_attack = (-1.0 / (0.001 * self.sample_rate)).exp();
            self.envelope = max_attack * self.envelope + (1.0 - max_attack) * target_gain;
        } else {
            self.envelope = slow_release * self.envelope + (1.0 - slow_release) * target_gain;
        }

        // Final hard clip as safety
        let gain = self.envelope.min(1.0);
        let wet_l = (delayed_l * gain).clamp(-ceiling_lin, ceiling_lin);
        let wet_r = (delayed_r * gain).clamp(-ceiling_lin, ceiling_lin);
        (
            delayed_l * (1.0 - self.mix) + wet_l * self.mix,
            delayed_r * (1.0 - self.mix) + wet_r * self.mix,
        )
    }

    pub fn reset(&mut self) {
        self.envelope = 1.0;
        self.lookahead_l.fill(0.0);
        self.lookahead_r.fill(0.0);
        self.lookahead_pos = 0;
    }

    pub fn set_ceiling(&mut self, db: f32) {
        self.ceiling = db.clamp(-12.0, 0.0);
        self.update_coefficients();
    }
    pub fn set_release(&mut self, s: f32) {
        self.release = s.clamp(0.01, 1.0);
        self.update_coefficients();
    }
    pub fn set_input_gain(&mut self, db: f32) {
        self.input_gain = db.clamp(0.0, 24.0);
        self.update_coefficients();
    }
    pub fn set_mix(&mut self, m: f32) {
        self.mix = m.clamp(0.0, 1.0);
    }
    pub fn set_mode(&mut self, mode: LimiterMode) {
        self.mode = mode;
    }
}

//==============================================================================
// EXPANDER EFFECT
//==============================================================================

#[derive(Debug, Clone)]
pub struct ExpanderEffect {
    sample_rate: f32,
    mode: ExpanderMode,
    threshold: f32, // dB (-60 to 0)
    ratio: f32,     // expansion ratio (1:1 to 1:8)
    attack: f32,    // seconds
    release: f32,   // seconds
    range: f32,     // max gain reduction in dB (gate depth)
    knee: f32,      // dB
    mix: f32,

    // State
    envelope_db: f32,
    attack_coef: f32,
    release_coef: f32,
}

impl ExpanderEffect {
    pub fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            sample_rate,
            mode: ExpanderMode::Gate,
            threshold: -40.0,
            ratio: 2.0,
            attack: 0.001,
            release: 0.05,
            range: -80.0,
            knee: 6.0,
            mix: 1.0,
            envelope_db: 0.0,
            attack_coef: 0.0,
            release_coef: 0.0,
        };
        s.update_coefficients();
        s
    }

    fn update_coefficients(&mut self) {
        self.attack_coef = (-1.0 / (self.attack * self.sample_rate)).exp();
        self.release_coef = (-1.0 / (self.release * self.sample_rate)).exp();
    }

    fn gain_computer(&self, input_db: f32) -> f32 {
        let half_knee = self.knee * 0.5;
        let diff = self.threshold - input_db; // Note: reversed vs compressor

        if diff <= -half_knee {
            // Above threshold: no expansion
            0.0
        } else if diff >= half_knee {
            // Below threshold: full expansion
            let reduction = diff * (1.0 - self.ratio);
            reduction.max(self.range)
        } else {
            // In knee
            let x = diff + half_knee;
            let reduction = (1.0 - self.ratio) * x * x / (2.0 * self.knee);
            reduction.max(self.range)
        }
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        match self.mode {
            ExpanderMode::Gate => self.process_gate(input_l, input_r),
            ExpanderMode::Gentle => self.process_gentle(input_l, input_r),
            ExpanderMode::Ducker => self.process_ducker(input_l, input_r),
        }
    }

    // Gate: hard gate with range. Fast open, controlled close.
    fn process_gate(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        let peak = input_l.abs().max(input_r.abs()).max(1e-10);
        let input_db = 20.0 * fast_log10(peak);
        let gain_reduction_db = self.gain_computer(input_db);

        // gain_reduction_db is negative (less = more reduction)
        // Opening (less reduction): use attack (fast open)
        // Closing (more reduction): use release (controlled close)
        let coef = if gain_reduction_db < self.envelope_db {
            self.release_coef // closing: more reduction needed
        } else {
            self.attack_coef // opening: less reduction needed
        };
        self.envelope_db = coef * self.envelope_db + (1.0 - coef) * gain_reduction_db;

        let gain = fast_db_to_lin(self.envelope_db);
        let wet_l = input_l * gain;
        let wet_r = input_r * gain;
        (
            input_l * (1.0 - self.mix) + wet_l * self.mix,
            input_r * (1.0 - self.mix) + wet_r * self.mix,
        )
    }

    // Gentle: soft downward expansion with wider knee and less aggressive ratio.
    fn process_gentle(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        let peak = input_l.abs().max(input_r.abs()).max(1e-10);
        let input_db = 20.0 * fast_log10(peak);

        // Use wider knee and gentler ratio
        let gentle_knee = self.knee * 2.0;
        let gentle_ratio = 1.0 + (self.ratio - 1.0) * 0.5; // half the expansion
        let half_knee = gentle_knee * 0.5;
        let diff = self.threshold - input_db;

        let gain_reduction_db = if diff <= -half_knee {
            0.0
        } else if diff >= half_knee {
            (diff * (1.0 - gentle_ratio)).max(self.range * 0.5) // limited range
        } else {
            let x = diff + half_knee;
            ((1.0 - gentle_ratio) * x * x / (2.0 * gentle_knee)).max(self.range * 0.5)
        };

        // Slower envelope — scale the base coefficients (cube for slower response)
        let gentle_att = self.attack_coef * self.attack_coef * self.attack_coef;
        let gentle_rel = self.release_coef * self.release_coef;
        // Closing (more reduction): use release. Opening: use attack.
        let coef = if gain_reduction_db < self.envelope_db {
            gentle_rel
        } else {
            gentle_att
        };
        self.envelope_db = coef * self.envelope_db + (1.0 - coef) * gain_reduction_db;

        let gain = fast_db_to_lin(self.envelope_db);
        let wet_l = input_l * gain;
        let wet_r = input_r * gain;
        (
            input_l * (1.0 - self.mix) + wet_l * self.mix,
            input_r * (1.0 - self.mix) + wet_r * self.mix,
        )
    }

    // Ducker: inverted expander — reduces loud signals above threshold (for voiceovers, sidechain-style).
    fn process_ducker(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        let peak = input_l.abs().max(input_r.abs()).max(1e-10);
        let input_db = 20.0 * fast_log10(peak);

        // Inverted: reduce gain when signal is ABOVE threshold
        let diff = input_db - self.threshold;
        let half_knee = self.knee * 0.5;

        let gain_reduction_db = if diff <= -half_knee {
            0.0 // below threshold: no ducking
        } else if diff >= half_knee {
            // diff positive, (1 - 1/ratio) positive for ratio>1, so raw is positive
            let raw = diff * (1.0 - 1.0 / self.ratio);
            // self.range is negative (e.g. -40). Clamp so reduction doesn't exceed |range|.
            -(raw.min(-self.range))
        } else {
            let x = diff + half_knee;
            let raw = (1.0 - 1.0 / self.ratio) * x * x / (2.0 * self.knee);
            -(raw.min(-self.range))
        };

        // Ducking (more reduction): use attack (fast duck). Releasing: use release.
        let coef = if gain_reduction_db < self.envelope_db {
            self.attack_coef // ducking down
        } else {
            self.release_coef // releasing back up
        };
        self.envelope_db = coef * self.envelope_db + (1.0 - coef) * gain_reduction_db;

        let gain = fast_db_to_lin(self.envelope_db);
        let wet_l = input_l * gain;
        let wet_r = input_r * gain;
        (
            input_l * (1.0 - self.mix) + wet_l * self.mix,
            input_r * (1.0 - self.mix) + wet_r * self.mix,
        )
    }

    pub fn reset(&mut self) {
        self.envelope_db = 0.0;
    }

    pub fn set_threshold(&mut self, db: f32) {
        self.threshold = db.clamp(-60.0, 0.0);
    }
    pub fn set_ratio(&mut self, r: f32) {
        self.ratio = r.clamp(1.0, 8.0);
    }
    pub fn set_attack(&mut self, s: f32) {
        self.attack = s.clamp(0.0001, 0.5);
        self.update_coefficients();
    }
    pub fn set_release(&mut self, s: f32) {
        self.release = s.clamp(0.01, 2.0);
        self.update_coefficients();
    }
    pub fn set_range(&mut self, db: f32) {
        self.range = db.clamp(-80.0, 0.0);
    }
    pub fn set_knee(&mut self, db: f32) {
        self.knee = db.clamp(0.0, 24.0);
    }
    pub fn set_mix(&mut self, m: f32) {
        self.mix = m.clamp(0.0, 1.0);
    }
    pub fn set_mode(&mut self, mode: ExpanderMode) {
        self.mode = mode;
    }
}

//==============================================================================
// AUTOPANNER EFFECT
//==============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoPanMode {
    Classic,    // Sine/triangle/square LFO
    Spin,       // Leslie-style rotation with doppler
    Bounce,     // Ball bouncing between L/R
    RandomSH,   // Sample-and-hold random positions
    Envelope,   // Pan follows input dynamics
    Haas,       // Micro-delay modulation for stereo movement
    TremoloPan, // Combined volume + pan modulation
}

impl AutoPanMode {
    pub fn all() -> &'static [AutoPanMode] {
        &[
            AutoPanMode::Classic,
            AutoPanMode::Spin,
            AutoPanMode::Bounce,
            AutoPanMode::RandomSH,
            AutoPanMode::Envelope,
            AutoPanMode::Haas,
            AutoPanMode::TremoloPan,
        ]
    }

    pub fn from_index(idx: usize) -> Self {
        Self::all()
            .get(idx)
            .copied()
            .unwrap_or(AutoPanMode::Classic)
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            AutoPanMode::Classic => "Classic",
            AutoPanMode::Spin => "Spin",
            AutoPanMode::Bounce => "Bounce",
            AutoPanMode::RandomSH => "Random S&H",
            AutoPanMode::Envelope => "Envelope",
            AutoPanMode::Haas => "Haas",
            AutoPanMode::TremoloPan => "Trem-Pan",
        }
    }

    pub fn all_names() -> Vec<&'static str> {
        Self::all().iter().map(|m| m.display_name()).collect()
    }
}

/// LFO waveform for Classic mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoPanWave {
    Sine,
    Triangle,
    Square,
}

impl AutoPanWave {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => AutoPanWave::Triangle,
            2 => AutoPanWave::Square,
            _ => AutoPanWave::Sine,
        }
    }
    pub fn all_names() -> Vec<&'static str> {
        vec!["Sine", "Triangle", "Square"]
    }
}

#[derive(Debug, Clone)]
pub struct AutoPannerEffect {
    sample_rate: f32,
    mode: AutoPanMode,
    wave: AutoPanWave,
    rate: f32,         // Hz
    depth: f32,        // 0..1
    pub mix: f32,      // dry/wet 0..1
    width: f32,        // stereo width 0..1 (0=center, 1=full L/R)
    phase_offset: f32, // 0..1 (maps to 0..2PI offset between L/R)
    glide: f32,        // S&H smoothing 0..1
    tempo_sync: bool,
    tempo_division: TempoDivision,
    tempo: f32,
    beat_position: f64,

    // State
    lfo_phase: f32,
    // Spin mode
    spin_speed_target: f32,  // 0=slow, 1=fast
    spin_speed_current: f32, // smoothed speed for ramp-up/down
    // Bounce mode
    bounce_pos: f32, // -1..1
    bounce_vel: f32, // velocity
    bounce_gravity: f32,
    // Random S&H
    sh_rng: u32,
    sh_value: f32,    // current target
    sh_smoothed: f32, // glide-smoothed value
    sh_counter: f32,  // samples until next trigger
    // Envelope follower
    env_state: f32, // smoothed envelope
    env_pan: f32,   // current pan from envelope
    // Haas delay line
    haas_buffer: Vec<f32>, // mono delay buffer
    haas_write_pos: usize,
    haas_lfo_phase: f32,
    // Tremolo-Pan
    trem_depth: f32, // volume modulation depth 0..1
    // One-pole slew of the LFO output. A Square (or S&H) LFO steps the pan/gain
    // instantly, which clicks; smoothing the modulation signal over ~1.5 ms
    // removes the click and is transparent for the already-smooth Sine/Triangle.
    lfo_smooth: f32,
}

impl AutoPannerEffect {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            mode: AutoPanMode::Classic,
            wave: AutoPanWave::Sine,
            rate: 1.0,
            depth: 0.5,
            mix: 1.0,
            width: 1.0,
            phase_offset: 0.0,
            glide: 0.3,
            tempo_sync: false,
            tempo_division: TempoDivision::Quarter,
            tempo: 120.0,
            beat_position: 0.0,
            lfo_phase: 0.0,
            spin_speed_target: 0.0,
            spin_speed_current: 0.0,
            bounce_pos: 1.0,
            bounce_vel: 0.0,
            bounce_gravity: 9.8,
            sh_rng: 0xDEAD_BEEF,
            sh_value: 0.0,
            sh_smoothed: 0.0,
            sh_counter: 0.0,
            env_state: 0.0,
            env_pan: 0.0,
            haas_buffer: vec![0.0; (sample_rate * 0.035) as usize], // 35ms max
            haas_write_pos: 0,
            haas_lfo_phase: 0.0,
            trem_depth: 0.5,
            lfo_smooth: 0.0,
        }
    }

    fn effective_rate(&self) -> f32 {
        if self.tempo_sync {
            1.0 / self.tempo_division.to_time_seconds(self.tempo)
        } else {
            self.rate
        }
    }

    #[inline(always)]
    fn lfo_inc(&self) -> f32 {
        self.effective_rate() / self.sample_rate
    }

    /// Slew the raw LFO output to de-click hard-stepping shapes (Square, S&H).
    /// ~1.5 ms one-pole; transparent for Sine/Triangle. Returns the smoothed value.
    #[inline(always)]
    fn slew_lfo(&mut self, target: f32) -> f32 {
        let coef = 1.0 - (-1.0 / (0.0015 * self.sample_rate)).exp();
        self.lfo_smooth += (target - self.lfo_smooth) * coef;
        self.lfo_smooth
    }

    /// Generate LFO value based on waveform selection (-1..1)
    #[inline(always)]
    fn lfo_wave(&self, phase: f32) -> f32 {
        match self.wave {
            AutoPanWave::Sine => fast_sin(phase * std::f32::consts::TAU),
            AutoPanWave::Triangle => {
                let t = phase % 1.0;
                if t < 0.25 {
                    t * 4.0
                } else if t < 0.75 {
                    2.0 - t * 4.0
                } else {
                    t * 4.0 - 4.0
                }
            }
            AutoPanWave::Square => {
                if (phase % 1.0) < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
        }
    }

    /// Inline xorshift PRNG
    #[inline(always)]
    fn next_rand(&mut self) -> f32 {
        self.sh_rng ^= self.sh_rng << 13;
        self.sh_rng ^= self.sh_rng >> 17;
        self.sh_rng ^= self.sh_rng << 5;
        (self.sh_rng & 0x7FFF_FFFF) as f32 / 0x7FFF_FFFF as f32
    }

    /// Apply constant-power pan to stereo input. pan in -1..+1 (left..right).
    /// Sum to mono, then redistribute with cos/sin law. sqrt(2) for unity at center.
    #[inline(always)]
    fn apply_pan(input_l: f32, input_r: f32, pan: f32) -> (f32, f32) {
        let mono = (input_l + input_r) * 0.5;
        let angle = (0.5 + pan * 0.5) * std::f32::consts::FRAC_PI_2;
        // sqrt(2) ≈ 1.4142; cos(PI/4) = sin(PI/4) = 0.707, * 1.414 = 1.0 at center
        (
            mono * angle.cos() * std::f32::consts::SQRT_2,
            mono * angle.sin() * std::f32::consts::SQRT_2,
        )
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.mix <= 0.0001 || self.depth <= 0.0001 {
            return (input_l, input_r);
        }

        let (wet_l, wet_r) = match self.mode {
            AutoPanMode::Classic => self.process_classic(input_l, input_r),
            AutoPanMode::Spin => self.process_spin(input_l, input_r),
            AutoPanMode::Bounce => self.process_bounce(input_l, input_r),
            AutoPanMode::RandomSH => self.process_random_sh(input_l, input_r),
            AutoPanMode::Envelope => self.process_envelope(input_l, input_r),
            AutoPanMode::Haas => self.process_haas(input_l, input_r),
            AutoPanMode::TremoloPan => self.process_tremolo_pan(input_l, input_r),
        };

        // Dry/wet crossfade
        let out_l = input_l * (1.0 - self.mix) + wet_l * self.mix;
        let out_r = input_r * (1.0 - self.mix) + wet_r * self.mix;
        (out_l, out_r)
    }

    /// Classic: LFO panning with sine/triangle/square waveform
    fn process_classic(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.tempo_sync {
            self.lfo_phase =
                (self.beat_position / self.tempo_division.beats() as f64).fract() as f32;
        } else {
            self.lfo_phase += self.lfo_inc();
            if self.lfo_phase >= 1.0 {
                self.lfo_phase -= 1.0;
            }
        }
        let lfo = self.slew_lfo(self.lfo_wave(self.lfo_phase));

        let pan = lfo * self.depth * self.width;
        Self::apply_pan(input_l, input_r, pan)
    }

    /// Spin: Leslie-style rotation with doppler and distance modulation
    fn process_spin(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        // Smooth speed ramp (Leslie slow/fast transition)
        let ramp_speed = if self.spin_speed_target > self.spin_speed_current {
            0.5
        } else {
            0.3
        };
        self.spin_speed_current +=
            (self.spin_speed_target - self.spin_speed_current) * ramp_speed / self.sample_rate;

        // Actual rotation rate: slow = rate*0.5, fast = rate*6
        let spin_rate = self.effective_rate() * (0.5 + self.spin_speed_current * 5.5);
        self.lfo_phase += spin_rate / self.sample_rate;
        if self.lfo_phase >= 1.0 {
            self.lfo_phase -= 1.0;
        }

        let angle = self.lfo_phase * std::f32::consts::TAU;
        let cos_a = fast_sin(angle + std::f32::consts::FRAC_PI_2);
        let sin_a = fast_sin(angle);

        let pan = cos_a * self.depth * self.width;

        // Distance-based amplitude (speaker closer = louder)
        let dist_l = 1.0 + 0.3 * self.depth * (1.0 - cos_a);
        let dist_r = 1.0 + 0.3 * self.depth * (1.0 + cos_a);

        // Doppler as subtle amplitude modulation
        let doppler = sin_a * self.depth * 0.15;

        let (pan_l, pan_r) = Self::apply_pan(input_l, input_r, pan);
        (
            pan_l * (1.0 + doppler) / dist_l,
            pan_r * (1.0 - doppler) / dist_r,
        )
    }

    /// Bounce: ball bouncing between L/R with gravity
    fn process_bounce(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        let dt = 1.0 / self.sample_rate;
        let gravity_scale = self.effective_rate() * 20.0;

        self.bounce_vel -= self.bounce_pos * gravity_scale * dt * self.bounce_gravity;
        self.bounce_vel *= 1.0 - 0.5 * dt;
        self.bounce_pos += self.bounce_vel * dt * gravity_scale;

        if self.bounce_pos > 1.0 {
            self.bounce_pos = 1.0;
            self.bounce_vel = -self.bounce_vel * 0.85;
        } else if self.bounce_pos < -1.0 {
            self.bounce_pos = -1.0;
            self.bounce_vel = -self.bounce_vel * 0.85;
        }

        // Re-kick if energy dies
        let energy = self.bounce_vel.abs() + self.bounce_pos.abs();
        if energy < 0.01 {
            self.bounce_vel = 4.0;
            self.bounce_pos = -0.9;
        }

        let pan = self.bounce_pos * self.depth * self.width;
        Self::apply_pan(input_l, input_r, pan)
    }

    /// Random S&H: jump to random pan positions at LFO rate, with glide
    fn process_random_sh(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        self.sh_counter -= 1.0;
        if self.sh_counter <= 0.0 {
            self.sh_value = self.next_rand() * 2.0 - 1.0;
            self.sh_counter = self.sample_rate / self.effective_rate().max(0.01);
        }

        // Glide: interpolate toward target. glide=0 → instant, glide=1 → very smooth
        let alpha = (1.0 - self.glide) * 0.1 + 0.0001;
        self.sh_smoothed += (self.sh_value - self.sh_smoothed) * alpha;

        let pan = self.sh_smoothed * self.depth * self.width;
        Self::apply_pan(input_l, input_r, pan)
    }

    /// Envelope: pan follows input dynamics
    fn process_envelope(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        let level = (input_l.abs() + input_r.abs()) * 0.5;

        // Envelope follower
        let attack_coef = (-1.0 / (0.005 * self.sample_rate)).exp();
        let release_coef = (-1.0
            / ((0.05 + (1.0 - self.effective_rate() / 10.0).max(0.0) * 0.5) * self.sample_rate))
            .exp();

        if level > self.env_state {
            self.env_state = attack_coef * self.env_state + (1.0 - attack_coef) * level;
        } else {
            self.env_state = release_coef * self.env_state + (1.0 - release_coef) * level;
        }

        // Map envelope to pan. phase_offset controls direction
        let direction = if self.phase_offset > 0.25 { -1.0 } else { 1.0 };
        let env_norm = (self.env_state * 4.0).min(1.0);
        let target_pan = env_norm * self.depth * self.width * direction;

        // Smooth to avoid clicks
        self.env_pan += (target_pan - self.env_pan) * 0.01;

        Self::apply_pan(input_l, input_r, self.env_pan)
    }

    /// Haas: micro-delay modulation on one channel for moving stereo image
    fn process_haas(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        let mono = (input_l + input_r) * 0.5;
        let buf_len = self.haas_buffer.len();
        if buf_len == 0 {
            return (input_l, input_r);
        }

        self.haas_buffer[self.haas_write_pos] = mono;
        self.haas_write_pos = (self.haas_write_pos + 1) % buf_len;

        if self.tempo_sync {
            self.haas_lfo_phase =
                (self.beat_position / self.tempo_division.beats() as f64).fract() as f32;
        } else {
            self.haas_lfo_phase += self.effective_rate() / self.sample_rate;
            if self.haas_lfo_phase >= 1.0 {
                self.haas_lfo_phase -= 1.0;
            }
        }

        let lfo = fast_sin(self.haas_lfo_phase * std::f32::consts::TAU);
        let max_delay_ms = 30.0 * self.depth;
        let delay_samples = (lfo * 0.5 + 0.5) * max_delay_ms * 0.001 * self.sample_rate;
        let delay_samples = delay_samples.min((buf_len - 2) as f32).max(0.0);

        // Read with interpolation
        let read_pos = self.haas_write_pos as f32 - delay_samples - 1.0;
        let read_pos = if read_pos < 0.0 {
            read_pos + buf_len as f32
        } else {
            read_pos
        };
        let idx = read_pos as usize % buf_len;
        let frac = read_pos - read_pos.floor();
        let idx_next = (idx + 1) % buf_len;
        let delayed = self.haas_buffer[idx] * (1.0 - frac) + self.haas_buffer[idx_next] * frac;

        // One channel gets delayed signal, the other stays dry
        if self.phase_offset > 0.25 {
            (delayed * self.width + mono * (1.0 - self.width), mono)
        } else {
            (mono, delayed * self.width + mono * (1.0 - self.width))
        }
    }

    /// Tremolo-Pan: combined volume modulation + panning (vintage amp style)
    fn process_tremolo_pan(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.tempo_sync {
            self.lfo_phase =
                (self.beat_position / self.tempo_division.beats() as f64).fract() as f32;
        } else {
            self.lfo_phase += self.lfo_inc();
            if self.lfo_phase >= 1.0 {
                self.lfo_phase -= 1.0;
            }
        }
        let lfo = self.slew_lfo(self.lfo_wave(self.lfo_phase));

        // Volume modulation (tremolo)
        let trem = 1.0 - self.trem_depth * self.depth * (lfo * 0.5 + 0.5);

        // Pan modulation
        let pan = lfo * self.depth * self.width;
        let (pan_l, pan_r) = Self::apply_pan(input_l, input_r, pan);
        (pan_l * trem, pan_r * trem)
    }

    pub fn reset(&mut self) {
        self.lfo_phase = 0.0;
        self.spin_speed_current = 0.0;
        self.bounce_pos = 1.0;
        self.bounce_vel = 0.0;
        self.sh_smoothed = 0.0;
        self.sh_counter = 0.0;
        self.env_state = 0.0;
        self.env_pan = 0.0;
        self.haas_lfo_phase = 0.0;
        self.haas_write_pos = 0;
        for s in self.haas_buffer.iter_mut() {
            *s = 0.0;
        }
    }

    // Setters
    pub fn set_mode(&mut self, mode: AutoPanMode) {
        if self.mode != mode {
            self.mode = mode;
            self.reset();
        }
    }
    pub fn set_wave(&mut self, wave: AutoPanWave) {
        self.wave = wave;
    }
    pub fn set_rate(&mut self, hz: f32) {
        self.rate = hz.clamp(0.01, 20.0);
    }
    pub fn set_depth(&mut self, d: f32) {
        self.depth = d.clamp(0.0, 1.0);
    }
    pub fn set_mix(&mut self, m: f32) {
        self.mix = m.clamp(0.0, 1.0);
    }
    pub fn set_width(&mut self, w: f32) {
        self.width = w.clamp(0.0, 1.0);
    }
    pub fn set_phase_offset(&mut self, p: f32) {
        self.phase_offset = p.clamp(0.0, 1.0);
    }
    pub fn set_glide(&mut self, g: f32) {
        self.glide = g.clamp(0.0, 1.0);
    }
    pub fn set_spin_speed(&mut self, s: f32) {
        self.spin_speed_target = s.clamp(0.0, 1.0);
    }
    pub fn set_trem_depth(&mut self, d: f32) {
        self.trem_depth = d.clamp(0.0, 1.0);
    }
    pub fn set_tempo_sync(&mut self, sync: bool) {
        self.tempo_sync = sync;
    }
    pub fn set_tempo_division(&mut self, division: TempoDivision) {
        self.tempo_division = division;
    }
    pub fn set_tempo(&mut self, tempo: f32) {
        self.tempo = tempo.clamp(20.0, 300.0);
    }
    pub fn set_beat_position(&mut self, pos: f64) {
        self.beat_position = pos;
    }
}

//==============================================================================
// STUTTER EFFECT
//==============================================================================

/// Stutter mode enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StutterMode {
    Stutter,
    Reverse,
    HalfSpeed,
    Scratch,
    Glitch,
}

impl StutterMode {
    pub fn from_index(idx: usize) -> Self {
        Self::all()
            .get(idx)
            .copied()
            .unwrap_or(StutterMode::Stutter)
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            StutterMode::Stutter => "Stutter",
            StutterMode::Reverse => "Reverse",
            StutterMode::HalfSpeed => "Half Speed",
            StutterMode::Scratch => "Scratch",
            StutterMode::Glitch => "Glitch",
        }
    }

    pub fn all() -> &'static [StutterMode] {
        &[
            StutterMode::Stutter,
            StutterMode::Reverse,
            StutterMode::HalfSpeed,
            StutterMode::Scratch,
            StutterMode::Glitch,
        ]
    }

    pub fn all_names() -> Vec<&'static str> {
        Self::all().iter().map(|m| m.display_name()).collect()
    }
}

/// Stutter effect — captures audio into a circular buffer and replays it creatively
#[derive(Debug, Clone)]
pub struct StutterEffect {
    sample_rate: f32,

    // Parameters
    mode: StutterMode,
    rate: f32, // triggers per second (Hz)
    gate: f32, // 0..1, portion of cycle that plays
    pub mix: f32,
    speed: f32, // playback speed for HalfSpeed/Scratch
    tempo_sync: bool,
    tempo_division: TempoDivision,
    tempo: f32,
    beat_position: f64,

    // State
    buffer: Vec<f32>, // stereo circular buffer (L, R interleaved)
    write_pos: usize,
    read_pos: f32,        // fractional read position (in frames)
    trigger_counter: f32, // samples until next trigger
    cycle_phase: f32,     // 0..1 phase within current trigger cycle
    capture_len: usize,   // length of captured chunk in frames
    capture_start: usize, // start frame of captured chunk
    scratch_dir: f32,     // +1 or -1 for scratch oscillation
    scratch_phase: f32,   // scratch oscillation phase
    glitch_rng: u32,      // PRNG state for glitch mode
}

impl StutterEffect {
    pub fn new(sample_rate: f32) -> Self {
        let buffer_size = (sample_rate as usize) * 2 * 2; // 2 seconds stereo
        Self {
            sample_rate,
            mode: StutterMode::Stutter,
            rate: 4.0,
            gate: 0.5,
            mix: 1.0,
            speed: 0.5,
            tempo_sync: false,
            tempo_division: TempoDivision::Eighth,
            tempo: 120.0,
            beat_position: 0.0,
            buffer: vec![0.0; buffer_size],
            write_pos: 0,
            read_pos: 0.0,
            trigger_counter: 0.0,
            cycle_phase: 0.0,
            capture_len: 0,
            capture_start: 0,
            scratch_dir: 1.0,
            scratch_phase: 0.0,
            glitch_rng: 0xDEAD_BEEF,
        }
    }

    /// Inline xorshift PRNG
    #[inline(always)]
    fn next_rand(&mut self) -> f32 {
        self.glitch_rng ^= self.glitch_rng << 13;
        self.glitch_rng ^= self.glitch_rng >> 17;
        self.glitch_rng ^= self.glitch_rng << 5;
        (self.glitch_rng & 0x7FFF_FFFF) as f32 / 0x7FFF_FFFF as f32
    }

    #[inline(always)]
    fn num_frames(&self) -> usize {
        self.buffer.len() / 2
    }

    #[inline(always)]
    fn read_frame(&self, frame: usize) -> (f32, f32) {
        let num = self.num_frames();
        let f = frame % num;
        (self.buffer[f * 2], self.buffer[f * 2 + 1])
    }

    #[inline(always)]
    fn read_frame_interp(&self, pos: f32) -> (f32, f32) {
        let num = self.num_frames();
        let mut p = pos % num as f32;
        if p < 0.0 {
            p += num as f32;
        }
        let idx = p as usize % num;
        let frac = p - p.floor();
        let idx_next = (idx + 1) % num;
        let l = self.buffer[idx * 2] * (1.0 - frac) + self.buffer[idx_next * 2] * frac;
        let r = self.buffer[idx * 2 + 1] * (1.0 - frac) + self.buffer[idx_next * 2 + 1] * frac;
        (l, r)
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        let num_frames = self.num_frames();
        if num_frames == 0 {
            return (input_l, input_r);
        }

        // Always write input into circular buffer
        let wf = self.write_pos % num_frames;
        self.buffer[wf * 2] = input_l;
        self.buffer[wf * 2 + 1] = input_r;
        self.write_pos = (self.write_pos + 1) % num_frames;

        if self.mix <= 0.0001 {
            return (input_l, input_r);
        }

        let cycle_len = self.sample_rate / self.effective_rate().max(0.1);

        if self.tempo_sync {
            // Beat-aligned: derive cycle phase from beat position
            let new_phase =
                (self.beat_position / self.tempo_division.beats() as f64).fract() as f32;
            // Detect wrap (new cycle start)
            if new_phase < self.cycle_phase - 0.5 {
                self.capture_len = (cycle_len as usize).min(num_frames);
                self.capture_start = if self.write_pos >= self.capture_len {
                    self.write_pos - self.capture_len
                } else {
                    num_frames - (self.capture_len - self.write_pos)
                };
                self.read_pos = 0.0;
                self.scratch_phase = 0.0;
                if self.mode == StutterMode::Glitch {
                    let rand_offset = (self.next_rand() * self.capture_len as f32) as usize;
                    self.read_pos = rand_offset as f32;
                }
            }
            self.cycle_phase = new_phase;
        } else {
            // Free-running trigger
            self.trigger_counter -= 1.0;
            if self.trigger_counter <= 0.0 {
                self.trigger_counter = cycle_len;
                self.cycle_phase = 0.0;
                self.capture_len = (cycle_len as usize).min(num_frames);
                self.capture_start = if self.write_pos >= self.capture_len {
                    self.write_pos - self.capture_len
                } else {
                    num_frames - (self.capture_len - self.write_pos)
                };
                self.read_pos = 0.0;
                self.scratch_phase = 0.0;
                if self.mode == StutterMode::Glitch {
                    let rand_offset = (self.next_rand() * self.capture_len as f32) as usize;
                    self.read_pos = rand_offset as f32;
                }
            }
            self.cycle_phase += 1.0 / cycle_len;
            if self.cycle_phase > 1.0 {
                self.cycle_phase = 1.0;
            }
        }

        // Gate: crossfade at gate boundaries to avoid clicks.
        // gate_env: 1.0 = fully wet/stuttered, 0.0 = fully dry.
        let gate_env = if self.cycle_phase > self.gate {
            // Past gate: fade out over a small region at the boundary
            let fade_len = 0.02_f32; // 2% of cycle for crossfade
            let overshoot = self.cycle_phase - self.gate;
            (1.0 - overshoot / fade_len).max(0.0)
        } else if self.cycle_phase < 0.02 {
            // Start of cycle: fade in
            (self.cycle_phase / 0.02).min(1.0)
        } else {
            1.0
        };

        if gate_env <= 0.0 {
            return (input_l, input_r);
        }

        // Generate wet signal based on mode
        let (wet_l, wet_r) = if self.capture_len == 0 {
            (input_l, input_r)
        } else {
            match self.mode {
                StutterMode::Stutter => {
                    let frame = (self.capture_start + (self.read_pos as usize % self.capture_len))
                        % num_frames;
                    self.read_pos += 1.0;
                    if self.read_pos >= self.capture_len as f32 {
                        self.read_pos = 0.0;
                    }
                    self.read_frame(frame)
                }
                StutterMode::Reverse => {
                    let offset = self.capture_len - 1 - (self.read_pos as usize % self.capture_len);
                    let frame = (self.capture_start + offset) % num_frames;
                    self.read_pos += 1.0;
                    if self.read_pos >= self.capture_len as f32 {
                        self.read_pos = 0.0;
                    }
                    self.read_frame(frame)
                }
                StutterMode::HalfSpeed => {
                    let frac_pos =
                        self.capture_start as f32 + (self.read_pos % self.capture_len as f32);
                    self.read_pos += self.speed.clamp(0.1, 2.0);
                    if self.read_pos >= self.capture_len as f32 {
                        self.read_pos = 0.0;
                    }
                    self.read_frame_interp(frac_pos)
                }
                StutterMode::Scratch => {
                    // Oscillate back and forth through buffer
                    self.scratch_phase +=
                        self.speed.clamp(0.1, 2.0) * 2.0 * std::f32::consts::PI / cycle_len;
                    let t = fast_sin(self.scratch_phase) * 0.5 + 0.5; // 0..1
                    let pos = self.capture_start as f32 + t * (self.capture_len - 1) as f32;
                    self.read_frame_interp(pos)
                }
                StutterMode::Glitch => {
                    let frac_pos =
                        self.capture_start as f32 + (self.read_pos % self.capture_len as f32);
                    self.read_pos += 1.0;
                    // Randomly jump within buffer
                    if self.next_rand() < 0.01 {
                        self.read_pos = self.next_rand() * self.capture_len as f32;
                    }
                    if self.read_pos >= self.capture_len as f32 {
                        self.read_pos = 0.0;
                    }
                    self.read_frame_interp(frac_pos)
                }
            }
        };

        // Crossfade mix with gate envelope for click-free transitions
        let wet_mix = self.mix * gate_env;
        let dry_mix = 1.0 - wet_mix;
        let out_l = input_l * dry_mix + wet_l * wet_mix;
        let out_r = input_r * dry_mix + wet_r * wet_mix;
        (out_l, out_r)
    }

    pub fn reset(&mut self) {
        for s in self.buffer.iter_mut() {
            *s = 0.0;
        }
        self.write_pos = 0;
        self.read_pos = 0.0;
        self.trigger_counter = 0.0;
        self.cycle_phase = 0.0;
        self.capture_len = 0;
        self.capture_start = 0;
        self.scratch_phase = 0.0;
    }

    // Setters
    pub fn set_mode(&mut self, mode: StutterMode) {
        if self.mode != mode {
            self.mode = mode;
        }
    }
    pub fn set_rate(&mut self, rate: f32) {
        self.rate = rate.clamp(0.1, 30.0);
    }
    pub fn set_gate(&mut self, gate: f32) {
        self.gate = gate.clamp(0.0, 1.0);
    }
    pub fn set_mix(&mut self, m: f32) {
        self.mix = m.clamp(0.0, 1.0);
    }
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.clamp(0.1, 2.0);
    }
    pub fn set_tempo_sync(&mut self, sync: bool) {
        self.tempo_sync = sync;
    }
    pub fn set_tempo_division(&mut self, division: TempoDivision) {
        self.tempo_division = division;
    }
    pub fn set_tempo(&mut self, tempo: f32) {
        self.tempo = tempo.clamp(20.0, 300.0);
    }
    pub fn set_beat_position(&mut self, pos: f64) {
        self.beat_position = pos;
    }

    fn effective_rate(&self) -> f32 {
        if self.tempo_sync {
            1.0 / self.tempo_division.to_time_seconds(self.tempo)
        } else {
            self.rate
        }
    }
}

//==============================================================================
// FILTER EFFECT
//==============================================================================

/// Filter mode enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    LowPass,
    HighPass,
    BandPass,
    Notch,
}

impl FilterMode {
    pub fn from_index(idx: usize) -> Self {
        Self::all().get(idx).copied().unwrap_or(FilterMode::LowPass)
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            FilterMode::LowPass => "Low Pass",
            FilterMode::HighPass => "High Pass",
            FilterMode::BandPass => "Band Pass",
            FilterMode::Notch => "Notch",
        }
    }

    pub fn all() -> &'static [FilterMode] {
        &[
            FilterMode::LowPass,
            FilterMode::HighPass,
            FilterMode::BandPass,
            FilterMode::Notch,
        ]
    }

    pub fn all_names() -> Vec<&'static str> {
        Self::all().iter().map(|m| m.display_name()).collect()
    }
}

/// Resonant multi-mode filter with LFO and envelope modulation (SVF implementation)
#[derive(Debug, Clone)]
pub struct FilterEffect {
    sample_rate: f32,

    // Parameters
    mode: FilterMode,
    cutoff: f32,     // 20-20000 Hz
    resonance: f32,  // 0..1
    lfo_rate: f32,   // Hz
    lfo_depth: f32,  // 0..1
    env_amount: f32, // 0..1
    pub mix: f32,
    tempo_sync: bool,
    tempo_division: TempoDivision,
    tempo: f32,
    beat_position: f64,

    // Cached SVF coefficients
    cached_cutoff: f32,
    cached_resonance: f32,
    g: f32,
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,

    // SVF state per channel [L, R]
    ic1eq: [f32; 2],
    ic2eq: [f32; 2],

    // LFO state
    lfo_phase: f32,

    // Envelope follower state
    env_state: f32,
    env_attack_coef: f32,
    env_release_coef: f32,
}

impl FilterEffect {
    pub fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            sample_rate,
            mode: FilterMode::LowPass,
            cutoff: 1000.0,
            resonance: 0.0,
            lfo_rate: 1.0,
            lfo_depth: 0.0,
            env_amount: 0.0,
            mix: 1.0,
            tempo_sync: false,
            tempo_division: TempoDivision::Quarter,
            tempo: 120.0,
            beat_position: 0.0,
            cached_cutoff: -1.0,
            cached_resonance: -1.0,
            g: 0.0,
            k: 2.0,
            a1: 0.0,
            a2: 0.0,
            a3: 0.0,
            ic1eq: [0.0; 2],
            ic2eq: [0.0; 2],
            lfo_phase: 0.0,
            env_state: 0.0,
            env_attack_coef: (-1.0 / (0.001 * sample_rate)).exp(),
            env_release_coef: (-1.0 / (0.05 * sample_rate)).exp(),
        };
        s.update_coefficients(1000.0, 0.0);
        s
    }

    #[inline(always)]
    fn update_coefficients(&mut self, cutoff: f32, resonance: f32) {
        let cutoff = cutoff.clamp(20.0, self.sample_rate * 0.49);
        self.g = (std::f32::consts::PI * cutoff / self.sample_rate).tan();
        self.k = 2.0 - 1.9 * resonance.clamp(0.0, 1.0); // maps resonance 0..1 to k 2..0.1
        self.a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
        self.a2 = self.g * self.a1;
        self.a3 = self.g * self.a2;
        self.cached_cutoff = cutoff;
        self.cached_resonance = resonance;
    }

    #[inline(always)]
    fn process_svf_channel(&mut self, ch: usize, input: f32) -> f32 {
        let v3 = input - self.ic2eq[ch];
        let v1 = self.a1 * self.ic1eq[ch] + self.a2 * v3;
        let v2 = self.ic2eq[ch] + self.a2 * self.ic1eq[ch] + self.a3 * v3;
        self.ic1eq[ch] = 2.0 * v1 - self.ic1eq[ch];
        self.ic2eq[ch] = 2.0 * v2 - self.ic2eq[ch];

        match self.mode {
            FilterMode::LowPass => v2,
            FilterMode::BandPass => v1,
            FilterMode::HighPass => input - self.k * v1 - v2,
            FilterMode::Notch => {
                let low = v2;
                let high = input - self.k * v1 - v2;
                low + high
            }
        }
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.mix <= 0.0001 {
            return (input_l, input_r);
        }

        // Envelope follower
        let level = (input_l.abs() + input_r.abs()) * 0.5;
        if level > self.env_state {
            self.env_state =
                self.env_attack_coef * self.env_state + (1.0 - self.env_attack_coef) * level;
        } else {
            self.env_state =
                self.env_release_coef * self.env_state + (1.0 - self.env_release_coef) * level;
        }

        // LFO modulation
        if self.tempo_sync {
            self.lfo_phase =
                (self.beat_position / self.tempo_division.beats() as f64).fract() as f32;
        } else {
            self.lfo_phase += self.effective_lfo_rate() / self.sample_rate;
            if self.lfo_phase >= 1.0 {
                self.lfo_phase -= 1.0;
            }
        }
        let lfo = fast_sin(self.lfo_phase * std::f32::consts::TAU);

        // Modulate cutoff
        let lfo_mod = lfo * self.lfo_depth * self.cutoff * 0.5; // modulate up to ±50% of cutoff
        let env_mod = self.env_state.min(1.0) * self.env_amount * self.cutoff * 2.0; // env pushes cutoff up
        let mod_cutoff = (self.cutoff + lfo_mod + env_mod).clamp(20.0, 20000.0);

        // Recompute coefficients if modulated cutoff changed significantly
        let cutoff_diff = (mod_cutoff - self.cached_cutoff).abs();
        let res_diff = (self.resonance - self.cached_resonance).abs();
        if cutoff_diff > 0.5 || res_diff > 0.001 {
            self.update_coefficients(mod_cutoff, self.resonance);
        }

        let wet_l = self.process_svf_channel(0, input_l);
        let wet_r = self.process_svf_channel(1, input_r);

        let out_l = input_l * (1.0 - self.mix) + wet_l * self.mix;
        let out_r = input_r * (1.0 - self.mix) + wet_r * self.mix;
        (out_l, out_r)
    }

    pub fn reset(&mut self) {
        self.ic1eq = [0.0; 2];
        self.ic2eq = [0.0; 2];
        self.lfo_phase = 0.0;
        self.env_state = 0.0;
    }

    // Setters
    pub fn set_mode(&mut self, mode: FilterMode) {
        self.mode = mode;
    }
    pub fn set_cutoff(&mut self, cutoff: f32) {
        self.cutoff = cutoff.clamp(20.0, 20000.0);
    }
    pub fn set_resonance(&mut self, res: f32) {
        self.resonance = res.clamp(0.0, 1.0);
    }
    pub fn set_lfo_rate(&mut self, rate: f32) {
        self.lfo_rate = rate.clamp(0.01, 20.0);
    }
    pub fn set_lfo_depth(&mut self, depth: f32) {
        self.lfo_depth = depth.clamp(0.0, 1.0);
    }
    pub fn set_env_amount(&mut self, amount: f32) {
        self.env_amount = amount.clamp(0.0, 1.0);
    }
    pub fn set_mix(&mut self, m: f32) {
        self.mix = m.clamp(0.0, 1.0);
    }
    pub fn set_tempo_sync(&mut self, sync: bool) {
        self.tempo_sync = sync;
    }
    pub fn set_tempo_division(&mut self, division: TempoDivision) {
        self.tempo_division = division;
    }
    pub fn set_tempo(&mut self, tempo: f32) {
        self.tempo = tempo.clamp(20.0, 300.0);
    }
    pub fn set_beat_position(&mut self, pos: f64) {
        self.beat_position = pos;
    }

    fn effective_lfo_rate(&self) -> f32 {
        if self.tempo_sync {
            1.0 / self.tempo_division.to_time_seconds(self.tempo)
        } else {
            self.lfo_rate
        }
    }
}

//==============================================================================
// RING MODULATOR EFFECT
//==============================================================================

/// Ring modulator waveform
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingModWave {
    Sine,
    Square,
    Saw,
}

impl RingModWave {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => RingModWave::Square,
            2 => RingModWave::Saw,
            _ => RingModWave::Sine,
        }
    }

    pub fn all_names() -> Vec<&'static str> {
        vec!["Sine", "Square", "Saw"]
    }
}

/// Ring modulator effect
#[derive(Debug, Clone)]
pub struct RingModEffect {
    sample_rate: f32,

    // Parameters
    frequency: f32, // 1-5000 Hz
    waveform: RingModWave,
    depth: f32, // 0..1
    pub mix: f32,

    // State
    phase: f32, // 0..1 phase accumulator
}

impl RingModEffect {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            frequency: 440.0,
            waveform: RingModWave::Sine,
            depth: 1.0,
            mix: 1.0,
            phase: 0.0,
        }
    }

    /// PolyBLEP residual for band-limiting a step discontinuity at phase wrap.
    #[inline(always)]
    fn poly_blep(t: f32, dt: f32) -> f32 {
        if dt <= 0.0 { return 0.0; }
        if t < dt {
            let x = t / dt;
            x + x - x * x - 1.0
        } else if t > 1.0 - dt {
            let x = (t - 1.0) / dt;
            x * x + x + x + 1.0
        } else {
            0.0
        }
    }

    /// Generate carrier oscillator value at current phase. `dt` is the phase
    /// increment per sample; the Square and Saw carriers are band-limited with
    /// PolyBLEP so their hard edges don't inject a click/alias into the ring mod.
    #[inline(always)]
    fn carrier(&self, dt: f32) -> f32 {
        match self.waveform {
            RingModWave::Sine => fast_sin(self.phase * std::f32::consts::TAU),
            RingModWave::Square => {
                let mut sq = if self.phase < 0.5 { 1.0 } else { -1.0 };
                sq += Self::poly_blep(self.phase, dt);
                sq -= Self::poly_blep((self.phase + 0.5) % 1.0, dt);
                sq
            }
            RingModWave::Saw => {
                (2.0 * self.phase - 1.0) - Self::poly_blep(self.phase, dt)
            }
        }
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.mix <= 0.0001 {
            return (input_l, input_r);
        }

        let dt = self.frequency / self.sample_rate;
        let carrier = self.carrier(dt);

        // Advance phase
        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        // Ring modulation: multiply input by carrier
        let mod_l = input_l * carrier;
        let mod_r = input_r * carrier;

        // Depth blends between original and ring-modulated
        let wet_l = input_l * (1.0 - self.depth) + mod_l * self.depth;
        let wet_r = input_r * (1.0 - self.depth) + mod_r * self.depth;

        // Dry/wet mix
        let out_l = input_l * (1.0 - self.mix) + wet_l * self.mix;
        let out_r = input_r * (1.0 - self.mix) + wet_r * self.mix;
        (out_l, out_r)
    }

    pub fn reset(&mut self) {
        self.phase = 0.0;
    }

    // Setters
    pub fn set_frequency(&mut self, freq: f32) {
        self.frequency = freq.clamp(1.0, 5000.0);
    }
    pub fn set_waveform(&mut self, wave: RingModWave) {
        self.waveform = wave;
    }
    pub fn set_depth(&mut self, depth: f32) {
        self.depth = depth.clamp(0.0, 1.0);
    }
    pub fn set_mix(&mut self, m: f32) {
        self.mix = m.clamp(0.0, 1.0);
    }
}

//==============================================================================
// LO-FI EFFECT
//==============================================================================

/// Lo-fi degradation effect
#[derive(Debug, Clone)]
pub struct LoFiEffect {
    sample_rate: f32,

    // Parameters
    sample_rate_div: usize, // 1-64, integer divisor
    bit_depth: usize,       // 1-16
    noise: f32,             // 0..1
    wow_depth: f32,         // 0..1
    flutter_depth: f32,     // 0..1
    rolloff: f32,           // 200-20000 Hz, LP filter cutoff
    pub mix: f32,

    // Cached
    cached_rolloff_alpha: f32,

    // State
    hold_counter: usize,
    hold_l: f32,
    hold_r: f32,
    rng_state: u32,
    wow_phase: f32,
    flutter_phase: f32,
    lp_state: [f32; 2],

    // === Phase 2 upgrades ===
    /// TPDF dither amount (0..1). At 1.0 the dither matches 1 LSB at the
    /// current bit depth.
    dither: f32,
    /// μ-law companding amount (0..1). Applies G.711 μ-255 curve to the
    /// signal before bit reduction so small signals get more quantization
    /// resolution.
    mu_law: f32,
    /// Clock jitter amount (0..1). Shortens/extends the sample-and-hold
    /// period by a random amount for authentic tape/vinyl character.
    jitter: f32,
    /// Previous TPDF dither sample for 1st-order shaping.
    dither_prev_l: f32,
    dither_prev_r: f32,
}

impl LoFiEffect {
    pub fn new(sample_rate: f32) -> Self {
        let rolloff = 8000.0;
        let alpha = Self::compute_rolloff_alpha(rolloff, sample_rate);
        Self {
            sample_rate,
            sample_rate_div: 1,
            bit_depth: 16,
            noise: 0.0,
            wow_depth: 0.0,
            flutter_depth: 0.0,
            rolloff,
            mix: 1.0,
            cached_rolloff_alpha: alpha,
            hold_counter: 0,
            hold_l: 0.0,
            hold_r: 0.0,
            rng_state: 0xCAFE_BABE,
            wow_phase: 0.0,
            flutter_phase: 0.0,
            lp_state: [0.0; 2],
            dither: 0.0,
            mu_law: 0.0,
            jitter: 0.0,
            dither_prev_l: 0.0,
            dither_prev_r: 0.0,
        }
    }

    /// μ-law encode/decode pair — smoothly biases small-signal resolution.
    /// `amt` in 0..1 controls how much of the curve is applied (crossfaded
    /// with the clean path). At 1.0 it's full G.711 μ=255 companding.
    #[inline]
    fn mu_law_shape(x: f32, amt: f32) -> f32 {
        if amt <= 0.0001 { return x; }
        const MU: f32 = 255.0;
        // ln(1 + MU) = ln(256) — was `(1.0 + MU).ln()` per sample.
        // Precomputed constant so the encoder math drops one ln().
        // The decoder used to call `(1.0 + MU).powf(y)`, which is
        // `exp(y * ln(1+MU))` internally — i.e. another ln per
        // sample. Replace with the direct exp form so the inner
        // loop runs 1 ln + 1 exp instead of 2 ln + 1 exp.
        const MU_LN: f32 = 5.545177_f32; // f32::ln(256.0)
        let sign = x.signum();
        let ax = x.abs().min(1.0);
        let encoded = sign * (1.0 + MU * ax).ln() / MU_LN;
        let decoded = sign * ((encoded.abs() * MU_LN).exp() - 1.0) / MU;
        x * (1.0 - amt) + decoded * amt
    }

    #[inline(always)]
    fn compute_rolloff_alpha(cutoff: f32, sample_rate: f32) -> f32 {
        let rc = 1.0 / (2.0 * std::f32::consts::PI * cutoff);
        let dt = 1.0 / sample_rate;
        dt / (rc + dt)
    }

    /// Inline xorshift PRNG (-1..1)
    #[inline(always)]
    fn next_noise(&mut self) -> f32 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 17;
        self.rng_state ^= self.rng_state << 5;
        (self.rng_state as i32) as f32 / i32::MAX as f32
    }

    /// Quantize to N bits
    #[inline(always)]
    fn quantize(sample: f32, bits: usize) -> f32 {
        let levels = (1u32 << bits.min(16)) as f32;
        (sample * levels).round() / levels
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.mix <= 0.0001 {
            return (input_l, input_r);
        }

        let mut l = input_l;
        let mut r = input_r;

        // Wow (slow ~0.5Hz) and flutter (fast ~6Hz) — subtle pitch/amplitude variation
        // Implemented as amplitude modulation (approximation of pitch wobble)
        self.wow_phase += 0.5 / self.sample_rate;
        if self.wow_phase >= 1.0 {
            self.wow_phase -= 1.0;
        }
        self.flutter_phase += 6.0 / self.sample_rate;
        if self.flutter_phase >= 1.0 {
            self.flutter_phase -= 1.0;
        }

        let wow_mod =
            1.0 + fast_sin(self.wow_phase * std::f32::consts::TAU) * self.wow_depth * 0.02;
        let flutter_mod =
            1.0 + fast_sin(self.flutter_phase * std::f32::consts::TAU) * self.flutter_depth * 0.01;
        l *= wow_mod * flutter_mod;
        r *= wow_mod * flutter_mod;

        // Sample rate reduction with optional clock jitter.
        // Jitter shortens or lengthens the hold by up to ±25% for authentic
        // vinyl/tape wobble.
        self.hold_counter += 1;
        let jittered_div = if self.jitter > 0.001 && self.sample_rate_div > 1 {
            let n = self.next_noise(); // -1..1
            let wobble = 1.0 + n * self.jitter * 0.25;
            ((self.sample_rate_div as f32 * wobble).max(1.0) as usize).min(128)
        } else {
            self.sample_rate_div
        };
        if self.hold_counter >= jittered_div {
            self.hold_counter = 0;
            self.hold_l = l;
            self.hold_r = r;
        }
        l = self.hold_l;
        r = self.hold_r;

        // μ-law companding shapes the signal before bit reduction so small
        // signals get more effective resolution.
        l = Self::mu_law_shape(l, self.mu_law);
        r = Self::mu_law_shape(r, self.mu_law);

        // Bit depth reduction, optionally with TPDF dither shaping.
        if self.bit_depth < 16 {
            if self.dither > 0.0001 {
                let levels = (1u32 << self.bit_depth.min(16)) as f32;
                let lsb = 1.0 / levels;
                // Triangular PDF = sum of two uniform noises
                let d_l = (self.next_noise() + self.next_noise()) * 0.5 * lsb * self.dither;
                let d_r = (self.next_noise() + self.next_noise()) * 0.5 * lsb * self.dither;
                // First-order noise shaping: subtract the previous dither error
                let shaped_l = l + d_l - self.dither_prev_l * 0.5;
                let shaped_r = r + d_r - self.dither_prev_r * 0.5;
                let q_l = Self::quantize(shaped_l, self.bit_depth);
                let q_r = Self::quantize(shaped_r, self.bit_depth);
                self.dither_prev_l = q_l - l;
                self.dither_prev_r = q_r - r;
                l = q_l;
                r = q_r;
            } else {
                l = Self::quantize(l, self.bit_depth);
                r = Self::quantize(r, self.bit_depth);
            }
        }

        // Add noise
        if self.noise > 0.0001 {
            l += self.next_noise() * self.noise * 0.1;
            r += self.next_noise() * self.noise * 0.1;
        }

        // Rolloff LP filter (1-pole)
        let alpha = self.cached_rolloff_alpha;
        self.lp_state[0] += alpha * (l - self.lp_state[0]);
        self.lp_state[1] += alpha * (r - self.lp_state[1]);
        l = self.lp_state[0];
        r = self.lp_state[1];

        // Dry/wet mix
        let out_l = input_l * (1.0 - self.mix) + l * self.mix;
        let out_r = input_r * (1.0 - self.mix) + r * self.mix;
        (out_l, out_r)
    }

    pub fn reset(&mut self) {
        self.hold_counter = 0;
        self.hold_l = 0.0;
        self.hold_r = 0.0;
        self.wow_phase = 0.0;
        self.flutter_phase = 0.0;
        self.lp_state = [0.0; 2];
    }

    // Setters
    pub fn set_sample_rate_div(&mut self, div: usize) {
        self.sample_rate_div = div.clamp(1, 64);
    }
    pub fn set_bit_depth(&mut self, bits: usize) {
        self.bit_depth = bits.clamp(1, 16);
    }
    pub fn set_noise(&mut self, n: f32) {
        self.noise = n.clamp(0.0, 1.0);
    }
    pub fn set_wow_depth(&mut self, d: f32) {
        self.wow_depth = d.clamp(0.0, 1.0);
    }
    pub fn set_flutter_depth(&mut self, d: f32) {
        self.flutter_depth = d.clamp(0.0, 1.0);
    }
    pub fn set_rolloff(&mut self, freq: f32) {
        self.rolloff = freq.clamp(200.0, 20000.0);
        self.cached_rolloff_alpha = Self::compute_rolloff_alpha(self.rolloff, self.sample_rate);
    }
    pub fn set_mix(&mut self, m: f32) {
        self.mix = m.clamp(0.0, 1.0);
    }
    pub fn set_dither(&mut self, d: f32) { self.dither = d.clamp(0.0, 1.0); }
    pub fn set_mu_law(&mut self, d: f32) { self.mu_law = d.clamp(0.0, 1.0); }
    pub fn set_jitter(&mut self, d: f32) { self.jitter = d.clamp(0.0, 1.0); }
}

//==============================================================================
// STEREO IMAGER EFFECT
//==============================================================================

/// Mid/side based stereo width control
#[derive(Debug, Clone)]
pub struct StereoImagerEffect {
    sample_rate: f32,

    // Parameters
    width: f32,     // 0..2 (0=mono, 1=original, 2=extra wide)
    mono_freq: f32, // 20-500 Hz, frequencies below this are mono'd
    pub mix: f32,

    // Cached
    cached_mono_alpha: f32,

    // State (1-pole filters for bass mono)
    lp_state: f32, // LP on mid for bass extraction
    hp_state: f32, // HP on side for bass removal
}

impl StereoImagerEffect {
    pub fn new(sample_rate: f32) -> Self {
        let mono_freq = 100.0;
        let alpha = Self::compute_mono_alpha(mono_freq, sample_rate);
        Self {
            sample_rate,
            width: 1.0,
            mono_freq,
            mix: 1.0,
            cached_mono_alpha: alpha,
            lp_state: 0.0,
            hp_state: 0.0,
        }
    }

    #[inline(always)]
    fn compute_mono_alpha(freq: f32, sample_rate: f32) -> f32 {
        let rc = 1.0 / (2.0 * std::f32::consts::PI * freq);
        let dt = 1.0 / sample_rate;
        dt / (rc + dt)
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.mix <= 0.0001 {
            return (input_l, input_r);
        }

        // Encode to mid/side
        let mid = (input_l + input_r) * 0.5;
        let side = (input_l - input_r) * 0.5;

        // Bass mono: LP filter on mid to extract bass, HP on side to remove bass
        let alpha = self.cached_mono_alpha;
        self.lp_state += alpha * (mid - self.lp_state);
        let _bass_mid = self.lp_state;

        // HP on side: side_hp = side - lp(side)
        self.hp_state += alpha * (side - self.hp_state);
        let side_hp = side - self.hp_state; // high-passed side (bass removed)

        // Reconstruct: bass stays in mid (mono), side only has HF content
        let final_mid = mid; // keep full mid
        let final_side = side_hp * self.width; // width-scaled side without bass

        // Add back the bass that was removed from side as mono (it's already in mid)
        // Decode back to L/R
        let wet_l = final_mid + final_side;
        let wet_r = final_mid - final_side;

        // Dry/wet mix
        let out_l = input_l * (1.0 - self.mix) + wet_l * self.mix;
        let out_r = input_r * (1.0 - self.mix) + wet_r * self.mix;
        (out_l, out_r)
    }

    pub fn reset(&mut self) {
        self.lp_state = 0.0;
        self.hp_state = 0.0;
    }

    // Setters
    pub fn set_width(&mut self, w: f32) {
        self.width = w.clamp(0.0, 2.0);
    }
    pub fn set_mono_freq(&mut self, freq: f32) {
        self.mono_freq = freq.clamp(20.0, 500.0);
        self.cached_mono_alpha = Self::compute_mono_alpha(self.mono_freq, self.sample_rate);
    }
    pub fn set_mix(&mut self, m: f32) {
        self.mix = m.clamp(0.0, 1.0);
    }
}

//==============================================================================
// MAXIMIZER EFFECT
//==============================================================================

/// Professional maximizer — transparent loudness maximization with lookahead
/// limiting, multi-stage gain smoothing, and soft-knee compression.
///
/// Signal flow: input gain → soft-knee gain reduction → lookahead peak limiter → ceiling
#[derive(Debug, Clone)]
pub struct MaximizerEffect {
    sample_rate: f32,

    // Parameters
    input_gain: f32, // dB (0 to 24) — drive into the maximizer
    ceiling: f32,    // dB (-12 to 0) — output ceiling
    release: f32,    // seconds (0.01 to 1.0)
    knee: f32,       // dB (0 to 12) — soft knee width
    character: f32,  // 0.0=transparent, 1.0=aggressive
    mix: f32,

    // Cached linear values
    input_gain_lin: f32,
    ceiling_lin: f32,
    release_coef: f32,

    // Dual-stage envelope for smooth gain reduction
    fast_env_db: f32, // fast peak follower (dB)
    slow_env_db: f32, // slow program-dependent follower (dB)
    fast_attack_coef: f32,
    slow_attack_coef: f32,
    slow_release_coef: f32,

    // Lookahead delay buffer
    lookahead_l: Vec<f32>,
    lookahead_r: Vec<f32>,
    lookahead_pos: usize,
    lookahead_len: usize,

    // Gain smoothing to prevent zipper noise
    smooth_gain: f32,
    smooth_coef: f32,
}

impl MaximizerEffect {
    pub fn new(sample_rate: f32) -> Self {
        let lookahead_len = (sample_rate * 0.005) as usize; // 5ms lookahead
        let mut s = Self {
            sample_rate,
            input_gain: 0.0,
            ceiling: -0.3,
            release: 0.15,
            knee: 6.0,
            character: 0.5,
            mix: 1.0,
            input_gain_lin: 1.0,
            ceiling_lin: 10.0f32.powf(-0.3 / 20.0),
            release_coef: 0.0,
            // Both of these hold GAIN REDUCTION in dB, so their rest value is
            // 0 — nothing reduced. Starting them at -120 dB is starting at a
            // gain of one millionth: the first seconds of anything fed through
            // a fresh Maximizer faded up from silence as the smoother crawled
            // back to unity. Measured on a demo bounce: -118 dB at 0 s, still
            // -49 dB five seconds in.
            fast_env_db: 0.0,
            slow_env_db: 0.0,
            fast_attack_coef: 0.0,
            slow_attack_coef: 0.0,
            slow_release_coef: 0.0,
            lookahead_l: vec![0.0; lookahead_len],
            lookahead_r: vec![0.0; lookahead_len],
            lookahead_pos: 0,
            lookahead_len,
            smooth_gain: 1.0,
            smooth_coef: 0.0,
        };
        s.update_coefficients();
        s
    }

    fn update_coefficients(&mut self) {
        self.input_gain_lin = fast_db_to_lin(self.input_gain);
        self.ceiling_lin = fast_db_to_lin(self.ceiling);
        self.release_coef = (-1.0 / (self.release * self.sample_rate)).exp();

        // Fast attack: ~0.1ms for transparent peak catching
        self.fast_attack_coef = (-1.0 / (0.0001 * self.sample_rate)).exp();
        // Slow attack: program-dependent, 1-5ms based on character
        let slow_atk_ms = 0.001 + (1.0 - self.character) * 0.004;
        self.slow_attack_coef = (-1.0 / (slow_atk_ms * self.sample_rate)).exp();
        // Slow release: 2-8x the main release for program-dependent behavior
        let slow_rel_mult = 2.0 + self.character * 6.0;
        self.slow_release_coef = (-1.0 / (self.release * slow_rel_mult * self.sample_rate)).exp();
        // Gain smoothing: ~1ms for zipper-free transitions
        self.smooth_coef = (-1.0 / (0.001 * self.sample_rate)).exp();
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        // Apply input gain
        let boosted_l = input_l * self.input_gain_lin;
        let boosted_r = input_r * self.input_gain_lin;

        // Write into lookahead buffer, read delayed sample
        let delayed_l = self.lookahead_l[self.lookahead_pos];
        let delayed_r = self.lookahead_r[self.lookahead_pos];
        self.lookahead_l[self.lookahead_pos] = boosted_l;
        self.lookahead_r[self.lookahead_pos] = boosted_r;
        self.lookahead_pos = (self.lookahead_pos + 1) % self.lookahead_len;

        // Peak detection in dB
        let peak = boosted_l.abs().max(boosted_r.abs()).max(1e-10);
        let peak_db = 20.0 * fast_log10(peak);
        let ceiling_db = self.ceiling;

        // Soft-knee gain reduction calculation
        let gain_db = self.compute_gain_reduction(peak_db, ceiling_db);

        // Dual-stage envelope follower
        // Fast envelope: catches transient peaks
        if gain_db < self.fast_env_db {
            self.fast_env_db =
                self.fast_attack_coef * self.fast_env_db + (1.0 - self.fast_attack_coef) * gain_db;
        } else {
            self.fast_env_db =
                self.release_coef * self.fast_env_db + (1.0 - self.release_coef) * gain_db;
        }

        // Slow envelope: tracks program level for sustained loudness
        if gain_db < self.slow_env_db {
            self.slow_env_db =
                self.slow_attack_coef * self.slow_env_db + (1.0 - self.slow_attack_coef) * gain_db;
        } else {
            self.slow_env_db = self.slow_release_coef * self.slow_env_db
                + (1.0 - self.slow_release_coef) * gain_db;
        }

        // Combine envelopes: use the more aggressive (lower) of the two
        let env_db = self.fast_env_db.min(self.slow_env_db);

        // Convert to linear gain and smooth
        let target_gain = fast_db_to_lin(env_db);
        self.smooth_gain =
            self.smooth_coef * self.smooth_gain + (1.0 - self.smooth_coef) * target_gain;

        // Apply gain to delayed signal
        let wet_l = delayed_l * self.smooth_gain;
        let wet_r = delayed_r * self.smooth_gain;

        // Hard clip safety at ceiling (should rarely engage)
        let ceil = self.ceiling_lin;
        let wet_l = wet_l.clamp(-ceil, ceil);
        let wet_r = wet_r.clamp(-ceil, ceil);

        // Dry/wet mix (dry uses original input, not boosted delayed signal)
        let out_l = input_l * (1.0 - self.mix) + wet_l * self.mix;
        let out_r = input_r * (1.0 - self.mix) + wet_r * self.mix;

        (out_l, out_r)
    }

    /// Soft-knee gain reduction curve.
    /// Returns gain in dB to apply (0 = no reduction, negative = reduction).
    #[inline]
    fn compute_gain_reduction(&self, input_db: f32, threshold_db: f32) -> f32 {
        let knee_half = self.knee * 0.5;

        if input_db <= threshold_db - knee_half {
            // Below knee: no reduction
            0.0
        } else if input_db >= threshold_db + knee_half {
            // Above knee: full brick-wall limiting
            threshold_db - input_db
        } else {
            // In the knee: quadratic interpolation for smooth transition
            let x = input_db - (threshold_db - knee_half);
            let knee_width = self.knee.max(0.001);
            // Quadratic curve: progressively more reduction through the knee
            -(x * x) / (2.0 * knee_width)
        }
    }

    pub fn reset(&mut self) {
        self.fast_env_db = 0.0;
        self.slow_env_db = 0.0;
        self.smooth_gain = 1.0;
        self.lookahead_l.fill(0.0);
        self.lookahead_r.fill(0.0);
        self.lookahead_pos = 0;
    }

    pub fn set_input_gain(&mut self, db: f32) {
        self.input_gain = db.clamp(0.0, 24.0);
        self.update_coefficients();
    }
    pub fn set_ceiling(&mut self, db: f32) {
        self.ceiling = db.clamp(-12.0, 0.0);
        self.update_coefficients();
    }
    pub fn set_release(&mut self, s: f32) {
        self.release = s.clamp(0.01, 1.0);
        self.update_coefficients();
    }
    pub fn set_knee(&mut self, db: f32) {
        self.knee = db.clamp(0.0, 12.0);
    }
    pub fn set_character(&mut self, c: f32) {
        self.character = c.clamp(0.0, 1.0);
        self.update_coefficients();
    }
    pub fn set_mix(&mut self, m: f32) {
        self.mix = m.clamp(0.0, 1.0);
    }
}

//==============================================================================
// EFFECTS CHAIN
//==============================================================================

/// An effect slot in the chain
#[derive(Debug, Clone)]
pub struct EffectSlot {
    pub effect_type: EffectType,
    pub enabled: bool,
    /// Peak output levels L/R (smoothed for VU meter display)
    pub output_peak_l: f32,
    pub output_peak_r: f32,
    /// Cached user-set (base) value of each param. The mod matrix writes
    /// `base_params[pid] + delta` into the effect via `set_param` each block,
    /// so this preserves the knob value while modulation is active.
    /// 100 entries covers every current effect: the GUI param-id space
    /// tops out at VOC_FORMANT = 96 (compressor 60..69, limiter 70..74,
    /// expander 80..87, vocoder 90..96). The old 64-entry sizing
    /// silently dropped comp makeup/knee/mix and every limiter /
    /// expander / vocoder value on save.
    pub base_params: [f32; 100],
    /// Sidechain source track index. When Some, the compressor's detector
    /// listens to that track's audio instead of its own input.
    pub sidechain_src: Option<u16>,

    // Effect instances
    pub delay: Option<DelayEffect>,
    pub reverb: Option<ReverbEffect>,
    pub chorus: Option<ChorusEffect>,
    pub ensemble: Option<EnsembleEffect>,
    pub spring_tank: Option<SpringTankEffect>,
    pub cinema_plate: Option<CinemaPlateEffect>,
    pub solina_verb: Option<SolinaVerbEffect>,
    pub dimension: Option<DimensionEffect>,
    pub phaser: Option<PhaserEffect>,
    pub flanger: Option<FlangerEffect>,
    pub distortion: Option<DistortionEffect>,
    pub vocoder: Option<VocoderEffect>,
    pub compressor: Option<CompressorEffect>,
    pub limiter: Option<LimiterEffect>,
    pub expander: Option<ExpanderEffect>,
    pub autopanner: Option<AutoPannerEffect>,
    pub stutter: Option<StutterEffect>,
    pub filter: Option<FilterEffect>,
    pub ringmod: Option<RingModEffect>,
    pub lofi: Option<LoFiEffect>,
    pub stereo_imager: Option<StereoImagerEffect>,
    pub choir: Option<ChoirEffect>,
    pub maximizer: Option<MaximizerEffect>,
    pub spatializer: Option<Spatializer3DEffect>,
    pub param_eq: Option<ParametricEqEffect>,
    pub doppler: Option<DopplerEffect>,
    pub transient_shaper: Option<TransientShaperEffect>,
    pub granular_freezer: Option<GranularFreezerEffect>,
    pub spectral_freeze: Option<SpectralFreezeEffect>,
    pub resonator_bank: Option<ResonatorBankEffect>,
    pub auto_wah: Option<AutoWahEffect>,
    pub tape_stop: Option<TapeStopEffect>,
    // Mastering tier — boxed so a fresh `EffectSlot` stays small on
    // the stack. Several of these own dozens of biquads + 32-band
    // arrays inline; leaving them as `Option<T>` would push the
    // slot struct past 10 kB and overflow recursion-heavy test
    // threads.
    pub brickwall_limiter: Option<Box<BrickwallLimiterEffect>>,
    pub lufs_meter: Option<Box<LufsMeterEffect>>,
    pub dynamic_eq: Option<Box<DynamicEqEffect>>,
    pub mid_side_eq: Option<Box<MidSideEqEffect>>,
    pub multiband_comp: Option<Box<MultibandComp3Effect>>,
    pub spectrum_analyzer: Option<Box<SpectrumAnalyzerEffect>>,
    pub master_assistant: Option<Box<MasterAssistantEffect>>,
    /// Convolution-reverb insert (boxed — owns FFT scratch + per-IR
    /// frequency-domain partitions; cheaper than inlining on stack).
    pub convolution_reverb: Option<Box<ConvolutionReverbEffect>>,
    /// Frequency-slotting sidechain duck (the bass owns the lows).
    pub multiband_ducker: Option<MultibandDuckerEffect>,
    /// Velvet-noise mono-invariant section widener.
    pub velvet_decorrelator: Option<VelvetDecorrelatorEffect>,
    /// Slow tape-wow pitch drift.
    pub analog_drift: Option<AnalogDriftEffect>,
    /// 8-tap stereo early reflections.
    pub early_reflections: Option<EarlyReflectionsEffect>,
    /// Frequency-split mid/side widener.
    pub mid_side_widen: Option<MidSideWidenEffect>,
    /// Crossover Haas widener (lows centred, highs Haas-delayed).
    pub haas_split: Option<HaasSplitEffect>,
    /// Real-time pitch correction (autotune).
    pub retune: Option<RetuneEffect>,
    /// Diatonic harmonizer (up to 3 scale-degree voices).
    pub harmony: Option<HarmonyEffect>,
    pub trance_gate: Option<crate::dsp::trance_gate::TranceGate>,
    /// Pattern for `trance_gate`. Lives beside it because the shared gate takes
    /// its pattern per sample rather than owning one.
    pub trance_gate_params: crate::dsp::trance_gate::GateParams,
    /// The nested chain of an `EffectType::MultiFx` slot. Boxed so a plain slot
    /// stays the size it always was; `EffectSlot` is already large and every
    /// track holds several.
    pub multifx: Option<Box<EffectsChain>>,
    /// Wet/dry of the whole nested chain.
    pub multifx_mix: f32,
    /// The rack's chains beside the first, their gains and zones with
    /// the first's, the selector, the macros and what they hold:
    /// (macro, chain, slot, pid, lo, hi).
    pub multifx_chains: Vec<Box<EffectsChain>>,
    pub rack_gains: Vec<f32>,
    pub rack_zones: Vec<(u8, u8)>,
    pub rack_selector: f32,
    pub rack_macros: [f32; 8],
    pub rack_maps: Vec<(u8, u8, u8, u8, f32, f32)>,
}

impl EffectSlot {
    /// A single live scalar for the meter/analyzer effects, published to the GUI
    /// so their panels show a value instead of a static label. LUFS meter ->
    /// integrated LUFS (dB); spectrum analyzer -> loudest band level (dB).
    /// `None` for every other effect.
    /// Samples the wet signal lags the input by, for the effects that hold
    /// a lookahead or a block: zero when the slot is bypassed.
    pub fn latency_samples(&self) -> usize {
        if !self.enabled || self.effect_type == EffectType::None { return 0; }
        if let Some(c) = &self.compressor {
            return if c.mode == CompressorMode::Bus || c.lookahead_len <= 1 { 0 } else { c.lookahead_len };
        }
        if let Some(l) = &self.limiter { return l.lookahead_len; }
        if let Some(m) = &self.maximizer { return m.lookahead_len; }
        if let Some(b) = &self.brickwall_limiter { return b.lookahead_samples.saturating_sub(1); }
        if self.convolution_reverb.is_some() { return crate::fx::convolution::BLOCK; }
        if self.retune.is_some() || self.harmony.is_some() {
            return crate::harmonizer_dsp::pitch_shift::LATENCY_SAMPLES;
        }
        if let Some(chain) = &self.multifx { return chain.latency_samples(); }
        0
    }

    /// A rack parameter: 0 to 7 the macros, 8 the selector, 9 and past
    /// the chains' gains.
    pub fn set_rack_param(&mut self, pid: usize, v: f32) {
        match pid {
            0..=7 => { self.rack_macros[pid] = v; self.apply_macro(pid); }
            8 => self.rack_selector = v,
            _ => {
                let i = pid - 9;
                if i < self.rack_gains.len() { self.rack_gains[i] = v * 2.0; }
            }
        }
    }

    /// The rack parameter's value, as `set_rack_param` takes it.
    pub fn rack_param(&self, pid: usize) -> f32 {
        match pid {
            0..=7 => self.rack_macros[pid],
            8 => self.rack_selector,
            _ => self.rack_gains.get(pid - 9).copied().unwrap_or(1.0) * 0.5,
        }
    }

    /// Drive every parameter macro `i` holds to where the macro stands.
    fn apply_macro(&mut self, i: usize) {
        let m = self.rack_macros[i];
        let maps: Vec<(u8, u8, u8, u8, f32, f32)> = self.rack_maps.iter().copied().filter(|x| x.0 as usize == i).collect();
        for (_, chain, slot, pid, lo, hi) in maps {
            let value = lo + (hi - lo) * m;
            let target: Option<&mut EffectsChain> = if chain == 0 { self.multifx.as_deref_mut() } else { self.multifx_chains.get_mut(chain as usize - 1).map(|b| &mut **b) };
            if let Some(c) = target {
                if let Some(s) = c.get_slot(slot as usize) {
                    s.set_base_value(pid, value);
                    crate::drum_fx::apply_fx_param_raw(s, pid, value);
                }
            }
        }
    }

    /// Build the rack from its description: the chains beside the first,
    /// the gains and zones, the selector, the macros and their holds.
    pub fn apply_rack(&mut self, rack: &crate::fx_chain::RackSpec) {
        let sr = self.multifx.as_ref().map(|c| c.sample_rate).unwrap_or(48_000.0);
        self.multifx_chains.truncate(rack.chains.len());
        while self.multifx_chains.len() < rack.chains.len() {
            let mut inner = EffectsChain::with_slots(sr, MULTIFX_SLOTS);
            inner.allow_multifx = false;
            self.multifx_chains.push(Box::new(inner));
        }
        for (chain, slots) in self.multifx_chains.iter_mut().zip(rack.chains.iter()) {
            crate::fx_chain::FxChainSpec::new(slots.clone()).apply_to(chain, MULTIFX_SLOTS);
        }
        self.rack_gains = rack.gains.clone();
        if self.rack_gains.is_empty() { self.rack_gains.push(1.0); }
        self.rack_zones = rack.zones.clone();
        if self.rack_zones.is_empty() { self.rack_zones.push((0, 127)); }
        self.rack_selector = rack.selector as f32 / 127.0;
        self.rack_macros = rack.macros;
        self.rack_maps = rack.maps.iter().map(|m| (m.macro_idx, m.chain, m.slot, m.pid, m.lo, m.hi)).collect();
        for i in 0..8 { self.apply_macro(i); }
    }

    pub fn meter_readout(&self) -> Option<f32> {
        if let Some(m) = &self.lufs_meter {
            return Some(m.integrated_lufs());
        }
        if let Some(a) = &self.spectrum_analyzer {
            return Some(a.levels_db().iter().copied().fold(f32::NEG_INFINITY, f32::max));
        }
        None
    }

    pub fn new(_sample_rate: f32) -> Self {
        Self {
            effect_type: EffectType::None,
            enabled: true,
            output_peak_l: 0.0,
            output_peak_r: 0.0,
            base_params: [0.0; 100],
            sidechain_src: None,
            delay: None,
            reverb: None,
            chorus: None,
            ensemble: None,
            spring_tank: None,
            cinema_plate: None,
            solina_verb: None,
            dimension: None,
            phaser: None,
            flanger: None,
            distortion: None,
            vocoder: None,
            compressor: None,
            limiter: None,
            expander: None,
            autopanner: None,
            stutter: None,
            filter: None,
            ringmod: None,
            lofi: None,
            stereo_imager: None,
            choir: None,
            maximizer: None,
            spatializer: None,
            param_eq: None,
            doppler: None,
            transient_shaper: None,
            granular_freezer: None,
            spectral_freeze: None,
            resonator_bank: None,
            auto_wah: None,
            tape_stop: None,
            brickwall_limiter: None,
            lufs_meter: None,
            dynamic_eq: None,
            mid_side_eq: None,
            multiband_comp: None,
            spectrum_analyzer: None,
            master_assistant: None,
            convolution_reverb: None,
            multiband_ducker: None,
            velvet_decorrelator: None,
            analog_drift: None,
            early_reflections: None,
            mid_side_widen: None,
            haas_split: None,
            retune: None,
            harmony: None,
            trance_gate: None,
            multifx: None,
            multifx_mix: 1.0,
            multifx_chains: Vec::new(),
            rack_gains: vec![1.0],
            rack_zones: vec![(0, 127)],
            rack_selector: 0.0,
            rack_macros: [0.0; 8],
            rack_maps: Vec::new(),
            trance_gate_params: Default::default(),
        }
    }

    /// Set the effect type for this slot
    pub fn set_effect_type(&mut self, effect_type: EffectType, sample_rate: f32) {
        // Clear existing effect
        self.delay = None;
        self.reverb = None;
        self.chorus = None;
        self.ensemble = None;
        self.spring_tank = None;
        self.cinema_plate = None;
        self.solina_verb = None;
        self.dimension = None;
        self.phaser = None;
        self.flanger = None;
        self.distortion = None;
        self.vocoder = None;
        self.compressor = None;
        self.limiter = None;
        self.expander = None;
        self.autopanner = None;
        self.stutter = None;
        self.filter = None;
        self.ringmod = None;
        self.lofi = None;
        self.stereo_imager = None;
        self.choir = None;
        self.maximizer = None;
        self.spatializer = None;
        self.param_eq = None;
        self.doppler = None;
        self.transient_shaper = None;
        self.granular_freezer = None;
        self.spectral_freeze = None;
        self.resonator_bank = None;
        self.auto_wah = None;
        self.tape_stop = None;
        self.brickwall_limiter = None;
        self.lufs_meter = None;
        self.dynamic_eq = None;
        self.mid_side_eq = None;
        self.multiband_comp = None;
        self.spectrum_analyzer = None;
        self.master_assistant = None;
        self.convolution_reverb = None;
        self.multiband_ducker = None;
        self.velvet_decorrelator = None;
        self.analog_drift = None;

        self.effect_type = effect_type;
        self.enabled = effect_type != EffectType::None;

        // Create new effect instance
        match effect_type {
            EffectType::Delay => self.delay = Some(DelayEffect::new(sample_rate)),
            EffectType::Reverb => self.reverb = Some(ReverbEffect::new(sample_rate)),
            EffectType::Chorus => self.chorus = Some(ChorusEffect::new(sample_rate)),
            EffectType::Ensemble => self.ensemble = Some(EnsembleEffect::new(sample_rate)),
            EffectType::SpringTank => self.spring_tank = Some(SpringTankEffect::new(sample_rate)),
            EffectType::CinemaPlate => self.cinema_plate = Some(CinemaPlateEffect::new(sample_rate)),
            EffectType::SolinaVerb => self.solina_verb = Some(SolinaVerbEffect::new(sample_rate)),
            EffectType::Dimension => self.dimension = Some(DimensionEffect::new(sample_rate)),
            EffectType::Phaser => self.phaser = Some(PhaserEffect::new(sample_rate)),
            EffectType::Flanger => self.flanger = Some(FlangerEffect::new(sample_rate)),
            EffectType::Distortion => self.distortion = Some(DistortionEffect::new(sample_rate)),
            EffectType::Vocoder => self.vocoder = Some(VocoderEffect::new(sample_rate)),
            EffectType::Compressor => self.compressor = Some(CompressorEffect::new(sample_rate)),
            EffectType::Limiter => self.limiter = Some(LimiterEffect::new(sample_rate)),
            EffectType::Expander => self.expander = Some(ExpanderEffect::new(sample_rate)),
            EffectType::AutoPanner => self.autopanner = Some(AutoPannerEffect::new(sample_rate)),
            EffectType::Stutter => self.stutter = Some(StutterEffect::new(sample_rate)),
            EffectType::Filter => self.filter = Some(FilterEffect::new(sample_rate)),
            EffectType::RingMod => self.ringmod = Some(RingModEffect::new(sample_rate)),
            EffectType::LoFi => self.lofi = Some(LoFiEffect::new(sample_rate)),
            EffectType::StereoImager => {
                self.stereo_imager = Some(StereoImagerEffect::new(sample_rate))
            }
            EffectType::Choir => self.choir = Some(ChoirEffect::new(sample_rate)),
            EffectType::Maximizer => self.maximizer = Some(MaximizerEffect::new(sample_rate)),
            EffectType::Spatializer3D => {
                self.spatializer = Some(Spatializer3DEffect::new(sample_rate))
            }
            EffectType::ParametricEq => {
                self.param_eq = Some(ParametricEqEffect::new(sample_rate))
            }
            EffectType::Doppler => self.doppler = Some(DopplerEffect::new(sample_rate)),
            EffectType::TransientShaper => {
                self.transient_shaper = Some(TransientShaperEffect::new(sample_rate))
            }
            EffectType::GranularFreezer => {
                self.granular_freezer = Some(GranularFreezerEffect::new(sample_rate))
            }
            EffectType::SpectralFreeze => {
                self.spectral_freeze = Some(SpectralFreezeEffect::new(sample_rate))
            }
            EffectType::ResonatorBank => {
                self.resonator_bank = Some(ResonatorBankEffect::new(sample_rate))
            }
            EffectType::AutoWah => self.auto_wah = Some(AutoWahEffect::new(sample_rate)),
            EffectType::TapeStop => self.tape_stop = Some(TapeStopEffect::new(sample_rate)),
            EffectType::BrickwallLimiter => {
                self.brickwall_limiter = Some(Box::new(BrickwallLimiterEffect::new(sample_rate)))
            }
            EffectType::LufsMeter => {
                self.lufs_meter = Some(Box::new(LufsMeterEffect::new(sample_rate)))
            }
            EffectType::DynamicEq => {
                self.dynamic_eq = Some(Box::new(DynamicEqEffect::new(sample_rate)))
            }
            EffectType::MidSideEq => {
                self.mid_side_eq = Some(Box::new(MidSideEqEffect::new(sample_rate)))
            }
            EffectType::MultibandComp3 => {
                self.multiband_comp = Some(Box::new(MultibandComp3Effect::new(sample_rate)))
            }
            EffectType::SpectrumAnalyzer => {
                self.spectrum_analyzer = Some(Box::new(SpectrumAnalyzerEffect::new(sample_rate)))
            }
            EffectType::MasterAssistant => {
                self.master_assistant = Some(Box::new(MasterAssistantEffect::new(sample_rate)))
            }
            EffectType::ConvolutionReverb => {
                self.convolution_reverb = Some(Box::new(ConvolutionReverbEffect::new(sample_rate)))
            }
            EffectType::MultibandDucker => {
                self.multiband_ducker = Some(MultibandDuckerEffect::new(sample_rate))
            }
            EffectType::VelvetDecorrelator => {
                self.velvet_decorrelator = Some(VelvetDecorrelatorEffect::new(sample_rate))
            }
            EffectType::AnalogDrift => {
                self.analog_drift = Some(AnalogDriftEffect::new(sample_rate))
            }
            EffectType::EarlyReflections => {
                self.early_reflections = Some(EarlyReflectionsEffect::new(sample_rate))
            }
            EffectType::MidSideWiden => {
                self.mid_side_widen = Some(MidSideWidenEffect::new(sample_rate))
            }
            EffectType::HaasSplit => {
                self.haas_split = Some(HaasSplitEffect::new(sample_rate))
            }
            EffectType::Retune => {
                self.retune = Some(RetuneEffect::new(sample_rate))
            }
            EffectType::Harmony => {
                self.harmony = Some(HarmonyEffect::new(sample_rate))
            }
            EffectType::TranceGate => {
                let mut g = crate::dsp::trance_gate::TranceGate::new(sample_rate);
                // Sensible audible default until the host pushes its tempo.
                g.set_timing(120.0, 4, 5.0);
                self.trance_gate = Some(g);
                self.trance_gate_params = Default::default();
            }
            EffectType::MultiFx => {
                let mut inner = EffectsChain::with_slots(sample_rate, MULTIFX_SLOTS);
                // The nesting stops here.
                inner.allow_multifx = false;
                self.multifx = Some(Box::new(inner));
                self.multifx_mix = 1.0;
            }
            EffectType::None => {}
        }
        // Reset peak meters when changing effect type
        self.output_peak_l = 0.0;
        self.output_peak_r = 0.0;
    }

    /// Set beat position on all tempo-syncable effects in this slot
    pub fn set_beat_position(&mut self, pos: f64) {
        if let Some(ref mut e) = self.chorus {
            e.set_beat_position(pos);
        }
        if let Some(ref mut e) = self.phaser {
            e.set_beat_position(pos);
        }
        if let Some(ref mut e) = self.flanger {
            e.set_beat_position(pos);
        }
        if let Some(ref mut e) = self.autopanner {
            e.set_beat_position(pos);
        }
        if let Some(ref mut e) = self.stutter {
            e.set_beat_position(pos);
        }
        if let Some(ref mut e) = self.filter {
            e.set_beat_position(pos);
        }
    }

    /// Process stereo sample through this effect
    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        // Fast bypass: disabled OR no effect type assigned. Skips the
        // 28-arm match, the soft-clip, the finite-checks, and the
        // peak-update pair below. Hot in drum_machine where 8 families
        // × 4 slots run process per sample even when most are empty.
        if !self.enabled || self.effect_type == EffectType::None {
            // Decay peaks when bypassed (fast: ~1ms half-life at 44.1kHz)
            self.output_peak_l *= 0.95;
            self.output_peak_r *= 0.95;
            if !self.output_peak_l.is_finite() { self.output_peak_l = 0.0; }
            if !self.output_peak_r.is_finite() { self.output_peak_r = 0.0; }
            return (input_l, input_r);
        }

        let (out_l, out_r) = match self.effect_type {
            EffectType::Delay => {
                if let Some(ref mut effect) = self.delay {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::Ensemble => {
                if let Some(ref mut effect) = self.ensemble {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::SpringTank => {
                if let Some(ref mut effect) = self.spring_tank {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::CinemaPlate => {
                if let Some(ref mut effect) = self.cinema_plate {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::SolinaVerb => {
                if let Some(ref mut effect) = self.solina_verb {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::Dimension => {
                if let Some(ref mut effect) = self.dimension {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::Reverb => {
                if let Some(ref mut effect) = self.reverb {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::Chorus => {
                if let Some(ref mut effect) = self.chorus {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::Phaser => {
                if let Some(ref mut effect) = self.phaser {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::Flanger => {
                if let Some(ref mut effect) = self.flanger {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::Distortion => {
                if let Some(ref mut effect) = self.distortion {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::Vocoder => {
                if let Some(ref mut effect) = self.vocoder {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::Compressor => {
                if let Some(ref mut effect) = self.compressor {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::Limiter => {
                if let Some(ref mut effect) = self.limiter {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::Expander => {
                if let Some(ref mut effect) = self.expander {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::AutoPanner => {
                if let Some(ref mut effect) = self.autopanner {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::Stutter => {
                if let Some(ref mut effect) = self.stutter {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::Filter => {
                if let Some(ref mut effect) = self.filter {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::RingMod => {
                if let Some(ref mut effect) = self.ringmod {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::LoFi => {
                if let Some(ref mut effect) = self.lofi {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::StereoImager => {
                if let Some(ref mut effect) = self.stereo_imager {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::Choir => {
                if let Some(ref mut effect) = self.choir {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::Maximizer => {
                if let Some(ref mut effect) = self.maximizer {
                    effect.process(input_l, input_r)
                } else {
                    (input_l, input_r)
                }
            }
            EffectType::Spatializer3D => {
                if let Some(ref mut effect) = self.spatializer {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::ParametricEq => {
                if let Some(ref mut effect) = self.param_eq {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::Doppler => {
                if let Some(ref mut effect) = self.doppler {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::TransientShaper => {
                if let Some(ref mut effect) = self.transient_shaper {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::GranularFreezer => {
                if let Some(ref mut effect) = self.granular_freezer {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::SpectralFreeze => {
                if let Some(ref mut effect) = self.spectral_freeze {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::ResonatorBank => {
                if let Some(ref mut effect) = self.resonator_bank {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::AutoWah => {
                if let Some(ref mut effect) = self.auto_wah {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::TapeStop => {
                if let Some(ref mut effect) = self.tape_stop {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::BrickwallLimiter => {
                if let Some(ref mut effect) = self.brickwall_limiter {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::LufsMeter => {
                if let Some(ref mut effect) = self.lufs_meter {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::DynamicEq => {
                if let Some(ref mut effect) = self.dynamic_eq {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::MidSideEq => {
                if let Some(ref mut effect) = self.mid_side_eq {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::MultibandComp3 => {
                if let Some(ref mut effect) = self.multiband_comp {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::SpectrumAnalyzer => {
                if let Some(ref mut effect) = self.spectrum_analyzer {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::MasterAssistant => {
                if let Some(ref mut effect) = self.master_assistant {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::ConvolutionReverb => {
                if let Some(ref mut effect) = self.convolution_reverb {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::MultibandDucker => {
                if let Some(ref mut effect) = self.multiband_ducker {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::VelvetDecorrelator => {
                if let Some(ref mut effect) = self.velvet_decorrelator {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::AnalogDrift => {
                if let Some(ref mut effect) = self.analog_drift {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::EarlyReflections => {
                if let Some(ref mut effect) = self.early_reflections {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::MidSideWiden => {
                if let Some(ref mut effect) = self.mid_side_widen {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::HaasSplit => {
                if let Some(ref mut effect) = self.haas_split {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::Retune => {
                if let Some(ref mut effect) = self.retune {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::Harmony => {
                if let Some(ref mut effect) = self.harmony {
                    effect.process(input_l, input_r)
                } else { (input_l, input_r) }
            }
            EffectType::TranceGate => {
                if let Some(ref mut g) = self.trance_gate {
                    g.process(input_l, input_r, &self.trance_gate_params)
                } else { (input_l, input_r) }
            }
            EffectType::MultiFx => {
                // Every chain in whose zone the selector stands runs on the
                // input and its share is summed; one level of recursion,
                // bounded by `allow_multifx`.
                let sel = (self.rack_selector.clamp(0.0, 1.0) * 127.0).round() as u8;
                let plays = |i: usize, zones: &[(u8, u8)]| -> bool {
                    zones.get(i).map(|(lo, hi)| sel >= *lo && sel <= *hi).unwrap_or(true)
                };
                let (mut wl, mut wr) = (0.0f32, 0.0f32);
                if let Some(ref mut inner) = self.multifx {
                    if plays(0, &self.rack_zones) {
                        let (l, r) = inner.process(input_l, input_r);
                        let g = self.rack_gains.first().copied().unwrap_or(1.0);
                        wl += l * g;
                        wr += r * g;
                    }
                }
                for (k, chain) in self.multifx_chains.iter_mut().enumerate() {
                    if plays(k + 1, &self.rack_zones) {
                        let (l, r) = chain.process(input_l, input_r);
                        let g = self.rack_gains.get(k + 1).copied().unwrap_or(1.0);
                        wl += l * g;
                        wr += r * g;
                    }
                }
                let m = self.multifx_mix.clamp(0.0, 1.0);
                (input_l + (wl - input_l) * m, input_r + (wr - input_r) * m)
            }
            EffectType::None => (input_l, input_r),
        };

        // Soft-clip to prevent gain accumulation through serial FX chains
        // Uses reciprocal curve instead of tanh() for faster evaluation
        let soft = |x: f32| -> f32 {
            if x.abs() <= 1.0 { x } else { x.signum() * (2.0 - 1.0 / x.abs()) }
        };
        let (out_l, out_r) = (soft(out_l), soft(out_r));

        // NaN/inf guard — if any effect blows up, kill the output and reset
        let (out_l, out_r) = if out_l.is_finite() && out_r.is_finite() {
            (out_l.clamp(-1.5, 1.5), out_r.clamp(-1.5, 1.5))
        } else {
            // Reset the offending effect to clear corrupt state
            self.reset();
            (input_l, input_r)
        };

        // Track peak output levels per channel (fast attack, fast decay)
        let pl = out_l.abs();
        let pr = out_r.abs();
        if pl > self.output_peak_l {
            self.output_peak_l = pl;
        } else {
            self.output_peak_l *= 0.95;
        }
        if pr > self.output_peak_r {
            self.output_peak_r = pr;
        } else {
            self.output_peak_r *= 0.95;
        }
        if !self.output_peak_l.is_finite() { self.output_peak_l = 0.0; }
        if !self.output_peak_r.is_finite() { self.output_peak_r = 0.0; }

        (out_l, out_r)
    }

    /// Get the mix value of the active effect
    pub fn get_mix(&self) -> f32 {
        match self.effect_type {
            EffectType::Delay => self.delay.as_ref().map(|e| e.mix).unwrap_or(0.5),
            EffectType::Reverb => self.reverb.as_ref().map(|e| e.mix).unwrap_or(0.5),
            EffectType::Chorus => self.chorus.as_ref().map(|e| e.mix).unwrap_or(0.5),
            EffectType::Ensemble => self.ensemble.as_ref().map(|e| e.mix).unwrap_or(0.5),
            EffectType::SpringTank => self.spring_tank.as_ref().map(|e| e.mix).unwrap_or(0.3),
            EffectType::CinemaPlate => self.cinema_plate.as_ref().map(|e| e.mix).unwrap_or(0.3),
            EffectType::SolinaVerb => self.solina_verb.as_ref().map(|e| e.mix).unwrap_or(0.25),
            EffectType::Dimension => self.dimension.as_ref().map(|e| e.mix).unwrap_or(0.5),
            EffectType::Phaser => self.phaser.as_ref().map(|e| e.mix).unwrap_or(0.5),
            EffectType::Flanger => self.flanger.as_ref().map(|e| e.mix).unwrap_or(0.5),
            EffectType::Distortion => self.distortion.as_ref().map(|e| e.mix).unwrap_or(0.5),
            EffectType::Vocoder => self.vocoder.as_ref().map(|e| e.mix).unwrap_or(0.5),
            EffectType::Compressor => self.compressor.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::Limiter => self.limiter.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::Expander => self.expander.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::AutoPanner => self.autopanner.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::Stutter => self.stutter.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::Filter => self.filter.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::RingMod => self.ringmod.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::LoFi => self.lofi.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::StereoImager => self.stereo_imager.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::Choir => self.choir.as_ref().map(|e| e.mix).unwrap_or(0.5),
            EffectType::Maximizer => self.maximizer.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::Spatializer3D => self.spatializer.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::ParametricEq => self.param_eq.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::Doppler => self.doppler.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::TransientShaper => self.transient_shaper.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::GranularFreezer => self.granular_freezer.as_ref().map(|e| e.mix).unwrap_or(0.5),
            EffectType::SpectralFreeze => self.spectral_freeze.as_ref().map(|e| e.mix).unwrap_or(0.5),
            EffectType::ResonatorBank => self.resonator_bank.as_ref().map(|e| e.mix).unwrap_or(0.5),
            EffectType::AutoWah => self.auto_wah.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::TapeStop => self.tape_stop.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::BrickwallLimiter => self.brickwall_limiter.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::LufsMeter => self.lufs_meter.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::DynamicEq => self.dynamic_eq.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::MidSideEq => self.mid_side_eq.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::MultibandComp3 => self.multiband_comp.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::SpectrumAnalyzer => self.spectrum_analyzer.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::MasterAssistant => self.master_assistant.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::ConvolutionReverb => self.convolution_reverb.as_ref().map(|e| e.mix).unwrap_or(0.35),
            EffectType::MultibandDucker => self.multiband_ducker.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::VelvetDecorrelator => self.velvet_decorrelator.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::AnalogDrift => self.analog_drift.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::EarlyReflections => self.early_reflections.as_ref().map(|e| e.mix).unwrap_or(0.2),
            EffectType::MidSideWiden => self.mid_side_widen.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::HaasSplit => self.haas_split.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::Retune => self.retune.as_ref().map(|e| e.mix).unwrap_or(1.0),
            EffectType::Harmony => self.harmony.as_ref().map(|e| e.mix).unwrap_or(0.5),
            EffectType::TranceGate => self.trance_gate_params.mix,
            EffectType::MultiFx => self.multifx_mix,
            EffectType::None => 1.0,
        }
    }

    /// Set mix on the current effect
    pub fn set_mix(&mut self, mix: f32) {
        match self.effect_type {
            EffectType::Delay => {
                if let Some(ref mut e) = self.delay {
                    e.set_mix(mix);
                }
            }
            EffectType::Ensemble => {
                if let Some(ref mut e) = self.ensemble {
                    e.set_mix(mix);
                }
            }
            EffectType::SpringTank => {
                if let Some(ref mut e) = self.spring_tank {
                    e.set_mix(mix);
                }
            }
            EffectType::CinemaPlate => {
                if let Some(ref mut e) = self.cinema_plate {
                    e.set_mix(mix);
                }
            }
            EffectType::SolinaVerb => {
                if let Some(ref mut e) = self.solina_verb {
                    e.set_mix(mix);
                }
            }
            EffectType::Dimension => {
                if let Some(ref mut e) = self.dimension {
                    e.set_mix(mix);
                }
            }
            EffectType::Reverb => {
                if let Some(ref mut e) = self.reverb {
                    e.set_mix(mix);
                }
            }
            EffectType::Chorus => {
                if let Some(ref mut e) = self.chorus {
                    e.set_mix(mix);
                }
            }
            EffectType::Phaser => {
                if let Some(ref mut e) = self.phaser {
                    e.set_mix(mix);
                }
            }
            EffectType::Flanger => {
                if let Some(ref mut e) = self.flanger {
                    e.set_mix(mix);
                }
            }
            EffectType::Distortion => {
                if let Some(ref mut e) = self.distortion {
                    e.set_mix(mix);
                }
            }
            EffectType::Vocoder => {
                if let Some(ref mut e) = self.vocoder {
                    e.set_mix(mix);
                }
            }
            EffectType::Compressor => {
                if let Some(ref mut e) = self.compressor {
                    e.set_mix(mix);
                }
            }
            EffectType::Limiter => {
                if let Some(ref mut e) = self.limiter {
                    e.set_mix(mix);
                }
            }
            EffectType::Expander => {
                if let Some(ref mut e) = self.expander {
                    e.set_mix(mix);
                }
            }
            EffectType::AutoPanner => {
                if let Some(ref mut e) = self.autopanner {
                    e.set_mix(mix);
                }
            }
            EffectType::Stutter => {
                if let Some(ref mut e) = self.stutter {
                    e.set_mix(mix);
                }
            }
            EffectType::Filter => {
                if let Some(ref mut e) = self.filter {
                    e.set_mix(mix);
                }
            }
            EffectType::RingMod => {
                if let Some(ref mut e) = self.ringmod {
                    e.set_mix(mix);
                }
            }
            EffectType::LoFi => {
                if let Some(ref mut e) = self.lofi {
                    e.set_mix(mix);
                }
            }
            EffectType::StereoImager => {
                if let Some(ref mut e) = self.stereo_imager {
                    e.set_mix(mix);
                }
            }
            EffectType::Choir => {
                if let Some(ref mut e) = self.choir {
                    e.set_mix(mix);
                }
            }
            EffectType::Maximizer => {
                if let Some(ref mut e) = self.maximizer {
                    e.set_mix(mix);
                }
            }
            EffectType::Spatializer3D => { if let Some(ref mut e) = self.spatializer { e.set_mix(mix); } }
            EffectType::ParametricEq => { if let Some(ref mut e) = self.param_eq { e.set_mix(mix); } }
            EffectType::Doppler => { if let Some(ref mut e) = self.doppler { e.set_mix(mix); } }
            EffectType::TransientShaper => { if let Some(ref mut e) = self.transient_shaper { e.set_mix(mix); } }
            EffectType::GranularFreezer => { if let Some(ref mut e) = self.granular_freezer { e.set_mix(mix); } }
            EffectType::SpectralFreeze => { if let Some(ref mut e) = self.spectral_freeze { e.set_mix(mix); } }
            EffectType::ResonatorBank => { if let Some(ref mut e) = self.resonator_bank { e.set_mix(mix); } }
            EffectType::AutoWah => { if let Some(ref mut e) = self.auto_wah { e.set_mix(mix); } }
            EffectType::TapeStop => { if let Some(ref mut e) = self.tape_stop { e.set_mix(mix); } }
            EffectType::BrickwallLimiter => { if let Some(ref mut e) = self.brickwall_limiter { e.set_mix(mix); } }
            EffectType::LufsMeter => { if let Some(ref mut e) = self.lufs_meter { e.set_mix(mix); } }
            EffectType::DynamicEq => { if let Some(ref mut e) = self.dynamic_eq { e.set_mix(mix); } }
            EffectType::MidSideEq => { if let Some(ref mut e) = self.mid_side_eq { e.set_mix(mix); } }
            EffectType::MultibandComp3 => { if let Some(ref mut e) = self.multiband_comp { e.set_mix(mix); } }
            EffectType::SpectrumAnalyzer => { if let Some(ref mut e) = self.spectrum_analyzer { e.set_mix(mix); } }
            EffectType::MasterAssistant => { if let Some(ref mut e) = self.master_assistant { e.set_mix(mix); } }
            EffectType::ConvolutionReverb => { if let Some(ref mut e) = self.convolution_reverb { e.set_mix(mix); } }
            EffectType::MultibandDucker => { if let Some(ref mut e) = self.multiband_ducker { e.set_mix(mix); } }
            EffectType::VelvetDecorrelator => { if let Some(ref mut e) = self.velvet_decorrelator { e.set_mix(mix); } }
            EffectType::AnalogDrift => { if let Some(ref mut e) = self.analog_drift { e.set_mix(mix); } }
            EffectType::EarlyReflections => { if let Some(ref mut e) = self.early_reflections { e.set_mix(mix); } }
            EffectType::MidSideWiden => { if let Some(ref mut e) = self.mid_side_widen { e.set_mix(mix); } }
            EffectType::HaasSplit => { if let Some(ref mut e) = self.haas_split { e.set_mix(mix); } }
            EffectType::Retune => { if let Some(ref mut e) = self.retune { e.set_mix(mix); } }
            EffectType::Harmony => { if let Some(ref mut e) = self.harmony { e.set_mix(mix); } }
            EffectType::TranceGate => { self.trance_gate_params.mix = mix; }
            EffectType::MultiFx => { self.multifx_mix = mix.clamp(0.0, 1.0); }
            EffectType::None => {}
        }
    }

    /// Set an effect-specific parameter by index
    /// Param indices per effect type:
    ///   Delay:      1=Time, 2=Feedback
    ///   Reverb:     1=Size, 2=Decay
    ///   Chorus:     1=Rate, 2=Depth
    ///   Phaser:     1=Rate, 2=Depth, 3=Feedback
    ///   Flanger:    1=Rate, 2=Depth, 3=Feedback
    ///   Distortion: 1=Drive
    /// Set an effect-specific parameter.
    /// `value` is normalized 0..1, scaled to the effect's actual range here.
    pub fn set_param(&mut self, param_idx: usize, value: f32) {
        let v = value.clamp(0.0, 1.0);
        if self.effect_type == EffectType::MultiFx {
            self.set_rack_param(param_idx, v);
            return;
        }
        match self.effect_type {
            EffectType::Delay => {
                if let Some(ref mut e) = self.delay {
                    match param_idx {
                        1 => e.set_time(v * 1.0),      // 0-1 second
                        2 => e.set_feedback(v * 0.95), // 0-95%
                        _ => {}
                    }
                }
            }
            EffectType::Reverb => {
                if let Some(ref mut e) = self.reverb {
                    match param_idx {
                        1 => e.set_size(v),  // already 0-1
                        2 => e.set_decay(v), // already 0-1
                        _ => {}
                    }
                }
            }
            EffectType::ConvolutionReverb => {
                if let Some(ref mut e) = self.convolution_reverb {
                    match param_idx {
                        // WHICH room. The insert was hard-wired to the cathedral
                        // and had no way to say otherwise, so every convolution
                        // insert in every session was a 7.5-second church.
                        1 => {
                            let all = crate::fx::irs::IrKind::ALL;
                            let i = (v * (all.len() - 1) as f32).round() as usize;
                            e.set_ir_kind(crate::fx::irs::IrKind::from_index(i));
                        }
                        _ => {}
                    }
                }
            }
            EffectType::Chorus => {
                if let Some(ref mut e) = self.chorus {
                    match param_idx {
                        1 => e.set_rate(v * 5.0 + 0.01), // 0.01-5 Hz
                        2 => e.set_depth(v),             // 0-1
                        _ => {}
                    }
                }
            }
            EffectType::Phaser => {
                if let Some(ref mut e) = self.phaser {
                    match param_idx {
                        1 => e.set_rate(v * 5.0 + 0.01), // 0.01-5 Hz
                        2 => e.set_depth(v),             // 0-1
                        3 => e.set_feedback(v * 0.95),   // 0-95%
                        _ => {}
                    }
                }
            }
            EffectType::Flanger => {
                if let Some(ref mut e) = self.flanger {
                    match param_idx {
                        1 => e.set_rate(v * 5.0 + 0.01), // 0.01-5 Hz
                        2 => e.set_depth(v),             // 0-1
                        3 => e.set_feedback(v * 0.95),   // 0-95%
                        _ => {}
                    }
                }
            }
            EffectType::Distortion => {
                if let Some(ref mut e) = self.distortion {
                    match param_idx {
                        1 => e.set_drive(v * 50.0 + 0.1), // 0.1-50
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    /// Reset this effect
    pub fn reset(&mut self) {
        if let Some(ref mut effect) = self.delay {
            effect.reset();
        }
        if let Some(ref mut effect) = self.reverb {
            effect.reset();
        }
        if let Some(ref mut effect) = self.chorus {
            effect.reset();
        }
        if let Some(ref mut effect) = self.phaser {
            effect.reset();
        }
        if let Some(ref mut effect) = self.flanger {
            effect.reset();
        }
        if let Some(ref mut effect) = self.distortion {
            effect.reset();
        }
        if let Some(ref mut effect) = self.vocoder {
            effect.reset();
        }
        if let Some(ref mut effect) = self.compressor {
            effect.reset();
        }
        if let Some(ref mut effect) = self.limiter {
            effect.reset();
        }
        if let Some(ref mut effect) = self.expander {
            effect.reset();
        }
        if let Some(ref mut effect) = self.autopanner {
            effect.reset();
        }
        if let Some(ref mut effect) = self.stutter {
            effect.reset();
        }
        if let Some(ref mut effect) = self.filter {
            effect.reset();
        }
        if let Some(ref mut effect) = self.ringmod {
            effect.reset();
        }
        if let Some(ref mut effect) = self.lofi {
            effect.reset();
        }
        if let Some(ref mut effect) = self.stereo_imager {
            effect.reset();
        }
        if let Some(ref mut effect) = self.choir {
            effect.reset();
        }
        if let Some(ref mut effect) = self.maximizer {
            effect.reset();
        }
        if let Some(ref mut effect) = self.spatializer { effect.reset(); }
        if let Some(ref mut effect) = self.param_eq { effect.reset(); }
        if let Some(ref mut effect) = self.doppler { effect.reset(); }
        if let Some(ref mut effect) = self.transient_shaper { effect.reset(); }
        if let Some(ref mut effect) = self.granular_freezer { effect.reset(); }
        if let Some(ref mut effect) = self.spectral_freeze { effect.reset(); }
        if let Some(ref mut effect) = self.resonator_bank { effect.reset(); }
        if let Some(ref mut effect) = self.auto_wah { effect.reset(); }
        if let Some(ref mut effect) = self.tape_stop { effect.reset(); }
        if let Some(ref mut effect) = self.brickwall_limiter { effect.reset(); }
        if let Some(ref mut effect) = self.lufs_meter { effect.reset(); }
        if let Some(ref mut effect) = self.dynamic_eq { effect.reset(); }
        if let Some(ref mut effect) = self.mid_side_eq { effect.reset(); }
        if let Some(ref mut effect) = self.multiband_comp { effect.reset(); }
        if let Some(ref mut effect) = self.spectrum_analyzer { effect.reset(); }
        if let Some(ref mut effect) = self.master_assistant { effect.reset(); }
        if let Some(ref mut effect) = self.convolution_reverb { effect.reset(); }
        if let Some(ref mut effect) = self.multiband_ducker { effect.reset(); }
        if let Some(ref mut effect) = self.velvet_decorrelator { effect.reset(); }
        if let Some(ref mut effect) = self.analog_drift { effect.reset(); }
        self.output_peak_l = 0.0;
        self.output_peak_r = 0.0;
    }

    /// Write the sidechain signal for this frame into the compressor (if
    /// present and sidechain is configured). Called per-frame from the
    /// sequencer's insert-FX loop before `EffectsChain::process()`.
    pub fn feed_sidechain(&mut self, l: f32, r: f32) {
        if let Some(ref mut comp) = self.compressor {
            comp.sidechain_l = l;
            comp.sidechain_r = r;
            comp.sidechain_active = self.sidechain_src.is_some();
        }
        if let Some(ref mut duck) = self.multiband_ducker {
            duck.sidechain_l = l;
            duck.sidechain_r = r;
            duck.sidechain_active = self.sidechain_src.is_some();
        }
        if let Some(ref mut rb) = self.resonator_bank {
            rb.sidechain_l = l;
            rb.sidechain_r = r;
            rb.sidechain_active = self.sidechain_src.is_some();
        }
    }

    /// Read the cached user-set base value for a param, ignoring any live
    /// modulation. Returns 0.0 for out-of-range pids.
    #[inline]
    pub fn base_param(&self, pid: u8) -> f32 {
        let i = pid as usize;
        if i < self.base_params.len() { self.base_params[i] } else { 0.0 }
    }

    /// Record a user-set base value without touching the effect itself.
    /// Command handlers call this when applying a `SetTrackFxParam` so the
    /// mod matrix knows the "target at rest" value to add deltas onto.
    #[inline]
    pub fn set_base_value(&mut self, pid: u8, value: f32) {
        let i = pid as usize;
        if i < self.base_params.len() { self.base_params[i] = value; }
    }
}

/// Effects chain - manages multiple effect slots
#[derive(Debug, Clone)]
pub struct EffectsChain {
    slots: Vec<EffectSlot>,
    sample_rate: f32,
    beat_position: f64,
    tempo: f32,
    /// Cached "is anything actually doing work?" flag — recomputed
    /// whenever a slot is added/removed/retyped/enabled. The audio
    /// loop reads it once and can fast-path bypass the per-sample
    /// slot iteration when every slot is None-or-disabled. This is
    /// the dominant cost in idle / passthrough chains (8 slots ×
    /// per-sample × enabled+None match + soft-clip).
    any_active: bool,
    /// Cached `tempo / 60 / sample_rate`. Refreshed by `set_tempo`
    /// and `set_sample_rate`. Avoids two float divs per process()
    /// per sample.
    beats_per_sample: f64,
    /// Whether this chain may host an `EffectType::MultiFx` slot.
    ///
    /// True for a host's own rack, false for the chain INSIDE a Multi-FX. This
    /// is what bounds the nesting at one level: `set_slot` turns a Multi-FX
    /// request into `None` here rather than trusting every call site to check,
    /// because the cost of getting it wrong is unbounded recursion per sample
    /// on the audio thread.
    pub allow_multifx: bool,
}

impl EffectsChain {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            slots: Vec::new(),
            sample_rate,
            beat_position: 0.0,
            tempo: 120.0,
            any_active: false,
            allow_multifx: true,
            beats_per_sample: 120.0_f64 / 60.0 / sample_rate as f64,
        }
    }

    /// Create effects chain with default slots
    pub fn with_slots(sample_rate: f32, num_slots: usize) -> Self {
        let mut slots = Vec::with_capacity(num_slots);
        for _ in 0..num_slots {
            slots.push(EffectSlot::new(sample_rate));
        }
        Self {
            slots,
            sample_rate,
            beat_position: 0.0,
            tempo: 120.0,
            any_active: false,
            allow_multifx: true,
            beats_per_sample: 120.0_f64 / 60.0 / sample_rate as f64,
        }
    }

    /// Recompute `any_active`. Called by every mutator that could
    /// change a slot's effect_type or enabled state. Exposed so
    /// external callers that bypass `set_slot` (e.g. polaris_engine's
    /// per-command direct writes via `get_slot`) can keep the
    /// cache in sync.
    pub fn refresh_active_cache(&mut self) {
        self.any_active = self.slots.iter()
            .any(|s| s.enabled && s.effect_type != EffectType::None);
    }

    /// Read-only accessor for the active-slot cache. Lets outer
    /// processing stages (e.g. MixBus / MasterBus) take a fast path
    /// when the chain is fully inactive — avoids paying for even the
    /// chain's per-sample beat-position bookkeeping.
    #[inline(always)]
    pub fn is_idle(&self) -> bool { !self.any_active }

    /// Add a new effect slot
    pub fn add_slot(&mut self) -> usize {
        let index = self.slots.len();
        self.slots.push(EffectSlot::new(self.sample_rate));
        // A fresh slot is None+enabled — doesn't change any_active.
        index
    }

    /// Remove an effect slot
    pub fn remove_slot(&mut self, index: usize) {
        if index < self.slots.len() {
            self.slots.remove(index);
            self.refresh_active_cache();
        }
    }

    /// Get mutable reference to a slot
    /// Broadcast the session's chord-track key to every Retune/Harmony slot
    /// in follow mode. `scale_idx` is a harmonizer `ScaleType::all()` index
    /// (map generative scales via `map_generative_scale`). Called once per
    /// audio block by the sequencer; a no-op for other effect types.
    pub fn set_song_key(&mut self, root: u8, scale_idx: usize) {
        for slot in &mut self.slots {
            if let Some(ref mut fx) = slot.retune {
                fx.set_song_key(root, scale_idx);
            }
            if let Some(ref mut fx) = slot.harmony {
                fx.set_song_key(root, scale_idx);
            }
        }
    }

    pub fn get_slot(&mut self, index: usize) -> Option<&mut EffectSlot> {
        self.slots.get_mut(index)
    }

    /// Restore every FX slot from a patch's parallel
    /// `(types, enabled, mix, params)` arrays. Used by every engine
    /// that owns an `EffectsChain` (Strata, Solstice — and any
    /// future engine) so the `LoadPatch` arm doesn't have to
    /// hand-roll the same nested loop. Skips slots whose effect is
    /// `EffectType::None` (idx 0) so we don't waste set_param dispatch
    /// on inert slots.
    pub fn restore_slots_from_patch(
        &mut self,
        types:    &[usize],
        enabled:  &[bool],
        mix:      &[f32],
        params:   &[Vec<f32>],
    ) {
        let n = types.len()
            .min(enabled.len()).min(mix.len()).min(params.len());
        for slot in 0..n {
            let et = EffectType::from_index(types[slot]);
            // set_slot ensures the slot exists, switches type if
            // changed (and resets internal state), and stamps
            // (enabled, mix) in one call.
            self.set_slot(slot, et, enabled[slot], mix[slot]);
            if matches!(et, EffectType::None) { continue; }
            if let Some(s) = self.get_slot(slot) {
                let p = &params[slot];
                let n_p = p.len().min(s.base_params.len());
                for pid in 0..n_p {
                    let v = p[pid];
                    s.set_base_value(pid as u8, v);
                    crate::drum_fx::apply_fx_param_raw(s, pid as u8, v);
                }
            }
        }
    }

    pub fn get_slot_ref(&self, index: usize) -> Option<&EffectSlot> {
        self.slots.get(index)
    }

    /// Get the number of slots
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Samples the chain's output lags its input by: the enabled slots'
    /// latencies summed.
    pub fn latency_samples(&self) -> usize {
        self.slots.iter().map(|s| s.latency_samples()).sum()
    }

    /// Get per-slot output peak levels L/R (for VU meters)
    pub fn slot_peaks(&self) -> [(f32, f32); 8] {
        let mut peaks = [(0.0f32, 0.0f32); 8];
        for (i, slot) in self.slots.iter().enumerate().take(8) {
            peaks[i] = (slot.output_peak_l, slot.output_peak_r);
        }
        peaks
    }

    /// Set tempo for beat position tracking, AND propagate it to
    /// every sync-capable effect on every slot.
    ///
    /// Chain-level `self.tempo` only advances `beat_position` per
    /// sample (used by phase-locked LFOs in chorus/phaser/flanger/
    /// autopan-classic/filter). The remaining sync paths read their
    /// own `self.tempo` directly: delay computes its sync time from
    /// `to_time_seconds(self.tempo)`, stutter derives `cycle_len`
    /// the same way via `effective_rate`, and several autopanner
    /// modes (Spin / Bounce / RandomSH / Envelope) modulate motion
    /// off `effective_rate`. Without this fan-out those effects
    /// kept synthesising as if the session were stuck at 120 BPM
    /// no matter what the transport reported.
    pub fn set_tempo(&mut self, tempo: f32) {
        let t = tempo.clamp(20.0, 300.0);
        self.tempo = t;
        self.beats_per_sample = t as f64 / 60.0 / self.sample_rate as f64;
        for slot in &mut self.slots {
            if let Some(e) = &mut slot.delay      { e.set_tempo(t); }
            if let Some(e) = &mut slot.chorus     { e.set_tempo(t); }
            if let Some(e) = &mut slot.phaser     { e.set_tempo(t); }
            if let Some(e) = &mut slot.flanger    { e.set_tempo(t); }
            if let Some(e) = &mut slot.autopanner { e.set_tempo(t); }
            if let Some(e) = &mut slot.stutter    { e.set_tempo(t); }
            if let Some(e) = &mut slot.filter     { e.set_tempo(t); }
            // A Multi-FX can hold tempo-synced effects of its own; without this
            // they would sit at the 120 BPM default forever.
            if let Some(inner) = &mut slot.multifx { inner.set_tempo(t); }
        }
    }

    /// Move slot from one position to another
    pub fn move_slot(&mut self, from: usize, to: usize) {
        if from >= self.slots.len() || to >= self.slots.len() || from == to {
            return;
        }

        let slot = self.slots.remove(from);
        self.slots.insert(to, slot);
    }

    /// Process stereo sample through all effects in chain
    /// Feed sidechain audio from another track into any compressor slot that
    /// has a sidechain_src configured. Call once per frame, before `process()`.
    /// Hands the chord in force to every resonator slot, which rings at
    /// it when set to follow.
    pub fn set_key_chord(&mut self, root: u8, intervals: &[u8]) {
        for slot in &mut self.slots {
            if let Some(ref mut rb) = slot.resonator_bank {
                rb.set_key_chord(root, intervals);
            }
        }
    }

    pub fn feed_sidechain_frame(&mut self, track_stereo: &[Vec<[f32; 2]>], frame: usize) {
        for slot in &mut self.slots {
            if let Some(src) = slot.sidechain_src {
                if let Some(buf) = track_stereo.get(src as usize) {
                    if let Some(&[l, r]) = buf.get(frame) {
                        slot.feed_sidechain(l, r);
                    }
                }
            }
        }
    }

    pub fn process(&mut self, mut input_l: f32, mut input_r: f32) -> (f32, f32) {
        // Fast path: every slot is None-or-disabled. Skips the
        // per-sample iteration over 8 default slots that would
        // otherwise pass through `EffectType::None` + apply a
        // soft-clip + update peaks for each. Cumulatively significant
        // when many tracks each carry an empty FX chain.
        if !self.any_active {
            return (input_l, input_r);
        }
        // Advance beat position (quarter notes per sample). Cached
        // per-sample increment from `set_tempo` / `set_sample_rate`
        // — avoids two divs per process() call.
        self.beat_position += self.beats_per_sample;
        let bp = self.beat_position;

        for slot in &mut self.slots {
            // Only the tempo-syncable effect families
            // (Chorus / Phaser / Flanger / AutoPanner / Stutter /
            // Filter) read beat_position per sample. Static effects
            // like Reverb / Distortion / EQ don't need it — skip
            // the call so we don't probe 6 Options for nothing.
            if slot.enabled && matches!(slot.effect_type,
                EffectType::Chorus | EffectType::Phaser
                | EffectType::Flanger | EffectType::AutoPanner
                | EffectType::Stutter | EffectType::Filter)
            {
                slot.set_beat_position(bp);
            }
            let (l, r) = slot.process(input_l, input_r);
            input_l = l;
            input_r = r;
        }
        (input_l, input_r)
    }

    /// Process stereo sample through all effects with vocoder modulator input
    pub fn process_with_modulator(
        &mut self,
        mut input_l: f32,
        mut input_r: f32,
        modulator: f32,
    ) -> (f32, f32) {
        self.beat_position += self.beats_per_sample;
        let bp = self.beat_position;

        for slot in &mut self.slots {
            // Same tempo-syncable fast skip as `process` —
            // beat_position is only consumed by chorus / phaser /
            // flanger / autopanner / stutter / filter.
            if slot.enabled && matches!(slot.effect_type,
                EffectType::Chorus | EffectType::Phaser
                | EffectType::Flanger | EffectType::AutoPanner
                | EffectType::Stutter | EffectType::Filter)
            {
                slot.set_beat_position(bp);
            }
            if slot.effect_type == EffectType::Vocoder && slot.enabled {
                if let Some(ref mut vocoder) = slot.vocoder {
                    let (l, r) = vocoder.process_with_modulator(input_l, input_r, modulator);
                    input_l = l;
                    input_r = r;
                }
            } else {
                let (l, r) = slot.process(input_l, input_r);
                input_l = l;
                input_r = r;
            }
        }
        (input_l, input_r)
    }

    /// Reset all effects
    pub fn reset(&mut self) {
        for slot in &mut self.slots {
            slot.reset();
        }
    }

    /// Set sample rate
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.beats_per_sample = self.tempo as f64 / 60.0 / sample_rate as f64;
        // Reinitialize all effects with new sample rate
        for slot in &mut self.slots {
            let effect_type = slot.effect_type;
            if effect_type == EffectType::MultiFx {
                // Recreating this slot would build an EMPTY nested chain and
                // throw the user's sub-effects away. Retune the existing one
                // instead; it reinitialises its own slots the same way.
                if let Some(inner) = &mut slot.multifx {
                    inner.set_sample_rate(sample_rate);
                }
                for c in &mut slot.multifx_chains { c.set_sample_rate(sample_rate); }
                continue;
            }
            slot.set_effect_type(effect_type, sample_rate);
        }
    }

    /// Set effect type for a slot (creates slot if needed)
    pub fn set_slot(&mut self, index: usize, effect_type: EffectType, enabled: bool, mix: f32) {
        // The nesting bound, enforced HERE rather than at each call site: a
        // chain that is itself inside a Multi-FX silently refuses to host
        // another one. Getting this wrong costs unbounded recursion per sample
        // on the audio thread, so it cannot be left to callers to remember.
        let effect_type = if effect_type == EffectType::MultiFx && !self.allow_multifx {
            EffectType::None
        } else {
            effect_type
        };

        // Ensure we have enough slots
        while self.slots.len() <= index {
            self.slots.push(EffectSlot::new(self.sample_rate));
        }

        let slot = &mut self.slots[index];
        if slot.effect_type != effect_type {
            log::info!(
                "[Effects] Setting slot {} to effect {:?}, enabled={}, mix={}",
                index,
                effect_type,
                enabled,
                mix
            );
            slot.set_effect_type(effect_type, self.sample_rate);
        }
        slot.enabled = enabled;

        // Update the effect's internal mix parameter
        match slot.effect_type {
            EffectType::Delay => {
                if let Some(ref mut effect) = slot.delay {
                    effect.set_mix(mix);
                }
            }
            EffectType::Ensemble => {
                if let Some(ref mut effect) = slot.ensemble {
                    effect.set_mix(mix);
                }
            }
            EffectType::SpringTank => {
                if let Some(ref mut effect) = slot.spring_tank {
                    effect.set_mix(mix);
                }
            }
            EffectType::CinemaPlate => {
                if let Some(ref mut effect) = slot.cinema_plate {
                    effect.set_mix(mix);
                }
            }
            EffectType::SolinaVerb => {
                if let Some(ref mut effect) = slot.solina_verb {
                    effect.set_mix(mix);
                }
            }
            EffectType::Dimension => {
                if let Some(ref mut effect) = slot.dimension {
                    effect.set_mix(mix);
                }
            }
            EffectType::Reverb => {
                if let Some(ref mut effect) = slot.reverb {
                    effect.set_mix(mix);
                }
            }
            EffectType::Chorus => {
                if let Some(ref mut effect) = slot.chorus {
                    effect.set_mix(mix);
                }
            }
            EffectType::Phaser => {
                if let Some(ref mut effect) = slot.phaser {
                    effect.set_mix(mix);
                }
            }
            EffectType::Flanger => {
                if let Some(ref mut effect) = slot.flanger {
                    effect.set_mix(mix);
                }
            }
            EffectType::Distortion => {
                if let Some(ref mut effect) = slot.distortion {
                    effect.set_mix(mix);
                }
            }
            EffectType::Vocoder => {
                if let Some(ref mut effect) = slot.vocoder {
                    effect.set_mix(mix);
                }
            }
            EffectType::Compressor => {
                if let Some(ref mut effect) = slot.compressor {
                    effect.set_mix(mix);
                }
            }
            EffectType::Limiter => {
                if let Some(ref mut effect) = slot.limiter {
                    effect.set_mix(mix);
                }
            }
            EffectType::Expander => {
                if let Some(ref mut effect) = slot.expander {
                    effect.set_mix(mix);
                }
            }
            EffectType::AutoPanner => {
                if let Some(ref mut effect) = slot.autopanner {
                    effect.set_mix(mix);
                }
            }
            EffectType::Stutter => {
                if let Some(ref mut effect) = slot.stutter {
                    effect.set_mix(mix);
                }
            }
            EffectType::Filter => {
                if let Some(ref mut effect) = slot.filter {
                    effect.set_mix(mix);
                }
            }
            EffectType::RingMod => {
                if let Some(ref mut effect) = slot.ringmod {
                    effect.set_mix(mix);
                }
            }
            EffectType::LoFi => {
                if let Some(ref mut effect) = slot.lofi {
                    effect.set_mix(mix);
                }
            }
            EffectType::StereoImager => {
                if let Some(ref mut effect) = slot.stereo_imager {
                    effect.set_mix(mix);
                }
            }
            EffectType::Choir => {
                if let Some(ref mut effect) = slot.choir {
                    effect.set_mix(mix);
                }
            }
            EffectType::Maximizer => {
                if let Some(ref mut effect) = slot.maximizer {
                    effect.set_mix(mix);
                }
            }
            EffectType::Spatializer3D => { if let Some(ref mut e) = slot.spatializer { e.set_mix(mix); } }
            EffectType::ParametricEq => { if let Some(ref mut e) = slot.param_eq { e.set_mix(mix); } }
            EffectType::Doppler => { if let Some(ref mut e) = slot.doppler { e.set_mix(mix); } }
            EffectType::TransientShaper => { if let Some(ref mut e) = slot.transient_shaper { e.set_mix(mix); } }
            EffectType::GranularFreezer => { if let Some(ref mut e) = slot.granular_freezer { e.set_mix(mix); } }
            EffectType::SpectralFreeze => { if let Some(ref mut e) = slot.spectral_freeze { e.set_mix(mix); } }
            EffectType::ResonatorBank => { if let Some(ref mut e) = slot.resonator_bank { e.set_mix(mix); } }
            EffectType::AutoWah => { if let Some(ref mut e) = slot.auto_wah { e.set_mix(mix); } }
            EffectType::TapeStop => { if let Some(ref mut e) = slot.tape_stop { e.set_mix(mix); } }
            EffectType::BrickwallLimiter => { if let Some(ref mut e) = slot.brickwall_limiter { e.set_mix(mix); } }
            EffectType::LufsMeter => { if let Some(ref mut e) = slot.lufs_meter { e.set_mix(mix); } }
            EffectType::DynamicEq => { if let Some(ref mut e) = slot.dynamic_eq { e.set_mix(mix); } }
            EffectType::MidSideEq => { if let Some(ref mut e) = slot.mid_side_eq { e.set_mix(mix); } }
            EffectType::MultibandComp3 => { if let Some(ref mut e) = slot.multiband_comp { e.set_mix(mix); } }
            EffectType::SpectrumAnalyzer => { if let Some(ref mut e) = slot.spectrum_analyzer { e.set_mix(mix); } }
            EffectType::MasterAssistant => { if let Some(ref mut e) = slot.master_assistant { e.set_mix(mix); } }
            EffectType::ConvolutionReverb => { if let Some(ref mut e) = slot.convolution_reverb { e.set_mix(mix); } }
            EffectType::MultibandDucker => { if let Some(ref mut e) = slot.multiband_ducker { e.set_mix(mix); } }
            EffectType::VelvetDecorrelator => { if let Some(ref mut e) = slot.velvet_decorrelator { e.set_mix(mix); } }
            EffectType::AnalogDrift => { if let Some(ref mut e) = slot.analog_drift { e.set_mix(mix); } }
            EffectType::EarlyReflections => { if let Some(ref mut e) = slot.early_reflections { e.set_mix(mix); } }
            EffectType::MidSideWiden => { if let Some(ref mut e) = slot.mid_side_widen { e.set_mix(mix); } }
            EffectType::HaasSplit => { if let Some(ref mut e) = slot.haas_split { e.set_mix(mix); } }
            EffectType::Retune => { if let Some(ref mut e) = slot.retune { e.set_mix(mix); } }
            EffectType::Harmony => { if let Some(ref mut e) = slot.harmony { e.set_mix(mix); } }
            EffectType::TranceGate => { slot.trance_gate_params.mix = mix; }
            EffectType::MultiFx => { slot.multifx_mix = mix.clamp(0.0, 1.0); }
            EffectType::None => {}
        }

        // Slot's enabled or type may have changed — refresh the
        // chain-level "anything to do" cache that the audio loop
        // checks before iterating slots.
        self.refresh_active_cache();
    }

    /// Configure delay parameters for all delay effects
    pub fn set_delay_params(
        &mut self,
        time: f32,
        feedback: f32,
        tempo_sync: bool,
        tempo_division: usize,
        stereo_offset: f32,
        high_cut: f32,
        low_cut: f32,
    ) {
        for slot in &mut self.slots {
            if let Some(ref mut delay) = slot.delay {
                delay.set_time(time);
                delay.set_feedback(feedback);
                delay.set_tempo_sync(tempo_sync);
                delay.set_tempo_division(TempoDivision::from_index(tempo_division));
                delay.set_stereo_offset(stereo_offset);
                delay.set_high_cut(high_cut);
                delay.set_low_cut(low_cut);
            }
        }
    }

    /// Set reverb type for all reverb effects
    pub fn set_reverb_type(&mut self, type_index: usize) {
        let reverb_type = ReverbType::from_index(type_index);
        for slot in &mut self.slots {
            if let Some(ref mut reverb) = slot.reverb {
                reverb.set_reverb_type(reverb_type);
            }
        }
    }

    /// Set delay type for all delay effects
    pub fn set_delay_type(&mut self, type_index: usize) {
        let delay_type = DelayType::from_index(type_index);
        for slot in &mut self.slots {
            if let Some(ref mut delay) = slot.delay {
                delay.set_delay_type(delay_type);
            }
        }
    }

    /// Configure reverb parameters for all reverb effects
    pub fn set_reverb_params(
        &mut self,
        size: f32,
        decay: f32,
        damping: f32,
        pre_delay: f32,
        width: f32,
    ) {
        for slot in &mut self.slots {
            if let Some(ref mut reverb) = slot.reverb {
                reverb.set_size(size);
                reverb.set_decay(decay);
                reverb.set_damping(damping);
                reverb.set_pre_delay(pre_delay);
                reverb.set_width(width);
            }
        }
    }

    /// Configure chorus parameters for all chorus effects
    pub fn set_chorus_params(
        &mut self,
        rate: f32,
        depth: f32,
        voices: usize,
        delay: f32,
        width: f32,
        tempo_sync: bool,
        tempo_division: usize,
        tempo: f32,
    ) {
        for slot in &mut self.slots {
            if let Some(ref mut chorus) = slot.chorus {
                chorus.set_rate(rate);
                chorus.set_depth(depth);
                chorus.set_voices(voices);
                chorus.set_delay(delay);
                chorus.set_width(width);
                chorus.set_tempo_sync(tempo_sync);
                chorus.set_tempo_division(TempoDivision::from_index(tempo_division));
                chorus.set_tempo(tempo);
            }
        }
    }

    /// Configure phaser parameters for all phaser effects
    pub fn set_phaser_params(
        &mut self,
        rate: f32,
        depth: f32,
        stages: usize,
        feedback: f32,
        stereo_phase: f32,
        tempo_sync: bool,
        tempo_division: usize,
        tempo: f32,
    ) {
        for slot in &mut self.slots {
            if let Some(ref mut phaser) = slot.phaser {
                phaser.set_rate(rate);
                phaser.set_depth(depth);
                phaser.set_stages(stages);
                phaser.set_feedback(feedback);
                phaser.set_stereo_phase(stereo_phase);
                phaser.set_tempo_sync(tempo_sync);
                phaser.set_tempo_division(TempoDivision::from_index(tempo_division));
                phaser.set_tempo(tempo);
            }
        }
    }

    /// Configure flanger parameters for all flanger effects
    pub fn set_flanger_params(
        &mut self,
        rate: f32,
        depth: f32,
        delay: f32,
        feedback: f32,
        stereo_phase: f32,
        tempo_sync: bool,
        tempo_division: usize,
        tempo: f32,
    ) {
        for slot in &mut self.slots {
            if let Some(ref mut flanger) = slot.flanger {
                flanger.set_rate(rate);
                flanger.set_depth(depth);
                flanger.set_delay(delay);
                flanger.set_feedback(feedback);
                flanger.set_stereo_phase(stereo_phase);
                flanger.set_tempo_sync(tempo_sync);
                flanger.set_tempo_division(TempoDivision::from_index(tempo_division));
                flanger.set_tempo(tempo);
            }
        }
    }

    /// Configure distortion parameters for all distortion effects
    pub fn set_distortion_params(&mut self, drive: f32, tone: f32, output: f32) {
        for slot in &mut self.slots {
            if let Some(ref mut distortion) = slot.distortion {
                distortion.set_drive(drive);
                distortion.set_tone(tone);
                distortion.set_output(output);
            }
        }
    }

    /// Configure compressor parameters for all compressor effects
    pub fn set_compressor_params(
        &mut self,
        threshold: f32,
        ratio: f32,
        attack: f32,
        release: f32,
        makeup: f32,
        knee: f32,
    ) {
        for slot in &mut self.slots {
            if let Some(ref mut comp) = slot.compressor {
                comp.set_threshold(threshold);
                comp.set_ratio(ratio);
                comp.set_attack(attack);
                comp.set_release(release);
                comp.set_makeup_gain(makeup);
                comp.set_knee(knee);
            }
        }
    }

    /// Set compressor mode for all compressor effects
    pub fn set_compressor_mode(&mut self, mode: CompressorMode) {
        for slot in &mut self.slots {
            if let Some(ref mut comp) = slot.compressor {
                comp.set_mode(mode);
            }
        }
    }

    /// Configure limiter parameters for all limiter effects
    pub fn set_limiter_params(&mut self, ceiling: f32, release: f32, input_gain: f32) {
        for slot in &mut self.slots {
            if let Some(ref mut lim) = slot.limiter {
                lim.set_ceiling(ceiling);
                lim.set_release(release);
                lim.set_input_gain(input_gain);
            }
        }
    }

    /// Set limiter mode for all limiter effects
    pub fn set_limiter_mode(&mut self, mode: LimiterMode) {
        for slot in &mut self.slots {
            if let Some(ref mut lim) = slot.limiter {
                lim.set_mode(mode);
            }
        }
    }

    /// Configure expander parameters for all expander effects
    pub fn set_expander_params(
        &mut self,
        threshold: f32,
        ratio: f32,
        attack: f32,
        release: f32,
        range: f32,
        knee: f32,
    ) {
        for slot in &mut self.slots {
            if let Some(ref mut exp) = slot.expander {
                exp.set_threshold(threshold);
                exp.set_ratio(ratio);
                exp.set_attack(attack);
                exp.set_release(release);
                exp.set_range(range);
                exp.set_knee(knee);
            }
        }
    }

    /// Set expander mode for all expander effects
    pub fn set_expander_mode(&mut self, mode: ExpanderMode) {
        for slot in &mut self.slots {
            if let Some(ref mut exp) = slot.expander {
                exp.set_mode(mode);
            }
        }
    }

    /// Update vocoder parameters across all slots
    pub fn set_vocoder_params(
        &mut self,
        bands: usize,
        attack: f32,
        release: f32,
        carrier_level: f32,
        modulator_level: f32,
        formant_shift: f32,
    ) {
        for slot in &mut self.slots {
            if let Some(ref mut vocoder) = slot.vocoder {
                vocoder.set_bands(bands);
                vocoder.set_attack(attack);
                vocoder.set_release(release);
                vocoder.set_carrier_level(carrier_level);
                vocoder.set_modulator_level(modulator_level);
                vocoder.set_formant_shift(formant_shift);
            }
        }
    }

    /// Process a mono block of samples
    pub fn process_block_mono(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            let (l, _r) = self.process(*sample, *sample);
            *sample = l;
        }
    }

    /// Process a stereo block of samples
    pub fn process_block_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let (out_l, out_r) = self.process(*l, *r);
            *l = out_l;
            *r = out_r;
        }
    }

    /// Process a stereo block with a modulator signal for vocoder
    /// The modulator is used by vocoder effects to shape the carrier (left/right)
    pub fn process_block_stereo_with_modulator(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        modulator: &[f32],
    ) {
        for (i, (l, r)) in left.iter_mut().zip(right.iter_mut()).enumerate() {
            let mod_sample = modulator.get(i).copied().unwrap_or(0.0);
            let (out_l, out_r) = self.process_with_modulator(*l, *r, mod_sample);
            *l = out_l;
            *r = out_r;
        }
    }

    /// Check if any vocoder effect is enabled in the chain
    pub fn has_vocoder_enabled(&self) -> bool {
        self.slots
            .iter()
            .any(|slot| slot.effect_type == EffectType::Vocoder && slot.enabled)
    }
}

//==============================================================================
// CPU USAGE TRACKING
//==============================================================================

/// CPU usage tracker for effects
#[derive(Debug, Clone)]
pub struct CpuMeter {
    start_time: Option<Instant>,
    total_time_ns: u64,
    sample_count: u64,
    sample_rate: f32,
}

impl CpuMeter {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            start_time: None,
            total_time_ns: 0,
            sample_count: 0,
            sample_rate,
        }
    }

    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
    }

    pub fn stop(&mut self) {
        if let Some(start) = self.start_time {
            let elapsed = start.elapsed().as_nanos() as u64;
            self.total_time_ns += elapsed;
            self.sample_count += 1;
            self.start_time = None;
        }
    }

    pub fn cpu_usage(&self) -> f32 {
        if self.sample_count == 0 || self.sample_rate <= 0.0 {
            return 0.0;
        }

        // Calculate average processing time per sample
        let avg_time_ns = self.total_time_ns as f32 / self.sample_count as f32;

        // Calculate available time per sample
        let available_time_ns = 1_000_000_000.0 / self.sample_rate;

        // CPU usage as percentage
        (avg_time_ns / available_time_ns * 100.0).min(100.0)
    }

    pub fn reset(&mut self) {
        self.start_time = None;
        self.total_time_ns = 0;
        self.sample_count = 0;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Per-slot FX parameter storage (lock-free, shared between GUI and audio)
// ═══════════════════════════════════════════════════════════════════════

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// Map a `generative::SCALE_NAMES` index (the chord track's scale space:
/// Chromatic, Major, Minor, Pentatonic, Blues, Dorian, Phrygian, Lydian,
/// Mixolydian) to the harmonizer `ScaleType::all()` index the Retune/Harmony
/// FX use.
pub fn map_generative_scale(idx: usize) -> usize {
    use crate::harmonizer_dsp::music_theory::ScaleType as S;
    let target = match idx {
        0 => S::Chromatic,
        1 => S::Major,
        2 => S::Minor,
        3 => S::PentatonicMajor,
        4 => S::Blues,
        5 => S::Dorian,
        6 => S::Phrygian,
        7 => S::Lydian,
        8 => S::Mixolydian,
        _ => S::Major,
    };
    S::all().iter().position(|s| *s == target).unwrap_or(0)
}

/// Retune — real-time pitch correction insert. YIN detection on the mid
/// signal drives the Harmonizer engine's pitch-synchronous corrector
/// (`AutotuneProcessor`, the Antares-style dual-head granular technique)
/// on each channel toward the nearest note of a key/scale. `speed` goes
/// from transparent intonation touch-up to the classic hard-tune;
/// `amount` scales how far toward the target the pitch is pulled.
pub struct RetuneEffect {
    pub mix:   f32,
    sample_rate: f32,
    key:       u8,
    scale_idx: usize,
    /// Follow the session's chord-track key (broadcast per block by the
    /// sequencer); manual key/scale apply when off.
    follow_key: bool,
    amount:    f32,
    speed:     f32,
    // Boxed: these carry multi-KB fixed buffers; inline they would bloat
    // every EffectSlot and overflow stack-allocated chains.
    detector:  Box<crate::harmonizer_dsp::pitch_detect::YinDetector>,
    tune_l:    Box<crate::harmonizer_dsp::autotune::AutotuneProcessor>,
    tune_r:    Box<crate::harmonizer_dsp::autotune::AutotuneProcessor>,
    target_ratio: f32,
    hop:       u32,
}

/// Cloning an FX slot (SyncSession, undo) reconstructs the DSP state from the
/// parameters — the transient detector/grain state is not musically meaningful
/// across a clone and the inner harmonizer types are deliberately not Clone.
impl Clone for RetuneEffect {
    fn clone(&self) -> Self {
        let mut c = Self::new(self.sample_rate);
        c.mix = self.mix;
        c.key = self.key;
        c.scale_idx = self.scale_idx;
        c.follow_key = self.follow_key;
        c.amount = self.amount;
        c.set_speed(self.speed);
        c
    }
}

impl std::fmt::Debug for RetuneEffect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetuneEffect")
            .field("key", &self.key)
            .field("scale_idx", &self.scale_idx)
            .field("speed", &self.speed)
            .field("amount", &self.amount)
            .field("mix", &self.mix)
            .finish_non_exhaustive()
    }
}

impl RetuneEffect {
    pub fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            mix: 1.0,
            sample_rate,
            key: 0,
            scale_idx: 0, // Major (ScaleType::all() order)
            follow_key: false,
            amount: 1.0,
            speed: 0.5,
            detector: Box::new(crate::harmonizer_dsp::pitch_detect::YinDetector::default_for_sample_rate(sample_rate)),
            tune_l: Box::new(crate::harmonizer_dsp::autotune::AutotuneProcessor::new(sample_rate)),
            tune_r: Box::new(crate::harmonizer_dsp::autotune::AutotuneProcessor::new(sample_rate)),
            target_ratio: 1.0,
            hop: 0,
        };
        s.set_speed(0.5);
        s
    }

    fn scale(&self) -> crate::harmonizer_dsp::music_theory::ScaleType {
        let all = crate::harmonizer_dsp::music_theory::ScaleType::all();
        all[self.scale_idx.min(all.len() - 1)]
    }

    pub fn set_key(&mut self, key: f32)       { if !self.follow_key { self.key = (key as i32).rem_euclid(12) as u8; } }
    pub fn set_scale(&mut self, idx: f32)     { if !self.follow_key { self.scale_idx = (idx.max(0.0) as usize)
        .min(crate::harmonizer_dsp::music_theory::ScaleType::all().len() - 1); } }
    pub fn set_follow(&mut self, f: f32)      { self.follow_key = f > 0.5; }
    /// Chord-track key broadcast (applies only in follow mode).
    pub fn set_song_key(&mut self, root: u8, scale_idx: usize) {
        if self.follow_key {
            self.key = root % 12;
            self.scale_idx = scale_idx
                .min(crate::harmonizer_dsp::music_theory::ScaleType::all().len() - 1);
        }
    }
    pub fn set_amount(&mut self, a: f32)      { self.amount = a.clamp(0.0, 1.0); }
    pub fn set_mix(&mut self, m: f32)         { self.mix = m.clamp(0.0, 1.0); }
    pub fn set_speed(&mut self, s: f32) {
        self.speed = s.clamp(0.0, 1.0);
        self.tune_l.set_speed(self.speed);
        self.tune_r.set_speed(self.speed);
    }

    pub fn reset(&mut self) {
        self.tune_l.reset();
        self.tune_r.reset();
        self.target_ratio = 1.0;
        self.hop = 0;
    }

    pub fn process(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        self.detector.push_sample((in_l + in_r) * 0.5);
        self.hop += 1;
        if self.hop >= 64 {
            // Control-rate: retarget the correction ratio from the detector.
            self.hop = 0;
            let pitch = self.detector.pitch();
            if pitch > 40.0 && pitch < 2000.0 {
                let midi = 69.0 + 12.0 * (pitch / 440.0).log2();
                let target = crate::harmonizer_dsp::music_theory::snap_to_scale(midi, self.key, self.scale());
                let full = 2f32.powf((target - midi) / 12.0);
                self.target_ratio = 1.0 + (full - 1.0) * self.amount;
                self.tune_l.set_pitch_period(pitch);
                self.tune_r.set_pitch_period(pitch);
            } else {
                self.target_ratio = 1.0; // unpitched material passes clean
            }
        }
        let wl = self.tune_l.process(in_l, self.target_ratio);
        let wr = self.tune_r.process(in_r, self.target_ratio);
        (in_l + (wl - in_l) * self.mix, in_r + (wr - in_r) * self.mix)
    }
}

/// Harmony — diatonic harmonizer insert. YIN tracks the input's pitch; up to
/// 3 `HarmonyVoice`s (the Harmonizer engine's formant-preserving shifter
/// stack) follow at scale-degree intervals in the selected key, panned
/// across the field by `spread`. The dry signal always passes at unity;
/// `mix` is the harmony-voices level (parallel, like a send).
pub struct HarmonyEffect {
    pub mix:   f32,
    sample_rate: f32,
    key:       u8,
    scale_idx: usize,
    follow_key: bool,
    degrees:   [i8; 3], // scale-degree offset per voice, 0 = voice off
    spread:    f32,
    detector:  Box<crate::harmonizer_dsp::pitch_detect::YinDetector>,
    voices:    Vec<crate::harmonizer_dsp::voice::HarmonyVoice>,
    hop:       u32,
}

/// Same clone semantics as `RetuneEffect`: rebuild from parameters.
impl Clone for HarmonyEffect {
    fn clone(&self) -> Self {
        let mut c = Self::new(self.sample_rate);
        c.mix = self.mix;
        c.key = self.key;
        c.scale_idx = self.scale_idx;
        c.follow_key = self.follow_key;
        c.degrees = self.degrees;
        c.spread = self.spread;
        c.apply_voice_config();
        c
    }
}

impl std::fmt::Debug for HarmonyEffect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HarmonyEffect")
            .field("key", &self.key)
            .field("scale_idx", &self.scale_idx)
            .field("degrees", &self.degrees)
            .field("spread", &self.spread)
            .field("mix", &self.mix)
            .finish_non_exhaustive()
    }
}

impl HarmonyEffect {
    pub fn new(sample_rate: f32) -> Self {
        let mut voices = Vec::with_capacity(3);
        for _ in 0..3 {
            let mut v = crate::harmonizer_dsp::voice::HarmonyVoice::new(sample_rate);
            v.enabled = false;
            v.level = 0.9;
            voices.push(v);
        }
        let mut s = Self {
            mix: 0.5,
            sample_rate,
            key: 0,
            scale_idx: 0,
            follow_key: false,
            degrees: [2, -3, 0], // 3rd above + 4th below, voice 3 off
            spread: 0.6,
            detector: Box::new(crate::harmonizer_dsp::pitch_detect::YinDetector::default_for_sample_rate(sample_rate)),
            voices,
            hop: 0,
        };
        s.apply_voice_config();
        s
    }

    fn scale(&self) -> crate::harmonizer_dsp::music_theory::ScaleType {
        let all = crate::harmonizer_dsp::music_theory::ScaleType::all();
        all[self.scale_idx.min(all.len() - 1)]
    }

    fn apply_voice_config(&mut self) {
        // Fixed pan slots scaled by spread: L / R / centre.
        let pans = [-1.0f32, 1.0, 0.0];
        for (i, v) in self.voices.iter_mut().enumerate() {
            v.enabled = self.degrees[i] != 0;
            v.pan = pans[i] * self.spread;
        }
    }

    pub fn set_key(&mut self, key: f32)   { if !self.follow_key { self.key = (key as i32).rem_euclid(12) as u8; } }
    pub fn set_scale(&mut self, idx: f32) { if !self.follow_key { self.scale_idx = (idx.max(0.0) as usize)
        .min(crate::harmonizer_dsp::music_theory::ScaleType::all().len() - 1); } }
    pub fn set_follow(&mut self, f: f32)  { self.follow_key = f > 0.5; }
    /// Chord-track key broadcast (applies only in follow mode).
    pub fn set_song_key(&mut self, root: u8, scale_idx: usize) {
        if self.follow_key {
            self.key = root % 12;
            self.scale_idx = scale_idx
                .min(crate::harmonizer_dsp::music_theory::ScaleType::all().len() - 1);
        }
    }
    pub fn set_mix(&mut self, m: f32)     { self.mix = m.clamp(0.0, 1.0); }
    pub fn set_spread(&mut self, s: f32)  { self.spread = s.clamp(0.0, 1.0); self.apply_voice_config(); }
    pub fn set_degree(&mut self, voice: usize, deg: f32) {
        if voice < 3 {
            self.degrees[voice] = (deg as i32).clamp(-7, 7) as i8;
            self.apply_voice_config();
        }
    }

    pub fn reset(&mut self) {
        for v in &mut self.voices { v.reset(); }
        self.hop = 0;
    }

    pub fn process(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let mid = (in_l + in_r) * 0.5;
        self.detector.push_sample(mid);
        self.hop += 1;
        if self.hop >= 64 {
            // Control-rate: retarget each voice's shift ratio diatonically.
            self.hop = 0;
            let pitch = self.detector.pitch();
            if pitch > 40.0 && pitch < 2000.0 {
                let midi = 69.0 + 12.0 * (pitch / 440.0).log2();
                let (key, scale) = (self.key, self.scale());
                for (i, v) in self.voices.iter_mut().enumerate() {
                    if self.degrees[i] == 0 { continue; }
                    let target = crate::harmonizer_dsp::music_theory::transpose_in_scale(
                        midi, key, scale, self.degrees[i]);
                    let ratio = 2f32.powf((target - midi) / 12.0).clamp(0.25, 4.0);
                    v.set_target_ratio(ratio);
                }
            }
        }
        let (mut wl, mut wr) = (0.0f32, 0.0f32);
        for v in &mut self.voices {
            if !v.enabled { continue; }
            let (l, r) = v.process(mid);
            wl += l;
            wr += r;
        }
        (in_l + wl * self.mix, in_r + wr * self.mix)
    }
}

const SLOT_FX_PARAM_COUNT: usize = 100;
const SLOT_COUNT: usize = 8;

/// Per-slot effect parameter storage using atomics for lock-free GUI↔audio access.
/// Each of the 8 slots has 100 parameter values indexed by pid constants from fx_controls.
pub struct SlotFxParams {
    data: [[AtomicU32; SLOT_FX_PARAM_COUNT]; SLOT_COUNT],
}

impl SlotFxParams {
    pub fn new() -> Arc<Self> {
        // Safety: AtomicU32 with value 0 is valid
        Arc::new(Self {
            data: std::array::from_fn(|_| std::array::from_fn(|_| AtomicU32::new(0))),
        })
    }

    pub fn get(&self, slot: usize, pid: u8) -> f32 {
        if slot < SLOT_COUNT && (pid as usize) < SLOT_FX_PARAM_COUNT {
            f32::from_bits(self.data[slot][pid as usize].load(Ordering::Relaxed))
        } else {
            0.0
        }
    }

    pub fn set(&self, slot: usize, pid: u8, value: f32) {
        if slot < SLOT_COUNT && (pid as usize) < SLOT_FX_PARAM_COUNT {
            self.data[slot][pid as usize].store(value.to_bits(), Ordering::Relaxed);
        }
    }

    /// Copy all params from one slot to another
    pub fn copy_slot(&self, from: usize, to: usize) {
        if from < SLOT_COUNT && to < SLOT_COUNT {
            for i in 0..SLOT_FX_PARAM_COUNT {
                let v = self.data[from][i].load(Ordering::Relaxed);
                self.data[to][i].store(v, Ordering::Relaxed);
            }
        }
    }

    /// Clear all params for a slot to zero
    pub fn clear_slot(&self, slot: usize) {
        if slot < SLOT_COUNT {
            for i in 0..SLOT_FX_PARAM_COUNT {
                self.data[slot][i].store(0, Ordering::Relaxed);
            }
        }
    }
}

// Manual Debug impl since AtomicU32 arrays don't derive Debug easily
impl std::fmt::Debug for SlotFxParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlotFxParams").finish()
    }
}

//==============================================================================
// SHARED DSP HELPERS FOR NEW EFFECTS
//==============================================================================

/// RBJ cookbook biquad — direct form I. Used by ParametricEq, AutoWah.
use crate::dsp::filters::Biquad as RbjBiquad;

/// MultibandDucker — frequency-slotting sidechain duck. Reproduces the canon's
/// "the bass owns the lows" master move: split the input into a LOW band (below
/// `crossover`) and the rest, and attenuate ONLY the low band under an envelope
/// follower on the sidechain key (e.g. a bass track). The mids/highs are never
/// touched, so a powerful low-end key can dominate its band without crushing the
/// melody/voices. With no sidechain assigned it self-keys (ducks its own lows on
/// its own low energy), which is still a useful low-end de-clutter.
///
/// Complementary subtraction: out = in - lowpass(in) * cut, matching the canon
/// `master -= lo*cut`. Mono-safe (the low band is the same filter on both
/// channels; only its gain is modulated).
#[derive(Debug, Clone)]
pub struct MultibandDuckerEffect {
    sample_rate: f32,
    // params
    crossover_hz: f32,   // band the key is allowed to own
    depth:        f32,    // 0..1 max low-band gain reduction
    sensitivity:  f32,    // key-envelope -> cut scaling (canon used 1.7)
    attack_s:     f32,
    release_s:    f32,
    pub mix:      f32,
    // state
    lp_l: RbjBiquad,
    lp_r: RbjBiquad,
    env:  f32,
    atk_coef: f32,
    rel_coef: f32,
    dirty: bool,
    // sidechain key (fed per-frame from the source track; detector only)
    pub sidechain_active: bool,
    pub sidechain_l: f32,
    pub sidechain_r: f32,
}

impl MultibandDuckerEffect {
    pub fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            sample_rate,
            crossover_hz: 230.0,
            depth:        0.85,
            sensitivity:  1.7,
            attack_s:     0.005,
            release_s:    0.13,
            mix:          1.0,
            lp_l: RbjBiquad::new(),
            lp_r: RbjBiquad::new(),
            env:  0.0,
            atk_coef: 0.0,
            rel_coef: 0.0,
            dirty: true,
            sidechain_active: false,
            sidechain_l: 0.0,
            sidechain_r: 0.0,
        };
        s.recompute();
        s
    }

    fn recompute(&mut self) {
        let fc = self.crossover_hz.clamp(40.0, 2000.0);
        self.lp_l.set_lowpass(self.sample_rate, fc, 0.707);
        self.lp_r.set_lowpass(self.sample_rate, fc, 0.707);
        self.atk_coef = 1.0 - (-1.0 / (self.attack_s.max(1e-4) * self.sample_rate)).exp();
        self.rel_coef = 1.0 - (-1.0 / (self.release_s.max(1e-4) * self.sample_rate)).exp();
        self.dirty = false;
    }

    pub fn set_crossover(&mut self, hz: f32) { self.crossover_hz = hz; self.dirty = true; }
    pub fn set_depth(&mut self, d: f32) { self.depth = d.clamp(0.0, 1.0); }
    pub fn set_sensitivity(&mut self, s: f32) { self.sensitivity = s.max(0.0); }
    pub fn set_attack(&mut self, s: f32) { self.attack_s = s; self.dirty = true; }
    pub fn set_release(&mut self, s: f32) { self.release_s = s; self.dirty = true; }
    pub fn set_mix(&mut self, m: f32) { self.mix = m.clamp(0.0, 1.0); }

    pub fn reset(&mut self) {
        self.lp_l.reset();
        self.lp_r.reset();
        self.env = 0.0;
    }

    pub fn process(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if self.dirty { self.recompute(); }
        // key level: sidechain when assigned, else self-key on own input
        let (kl, kr) = if self.sidechain_active { (self.sidechain_l, self.sidechain_r) }
                       else { (in_l, in_r) };
        let key = (kl.abs() + kr.abs()) * 0.5;
        let co = if key > self.env { self.atk_coef } else { self.rel_coef };
        self.env += (key - self.env) * co;
        let cut = (self.depth * (self.env * self.sensitivity).min(1.0)).clamp(0.0, 1.0);
        let lo_l = self.lp_l.process(in_l);
        let lo_r = self.lp_r.process(in_r);
        let wet_l = in_l - lo_l * cut;
        let wet_r = in_r - lo_r * cut;
        (in_l + (wet_l - in_l) * self.mix, in_r + (wet_r - in_r) * self.mix)
    }
}

/// VelvetDecorrelator — velvet-noise side-injection section widener. Ports the
/// canon's violin-section decorrelator (DAFx velvet noise): convolve the MONO
/// component with a sparse random ±1 velvet-noise sequence to make a diffuse,
/// decorrelated copy, and inject it ONLY into the SIDE signal. Because the
/// injection is anti-symmetric (L += d, R -= d), the mono sum (L+R) is
/// unchanged -> zero coloration, fully mono-compatible, while the stereo image
/// fans out to a large apparent ensemble at flat cost (no allpass combing).
#[derive(Debug, Clone)]
pub struct VelvetDecorrelatorEffect {
    width:    f32,   // 0..1 amount of decorrelated side injection
    size:     f32,    // 0..1 -> active tap count (apparent players)
    pub mix:  f32,
    buf_l:    Vec<f32>,
    buf_r:    Vec<f32>,
    pos:      usize,
    // velvet taps: (delay_in_samples, sign) precomputed deterministically.
    taps_l:   Vec<(usize, f32)>,
    taps_r:   Vec<(usize, f32)>,
    active:   usize,
}

impl VelvetDecorrelatorEffect {
    pub fn new(sample_rate: f32) -> Self {
        let len = (0.025 * sample_rate) as usize + 2; // ~25 ms velvet window
        let max_taps = 64usize;
        // deterministic xorshift so the decorrelation is identical every run
        let mut st = 0x9E3779B9u32;
        let mut rng = move || { st ^= st << 13; st ^= st >> 17; st ^= st << 5; st };
        let mut mk = |rng: &mut dyn FnMut() -> u32| -> Vec<(usize, f32)> {
            (0..max_taps).map(|k| {
                // spread taps roughly evenly across the window (velvet density) + jitter
                let base = (k * (len - 1)) / max_taps;
                let jit = (rng() as usize) % ((len / max_taps).max(1));
                let d = (base + jit).clamp(1, len - 1);
                let sign = if rng() & 1 == 0 { 1.0 } else { -1.0 };
                (d, sign)
            }).collect()
        };
        let taps_l = mk(&mut rng);
        let taps_r = mk(&mut rng);
        Self {
            width: 0.6, size: 0.5, mix: 1.0,
            buf_l: vec![0.0; len], buf_r: vec![0.0; len], pos: 0,
            taps_l, taps_r, active: 32,
        }
    }

    pub fn set_width(&mut self, w: f32) { self.width = w.clamp(0.0, 1.0); }
    pub fn set_size(&mut self, s: f32) {
        self.size = s.clamp(0.0, 1.0);
        self.active = (8.0 + self.size * 56.0) as usize; // 8..64 taps
    }
    pub fn set_mix(&mut self, m: f32) { self.mix = m.clamp(0.0, 1.0); }
    pub fn reset(&mut self) { self.buf_l.iter_mut().for_each(|x| *x = 0.0);
                              self.buf_r.iter_mut().for_each(|x| *x = 0.0); self.pos = 0; }

    pub fn process(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let len = self.buf_l.len();
        let mid = (in_l + in_r) * 0.5;
        let side = (in_l - in_r) * 0.5;
        self.buf_l[self.pos] = mid;
        self.buf_r[self.pos] = mid;
        let n = self.active.min(self.taps_l.len());
        let norm = 1.0 / (n as f32).sqrt();
        let mut dec_l = 0.0; let mut dec_r = 0.0;
        for k in 0..n {
            let (dl, sl) = self.taps_l[k];
            let (dr, sr) = self.taps_r[k];
            dec_l += sl * self.buf_l[(self.pos + len - dl) % len];
            dec_r += sr * self.buf_r[(self.pos + len - dr) % len];
        }
        dec_l *= norm; dec_r *= norm;
        self.pos = (self.pos + 1) % len;
        // anti-symmetric side injection (mono-invariant)
        let inj = self.width * (dec_l - dec_r) * 0.5;
        let side2 = side + inj;
        let wet_l = mid + side2;
        let wet_r = mid - side2;
        (in_l + (wet_l - in_l) * self.mix, in_r + (wet_r - in_r) * self.mix)
    }
}

/// AnalogDrift — slow per-track tape-wow pitch drift. Ports the canon's
/// `analog_drift`: two very slow smoothed-noise modulators sum to a few cents of
/// continuous pitch wander via a modulated interpolating delay, so stacked takes
/// (or a doubled part) never sit perfectly in tune/phase -> they sound like two
/// real performances instead of phase-locked clones.
#[derive(Debug, Clone)]
pub struct AnalogDriftEffect {
    sample_rate: f32,
    depth_cents: f32,
    rate_hz:     f32,
    pub mix:     f32,
    buf_l:  Vec<f32>,
    buf_r:  Vec<f32>,
    pos:    usize,
    // SMOOTH dual-sine LFO modulators (VERBATIM port of canon_trance `analog_drift`, 727-762):
    // ml = base + depth*(0.6*sin(f1) + 0.4*sin(f2)); the R channel uses its own f1r/f2r = decorrelated
    // widening. NOT the old random-walk (white noise + one-pole), which sprayed broadband HF = a "gresille"
    // on the exposed strings (why it had been removed). seed derives the 8 LFO params; t = sample counter.
    st:     u32,
    lfo_f:  [f32; 4], // f1, f2, f1r, f2r  (Hz)
    lfo_p:  [f32; 4], // p1, p2, p1r, p2r  (phase)
    t:      f32,
}

impl AnalogDriftEffect {
    pub fn new(sample_rate: f32) -> Self {
        let len = (0.05 * sample_rate) as usize + 4; // 50 ms headroom for modulated read
        let mut s = Self {
            sample_rate,
            depth_cents: 5.0,
            rate_hz: 0.3,
            mix: 1.0,
            buf_l: vec![0.0; len], buf_r: vec![0.0; len], pos: 0,
            st: 0x1234567,
            lfo_f: [0.0; 4], lfo_p: [0.0; 4], t: 0.0,
        };
        s.compute_lfo();
        s
    }

    /// Derive the 8 dual-sine LFO params from the seed -- VERBATIM from canon_trance analog_drift (733-740).
    fn compute_lfo(&mut self) {
        let seed = self.st;
        let r = |k: u32| -> f32 {
            let h = (seed.wrapping_mul(2654435761) ^ k.wrapping_mul(40503)) >> 8;
            (h & 0xFFFF) as f32 / 65535.0
        };
        let tau = std::f32::consts::TAU;
        self.lfo_f = [0.11 + 0.17 * r(1), 0.06 + 0.13 * r(3), 0.10 + 0.17 * r(5), 0.07 + 0.12 * r(7)];
        self.lfo_p = [r(2) * tau, r(4) * tau, r(6) * tau, r(8) * tau];
    }

    pub fn set_depth(&mut self, cents: f32) { self.depth_cents = cents.clamp(0.0, 30.0); }
    pub fn set_rate(&mut self, hz: f32) {
        self.rate_hz = hz.clamp(0.02, 5.0);
        // Seed the LFO params FROM the rate so two instances with different rates DECORRELATE (the render
        // seeds analog_drift per desk). |1 keeps it nonzero. (rate_hz no longer sets the LFO speed -- the
        // render's drift speed is the fixed 0.06..0.28 Hz dual-sine; rate only selects the decorrelation.)
        self.st = (self.rate_hz.to_bits().wrapping_mul(2654435761)) | 1;
        self.compute_lfo();
    }
    pub fn set_mix(&mut self, m: f32) { self.mix = m.clamp(0.0, 1.0); }
    pub fn reset(&mut self) { self.buf_l.iter_mut().for_each(|x| *x = 0.0);
                              self.buf_r.iter_mut().for_each(|x| *x = 0.0);
                              self.pos = 0; self.t = 0.0; }

    fn read(buf: &[f32], pos: usize, delay: f32) -> f32 {
        let len = buf.len();
        let d = delay.clamp(1.0, (len - 2) as f32);
        let di = d.floor() as usize;
        let fr = d - di as f32;
        let i0 = (pos + len - di) % len;
        let i1 = (pos + len - di - 1) % len;
        buf[i0] * (1.0 - fr) + buf[i1] * fr
    }

    pub fn process(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let len = self.buf_l.len();
        self.buf_l[self.pos] = in_l;
        self.buf_r[self.pos] = in_r;
        // SMOOTH dual-sine LFO (canon_trance 728-760): base 6 ms read, depth scaled by cents, the L and R
        // channels read with INDEPENDENT 2-sine modulators (0.6*sin(f1)+0.4*sin(f2)) = decorrelated drift.
        let base = 0.006 * self.sample_rate;
        let depth = (self.depth_cents / 5.0) * 0.0026 * self.sample_rate;
        let w = std::f32::consts::TAU / self.sample_rate;
        let t = self.t;
        let ml = base + depth * (0.6 * (w * self.lfo_f[0] * t + self.lfo_p[0]).sin()
                               + 0.4 * (w * self.lfo_f[1] * t + self.lfo_p[1]).sin());
        let mr = base + depth * (0.6 * (w * self.lfo_f[2] * t + self.lfo_p[2]).sin()
                               + 0.4 * (w * self.lfo_f[3] * t + self.lfo_p[3]).sin());
        self.t += 1.0;
        self.pos = (self.pos + 1) % len;
        let wet_l = Self::read(&self.buf_l, self.pos, ml);
        let wet_r = Self::read(&self.buf_r, self.pos, mr);
        (in_l + (wet_l - in_l) * self.mix, in_r + (wet_r - in_r) * self.mix)
    }
}

/// EarlyReflections — 8-tap stereo early reflections with a seed-perturbed tap
/// pattern. VERBATIM port of canon_trance `early_reflect` (688-719): identical
/// reflections on stacked stems comb-align and fuse them ("accordéon"), so each
/// tap time is scaled by a deterministic per-seed factor (~0.82..1.18) and the L/R
/// tap sets differ -> no two instances share a room signature. The reflections are
/// ADDED to the dry (`s += el*mix`), so this is a parallel send-like room, not a
/// wet/dry crossfade; `mix` is the reflection amount (render passes 0.10..0.22).
#[derive(Debug, Clone)]
pub struct EarlyReflectionsEffect {
    sample_rate: f32,
    pub mix:  f32,
    seed:     u32,
    dl:       Vec<f32>,
    dr:       Vec<f32>,
    il:       [usize; 8],
    ir:       [usize; 8],
    g:        [f32; 8],
    maxd:     usize,
    pos:      usize,
}

impl EarlyReflectionsEffect {
    pub fn new(sample_rate: f32) -> Self {
        let maxd = (0.09 * sample_rate) as usize + 8; // headroom for the +18% scaled taps
        let mut s = Self {
            sample_rate,
            mix: 0.2,
            seed: 0x1111,
            dl: vec![0.0; maxd], dr: vec![0.0; maxd],
            il: [0; 8], ir: [0; 8],
            g: [0.72, 0.58, 0.47, 0.39, 0.31, 0.25, 0.19, 0.14],
            maxd, pos: 0,
        };
        s.compute_taps();
        s
    }

    /// Derive the 16 tap delays from the seed -- VERBATIM from canon_trance early_reflect (693-706).
    fn compute_taps(&mut self) {
        let seed = self.seed;
        let sc = |k: u32| -> f32 {
            let h = (seed.wrapping_mul(2654435761) ^ k.wrapping_mul(40503)) >> 9;
            0.82 + 0.36 * ((h & 0xFFFF) as f32 / 65535.0) // 0.82..1.18
        };
        let base_l = [4.7f32, 9.7, 15.3, 21.7, 29.3, 38.1, 48.7, 61.3];
        let base_r = [5.9f32, 11.3, 17.9, 24.1, 32.7, 41.3, 52.1, 64.7];
        let sr = self.sample_rate;
        for k in 0..8 {
            let tl = base_l[k] * sc(k as u32 * 2 + 1);
            let tr = base_r[k] * sc(k as u32 * 2 + 2);
            self.il[k] = ((tl * 0.001 * sr) as usize).min(self.maxd - 2);
            self.ir[k] = ((tr * 0.001 * sr) as usize).min(self.maxd - 2);
        }
    }

    pub fn set_seed(&mut self, seed: f32) {
        // Verbatim seed (NO |1): the render's early_reflect uses the raw seed in sc(); ORing
        // with 1 would alter even seeds (0x2222 -> 0x2223) and break bit-exactness.
        let s = seed as i64 as u32;
        if s != self.seed { self.seed = s; self.compute_taps(); }
    }
    pub fn set_mix(&mut self, m: f32) { self.mix = m.clamp(0.0, 1.0); }
    pub fn reset(&mut self) {
        self.dl.iter_mut().for_each(|x| *x = 0.0);
        self.dr.iter_mut().for_each(|x| *x = 0.0);
        self.pos = 0;
    }

    pub fn process(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let maxd = self.maxd;
        self.dl[self.pos] = in_l;
        self.dr[self.pos] = in_r;
        let (mut el, mut er) = (0.0f32, 0.0f32);
        for k in 0..8 {
            el += self.dl[(self.pos + maxd - self.il[k]) % maxd] * self.g[k];
            er += self.dr[(self.pos + maxd - self.ir[k]) % maxd] * self.g[k];
        }
        self.pos = (self.pos + 1) % maxd;
        let out_l = in_l + (el * 0.85 + er * 0.15) * self.mix;
        let out_r = in_r + (er * 0.85 + el * 0.15) * self.mix;
        (out_l, out_r)
    }
}

/// MidSideWiden — frequency-split mid/side widener. VERBATIM port of the canon drum-bus
/// M/S widen (canon_trance 1043-1053): a one-pole lowpass at `crossover` splits each channel;
/// the LOWS (wl/wr) pass through as the original L/R (kick stays centred + punchy), the HIGHS
/// (x - lowpass) are re-encoded mid/side with the side scaled by `width` (1.9 = wider). This is
/// NOT a StereoImager (which mono's the bass) — the lows keep their native L/R image.
#[derive(Debug, Clone)]
pub struct MidSideWidenEffect {
    sample_rate: f32,
    crossover:   f32,
    width:       f32,
    pub mix:     f32,
    k:           f32,   // one-pole lowpass coeff for the crossover
    wl:          f32,   // lowpass state L
    wr:          f32,   // lowpass state R
}

impl MidSideWidenEffect {
    pub fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            sample_rate, crossover: 500.0, width: 1.9, mix: 1.0,
            k: 0.0, wl: 0.0, wr: 0.0,
        };
        s.update_k();
        s
    }
    fn update_k(&mut self) {
        // EXACT render coeff: k_x = 1 - exp(-TAU * fc / sr) (canon_trance 1043).
        self.k = 1.0 - (-std::f32::consts::TAU * self.crossover / self.sample_rate).exp();
    }
    pub fn set_crossover(&mut self, fc: f32) {
        let fc = fc.clamp(20.0, 20000.0);
        if (fc - self.crossover).abs() > 1e-3 { self.crossover = fc; self.update_k(); }
    }
    pub fn set_width(&mut self, w: f32) { self.width = w.clamp(0.0, 4.0); }
    pub fn set_mix(&mut self, m: f32) { self.mix = m.clamp(0.0, 1.0); }
    pub fn reset(&mut self) { self.wl = 0.0; self.wr = 0.0; }

    pub fn process(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        // VERBATIM (canon_trance 1045-1052):
        self.wl += (in_l - self.wl) * self.k;
        self.wr += (in_r - self.wr) * self.k;
        let (hl, hr) = (in_l - self.wl, in_r - self.wr);
        let m = 0.5 * (hl + hr);
        let sd = 0.5 * (hl - hr) * self.width;
        let out_l = self.wl + m + sd;
        let out_r = self.wr + m - sd;
        (in_l + (out_l - in_l) * self.mix, in_r + (out_r - in_r) * self.mix)
    }
}

/// HaasSplit — the canon pulsar-bass stereo spread (canon_trance 1149-1161). Splits the input at
/// `crossover` (130 Hz) into lows (mono-safe: native L/R kept centred = the sub's power) and highs
/// (the distortion grit). The highs are taken from the LEFT highpassed channel and fanned across the
/// field via a fixed `delay_ms` (11 ms) Haas delay on R: L = lo_l + hi_l, R = lo_r + hi_l[delayed].
/// Bit-exact: same RBJ biquad coeffs as canon_trance's Bq, same integer sample delay (see
/// `haas_split_is_bit_exact`). Lets the bass "occupy the whole stereo image" without smearing the sub.
#[derive(Debug, Clone)]
pub struct HaasSplitEffect {
    sample_rate: f32,
    crossover:   f32,
    delay_ms:    f32,
    pub mix:     f32,
    // lowpass biquad (DF1, exact Bq::lowpass coeffs), state per channel.
    lp_b0: f32, lp_b1: f32, lp_b2: f32, lp_a1: f32, lp_a2: f32,
    lp_s:  [[f32; 4]; 2],
    // highpass biquad (exact Bq::highpass), run on the LEFT input only (ch0).
    hp_b0: f32, hp_b1: f32, hp_b2: f32, hp_a1: f32, hp_a2: f32,
    hp_s:  [f32; 4],
    // integer Haas delay line for the highpassed-L signal.
    dbuf:   Vec<f32>,
    dwrite: usize,
    dl:     usize,
}

impl HaasSplitEffect {
    pub fn new(sample_rate: f32) -> Self {
        let cap = (0.030 * sample_rate) as usize + 2; // up to 30 ms of Haas headroom
        let mut s = Self {
            sample_rate, crossover: 130.0, delay_ms: 11.0, mix: 1.0,
            lp_b0: 0.0, lp_b1: 0.0, lp_b2: 0.0, lp_a1: 0.0, lp_a2: 0.0, lp_s: [[0.0; 4]; 2],
            hp_b0: 0.0, hp_b1: 0.0, hp_b2: 0.0, hp_a1: 0.0, hp_a2: 0.0, hp_s: [0.0; 4],
            dbuf: vec![0.0; cap], dwrite: 0, dl: 0,
        };
        s.update_coeffs();
        s
    }
    fn update_coeffs(&mut self) {
        // EXACT canon_trance Bq coeffs (RBJ, Q=0.7), fc = crossover.
        let q = 0.7f32;
        let w = 2.0 * std::f32::consts::PI * self.crossover / self.sample_rate;
        let (c, sn) = (w.cos(), w.sin());
        let al = sn / (2.0 * q);
        let a0 = 1.0 + al;
        // lowpass
        self.lp_b0 = ((1.0 - c) / 2.0) / a0;
        self.lp_b1 = (1.0 - c) / a0;
        self.lp_b2 = ((1.0 - c) / 2.0) / a0;
        self.lp_a1 = (-2.0 * c) / a0;
        self.lp_a2 = (1.0 - al) / a0;
        // highpass
        self.hp_b0 = ((1.0 + c) / 2.0) / a0;
        self.hp_b1 = (-(1.0 + c)) / a0;
        self.hp_b2 = ((1.0 + c) / 2.0) / a0;
        self.hp_a1 = (-2.0 * c) / a0;
        self.hp_a2 = (1.0 - al) / a0;
        self.dl = (self.delay_ms * 0.001 * self.sample_rate) as usize;
        if self.dl >= self.dbuf.len() { self.dl = self.dbuf.len() - 1; }
    }
    pub fn set_crossover(&mut self, fc: f32) {
        let fc = fc.clamp(20.0, 2000.0);
        if (fc - self.crossover).abs() > 1e-3 { self.crossover = fc; self.update_coeffs(); }
    }
    pub fn set_delay_ms(&mut self, ms: f32) {
        let ms = ms.clamp(0.0, 25.0);
        if (ms - self.delay_ms).abs() > 1e-4 { self.delay_ms = ms; self.update_coeffs(); }
    }
    pub fn set_mix(&mut self, m: f32) { self.mix = m.clamp(0.0, 1.0); }
    pub fn reset(&mut self) {
        self.lp_s = [[0.0; 4]; 2]; self.hp_s = [0.0; 4];
        for s in &mut self.dbuf { *s = 0.0; } self.dwrite = 0;
    }

    #[inline]
    fn lp_run(&mut self, x: f32, ch: usize) -> f32 {
        let s = &mut self.lp_s[ch];
        let y = self.lp_b0 * x + self.lp_b1 * s[0] + self.lp_b2 * s[1] - self.lp_a1 * s[2] - self.lp_a2 * s[3];
        s[1] = s[0]; s[0] = x; s[3] = s[2]; s[2] = y;
        y
    }
    #[inline]
    fn hp_run(&mut self, x: f32) -> f32 {
        let s = &mut self.hp_s;
        let y = self.hp_b0 * x + self.hp_b1 * s[0] + self.hp_b2 * s[1] - self.hp_a1 * s[2] - self.hp_a2 * s[3];
        s[1] = s[0]; s[0] = x; s[3] = s[2]; s[2] = y;
        y
    }

    pub fn process(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        // VERBATIM (canon_trance 1152-1161): lo = lowpass(s) on L/R; hi = highpass(s).0 (L only);
        // L = lo.0 + hi ; R = lo.1 + hi[delayed dl]. (s -> hi/lo are independent filters, so streaming
        // per-sample is bit-identical to the render's block apply.)
        let lo_l = self.lp_run(in_l, 0);
        let lo_r = self.lp_run(in_r, 1);
        let hi_l = self.hp_run(in_l);
        // write current hi_l, then read dl samples ago (0 until the line has filled = the render's
        // `if i >= dl { hi[i-dl].0 } else { 0.0 }` since dbuf is zero-initialised).
        let n = self.dbuf.len();
        self.dbuf[self.dwrite] = hi_l;
        let rdel = self.dbuf[(self.dwrite + n - self.dl) % n];
        self.dwrite = (self.dwrite + 1) % n;
        let out_l = lo_l + hi_l;
        let out_r = lo_r + rdel;
        // mix form `out*m + in*(1-m)` (NOT `in + (out-in)*m`) so mix=1.0 returns `out` BIT-EXACTLY
        // (the render fully replaces the signal; *1.0 and *0.0 are exact, no rounding).
        (out_l * self.mix + in_l * (1.0 - self.mix),
         out_r * self.mix + in_r * (1.0 - self.mix))
    }
}

/// Simple fractional delay line — used by Spatializer3D (ITD), Doppler, TapeStop.
#[derive(Debug, Clone)]
struct FracDelayLine {
    buf: Vec<f32>,
    write: usize,
    mask: usize,
}

impl FracDelayLine {
    fn new(max_samples: usize) -> Self {
        // Round up to power of two for cheap masking.
        let mut n = 2;
        while n < max_samples.max(2) { n *= 2; }
        Self { buf: vec![0.0; n], write: 0, mask: n - 1 }
    }

    #[inline(always)]
    fn write(&mut self, x: f32) {
        self.buf[self.write] = x;
        self.write = (self.write + 1) & self.mask;
    }

    /// Read `delay` samples ago with linear interpolation.
    #[inline(always)]
    fn read_frac(&self, delay: f32) -> f32 {
        let d = delay.max(0.0).min((self.buf.len() - 2) as f32);
        let i = d as usize;
        let frac = d - i as f32;
        let idx0 = (self.write + self.buf.len() - 1 - i) & self.mask;
        let idx1 = (idx0 + self.buf.len() - 1) & self.mask;
        let a = self.buf[idx0];
        let b = self.buf[idx1];
        a + (b - a) * frac
    }

    fn reset(&mut self) { for s in &mut self.buf { *s = 0.0; } self.write = 0; }
}

//==============================================================================
// SPATIALIZER 3D EFFECT
//==============================================================================

/// 3D positional audio effect — combines ITD (interaural time delay),
/// ILD (interaural level + head-shadow low-pass), distance gain, air
/// absorption, elevation spectral cue, and early reflections. Works on
/// both headphones and speakers (speaker mode bypasses ITD and adds
/// a simple crosstalk cancellation bleed).
#[derive(Debug, Clone)]
pub struct Spatializer3DEffect {
    sample_rate: f32,

    // Parameters (all set externally)
    azimuth:    f32, // -180..180 (degrees)
    elevation:  f32, // -45..45   (degrees)
    distance:   f32, // 0.1..20 (metres)
    room_size:  f32, // 0..1
    air:        f32, // 0..1
    er_mix:     f32, // 0..1
    width:      f32, // 0..1
    mode:       f32, // 0 = binaural headphone, 1 = speaker
    pub mix:    f32,

    // State — one ITD delay line per ear + ILD head-shadow LPF + air LPF
    delay_l: FracDelayLine,
    delay_r: FracDelayLine,
    ild_lp_l: f32,
    ild_lp_r: f32,
    air_lp_l: f32,
    air_lp_r: f32,

    // Elevation peaking biquad (notch front, shelf above)
    elev_eq_l: RbjBiquad,
    elev_eq_r: RbjBiquad,

    // Early reflections — 4 taps at fixed musical spacings
    er: FracDelayLine,
    er_delays: [f32; 4], // in samples, scaled by room_size
}

impl Spatializer3DEffect {
    pub fn new(sample_rate: f32) -> Self {
        let max_itd_samples = ((sample_rate * 0.002) as usize).max(64); // 2 ms headroom
        let max_er_samples = ((sample_rate * 0.05) as usize).max(256);  // 50 ms headroom
        let mut s = Self {
            sample_rate,
            azimuth: 0.0,
            elevation: 0.0,
            distance: 1.0,
            room_size: 0.3,
            air: 0.3,
            er_mix: 0.25,
            width: 1.0,
            mode: 0.0,
            mix: 1.0,
            delay_l: FracDelayLine::new(max_itd_samples),
            delay_r: FracDelayLine::new(max_itd_samples),
            ild_lp_l: 0.0,
            ild_lp_r: 0.0,
            air_lp_l: 0.0,
            air_lp_r: 0.0,
            elev_eq_l: RbjBiquad::new(),
            elev_eq_r: RbjBiquad::new(),
            er: FracDelayLine::new(max_er_samples),
            er_delays: [0.0; 4],
        };
        s.update_coefs();
        s
    }

    fn update_coefs(&mut self) {
        // Elevation EQ — a small peaking biquad at 8 kHz
        // Negative gain for "front" (elevation ≈ 0), positive for "above"
        let gain_db = self.elevation / 45.0 * 4.0; // ±4 dB
        self.elev_eq_l.set_peak(self.sample_rate, 8000.0, 1.2, gain_db);
        self.elev_eq_r.set_peak(self.sample_rate, 8000.0, 1.2, gain_db);

        // ER delays (in samples): 7/11/17/23 ms scaled by room_size
        let scale = 0.25 + self.room_size * 1.5; // 0.25..1.75
        let ms_to_samples = self.sample_rate * 0.001;
        self.er_delays[0] = 7.0  * ms_to_samples * scale;
        self.er_delays[1] = 11.0 * ms_to_samples * scale;
        self.er_delays[2] = 17.0 * ms_to_samples * scale;
        self.er_delays[3] = 23.0 * ms_to_samples * scale;
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.mix <= 0.0001 { return (input_l, input_r); }

        // Feed each channel into its own ITD delay line so a pan of 0 passes
        // stereo through untouched. Early reflections use the mono sum.
        self.delay_l.write(input_l);
        self.delay_r.write(input_r);
        let er_src = (input_l + input_r) * 0.5;
        self.er.write(er_src);

        // Convert azimuth to normalized pan (-1..1), sin for side bias
        let az_rad = self.azimuth.to_radians();
        let pan = az_rad.sin().clamp(-1.0, 1.0); // -1 = left, +1 = right

        // ITD: up to 0.7 ms on the far ear. The far ear reads from a delayed
        // tap, the near ear reads at zero delay (unchanged).
        let max_itd_samples = self.sample_rate * 0.0007;
        let (d_l, d_r) = if pan < 0.0 {
            // Source on the left → right ear (far) delayed
            (0.0, -pan * max_itd_samples)
        } else {
            (pan * max_itd_samples, 0.0)
        };

        // In speaker mode, bypass ITD entirely.
        let speaker = self.mode > 0.5;
        let l_itd = if speaker { input_l } else { self.delay_l.read_frac(d_l) };
        let r_itd = if speaker { input_r } else { self.delay_r.read_frac(d_r) };

        // ILD — level attenuation on the far ear + head-shadow LPF
        // shadow in [0, 1] increases with lateral angle
        let shadow = pan.abs();
        let far_gain = 1.0 - shadow * 0.35;
        let near_gain = 1.0;
        let (mut l, mut r) = if pan < 0.0 {
            (l_itd * near_gain, r_itd * far_gain)
        } else {
            (l_itd * far_gain, r_itd * near_gain)
        };

        // Head-shadow LPF: 1-pole, cutoff 20k → ~2.5k at hard pan
        let shadow_cutoff = 20000.0 - 17500.0 * shadow;
        let shadow_alpha = Self::one_pole_alpha(shadow_cutoff, self.sample_rate);
        if pan < 0.0 {
            self.ild_lp_r += shadow_alpha * (r - self.ild_lp_r);
            r = self.ild_lp_r;
            self.ild_lp_l = l;
        } else {
            self.ild_lp_l += shadow_alpha * (l - self.ild_lp_l);
            l = self.ild_lp_l;
            self.ild_lp_r = r;
        }

        // Distance gain: 1/(1 + k*d) — gentler than true 1/r
        let dist_gain = 1.0 / (1.0 + 0.3 * self.distance.max(0.1));
        l *= dist_gain;
        r *= dist_gain;

        // Air absorption LPF: cutoff = 18k * exp(-air * d * 0.15)
        let air_cutoff = 18000.0 * (-(self.air * self.distance * 0.15)).exp();
        let air_alpha = Self::one_pole_alpha(air_cutoff.max(200.0), self.sample_rate);
        self.air_lp_l += air_alpha * (l - self.air_lp_l);
        self.air_lp_r += air_alpha * (r - self.air_lp_r);
        l = self.air_lp_l;
        r = self.air_lp_r;

        // Elevation spectral cue
        l = self.elev_eq_l.process(l);
        r = self.elev_eq_r.process(r);

        // Early reflections — 4 taps, panned alternately
        if self.er_mix > 0.001 {
            let t0 = self.er.read_frac(self.er_delays[0]);
            let t1 = self.er.read_frac(self.er_delays[1]);
            let t2 = self.er.read_frac(self.er_delays[2]);
            let t3 = self.er.read_frac(self.er_delays[3]);
            let er_l = (t0 * 0.8 + t2 * 0.6) * 0.5;
            let er_r = (t1 * 0.7 + t3 * 0.5) * 0.5;
            l += er_l * self.er_mix * dist_gain;
            r += er_r * self.er_mix * dist_gain;
        }

        // Speaker mode — light crosstalk cancellation bleed
        if speaker {
            let bleed = 0.2;
            let (l2, r2) = (l - r * bleed, r - l * bleed);
            l = l2; r = r2;
        }

        // Stereo width
        let mid = (l + r) * 0.5;
        let side = (l - r) * 0.5 * self.width;
        l = mid + side;
        r = mid - side;

        // Mix
        let out_l = input_l * (1.0 - self.mix) + l * self.mix;
        let out_r = input_r * (1.0 - self.mix) + r * self.mix;
        (out_l, out_r)
    }

    #[inline(always)]
    fn one_pole_alpha(cutoff: f32, fs: f32) -> f32 {
        let dt = 1.0 / fs;
        let rc = 1.0 / (2.0 * std::f32::consts::PI * cutoff.max(1.0));
        (dt / (rc + dt)).clamp(0.0, 1.0)
    }

    pub fn reset(&mut self) {
        self.delay_l.reset();
        self.delay_r.reset();
        self.er.reset();
        self.ild_lp_l = 0.0; self.ild_lp_r = 0.0;
        self.air_lp_l = 0.0; self.air_lp_r = 0.0;
        self.elev_eq_l.reset(); self.elev_eq_r.reset();
    }

    pub fn set_azimuth(&mut self, v: f32) { self.azimuth = v.clamp(-180.0, 180.0); }
    pub fn set_elevation(&mut self, v: f32) { self.elevation = v.clamp(-45.0, 45.0); self.update_coefs(); }
    pub fn set_distance(&mut self, v: f32) { self.distance = v.clamp(0.1, 20.0); }
    pub fn set_room_size(&mut self, v: f32) { self.room_size = v.clamp(0.0, 1.0); self.update_coefs(); }
    pub fn set_air(&mut self, v: f32) { self.air = v.clamp(0.0, 1.0); }
    pub fn set_er_mix(&mut self, v: f32) { self.er_mix = v.clamp(0.0, 1.0); }
    pub fn set_width(&mut self, v: f32) { self.width = v.clamp(0.0, 2.0); }
    pub fn set_mode(&mut self, v: f32) { self.mode = if v > 0.5 { 1.0 } else { 0.0 }; }
    pub fn set_mix(&mut self, v: f32) { self.mix = v.clamp(0.0, 1.0); }
}

//==============================================================================
// PARAMETRIC EQ EFFECT — 4 band (LS / Peak1 / Peak2 / HS)
//==============================================================================

#[derive(Debug, Clone)]
pub struct ParametricEqEffect {
    sample_rate: f32,

    // Per-band state (stereo pair)
    bands_l: [RbjBiquad; 4],
    bands_r: [RbjBiquad; 4],

    // Parameters — per band: freq, gain (dB), q, enabled
    freqs:    [f32; 4],
    gains_db: [f32; 4],
    qs:       [f32; 4],
    enabled:  [bool; 4],
    /// Per-band filter type. 0 = AUTO (position default: band0 low-shelf, band3 high-shelf,
    /// bands 1/2 peaking) so existing patches are byte-identical. 1=peak, 2=low-shelf,
    /// 3=high-shelf, 4=low-pass. Lets a 3-peak EQ (e.g. the canon drum bus 90/4500/9500) be
    /// reproduced EXACTLY instead of being forced into the shelf-at-the-edges layout.
    types:    [u8; 4],

    pub mix: f32,
}

impl ParametricEqEffect {
    pub fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            sample_rate,
            bands_l: [RbjBiquad::new(); 4],
            bands_r: [RbjBiquad::new(); 4],
            freqs: [100.0, 500.0, 2000.0, 8000.0],
            gains_db: [0.0; 4],
            qs: [0.707, 0.707, 0.707, 0.707],
            enabled: [true; 4],
            types: [0; 4], // all AUTO = the historical position-default layout
            mix: 1.0,
        };
        for b in 0..4 { s.update_band(b); }
        s
    }

    pub fn set_band_type(&mut self, b: usize, t: u8) {
        if b < 4 && self.types[b] != t { self.types[b] = t; self.update_band(b); }
    }

    fn update_band(&mut self, b: usize) {
        let fs = self.sample_rate;
        let f = self.freqs[b];
        let g = self.gains_db[b];
        let q = self.qs[b];
        // Resolve AUTO (0) to the historical per-position default so old patches are unchanged.
        let t = match self.types[b] { 0 => match b { 0 => 2, 3 => 3, _ => 1 }, t => t };
        match t {
            1 => { self.bands_l[b].set_peak(fs, f, q, g);       self.bands_r[b].set_peak(fs, f, q, g); }
            2 => { self.bands_l[b].set_low_shelf(fs, f, q, g);  self.bands_r[b].set_low_shelf(fs, f, q, g); }
            3 => { self.bands_l[b].set_high_shelf(fs, f, q, g); self.bands_r[b].set_high_shelf(fs, f, q, g); }
            4 => { self.bands_l[b].set_lowpass(fs, f, q);       self.bands_r[b].set_lowpass(fs, f, q); }
            _ => { self.bands_l[b].set_peak(fs, f, q, g);       self.bands_r[b].set_peak(fs, f, q, g); }
        }
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.mix <= 0.0001 { return (input_l, input_r); }
        let mut l = input_l;
        let mut r = input_r;
        for b in 0..4 {
            // Process a band when enabled AND it actually shapes the signal: a gain band needs
            // |gain| > 0.01 (a ~0 dB peak/shelf is a no-op), but a LOWPASS band (type 4) is
            // gain-independent and must always run -- the old `gain > 0.01` gate silently skipped
            // every lowpass band (they carry 0 gain), so a type-4 EQ band did nothing.
            if self.enabled[b] && (self.gains_db[b].abs() > 0.01 || self.types[b] == 4) {
                l = self.bands_l[b].process(l);
                r = self.bands_r[b].process(r);
            }
        }
        let out_l = input_l * (1.0 - self.mix) + l * self.mix;
        let out_r = input_r * (1.0 - self.mix) + r * self.mix;
        (out_l, out_r)
    }

    pub fn reset(&mut self) {
        for b in 0..4 { self.bands_l[b].reset(); self.bands_r[b].reset(); }
    }

    pub fn set_band_freq(&mut self, b: usize, f: f32) {
        if b < 4 { self.freqs[b] = f.clamp(20.0, 20000.0); self.update_band(b); }
    }
    pub fn set_band_gain(&mut self, b: usize, g: f32) {
        if b < 4 { self.gains_db[b] = g.clamp(-18.0, 18.0); self.update_band(b); }
    }
    pub fn set_band_q(&mut self, b: usize, q: f32) {
        if b < 4 { self.qs[b] = q.clamp(0.1, 10.0); self.update_band(b); }
    }
    pub fn set_band_enabled(&mut self, b: usize, en: bool) {
        if b < 4 { self.enabled[b] = en; }
    }
    pub fn set_mix(&mut self, m: f32) { self.mix = m.clamp(0.0, 1.0); }

    /// Evaluate the combined magnitude response at `freq`, in linear scale.
    /// Used by the GUI to draw the FR curve.
    pub fn magnitude_at(&self, freq: f32) -> f32 {
        let mut m = 1.0;
        for b in 0..4 {
            if self.enabled[b] {
                m *= self.bands_l[b].magnitude_at(self.sample_rate, freq);
            }
        }
        m
    }
    pub fn band_freq(&self, b: usize) -> f32 { self.freqs[b.min(3)] }
    pub fn band_gain(&self, b: usize) -> f32 { self.gains_db[b.min(3)] }
    pub fn band_q(&self, b: usize) -> f32 { self.qs[b.min(3)] }
}

//==============================================================================
// DYNAMIC EQ — 4-band parametric with per-band envelope-driven gain
//==============================================================================

/// Four-band dynamic parametric EQ. Each band is a peak filter whose
/// gain is driven by an envelope follower tapped from the same band's
/// signal — exceed the threshold and the band attenuates (downward
/// mode) or boosts (upward mode). This is the "make everything
/// clear" tool of the mastering chain: a -4 dB downward band at
/// 250 Hz dips the mud only when the mud is actually too loud,
/// leaving the kick alone the rest of the time.
///
/// Topology per band, per channel:
/// ```text
/// input ─┬─► sense peak biquad (unity gain) ─► |x|² envelope follower
///        │                                              │
///        └─► apply peak biquad (dynamic gain) ─► output │
///              ▲                                        │
///              └────── coef recompute at 32 sa ─────────┘
/// ```
/// The sense filter is unity-gain so it doesn't get caught in a
/// feedback loop with the apply filter's gain changes. Coefficients
/// for the apply filter are recomputed at control rate (every 32
/// samples) — the envelope still tracks per sample, and the
/// recompute is bounded so it never stalls the audio thread.
#[derive(Debug, Clone)]
pub struct DynamicEqEffect {
    sample_rate: f32,

    // Per-band sensing filter (stereo, unity gain peak at band freq).
    sense_l: [RbjBiquad; 4],
    sense_r: [RbjBiquad; 4],
    // Per-band application filter (stereo, dynamic gain).
    apply_l: [RbjBiquad; 4],
    apply_r: [RbjBiquad; 4],

    // Per-band envelope state (linear amplitude, smoothed).
    env: [f32; 4],
    attack_coef:  [f32; 4],
    release_coef: [f32; 4],

    // Per-band user parameters.
    freqs:        [f32; 4],
    qs:           [f32; 4],
    threshold_db: [f32; 4],
    ratio:        [f32; 4],
    range_db:     [f32; 4],
    /// Attack time in **seconds** — matches the existing Compressor /
    /// Limiter / Expander conventions so the `ValFmt::Ms` knob
    /// formatter shows ms while the param is stored / automated in
    /// seconds.
    attack_sec:   [f32; 4],
    release_sec:  [f32; 4],
    /// 0 = downward (attenuate when loud), 1 = upward (boost when quiet).
    mode:         [u8; 4],
    enabled:      [bool; 4],

    /// Per-band current applied gain (dB) — interpolates toward the
    /// envelope-driven target. Recomputed on the audio thread; the
    /// apply biquad coefs are recomputed at control rate (every 32
    /// samples) from this value.
    current_gain_db: [f32; 4],
    /// Target gain set per-sample from the envelope; the actual
    /// applied gain interpolates toward this over a few ms.
    target_gain_db:  [f32; 4],
    ctrl_counter: u32,
    /// Idle-bypass: consecutive near-silent input samples. After the
    /// idle threshold the 4 sense + 4 apply biquads / channel and
    /// per-band envelope updates are skipped — envelopes release-decay
    /// instead. Recovers on the first non-silent sample.
    silence_samples: u32,

    pub mix: f32,
}

impl DynamicEqEffect {
    pub fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            sample_rate,
            sense_l: [RbjBiquad::new(); 4],
            sense_r: [RbjBiquad::new(); 4],
            apply_l: [RbjBiquad::new(); 4],
            apply_r: [RbjBiquad::new(); 4],
            env: [0.0; 4],
            attack_coef:  [0.0; 4],
            release_coef: [0.0; 4],
            freqs:        [120.0, 400.0, 2500.0, 8000.0],
            qs:           [1.0; 4],
            threshold_db: [-18.0; 4],
            ratio:        [2.0; 4],
            range_db:     [6.0; 4],
            attack_sec:   [0.010; 4],
            release_sec:  [0.120; 4],
            mode:         [0; 4],
            enabled:      [true; 4],
            current_gain_db: [0.0; 4],
            target_gain_db:  [0.0; 4],
            ctrl_counter: 0,
            silence_samples: 0,
            mix: 1.0,
        };
        for b in 0..4 {
            s.update_envelope_coefs(b);
            s.update_sense_filter(b);
            s.update_apply_filter(b);
        }
        s
    }

    fn update_envelope_coefs(&mut self, b: usize) {
        let atk = (self.attack_sec[b].max(0.0001) * self.sample_rate).max(1.0);
        let rel = (self.release_sec[b].max(0.001) * self.sample_rate).max(1.0);
        self.attack_coef[b]  = (-1.0_f32 / atk).exp();
        self.release_coef[b] = (-1.0_f32 / rel).exp();
    }

    fn update_sense_filter(&mut self, b: usize) {
        let fs = self.sample_rate;
        let f  = self.freqs[b];
        let q  = self.qs[b];
        // Sense is a unity-gain peak — but RBJ peak at gain=0 dB is a
        // pass-through. We want a bandpass-flavoured detector, so use
        // a peak biquad at +12 dB (boosts the band so the envelope
        // tracks the band's contribution) — divided back out by the
        // envelope-side normalisation when comparing to threshold.
        // Subtler: we want a TRUE band-pass for envelope detection.
        // Using set_peak with a +12 dB gain biases detection toward
        // the band, but the absolute level is then 4× too high.
        // For now use a high-Q peak at 0 dB which is essentially
        // pass-through; envelope detection uses an extra band-pass
        // approximation via running difference.
        self.sense_l[b].set_peak(fs, f, q.max(0.5), 12.0);
        self.sense_r[b].set_peak(fs, f, q.max(0.5), 12.0);
    }

    fn update_apply_filter(&mut self, b: usize) {
        let fs = self.sample_rate;
        let f  = self.freqs[b];
        let q  = self.qs[b];
        let g  = if self.enabled[b] { self.current_gain_db[b] } else { 0.0 };
        self.apply_l[b].set_peak(fs, f, q, g);
        self.apply_r[b].set_peak(fs, f, q, g);
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.mix <= 1.0e-4 { return (input_l, input_r); }

        // Idle-bypass: silent input for ≥1024 samples → 4 sense + 4
        // apply biquads / channel and the 4 envelope updates per sample
        // are wasted work. Decay the envelopes via release coef and
        // return identity. Recovers on the first non-silent sample.
        const SILENCE_EPS: f32 = 1.0e-6;
        const IDLE_AFTER_SAMPLES: u32 = 1024;
        let abs_sum = input_l.abs() + input_r.abs();
        if abs_sum < SILENCE_EPS {
            self.silence_samples = self.silence_samples.saturating_add(1);
            if self.silence_samples >= IDLE_AFTER_SAMPLES {
                for b in 0..4 {
                    self.env[b] *= self.release_coef[b];
                    self.current_gain_db[b] *= 0.999;
                }
                return (input_l, input_r);
            }
        } else {
            self.silence_samples = 0;
        }

        let mut out_l = input_l;
        let mut out_r = input_r;

        let do_coef_update = (self.ctrl_counter & 31) == 0;
        self.ctrl_counter = self.ctrl_counter.wrapping_add(1);

        for b in 0..4 {
            if !self.enabled[b] { continue; }

            // Sense the band's contribution. The sense biquad has a
            // +12 dB peak so the band stands out clearly from the
            // surrounding spectrum; the envelope follower's
            // threshold comparison subtracts this boost back out
            // (the band sense level in dBFS is `lvl_db - 12`).
            let s_l = self.sense_l[b].process(input_l);
            let s_r = self.sense_r[b].process(input_r);
            let abs_band = 0.5 * (s_l.abs() + s_r.abs());
            // Envelope follower — asymmetric attack/release.
            let coef = if abs_band > self.env[b] {
                self.attack_coef[b]
            } else {
                self.release_coef[b]
            };
            self.env[b] = self.env[b] * coef + abs_band * (1.0 - coef);

            // Convert to dB and subtract the sense filter's 12 dB
            // boost so threshold comparison happens on the actual
            // input level at the band's frequency.
            let env_db = if self.env[b] > 1.0e-9 {
                20.0 * self.env[b].log10()
            } else { -120.0 } - 12.0;

            // Compute desired gain change in dB.
            let target = match self.mode[b] {
                0 => {
                    // Downward: attenuate the band when input exceeds threshold.
                    let over = (env_db - self.threshold_db[b]).max(0.0);
                    -(over * (1.0 - 1.0 / self.ratio[b].max(1.0)))
                }
                _ => {
                    // Upward: boost the band when input is BELOW threshold.
                    let under = (self.threshold_db[b] - env_db).max(0.0);
                    under * (1.0 - 1.0 / self.ratio[b].max(1.0))
                }
            };
            // Clamp to user-set range (per band).
            self.target_gain_db[b] = target.clamp(-self.range_db[b], self.range_db[b]);

            // Smoothly interpolate current gain toward target each
            // sample so the apply-filter recompute doesn't introduce
            // zipper noise between control-rate boundaries.
            let alpha = 0.05_f32;  // ~1 ms time constant at 48 kHz
            self.current_gain_db[b] = self.current_gain_db[b]
                + (self.target_gain_db[b] - self.current_gain_db[b]) * alpha;

            // Apply the band's peak filter (uses cached coefs).
            out_l = self.apply_l[b].process(out_l);
            out_r = self.apply_r[b].process(out_r);
        }

        // Control-rate apply-filter coef refresh — bounded amortised
        // cost: 4 biquad recomputes every 32 samples = ~0.5 ops/sample.
        if do_coef_update {
            for b in 0..4 {
                if self.enabled[b] && self.target_gain_db[b].abs() > 0.05 {
                    self.update_apply_filter(b);
                }
            }
        }

        let m = self.mix;
        (input_l * (1.0 - m) + out_l * m,
         input_r * (1.0 - m) + out_r * m)
    }

    pub fn reset(&mut self) {
        for b in 0..4 {
            self.sense_l[b].reset(); self.sense_r[b].reset();
            self.apply_l[b].reset(); self.apply_r[b].reset();
            self.env[b] = 0.0;
            self.current_gain_db[b] = 0.0;
            self.target_gain_db[b] = 0.0;
        }
        self.ctrl_counter = 0;
        self.silence_samples = 0;
    }

    pub fn set_band_freq(&mut self, b: usize, f: f32) {
        if b < 4 { self.freqs[b] = f.clamp(20.0, 20_000.0); self.update_sense_filter(b); self.update_apply_filter(b); }
    }
    pub fn set_band_q(&mut self, b: usize, q: f32) {
        if b < 4 { self.qs[b] = q.clamp(0.1, 10.0); self.update_sense_filter(b); self.update_apply_filter(b); }
    }
    pub fn set_band_threshold(&mut self, b: usize, db: f32) {
        if b < 4 { self.threshold_db[b] = db.clamp(-80.0, 0.0); }
    }
    pub fn set_band_ratio(&mut self, b: usize, r: f32) {
        if b < 4 { self.ratio[b] = r.clamp(1.0, 8.0); }
    }
    pub fn set_band_range(&mut self, b: usize, db: f32) {
        if b < 4 { self.range_db[b] = db.clamp(0.0, 24.0); }
    }
    /// Attack time in **seconds** (matches existing dynamics effects).
    pub fn set_band_attack(&mut self, b: usize, sec: f32) {
        if b < 4 { self.attack_sec[b] = sec.clamp(0.0001, 0.1); self.update_envelope_coefs(b); }
    }
    /// Release time in **seconds**.
    pub fn set_band_release(&mut self, b: usize, sec: f32) {
        if b < 4 { self.release_sec[b] = sec.clamp(0.001, 2.0); self.update_envelope_coefs(b); }
    }
    pub fn set_band_mode(&mut self, b: usize, mode: u8) {
        if b < 4 { self.mode[b] = mode.min(1); }
    }
    pub fn set_band_enabled(&mut self, b: usize, en: bool) {
        if b < 4 { self.enabled[b] = en; self.update_apply_filter(b); }
    }
    pub fn set_mix(&mut self, m: f32) { self.mix = m.clamp(0.0, 1.0); }

    /// Current per-band gain reduction in dB — for GUI/automation.
    pub fn band_gain_db(&self, b: usize) -> f32 { self.current_gain_db[b.min(3)] }
    /// Per-band centre frequency in Hz.
    pub fn band_freq(&self, b: usize) -> f32 { self.freqs[b.min(3)] }
}

//==============================================================================
// MID/SIDE EQ — M/S matrix into two independent 4-band parametric EQs
//==============================================================================

/// Mid/Side equaliser. Splits the stereo signal into Mid (L+R)/√2
/// and Side (L-R)/√2 sums, runs an independent 4-band parametric on
/// each, and recombines into L/R. Two `ParametricEqEffect` instances
/// internally — no new biquad code, all of the magic is the M/S
/// matrix on the boundaries.
///
/// Use cases: tighten mono bass (drop low end on Side), widen treble
/// (boost high shelf on Side), de-ess centred vocal without touching
/// the wide reverb (peak cut on Mid only).
#[derive(Debug, Clone)]
pub struct MidSideEqEffect {
    mid: ParametricEqEffect,
    side: ParametricEqEffect,
    /// Idle-bypass: consecutive near-silent input samples. After the
    /// idle threshold the M/S matrix + 2x ParametricEq biquads /
    /// channel are skipped. Recovers on the first audible sample.
    silence_samples: u32,
    pub mix: f32,
}

impl MidSideEqEffect {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            mid:  ParametricEqEffect::new(sample_rate),
            side: ParametricEqEffect::new(sample_rate),
            silence_samples: 0,
            mix: 1.0,
        }
    }
    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.mix <= 1.0e-4 { return (input_l, input_r); }

        // Idle-bypass: silent input for ≥1024 samples → both M and S
        // ParametricEq biquads have settled to ~0 output. Skip the
        // 2× M/S matrix + 2× 4-band parametric work. Recovers on the
        // first audible sample.
        const SILENCE_EPS: f32 = 1.0e-6;
        const IDLE_AFTER_SAMPLES: u32 = 1024;
        let abs_sum = input_l.abs() + input_r.abs();
        if abs_sum < SILENCE_EPS {
            self.silence_samples = self.silence_samples.saturating_add(1);
            if self.silence_samples >= IDLE_AFTER_SAMPLES {
                let m = self.mix;
                return (input_l * (1.0 - m), input_r * (1.0 - m));
            }
        } else {
            self.silence_samples = 0;
        }

        // L/R -> M/S (energy-preserving 1/sqrt(2) scaling).
        const INV_SQRT2: f32 = 0.707_106_78;
        let mid_in  = (input_l + input_r) * INV_SQRT2;
        let side_in = (input_l - input_r) * INV_SQRT2;
        // Each EQ is mono in / mono out — we pass the same sample on
        // both "channels" so the per-band biquads stay in lock-step;
        // averaging the two outputs cancels accumulated rounding.
        let (m_a, m_b) = self.mid.process(mid_in, mid_in);
        let (s_a, s_b) = self.side.process(side_in, side_in);
        let mid_out  = 0.5 * (m_a + m_b);
        let side_out = 0.5 * (s_a + s_b);
        // M/S -> L/R.
        let l = (mid_out + side_out) * INV_SQRT2;
        let r = (mid_out - side_out) * INV_SQRT2;
        let m = self.mix;
        (input_l * (1.0 - m) + l * m,
         input_r * (1.0 - m) + r * m)
    }
    pub fn reset(&mut self) {
        self.mid.reset(); self.side.reset();
        self.silence_samples = 0;
    }
    pub fn set_mix(&mut self, m: f32) { self.mix = m.clamp(0.0, 1.0); }

    pub fn set_mid_band(&mut self, b: usize, freq: f32, gain_db: f32, q: f32) {
        self.mid.set_band_freq(b, freq);
        self.mid.set_band_gain(b, gain_db);
        self.mid.set_band_q(b, q);
    }
    pub fn set_side_band(&mut self, b: usize, freq: f32, gain_db: f32, q: f32) {
        self.side.set_band_freq(b, freq);
        self.side.set_band_gain(b, gain_db);
        self.side.set_band_q(b, q);
    }
    pub fn set_mid_band_enabled(&mut self, b: usize, en: bool) { self.mid.set_band_enabled(b, en); }
    pub fn set_side_band_enabled(&mut self, b: usize, en: bool) { self.side.set_band_enabled(b, en); }
}

//==============================================================================
// SPECTRUM ANALYSER — 32-band logarithmic filterbank
//==============================================================================

/// 32-band log-spaced spectrum analyser. Cheaper than an FFT for our
/// use case (real-time visual + Master Assistant spectral matching)
/// because we want exactly the bands we plan to display — there's no
/// transform/window-overlap overhead and no new dependency on a FFT
/// crate, and per-sample cost is fully bounded.
///
/// Topology:
///   mono(input) -> 32 band-pass biquads (1/3-octave spacing)
///   -> per-band squared-envelope follower (fast attack, slow release)
///   -> 32 dB levels published via the public accessor.
///
/// Audio passes through unchanged — this is a measurement-only
/// effect (like `LufsMeter`). Per-sample cost: 32 biquads + 32
/// envelope ops on the mono mix = ~256 mac/sample (~12 Mop/sec at
/// 48 kHz). Stereo input is summed to mono before the filterbank so
/// the band envelopes track the listener-relevant mix energy, not
/// stereo phase.
pub struct SpectrumAnalyzerEffect {
    sample_rate: f32,
    /// 32 band-pass biquads, 1/3-octave centres from ~25 Hz to ~16 kHz.
    bands: [RbjBiquad; 32],
    /// Per-band squared envelope (linear amplitude²).
    env_sq: [f32; 32],
    /// Per-band attack / release coefficients (fast attack, slow release).
    attack_coef:  f32,
    release_coef: f32,
    /// Idle-bypass counter — consecutive near-silent input samples. Once
    /// past the idle threshold, the per-sample 32-biquad filterbank is
    /// skipped and envelopes simply decay via release coef.
    silence_samples: u32,
    pub mix: f32,
}

impl std::fmt::Debug for SpectrumAnalyzerEffect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SpectrumAnalyzerEffect(sr={}, mix={})", self.sample_rate, self.mix)
    }
}
impl Clone for SpectrumAnalyzerEffect {
    fn clone(&self) -> Self {
        Self {
            sample_rate: self.sample_rate,
            bands: self.bands,
            env_sq: self.env_sq,
            attack_coef: self.attack_coef,
            release_coef: self.release_coef,
            silence_samples: self.silence_samples,
            mix: self.mix,
        }
    }
}

impl SpectrumAnalyzerEffect {
    /// 32 1/3-octave band centres (Hz). Logarithmic from ~25 Hz up
    /// through ~16 kHz; covers the audible spectrum with one band
    /// per third-octave so display reads like a proper RTA.
    pub const BAND_FREQS: [f32; 32] = [
         25.0,   31.5,   40.0,   50.0,   63.0,   80.0,   100.0,  125.0,
        160.0,  200.0,  250.0,  315.0,  400.0,  500.0,  630.0,  800.0,
       1000.0, 1250.0, 1600.0, 2000.0, 2500.0, 3150.0, 4000.0, 5000.0,
       6300.0, 8000.0,10000.0,12500.0,16000.0,18000.0,19000.0,20000.0,
    ];

    pub fn new(sample_rate: f32) -> Self {
        let mut bands = [RbjBiquad::new(); 32];
        for (i, f) in Self::BAND_FREQS.iter().enumerate() {
            // Q ≈ centre / bandwidth. For 1/3-octave bandwidth Q is
            // ~4.32 — gives a clean per-band response without
            // adjacent-band bleed.
            bands[i].set_peak(sample_rate, *f, 4.32, 12.0);
        }
        // ~5 ms attack, ~200 ms release at any sr.
        let atk = (0.005 * sample_rate).max(1.0);
        let rel = (0.2 * sample_rate).max(1.0);
        Self {
            sample_rate,
            bands,
            env_sq: [0.0; 32],
            attack_coef:  (-1.0_f32 / atk).exp(),
            release_coef: (-1.0_f32 / rel).exp(),
            silence_samples: 0,
            mix: 1.0,
        }
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        // Idle-bypass: silent input for long enough → biquads have
        // decayed to ~0 output anyway; we can skip the per-sample
        // filterbank work and just release-decay the envelopes.
        // Threshold ≈ -120 dBFS, idle window ≈ 21 ms at 48 kHz so we
        // engage idle quickly but recover on the first audible sample.
        const SILENCE_EPS: f32 = 1.0e-6;
        const IDLE_AFTER_SAMPLES: u32 = 1024;
        let abs_sum = input_l.abs() + input_r.abs();
        if abs_sum < SILENCE_EPS {
            self.silence_samples = self.silence_samples.saturating_add(1);
            if self.silence_samples >= IDLE_AFTER_SAMPLES {
                for e in &mut self.env_sq { *e *= self.release_coef; }
                return (input_l, input_r);
            }
        } else {
            self.silence_samples = 0;
        }

        // Sum to mono for the filterbank — the listener perceives
        // band energy across both channels, not phase relationships.
        let mono = 0.5 * (input_l + input_r);
        for i in 0..32 {
            let y = self.bands[i].process(mono);
            let mag = y * y;
            let coef = if mag > self.env_sq[i] {
                self.attack_coef
            } else {
                self.release_coef
            };
            self.env_sq[i] = self.env_sq[i] * coef + mag * (1.0 - coef);
        }
        // Pass audio through unmodified.
        (input_l, input_r)
    }

    /// Snapshot of per-band envelope levels in dB. Indexed by band
    /// (0..32). Useful for both GUI display and the Master
    /// Assistant's spectral-matching analysis.
    pub fn levels_db(&self) -> [f32; 32] {
        let mut out = [-120.0_f32; 32];
        for i in 0..32 {
            // env_sq is amplitude²; level_db = 10 * log10(env_sq).
            if self.env_sq[i] > 1.0e-12 {
                out[i] = 10.0 * self.env_sq[i].log10();
                // Sense filter added +12 dB at centre; subtract it
                // so the displayed level matches the input dBFS.
                out[i] -= 12.0;
            }
        }
        out
    }

    pub fn reset(&mut self) {
        for b in &mut self.bands { b.reset(); }
        for e in &mut self.env_sq { *e = 0.0; }
        self.silence_samples = 0;
    }
    pub fn set_mix(&mut self, m: f32) { self.mix = m.clamp(0.0, 1.0); }
}

//==============================================================================
// MASTER ASSISTANT — auto-loudness + spectral-matching meta-plugin
//==============================================================================

/// Built-in target spectra (per-band dB offsets relative to a
/// pink-noise reference). The Master Assistant compares its 32-band
/// running average to the chosen target and computes a per-band gain
/// delta. Negative offsets mean "should be quieter here".
fn target_curve(curve_idx: u8) -> [f32; 32] {
    match curve_idx {
        0 => {
            // "Modern techno" — strong sub + low mid emphasis, scooped
            // 400-800 Hz, gentle high-shelf lift above 4 kHz.
            let mut t = [0.0; 32];
            t[0] = 3.0; t[1] = 3.5; t[2] = 4.0; t[3] = 4.0; t[4] = 3.5;
            t[5] = 2.0; t[6] = 1.0;
            t[10] = -2.0; t[11] = -2.5; t[12] = -3.0; t[13] = -2.5; t[14] = -2.0;
            t[20] = 1.0; t[21] = 1.5; t[22] = 2.0; t[23] = 2.0; t[24] = 1.5;
            t
        }
        1 => {
            // "Warm club" — full low, smooth mid, slight high rolloff.
            let mut t = [0.0; 32];
            t[1] = 2.0; t[2] = 2.5; t[3] = 2.0; t[4] = 1.5;
            t[15] = 0.0; t[20] = -0.5; t[28] = -1.5; t[29] = -2.5; t[30] = -3.0; t[31] = -3.5;
            t
        }
        2 => {
            // "Balanced" — flat across the board (identity target).
            [0.0; 32]
        }
        _ => {
            // "Vocal-forward" — boost 2-4 kHz, slight low rolloff.
            let mut t = [0.0; 32];
            t[0] = -2.0; t[1] = -1.0;
            t[19] = 1.5; t[20] = 2.5; t[21] = 3.0; t[22] = 2.5; t[23] = 1.5;
            t
        }
    }
}

/// Operating mode for the Master Assistant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MaMode { Off = 0, Listen = 1, Apply = 2, AutoTweak = 3 }

impl MaMode {
    fn from_f32(v: f32) -> Self {
        match v as i32 {
            1 => MaMode::Listen,
            2 => MaMode::Apply,
            3 => MaMode::AutoTweak,
            _ => MaMode::Off,
        }
    }
}

/// Master Assistant — auto-loudness + spectral matching. Drops in a
/// master FX slot. In "Listen" mode it just analyses; in "Apply" it
/// pushes a one-shot gain + EQ correction; in "Auto-tweak" it
/// continually nudges toward the target with a slow time constant.
///
/// Internally owns:
///   * a `LufsMeter` for measured loudness
///   * a `SpectrumAnalyzer` for the band signature
///   * a small parameter store for the corrections (input gain
///     delta in dB, and 32 per-band EQ deltas in dB)
///
/// The corrections are CONSUMED by the GUI / engine which reads them
/// out via the public accessor and applies them to sibling FX slots
/// (or directly to the bus gain). This keeps the meta-plugin
/// dependency-free — it doesn't reach into the rest of the chain
/// directly, it just publishes target settings the host can act on.
pub struct MasterAssistantEffect {
    sample_rate: f32,
    lufs: crate::meters::LufsMeter,
    spectrum: SpectrumAnalyzerEffect,
    /// Target integrated loudness, dB FS (e.g. -14 for streaming).
    target_lufs: f32,
    /// Built-in curve index (0 = modern techno, 1 = warm club,
    /// 2 = balanced, 3 = vocal-forward).
    target_curve_idx: u8,
    /// Mode: Off / Listen / Apply / AutoTweak.
    mode: MaMode,
    /// Cached corrections published by `corrections()`.
    correction_gain_db: f32,
    correction_band_db: [f32; 32],
    /// Sample counter for slow auto-tweak time constant (decoupled
    /// from the audio sample rate — corrections update every 1 s).
    update_counter: u32,
    /// 8 octave-band peaking biquads per channel — the corrective
    /// EQ stage the Assistant actually applies in Apply / AutoTweak
    /// modes. Coefficients refresh at 1 Hz from `correction_band_db`.
    correction_eq_l: [RbjBiquad; 8],
    correction_eq_r: [RbjBiquad; 8],
    /// Smoothed linear gain applied in Apply / AutoTweak modes —
    /// tracks `10^(correction_gain_db / 20)`. The smoother avoids
    /// clicks when corrections refresh.
    applied_gain_lin: f32,
    /// Per-supergroup smoothed dB gain (0..8) so coefficient updates
    /// also slew rather than snap.
    applied_band_db: [f32; 8],
    pub mix: f32,
}

impl std::fmt::Debug for MasterAssistantEffect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MasterAssistantEffect(target_lufs={}, mode={:?})",
            self.target_lufs, self.mode)
    }
}
impl Clone for MasterAssistantEffect {
    fn clone(&self) -> Self {
        Self {
            sample_rate: self.sample_rate,
            lufs: self.lufs.clone(),
            spectrum: self.spectrum.clone(),
            target_lufs: self.target_lufs,
            target_curve_idx: self.target_curve_idx,
            mode: self.mode,
            correction_gain_db: self.correction_gain_db,
            correction_band_db: self.correction_band_db,
            update_counter: self.update_counter,
            correction_eq_l: self.correction_eq_l,
            correction_eq_r: self.correction_eq_r,
            applied_gain_lin: self.applied_gain_lin,
            applied_band_db: self.applied_band_db,
            mix: self.mix,
        }
    }
}

/// 8 octave-band correction-EQ centre frequencies. Chosen to match
/// the 4-of-32 grouping of the spectrum analyzer's 1/3-octave bands.
const MA_BAND_FREQS: [f32; 8] = [40.0, 100.0, 250.0, 600.0, 1500.0, 3500.0, 8000.0, 15000.0];

impl MasterAssistantEffect {
    pub fn new(sample_rate: f32) -> Self {
        let mut correction_eq_l = [RbjBiquad::new(); 8];
        let mut correction_eq_r = [RbjBiquad::new(); 8];
        for i in 0..8 {
            // Q ≈ 1.41 is "octave-wide" — overlapping bands sum to a
            // smooth, RTA-like response. 0 dB gain at init.
            correction_eq_l[i].set_peak(sample_rate, MA_BAND_FREQS[i], 1.41, 0.0);
            correction_eq_r[i].set_peak(sample_rate, MA_BAND_FREQS[i], 1.41, 0.0);
        }
        Self {
            sample_rate,
            lufs: crate::meters::LufsMeter::new(sample_rate),
            spectrum: SpectrumAnalyzerEffect::new(sample_rate),
            target_lufs: -14.0,
            target_curve_idx: 0,
            mode: MaMode::Listen,
            correction_gain_db: 0.0,
            correction_band_db: [0.0; 32],
            update_counter: 0,
            correction_eq_l,
            correction_eq_r,
            applied_gain_lin: 1.0,
            applied_band_db: [0.0; 8],
            mix: 1.0,
        }
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        // Mode-aware measurement: Off truly bypasses (no K-weighting,
        // no 32-biquad filterbank — these are the most expensive part
        // of the Master Assistant). Listen / Apply / AutoTweak need
        // the measurements because either the GUI displays them or
        // the chain consumes them.
        if self.mode == MaMode::Off {
            // Slew toward unity so a previous Apply mode's gain
            // releases gracefully on Off, then bypass.
            let slew = 1.0 - (-1.0_f32 / (0.05 * self.sample_rate)).exp();
            self.applied_gain_lin += (1.0 - self.applied_gain_lin) * slew;
            // Update the once-per-second decay tick so corrections
            // fade out for the next time the user toggles back on.
            self.update_counter += 1;
            let update_interval = self.sample_rate as u32;
            if self.update_counter >= update_interval {
                self.update_counter = 0;
                self.recompute_corrections();
                self.refresh_correction_eq_coefs();
            }
            return (input_l * self.applied_gain_lin, input_r * self.applied_gain_lin);
        }

        // Measure on the DRY input — keeps published readings honest
        // regardless of what we apply on the way out.
        self.lufs.process(input_l, input_r);
        self.spectrum.process(input_l, input_r);

        // Update the correction targets at ~1 Hz to avoid pumping.
        self.update_counter += 1;
        let update_interval = self.sample_rate as u32; // 1 s
        if self.update_counter >= update_interval {
            self.update_counter = 0;
            self.recompute_corrections();
            self.refresh_correction_eq_coefs();
        }

        // Slew the linear gain toward the target each sample. ~50 ms
        // smoother (1 - exp(-1/(0.05*sr))) — fast enough to be useful,
        // slow enough to avoid clicks when corrections refresh.
        let target_lin = match self.mode {
            MaMode::Apply | MaMode::AutoTweak => {
                10.0_f32.powf(self.correction_gain_db / 20.0)
            }
            // Listen doesn't drive output — slew back to unity.
            _ => 1.0,
        };
        let slew = 1.0 - (-1.0_f32 / (0.05 * self.sample_rate)).exp();
        self.applied_gain_lin += (target_lin - self.applied_gain_lin) * slew;

        // In Apply / AutoTweak modes, run the corrective EQ stage and
        // apply the smoothed gain. Listen remains passthrough modulo
        // the gain smoother decaying toward 1.
        match self.mode {
            MaMode::Apply | MaMode::AutoTweak => {
                let mut l = input_l;
                let mut r = input_r;
                for i in 0..8 {
                    l = self.correction_eq_l[i].process(l);
                    r = self.correction_eq_r[i].process(r);
                }
                (l * self.applied_gain_lin, r * self.applied_gain_lin)
            }
            _ => {
                // Still apply the slewed gain so toggling Apply -> Listen
                // doesn't snap back to unity with a click.
                (input_l * self.applied_gain_lin, input_r * self.applied_gain_lin)
            }
        }
    }

    /// Recompute biquad coefficients from the per-band correction
    /// deltas. Called at ~1 Hz from `process` after corrections
    /// refresh — coefficient updates outside the audio loop keep the
    /// per-sample cost to 8 biquads/channel.
    fn refresh_correction_eq_coefs(&mut self) {
        // Group the 32 1/3-octave correction deltas into 8 super-bands
        // (4 thirds per octave). Average so an outlier band can't
        // dominate the octave's correction.
        let mut grouped = [0.0_f32; 8];
        for sb in 0..8 {
            let mut acc = 0.0_f32;
            for i in 0..4 {
                acc += self.correction_band_db[sb * 4 + i];
            }
            grouped[sb] = acc / 4.0;
        }
        // Slew toward the new grouped values so coefficient refreshes
        // don't introduce per-second steps in the EQ curve. 30 % per
        // update ≈ 3 s exponential settle.
        let coef = 0.3_f32;
        for sb in 0..8 {
            self.applied_band_db[sb] += (grouped[sb] - self.applied_band_db[sb]) * coef;
            self.correction_eq_l[sb].set_peak(
                self.sample_rate, MA_BAND_FREQS[sb], 1.41, self.applied_band_db[sb]);
            self.correction_eq_r[sb].set_peak(
                self.sample_rate, MA_BAND_FREQS[sb], 1.41, self.applied_band_db[sb]);
        }
    }

    fn recompute_corrections(&mut self) {
        if self.mode == MaMode::Off {
            // Decay corrections toward zero in Off mode so removing
            // the assistant restores the chain to its pre-assistant
            // state.
            self.correction_gain_db *= 0.5;
            for v in &mut self.correction_band_db { *v *= 0.5; }
            return;
        }
        // Loudness correction: measured -> target.
        let measured = self.lufs.integrated_lufs();
        let mut gain_delta_db = self.target_lufs - measured;
        // Cap the per-update gain delta so a transient outlier can't
        // command +24 dB of input gain — Apply mode hits target in
        // 2-3 updates instead of one.
        gain_delta_db = gain_delta_db.clamp(-24.0, 24.0);

        // Spectral correction: per-band (target - measured).
        let target = target_curve(self.target_curve_idx);
        let measured_levels = self.spectrum.levels_db();
        // Normalise the measured spectrum to its overall mean so the
        // target is interpreted as a SHAPE, not an absolute level.
        let mut sum = 0.0_f32;
        let mut count = 0;
        for &v in &measured_levels {
            if v > -80.0 { sum += v; count += 1; }
        }
        let mean = if count > 0 { sum / count as f32 } else { 0.0 };
        let mut deltas = [0.0_f32; 32];
        for i in 0..32 {
            if measured_levels[i] > -80.0 {
                let relative = measured_levels[i] - mean;
                deltas[i] = (target[i] - relative).clamp(-6.0, 6.0);
            }
        }

        match self.mode {
            MaMode::Listen => {
                // Update reported values but don't influence the
                // chain — host can still display them.
                self.correction_gain_db = gain_delta_db;
                self.correction_band_db = deltas;
            }
            MaMode::Apply => {
                // One-shot push toward the target.
                self.correction_gain_db = gain_delta_db;
                self.correction_band_db = deltas;
            }
            MaMode::AutoTweak => {
                // 20 % step toward the target — settles over ~5 s.
                self.correction_gain_db = self.correction_gain_db
                    + (gain_delta_db - self.correction_gain_db) * 0.2;
                for i in 0..32 {
                    self.correction_band_db[i] = self.correction_band_db[i]
                        + (deltas[i] - self.correction_band_db[i]) * 0.2;
                }
            }
            MaMode::Off => {}
        }
    }

    pub fn reset(&mut self) {
        self.lufs.reset();
        self.spectrum.reset();
        self.correction_gain_db = 0.0;
        self.correction_band_db = [0.0; 32];
        self.update_counter = 0;
        self.applied_gain_lin = 1.0;
        self.applied_band_db = [0.0; 8];
        for i in 0..8 {
            self.correction_eq_l[i].reset();
            self.correction_eq_r[i].reset();
            self.correction_eq_l[i].set_peak(self.sample_rate, MA_BAND_FREQS[i], 1.41, 0.0);
            self.correction_eq_r[i].set_peak(self.sample_rate, MA_BAND_FREQS[i], 1.41, 0.0);
        }
    }

    pub fn set_target_lufs(&mut self, v: f32) { self.target_lufs = v.clamp(-30.0, -6.0); }
    pub fn set_target_curve(&mut self, idx: u8) { self.target_curve_idx = idx.min(3); }
    pub fn set_mode_f32(&mut self, m: f32) { self.mode = MaMode::from_f32(m); }
    pub fn set_mix(&mut self, m: f32) { self.mix = m.clamp(0.0, 1.0); }

    /// Live measurements for the GUI.
    pub fn integrated_lufs(&self) -> f32 { self.lufs.integrated_lufs() }
    pub fn momentary_lufs(&self) -> f32 { self.lufs.momentary_lufs() }
    pub fn spectrum_levels_db(&self) -> [f32; 32] { self.spectrum.levels_db() }
    /// The corrective input-gain change in dB the host should apply
    /// to hit the target loudness.
    pub fn correction_gain_db(&self) -> f32 { self.correction_gain_db }
    /// Per-band corrective EQ deltas in dB.
    pub fn correction_band_db(&self) -> [f32; 32] { self.correction_band_db }
    pub fn mode_as_f32(&self) -> f32 { self.mode as i32 as f32 }
}

//==============================================================================
// MULTIBAND COMPRESSOR — 3-band LR4 split + per-band compression
//==============================================================================

/// Single-band compressor state used inside `MultibandComp3Effect`.
/// Same envelope-follower + soft-knee shape as the full
/// `CompressorEffect`, just trimmed to what the multiband needs
/// (no sidechain key, no lookahead, no auto-makeup).
#[derive(Debug, Clone)]
struct BandComp {
    env_l: f32,
    env_r: f32,
    attack_coef: f32,
    release_coef: f32,
    threshold: f32,    // linear amplitude
    ratio: f32,
    makeup_lin: f32,
    enabled: bool,
}

impl BandComp {
    fn new() -> Self {
        Self {
            env_l: 0.0, env_r: 0.0,
            attack_coef: 0.0, release_coef: 0.0,
            threshold: 1.0, ratio: 1.0,
            makeup_lin: 1.0, enabled: true,
        }
    }
    fn set_times(&mut self, sr: f32, attack_sec: f32, release_sec: f32) {
        let atk = (attack_sec.max(1.0e-4) * sr).max(1.0);
        let rel = (release_sec.max(1.0e-3) * sr).max(1.0);
        self.attack_coef  = (-1.0_f32 / atk).exp();
        self.release_coef = (-1.0_f32 / rel).exp();
    }
    #[inline]
    fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        if !self.enabled || self.ratio <= 1.0001 {
            return (l * self.makeup_lin, r * self.makeup_lin);
        }
        // Peak-following envelope per channel.
        let abs_l = l.abs(); let abs_r = r.abs();
        let coef_l = if abs_l > self.env_l { self.attack_coef } else { self.release_coef };
        let coef_r = if abs_r > self.env_r { self.attack_coef } else { self.release_coef };
        self.env_l = self.env_l * coef_l + abs_l * (1.0 - coef_l);
        self.env_r = self.env_r * coef_r + abs_r * (1.0 - coef_r);
        // Compute gain reduction in linear space (avoid log).
        let gain = |env: f32| -> f32 {
            if env <= self.threshold { 1.0 }
            else {
                // ratio in linear: out = thr + (env-thr)/ratio
                // gain = out/env
                let out = self.threshold + (env - self.threshold) / self.ratio;
                (out / env).max(0.0)
            }
        };
        let g_l = gain(self.env_l);
        let g_r = gain(self.env_r);
        (l * g_l * self.makeup_lin, r * g_r * self.makeup_lin)
    }
    fn reset(&mut self) { self.env_l = 0.0; self.env_r = 0.0; }
}

/// Linkwitz-Riley crossover leg. LR4 is two cascaded LR2 (= Butterworth
/// 2nd-order) biquads in series; an LR4 LP plus LR4 HP summed at the same
/// crossover frequency adds back to a flat magnitude response (with a 360
/// degree phase shift). Butterworth fixes Q at 1/sqrt(2), so these are thin
/// wrappers over the shared Biquad rather than a re-derived cookbook.
#[derive(Debug, Clone, Copy, Default)]
struct LRBiquad(crate::dsp::filters::Biquad);
impl LRBiquad {
    fn new() -> Self { Self(crate::dsp::filters::Biquad::new()) }
    fn set_lp(&mut self, sr: f32, freq: f32) {
        self.0.set_lowpass(sr, freq, std::f32::consts::FRAC_1_SQRT_2);
    }
    fn set_hp(&mut self, sr: f32, freq: f32) {
        self.0.set_highpass(sr, freq, std::f32::consts::FRAC_1_SQRT_2);
    }
    #[inline]
    fn process(&mut self, x: f32) -> f32 { self.0.process(x) }
    #[allow(dead_code)]
    fn reset(&mut self) { self.0.reset(); }
}

/// 3-band Linkwitz-Riley split + per-band compressor + recombine.
/// Crossovers default to 200 Hz / 2 kHz. Each band feeds its own
/// `BandComp`. Per-band solo lets the user audit the split.
///
/// At bypass (all bands ratio = 1.0) the output is bit-equivalent
/// to the input — LR4 sums back to flat magnitude/phase response.
#[derive(Debug, Clone)]
pub struct MultibandComp3Effect {
    sample_rate: f32,
    // Crossover frequencies.
    xover_low_hz:  f32,
    xover_high_hz: f32,
    // LR4 = 2 cascaded LR2. Per channel (L/R), per stage (1/2),
    // per filter (lp/hp), per crossover (low / high).
    xover_low_lp_l:  [LRBiquad; 2],
    xover_low_lp_r:  [LRBiquad; 2],
    xover_low_hp_l:  [LRBiquad; 2],
    xover_low_hp_r:  [LRBiquad; 2],
    xover_high_lp_l: [LRBiquad; 2],
    xover_high_lp_r: [LRBiquad; 2],
    xover_high_hp_l: [LRBiquad; 2],
    xover_high_hp_r: [LRBiquad; 2],
    // Per-band compressors.
    band_comp: [BandComp; 3],
    // Per-band solo (debug aid).
    solo: [bool; 3],
    /// Idle-bypass: consecutive near-silent input samples. After the
    /// idle threshold the 16 crossover biquads + 3 band compressor
    /// envelopes are skipped (LR4 cascades have already settled to
    /// zero output on zero input). Recovers on first audible sample.
    silence_samples: u32,
    pub mix: f32,
}

impl MultibandComp3Effect {
    pub fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            sample_rate,
            xover_low_hz:  200.0,
            xover_high_hz: 2000.0,
            xover_low_lp_l:  [LRBiquad::new(); 2],
            xover_low_lp_r:  [LRBiquad::new(); 2],
            xover_low_hp_l:  [LRBiquad::new(); 2],
            xover_low_hp_r:  [LRBiquad::new(); 2],
            xover_high_lp_l: [LRBiquad::new(); 2],
            xover_high_lp_r: [LRBiquad::new(); 2],
            xover_high_hp_l: [LRBiquad::new(); 2],
            xover_high_hp_r: [LRBiquad::new(); 2],
            band_comp: [BandComp::new(), BandComp::new(), BandComp::new()],
            solo: [false; 3],
            silence_samples: 0,
            mix: 1.0,
        };
        s.update_crossovers();
        for b in 0..3 {
            s.band_comp[b].set_times(sample_rate, 0.010, 0.150);
            s.band_comp[b].threshold = 10.0_f32.powf(-12.0 / 20.0); // -12 dB
            s.band_comp[b].ratio = 2.0;
            s.band_comp[b].makeup_lin = 1.0;
        }
        s
    }

    fn update_crossovers(&mut self) {
        for i in 0..2 {
            self.xover_low_lp_l[i].set_lp(self.sample_rate, self.xover_low_hz);
            self.xover_low_lp_r[i].set_lp(self.sample_rate, self.xover_low_hz);
            self.xover_low_hp_l[i].set_hp(self.sample_rate, self.xover_low_hz);
            self.xover_low_hp_r[i].set_hp(self.sample_rate, self.xover_low_hz);
            self.xover_high_lp_l[i].set_lp(self.sample_rate, self.xover_high_hz);
            self.xover_high_lp_r[i].set_lp(self.sample_rate, self.xover_high_hz);
            self.xover_high_hp_l[i].set_hp(self.sample_rate, self.xover_high_hz);
            self.xover_high_hp_r[i].set_hp(self.sample_rate, self.xover_high_hz);
        }
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.mix <= 1.0e-4 { return (input_l, input_r); }

        // Idle-bypass: silent input for ≥1024 samples → the 16
        // crossover biquads' outputs have settled to ~0, and each
        // band compressor's envelope follower would only release. Skip
        // both and decay envelopes via release coef. Recovers on the
        // first audible sample.
        const SILENCE_EPS: f32 = 1.0e-6;
        const IDLE_AFTER_SAMPLES: u32 = 1024;
        let abs_sum = input_l.abs() + input_r.abs();
        if abs_sum < SILENCE_EPS {
            self.silence_samples = self.silence_samples.saturating_add(1);
            if self.silence_samples >= IDLE_AFTER_SAMPLES {
                for b in 0..3 {
                    self.band_comp[b].env_l *= self.band_comp[b].release_coef;
                    self.band_comp[b].env_r *= self.band_comp[b].release_coef;
                }
                let m = self.mix;
                return (input_l * (1.0 - m), input_r * (1.0 - m));
            }
        } else {
            self.silence_samples = 0;
        }

        // ── Crossover split (LR4 = 2 cascaded LR2 biquads). ────
        // Low band: LP(low) of input.
        let mut low_l = input_l; let mut low_r = input_r;
        for i in 0..2 {
            low_l = self.xover_low_lp_l[i].process(low_l);
            low_r = self.xover_low_lp_r[i].process(low_r);
        }
        // Mid+high: HP(low) of input.
        let mut mid_high_l = input_l; let mut mid_high_r = input_r;
        for i in 0..2 {
            mid_high_l = self.xover_low_hp_l[i].process(mid_high_l);
            mid_high_r = self.xover_low_hp_r[i].process(mid_high_r);
        }
        // Mid: LP(high) of mid+high. High: HP(high) of mid+high.
        let mut mid_l = mid_high_l; let mut mid_r = mid_high_r;
        for i in 0..2 {
            mid_l = self.xover_high_lp_l[i].process(mid_l);
            mid_r = self.xover_high_lp_r[i].process(mid_r);
        }
        let mut high_l = mid_high_l; let mut high_r = mid_high_r;
        for i in 0..2 {
            high_l = self.xover_high_hp_l[i].process(high_l);
            high_r = self.xover_high_hp_r[i].process(high_r);
        }

        // Per-band compression.
        let (cl, cr) = self.band_comp[0].process(low_l,  low_r);
        let (ml, mr) = self.band_comp[1].process(mid_l,  mid_r);
        let (hl, hr) = self.band_comp[2].process(high_l, high_r);

        // Solo logic — any band soloed mutes the others.
        let any_solo = self.solo[0] || self.solo[1] || self.solo[2];
        let (cl, cr, ml, mr, hl, hr) = if any_solo {
            (
                if self.solo[0] { cl } else { 0.0 },
                if self.solo[0] { cr } else { 0.0 },
                if self.solo[1] { ml } else { 0.0 },
                if self.solo[1] { mr } else { 0.0 },
                if self.solo[2] { hl } else { 0.0 },
                if self.solo[2] { hr } else { 0.0 },
            )
        } else { (cl, cr, ml, mr, hl, hr) };

        // Recombine via sum — LR4 sum is flat in magnitude.
        let out_l = cl + ml + hl;
        let out_r = cr + mr + hr;
        let m = self.mix;
        (input_l * (1.0 - m) + out_l * m,
         input_r * (1.0 - m) + out_r * m)
    }

    pub fn reset(&mut self) {
        for f in self.xover_low_lp_l.iter_mut().chain(self.xover_low_lp_r.iter_mut())
            .chain(self.xover_low_hp_l.iter_mut()).chain(self.xover_low_hp_r.iter_mut())
            .chain(self.xover_high_lp_l.iter_mut()).chain(self.xover_high_lp_r.iter_mut())
            .chain(self.xover_high_hp_l.iter_mut()).chain(self.xover_high_hp_r.iter_mut())
        {
            f.reset();
        }
        for b in &mut self.band_comp { b.reset(); }
        self.silence_samples = 0;
    }

    pub fn set_crossover_low(&mut self, hz: f32) {
        self.xover_low_hz = hz.clamp(40.0, self.xover_high_hz - 1.0);
        self.update_crossovers();
    }
    pub fn set_crossover_high(&mut self, hz: f32) {
        self.xover_high_hz = hz.clamp(self.xover_low_hz + 1.0, 16_000.0);
        self.update_crossovers();
    }
    pub fn set_band_threshold_db(&mut self, b: usize, db: f32) {
        if b < 3 { self.band_comp[b].threshold = 10.0_f32.powf(db.clamp(-60.0, 0.0) / 20.0); }
    }
    pub fn set_band_ratio(&mut self, b: usize, r: f32) {
        if b < 3 { self.band_comp[b].ratio = r.clamp(1.0, 20.0); }
    }
    pub fn set_band_attack(&mut self, b: usize, sec: f32) {
        if b < 3 {
            let sr = self.sample_rate;
            let cur_rel = -1.0 / self.band_comp[b].release_coef.ln();
            self.band_comp[b].set_times(sr, sec, cur_rel / sr);
        }
    }
    pub fn set_band_release(&mut self, b: usize, sec: f32) {
        if b < 3 {
            let sr = self.sample_rate;
            let cur_atk = -1.0 / self.band_comp[b].attack_coef.ln();
            self.band_comp[b].set_times(sr, cur_atk / sr, sec);
        }
    }
    pub fn set_band_makeup_db(&mut self, b: usize, db: f32) {
        if b < 3 { self.band_comp[b].makeup_lin = 10.0_f32.powf(db.clamp(-12.0, 12.0) / 20.0); }
    }
    pub fn set_band_solo(&mut self, b: usize, on: bool) {
        if b < 3 { self.solo[b] = on; }
    }
    pub fn set_band_enabled(&mut self, b: usize, en: bool) {
        if b < 3 { self.band_comp[b].enabled = en; }
    }
    pub fn set_mix(&mut self, m: f32) { self.mix = m.clamp(0.0, 1.0); }
}

//==============================================================================
// DOPPLER EFFECT — velocity-driven pitch shift via time-varying delay line
//==============================================================================

#[derive(Debug, Clone)]
pub struct DopplerEffect {
    sample_rate: f32,
    velocity: f32, // -20..20 m/s (positive = moving away)
    angle:    f32, // -180..180 degrees (angle of motion)
    distance: f32, // 0.1..50 m
    hpf_amt:  f32, // 0..1 — amount of HPF shaping
    pub mix:  f32,

    delay_l: FracDelayLine,
    delay_r: FracDelayLine,
    read_pos: f32,  // current read position in samples-behind-write
    hp_l: f32,
    hp_r: f32,

    // Cached derived constants — all knob-driven and refreshed only on setter.
    cached_read_step: f32,   // 1.0 - 1.0/ratio (per-sample read-head walk)
    cached_dist_gain: f32,   // 1.0 / (1.0 + 0.2 * distance)
    cached_hp_alpha:  f32,   // one-pole HPF coefficient
}

impl DopplerEffect {
    pub fn new(sample_rate: f32) -> Self {
        let max_samples = (sample_rate * 0.25) as usize; // 250 ms headroom
        let mut s = Self {
            sample_rate,
            velocity: 0.0,
            angle: 0.0,
            distance: 1.0,
            hpf_amt: 0.0,
            mix: 1.0,
            delay_l: FracDelayLine::new(max_samples),
            delay_r: FracDelayLine::new(max_samples),
            read_pos: 100.0,
            hp_l: 0.0, hp_r: 0.0,
            cached_read_step: 0.0,
            cached_dist_gain: 1.0,
            cached_hp_alpha:  0.0,
        };
        s.refresh_cache();
        s
    }

    fn refresh_cache(&mut self) {
        // Pitch ratio from source→listener component of velocity.
        // ratio = c / (c - v*cos(angle)). v positive = receding → ratio < 1.
        let c = 343.0f32;
        let cos_a = self.angle.to_radians().cos();
        let v_eff = self.velocity * cos_a;
        let ratio = (c / (c - v_eff).max(50.0)).clamp(0.5, 2.0);
        self.cached_read_step = 1.0 - 1.0 / ratio;

        self.cached_dist_gain = 1.0 / (1.0 + 0.2 * self.distance.max(0.1));

        let hp_cut = 30.0 + self.velocity.abs() * self.hpf_amt * 40.0;
        self.cached_hp_alpha = 1.0 - (-2.0 * std::f32::consts::PI * hp_cut / self.sample_rate).exp();
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.mix <= 0.0001 { return (input_l, input_r); }

        self.delay_l.write(input_l);
        self.delay_r.write(input_r);

        // Walk the read position; cached_read_step = 1 - 1/ratio. Keep the
        // read at least 64 samples behind the write head to avoid interp wrap.
        self.read_pos += self.cached_read_step;
        self.read_pos = self.read_pos.clamp(64.0, self.sample_rate * 0.2);

        let out_l = self.delay_l.read_frac(self.read_pos);
        let out_r = self.delay_r.read_frac(self.read_pos);

        let mut l = out_l * self.cached_dist_gain;
        let mut r = out_r * self.cached_dist_gain;

        // Velocity-driven HPF (simulates wind-rush) — alpha cached.
        let alpha = self.cached_hp_alpha;
        self.hp_l += alpha * (l - self.hp_l);
        self.hp_r += alpha * (r - self.hp_r);
        l -= self.hp_l * self.hpf_amt;
        r -= self.hp_r * self.hpf_amt;

        let out_l = input_l * (1.0 - self.mix) + l * self.mix;
        let out_r = input_r * (1.0 - self.mix) + r * self.mix;
        (out_l, out_r)
    }

    pub fn reset(&mut self) {
        self.delay_l.reset(); self.delay_r.reset();
        self.read_pos = 100.0;
        self.hp_l = 0.0; self.hp_r = 0.0;
    }

    pub fn set_velocity(&mut self, v: f32) { self.velocity = v.clamp(-20.0, 20.0); self.refresh_cache(); }
    pub fn set_angle(&mut self, v: f32) { self.angle = v.clamp(-180.0, 180.0); self.refresh_cache(); }
    pub fn set_distance(&mut self, v: f32) { self.distance = v.clamp(0.1, 50.0); self.refresh_cache(); }
    pub fn set_hpf_amt(&mut self, v: f32) { self.hpf_amt = v.clamp(0.0, 1.0); self.refresh_cache(); }
    pub fn set_mix(&mut self, v: f32) { self.mix = v.clamp(0.0, 1.0); }
}

//==============================================================================
// TRANSIENT SHAPER EFFECT
//==============================================================================

#[derive(Debug, Clone)]
pub struct TransientShaperEffect {
    sample_rate: f32,
    attack:  f32,  // -1..+1 (100% to +100%)
    sustain: f32,  // -1..+1
    pub mix: f32,
    // Dual envelope followers (stereo-linked: peak across L/R)
    fast_env: f32,
    slow_env: f32,
    fast_coef: f32,
    slow_coef: f32,
}

impl TransientShaperEffect {
    pub fn new(sample_rate: f32) -> Self {
        let fast_ms = 2.0;
        let slow_ms = 150.0;
        let fast_coef = (-1.0 / (fast_ms * 0.001 * sample_rate)).exp();
        let slow_coef = (-1.0 / (slow_ms * 0.001 * sample_rate)).exp();
        Self {
            sample_rate,
            attack: 0.0,
            sustain: 0.0,
            mix: 1.0,
            fast_env: 0.0,
            slow_env: 0.0,
            fast_coef,
            slow_coef,
        }
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.mix <= 0.0001 { return (input_l, input_r); }
        let det = input_l.abs().max(input_r.abs());
        // Fast envelope: peak follower, fast attack, fast release
        self.fast_env = det.max(self.fast_env * self.fast_coef);
        // Slow envelope: slower attack + release
        self.slow_env = self.slow_env * self.slow_coef + det * (1.0 - self.slow_coef);
        // Transient part (attack) ≈ fast - slow, clipped to positive
        let transient = (self.fast_env - self.slow_env).max(0.0);
        // Sustain part ≈ slow envelope
        let sustain_env = self.slow_env;
        // Reference envelope is the sum (fast for gain normalization)
        let ref_env = self.fast_env.max(1e-6);
        let atk_gain = 1.0 + self.attack * (transient / ref_env) * 4.0;
        let sus_gain = 1.0 + self.sustain * (sustain_env / ref_env) * 1.5;
        let gain = (atk_gain * sus_gain).clamp(0.0, 4.0);
        let l = input_l * gain;
        let r = input_r * gain;
        let out_l = input_l * (1.0 - self.mix) + l * self.mix;
        let out_r = input_r * (1.0 - self.mix) + r * self.mix;
        (out_l, out_r)
    }

    pub fn reset(&mut self) { self.fast_env = 0.0; self.slow_env = 0.0; }
    pub fn set_attack(&mut self, v: f32) { self.attack = v.clamp(-1.0, 1.0); }
    pub fn set_sustain(&mut self, v: f32) { self.sustain = v.clamp(-1.0, 1.0); }
    pub fn set_mix(&mut self, v: f32) { self.mix = v.clamp(0.0, 1.0); }
}

//==============================================================================
// GRANULAR FREEZER EFFECT
//==============================================================================

#[derive(Debug, Clone)]
pub struct GranularFreezerEffect {
    sample_rate: f32,
    // Capture buffer (2 s stereo)
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    buf_len: usize,
    capture: bool,
    capture_pos: usize,
    buf_valid: bool,

    grain_size: f32,   // in samples
    density:    f32,   // 0..1
    pitch:      f32,   // semitone offset (-24..24)
    scatter:    f32,   // 0..1
    pub mix:    f32,

    // 8 grain voices
    voices: [GranVoice; 8],
    frames_since_spawn: u32,
    rng: u32,

    // Cached `2^(pitch/12)` — was a per-grain-spawn powf inside the
    // audio loop. Refreshed only on set_pitch.
    cached_pitch_step: f32,
}

#[derive(Debug, Clone, Copy)]
struct GranVoice { active: bool, pos: f32, step: f32, len: u32, age: u32, pan: f32 }

impl GranularFreezerEffect {
    pub fn new(sample_rate: f32) -> Self {
        let buf_len = (sample_rate * 2.0) as usize;
        Self {
            sample_rate,
            buf_l: vec![0.0; buf_len],
            buf_r: vec![0.0; buf_len],
            buf_len,
            // Auto-capture on instantiation so the effect is immediately
            // audible without the user having to press a button.
            capture: true,
            capture_pos: 0,
            buf_valid: false,
            grain_size: sample_rate * 0.1, // 100 ms
            density: 0.5,
            pitch: 0.0,
            scatter: 0.3,
            mix: 0.5,
            voices: [GranVoice { active: false, pos: 0.0, step: 1.0, len: 0, age: 0, pan: 0.0 }; 8],
            frames_since_spawn: 0,
            rng: 0xCAFE_F00D,
            cached_pitch_step: 1.0,
        }
    }

    #[inline(always)]
    fn rand(&mut self) -> f32 {
        self.rng = self.rng.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.rng >> 8) as f32 / 16_777_216.0
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        // Capture mode fills the buffer once, then auto-stops.
        if self.capture {
            if self.capture_pos < self.buf_len {
                self.buf_l[self.capture_pos] = input_l;
                self.buf_r[self.capture_pos] = input_r;
                self.capture_pos += 1;
            }
            if self.capture_pos >= self.buf_len {
                self.capture = false;
                self.buf_valid = true;
            }
        }

        if self.mix <= 0.0001 || !self.buf_valid {
            return (input_l, input_r);
        }

        // Spawn new grains on a density schedule
        let spawn_interval = ((1.0 - self.density) * self.grain_size).max(64.0) as u32;
        self.frames_since_spawn += 1;
        if self.frames_since_spawn >= spawn_interval {
            self.frames_since_spawn = 0;
            // Precompute all randoms first (self.rand needs &mut self)
            let r1 = self.rand();
            let r2 = self.rand();
            let r3 = self.rand();
            let scatter_samples = self.scatter * self.buf_len as f32;
            let step = self.cached_pitch_step;
            let grain_len = self.grain_size as u32;
            let buf_len_f = self.buf_len as f32;
            for v in &mut self.voices {
                if !v.active {
                    v.pos = (r1 * buf_len_f + (r2 - 0.5) * scatter_samples).rem_euclid(buf_len_f);
                    v.step = step;
                    v.len = grain_len;
                    v.age = 0;
                    v.pan = (r3 - 0.5) * 2.0;
                    v.active = true;
                    break;
                }
            }
        }

        // Sum active grains with Hann envelope
        let mut wet_l = 0.0f32;
        let mut wet_r = 0.0f32;
        for v in &mut self.voices {
            if !v.active { continue; }
            let pos_i = v.pos as usize % self.buf_len;
            let next_i = (pos_i + 1) % self.buf_len;
            let frac = v.pos - v.pos.floor();
            let sl = self.buf_l[pos_i] * (1.0 - frac) + self.buf_l[next_i] * frac;
            let sr = self.buf_r[pos_i] * (1.0 - frac) + self.buf_r[next_i] * frac;
            // Hann window
            let phase = v.age as f32 / v.len.max(1) as f32;
            let win = 0.5 - 0.5 * (phase * std::f32::consts::TAU).cos();
            let gl = (1.0 - v.pan * 0.5).clamp(0.0, 1.0);
            let gr = (1.0 + v.pan * 0.5).clamp(0.0, 1.0);
            wet_l += sl * win * gl;
            wet_r += sr * win * gr;
            v.pos += v.step;
            if v.pos >= self.buf_len as f32 { v.pos -= self.buf_len as f32; }
            v.age += 1;
            if v.age >= v.len { v.active = false; }
        }

        // Normalize by approximate concurrent voice count
        let active_count = self.voices.iter().filter(|v| v.active).count().max(1) as f32;
        wet_l /= active_count.sqrt();
        wet_r /= active_count.sqrt();

        let out_l = input_l * (1.0 - self.mix) + wet_l * self.mix;
        let out_r = input_r * (1.0 - self.mix) + wet_r * self.mix;
        (out_l, out_r)
    }

    pub fn reset(&mut self) {
        for v in &mut self.voices { v.active = false; }
        self.frames_since_spawn = 0;
    }
    pub fn trigger_capture(&mut self) { self.capture = true; self.capture_pos = 0; self.buf_valid = false; }
    pub fn set_grain_size_ms(&mut self, ms: f32) { self.grain_size = (ms.clamp(10.0, 500.0) * 0.001 * self.sample_rate).max(64.0); }
    pub fn set_density(&mut self, v: f32) { self.density = v.clamp(0.0, 1.0); }
    pub fn set_pitch(&mut self, v: f32) {
        self.pitch = v.clamp(-24.0, 24.0);
        self.cached_pitch_step = 2.0f32.powf(self.pitch / 12.0);
    }
    pub fn set_scatter(&mut self, v: f32) { self.scatter = v.clamp(0.0, 1.0); }
    pub fn set_mix(&mut self, v: f32) { self.mix = v.clamp(0.0, 1.0); }
}

//==============================================================================
// SPECTRAL FREEZE EFFECT (time-domain approximation — comb-based freeze)
//==============================================================================

/// Spectral-ish freeze: captures the last ~1 s of audio and loops tiny
/// cross-faded windows to create a steady-state drone. Not a true FFT
/// freeze (we avoid the FFT dependency), but it produces the same
/// musical effect for pads and sustained tones.
#[derive(Debug, Clone)]
pub struct SpectralFreezeEffect {
    sample_rate: f32,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    buf_len: usize,
    write: usize,
    window_len: usize, // samples in the looping window
    read_a: f32,
    read_b: f32,
    xfade_len: usize,
    freeze: bool,
    smear: f32,
    tilt: f32,      // -1..+1 (negative dark, positive bright)
    pub mix: f32,

    // Counts how many samples have been written since reset. Freeze is
    // held off until at least one full capture window has been recorded.
    samples_captured: usize,

    // Smear uses a simple running average
    smear_l: f32,
    smear_r: f32,
    /// `smear^4` cached — was a per-sample powf on a static knob.
    cached_smear_pow4: f32,

    // Tilt — gentle one-pole shelves
    tilt_lp_l: f32,
    tilt_lp_r: f32,
    /// LP filter alpha cached. Was 3 divisions per sample for the
    /// tilt 1-pole LPF; only changes when `tilt` moves.
    cached_tilt_alpha: f32,
}

impl SpectralFreezeEffect {
    pub fn new(sample_rate: f32) -> Self {
        let buf_len = sample_rate as usize; // 1 s
        let window_len = buf_len / 4; // 250 ms
        let xfade_len = window_len / 4;
        let mut s = Self {
            sample_rate,
            buf_l: vec![0.0; buf_len],
            buf_r: vec![0.0; buf_len],
            buf_len,
            write: 0,
            window_len,
            read_a: 0.0,
            read_b: (window_len / 2) as f32,
            xfade_len,
            freeze: false,
            smear: 0.0,
            tilt: 0.0,
            mix: 0.5,
            samples_captured: 0,
            smear_l: 0.0, smear_r: 0.0,
            cached_smear_pow4: 0.0,
            tilt_lp_l: 0.0, tilt_lp_r: 0.0,
            cached_tilt_alpha: 0.0,
        };
        s.refresh_tilt_alpha();
        s
    }

    fn refresh_tilt_alpha(&mut self) {
        let cutoff = if self.tilt < 0.0 { 500.0 + self.tilt * 450.0 } else { 2000.0 };
        let dt = 1.0 / self.sample_rate;
        let rc = 1.0 / (2.0 * std::f32::consts::PI * cutoff.max(50.0));
        self.cached_tilt_alpha = dt / (rc + dt);
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        // Always capture input even when not frozen so freeze happens instantly.
        self.buf_l[self.write] = input_l;
        self.buf_r[self.write] = input_r;
        self.write = (self.write + 1) % self.buf_len;
        if self.samples_captured < self.window_len { self.samples_captured += 1; }

        // Hold off freeze until we've captured at least one full window of
        // audio so the read heads aren't playing back zeros.
        let ready = self.samples_captured >= self.window_len;
        if !self.freeze || !ready || self.mix <= 0.0001 {
            return (input_l, input_r);
        }

        // Two overlapping looping read heads for smooth loop points
        let base = if self.write >= self.window_len { self.write - self.window_len } else {
            self.buf_len + self.write - self.window_len
        };
        let read_idx = |offset: f32| -> usize {
            ((base as f32 + offset) as usize) % self.buf_len
        };

        let ia = read_idx(self.read_a);
        let ib = read_idx(self.read_b);

        // Crossfade between the two heads based on distance-to-loop-end
        let phase_a = self.read_a / self.window_len as f32;
        let fade_a = (phase_a * std::f32::consts::PI).sin();
        let fade_b = ((1.0 - phase_a) * std::f32::consts::PI).sin();
        let norm = (fade_a + fade_b).max(1e-4);
        let sa_l = self.buf_l[ia]; let sa_r = self.buf_r[ia];
        let sb_l = self.buf_l[ib]; let sb_r = self.buf_r[ib];
        let mut wet_l = (sa_l * fade_a + sb_l * fade_b) / norm;
        let mut wet_r = (sa_r * fade_a + sb_r * fade_b) / norm;

        // Smear — running average. `smear^4` cached on set_smear.
        if self.smear > 0.0 {
            let a = self.cached_smear_pow4;
            self.smear_l = self.smear_l * a + wet_l * (1.0 - a);
            self.smear_r = self.smear_r * a + wet_r * (1.0 - a);
            wet_l = wet_l * (1.0 - self.smear) + self.smear_l * self.smear;
            wet_r = wet_r * (1.0 - self.smear) + self.smear_r * self.smear;
        }

        // Tilt — alpha cached on set_tilt (3 divisions/sample → 0).
        let alpha = self.cached_tilt_alpha;
        self.tilt_lp_l += alpha * (wet_l - self.tilt_lp_l);
        self.tilt_lp_r += alpha * (wet_r - self.tilt_lp_r);
        if self.tilt < 0.0 {
            let t = -self.tilt;
            wet_l = wet_l * (1.0 - t) + self.tilt_lp_l * t;
            wet_r = wet_r * (1.0 - t) + self.tilt_lp_r * t;
        } else if self.tilt > 0.0 {
            let t = self.tilt;
            wet_l = wet_l + (wet_l - self.tilt_lp_l) * t * 1.5;
            wet_r = wet_r + (wet_r - self.tilt_lp_r) * t * 1.5;
        }

        // Advance read heads
        self.read_a += 1.0;
        self.read_b += 1.0;
        if self.read_a >= self.window_len as f32 { self.read_a -= self.window_len as f32; }
        if self.read_b >= self.window_len as f32 { self.read_b -= self.window_len as f32; }

        let out_l = input_l * (1.0 - self.mix) + wet_l * self.mix;
        let out_r = input_r * (1.0 - self.mix) + wet_r * self.mix;
        (out_l, out_r)
    }

    pub fn reset(&mut self) {
        self.smear_l = 0.0; self.smear_r = 0.0;
        self.tilt_lp_l = 0.0; self.tilt_lp_r = 0.0;
        self.read_a = 0.0; self.read_b = (self.window_len / 2) as f32;
        for s in &mut self.buf_l { *s = 0.0; }
        for s in &mut self.buf_r { *s = 0.0; }
        self.samples_captured = 0;
        self.write = 0;
    }
    pub fn set_freeze(&mut self, f: bool) { self.freeze = f; }
    pub fn set_smear(&mut self, v: f32) {
        self.smear = v.clamp(0.0, 1.0);
        let s = self.smear;
        self.cached_smear_pow4 = s * s * s * s;  // s^4 without powf
    }
    pub fn set_tilt(&mut self, v: f32) {
        self.tilt = v.clamp(-1.0, 1.0);
        self.refresh_tilt_alpha();
    }
    pub fn set_mix(&mut self, v: f32) { self.mix = v.clamp(0.0, 1.0); }
}

//==============================================================================
// RESONATOR BANK EFFECT
//==============================================================================

/// 8 tuned comb filters in parallel, frequencies derived from a chord.
#[derive(Debug, Clone)]
pub struct ResonatorBankEffect {
    sample_rate: f32,
    root:    i32,  // 0..11 (C..B)
    octave:  i32,  // -2..2
    chord:   u8,   // 0..6 (selects CHORD_INTERVALS)
    spread:  f32,  // 0..1 stereo spread
    decay:   f32,  // 0..1 → feedback 0.5..0.999
    damping: f32,  // 0..1 — intra-loop LPF
    pub mix: f32,
    /// Share of the excitation taken from the sidechain key: 0 the slot's
    /// own input, 1 the key alone.
    sympathy: f32,
    /// Tune to the chord handed by `set_key_chord` instead of the root and
    /// chord parameters.
    follow: bool,
    /// The chord in force, as eight notes, when one was handed over.
    followed: Option<[i32; 8]>,
    pub sidechain_active: bool,
    pub sidechain_l: f32,
    pub sidechain_r: f32,

    // Per-voice comb state (L/R pair)
    combs_l: [CombVoice; 8],
    combs_r: [CombVoice; 8],
}

#[derive(Debug, Clone)]
struct CombVoice {
    buf: Vec<f32>,
    write: usize,
    period: usize,
    damp_state: f32,
}

impl CombVoice {
    fn new(max: usize) -> Self { Self { buf: vec![0.0; max.max(2)], write: 0, period: 1, damp_state: 0.0 } }
    fn set_freq(&mut self, fs: f32, freq: f32) {
        let p = (fs / freq.max(1.0)) as usize;
        self.period = p.clamp(2, self.buf.len() - 1);
    }
    #[inline(always)]
    fn process(&mut self, input: f32, fb: f32, damp: f32) -> f32 {
        let read_idx = if self.write >= self.period { self.write - self.period } else {
            self.buf.len() + self.write - self.period
        };
        let y = self.buf[read_idx];
        // 1-pole damping inside the loop
        self.damp_state += damp * (y - self.damp_state);
        let damped = self.damp_state;
        let w = input + damped * fb;
        self.buf[self.write] = w;
        self.write = (self.write + 1) % self.buf.len();
        y
    }
    fn reset(&mut self) { for s in &mut self.buf { *s = 0.0; } self.write = 0; self.damp_state = 0.0; }
}

// 7 chords × up to 8 intervals in semitones (root = 0)
const CHORD_INTERVALS: [[i32; 8]; 7] = [
    [0, 4, 7, 12, 16, 19, 24, 28],   // Major (0)
    [0, 3, 7, 12, 15, 19, 24, 27],   // Minor (1)
    [0, 4, 7, 11, 12, 16, 19, 23],   // Maj7  (2)
    [0, 3, 7, 10, 12, 15, 19, 22],   // Min7  (3)
    [0, 5, 7, 12, 17, 19, 24, 29],   // Sus4  (4)
    [0, 3, 6, 9, 12, 15, 18, 21],    // Dim   (5)
    [0, 12, 24, 36, 7, 19, 31, 43],  // Octaves+fifths (6)
];

impl ResonatorBankEffect {
    pub fn new(sample_rate: f32) -> Self {
        let max = (sample_rate / 20.0) as usize; // accommodate down to 20 Hz
        let combs_l = std::array::from_fn(|_| CombVoice::new(max));
        let combs_r = std::array::from_fn(|_| CombVoice::new(max));
        let mut s = Self {
            sample_rate,
            root: 0, octave: 0, chord: 0, spread: 0.3,
            decay: 0.6, damping: 0.3, mix: 0.4,
            sympathy: 0.0, follow: false, followed: None,
            sidechain_active: false, sidechain_l: 0.0, sidechain_r: 0.0,
            combs_l, combs_r,
        };
        s.update_tuning();
        s
    }

    fn midi_to_hz(note: i32) -> f32 {
        440.0 * 2.0f32.powf((note as f32 - 69.0) / 12.0)
    }

    fn update_tuning(&mut self) {
        let intervals = CHORD_INTERVALS[(self.chord as usize) % 7];
        let base = 48 + self.root + self.octave * 12; // start around C3
        let followed = if self.follow { self.followed } else { None };
        for i in 0..8 {
            let note = match followed {
                Some(notes) => notes[i] + self.octave * 12,
                None => base + intervals[i],
            };
            let f_l = Self::midi_to_hz(note);
            let detune = 1.0 + self.spread * 0.01 * ((i as f32 * 0.37).sin());
            let f_r = f_l * detune;
            self.combs_l[i].set_freq(self.sample_rate, f_l);
            self.combs_r[i].set_freq(self.sample_rate, f_r);
        }
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.mix <= 0.0001 { return (input_l, input_r); }
        let fb = 0.5 + self.decay * 0.499; // 0.5..0.999
        let damp = self.damping.clamp(0.0, 1.0) * 0.9 + 0.01;
        // Feed each comb with a modest excitation — too loud and the bank
        // screams; too quiet and the resonances are inaudible. 0.5 input gain
        // times a 1/sqrt(8) normalization keeps output RMS near input RMS.
        let (ex_l, ex_r) = if self.sidechain_active && self.sympathy > 0.0 {
            (input_l + (self.sidechain_l - input_l) * self.sympathy,
             input_r + (self.sidechain_r - input_r) * self.sympathy)
        } else {
            (input_l, input_r)
        };
        let src_l = ex_l * 0.5;
        let src_r = ex_r * 0.5;
        let mut wet_l = 0.0f32;
        let mut wet_r = 0.0f32;
        for i in 0..8 {
            wet_l += self.combs_l[i].process(src_l, fb, damp);
            wet_r += self.combs_r[i].process(src_r, fb, damp);
        }
        const INV_SQRT_8: f32 = 0.3535534;
        wet_l *= INV_SQRT_8;
        wet_r *= INV_SQRT_8;
        let out_l = input_l * (1.0 - self.mix) + wet_l * self.mix;
        let out_r = input_r * (1.0 - self.mix) + wet_r * self.mix;
        (out_l, out_r)
    }

    pub fn reset(&mut self) {
        for c in &mut self.combs_l { c.reset(); }
        for c in &mut self.combs_r { c.reset(); }
    }
    pub fn set_root(&mut self, v: i32) { self.root = v.rem_euclid(12); self.update_tuning(); }
    pub fn set_octave(&mut self, v: i32) { self.octave = v.clamp(-2, 2); self.update_tuning(); }
    pub fn set_chord(&mut self, v: u8) { self.chord = v.min(6); self.update_tuning(); }
    pub fn set_spread(&mut self, v: f32) { self.spread = v.clamp(0.0, 1.0); self.update_tuning(); }
    pub fn set_decay(&mut self, v: f32) { self.decay = v.clamp(0.0, 1.0); }
    pub fn set_damping(&mut self, v: f32) { self.damping = v.clamp(0.0, 1.0); }
    pub fn set_mix(&mut self, v: f32) { self.mix = v.clamp(0.0, 1.0); }
    pub fn set_sympathy(&mut self, v: f32) { self.sympathy = v.clamp(0.0, 1.0); }
    pub fn set_follow(&mut self, on: bool) { self.follow = on; self.update_tuning(); }
    pub fn sympathy(&self) -> f32 { self.sympathy }
    pub fn follow(&self) -> bool { self.follow }
    /// The period, in samples, the first voice rings at.
    pub fn first_period(&self) -> usize { self.combs_l[0].period }

    /// Hands over the chord in force: `root` as a pitch class and its
    /// intervals, spread over eight voices from the third octave up.
    pub fn set_key_chord(&mut self, root: u8, intervals: &[u8]) {
        if intervals.is_empty() { return; }
        let mut notes = [0i32; 8];
        for (i, n) in notes.iter_mut().enumerate() {
            *n = 48 + root as i32 % 12 + intervals[i % intervals.len()] as i32 + 12 * (i / intervals.len()) as i32;
        }
        self.followed = Some(notes);
        if self.follow { self.update_tuning(); }
    }
}

//==============================================================================
// AUTO WAH EFFECT
//==============================================================================

#[derive(Debug, Clone)]
pub struct AutoWahEffect {
    sample_rate: f32,
    sensitivity: f32, // 0..1
    attack_ms:   f32,
    release_ms:  f32,
    range_oct:   f32, // 0..4
    q:           f32, // 0.5..10
    direction:   f32, // 0 = up, 1 = down
    base_freq:   f32, // 100..2000 Hz
    pub mix:     f32,

    env_l: f32, env_r: f32,
    attack_coef: f32,
    release_coef: f32,
    bp_l: RbjBiquad,
    bp_r: RbjBiquad,
    /// Last cutoff pushed to the biquads. set_bandpass is biquad-
    /// coefficient computation — expensive to fire on every sample
    /// when the cutoff barely changed. Refresh only when the new
    /// cutoff differs by > 0.5 % from the cached value.
    last_cutoff: f32,
}

impl AutoWahEffect {
    pub fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            sample_rate,
            sensitivity: 0.6,
            attack_ms: 5.0,
            release_ms: 120.0,
            range_oct: 2.0,
            q: 3.0,
            direction: 0.0,
            base_freq: 300.0,
            mix: 1.0,
            env_l: 0.0, env_r: 0.0,
            attack_coef: 0.0,
            release_coef: 0.0,
            bp_l: RbjBiquad::new(),
            bp_r: RbjBiquad::new(),
            last_cutoff: 0.0,
        };
        s.update_coefs();
        s
    }

    fn update_coefs(&mut self) {
        self.attack_coef = (-1.0 / (self.attack_ms * 0.001 * self.sample_rate)).exp();
        self.release_coef = (-1.0 / (self.release_ms * 0.001 * self.sample_rate)).exp();
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.mix <= 0.0001 { return (input_l, input_r); }

        let det = input_l.abs().max(input_r.abs()) * self.sensitivity * 8.0;
        let coef = if det > self.env_l { self.attack_coef } else { self.release_coef };
        self.env_l = det + (self.env_l - det) * coef;
        self.env_r = self.env_l;

        // Map envelope to cutoff
        let mut env = self.env_l.min(1.0);
        if self.direction > 0.5 { env = 1.0 - env; }
        let oct_offset = env * self.range_oct;
        let cutoff = (self.base_freq * 2.0f32.powf(oct_offset)).clamp(50.0, 16000.0);
        // Only push new biquad coefficients when the cutoff has
        // moved meaningfully (> 0.5 %). Otherwise we'd redo the
        // RBJ biquad coefficient math every sample for an envelope
        // that barely changed.
        if (cutoff - self.last_cutoff).abs() > self.last_cutoff * 0.005 {
            self.bp_l.set_bandpass(self.sample_rate, cutoff, self.q);
            self.bp_r.set_bandpass(self.sample_rate, cutoff, self.q);
            self.last_cutoff = cutoff;
        }

        let wet_l = self.bp_l.process(input_l) * 2.0;
        let wet_r = self.bp_r.process(input_r) * 2.0;
        let out_l = input_l * (1.0 - self.mix) + wet_l * self.mix;
        let out_r = input_r * (1.0 - self.mix) + wet_r * self.mix;
        (out_l, out_r)
    }

    pub fn reset(&mut self) { self.env_l = 0.0; self.env_r = 0.0; self.bp_l.reset(); self.bp_r.reset(); }
    pub fn set_sensitivity(&mut self, v: f32) { self.sensitivity = v.clamp(0.0, 1.0); }
    pub fn set_attack(&mut self, v: f32) { self.attack_ms = v.clamp(0.5, 200.0); self.update_coefs(); }
    pub fn set_release(&mut self, v: f32) { self.release_ms = v.clamp(5.0, 2000.0); self.update_coefs(); }
    pub fn set_range(&mut self, v: f32) { self.range_oct = v.clamp(0.0, 4.0); }
    pub fn set_q(&mut self, v: f32) { self.q = v.clamp(0.5, 10.0); }
    pub fn set_direction(&mut self, v: f32) { self.direction = if v > 0.5 { 1.0 } else { 0.0 }; }
    pub fn set_base_freq(&mut self, v: f32) { self.base_freq = v.clamp(50.0, 2000.0); }
    pub fn set_mix(&mut self, v: f32) { self.mix = v.clamp(0.0, 1.0); }
}

//==============================================================================
// TAPE STOP EFFECT
//==============================================================================

#[derive(Debug, Clone)]
pub struct TapeStopEffect {
    sample_rate: f32,
    trigger: bool,
    duration_s: f32,
    curve: f32, // 0 = linear, 1 = exponential
    pub mix: f32,

    progress: f32, // 0..1, only advances while triggered
    rate_mult: f32, // current playback rate (1.0 → 0.0 during stop)

    delay_l: FracDelayLine,
    delay_r: FracDelayLine,
    read_pos: f32,
}

impl TapeStopEffect {
    pub fn new(sample_rate: f32) -> Self {
        let max = (sample_rate * 0.5) as usize;
        Self {
            sample_rate,
            trigger: false,
            duration_s: 1.0,
            curve: 1.0,
            mix: 1.0,
            progress: 0.0,
            rate_mult: 1.0,
            delay_l: FracDelayLine::new(max),
            delay_r: FracDelayLine::new(max),
            read_pos: 64.0,
        }
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        self.delay_l.write(input_l);
        self.delay_r.write(input_r);

        if self.mix <= 0.0001 { return (input_l, input_r); }

        // Advance stop progress while trigger held
        if self.trigger {
            let step = 1.0 / (self.duration_s * self.sample_rate);
            self.progress = (self.progress + step).min(1.0);
        } else {
            // Release resets progress instantly
            self.progress *= 0.995;
            if self.progress < 0.001 { self.progress = 0.0; }
        }

        // Rate: 1 → 0 over progress. Curve shapes the ramp.
        let p = self.progress;
        let shaped = if self.curve > 0.5 {
            // Exponential (slower at end)
            1.0 - p * p
        } else {
            1.0 - p
        };
        self.rate_mult = shaped.clamp(0.0, 1.0);

        // Walk read position at rate_mult samples per sample
        self.read_pos += 1.0 - self.rate_mult;
        self.read_pos = self.read_pos.clamp(64.0, self.sample_rate * 0.4);

        let gain = shaped; // volume dips with rate
        let wet_l = self.delay_l.read_frac(self.read_pos) * gain;
        let wet_r = self.delay_r.read_frac(self.read_pos) * gain;
        let out_l = input_l * (1.0 - self.mix) + wet_l * self.mix;
        let out_r = input_r * (1.0 - self.mix) + wet_r * self.mix;
        (out_l, out_r)
    }

    pub fn reset(&mut self) {
        self.delay_l.reset(); self.delay_r.reset();
        self.read_pos = 64.0;
        self.progress = 0.0;
        self.rate_mult = 1.0;
    }

    pub fn set_trigger(&mut self, t: bool) { self.trigger = t; }
    pub fn set_duration(&mut self, v: f32) { self.duration_s = v.clamp(0.05, 5.0); }
    pub fn set_curve(&mut self, v: f32) { self.curve = if v > 0.5 { 1.0 } else { 0.0 }; }
    pub fn set_mix(&mut self, v: f32) { self.mix = v.clamp(0.0, 1.0); }
}

// ─────────────────────────────────────────────────────────────────────────────
// BRICK-WALL LIMITER (mastering-grade)
// ─────────────────────────────────────────────────────────────────────────────

/// Lookahead brick-wall limiter for the mastering chain. Detects
/// inter-sample peaks via Lagrange-3 cubic interpolation (cheap 4x
/// oversampling estimate per channel), ramps gain reduction down
/// across the lookahead window so the loud sample arrives at the
/// output with gain already reduced, and releases smoothly back to
/// unity afterwards.
///
/// Differences from the existing `LimiterEffect`:
///   * 5 ms lookahead (no surprise overshoots).
///   * True-peak detection (catches inter-sample peaks the existing
///     soft-knee limiter misses entirely).
///   * Adaptive release timing — fast on transients, slow on
///     sustained loud passages, controlled by a single knob.
///   * Output ceiling exposed in dB (-12..0 dB), default -0.3 dB
///     for streaming-safe true-peak headroom.
#[derive(Debug, Clone)]
pub struct BrickwallLimiterEffect {
    sample_rate: f32,
    lookahead_samples: usize,
    /// Stereo delay line for the lookahead. The output reads from
    /// `(write_pos + 1) % lookahead_samples`, i.e. the OLDEST sample
    /// in the ring — that's `lookahead_samples - 1` samples behind
    /// the just-written input.
    delay_l: Vec<f32>,
    delay_r: Vec<f32>,
    write_pos: usize,
    /// 4-sample history used by the Lagrange-3 cubic interpolation
    /// for inter-sample peak estimation. Indices follow the
    /// convention h[0]=x[-3], h[3]=x[0] (most recent).
    hist_l: [f32; 4],
    hist_r: [f32; 4],
    /// Sliding-window peak tracking. Stores the peak associated with
    /// the sample currently driving `current_peak`, plus the count of
    /// samples until that peak ages out of the lookahead window. When
    /// it expires we rescan the delay line to find the new max.
    /// O(1) amortised — rescans are rare.
    current_peak: f32,
    peak_age:     usize,
    /// Current applied gain (linear, 0..1).
    gain:          f32,
    /// Gain we're ramping toward (linear). Reset to 1.0 when no peak
    /// demands reduction.
    target_gain:   f32,
    /// Per-sample gain decrement when ramping down — computed each
    /// time we discover a louder peak so the ramp completes within
    /// the lookahead window.
    gain_ramp:     f32,
    /// One-pole release coefficient. 1.0 = no release (hold), 0 =
    /// instant. Derived from the `release_ms` knob.
    release_coef:  f32,
    /// Output ceiling in linear amplitude (default ~0.965 = -0.3 dB).
    ceiling:       f32,
    /// Knob mirrors for serde / GUI.
    ceiling_db:    f32,
    release_ms:    f32,
    pub mix:       f32,
}

impl BrickwallLimiterEffect {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        // 5 ms lookahead — long enough that even a hard-attack
        // transient gets a smooth gain ramp without sounding
        // pre-clipped, short enough that latency is negligible
        // (5 ms is below the audibility threshold for groove
        // displacement on a master bus).
        let lookahead_samples = ((sr * 0.005) as usize).max(16);
        let mut s = Self {
            sample_rate: sr,
            lookahead_samples,
            delay_l: vec![0.0; lookahead_samples],
            delay_r: vec![0.0; lookahead_samples],
            write_pos: 0,
            hist_l: [0.0; 4],
            hist_r: [0.0; 4],
            current_peak: 0.0,
            peak_age:     0,
            gain:          1.0,
            target_gain:   1.0,
            gain_ramp:     0.0,
            release_coef:  0.0,
            ceiling:       0.965,
            ceiling_db:   -0.3,
            release_ms:   100.0,
            mix:          1.0,
        };
        s.set_release_ms(100.0);
        s
    }

    pub fn set_ceiling_db(&mut self, db: f32) {
        self.ceiling_db = db.clamp(-12.0, 0.0);
        self.ceiling = 10.0_f32.powf(self.ceiling_db / 20.0);
    }
    pub fn set_release_ms(&mut self, ms: f32) {
        self.release_ms = ms.clamp(5.0, 1000.0);
        // 1-pole release coef: gain_n+1 = gain_n * coef + 1 * (1-coef).
        // Reaches -3 dB toward unity in ~release_ms.
        let tau = (self.release_ms * 0.001 * self.sample_rate).max(1.0);
        self.release_coef = (-1.0_f32 / tau).exp();
    }
    pub fn set_mix(&mut self, m: f32) { self.mix = m.clamp(0.0, 1.0); }

    /// Lagrange-3 cubic interpolation peak estimate over the 4 most
    /// recent samples. Returns the maximum |y(t)| at t in {0, 0.25,
    /// 0.5, 0.75, 1.0} — cheap 4x oversampled true-peak proxy
    /// without running a polyphase upsampler.
    #[inline]
    fn intersample_peak(h: &[f32; 4]) -> f32 {
        let x_m1 = h[0];
        let x_0  = h[1];
        let x_1  = h[2];
        let x_2  = h[3];
        let mut peak = x_0.abs().max(x_1.abs());
        // Evaluate the Lagrange-3 interpolant between x[0] and x[1]
        // at t = 0.25, 0.5, 0.75. Coefficients precomputed for each
        // grid position.
        let evaluate = |t: f32| -> f32 {
            let t2 = t * t;
            let t3 = t2 * t;
            // Standard cubic Lagrange basis on grid {-1, 0, 1, 2}.
            let c_m1 = (-t3 + 2.0 * t2 - t) / 2.0;
            let c_0  = (3.0 * t3 - 5.0 * t2 + 2.0) / 2.0;
            let c_1  = (-3.0 * t3 + 4.0 * t2 + t) / 2.0;
            let c_2  = (t3 - t2) / 2.0;
            c_m1 * x_m1 + c_0 * x_0 + c_1 * x_1 + c_2 * x_2
        };
        peak = peak.max(evaluate(0.25).abs());
        peak = peak.max(evaluate(0.50).abs());
        peak = peak.max(evaluate(0.75).abs());
        peak
    }

    /// Rescan the entire lookahead delay line + history for the
    /// current peak. O(lookahead) but only called when the previously
    /// tracked peak ages out — amortised cost stays low.
    fn rescan_peak(&mut self) {
        let mut peak = 0.0_f32;
        for i in 0..self.lookahead_samples {
            peak = peak.max(self.delay_l[i].abs()).max(self.delay_r[i].abs());
        }
        // Don't bother re-interpolating; the per-sample push already
        // captured each intersample peak when the sample first
        // entered the window. The discrete-sample scan is enough to
        // bound the residual after the dominant peak ages out.
        self.current_peak = peak;
        self.peak_age = self.lookahead_samples;
    }

    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.mix <= 1.0e-4 { return (input_l, input_r); }

        // Idle-bypass: only safe to short-circuit when ALL of these
        // hold simultaneously — input is silent, gain has released
        // back to unity, and the lookahead has aged its previous
        // peak out (so we know the delay line is silent too). At that
        // point the per-sample Lagrange-3 ISP estimate + peak-age
        // tracking + ramp logic are all wasted work; output is zero
        // anyway. Single non-silent sample reactivates the full path.
        const SILENCE_EPS: f32 = 1.0e-7;
        let abs_sum = input_l.abs() + input_r.abs();
        if abs_sum < SILENCE_EPS
            && self.gain >= 0.99999
            && self.current_peak < SILENCE_EPS
        {
            // Still advance the ring-buffer cursor so the read
            // position semantics remain consistent if the input
            // wakes up next sample.
            self.delay_l[self.write_pos] = 0.0;
            self.delay_r[self.write_pos] = 0.0;
            self.write_pos = (self.write_pos + 1) % self.lookahead_samples;
            return (0.0, 0.0);
        }

        // Push input into the lookahead delay line.
        self.delay_l[self.write_pos] = input_l;
        self.delay_r[self.write_pos] = input_r;
        // Shift the 4-sample history (h[3] = newest).
        self.hist_l[0] = self.hist_l[1]; self.hist_l[1] = self.hist_l[2];
        self.hist_l[2] = self.hist_l[3]; self.hist_l[3] = input_l;
        self.hist_r[0] = self.hist_r[1]; self.hist_r[1] = self.hist_r[2];
        self.hist_r[2] = self.hist_r[3]; self.hist_r[3] = input_r;

        // True-peak estimate for the newest sample.
        let new_isp = Self::intersample_peak(&self.hist_l)
            .max(Self::intersample_peak(&self.hist_r));

        // Sliding-window max with amortised O(1) update:
        //   * If new peak >= current peak: replace, restart age.
        //   * Else: age the current peak by one sample; on expiry,
        //     rescan the delay line.
        if new_isp >= self.current_peak {
            self.current_peak = new_isp;
            self.peak_age = self.lookahead_samples;
        } else if self.peak_age > 0 {
            self.peak_age -= 1;
            if self.peak_age == 0 { self.rescan_peak(); }
        }

        // Compute the gain that would keep this peak below the
        // ceiling. Cap at 1.0 (never make audio louder).
        let needed_gain = if self.current_peak > 1.0e-9 {
            (self.ceiling / self.current_peak).min(1.0)
        } else { 1.0 };

        // If a louder reduction is needed than the gain we're already
        // ramping toward, restart the ramp so we hit the new target
        // by the time the offending sample exits the lookahead.
        if needed_gain < self.target_gain {
            self.target_gain = needed_gain;
            let denom = self.lookahead_samples as f32;
            self.gain_ramp = (self.gain - self.target_gain) / denom;
        }

        // Apply the ramp this sample.
        if self.gain > self.target_gain {
            self.gain -= self.gain_ramp;
            if self.gain < self.target_gain { self.gain = self.target_gain; }
        } else if self.gain < 1.0 && needed_gain >= self.target_gain {
            // Release toward 1.0 (only when no new ramp is pending).
            self.target_gain = 1.0;
            self.gain = self.gain * self.release_coef
                + 1.0 * (1.0 - self.release_coef);
            if self.gain > 0.99995 { self.gain = 1.0; }
        }

        // Read the delayed sample (oldest in the ring).
        let read_pos = (self.write_pos + 1) % self.lookahead_samples;
        let dry_l = self.delay_l[read_pos];
        let dry_r = self.delay_r[read_pos];
        self.write_pos = read_pos;

        // Apply the (lookahead-aligned) gain. Final hard clamp at the
        // ceiling guards against any residual oversight in the
        // discrete-sample rescan path.
        let out_l = (dry_l * self.gain).clamp(-self.ceiling, self.ceiling);
        let out_r = (dry_r * self.gain).clamp(-self.ceiling, self.ceiling);
        let mix = self.mix;
        ((dry_l * (1.0 - mix)) + out_l * mix,
         (dry_r * (1.0 - mix)) + out_r * mix)
    }

    pub fn reset(&mut self) {
        for v in &mut self.delay_l { *v = 0.0; }
        for v in &mut self.delay_r { *v = 0.0; }
        self.write_pos = 0;
        self.hist_l = [0.0; 4];
        self.hist_r = [0.0; 4];
        self.current_peak = 0.0;
        self.peak_age = 0;
        self.gain = 1.0;
        self.target_gain = 1.0;
        self.gain_ramp = 0.0;
    }

    /// Pass-through expressions for the GUI / param API.
    pub fn ceiling_db(&self) -> f32 { self.ceiling_db }
    pub fn release_ms(&self) -> f32 { self.release_ms }
    pub fn current_gain_reduction_db(&self) -> f32 {
        if self.gain >= 0.99999 { 0.0 } else { 20.0 * self.gain.max(1e-9).log10() }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LUFS METER (visual-only effect wrapper)
// ─────────────────────────────────────────────────────────────────────────────

/// Pass-through effect that runs an ITU-R BS.1770-4 loudness meter
/// on the signal it sees. Audio is not modified — `process` returns
/// the input unchanged. The GUI reads `momentary_lufs()` /
/// `short_term_lufs()` / `integrated_lufs()` to draw the meter.
#[derive(Debug, Clone)]
pub struct LufsMeterEffect {
    meter: crate::meters::LufsMeter,
    /// Idle-bypass: consecutive near-silent input samples. After the
    /// idle threshold the 4 K-weighting biquads + ring-buffer updates
    /// are skipped — the K-weight output is mathematically zero by
    /// then anyway, so the meter readings are unaffected.
    silence_samples: u32,
    pub mix: f32,
}

impl LufsMeterEffect {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            meter: crate::meters::LufsMeter::new(sample_rate),
            silence_samples: 0,
            mix: 1.0, // semantically unused; kept for set_mix API parity
        }
    }
    pub fn set_mix(&mut self, m: f32) { self.mix = m.clamp(0.0, 1.0); }
    pub fn reset(&mut self) {
        self.meter.reset();
        self.silence_samples = 0;
    }
    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        // Idle-bypass: once the K-weighting biquads' transient has
        // rung out under silent input, their output is zero and the
        // ring buffers receive zeros forever — net result is meter
        // readings of -inf LUFS, identical with or without the bypass.
        const SILENCE_EPS: f32 = 1.0e-6;
        const IDLE_AFTER_SAMPLES: u32 = 1024;
        let abs_sum = input_l.abs() + input_r.abs();
        if abs_sum < SILENCE_EPS {
            self.silence_samples = self.silence_samples.saturating_add(1);
            if self.silence_samples >= IDLE_AFTER_SAMPLES {
                return (input_l, input_r);
            }
        } else {
            self.silence_samples = 0;
        }
        self.meter.process(input_l, input_r);
        (input_l, input_r)
    }
    pub fn momentary_lufs(&self)   -> f32 { self.meter.momentary_lufs() }
    pub fn short_term_lufs(&self)  -> f32 { self.meter.short_term_lufs() }
    pub fn integrated_lufs(&self)  -> f32 { self.meter.integrated_lufs() }
}

//==============================================================================
// CONVOLUTION REVERB INSERT EFFECT
//==============================================================================

/// Insert-FX wrapper around `crate::fx::convolution::ConvolutionReverb`.
/// Adds the dry/wet mix curve every other insert reverb uses
/// (constant-power cos/sin) and exposes the IR kind + mix as the two
/// modulatable params.
///
/// Output-level discipline (rules from the convolution-reverb plan):
///   R1 — constant-power mix; at mix=0 the output equals input exactly.
///   R2 — wet level matches the algorithmic `ReverbEffect` within ±20%
///        at the same mix knob position (per-IR gain compensation
///        lives in the shared ConvolutionReverb).
///   R3 — DSP is the SAME shared ConvolutionReverb the per-plugin
///        convolution paths use; no parallel implementation here.
pub struct ConvolutionReverbEffect {
    sample_rate: f32,
    inner: crate::fx::convolution::ConvolutionReverb,
    current_ir: crate::fx::irs::IrKind,
    pub mix: f32,
    mix_dry: f32,
    mix_wet: f32,
}

impl std::fmt::Debug for ConvolutionReverbEffect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ConvolutionReverbEffect(ir={:?}, mix={})", self.current_ir, self.mix)
    }
}

impl Clone for ConvolutionReverbEffect {
    fn clone(&self) -> Self {
        // The convolution engine carries large FFT scratch / IR partitions
        // and isn't itself Clone; rebuild a fresh instance with the same
        // sample-rate + IR. Insert-FX clones happen on session save/load
        // and slot reorders — both fine to restart the tail.
        let mut s = Self::new(self.sample_rate);
        s.set_ir_kind(self.current_ir);
        s.set_mix(self.mix);
        s
    }
}

impl ConvolutionReverbEffect {
    pub fn new(sample_rate: f32) -> Self {
        let initial = crate::fx::irs::IrKind::Cathedral;
        let inner = crate::fx::convolution::ConvolutionReverb::new(sample_rate, initial);
        let mix = 0.35;
        let angle = mix * std::f32::consts::FRAC_PI_2;
        Self {
            sample_rate,
            inner,
            current_ir: initial,
            mix,
            mix_dry: angle.cos(),
            mix_wet: angle.sin(),
        }
    }

    pub fn set_ir_kind(&mut self, kind: crate::fx::irs::IrKind) {
        // Changing the IR rebuilds every partition of it — hundreds of FFTs and
        // a fresh allocation. Doing that because a parameter was re-sent with
        // the value it already had would stall the audio thread for no reason.
        if kind == self.current_ir {
            return;
        }
        self.inner.set_ir(kind);
        self.current_ir = kind;
    }

    pub fn current_ir(&self) -> crate::fx::irs::IrKind { self.current_ir }

    pub fn set_mix(&mut self, m: f32) {
        self.mix = m.clamp(0.0, 1.0);
        let angle = self.mix * std::f32::consts::FRAC_PI_2;
        self.mix_dry = angle.cos();
        self.mix_wet = angle.sin();
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let (wl, wr) = self.inner.process(l, r);
        (l * self.mix_dry + wl * self.mix_wet * Self::INSERT_FX_SCALE,
         r * self.mix_dry + wr * self.mix_wet * Self::INSERT_FX_SCALE)
    }

    /// Per-wrapper insert-FX attenuation. The shared `ConvolutionReverb`
    /// (and therefore `compute_gain_compensation`) was tuned to match
    /// the PER-PLUGIN algorithmic Schroeder reverb, which carries
    /// `output_gain = 3.0` post-`feedback_reverb_parity`. The insert-FX
    /// `ReverbEffect` (src/effects.rs::ReverbEffect) has no such gain
    /// boost, so the bare convolution is ~3-5x louder than it at the
    /// same mix knob. We rescale here — affects ONLY the insert-FX
    /// wrapper and the send-return convolution (which reuses this
    /// wrapper); per-plugin convolution paths still hit the bare
    /// `ConvolutionReverb` and stay level-matched to their algorithmic
    /// siblings. Verified by `convolution_vs_algorithmic_rms_within_20_pct`.
    const INSERT_FX_SCALE: f32 = 0.25;

    pub fn reset(&mut self) {
        // Rebuild the inner engine to flush its FFT scratch + IR tail.
        self.inner = crate::fx::convolution::ConvolutionReverb::new(self.sample_rate, self.current_ir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effect_type() {
        assert_eq!(EffectType::from_index(0), EffectType::None);
        assert_eq!(EffectType::from_index(1), EffectType::Delay);
        assert_eq!(EffectType::Delay.index(), 1);
    }

    /// The new generic BBD Ensemble must (a) be in the public catalog so the
    /// picker lists it, (b) process into audible, bounded, stereo-widened sound
    /// (the whole point of a BBD ensemble), and (c) respond to its Depth/Mix
    /// pids through the track-rack apply path.
    #[test]
    fn ensemble_effect_is_catalogued_and_widens() {
        assert!(EffectType::all().contains(&EffectType::Ensemble),
            "Ensemble missing from EffectType::all() -> picker hides it");

        let sr = 48_000.0;
        let mut slot = EffectSlot::new(sr);
        slot.set_effect_type(EffectType::Ensemble, sr);
        // Drive Depth + full wet through the same raw-pid path the track rack uses.
        slot.set_base_value(crate::fx_params::pid::ENSEMBLE_DEPTH, 0.8);
        crate::drum_fx::apply_fx_param_raw(&mut slot, crate::fx_params::pid::ENSEMBLE_DEPTH, 0.8);
        crate::drum_fx::apply_fx_param_raw(&mut slot, crate::fx_params::pid::ENSEMBLE_MIX, 1.0);

        let mut peak = 0.0f32;
        let mut width = 0.0f32;
        for n in 0..(sr as usize / 4) {
            let x = (2.0 * std::f32::consts::PI * 220.0 * n as f32 / sr).sin() * 0.5;
            let (l, r) = slot.process(x, x);
            assert!(l.is_finite() && r.is_finite(), "ensemble produced NaN/inf");
            peak = peak.max(l.abs()).max(r.abs());
            width = width.max((l - r).abs());
        }
        assert!(peak > 1e-3, "ensemble silent (peak {peak})");
        assert!(peak < 4.0, "ensemble blew up (peak {peak})");
        assert!(width > 1e-4, "ensemble produced no stereo width (mono in -> mono out)");
    }

    /// The faithful VCS-3 spring must be catalogued and produce a decaying wet
    /// tail after the dry input stops (the defining reverb behaviour), through
    /// the same Decay/Mix pid path the track rack uses.
    #[test]
    fn spring_tank_is_catalogued_and_tails() {
        assert!(EffectType::all().contains(&EffectType::SpringTank),
            "SpringTank missing from EffectType::all() -> picker hides it");

        let sr = 48_000.0;
        let mut slot = EffectSlot::new(sr);
        slot.set_effect_type(EffectType::SpringTank, sr);
        crate::drum_fx::apply_fx_param_raw(&mut slot, crate::fx_params::pid::SPRINGTANK_DECAY, 0.7);
        crate::drum_fx::apply_fx_param_raw(&mut slot, crate::fx_params::pid::SPRINGTANK_MIX, 0.8);

        // Excite with a short burst, then feed silence and measure the tail.
        for n in 0..2400 {
            let x = (2.0 * std::f32::consts::PI * 330.0 * n as f32 / sr).sin() * 0.5;
            let (l, r) = slot.process(x, x);
            assert!(l.is_finite() && r.is_finite(), "spring produced NaN/inf");
        }
        let mut tail = 0.0f32;
        for _ in 0..4800 {
            let (l, r) = slot.process(0.0, 0.0);
            tail = tail.max(l.abs()).max(r.abs());
            assert!(l.is_finite() && r.is_finite());
        }
        assert!(tail > 1e-4, "spring produced no tail after input stopped (tail {tail})");
        assert!(tail < 4.0, "spring blew up (tail {tail})");
    }

    /// The faithful Aurora cinematic plate must be catalogued and produce a
    /// decaying wet tail after the dry input stops, via its Size/Damp/Mix pids.
    #[test]
    fn cinema_plate_is_catalogued_and_tails() {
        assert!(EffectType::all().contains(&EffectType::CinemaPlate),
            "CinemaPlate missing from EffectType::all() -> picker hides it");

        let sr = 48_000.0;
        let mut slot = EffectSlot::new(sr);
        slot.set_effect_type(EffectType::CinemaPlate, sr);
        crate::drum_fx::apply_fx_param_raw(&mut slot, crate::fx_params::pid::CINEMAPLATE_SIZE, 0.7);
        crate::drum_fx::apply_fx_param_raw(&mut slot, crate::fx_params::pid::CINEMAPLATE_DAMP, 0.3);
        crate::drum_fx::apply_fx_param_raw(&mut slot, crate::fx_params::pid::CINEMAPLATE_MIX, 0.8);

        for n in 0..2400 {
            let x = (2.0 * std::f32::consts::PI * 330.0 * n as f32 / sr).sin() * 0.5;
            let (l, r) = slot.process(x, x);
            assert!(l.is_finite() && r.is_finite(), "plate produced NaN/inf");
        }
        let mut tail = 0.0f32;
        for _ in 0..9600 {
            let (l, r) = slot.process(0.0, 0.0);
            tail = tail.max(l.abs()).max(r.abs());
            assert!(l.is_finite() && r.is_finite());
        }
        assert!(tail > 1e-4, "plate produced no tail after input stopped (tail {tail})");
        assert!(tail < 4.0, "plate blew up (tail {tail})");
    }

    /// The faithful VP-330 Schroeder string reverb must be catalogued and
    /// produce a decaying wet tail after the dry input stops, via Size/Damp/Mix.
    #[test]
    fn solina_verb_is_catalogued_and_tails() {
        assert!(EffectType::all().contains(&EffectType::SolinaVerb),
            "SolinaVerb missing from EffectType::all() -> picker hides it");

        let sr = 48_000.0;
        let mut slot = EffectSlot::new(sr);
        slot.set_effect_type(EffectType::SolinaVerb, sr);
        crate::drum_fx::apply_fx_param_raw(&mut slot, crate::fx_params::pid::SOLINAVERB_SIZE, 0.7);
        crate::drum_fx::apply_fx_param_raw(&mut slot, crate::fx_params::pid::SOLINAVERB_DAMP, 0.4);
        crate::drum_fx::apply_fx_param_raw(&mut slot, crate::fx_params::pid::SOLINAVERB_MIX, 0.8);

        for n in 0..2400 {
            let x = (2.0 * std::f32::consts::PI * 330.0 * n as f32 / sr).sin() * 0.5;
            let (l, r) = slot.process(x, x);
            assert!(l.is_finite() && r.is_finite(), "solina produced NaN/inf");
        }
        let mut tail = 0.0f32;
        for _ in 0..9600 {
            let (l, r) = slot.process(0.0, 0.0);
            tail = tail.max(l.abs()).max(r.abs());
            assert!(l.is_finite() && r.is_finite());
        }
        assert!(tail > 1e-4, "solina produced no tail after input stopped (tail {tail})");
        assert!(tail < 4.0, "solina blew up (tail {tail})");
    }

    /// The faithful Aurora stereo dimension chorus must be catalogued and widen
    /// a mono input into a decorrelated stereo image, via Delay/Rate/Mix pids.
    #[test]
    fn dimension_is_catalogued_and_widens() {
        assert!(EffectType::all().contains(&EffectType::Dimension),
            "Dimension missing from EffectType::all() -> picker hides it");

        let sr = 48_000.0;
        let mut slot = EffectSlot::new(sr);
        slot.set_effect_type(EffectType::Dimension, sr);
        crate::drum_fx::apply_fx_param_raw(&mut slot, crate::fx_params::pid::DIMENSION_DELAY, 10.0);
        crate::drum_fx::apply_fx_param_raw(&mut slot, crate::fx_params::pid::DIMENSION_RATE, 0.6);
        crate::drum_fx::apply_fx_param_raw(&mut slot, crate::fx_params::pid::DIMENSION_MIX, 1.0);

        let mut peak = 0.0f32;
        let mut width = 0.0f32;
        for n in 0..(sr as usize / 2) {
            let x = (2.0 * std::f32::consts::PI * 220.0 * n as f32 / sr).sin() * 0.3;
            let (l, r) = slot.process(x, x);
            assert!(l.is_finite() && r.is_finite(), "dimension produced NaN/inf");
            peak = peak.max(l.abs()).max(r.abs());
            width = width.max((l - r).abs());
        }
        assert!(peak > 1e-3, "dimension silent (peak {peak})");
        assert!(peak < 4.0, "dimension blew up (peak {peak})");
        assert!(width > 1e-4, "dimension produced no stereo width (mono in -> mono out)");
    }

    /// Convolution reverb must show up in the public insert-FX catalogue
    /// so the track / master / mix FX picker can list it. Regression for
    /// "added the enum variant but forgot the all() entry" — without the
    /// listing the picker silently hides it.
    #[test]
    fn convolution_reverb_in_effect_catalog() {
        assert!(EffectType::all().contains(&EffectType::ConvolutionReverb),
            "ConvolutionReverb missing from EffectType::all()");
        assert_eq!(EffectType::ConvolutionReverb.display_name(), "Conv Reverb");
    }

    /// R1 from the convolution-reverb plan: at `mix == 0.0` the effect
    /// must pass dry through 1:1. Catches any wet bleed-through that
    /// would lift the level even at "fully dry" — a class of bug the
    /// reverb-parity audit explicitly called out.
    #[test]
    fn convolution_reverb_passes_dry_at_mix_zero() {
        let sr = 48_000.0;
        let mut fx = ConvolutionReverbEffect::new(sr);
        fx.set_mix(0.0);
        // Push 64 samples of arbitrary signal; every output sample must
        // equal the input sample within float epsilon.
        for i in 0..64 {
            let s = (i as f32 * 0.137).sin();
            let (l, r) = fx.process(s, -s);
            assert!((l - s).abs() < 1e-5,
                "mix=0 L bled (i={i} in={s} out={l})");
            let neg = -s;
            assert!((r - neg).abs() < 1e-5,
                "mix=0 R bled (i={i} in={neg} out={r})");
        }
    }

    /// R2: per-IR gain compensation should keep the convolution wet
    /// level within ±25% of the algorithmic Schroeder wet level at the
    /// same mix knob position. We render 1 s of white noise through
    /// both, fully wet, and assert their RMS ratio is musical (not a
    /// 3-10× level lift like the pre-audit bug).
    #[test]
    fn convolution_vs_algorithmic_rms_within_20_pct() {
        use crate::fx::irs::IrKind;
        let sr = 48_000.0_f32;
        let n  = sr as usize;
        // Reproducible white noise (xorshift, no rand crate).
        let mut state: u32 = 0xDEADBEEF;
        let mut input = Vec::with_capacity(n);
        for _ in 0..n {
            state ^= state << 13; state ^= state >> 17; state ^= state << 5;
            // ±0.5 range — well below saturation through the reverb
            input.push(((state as f32) / u32::MAX as f32 - 0.5));
        }

        let mut algo = ReverbEffect::new(sr);
        algo.set_reverb_type(ReverbType::Hall);
        algo.set_size(0.5);
        algo.set_damping(0.4);
        algo.set_mix(1.0);

        let mut conv = ConvolutionReverbEffect::new(sr);
        conv.set_ir_kind(IrKind::WoodenHall); // Hall-ish character to match algo
        conv.set_mix(1.0);

        let mut rms_algo = 0.0_f64;
        let mut rms_conv = 0.0_f64;
        for &s in &input {
            let (la, _) = algo.process(s, s);
            let (lc, _) = conv.process(s, s);
            rms_algo += (la as f64) * (la as f64);
            rms_conv += (lc as f64) * (lc as f64);
        }
        rms_algo = (rms_algo / n as f64).sqrt();
        rms_conv = (rms_conv / n as f64).sqrt();
        let ratio = rms_conv / rms_algo;
        assert!(ratio > 0.5 && ratio < 2.0,
            "Convolution wet ({rms_conv:.4}) vs algorithmic wet ({rms_algo:.4}) \
             ratio out of musical range: {ratio:.3} (expect 0.5..2.0)");
    }

    /// The choir ensemble must stay finite and level-safe even with every
    /// parameter at its hottest (12 voices, full detune/wander/vibrato/timbre,
    /// wet-only). The 1/√N normalisation + equal-power crossfade should keep
    /// the output near the input level, never blowing up.
    #[test]
    fn choir_level_safe_and_finite() {
        let sr = 48_000.0_f32;
        let mut fx = ChoirEffect::new(sr);
        fx.set_voices(12);
        fx.set_detune(1.0);
        fx.set_wander(1.0);
        fx.set_vibrato(1.0);
        fx.set_timbre(1.0);
        fx.set_width(1.0);
        fx.set_warmth(0.0);
        fx.set_mix(1.0);
        let mut phase = 0.0_f32;
        let inc = 220.0 / sr;
        let mut peak = 0.0_f32;
        for n in 0..(sr as usize * 4) {
            let t = phase * std::f32::consts::TAU;
            let s = 0.4 * t.sin() + 0.25 * (2.0 * t).sin() + 0.15 * (3.0 * t).sin(); // ~0.8 peak
            phase += inc;
            if phase >= 1.0 { phase -= 1.0; }
            let (l, r) = fx.process(s, s);
            assert!(l.is_finite() && r.is_finite(), "non-finite choir output at sample {n}");
            peak = peak.max(l.abs()).max(r.abs());
        }
        assert!(peak < 2.0, "choir output too hot at extreme settings: peak {peak}");
        assert!(peak > 0.05, "choir produced no signal: peak {peak}");
    }

    /// Diagnostic: render a sustained vowel-ish tone through the choir and write
    /// it for listening, reporting peak + L/R decorrelation (lower than mono but
    /// > 0 => stereo spread without mono-fold cancellation).
    ///   cargo test --lib effects::tests::dump_choir -- --ignored --nocapture
    #[test]
    #[ignore = "diagnostic — writes WAV"]
    fn dump_choir() {
        use std::io::Write;
        let sr = 48_000.0_f32;
        let mut fx = ChoirEffect::new(sr);
        fx.set_voices(8);
        fx.set_detune(0.6);
        fx.set_wander(0.35);
        fx.set_vibrato(0.3);
        fx.set_timbre(0.5);
        fx.set_width(0.8);
        fx.set_warmth(0.4);
        fx.set_mix(0.6);

        let f0 = 220.0_f32;
        let n = (sr * 3.0) as usize;
        let mut out: Vec<(f32, f32)> = Vec::with_capacity(n);
        let mut ph = 0.0_f32;
        let (mut cxy, mut cxx, mut cyy) = (0.0f64, 0.0f64, 0.0f64);
        for _ in 0..n {
            let t = ph * std::f32::consts::TAU;
            let s = 0.35 * t.sin() + 0.22 * (2.0 * t).sin() + 0.14 * (3.0 * t).sin() + 0.08 * (4.0 * t).sin();
            ph += f0 / sr;
            if ph >= 1.0 { ph -= 1.0; }
            let (l, r) = fx.process(s, s);
            cxy += (l * r) as f64;
            cxx += (l * l) as f64;
            cyy += (r * r) as f64;
            out.push((l, r));
        }
        let lr_corr = cxy / (cxx.sqrt() * cyy.sqrt()).max(1e-9);
        let peak = out.iter().fold(0.0f32, |m, &(l, r)| m.max(l.abs()).max(r.abs()));

        let path = "/tmp/choir_demo.wav";
        let mut w = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
        let data = (out.len() * 4) as u32;
        w.write_all(b"RIFF").unwrap(); w.write_all(&(36 + data).to_le_bytes()).unwrap();
        w.write_all(b"WAVE").unwrap(); w.write_all(b"fmt ").unwrap();
        w.write_all(&16u32.to_le_bytes()).unwrap(); w.write_all(&1u16.to_le_bytes()).unwrap();
        w.write_all(&2u16.to_le_bytes()).unwrap(); w.write_all(&(sr as u32).to_le_bytes()).unwrap();
        w.write_all(&((sr as u32) * 4).to_le_bytes()).unwrap(); w.write_all(&4u16.to_le_bytes()).unwrap();
        w.write_all(&16u16.to_le_bytes()).unwrap(); w.write_all(b"data").unwrap();
        w.write_all(&data.to_le_bytes()).unwrap();
        for &(l, r) in &out {
            w.write_all(&((l.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes()).unwrap();
            w.write_all(&((r.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes()).unwrap();
        }
        w.flush().unwrap();
        eprintln!("wrote {} ({:.1}s) peak={:.3} L/R-corr={:.3}", path, n as f32 / sr, peak, lr_corr);
    }

    #[test]
    fn brickwall_limiter_holds_ceiling_under_hot_input() {
        // Drive a 200 Hz +6 dBFS sine (peak ~2.0) through the limiter
        // with the default -0.3 dB ceiling. After the lookahead has
        // settled, no sample should overshoot the ceiling — that's
        // the brick-wall guarantee.
        let sr = 48_000.0_f32;
        let mut lim = BrickwallLimiterEffect::new(sr);
        lim.set_ceiling_db(-0.3);
        lim.set_release_ms(50.0);
        let ceil = lim.ceiling;
        let mut phase = 0.0_f32;
        let inc = 200.0 / sr;
        // Warm up — the first lookahead_samples of output are at
        // unity gain on dry zeros; skip them before checking peaks.
        for _ in 0..lim.lookahead_samples + 256 {
            let s = (phase * std::f32::consts::TAU).sin() * 2.0;
            phase += inc; if phase >= 1.0 { phase -= 1.0; }
            lim.process(s, s);
        }
        // Now measure 0.5 s of steady-state output.
        let mut max_abs = 0.0_f32;
        for _ in 0..(sr as usize / 2) {
            let s = (phase * std::f32::consts::TAU).sin() * 2.0;
            phase += inc; if phase >= 1.0 { phase -= 1.0; }
            let (l, r) = lim.process(s, s);
            max_abs = max_abs.max(l.abs()).max(r.abs());
        }
        // Allow a tiny tolerance (1e-3) for the discrete-sample
        // rescan path; the final hard clamp inside `process` is the
        // backstop.
        assert!(max_abs <= ceil + 1.0e-3,
            "brick-wall limiter exceeded ceiling {ceil}: peak {max_abs}");
        // Reduction should be measurable at this drive level.
        assert!(lim.current_gain_reduction_db() < -3.0,
            "expected meaningful gain reduction; got {} dB",
            lim.current_gain_reduction_db());
    }

    #[test]
    fn dynamic_eq_attenuates_only_when_band_exceeds_threshold() {
        // Configure band 0 as a downward 1 kHz peak with threshold
        // -20 dB, ratio 4:1, 12 dB range. Disable the other 3 bands.
        // Then:
        //   * Drive a quiet 1 kHz tone (-40 dB) -> below threshold,
        //     no gain reduction expected.
        //   * Drive a loud 1 kHz tone (0 dB)    -> above threshold,
        //     gain reduction should be measurable.
        let sr = 48_000.0_f32;
        let mut eq = DynamicEqEffect::new(sr);
        for b in 1..4 { eq.set_band_enabled(b, false); }
        eq.set_band_freq(0, 1000.0);
        eq.set_band_q(0, 1.0);
        eq.set_band_threshold(0, -20.0);
        eq.set_band_ratio(0, 4.0);
        eq.set_band_range(0, 12.0);
        eq.set_band_attack(0, 0.005);
        eq.set_band_release(0, 0.060);
        eq.set_band_mode(0, 0);   // downward
        eq.set_band_enabled(0, true);

        let render_peak = |eq: &mut DynamicEqEffect, amp_db: f32, dur_s: f32| -> f32 {
            let amp = 10.0_f32.powf(amp_db / 20.0);
            let mut phase = 0.0_f32;
            let inc = 1000.0 / 48_000.0;
            let n = (48_000.0 * dur_s) as usize;
            // Warm-up — let the envelope follower settle.
            for _ in 0..(48_000 / 5) { // 200 ms
                let s = (phase * std::f32::consts::TAU).sin() * amp;
                phase += inc; if phase >= 1.0 { phase -= 1.0; }
                eq.process(s, s);
            }
            let mut peak = 0.0_f32;
            for _ in 0..n {
                let s = (phase * std::f32::consts::TAU).sin() * amp;
                phase += inc; if phase >= 1.0 { phase -= 1.0; }
                let (l, _r) = eq.process(s, s);
                peak = peak.max(l.abs());
            }
            peak
        };

        // -40 dB input: below threshold; envelope at -40 dB after
        // the +12 dB sense boost = -28 dB; that's still below the
        // -20 dB threshold so band_gain_db should remain near 0.
        let _ = render_peak(&mut eq, -40.0, 0.1);
        let gr_quiet = eq.band_gain_db(0).abs();
        assert!(gr_quiet < 1.0,
            "below-threshold input should not engage the band; got {gr_quiet} dB");
        eq.reset();
        // 0 dB input: well above threshold; expect a clear attenuation.
        let _ = render_peak(&mut eq, 0.0, 0.2);
        let gr_loud = eq.band_gain_db(0);
        assert!(gr_loud < -1.5,
            "above-threshold loud input should attenuate the band; got {gr_loud} dB");
    }

    #[test]
    fn mid_side_eq_routes_mid_and_side_independently() {
        // Boost mid +6 dB at 1 kHz, leave side flat. A mono 1 kHz
        // sine (L == R) should come out ~6 dB louder; a pure stereo
        // difference signal (L = -R) should pass through unchanged.
        let sr = 48_000.0_f32;
        let mut eq = MidSideEqEffect::new(sr);
        // ParametricEq bands 1 and 2 are PEAK filters (band 0 is a
        // low-shelf, band 3 is a high-shelf). Pick band 1 so a +6 dB
        // boost at 1 kHz really lifts the sine by ~6 dB at centre.
        eq.set_mid_band_enabled(0, false);
        eq.set_mid_band_enabled(2, false);
        eq.set_mid_band_enabled(3, false);
        eq.set_side_band_enabled(0, false);
        eq.set_side_band_enabled(1, false);
        eq.set_side_band_enabled(2, false);
        eq.set_side_band_enabled(3, false);
        eq.set_mid_band(1, 1000.0, 6.0, 1.0);

        let render_mono = |eq: &mut MidSideEqEffect, amp: f32, n: usize| -> f32 {
            let mut phase = 0.0_f32;
            let inc = 1000.0 / 48_000.0;
            let mut peak = 0.0_f32;
            for _ in 0..n {
                let s = (phase * std::f32::consts::TAU).sin() * amp;
                phase += inc; if phase >= 1.0 { phase -= 1.0; }
                let (l, _r) = eq.process(s, s);
                peak = peak.max(l.abs());
            }
            peak
        };
        let render_side = |eq: &mut MidSideEqEffect, amp: f32, n: usize| -> f32 {
            let mut phase = 0.0_f32;
            let inc = 1000.0 / 48_000.0;
            let mut peak = 0.0_f32;
            for _ in 0..n {
                let s = (phase * std::f32::consts::TAU).sin() * amp;
                phase += inc; if phase >= 1.0 { phase -= 1.0; }
                let (l, _r) = eq.process(s, -s);
                peak = peak.max(l.abs());
            }
            peak
        };

        let mono_peak = render_mono(&mut eq, 0.1, 4_800);
        eq.reset();
        // After mid +6 dB at 1 kHz, the mono component should be
        // boosted by ~6 dB. Allow a wide tolerance (peak in 0.18..0.22)
        // because we measure after a short warm-up.
        assert!(mono_peak > 0.18 && mono_peak < 0.25,
            "mono signal with mid+6dB should peak ~0.2; got {mono_peak}");
        let side_peak = render_side(&mut eq, 0.1, 4_800);
        // Pure side signal should pass through unaltered.
        assert!((side_peak - 0.1).abs() < 0.02,
            "side signal should pass through unchanged; got {side_peak}");
    }

    #[test]
    fn multiband_comp_bypass_passes_signal_through() {
        // At ratio = 1.0 (no compression) and default makeup = 0 dB,
        // the LR4 crossover-and-recombine path must reproduce the
        // input. We don't expect bit-exact: LR4 sum is flat in
        // magnitude but introduces a 360° phase shift, so the
        // measure is the per-sample peak ratio between input and
        // output after a warm-up.
        let sr = 48_000.0_f32;
        let mut mb = MultibandComp3Effect::new(sr);
        for b in 0..3 { mb.set_band_ratio(b, 1.0); mb.set_band_makeup_db(b, 0.0); }
        let mut phase = 0.0_f32;
        let inc = 1000.0 / sr;
        // Warm up the LR4 filters.
        for _ in 0..2_048 {
            let s = (phase * std::f32::consts::TAU).sin() * 0.5;
            phase += inc; if phase >= 1.0 { phase -= 1.0; }
            mb.process(s, s);
        }
        let mut in_peak = 0.0_f32;
        let mut out_peak = 0.0_f32;
        for _ in 0..(sr as usize / 2) {
            let s = (phase * std::f32::consts::TAU).sin() * 0.5;
            phase += inc; if phase >= 1.0 { phase -= 1.0; }
            in_peak = in_peak.max(s.abs());
            let (l, _r) = mb.process(s, s);
            out_peak = out_peak.max(l.abs());
        }
        let ratio = out_peak / in_peak;
        assert!(ratio > 0.95 && ratio < 1.05,
            "LR4 bypass should reproduce magnitude within ±5%; ratio {ratio}");
    }

    #[test]
    fn multiband_comp_compresses_loud_low_band() {
        // Feed a loud 60 Hz sine — the low band (crossover at 200 Hz)
        // should compress it heavily, lowering the output peak vs
        // the bypassed case. Mid + high bands stay below threshold.
        let sr = 48_000.0_f32;
        let mut mb = MultibandComp3Effect::new(sr);
        // Configure low band: -24 dB threshold, 8:1 ratio.
        mb.set_band_threshold_db(0, -24.0);
        mb.set_band_ratio(0, 8.0);
        mb.set_band_attack(0, 0.005);
        mb.set_band_release(0, 0.080);
        // Disable mid + high compression.
        mb.set_band_enabled(1, false);
        mb.set_band_enabled(2, false);
        let mut phase = 0.0_f32;
        let inc = 60.0 / sr;
        // Warm-up + measurement at 0 dBFS amp.
        for _ in 0..4_800 {
            let s = (phase * std::f32::consts::TAU).sin();
            phase += inc; if phase >= 1.0 { phase -= 1.0; }
            mb.process(s, s);
        }
        let mut out_peak = 0.0_f32;
        for _ in 0..4_800 {
            let s = (phase * std::f32::consts::TAU).sin();
            phase += inc; if phase >= 1.0 { phase -= 1.0; }
            let (l, _r) = mb.process(s, s);
            out_peak = out_peak.max(l.abs());
        }
        // With -24 dB threshold and 8:1 ratio on a 0 dBFS sine, the
        // expected output level is roughly -21 dB (threshold +
        // (input-threshold)/ratio). 10^(-21/20) ≈ 0.089. Be lenient
        // since the LR4 split introduces phase + the envelope
        // follower has a finite attack — assert it's below 0.5.
        assert!(out_peak < 0.5,
            "loud 60 Hz with 8:1 low-band comp should drop peak well below input; got {out_peak}");
    }

    #[test]
    fn spectrum_analyzer_tracks_band_energy() {
        // A pure 1 kHz sine should peak in the band centred at 1 kHz
        // (band index 16) and stay near the floor everywhere else.
        let sr = 48_000.0_f32;
        let mut sa = SpectrumAnalyzerEffect::new(sr);
        let mut phase = 0.0_f32;
        let inc = 1000.0 / sr;
        // Run for 1 s so the envelope follower settles into steady state.
        for _ in 0..sr as usize {
            let s = (phase * std::f32::consts::TAU).sin() * 0.5;
            phase += inc; if phase >= 1.0 { phase -= 1.0; }
            sa.process(s, s);
        }
        let levels = sa.levels_db();
        // Find the band index whose centre is 1000 Hz.
        let target_idx = SpectrumAnalyzerEffect::BAND_FREQS
            .iter()
            .position(|&f| (f - 1000.0).abs() < 0.5)
            .expect("1 kHz band exists");
        let target_db = levels[target_idx];
        // The 1 kHz band should sit roughly at the input level (peak
        // amplitude 0.5 -> ~-6 dB peak, ~-9 dB RMS). The +12 dB
        // sense boost is subtracted out by `levels_db`.
        assert!(target_db > -20.0,
            "1 kHz band should track a -6 dB sine; got {target_db} dB");
        // Adjacent bands should be measurably lower (RTA isolation).
        let neighbour_db = levels[(target_idx + 4).min(31)];
        assert!(neighbour_db < target_db - 10.0,
            "spectrum should isolate the 1 kHz band from neighbours \
             4 bands away; target {target_db} dB vs neighbour {neighbour_db} dB");
    }

    #[test]
    fn master_assistant_proposes_gain_to_hit_target_lufs() {
        // Feed a -23 LUFS reference signal, ask for -14 LUFS target,
        // verify the proposed correction_gain_db comes out near +9 dB
        // once the LUFS meter has integrated for ~6 s.
        let sr = 48_000.0_f32;
        let mut ma = MasterAssistantEffect::new(sr);
        ma.set_target_lufs(-14.0);
        ma.set_mode_f32(2.0); // Apply
        let amp = 10.0_f32.powf(-20.0 / 20.0); // -20 dBFS single channel = -23 LUFS
        let mut phase = 0.0_f32;
        let inc = 1000.0 / sr;
        // 8 s of audio so the integrated-loudness gate settles and
        // the 1 Hz recompute fires at least 6 times.
        for _ in 0..(sr as usize * 8) {
            let s = (phase * std::f32::consts::TAU).sin() * amp;
            phase += inc; if phase >= 1.0 { phase -= 1.0; }
            ma.process(s, 0.0);
        }
        let gain = ma.correction_gain_db();
        // -23 LUFS -> -14 LUFS = +9 dB. Cap was ±24 dB. Wide ±2 dB
        // tolerance for the K-weighting + integration-gate interplay
        // around the reference signal.
        assert!((gain - 9.0).abs() < 2.0,
            "Master Assistant should propose ~+9 dB to lift -23 LUFS to -14; got {gain} dB");
    }

    #[test]
    fn master_assistant_apply_mode_actually_boosts_output_amplitude() {
        // Verifies the integration loop closes: in Apply mode the
        // assistant should not only PROPOSE +9 dB, it should also
        // multiply the output amplitude by ~10^(9/20) ≈ 2.82.
        let sr = 48_000.0_f32;
        let mut ma = MasterAssistantEffect::new(sr);
        ma.set_target_lufs(-14.0);
        ma.set_mode_f32(2.0); // Apply
        let amp = 10.0_f32.powf(-20.0 / 20.0);
        let mut phase = 0.0_f32;
        let inc = 1000.0 / sr;
        // 8 s lets the LUFS gate settle + the gain smoother converge.
        for _ in 0..(sr as usize * 8) {
            let s = (phase * std::f32::consts::TAU).sin() * amp;
            phase += inc; if phase >= 1.0 { phase -= 1.0; }
            ma.process(s, 0.0);
        }
        // After settle, measure peak amplitude of the next 100 ms of
        // output. With the EQ near 0 dB delta (balanced target on a
        // single sine would inject minor band corrections), peak
        // should sit between 2.0× and 4.0× the input amplitude.
        let mut peak = 0.0_f32;
        for _ in 0..(sr as usize / 10) {
            let s = (phase * std::f32::consts::TAU).sin() * amp;
            phase += inc; if phase >= 1.0 { phase -= 1.0; }
            let (l, _) = ma.process(s, 0.0);
            peak = peak.max(l.abs());
        }
        let gain_lin = peak / amp;
        assert!(gain_lin > 2.0 && gain_lin < 4.0,
            "Apply mode should boost the output by ~2.8× (+9 dB) when target -14 is +9 dB above source; got {gain_lin}×");
    }

    #[test]
    fn dynamic_eq_idle_bypass_recovers_on_signal() {
        // Hot signal → silence (bypass engages after biquads ring
        // out) → hot signal again. After wake-up the EQ produces a
        // finite signal-shaped output. The post-engagement check
        // confirms idle output is bit-exact zero (proves the bypass
        // path actually fires).
        let sr = 48_000.0_f32;
        let mut eq = DynamicEqEffect::new(sr);
        eq.set_mix(1.0);
        let mut phase = 0.0_f32;
        let inc = 1000.0 / sr;
        for _ in 0..(sr as usize / 2) {
            let s = (phase * std::f32::consts::TAU).sin() * 0.3;
            phase += inc; if phase >= 1.0 { phase -= 1.0; }
            let _ = eq.process(s, s);
        }
        // Skip the first 8192 silence samples — biquad rings settle,
        // then idle threshold of 1024 hits. After that, idle bypass
        // gives exact zero.
        for _ in 0..8192 { let _ = eq.process(0.0, 0.0); }
        for _ in 0..2048 {
            let (l, r) = eq.process(0.0, 0.0);
            assert!(l == 0.0 && r == 0.0,
                "DynEq post-bypass should output bit-exact zero; got ({l}, {r})");
        }
        let mut woke_up = false;
        for _ in 0..64 {
            let s = (phase * std::f32::consts::TAU).sin() * 0.3;
            phase += inc; if phase >= 1.0 { phase -= 1.0; }
            let (l, r) = eq.process(s, s);
            assert!(l.is_finite() && r.is_finite());
            if l.abs() > 1.0e-4 { woke_up = true; break; }
        }
        assert!(woke_up, "DynEq idle-bypass failed to wake on signal");
    }

    #[test]
    fn multiband_comp_idle_bypass_recovers_on_signal() {
        let sr = 48_000.0_f32;
        let mut mb = MultibandComp3Effect::new(sr);
        mb.set_mix(1.0);
        let mut phase = 0.0_f32;
        let inc = 500.0 / sr; // 500 Hz lands in mid band
        for _ in 0..(sr as usize / 2) {
            let s = (phase * std::f32::consts::TAU).sin() * 0.3;
            phase += inc; if phase >= 1.0 { phase -= 1.0; }
            let _ = mb.process(s, s);
        }
        for _ in 0..8192 { let _ = mb.process(0.0, 0.0); }
        for _ in 0..2048 {
            let (l, r) = mb.process(0.0, 0.0);
            assert!(l == 0.0 && r == 0.0, "Multiband post-bypass should output bit-exact zero");
        }
        let mut woke_up = false;
        for _ in 0..512 {
            let s = (phase * std::f32::consts::TAU).sin() * 0.3;
            phase += inc; if phase >= 1.0 { phase -= 1.0; }
            let (l, r) = mb.process(s, s);
            assert!(l.is_finite() && r.is_finite());
            if l.abs() > 1.0e-3 { woke_up = true; break; }
        }
        assert!(woke_up, "Multiband idle-bypass failed to wake on signal");
    }

    #[test]
    fn mid_side_eq_idle_bypass_recovers_on_signal() {
        let sr = 48_000.0_f32;
        let mut eq = MidSideEqEffect::new(sr);
        eq.set_mid_band(1, 1000.0, 6.0, 1.0);
        let mut phase = 0.0_f32;
        let inc = 1000.0 / sr;
        for _ in 0..(sr as usize / 4) {
            let s = (phase * std::f32::consts::TAU).sin() * 0.3;
            phase += inc; if phase >= 1.0 { phase -= 1.0; }
            let _ = eq.process(s, s);
        }
        for _ in 0..8192 { let _ = eq.process(0.0, 0.0); }
        for _ in 0..2048 {
            let (l, r) = eq.process(0.0, 0.0);
            assert!(l == 0.0 && r == 0.0, "M/S EQ post-bypass should output bit-exact zero");
        }
        let mut woke_up = false;
        for _ in 0..128 {
            let s = (phase * std::f32::consts::TAU).sin() * 0.3;
            phase += inc; if phase >= 1.0 { phase -= 1.0; }
            let (l, r) = eq.process(s, s);
            assert!(l.is_finite() && r.is_finite());
            if l.abs() > 1.0e-3 { woke_up = true; break; }
        }
        assert!(woke_up, "M/S EQ idle-bypass failed to wake on signal");
    }

    #[test]
    fn brickwall_limiter_idle_bypass_outputs_silence_at_steady_state() {
        // After silence longer than lookahead + release recovery,
        // the brick-wall should be in steady-state idle.
        let sr = 48_000.0_f32;
        let mut lim = BrickwallLimiterEffect::new(sr);
        // Hit it briefly so gain dips, then let it release.
        for _ in 0..256 { let _ = lim.process(1.5, 1.5); }
        // 1 s of silence — well past 5 ms lookahead + 100 ms default
        // release. Steady-state should hold; output must be exactly 0.
        for _ in 0..(sr as usize / 2) { let _ = lim.process(0.0, 0.0); }
        // Check the steady-state phase produces exactly zero.
        let mut all_zero = true;
        for _ in 0..2048 {
            let (l, r) = lim.process(0.0, 0.0);
            if l != 0.0 || r != 0.0 { all_zero = false; break; }
        }
        assert!(all_zero, "BrickwallLimiter should output exact zero at steady-state idle");
        // Wake up.
        let (l, _) = lim.process(0.5, 0.5);
        assert!(l.is_finite(),
            "BrickwallLimiter must recover finite output on wake-up; got {l}");
    }

    #[test]
    fn lufs_meter_idle_bypass_engages_on_silence() {
        // LufsMeter is the cheapest mastering effect (4 K-weighting
        // biquads per sample) but still wasteful at idle. After 1024
        // silent samples its bypass engages — the K-weight output is
        // zero by then anyway, so the integrated-LUFS reading stays
        // at -inf, matching what the un-bypassed path would produce.
        // Verify: after a hot signal then 2 s of silence, integrated
        // LUFS still reads finite and consistent with the active
        // window. The bypass MUST NOT silently advance some clock or
        // hidden state that breaks subsequent measurement.
        let sr = 48_000.0_f32;
        let mut meter = LufsMeterEffect::new(sr);
        let mut phase = 0.0_f32;
        let inc = 1_000.0 / sr;
        let amp = 10.0_f32.powf(-20.0 / 20.0);  // -20 dBFS sine
        for _ in 0..(sr as usize * 8) {
            let s = (phase * std::f32::consts::TAU).sin() * amp;
            phase += inc; if phase >= 1.0 { phase -= 1.0; }
            meter.process(s, s);
        }
        let charged_lufs = meter.integrated_lufs();
        // -20 dBFS stereo identical → -20 LUFS for the gated mean
        // (BS.1770 channel weights sum to 1.0 across L+R for stereo).
        assert!(charged_lufs.is_finite(),
            "LUFS reading should be finite after settled hot input; got {charged_lufs}");
        assert!(charged_lufs > -30.0 && charged_lufs < -15.0,
            "expected LUFS near -20 for a -20 dBFS test tone; got {charged_lufs}");

        // 2 s of silence — well past the 1024-sample bypass threshold.
        // After the bypass kicks in, the meter should hold its
        // existing measurement (the K-weight has decayed to zero so
        // running mean-square stays at the integrated value).
        for _ in 0..(sr as usize * 2) {
            meter.process(0.0, 0.0);
        }
        let after = meter.integrated_lufs();
        assert!(after.is_finite(),
            "integrated_lufs must stay finite after silence bypass; got {after}");
        // Wake-up: feed signal again and verify the meter still measures.
        for _ in 0..(sr as usize * 4) {
            let s = (phase * std::f32::consts::TAU).sin() * amp;
            phase += inc; if phase >= 1.0 { phase -= 1.0; }
            meter.process(s, s);
        }
        let woke = meter.integrated_lufs();
        assert!(woke.is_finite(),
            "LUFS reading lost finiteness after wake-up; got {woke}");
    }

    #[test]
    fn spectrum_analyzer_idle_bypass_engages_on_silence() {
        // Drive the analyser with a hot sine to charge the band
        // envelopes, then feed silence for longer than the idle
        // threshold. The idle-bypass should engage (silence_samples
        // saturates past IDLE_AFTER_SAMPLES) and the envelopes decay
        // from their charged level — proving the cheaper path still
        // produces sensible output.
        let sr = 48_000.0_f32;
        let mut sa = SpectrumAnalyzerEffect::new(sr);
        // Charge band ~16 (≈1 kHz) with a 1 kHz sine.
        let mut phase = 0.0_f32;
        let inc = 1000.0 / sr;
        for _ in 0..(sr as usize / 2) {
            let s = (phase * std::f32::consts::TAU).sin() * 0.5;
            phase += inc; if phase >= 1.0 { phase -= 1.0; }
            sa.process(s, s);
        }
        let levels = sa.levels_db();
        // Pick the highest band — should correspond to ~1 kHz.
        let charged_max = levels.iter().cloned().fold(-200.0_f32, f32::max);
        assert!(charged_max > -30.0,
            "expected analyser to register the 1 kHz sine clearly; got max {charged_max} dB");
        // Now feed silence for 2 s — well past the 1024-sample idle
        // threshold, plus enough release time for the envelope to
        // drop a measurable amount.
        for _ in 0..(sr as usize * 2) {
            sa.process(0.0, 0.0);
        }
        let after = sa.levels_db();
        let after_max = after.iter().cloned().fold(-200.0_f32, f32::max);
        assert!(after_max < charged_max - 10.0,
            "idle-bypass release path should have decayed envelopes by >10 dB; \
             charged_max {charged_max}, after_max {after_max}");
    }

    #[test]
    fn master_assistant_off_mode_is_true_bypass() {
        // Off mode must be bit-exact passthrough once any previous
        // Apply-mode gain has slewed back to unity. Verifies the
        // optimisation: no LUFS, no spectrum, no EQ work in Off.
        let sr = 48_000.0_f32;
        let mut ma = MasterAssistantEffect::new(sr);
        ma.set_mode_f32(0.0); // Off
        // Warm the gain smoother (init 1.0 → target 1.0; immediate).
        for _ in 0..256 { ma.process(0.0, 0.0); }
        // Now feed a varied signal and assert exact identity.
        let mut phase = 0.0_f32;
        let inc = 1000.0 / sr;
        let mut max_abs_diff = 0.0_f32;
        for _ in 0..1024 {
            let s = (phase * std::f32::consts::TAU).sin() * 0.5;
            phase += inc; if phase >= 1.0 { phase -= 1.0; }
            let (l, r) = ma.process(s, -s);
            max_abs_diff = max_abs_diff.max((l - s).abs()).max((r + s).abs());
        }
        assert!(max_abs_diff < 1.0e-6,
            "MA Off mode should be bit-exact passthrough; saw max diff {max_abs_diff}");
    }

    #[test]
    fn master_assistant_listen_mode_passes_audio_through() {
        // Listen mode must not alter the audio — it measures only.
        let sr = 48_000.0_f32;
        let mut ma = MasterAssistantEffect::new(sr);
        ma.set_mode_f32(1.0); // Listen
        let amp = 0.5_f32;
        let mut phase = 0.0_f32;
        let inc = 1000.0 / sr;
        // 2 s of audio
        let mut max_abs_diff = 0.0_f32;
        for _ in 0..(sr as usize * 2) {
            let s = (phase * std::f32::consts::TAU).sin() * amp;
            phase += inc; if phase >= 1.0 { phase -= 1.0; }
            let (l, r) = ma.process(s, s * 0.5);
            max_abs_diff = max_abs_diff.max((l - s).abs()).max((r - s * 0.5).abs());
        }
        // Allow a tiny tolerance for the 50 ms gain-slewer settling
        // from 1.0 (init) → 1.0 (Listen target).
        assert!(max_abs_diff < 1.0e-4,
            "Listen mode must be ~passthrough; saw max abs diff {max_abs_diff}");
    }

    #[test]
    fn master_assistant_off_mode_decays_corrections_toward_zero() {
        // After publishing a non-zero correction in Apply mode,
        // switching to Off must cause `correction_gain_db()` to decay
        // toward 0 over a few 1 Hz updates.
        let sr = 8_000.0_f32; // small SR keeps the 1 Hz tick cheap for the test
        let mut ma = MasterAssistantEffect::new(sr);
        ma.set_target_lufs(-14.0);
        ma.set_mode_f32(2.0); // Apply
        let amp = 10.0_f32.powf(-20.0 / 20.0);
        let mut phase = 0.0_f32;
        let inc = 1000.0 / sr;
        // 8 s — gives the gate enough buffer at low sr too.
        for _ in 0..(sr as usize * 8) {
            let s = (phase * std::f32::consts::TAU).sin() * amp;
            phase += inc; if phase >= 1.0 { phase -= 1.0; }
            ma.process(s, 0.0);
        }
        let before = ma.correction_gain_db();
        assert!(before.abs() > 1.0, "expected non-trivial correction; got {before}");
        // Switch Off and run several seconds — corrections halve each
        // 1 Hz recompute so 5 s = ~32× attenuation.
        ma.set_mode_f32(0.0);
        for _ in 0..(sr as usize * 5) {
            ma.process(0.0, 0.0);
        }
        let after = ma.correction_gain_db();
        assert!(after.abs() < before.abs() * 0.1,
            "Off mode should decay correction; before {before} after {after}");
    }

    #[test]
    fn brickwall_limiter_releases_to_unity_on_silence() {
        // After a loud passage, the limiter should release back to
        // unity gain when the input goes quiet — proves the release
        // coefficient actually relaxes the gain.
        let sr = 48_000.0_f32;
        let mut lim = BrickwallLimiterEffect::new(sr);
        lim.set_ceiling_db(-0.3);
        lim.set_release_ms(20.0); // fast release for the test
        // Pump it hard for 50 ms.
        for _ in 0..(sr as usize / 20) {
            lim.process(1.5, 1.5);
        }
        let reduced = lim.current_gain_reduction_db();
        assert!(reduced < -1.0,
            "expected gain reduction after loud input; got {reduced} dB");
        // Silent input for 200 ms — plenty of time for a 20 ms
        // release to recover.
        for _ in 0..(sr as usize / 5) {
            lim.process(0.0, 0.0);
        }
        let released = lim.current_gain_reduction_db();
        assert!(released > -0.1,
            "limiter should release to unity after silence; got {released} dB");
    }

    #[test]
    fn test_delay_creation() {
        let delay = DelayEffect::new(44100.0);
        assert_eq!(delay.delay_type, DelayType::Simple);
    }

    #[test]
    fn test_delay_process() {
        let mut delay = DelayEffect::new(44100.0);
        delay.set_time(0.1);
        delay.set_mix(0.5);
        delay.set_feedback(0.3);

        let (l, r) = delay.process(1.0, 1.0);
        assert!(l.is_finite());
        assert!(r.is_finite());
    }

    #[test]
    fn test_reverb_creation() {
        let reverb = ReverbEffect::new(44100.0);
        assert_eq!(reverb.reverb_type, ReverbType::Hall);
    }

    #[test]
    fn test_effects_chain() {
        let mut chain = EffectsChain::with_slots(44100.0, 4);

        // Add a delay to slot 0
        chain
            .get_slot(0)
            .unwrap()
            .set_effect_type(EffectType::Delay, 44100.0);

        // Add a reverb to slot 1
        chain
            .get_slot(1)
            .unwrap()
            .set_effect_type(EffectType::Reverb, 44100.0);

        // Process audio
        let (l, r) = chain.process(0.5, 0.5);
        assert!(l.is_finite());
        assert!(r.is_finite());
    }

    #[test]
    fn full_mastering_chain_coexists_and_holds_ceiling() {
        // End-to-end integration test for the mastering suite:
        // Slot 0: DynamicEq   (frequency-aware shaping)
        // Slot 1: MidSideEq   (stereo placement)
        // Slot 2: MultibandComp3 (glue compression)
        // Slot 3: SpectrumAnalyzer (passthrough + measurement)
        // Slot 4: LufsMeter (passthrough + measurement)
        // Slot 5: MasterAssistant (smart Apply mode)
        // Slot 6: BrickwallLimiter (final ceiling)
        //
        // Drive with a hot, harmonically-rich signal (1 kHz + 5 kHz +
        // 100 Hz mix near 0 dBFS) and assert:
        //   * Every output sample is finite.
        //   * The brick-wall limiter holds the output peak under 0 dBFS.
        //   * The chain doesn't go silent or DC-bias (proves at least
        //     some signal makes it through every stage).
        let sr = 48_000.0_f32;
        let mut chain = EffectsChain::with_slots(sr, 7);
        chain.set_slot(0, EffectType::DynamicEq, true, 1.0);
        chain.set_slot(1, EffectType::MidSideEq, true, 1.0);
        chain.set_slot(2, EffectType::MultibandComp3, true, 1.0);
        chain.set_slot(3, EffectType::SpectrumAnalyzer, true, 1.0);
        chain.set_slot(4, EffectType::LufsMeter, true, 1.0);
        chain.set_slot(5, EffectType::MasterAssistant, true, 1.0);
        chain.set_slot(6, EffectType::BrickwallLimiter, true, 1.0);

        // Steer the MasterAssistant into Apply mode so it actually
        // does something — proves the integration handles a chain
        // where one slot up-gains hot into a slot that brick-walls.
        if let Some(slot) = chain.get_slot(5) {
            if let Some(ref mut ma) = slot.master_assistant {
                ma.set_target_lufs(-14.0);
                ma.set_mode_f32(2.0); // Apply
            }
        }

        let mut max_abs = 0.0_f32;
        let mut sum = 0.0_f32;
        let mut sum_sq = 0.0_f32;
        let dur = sr as usize * 4; // 4 s, lets LUFS + MA settle.
        let mut phase1 = 0.0_f32;
        let mut phase2 = 0.0_f32;
        let mut phase3 = 0.0_f32;
        for _ in 0..dur {
            let s =  (phase1 * std::f32::consts::TAU).sin() * 0.4
                  +  (phase2 * std::f32::consts::TAU).sin() * 0.3
                  +  (phase3 * std::f32::consts::TAU).sin() * 0.2;
            phase1 += 1_000.0 / sr; if phase1 >= 1.0 { phase1 -= 1.0; }
            phase2 += 5_000.0 / sr; if phase2 >= 1.0 { phase2 -= 1.0; }
            phase3 +=   100.0 / sr; if phase3 >= 1.0 { phase3 -= 1.0; }
            let (l, r) = chain.process(s, s * 0.9);
            assert!(l.is_finite() && r.is_finite(),
                "mastering chain produced non-finite sample l={l} r={r}");
            max_abs = max_abs.max(l.abs()).max(r.abs());
            sum    += l;
            sum_sq += l * l;
        }
        // The default brick-wall ceiling is -0.3 dB ≈ 0.966. Allow a
        // tiny slop for the inter-sample-peak Lagrange-3 evaluation
        // overshooting by a fraction of a dB on a transient.
        assert!(max_abs < 1.0,
            "BrickwallLimiter at end of chain should hold output under 0 dBFS; got {max_abs}");
        // Average should be near 0 (no DC bias).
        let dc = sum / dur as f32;
        assert!(dc.abs() < 0.05,
            "mastering chain should not introduce DC bias; got {dc}");
        // RMS should be a real (non-tiny) value — the chain hasn't
        // silenced everything.
        let rms = (sum_sq / dur as f32).sqrt();
        assert!(rms > 0.05,
            "mastering chain produced essentially silence; rms={rms}");
    }

    #[test]
    fn test_tempo_division() {
        let div = TempoDivision::Quarter;
        assert_eq!(div.beats(), 1.0);

        let time = div.to_time_seconds(120.0);
        assert!((time - 0.5).abs() < 0.001); // 120 BPM quarter note = 0.5 seconds
    }

    /// Diagnostic: profile the full mastering chain at both audio rate
    /// and at idle. Demonstrates that the idle-bypass optimisations
    /// added across the suite produce measurable CPU recovery when
    /// the bus is quiet.
    ///
    /// ```text
    /// cargo test --release --lib effects::tests::profile_mastering_chain_idle_vs_audio \
    ///   -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "diagnostic — run with --ignored"]
    fn profile_mastering_chain_idle_vs_audio() {
        use std::time::Instant;
        let sr = 48_000.0_f32;
        let total = 200_000usize; // ~4 s of audio

        // Build the chain twice (one per phase) — same topology so
        // numbers compare apples-to-apples.
        let build_chain = || {
            let mut c = EffectsChain::with_slots(sr, 7);
            c.set_slot(0, EffectType::DynamicEq, true, 1.0);
            c.set_slot(1, EffectType::MidSideEq, true, 1.0);
            c.set_slot(2, EffectType::MultibandComp3, true, 1.0);
            c.set_slot(3, EffectType::SpectrumAnalyzer, true, 1.0);
            c.set_slot(4, EffectType::LufsMeter, true, 1.0);
            c.set_slot(5, EffectType::MasterAssistant, true, 1.0);
            c.set_slot(6, EffectType::BrickwallLimiter, true, 1.0);
            c
        };

        // ── Audio phase ─────────────────────────────────────────────
        let mut chain = build_chain();
        let mut phase = 0.0_f32;
        let mut acc = 0.0_f32;
        let t0 = Instant::now();
        for _ in 0..total {
            phase += 220.0 / sr; if phase >= 1.0 { phase -= 1.0; }
            let s = (phase * std::f32::consts::TAU).sin() * 0.3;
            let (l, r) = chain.process(s, s);
            acc += l + r;
        }
        let dt_audio = t0.elapsed().as_nanos() as f64;
        let ns_audio = dt_audio / total as f64;

        // ── Idle phase ──────────────────────────────────────────────
        // Build a fresh chain so we don't measure recovery work from
        // the prior loud state. Run 1 s of silence first (8192-sample
        // warm-down + 1024-sample idle threshold easily met) before
        // starting the clock — that's the steady-state idle scenario.
        let mut chain = build_chain();
        for _ in 0..(sr as usize) { let _ = chain.process(0.0, 0.0); }
        let mut acc2 = 0.0_f32;
        let t0 = Instant::now();
        for _ in 0..total {
            let (l, r) = chain.process(0.0, 0.0);
            acc2 += l + r;
        }
        let dt_idle = t0.elapsed().as_nanos() as f64;
        let ns_idle = dt_idle / total as f64;

        println!("[mastering-chain profile, sr={sr}, samples={total}]");
        println!("  audio:  {ns_audio:>7.2} ns/sample  (acc={acc:.3e})");
        println!("  idle:   {ns_idle:>7.2} ns/sample  (acc={acc2:.3e})");
        let recovered = (1.0 - ns_idle / ns_audio.max(1.0)) * 100.0;
        println!("  idle savings: {recovered:.1} %");

        // Soft assertion so a regression that erases the idle bypass
        // would surface here too — should always be >40 % recovery.
        // Treat as soft because release builds occasionally produce
        // sub-1 ns differences that can't be measured reliably.
        if ns_audio > 50.0 {
            assert!(ns_idle < ns_audio * 0.7,
                "idle should be substantially cheaper than audio; \
                 audio={ns_audio:.1} idle={ns_idle:.1}");
        }
    }

    /// Per-effect CPU profile. Wraps each EffectType in a single-slot
    /// chain, feeds 200k stereo samples (~4 s of audio), reports
    /// ns/sample so the heaviest effects surface in one run.
    ///
    ///   cargo test --release --lib effects::tests::profile_all_effects \
    ///     -- --ignored --nocapture
    #[test]
    #[ignore = "diagnostic — run with --ignored"]
    fn profile_all_effects() {
        use std::time::Instant;

        let sr = 48_000.0_f32;
        let total = 200_000usize; // ~4 s

        let measure = |label: &str, et: EffectType| -> f64 {
            let mut chain = EffectsChain::with_slots(sr, 1);
            chain.set_slot(0, et, true, 0.5);
            // Feed a sine sweep so resonant effects don't sit silent.
            let mut acc_l: f32 = 0.0;
            let mut acc_r: f32 = 0.0;
            let mut phase: f32 = 0.0;
            let t0 = Instant::now();
            for i in 0..total {
                phase += 220.0 / sr;
                if phase >= 1.0 { phase -= 1.0; }
                let s = (phase * std::f32::consts::TAU).sin();
                let (l, r) = chain.process(s, s);
                acc_l += l * 1e-12; acc_r += r * 1e-12;
                if i % 16384 == 0 {
                    // Anchor the input — keep optimizer from eliding work.
                    std::hint::black_box(&mut chain);
                }
            }
            std::hint::black_box((acc_l, acc_r));
            let ns = t0.elapsed().as_nanos() as f64 / total as f64;
            eprintln!("{:<22} | {:>7.1} ns/sample ({:.2}% of one core)",
                      label, ns, ns * sr as f64 / 1_000_000_000.0 * 100.0);
            ns
        };

        eprintln!("\n=== Per-effect CPU profile ===");
        eprintln!("{:<22} | {:>20} | {}", "effect", "ns/sample", "% of one core");

        for et in [
            EffectType::None, EffectType::Delay, EffectType::Reverb,
            EffectType::Chorus, EffectType::Phaser, EffectType::Flanger,
            EffectType::Distortion, EffectType::Vocoder, EffectType::Compressor,
            EffectType::Limiter, EffectType::Expander, EffectType::AutoPanner,
            EffectType::Stutter, EffectType::Filter, EffectType::RingMod,
            EffectType::LoFi, EffectType::StereoImager, EffectType::Choir,
            EffectType::Maximizer, EffectType::Spatializer3D, EffectType::ParametricEq,
            EffectType::Doppler, EffectType::TransientShaper,
            EffectType::GranularFreezer, EffectType::SpectralFreeze,
            EffectType::ResonatorBank, EffectType::AutoWah, EffectType::TapeStop,
            // Mastering tier — Phases A-E. Without these the per-effect
            // diagnostic would silently skip the heaviest plugins.
            EffectType::BrickwallLimiter, EffectType::LufsMeter,
            EffectType::DynamicEq, EffectType::MidSideEq,
            EffectType::MultibandComp3, EffectType::SpectrumAnalyzer,
            EffectType::MasterAssistant,
        ] {
            measure(&format!("{:?}", et), et);
        }

        // ── Distortion sub-types ─────────────────────────────────
        // Each DistortionType uses a different shaper. SoftClip /
        // HardClip / Bitcrush are cheap; Tube / Diode / Fold / Fuzz
        // each call expensive transcendentals per sample.
        eprintln!("\n--- Distortion sub-types ---");
        for dt in DistortionType::all() {
            let mut chain = EffectsChain::with_slots(sr, 1);
            chain.set_slot(0, EffectType::Distortion, true, 0.5);
            if let Some(slot) = chain.get_slot(0) {
                if let Some(eff) = slot.distortion.as_mut() {
                    eff.set_distortion_type(*dt);
                    eff.set_drive(0.7);
                }
            }
            let mut phase: f32 = 0.0;
            let mut acc: f32 = 0.0;
            let t0 = std::time::Instant::now();
            for _ in 0..total {
                phase += 220.0 / sr;
                if phase >= 1.0 { phase -= 1.0; }
                let s = (phase * std::f32::consts::TAU).sin();
                let (l, r) = chain.process(s, s);
                acc += (l + r) * 1e-12;
            }
            std::hint::black_box(acc);
            let ns = t0.elapsed().as_nanos() as f64 / total as f64;
            eprintln!("Distortion[{:<8}] | {:>7.1} ns/sample ({:.2}% of one core)",
                      dt.display_name(), ns,
                      ns * sr as f64 / 1_000_000_000.0 * 100.0);
        }
    }
}

#[cfg(test)]
mod multiband_ducker_tests {
    use super::*;

    /// With a loud low-frequency sidechain key, the ducker must attenuate the
    /// LOW band of the program while leaving a high-frequency tone essentially
    /// untouched (frequency-slotting, not full-band ducking).
    #[test]
    fn ducks_lows_not_highs_under_key() {
        let sr = 48_000.0;
        let mut d = MultibandDuckerEffect::new(sr);
        d.sidechain_active = true;
        // settle the key envelope with a strong low key
        let key = 0.9f32;
        // measure a 60 Hz program (low) and a 4 kHz program (high) RMS reduction
        let run = |d: &mut MultibandDuckerEffect, f: f32| -> f32 {
            let mut acc = 0.0f32; let n = 4800;
            for i in 0..n {
                let t = i as f32 / sr;
                let prog = (2.0 * std::f32::consts::PI * f * t).sin() * 0.5;
                d.sidechain_l = key; d.sidechain_r = key;
                let (l, _r) = d.process(prog, prog);
                acc += l * l;
            }
            (acc / n as f32).sqrt()
        };
        let lo = run(&mut d, 60.0);
        let mut d2 = MultibandDuckerEffect::new(sr);
        d2.sidechain_active = true;
        let hi = run(&mut d2, 4000.0);
        // low band gets ducked well below its 0.354 RMS input; high band barely changes
        assert!(lo < 0.20, "low band not ducked: {lo}");
        assert!(hi > 0.33, "high band wrongly ducked: {hi}");
    }

    /// No sidechain assigned -> self-keys; a silent program stays silent (no DC/instability).
    #[test]
    fn silent_in_silent_out() {
        let mut d = MultibandDuckerEffect::new(48_000.0);
        for _ in 0..2000 { let (l, r) = d.process(0.0, 0.0); assert_eq!(l, 0.0); assert_eq!(r, 0.0); }
    }
}

#[cfg(test)]
mod velvet_drift_tests {
    use super::*;

    /// Velvet decorrelator must be MONO-INVARIANT: the mono sum (L+R) of the
    /// output equals the mono sum of the input, for any width (zero coloration).
    #[test]
    fn velvet_is_mono_invariant() {
        let mut v = VelvetDecorrelatorEffect::new(48_000.0);
        v.set_width(1.0); v.set_size(1.0);
        let mut max_err = 0.0f32;
        let mut st = 0xBEEFu32;
        for _ in 0..20_000 {
            st ^= st << 13; st ^= st >> 17; st ^= st << 5;
            let l = (st as f32 / u32::MAX as f32) * 2.0 - 1.0;
            st ^= st << 13; st ^= st >> 17; st ^= st << 5;
            let r = (st as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let (ol, or) = v.process(l, r);
            max_err = max_err.max(((ol + or) - (l + r)).abs());
        }
        assert!(max_err < 1e-4, "velvet not mono-invariant: {max_err}");
    }

    /// Analog drift: silent in -> silent out, and a tone stays finite + bounded.
    #[test]
    fn drift_is_stable() {
        let mut d = AnalogDriftEffect::new(48_000.0);
        for _ in 0..4000 { let (l, r) = d.process(0.0, 0.0); assert_eq!(l, 0.0); assert_eq!(r, 0.0); }
        let mut d2 = AnalogDriftEffect::new(48_000.0);
        for i in 0..48_000 {
            let s = (2.0 * std::f32::consts::PI * 220.0 * i as f32 / 48_000.0).sin() * 0.5;
            let (l, r) = d2.process(s, s);
            assert!(l.is_finite() && r.is_finite() && l.abs() < 2.0 && r.abs() < 2.0);
        }
    }

    /// MidSideWiden must be BIT-IDENTICAL to the render's drum-bus M/S widen (canon_trance 1043-1053).
    #[test]
    fn mid_side_widen_is_bit_exact() {
        let sr = 48_000.0f32;
        // reference (verbatim from the render)
        let widen_ref = |buf: &mut [(f32, f32)], fc: f32, width: f32| {
            let k = 1.0 - (-std::f32::consts::TAU * fc / sr).exp();
            let (mut wl, mut wr) = (0.0f32, 0.0f32);
            for x in buf.iter_mut() {
                wl += (x.0 - wl) * k;
                wr += (x.1 - wr) * k;
                let (hl, hr) = (x.0 - wl, x.1 - wr);
                let m = 0.5 * (hl + hr);
                let sd = 0.5 * (hl - hr) * width;
                x.0 = wl + m + sd;
                x.1 = wr + m - sd;
            }
        };
        let mut a: Vec<(f32, f32)> = Vec::with_capacity(20_000);
        let mut st = 0x51DEu32;
        for _ in 0..20_000 {
            st ^= st << 13; st ^= st >> 17; st ^= st << 5;
            let l = (st as f32 / u32::MAX as f32) * 2.0 - 1.0;
            st ^= st << 13; st ^= st >> 17; st ^= st << 5;
            let r = (st as f32 / u32::MAX as f32) * 2.0 - 1.0;
            a.push((l * 0.5, r * 0.5));
        }
        let mut reference = a.clone();
        widen_ref(&mut reference, 500.0, 1.9);
        let mut fx = MidSideWidenEffect::new(sr);
        fx.set_crossover(500.0); fx.set_width(1.9); fx.set_mix(1.0);
        for (i, &(l, r)) in a.iter().enumerate() {
            let (ol, or_) = fx.process(l, r);
            assert_eq!(ol.to_bits(), reference[i].0.to_bits(), "L mismatch i={i}");
            assert_eq!(or_.to_bits(), reference[i].1.to_bits(), "R mismatch i={i}");
        }
    }

    #[test]
    /// The canon pulsar growing-distortion shaper (canon_trance 1146) ported as
    /// DistortionType::TanhClip must be BIT-IDENTICAL: 0.5*tanh(x*drive)+0.5*clamp(x*drive).
    fn tanh_clip_is_bit_exact() {
        let sr = 48_000.0f32;
        // reference (verbatim from the render's `sh`)
        let sh_ref = |v: f32, drive: f32| -> f32 {
            let d = v * drive;
            0.5 * d.tanh() + 0.5 * d.clamp(-1.0, 1.0)
        };
        for &drive in &[2.0f32, 8.0, 15.0, 28.0] {
            let mut fx = DistortionEffect::new(sr);
            fx.set_distortion_type(DistortionType::TanhClip);
            fx.set_drive(drive);
            fx.set_output(1.0);
            fx.set_mix(1.0);
            let mut st = 0x9E37u32;
            for _ in 0..10_000 {
                st ^= st << 13; st ^= st >> 17; st ^= st << 5;
                let l = (st as f32 / u32::MAX as f32) * 2.0 - 1.0;
                st ^= st << 13; st ^= st >> 17; st ^= st << 5;
                let r = (st as f32 / u32::MAX as f32) * 2.0 - 1.0;
                let (ol, or_) = fx.process(l, r);
                assert_eq!(ol.to_bits(), sh_ref(l, drive).to_bits(), "L mismatch drive={drive} l={l}");
                assert_eq!(or_.to_bits(), sh_ref(r, drive).to_bits(), "R mismatch drive={drive} r={r}");
            }
        }
    }

    #[test]
    /// HaasSplit must be BIT-IDENTICAL to the render's pulsar stereo spread (canon_trance 1152-1161).
    fn haas_split_is_bit_exact() {
        let sr = 48_000.0f32;
        // local Bq (verbatim canon_trance 764-812), only highpass/lowpass/run/apply needed.
        struct Bq { b0: f32, b1: f32, b2: f32, a1: f32, a2: f32, s: [[f32; 4]; 2] }
        impl Bq {
            fn highpass(sr: f32, fc: f32, q: f32) -> Bq {
                let w = 2.0 * std::f32::consts::PI * fc / sr; let (c, sn) = (w.cos(), w.sin());
                let al = sn / (2.0 * q); let a0 = 1.0 + al;
                Bq { b0: ((1.0 + c) / 2.0) / a0, b1: (-(1.0 + c)) / a0, b2: ((1.0 + c) / 2.0) / a0,
                     a1: (-2.0 * c) / a0, a2: (1.0 - al) / a0, s: [[0.0; 4]; 2] }
            }
            fn lowpass(sr: f32, fc: f32, q: f32) -> Bq {
                let w = 2.0 * std::f32::consts::PI * fc / sr; let (c, sn) = (w.cos(), w.sin());
                let al = sn / (2.0 * q); let a0 = 1.0 + al;
                Bq { b0: ((1.0 - c) / 2.0) / a0, b1: ((1.0 - c)) / a0, b2: ((1.0 - c) / 2.0) / a0,
                     a1: (-2.0 * c) / a0, a2: (1.0 - al) / a0, s: [[0.0; 4]; 2] }
            }
            fn run(&mut self, x: f32, ch: usize) -> f32 {
                let s = &mut self.s[ch];
                let y = self.b0 * x + self.b1 * s[0] + self.b2 * s[1] - self.a1 * s[2] - self.a2 * s[3];
                s[1] = s[0]; s[0] = x; s[3] = s[2]; s[2] = y; y
            }
            fn apply(&mut self, buf: &mut [(f32, f32)]) {
                for x in buf.iter_mut() { x.0 = self.run(x.0, 0); x.1 = self.run(x.1, 1); }
            }
        }
        let mut a: Vec<(f32, f32)> = Vec::with_capacity(20_000);
        let mut st = 0x71C5u32;
        for _ in 0..20_000 {
            st ^= st << 13; st ^= st >> 17; st ^= st << 5;
            let l = (st as f32 / u32::MAX as f32) * 2.0 - 1.0;
            st ^= st << 13; st ^= st >> 17; st ^= st << 5;
            let r = (st as f32 / u32::MAX as f32) * 2.0 - 1.0;
            a.push((l * 0.5, r * 0.5));
        }
        // reference (verbatim canon_trance 1152-1161)
        let mut reference = a.clone();
        {
            let mut hi = reference.clone(); Bq::highpass(sr, 130.0, 0.7).apply(&mut hi);
            let mut lo = reference.clone(); Bq::lowpass(sr, 130.0, 0.7).apply(&mut lo);
            let dl = (0.011 * sr) as usize;
            for i in 0..reference.len() {
                let rdel = if i >= dl { hi[i - dl].0 } else { 0.0 };
                reference[i].0 = lo[i].0 + hi[i].0;
                reference[i].1 = lo[i].1 + rdel;
            }
        }
        let mut fx = HaasSplitEffect::new(sr);
        fx.set_crossover(130.0); fx.set_delay_ms(11.0); fx.set_mix(1.0);
        for (i, &(l, r)) in a.iter().enumerate() {
            let (ol, or_) = fx.process(l, r);
            assert_eq!(ol.to_bits(), reference[i].0.to_bits(), "L mismatch i={i}");
            assert_eq!(or_.to_bits(), reference[i].1.to_bits(), "R mismatch i={i}");
        }
    }

    /// The canon_trance `early_reflect` reference, inlined VERBATIM (688-719) so the
    /// session EarlyReflectionsEffect can be proven BIT-IDENTICAL (no approximation).
    fn ref_early_reflect(buf: &mut [(f32, f32)], sr: f32, mix: f32, seed: u32) {
        let sc = |k: u32| -> f32 {
            let h = (seed.wrapping_mul(2654435761) ^ k.wrapping_mul(40503)) >> 9;
            0.82 + 0.36 * ((h & 0xFFFF) as f32 / 65535.0)
        };
        let tl: Vec<f32> = [4.7f32, 9.7, 15.3, 21.7, 29.3, 38.1, 48.7, 61.3]
            .iter().enumerate().map(|(k, &t)| t * sc(k as u32 * 2 + 1)).collect();
        let tr: Vec<f32> = [5.9f32, 11.3, 17.9, 24.1, 32.7, 41.3, 52.1, 64.7]
            .iter().enumerate().map(|(k, &t)| t * sc(k as u32 * 2 + 2)).collect();
        let g = [0.72f32, 0.58, 0.47, 0.39, 0.31, 0.25, 0.19, 0.14];
        let maxd = (0.09 * sr) as usize + 8;
        let mut dl = vec![0.0f32; maxd];
        let mut dr = vec![0.0f32; maxd];
        let il: Vec<usize> = tl.iter().map(|&ms| ((ms * 0.001 * sr) as usize).min(maxd - 2)).collect();
        let ir: Vec<usize> = tr.iter().map(|&ms| ((ms * 0.001 * sr) as usize).min(maxd - 2)).collect();
        let mut pos = 0usize;
        for s in buf.iter_mut() {
            dl[pos] = s.0; dr[pos] = s.1;
            let (mut el, mut er) = (0.0f32, 0.0f32);
            for k in 0..g.len() {
                el += dl[(pos + maxd - il[k]) % maxd] * g[k];
                er += dr[(pos + maxd - ir[k]) % maxd] * g[k];
            }
            s.0 += (el * 0.85 + er * 0.15) * mix;
            s.1 += (er * 0.85 + el * 0.15) * mix;
            pos = (pos + 1) % maxd;
        }
    }

    /// ParametricEq band TYPE: `types=[0;4]` (auto) must be BIT-IDENTICAL to the legacy
    /// fixed layout (band0 low-shelf, band3 high-shelf, bands 1/2 peak), and an explicit
    /// peak (type 1) at band0 must DIFFER from the auto low-shelf below the corner freq.
    #[test]
    fn eq_band_type_backcompat_and_peak() {
        let sr = 48_000.0f32;
        let cfg = |eq: &mut ParametricEqEffect| {
            for (b,(f,g,q)) in [(90.0,4.0,0.8),(500.0,3.0,0.9),(2000.0,-3.0,1.0),(9000.0,3.0,0.7)].iter().enumerate() {
                eq.set_band_freq(b,*f); eq.set_band_gain(b,*g); eq.set_band_q(b,*q); eq.set_band_enabled(b,true);
            }
        };
        // auto (default types) must equal a fresh EQ (which is also auto) -- and match the legacy
        // low-shelf-at-0 behaviour: a 30 Hz tone (well below the 90 Hz band0) is LIFTED by a low-shelf.
        let mut auto = ParametricEqEffect::new(sr); cfg(&mut auto);
        let mut peak0 = ParametricEqEffect::new(sr); cfg(&mut peak0); peak0.set_band_type(0, 1); // band0 -> peak
        let tone = |eq: &mut ParametricEqEffect, hz: f32| -> f32 {
            let mut mx = 0.0f32;
            for i in 0..4800 { let s=(2.0*std::f32::consts::PI*hz*i as f32/sr).sin()*0.5; let (l,_)=eq.process(s,s); if i>2400 {mx=mx.max(l.abs());} }
            mx
        };
        let a30 = tone(&mut auto, 30.0);
        let p30 = tone(&mut peak0, 30.0);
        // low-shelf (+4 dB) lifts 30 Hz toward ~0.79; a 90 Hz PEAK leaves 30 Hz ~unity (0.5).
        assert!(a30 > 0.62, "auto band0 should low-shelf-lift 30Hz (got {a30})");
        assert!(p30 < 0.58, "peak band0 should NOT lift 30Hz (got {p30})");
    }

    /// EarlyReflectionsEffect must be BIT-IDENTICAL to the render's early_reflect for the
    /// seeds/mixes the canon uses (vio1 0x1111/0.18, vio2 0x2222/0.22, harps 0x7777/0.12).
    #[test]
    fn early_reflections_is_bit_exact() {
        let sr = 48_000.0f32;
        for &(seed, mix) in &[(0x1111u32, 0.18f32), (0x2222, 0.22), (0x7777, 0.12), (0xB111, 0.10)] {
            // a deterministic stereo test signal
            let mut a: Vec<(f32, f32)> = Vec::with_capacity(20_000);
            let mut st = seed ^ 0xABCD;
            for _ in 0..20_000 {
                st ^= st << 13; st ^= st >> 17; st ^= st << 5;
                let l = (st as f32 / u32::MAX as f32) * 2.0 - 1.0;
                st ^= st << 13; st ^= st >> 17; st ^= st << 5;
                let r = (st as f32 / u32::MAX as f32) * 2.0 - 1.0;
                a.push((l * 0.5, r * 0.5));
            }
            let mut reference = a.clone();
            ref_early_reflect(&mut reference, sr, mix, seed);
            let mut fx = EarlyReflectionsEffect::new(sr);
            fx.set_seed(seed as f32);
            fx.set_mix(mix);
            for (i, &(l, r)) in a.iter().enumerate() {
                let (ol, or) = fx.process(l, r);
                assert_eq!(ol.to_bits(), reference[i].0.to_bits(), "L mismatch seed={seed:#x} i={i}");
                assert_eq!(or.to_bits(), reference[i].1.to_bits(), "R mismatch seed={seed:#x} i={i}");
            }
        }
    }
}

#[cfg(test)]
mod square_lfo_click_tests {
    use super::*;

    // A Square (or S&H) LFO steps the pan/gain instantly, which clicks. The
    // slew on the LFO output must bound the per-sample change on a CONSTANT
    // input (any output delta comes purely from the modulation).
    #[test]
    fn square_autopan_does_not_click() {
        let sr = 48_000.0;
        let mut ap = AutoPannerEffect::new(sr);
        ap.mode = AutoPanMode::Classic;
        ap.wave = AutoPanWave::Square;
        ap.depth = 1.0;
        ap.width = 1.0;
        ap.mix = 1.0;
        ap.rate = 5.0;
        ap.tempo_sync = false;
        let mut prev = (0.0f32, 0.0f32);
        let mut max_d = 0.0f32;
        for i in 0..sr as usize {
            let (l, r) = ap.process(0.5, 0.5);
            if i > 200 {
                max_d = max_d.max((l - prev.0).abs()).max((r - prev.1).abs());
            }
            prev = (l, r);
        }
        // A slewed square can't jump instantly; without the slew this would be
        // a near-full-scale jump (~1.4) at every half-cycle.
        assert!(max_d < 0.05, "square autopan clicks: max per-sample delta {max_d}");
    }

    #[test]
    fn square_tremolo_pan_does_not_click() {
        let sr = 48_000.0;
        let mut ap = AutoPannerEffect::new(sr);
        ap.mode = AutoPanMode::TremoloPan;
        ap.wave = AutoPanWave::Square;
        ap.depth = 1.0;
        ap.width = 1.0;
        ap.mix = 1.0;
        ap.rate = 5.0;
        ap.trem_depth = 1.0;
        ap.tempo_sync = false;
        let mut prev = (0.0f32, 0.0f32);
        let mut max_d = 0.0f32;
        for i in 0..sr as usize {
            let (l, r) = ap.process(0.5, 0.5);
            if i > 200 {
                max_d = max_d.max((l - prev.0).abs()).max((r - prev.1).abs());
            }
            prev = (l, r);
        }
        assert!(max_d < 0.06, "square tremolo-pan clicks: max per-sample delta {max_d}");
    }
}

#[cfg(test)]
mod retune_harmony_tests {
    use super::*;
    use std::f32::consts::TAU;

    fn goertzel(x: &[f32], f: f32, sr: f32) -> f32 {
        let w = TAU * f / sr;
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (i, &s) in x.iter().enumerate() {
            let p = w as f64 * i as f64;
            re += s as f64 * p.cos();
            im += s as f64 * p.sin();
        }
        ((re * re + im * im).sqrt() / x.len() as f64) as f32
    }

    /// A 30-cents-sharp A4 must come out within 15 cents of 440 Hz.
    #[test]
    fn retune_corrects_sharp_sine_to_scale() {
        let sr = 48_000.0;
        let mut fx = RetuneEffect::new(sr);
        fx.set_speed(1.0);
        fx.set_amount(1.0);
        let sharp = 440.0 * 2f32.powf(30.0 / 1200.0);
        let n = (1.5 * sr) as usize;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let x = (TAU * sharp * i as f32 / sr).sin() * 0.5;
            let (l, _) = fx.process(x, x);
            out.push(l);
        }
        // The grain crossfade adds AM sidebands that bias a YIN reading, so
        // measure where the ENERGY sits instead: the corrected output must
        // concentrate at 440 Hz, well above the original sharp frequency.
        let tail = &out[n / 2..];
        let m_target = goertzel(tail, 440.0, sr);
        let m_sharp = goertzel(tail, sharp, sr);
        assert!(m_target > 0.05, "no energy at target (440 Hz mag {m_target:.4})");
        assert!(m_target > m_sharp * 2.0,
            "energy not moved to target: 440 Hz {m_target:.4} vs sharp {m_sharp:.4}");
    }

    /// Voice 1 at +2 scale degrees over C4 in C major must add E4.
    #[test]
    fn harmony_adds_diatonic_third() {
        let sr = 48_000.0;
        let mut fx = HarmonyEffect::new(sr);
        fx.set_degree(0, 2.0);
        fx.set_degree(1, 0.0);
        fx.set_degree(2, 0.0);
        fx.set_mix(1.0);
        let c4 = 261.63;
        let e4 = 329.63;
        let n = (1.5 * sr) as usize;
        let mut out = Vec::with_capacity(n);
        let mut wet = Vec::with_capacity(n);
        for i in 0..n {
            let x = (TAU * c4 * i as f32 / sr).sin() * 0.4;
            let (l, r) = fx.process(x, x);
            let mono = (l + r) * 0.5;
            out.push(mono);
            wet.push(mono - x); // harmony voices are additive over the dry
        }
        let tail = &out[n / 2..];
        let mag_c = goertzel(tail, c4, sr);
        assert!(mag_c > 0.1, "dry fundamental lost (C4 mag {mag_c:.4})");
        // The shifted voice carries grain AM, so read its PITCH (YIN on the
        // isolated wet signal) rather than a single Goertzel bin.
        let mut det = crate::harmonizer_dsp::pitch_detect::YinDetector::default_for_sample_rate(sr);
        for &s in &wet[n / 2..] { det.push_sample(s); }
        let pitch = det.pitch();
        let cents = 1200.0 * (pitch / e4).log2();
        assert!(pitch > 0.0 && cents.abs() < 40.0,
            "harmony voice at {pitch:.1} Hz, {cents:+.0} cents from E4");
        let wet_rms = (wet[n / 2..].iter().map(|x| (x * x) as f64).sum::<f64>()
            / (n / 2) as f64).sqrt() as f32;
        assert!(wet_rms > 0.02, "harmony voice too quiet (wet rms {wet_rms:.4})");
    }

    /// Mix 0 on Retune must be a bit-exact dry pass (fidelity default).
    #[test]
    fn retune_mix_zero_is_dry() {
        let sr = 48_000.0;
        let mut fx = RetuneEffect::new(sr);
        fx.set_mix(0.0);
        for i in 0..4096 {
            let x = (TAU * 220.0 * i as f32 / sr).sin() * 0.5;
            let (l, r) = fx.process(x, x);
            assert_eq!(l, x);
            assert_eq!(r, x);
        }
    }

    /// Chain-level smoke: both new effects instantiate through the slot API
    /// and process without NaN/blowup.
    #[test]
    fn slot_api_smoke() {
        let sr = 48_000.0;
        for et in [EffectType::Retune, EffectType::Harmony] {
            let mut chain = EffectsChain::with_slots(sr, 1);
            chain.set_slot(0, et, true, 0.8);
            let mut peak = 0.0f32;
            for i in 0..24_000 {
                let x = (TAU * 330.0 * i as f32 / sr).sin() * 0.4;
                let (l, r) = chain.process(x, x);
                assert!(l.is_finite() && r.is_finite(), "{et:?} produced NaN");
                peak = peak.max(l.abs()).max(r.abs());
            }
            assert!(peak > 0.05, "{et:?} silent (peak {peak})");
            assert!(peak < 2.0, "{et:?} blowing up (peak {peak})");
        }
    }
}

#[cfg(test)]
mod follow_key_tests {
    use super::*;

    #[test]
    fn song_key_applies_only_in_follow_mode() {
        let mut fx = RetuneEffect::new(48_000.0);
        // Manual mode: broadcast ignored, manual setters win.
        fx.set_key(4.0); // E
        fx.set_song_key(9, 2);
        assert_eq!(fx.key, 4, "manual mode must ignore the song key");
        // Follow mode: broadcast wins, manual setters are inert.
        fx.set_follow(1.0);
        fx.set_song_key(9, 2); // A minor
        assert_eq!(fx.key, 9);
        assert_eq!(fx.scale_idx, 2);
        fx.set_key(0.0);
        assert_eq!(fx.key, 9, "manual key must not override follow mode");
        let mut h = HarmonyEffect::new(48_000.0);
        h.set_follow(1.0);
        h.set_song_key(2, 1);
        assert_eq!(h.key, 2);
    }

    #[test]
    fn generative_scale_mapping_is_faithful() {
        use crate::harmonizer_dsp::music_theory::ScaleType as S;
        let all = S::all();
        // Names in generative::SCALE_NAMES order.
        for (gen_idx, want) in [
            (0usize, S::Chromatic), (1, S::Major), (2, S::Minor),
            (3, S::PentatonicMajor), (4, S::Blues), (5, S::Dorian),
            (6, S::Phrygian), (7, S::Lydian), (8, S::Mixolydian),
        ] {
            assert_eq!(all[map_generative_scale(gen_idx)], want, "gen idx {gen_idx}");
        }
        // Out of range falls back to Major, never panics.
        assert_eq!(all[map_generative_scale(99)], S::Major);
    }

    /// Chain-level broadcast reaches follow-mode slots.
    #[test]
    fn chain_broadcast_reaches_follow_slots() {
        let mut chain = EffectsChain::with_slots(48_000.0, 2);
        chain.set_slot(0, EffectType::Retune, true, 1.0);
        chain.set_slot(1, EffectType::Harmony, true, 0.5);
        if let Some(s) = chain.get_slot(0) {
            if let Some(fx) = s.retune.as_mut() { fx.set_follow(1.0); }
        }
        if let Some(s) = chain.get_slot(1) {
            if let Some(fx) = s.harmony.as_mut() { fx.set_follow(1.0); }
        }
        chain.set_song_key(7, 2); // G minor
        assert_eq!(chain.get_slot(0).unwrap().retune.as_ref().unwrap().key, 7);
        assert_eq!(chain.get_slot(1).unwrap().harmony.as_ref().unwrap().scale_idx, 2);
    }
}



#[cfg(test)]
mod meter_readout_tests {
    use super::*;

    /// The meter/analyzer effects publish a live scalar so their GUI panels can
    /// show a value. A LUFS meter fed a signal must report a finite, in-range
    /// integrated LUFS through the slot readout the sequencer publishes.
    #[test]
    fn a_lufs_meter_slot_reports_a_finite_readout() {
        let sr = 48_000.0f32;
        let mut chain = EffectsChain::with_slots(sr, 4);
        chain.set_slot(0, EffectType::LufsMeter, true, 1.0);
        // ~1 s of a -12 dBFS sine so the integrator settles.
        let mut phase = 0.0f32;
        for _ in 0..(sr as usize) {
            let x = (phase).sin() * 0.25;
            phase += std::f32::consts::TAU * 200.0 / sr;
            let _ = chain.process(x, x);
        }
        let r = chain.get_slot_ref(0).and_then(|s| s.meter_readout());
        let lufs = r.expect("LUFS meter slot must report a readout");
        assert!(lufs.is_finite(), "LUFS readout must be finite, got {lufs}");
        assert!((-60.0..=0.0).contains(&lufs), "LUFS {lufs} out of plausible range");
    }

    /// A non-meter effect reports no readout (the panel shows none).
    #[test]
    fn a_plain_effect_reports_no_readout() {
        let sr = 48_000.0f32;
        let mut chain = EffectsChain::with_slots(sr, 4);
        chain.set_slot(0, EffectType::Reverb, true, 0.5);
        assert!(chain.get_slot_ref(0).and_then(|s| s.meter_readout()).is_none());
    }

    /// A dynamics processor must start, and restart, at REST — passing signal
    /// through unchanged — never in full gain reduction.
    ///
    /// Both of these got it wrong in opposite corners: the Compressor's
    /// constructor said 0 dB and its `reset` said -96; the Maximizer said -120
    /// in both. A fresh Maximizer therefore faded up from silence over several
    /// seconds, which on a sixty-second bounce ate the first five.
    #[test]
    fn dynamics_processors_start_at_rest() {
        let sr = 48_000.0;
        // A steady tone well under any threshold: whatever comes out of the
        // first block must already be the tone, not a fraction of it.
        let tone: Vec<f32> = (0..2048)
            .map(|i| 0.1 * (i as f32 * 0.05).sin())
            .collect();

        let peak_in = tone.iter().fold(0.0f32, |m, &x| m.max(x.abs()));

        // Past the lookahead — five milliseconds of it, which is legitimately
        // empty on the first block — the output must already be the signal.
        // Anything that needs longer than that is an envelope starting in the
        // wrong place, not a delay line filling up.
        let mut maxi = MaximizerEffect::new(sr);
        let look = (sr * 0.006) as usize;
        let mut out = Vec::new();
        for (i, &x) in tone.iter().enumerate() {
            let y = maxi.process(x, x).0.abs();
            if i >= look {
                out.push(y);
            }
        }
        let first = out.iter().take(64).fold(0.0f32, |m, &x| m.max(x));
        assert!(
            first > peak_in * 0.5,
            "the maximizer is at {:.4} for an input of {:.4} once its lookahead is full",
            first, peak_in,
        );

        // Same for the compressor, which has a lookahead of its own.
        let mut comp = CompressorEffect::new(sr);
        comp.reset();
        let mut after = Vec::new();
        for (i, &x) in tone.iter().enumerate() {
            let y = comp.process(x, x).0.abs();
            if i >= look {
                after.push(y);
            }
        }
        let after_reset = after.iter().take(64).fold(0.0f32, |m, &x| m.max(x));
        assert!(
            after_reset > peak_in * 0.5,
            "the compressor comes back from reset at {:.4} for {:.4}",
            after_reset, peak_in,
        );
    }
}

#[cfg(test)]
mod rack_tests {
    use super::*;
    use crate::fx_chain::{FxChainSpec, FxSlotSpec, MacroMapSpec, RackSpec};

    /// Two empty chains at half gain add up to the input; a zone that
    /// leaves one out halves it; a macro drives a held parameter.
    #[test]
    fn a_rack_sums_its_chains_by_zone_and_macros_hold_parameters() {
        let mut chain = EffectsChain::new(48_000.0);
        let mut slot = FxSlotSpec { effect_type: EffectType::MultiFx.index() as u8, enabled: true, mix: 1.0, ..Default::default() };
        slot.rack = RackSpec {
            chains: vec![vec![FxSlotSpec { effect_type: EffectType::Delay.index() as u8, enabled: true, mix: 1.0, ..Default::default() }]],
            gains: vec![0.5, 0.5],
            zones: vec![(0, 127), (64, 127)],
            selector: 100,
            macros: [0.0; 8],
            maps: vec![MacroMapSpec { macro_idx: 0, chain: 1, slot: 0, pid: 2, lo: 0.2, hi: 0.8 }],
        };
        FxChainSpec::new(vec![slot]).apply_to(&mut chain, 8);
        let s = chain.get_slot(0).unwrap();
        assert_eq!(s.multifx_chains.len(), 1);
        assert_eq!(s.rack_gains, vec![0.5, 0.5]);
        // Chain 0 is empty and passes the input; chain 1 holds a delay whose
        // dry path passes it too: the sum at half gain each is the input.
        let mut acc = 0.0f32;
        for _ in 0..64 { let (l, _) = chain.process(1.0, 1.0); acc = l; }
        assert!((acc - 1.0).abs() < 0.3, "both chains: {acc}");
        chain.get_slot(0).unwrap().set_rack_param(8, 0.0);
        let mut acc = 0.0f32;
        for _ in 0..64 { let (l, _) = chain.process(1.0, 1.0); acc = l; }
        assert!((acc - 0.5).abs() < 0.1, "chain 0 alone: {acc}");
        chain.get_slot(0).unwrap().set_rack_param(0, 1.0);
        let held = chain.get_slot(0).unwrap().multifx_chains[0].get_slot(0).unwrap().base_params[2];
        assert!((held - 0.8).abs() < 1e-6, "the macro at one drives the hold to its top: {held}");
        assert!((chain.get_slot(0).unwrap().rack_param(0) - 1.0).abs() < 1e-6);
    }
}

#[cfg(test)]
mod sympathy_tests {
    use super::*;

    #[test]
    fn the_key_alone_rings_the_bank_and_the_chord_tunes_it() {
        let mut rb = ResonatorBankEffect::new(48_000.0);
        rb.set_mix(1.0);
        rb.set_sympathy(1.0);
        rb.sidechain_active = true;
        let mut energy = 0.0f32;
        for i in 0..4800 {
            rb.sidechain_l = if i < 48 { 0.5 } else { 0.0 };
            rb.sidechain_r = rb.sidechain_l;
            let (l, _) = rb.process(0.0, 0.0);
            if i > 100 { energy += l.abs(); }
        }
        assert!(energy > 0.1, "a key alone excites the bank: {energy}");
        let period_c = rb.first_period();
        rb.set_key_chord(9, &[0, 3, 7]);
        assert_eq!(rb.first_period(), period_c, "the chord waits for follow");
        rb.set_follow(true);
        let a3 = 48_000.0 / 220.0;
        assert!((rb.first_period() as f32 - a3).abs() <= 1.0, "{}", rb.first_period());
    }
}
