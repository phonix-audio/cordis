//! Shared FX primitives.
//!
//! Plugin-agnostic effects hoisted out of individual engines for reuse.
//! See `convolution` for partitioned FFT IR convolution and `irs` for
//! the algorithmic IR library (6 stereo IRs: cathedral / concrete /
//! plate / wooden hall / spring / ambient room).
//!
//! These are PLUGIN reverb primitives, not track / master FX rack
//! entries — for that see `crate::effects`.

pub mod convolution;
pub mod irs;

pub use phonix_data::fx::ReverbKind;
