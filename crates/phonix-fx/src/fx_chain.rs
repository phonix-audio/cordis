//! Host-agnostic description of an insert-FX chain.
//!
//! Effects belong to the HOST, never to an engine: a sequencer track owns its
//! inserts, and a standalone binary owns a chain placed after its engine. What a
//! PRESET carries is only this *description*, which whichever host is running
//! then applies. That keeps a single implementation of the effects themselves
//! (`crate::effects::EffectsChain`) instead of every engine embedding its own.
//!
//! This module is deliberately a leaf: it depends on `crate::effects` only, so
//! engine patches can carry an `FxChainSpec` without dragging in the sequencer.
//! The conversions to/from the session model's `FxSlotState` are at the bottom
//! of this file: both types are now foreign to the application, so the orphan
//! rule wants them here, where `FxSlotSpec` is local.

use serde::{Deserialize, Serialize};

use crate::effects::{EffectType, EffectsChain};

/// One slot of a described chain. Mirrors the sequencer's `FxSlotState` field
/// for field, including `sidechain_src`, so a round-trip is lossless.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct FxSlotSpec {
    /// `EffectType` discriminant. 0 = None/empty.
    pub effect_type: u8,
    pub enabled: bool,
    /// 0..1
    pub mix: f32,
    /// Sparse `(pid, value)` pairs, same shape as `FxSlotState::params`.
    #[serde(default)]
    pub params: Vec<(u8, f32)>,
    /// Sidechain key source track index for keyed effects. `None` = self-keyed.
    #[serde(default)]
    pub sidechain_src: Option<u16>,
    /// Sub-slots, when this slot is an `EffectType::MultiFx`.
    ///
    /// A flat `Vec`, not a nested `FxChainSpec`, so the type itself cannot
    /// describe two levels of nesting: `apply_to` ignores any `nested` a
    /// sub-slot carries, and the engine's chain refuses the effect type anyway.
    /// Empty for every other effect, and `#[serde(default)]` so every spec
    /// written before this still loads.
    #[serde(default)]
    pub nested: Vec<FxSlotSpec>,
    /// The rack the slot holds beside its first chain.
    #[serde(default)]
    pub rack: RackSpec,
}

/// A macro's hold on one parameter, as `phonix_data::data::MacroMap`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct MacroMapSpec {
    pub macro_idx: u8,
    pub chain: u8,
    pub slot: u8,
    pub pid: u8,
    pub lo: f32,
    pub hi: f32,
}

/// A rack as `phonix_data::data::RackState`, in specs.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RackSpec {
    #[serde(default)]
    pub chains: Vec<Vec<FxSlotSpec>>,
    #[serde(default = "RackSpec::default_gains")]
    pub gains: Vec<f32>,
    #[serde(default = "RackSpec::default_zones")]
    pub zones: Vec<(u8, u8)>,
    #[serde(default)]
    pub selector: u8,
    #[serde(default)]
    pub macros: [f32; 8],
    #[serde(default)]
    pub maps: Vec<MacroMapSpec>,
}

impl Default for RackSpec {
    fn default() -> Self {
        Self { chains: Vec::new(), gains: vec![1.0], zones: vec![(0, 127)], selector: 0, macros: [0.0; 8], maps: Vec::new() }
    }
}

impl RackSpec {
    fn default_gains() -> Vec<f32> { vec![1.0] }
    fn default_zones() -> Vec<(u8, u8)> { vec![(0, 127)] }
    pub fn chain_count(&self) -> usize { self.chains.len() + 1 }
}

impl From<&RackSpec> for phonix_data::data::RackState {
    fn from(r: &RackSpec) -> Self {
        Self {
            chains: r.chains.iter().map(|c| c.iter().map(Into::into).collect()).collect(),
            gains: r.gains.clone(),
            zones: r.zones.clone(),
            selector: r.selector,
            macros: r.macros,
            maps: r.maps.iter().map(|m| phonix_data::data::MacroMap { macro_idx: m.macro_idx, chain: m.chain, slot: m.slot, pid: m.pid, lo: m.lo, hi: m.hi }).collect(),
        }
    }
}

