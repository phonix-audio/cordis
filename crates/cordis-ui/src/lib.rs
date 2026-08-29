//! The Cordis editor: a grand piano seen from above, in egui.
//!
//! The instrument is the interface. The two microphones are dots you drag on
//! the soundboard, because `width` is literally where the model listens to the
//! plate; the strings light as they sound; the dampers lift when the pedal
//! goes down. The technician's adjustments sit around the case.
//!
//! A separate crate from the engine, and not a feature of it. Cargo unifies
//! features across a resolved dependency graph, so an optional `egui` inside
//! `cordis` would be switched on for the engine's own tests by any
//! `cargo test --workspace`. A crate boundary is the only thing that makes
//! "the engine never sees egui" true rather than merely intended.
//!
//! It also gives the orphan rule somewhere to stand: `Preset` is declared here
//! and implemented on `cordis::CordisPatch`, which is legal precisely because
//! the trait is local. The engine used to carry that impl itself, which made a
//! physics model depend on a DAW's widget layer.

pub mod app;
pub mod colors;
pub mod preset_io;
pub mod preset_picker;
pub mod header;
pub mod keyboard;
pub mod piano_geom;
pub mod scene;
pub mod theme;
pub mod vu;
pub mod widgets;

pub use app::CordisApp;

impl preset_picker::Preset for cordis::CordisPatch {
    fn preset_name(&self) -> &str { &self.name }
}
