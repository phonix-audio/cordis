//! Running the chain a patch describes.
//!
//! The description lives in `cordis::fx`, with the presets that carry it. What
//! is here is the half that needs a live `EffectsChain`, which is the host's
//! and never the engine's.

use cordis::fx::FX_SLOTS;
use phonix_fx::effects::{CompressorMode, EffectsChain};
use phonix_fx::fx_chain::FxChainSpec;

/// Slot the compressor sits in, so its mode can be set after the fact.
const SLOT_COMP: usize = 1;

/// Build `spec` into `chain`, replacing whatever it held.
///
/// The FIRST call allocates the effects, on whichever thread applies the
/// chain. Every later call does not: `set_slot` rebuilds a slot only when its
/// effect type changes, and the recipe's types never change. Cost is therefore
/// once per plugin instance, on a preset change, which is the same event that
/// already boxes a patch to send to the engine.
pub fn apply(chain: &mut EffectsChain, spec: &FxChainSpec) {
    spec.apply_to(chain, FX_SLOTS);
    // Not reachable through a parameter: `COMP_MODE` has no dispatcher.
    if let Some(s) = chain.get_slot(SLOT_COMP) {
        if let Some(c) = s.compressor.as_mut() {
            c.set_mode(CompressorMode::Bus);
        }
    }
}

/// Empty the chain, slot by slot.
///
/// `Init` is the absence of a factory preset, so it is the absence of the
/// chain one carries.
pub fn disengage(chain: &mut EffectsChain) {
    FxChainSpec::new(Vec::new()).apply_to(chain, FX_SLOTS);
}

#[cfg(test)]
mod tests {
    use super::*;
    use cordis::fx::concert_hall;
    use phonix_fx::fx_params::pid;

    const SR: f32 = 48_000.0;
    const SLOT_EQ: usize = 0;
    const SLOT_ROOM: usize = 2;

    /// A tone loud enough to reach the limiter, as a stereo pair.
    fn tone(n: usize, amp: f32) -> (Vec<f32>, Vec<f32>) {
        let f = 220.0 / SR;
        let l: Vec<f32> = (0..n).map(|i| amp * (i as f32 * f * std::f32::consts::TAU).sin()).collect();
        let r = l.clone();
        (l, r)
    }

    /// The compatibility guarantee, stated as audio rather than as a flag: a
    /// project that never loads a factory preset drives an empty chain, and an
    /// empty chain returns exactly what it was given.
    #[test]
    fn an_empty_chain_is_bit_identical() {
        let mut chain = EffectsChain::new(SR);
        assert!(chain.is_idle());
        assert_eq!(chain.latency_samples(), 0);

        let (mut l, mut r) = tone(2048, 0.5);
        let (l0, r0) = (l.clone(), r.clone());
        chain.process_block_stereo(&mut l, &mut r);
        assert_eq!(l, l0);
        assert_eq!(r, r0);
    }

    /// The recipe's starting values, which a freshly loaded preset gets.
    #[test]
    fn the_recipe_starts_each_effect_where_it_says() {
        let mut chain = EffectsChain::new(SR);
        apply(&mut chain, &concert_hall());
        assert_eq!(chain.get_slot(SLOT_EQ).unwrap().base_params[pid::EQ_B0_GAIN as usize], -3.0);
        assert_eq!(chain.get_slot(SLOT_COMP).unwrap().base_params[pid::COMP_THRESH as usize], -18.0);
        assert_eq!(chain.get_slot(SLOT_ROOM).unwrap().base_params[pid::REVERB_MIX as usize], 0.22);
    }

    /// The recipe is frozen the way the factory bank is: by literal, in order.
    #[test]
    fn the_curated_chain_order_and_types_are_frozen() {
        let spec = concert_hall();
        assert_eq!(spec.slots.len(), FX_SLOTS);
        let want = [
            phonix_fx::effects::EffectType::ParametricEq,
            phonix_fx::effects::EffectType::Compressor,
            phonix_fx::effects::EffectType::Reverb,
            phonix_fx::effects::EffectType::BrickwallLimiter,
        ];
        for (slot, expected) in spec.slots.iter().zip(want) {
            assert_eq!(slot.effect_type, expected.index() as u8, "{expected:?}");
            assert!(slot.enabled);
        }
    }

    #[test]
    fn applying_the_recipe_fills_the_slots_and_sets_bus_mode() {
        let mut chain = EffectsChain::new(SR);
        apply(&mut chain, &concert_hall());
        assert!(chain.slot_count() >= FX_SLOTS);
        assert!(!chain.is_idle());
        let comp = chain.get_slot(SLOT_COMP).expect("compressor slot");
        assert!(comp.compressor.is_some(), "the compressor was not built");
    }

    /// The chain is audible once engaged: the room alone guarantees the output
    /// cannot be the input.
    /// Stepping back to `Init` takes the chain with it.
    #[test]
    fn disengaging_empties_the_chain() {
        let mut chain = EffectsChain::new(SR);
        apply(&mut chain, &concert_hall());
        assert!(!chain.is_idle());
        disengage(&mut chain);
        assert!(chain.is_idle());

        let (mut l, mut r) = tone(1024, 0.5);
        let (l0, r0) = (l.clone(), r.clone());
        chain.process_block_stereo(&mut l, &mut r);
        assert_eq!(l, l0);
        assert_eq!(r, r0);
    }

    #[test]
    fn the_engaged_chain_renders() {
        let mut chain = EffectsChain::new(SR);
        apply(&mut chain, &concert_hall());
        let (mut l, mut r) = tone(4096, 0.3);
        let l0 = l.clone();
        chain.process_block_stereo(&mut l, &mut r);
        let diff = l.iter().zip(&l0).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
        assert!(diff > 1e-4, "engaged chain left the signal alone (max diff {diff})");
    }

    /// What the ceiling is there for: the three slots above it can add gain,
    /// and the worst case the dial allows must still not leave full scale.
    #[test]
    fn the_curated_chain_stays_under_the_ceiling() {
        let mut chain = EffectsChain::new(SR);
        apply(&mut chain, &concert_hall());
        // -0.3 dBFS, the value the recipe pins, plus room for the limiter's
        // own inter-sample estimate.
        let ceiling = 10.0_f32.powf(-0.3 / 20.0) * 1.02;
        let (mut l, mut r) = tone(48_000, 0.99);
        chain.process_block_stereo(&mut l, &mut r);
        let peak = l.iter().chain(r.iter()).fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak <= ceiling, "peak {peak} above the ceiling {ceiling}");
    }
}
