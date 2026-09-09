//! Running the chain a patch describes.
//!
//! The description lives in `cordis::fx`, with the presets that carry it. What
//! is here is the half that needs a live `Chain`, which is the host's and
//! never the engine's.

use phonix_fx::{ApplyReport, Chain, ChainSpec, Registry};

/// The four kinds this build ships, and nothing else.
pub fn registry() -> Registry {
    Registry::builtin()
}

/// Build `spec` into `chain`, replacing whatever it held. Allocates: called
/// on a preset change and on an edit, never per block.
pub fn apply(chain: &mut Chain, spec: &ChainSpec) -> ApplyReport {
    chain.apply(spec, &registry())
}

/// Empty the chain. `Init` is the absence of a factory preset, so it is the
/// absence of the chain one carries.
pub fn disengage(chain: &mut Chain) {
    chain.apply(&ChainSpec::default(), &registry());
}

#[cfg(test)]
mod tests {
    use super::*;
    use cordis::fx::{concert_hall, FX_SLOTS};
    use phonix_fx::{Musical, Transport, Value};

    const SR: f32 = 48_000.0;
    const BLOCK: usize = 256;
    const SLOT_EQ: usize = 0;
    const SLOT_COMP: usize = 1;
    const SLOT_ROOM: usize = 2;

    fn run(chain: &mut Chain, l: &mut [f32], r: &mut [f32]) {
        chain.process(l, r, &[], Transport::default(), Musical::default());
    }

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
        let mut chain = Chain::new(SR, BLOCK);
        assert!(chain.is_idle());
        assert_eq!(chain.latency_samples(), 0);
        let (mut l, mut r) = tone(2048, 0.5);
        let (l0, r0) = (l.clone(), r.clone());
        run(&mut chain, &mut l, &mut r);
        assert_eq!(l, l0);
        assert_eq!(r, r0);
    }

    /// The recipe's starting values, which a freshly loaded preset gets.
    #[test]
    fn the_recipe_starts_each_effect_where_it_says() {
        let mut chain = Chain::new(SR, BLOCK);
        assert!(apply(&mut chain, &concert_hall()).is_clean());
        let at = |c: &Chain, slot: usize, id: &str| c.param(c.param_ref(slot, id).unwrap()).unwrap();
        assert_eq!(at(&chain, SLOT_EQ, "band.0.gain"), Value::F(-3.0));
        assert_eq!(at(&chain, SLOT_COMP, "threshold"), Value::F(-18.0));
        assert_eq!(at(&chain, SLOT_COMP, "mode"), Value::E("bus"));
        assert_eq!(chain.mix(SLOT_ROOM), 0.22);
        assert_eq!(chain.latency_samples(), 239, "only the limiter's lookahead");
    }

    #[test]
    fn applying_the_recipe_fills_the_slots() {
        let mut chain = Chain::new(SR, BLOCK);
        apply(&mut chain, &concert_hall());
        assert_eq!(chain.len(), FX_SLOTS);
        assert!(!chain.is_idle());
        assert!(!chain.is_placeholder(SLOT_COMP));
    }

    /// Stepping back to `Init` takes the chain with it.
    #[test]
    fn disengaging_empties_the_chain() {
        let mut chain = Chain::new(SR, BLOCK);
        apply(&mut chain, &concert_hall());
        assert!(!chain.is_idle());
        disengage(&mut chain);
        assert!(chain.is_idle());
        let (mut l, mut r) = tone(1024, 0.5);
        let (l0, r0) = (l.clone(), r.clone());
        run(&mut chain, &mut l, &mut r);
        assert_eq!(l, l0);
        assert_eq!(r, r0);
    }

    #[test]
    fn the_engaged_chain_renders() {
        let mut chain = Chain::new(SR, BLOCK);
        apply(&mut chain, &concert_hall());
        let (mut l, mut r) = tone(4096, 0.3);
        let l0 = l.clone();
        run(&mut chain, &mut l, &mut r);
        let diff = l.iter().zip(&l0).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
        assert!(diff > 1e-4, "engaged chain left the signal alone (max diff {diff})");
    }

    /// What the ceiling is there for: the three slots above it can add gain,
    /// and the worst case the dial allows must still not leave full scale.
    #[test]
    fn the_curated_chain_stays_under_the_ceiling() {
        let mut chain = Chain::new(SR, BLOCK);
        apply(&mut chain, &concert_hall());
        let ceiling = 10.0_f32.powf(-0.3 / 20.0) * 1.02;
        let (mut l, mut r) = tone(48_000, 0.99);
        run(&mut chain, &mut l, &mut r);
        let peak = l.iter().chain(r.iter()).fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak <= ceiling, "peak {peak} above the ceiling {ceiling}");
    }
}
