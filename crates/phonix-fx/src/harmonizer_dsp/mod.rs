//! The harmonizer's pitch machinery.
//!
//! YIN detection, PSOLA-style shifting, scale/chord theory and the autotune
//! corrector. `effects::RetuneEffect` and `effects::HarmonyEffect` are built
//! on them, so the one copy lives here; `phonix::harmonizer` re-exports each
//! module from its original path.

pub mod autotune;
pub mod music_theory;
pub mod pitch_detect;
pub mod pitch_shift;
pub mod voice;
