//! Phonix's effects layer.
//!
//! Everything that processes audio *after* an engine has made it: the FX rack
//! (`effects`), the host-agnostic chain description a preset carries
//! (`fx_chain`), the factory preset banks (`fx_presets`), the flat parameter
//! space every UI speaks (`fx_params`) and the appliers that write it into a
//! live slot (`drum_fx`), the convolution/IR reverb primitives (`fx`), and the
//! loudness/true-peak meters the dynamics effects read (`meters`).
//!
//! **One copy, one place.** Several effect types wrap DSP that was originally
//! written for a single instrument — the Aurora hall and chorus, the VP-330
//! ensemble and reverb, the VCS-3 spring tank, the harmonizer's pitch
//! machinery. Those leaf modules live HERE (`aurora_dsp`, `vp330_dsp`,
//! `spring_reverb`, `harmonizer_dsp`) and the plugins that own them re-export
//! them from their old paths, so the code exists exactly once.
//!
//! No egui: the drawing half of the FX controls stays in the application.

// The shared DSP primitives, re-exported so every `crate::dsp::...` path
// inside this crate resolves exactly as it did in the application.
pub use phonix_dsp as dsp;

pub mod aurora_dsp;
pub mod drum_fx;
pub mod effects;
pub mod fx;
pub mod fx_chain;
pub mod fx_params;
pub mod fx_presets;
pub mod harmonizer_dsp;
pub mod meters;
pub mod spring_reverb;
pub mod vp330_dsp;
