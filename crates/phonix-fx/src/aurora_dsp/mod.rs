//! The Aurora pad engine's own reverb and chorus.
//!
//! They live in the effects crate because `effects::CinemaPlateEffect` and
//! `effects::DimensionEffect` wrap them verbatim; `phonix::aurora` re-exports
//! them from their original paths so the engine is unchanged.

pub mod chorus;
pub mod reverb;