impl From<&phonix_data::data::RackState> for RackSpec {
    fn from(r: &phonix_data::data::RackState) -> Self {
        Self {
            chains: r.chains.iter().map(|c| c.iter().map(Into::into).collect()).collect(),
            gains: r.gains.clone(),
            zones: r.zones.clone(),
            selector: r.selector,
            macros: r.macros,
            maps: r.maps.iter().map(|m| MacroMapSpec { macro_idx: m.macro_idx, chain: m.chain, slot: m.slot, pid: m.pid, lo: m.lo, hi: m.hi }).collect(),
        }
    }
}

impl FxSlotSpec {
    /// True when this slot carries no effect.
    pub fn is_empty(&self) -> bool {
        EffectType::from_index(self.effect_type as usize) == EffectType::None
    }
}

/// A whole insert chain, as carried by a preset.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct FxChainSpec {
    pub slots: Vec<FxSlotSpec>,
}

impl FxChainSpec {
    pub fn new(slots: Vec<FxSlotSpec>) -> Self {
        Self { slots }
    }

    /// True when there is nothing worth applying. Hosts skip empty specs so a
    /// preset without a chain never disturbs the track the user already set up.
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(FxSlotSpec::is_empty)
    }

    /// Number of leading slots actually used.
    pub fn used_slots(&self) -> usize {
        self.slots.iter().filter(|s| !s.is_empty()).count()
    }

    /// Drop slots past `max`, which is what a host with fewer slots than the
    /// spec must do. Track inserts are capped at 4 while several bundled rack
    /// presets were authored for 8-slot master racks, so truncation is real and
    /// must be explicit rather than silent at the call site.
    pub fn truncated(&self, max: usize) -> Self {
        let mut slots = self.slots.clone();
        slots.truncate(max);
        Self { slots }
    }

    /// Apply this description onto a live chain. Mirrors the per-track rebuild
    /// in `SeqEngine` (session -> `EffectsChain`) so both paths stay identical:
    /// set the slot, then push every sparse param through both the base-value
    /// store and the raw dispatcher.
    ///
    /// Slots beyond the spec are cleared, so applying a spec fully REPLACES the
    /// chain rather than merging into whatever was there.
    pub fn apply_to(&self, chain: &mut EffectsChain, max_slots: usize) {
        for i in 0..max_slots {
            match self.slots.get(i) {
                Some(slot) if !slot.is_empty() => {
                    let ft = EffectType::from_index(slot.effect_type as usize);
                    chain.set_slot(i, ft, slot.enabled, slot.mix);
                    for &(pid, val) in &slot.params {
                        if let Some(s) = chain.get_slot(i) {
                            s.set_base_value(pid, val);
                            crate::drum_fx::apply_fx_param_raw(s, pid, val);
                        }
                    }
                    // Fill the nested chain of a Multi-FX. `set_slot` above may
                    // have refused the type (a chain inside a Multi-FX rejects
                    // another), in which case there is no nested chain to fill
                    // and the sub-slots are dropped, which is the intent.
                    if !slot.nested.is_empty() {
                        if let Some(s) = chain.get_slot(i) {
                            if let Some(inner) = s.multifx.as_deref_mut() {
                                // One level only: sub-slots' own `nested` is
                                // ignored by construction, since this is not a
                                // recursive call.
                                let sub = FxChainSpec::new(slot.nested.clone());
                                sub.apply_to(inner, crate::effects::MULTIFX_SLOTS);
                            }
                        }
                    }
                    if ft == EffectType::MultiFx {
                        if let Some(s) = chain.get_slot(i) { s.apply_rack(&slot.rack); }
                    }
                }
                // Empty or missing: clear the slot so the chain matches the spec.
                _ => chain.set_slot(i, EffectType::None, false, 1.0),
            }
        }
    }
}

// Conversions to/from the session model's `FxSlotState`. They used to live in
// `phonix::sequencer::data`; once the model moved to `phonix-data` and the
// specs here, the application owned neither type and the orphan rule pushed
// them to this side. `phonix-data` is a leaf, so this costs the module nothing.
impl From<&FxSlotSpec> for phonix_data::data::FxSlotState {
    fn from(s: &FxSlotSpec) -> Self {
        Self {
            effect_type: s.effect_type,
            enabled: s.enabled,
            mix: s.mix,
            params: s.params.clone(),
            sidechain_src: s.sidechain_src,
            nested: s.nested.iter().map(Into::into).collect(),
            rack: (&s.rack).into(),
        }
    }
}

