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

/// The chain as it was against the chain as it is, on the same audio. Exists
/// while both crates are in the tree; leaves with the old one.
#[cfg(test)]
mod ab {
    use super::*;
    use phonix_fx::{Musical, Transport};
    use phonix_fx_old::effects::{CompressorMode, EffectType, EffectsChain};
    use phonix_fx_old::fx_chain::{FxChainSpec, FxSlotSpec};
    use phonix_fx_old::fx_params::pid;

    const SR: f32 = 48_000.0;

    /// The old recipe, as `cordis::fx::chain` wrote it before names.
    fn old_slots() -> Vec<FxSlotSpec> {
        let slot = |t: EffectType, params: Vec<(u8, f32)>| FxSlotSpec { effect_type: t.index() as u8, enabled: true, mix: 1.0, params, ..Default::default() };
        vec![
            slot(EffectType::ParametricEq, vec![(pid::EQ_B0_FREQ, 90.0), (pid::EQ_B0_Q, 0.7), (pid::EQ_B0_GAIN, -3.0), (pid::EQ_ENABLE_MASK, 1.0), (pid::EQ_MIX, 1.0)]),
            slot(EffectType::Compressor, vec![(pid::COMP_THRESH, -18.0), (pid::COMP_RATIO, 2.0), (pid::COMP_ATTACK, 0.020), (pid::COMP_RELEASE, 0.150), (pid::COMP_KNEE, 6.0), (pid::COMP_MIX, 1.0)]),
            slot(EffectType::Reverb, vec![(pid::REVERB_TYPE, 1.0), (pid::REVERB_SIZE, 0.35), (pid::REVERB_DECAY, 0.40), (pid::REVERB_DAMP, 0.50), (pid::REVERB_PREDELAY, 0.008), (pid::REVERB_WIDTH, 1.0), (pid::REVERB_MIX, 0.22)]),
            slot(EffectType::BrickwallLimiter, vec![(pid::BRICK_CEILING_DB, -0.3), (pid::BRICK_RELEASE_MS, 50.0), (pid::BRICK_MIX, 1.0)]),
        ]
    }

    fn old_chain(slots: Vec<FxSlotSpec>) -> EffectsChain {
        let mut chain = EffectsChain::new(SR);
        let n = slots.len();
        FxChainSpec::new(slots).apply_to(&mut chain, n);
        for i in 0..n {
            if let Some(s) = chain.get_slot(i) {
                if let Some(c) = s.compressor.as_mut() {
                    c.set_mode(CompressorMode::Bus);
                }
            }
        }
        chain
    }

    fn new_chain(spec: &phonix_fx::ChainSpec) -> Chain {
        let mut chain = Chain::new(SR, 256);
        let report = apply(&mut chain, spec);
        assert!(report.is_clean(), "{report}");
        chain
    }

    /// Two seconds: a chord's worth of tone with an impulse and a burst, loud
    /// enough to reach the limiter, and a silent tail for the room.
    fn material() -> (Vec<f32>, Vec<f32>) {
        let n = (SR * 2.0) as usize;
        let mut l = vec![0.0_f32; n];
        let mut r = vec![0.0_f32; n];
        for i in 0..n {
            let t = i as f32 / SR;
            let env = if t < 1.2 { 1.0 } else { 0.0 };
            let tone = 0.5 * (t * 220.0 * std::f32::consts::TAU).sin() + 0.3 * (t * 277.18 * std::f32::consts::TAU).sin() + 0.2 * (t * 329.63 * std::f32::consts::TAU).sin();
            l[i] = env * tone;
            r[i] = env * tone * 0.8 + if i == 1000 { 0.9 } else { 0.0 };
        }
        (l, r)
    }

    fn max_diff(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f32::max)
    }

    fn render_old(mut chain: EffectsChain) -> (Vec<f32>, Vec<f32>) {
        let (mut l, mut r) = material();
        for start in (0..l.len()).step_by(256) {
            let end = (start + 256).min(l.len());
            chain.process_block_stereo(&mut l[start..end], &mut r[start..end]);
        }
        (l, r)
    }

    fn render_new(mut chain: Chain) -> (Vec<f32>, Vec<f32>) {
        let (mut l, mut r) = material();
        for start in (0..l.len()).step_by(256) {
            let end = (start + 256).min(l.len());
            chain.process(&mut l[start..end], &mut r[start..end], &[], Transport::default(), Musical::default());
        }
        (l, r)
    }

    /// Each effect alone, at mix 1, within 1e-6.
    #[test]
    fn each_effect_renders_what_it_rendered() {
        let old = old_slots();
        let new = cordis::fx::concert_hall();
        for (i, name) in ["parametric-eq", "compressor", "reverb", "brickwall-limiter"].iter().enumerate() {
            let mut o = old[i].clone();
            for (p, v) in o.params.iter_mut() {
                if *p == pid::REVERB_MIX { *v = 1.0; }
            }
            let mut n = new.slots[i].clone();
            n.mix = 1.0;
            let (ol, or) = render_old(old_chain(vec![o]));
            let (nl, nr) = render_new(new_chain(&phonix_fx::ChainSpec::new(vec![n])));
            let d = max_diff(&ol, &nl).max(max_diff(&or, &nr));
            assert!(d < 1e-6, "{name}: max difference {d}");
        }
    }

    /// The whole recipe, room at 0.22, within 1e-5.
    #[test]
    fn the_whole_chain_renders_what_it_rendered() {
        let (ol, or) = render_old(old_chain(old_slots()));
        let (nl, nr) = render_new(new_chain(&cordis::fx::concert_hall()));
        let d = max_diff(&ol, &nl).max(max_diff(&or, &nr));
        assert!(d < 1e-5, "max difference {d}");
        let peak = ol.iter().fold(0.0_f32, |m, v| m.max(v.abs()));
        assert!(peak > 0.5, "the material did not reach the chain: peak {peak}");
    }
}
