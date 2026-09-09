//! Shared DSP primitives.
//!
//! Plugin-agnostic building blocks hoisted out of individual engines
//! so other plugins can compose them. See `samples` for the SampleAsset
//! shape + Aurora's procedural library, `sample_player` for pitch-
//! tracked looped playback, and `granular` for the 16-grain scheduler.

pub mod samples;
pub mod sample_player;
pub mod granular;
pub mod fastmath;
pub mod filters;
pub mod lfo;
// Magma's own LFO/ducker cores. DISTINCT from `lfo::Lfo` above (different
// shape set, sine approximation and sync table) — deliberately NOT merged.
pub mod ducker;
pub mod vibrato;
pub mod oscillator;
// Solstice's 16-bank wavetable oscillator (Aurora and Pulsar read the same banks).
pub mod wavetable;
// Loquace's ADSR. DISTINCT from any other envelope in the tree — not merged.
pub mod adsr;

/// Distinct from `adsr`: this is the envelope generator the Synth B lineage and
/// four instrument engines share. The two are NOT interchangeable and must not
/// be merged — they differ in curve and in retrigger behaviour.
pub mod envelope;
// TranceVoice's FOF choir (Nebula layers it too).
pub mod fof;
// Techno-Kick's RMS compressor / limiter (Magma reuses it).
pub mod rms_compressor;
// The Drum Machine's synthesized rock-kit voices (Canon909 reuses them).
// Engine-agnostic modulation sources, and the ADSR / LFO parameter shapes.
pub mod mod_sources;
pub mod synth_params;
pub mod spectral_resynth;
pub mod pitch;
pub mod reverb;
pub mod chorus;
pub mod meters;