impl From<&phonix_data::data::FxSlotState> for FxSlotSpec {
    fn from(s: &phonix_data::data::FxSlotState) -> Self {
        Self {
            effect_type: s.effect_type,
            enabled: s.enabled,
            mix: s.mix,
            params: s.params.clone(),
            sidechain_src: s.sidechain_src,
            nested: s.nested.iter().map(Into::into).collect(),
            rack: (&s.rack).into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec_with(effect_type: u8, params: Vec<(u8, f32)>) -> FxChainSpec {
        FxChainSpec::new(vec![FxSlotSpec {
            effect_type,
            enabled: true,
            mix: 0.75,
            params,
            sidechain_src: None,
            nested: Vec::new(),
            rack: Default::default(),
        }])
    }

    #[test]
    fn empty_spec_is_detected() {
        assert!(FxChainSpec::default().is_empty());
        // EffectType index 0 is None.
        assert!(spec_with(0, vec![]).is_empty());
        let delay = EffectType::Delay.index() as u8;
        assert!(!spec_with(delay, vec![]).is_empty());
    }

    /// The 5 faithful engine-extracted effects must build their correct instance
    /// via `apply_to` (guards the hard-coded all() indices), and their pid
    /// ALIASING (Ensemble/Dimension reuse Chorus slots 20-23; the reverbs reuse
    /// Reverb slots 10-14) must NOT cross-talk with a real Chorus/Reverb in an
    /// adjacent slot — `apply_fx_param_raw` gates on the slot's effect type.
    #[test]
    fn faithful_effects_build_correctly_and_aliased_pids_dont_crosstalk() {
        use crate::fx_params::pid;
        let sr = 48_000.0;

        // Each new effect builds the matching instance (round-trips through index).
        for t in [EffectType::Ensemble, EffectType::SpringTank, EffectType::CinemaPlate,
                  EffectType::SolinaVerb, EffectType::Dimension] {
            let mut chain = EffectsChain::with_slots(sr, 2);
            spec_with(t.index() as u8, vec![]).apply_to(&mut chain, 2);
            assert_eq!(chain.get_slot(0).map(|s| s.effect_type), Some(t),
                "{t:?} did not build via apply_to (index drift?)");
        }

        // Ensemble (pid 21/22 = Chorus slots) beside a real Chorus (same pids):
        // each keeps its own base_params, no bleed.
        let mut chain = EffectsChain::with_slots(sr, 2);
        FxChainSpec::new(vec![
            FxSlotSpec { effect_type: EffectType::Ensemble.index() as u8, enabled: true, mix: 1.0,
                params: vec![(pid::ENSEMBLE_DEPTH, 0.83)], sidechain_src: None, nested: Vec::new(), rack: Default::default() },
            FxSlotSpec { effect_type: EffectType::Chorus.index() as u8, enabled: true, mix: 0.4,
                params: vec![(pid::CHORUS_DEPTH, 0.22)], sidechain_src: None, nested: Vec::new(), rack: Default::default() },
        ]).apply_to(&mut chain, 2);
        assert!(chain.get_slot(0).unwrap().ensemble.is_some() && chain.get_slot(0).unwrap().chorus.is_none());
        assert!(chain.get_slot(1).unwrap().chorus.is_some() && chain.get_slot(1).unwrap().ensemble.is_none());
        assert!((chain.get_slot(0).unwrap().base_params[pid::ENSEMBLE_DEPTH as usize] - 0.83).abs() < 1e-6);
        assert!((chain.get_slot(1).unwrap().base_params[pid::CHORUS_DEPTH as usize] - 0.22).abs() < 1e-6);
    }

    #[test]
    fn apply_sets_the_slot_and_clears_the_rest() {
        let mut chain = EffectsChain::with_slots(48_000.0, 4);
        let delay = EffectType::Delay.index() as u8;
        spec_with(delay, vec![(0, 0.4)]).apply_to(&mut chain, 4);
        // Slot 0 carries the effect, the others were cleared.
        assert_eq!(chain.get_slot(0).map(|s| s.effect_type), Some(EffectType::Delay));
        for i in 1..4 {
            assert_eq!(chain.get_slot(i).map(|s| s.effect_type), Some(EffectType::None));
        }
    }

    #[test]
    fn applying_replaces_rather_than_merges() {
        let mut chain = EffectsChain::with_slots(48_000.0, 4);
        let reverb = EffectType::Reverb.index() as u8;
        spec_with(reverb, vec![]).apply_to(&mut chain, 4);
        assert_eq!(chain.get_slot(0).map(|s| s.effect_type), Some(EffectType::Reverb));
        // A different spec must not leave the previous effect behind.
        let delay = EffectType::Delay.index() as u8;
        spec_with(delay, vec![]).apply_to(&mut chain, 4);
        assert_eq!(chain.get_slot(0).map(|s| s.effect_type), Some(EffectType::Delay));
    }

    #[test]
    fn truncation_is_explicit() {
        let slot = FxSlotSpec { effect_type: 1, enabled: true, mix: 1.0, ..Default::default() };
        let spec = FxChainSpec::new(vec![slot.clone(), slot.clone(), slot.clone(), slot.clone(), slot]);
        assert_eq!(spec.slots.len(), 5);
        assert_eq!(spec.truncated(4).slots.len(), 4);
    }

    fn multifx_spec(sub: Vec<FxSlotSpec>) -> FxChainSpec {
        FxChainSpec::new(vec![FxSlotSpec {
            effect_type: EffectType::MultiFx.index() as u8,
            enabled: true,
            mix: 1.0,
            params: vec![],
            sidechain_src: None,
            nested: sub,
            rack: Default::default(),
        }])
    }

    fn leaf(t: EffectType) -> FxSlotSpec {
        FxSlotSpec { effect_type: t.index() as u8, enabled: true, mix: 1.0, ..Default::default() }
    }

    #[test]
    fn a_multifx_slot_fills_its_nested_chain() {
        let mut chain = EffectsChain::with_slots(48_000.0, 4);
        multifx_spec(vec![leaf(EffectType::Delay), leaf(EffectType::Reverb)])
            .apply_to(&mut chain, 4);
        let slot = chain.get_slot(0).expect("slot 0");
        assert_eq!(slot.effect_type, EffectType::MultiFx);
        let inner = slot.multifx.as_deref_mut().expect("nested chain");
        assert_eq!(inner.get_slot(0).map(|s| s.effect_type), Some(EffectType::Delay));
        assert_eq!(inner.get_slot(1).map(|s| s.effect_type), Some(EffectType::Reverb));
    }

    /// The user's rule: a Multi-FX may not chain Multi-FX.
    ///
    /// Enforced in `EffectsChain::set_slot` rather than by checking specs at
    /// each call site, so a hand-edited `.phx` or a future caller cannot get
    /// past it. The cost of a miss is unbounded recursion per sample.
    #[test]
    fn a_multifx_cannot_contain_another_multifx() {
        let mut chain = EffectsChain::with_slots(48_000.0, 4);
        multifx_spec(vec![leaf(EffectType::MultiFx), leaf(EffectType::Delay)])
            .apply_to(&mut chain, 4);
        let inner = chain.get_slot(0).unwrap().multifx.as_deref_mut().unwrap();
        assert_eq!(inner.get_slot(0).map(|s| s.effect_type), Some(EffectType::None),
            "a nested chain must refuse to host a Multi-FX");
        // The rest of the sub-chain still lands.
        assert_eq!(inner.get_slot(1).map(|s| s.effect_type), Some(EffectType::Delay));
        // And the nesting really did stop: no third level exists.
        assert!(inner.get_slot(0).unwrap().multifx.is_none());
    }

    #[test]
    fn nested_slots_survive_a_serde_roundtrip() {
        let spec = multifx_spec(vec![leaf(EffectType::Delay)]);
        let json = serde_json::to_string(&spec).unwrap();
        let back: FxChainSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, spec);
    }

    /// Specs written before Multi-FX existed must still load.
    #[test]
    fn a_spec_without_the_nested_field_still_loads() {
        let json = r#"{"slots":[{"effect_type":1,"enabled":true,"mix":0.5}]}"#;
        let back: FxChainSpec = serde_json::from_str(json).unwrap();
        assert_eq!(back.slots.len(), 1);
        assert!(back.slots[0].nested.is_empty());
    }

    #[test]
    fn serde_roundtrip_keeps_sparse_params() {
        let delay = EffectType::Delay.index() as u8;
        let spec = spec_with(delay, vec![(3, 0.25), (7, 0.9)]);
        let json = serde_json::to_string(&spec).unwrap();
        let back: FxChainSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, spec);
    }
}
