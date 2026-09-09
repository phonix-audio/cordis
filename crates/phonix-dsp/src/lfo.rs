//! Shared low-frequency oscillator.
//!
//! Multi-waveform LFO (sine / triangle / saw / square / sample-and-hold /
//! smoothed random) with a cached last value so a mod matrix can read it at
//! control rate without advancing the phase twice.
//!
//! Hoisted out of the Polaris engine, which several engines already imported.
//! The generator is engine-neutral; the *destination* of the modulation is not,
//! so it stays with each engine. This type only carries an `enabled` gate:
//! set it from `(destination != Off)` and route the value yourself.

use std::f32::consts::TAU;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LfoWaveform {
    Sine,
    Triangle,
    Saw,
    Square,
    SampleHold,
    Random,
}

impl LfoWaveform {
    pub const ALL: &'static [LfoWaveform] = &[
        LfoWaveform::Sine,
        LfoWaveform::Triangle,
        LfoWaveform::Saw,
        LfoWaveform::Square,
        LfoWaveform::SampleHold,
        LfoWaveform::Random,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            LfoWaveform::Sine => "Sine",
            LfoWaveform::Triangle => "Tri",
            LfoWaveform::Saw => "Saw",
            LfoWaveform::Square => "Sqr",
            LfoWaveform::SampleHold => "S&H",
            LfoWaveform::Random => "Rnd",
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i {
            0 => LfoWaveform::Sine,
            1 => LfoWaveform::Triangle,
            2 => LfoWaveform::Saw,
            3 => LfoWaveform::Square,
            4 => LfoWaveform::SampleHold,
            5 => LfoWaveform::Random,
            _ => LfoWaveform::Sine,
        }
    }

    pub fn index(&self) -> usize {
        match self {
            LfoWaveform::Sine => 0,
            LfoWaveform::Triangle => 1,
            LfoWaveform::Saw => 2,
            LfoWaveform::Square => 3,
            LfoWaveform::SampleHold => 4,
            LfoWaveform::Random => 5,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Lfo {
    pub rate: f32,
    pub depth: f32,
    pub waveform: LfoWaveform,
    /// Gate: when false, `tick()` returns 0 and does not advance. Engines set
    /// this from their own `destination != Off`.
    pub enabled: bool,
    pub key_sync: bool,

    phase: f32,
    sh_value: f32,
    sh_rng: u32,
    // Random waveform: smoothed interpolation between targets
    random_current: f32,
    random_target: f32,
    sample_rate: f32,
    /// Last value returned by `tick()` — exposed so the mod matrix
    /// can read the LFO at control rate without advancing phase.
    last_value: f32,
}

impl Lfo {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            rate: 1.0,
            depth: 0.0,
            waveform: LfoWaveform::Sine,
            enabled: false,
            key_sync: false,
            phase: 0.0,
            sh_value: 0.0,
            sh_rng: 0x1234_ABCD,
            random_current: 0.0,
            random_target: 0.0,
            sample_rate,
            last_value: 0.0,
        }
    }

    /// Read the last computed LFO value (the cached output from the
    /// most recent `tick()` call). Used by the mod matrix at control
    /// rate so it doesn't advance the LFO phase a second time.
    #[inline(always)]
    pub fn current_value(&self) -> f32 { self.last_value }

    pub fn reset_phase(&mut self) {
        self.phase = 0.0;
    }

    #[inline(always)]
    fn next_rand(&mut self) -> f32 {
        self.sh_rng ^= self.sh_rng << 13;
        self.sh_rng ^= self.sh_rng >> 17;
        self.sh_rng ^= self.sh_rng << 5;
        (self.sh_rng as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// Advance LFO and return value in -1..1 (scaled by depth)
    pub fn tick(&mut self) -> f32 {
        if !self.enabled || self.depth < 0.001 {
            self.last_value = 0.0;
            return 0.0;
        }

        let inc = self.rate / self.sample_rate;
        self.phase += inc;
        let wrapped = self.phase >= 1.0;
        if wrapped {
            self.phase -= 1.0;
            // Update S&H / Random target on phase wrap
            self.sh_value = self.next_rand();
            self.random_target = self.next_rand();
        }

        let raw = match self.waveform {
            LfoWaveform::Sine => {
                (self.phase * TAU).sin()
            }
            LfoWaveform::Triangle => {
                let p = self.phase;
                if p < 0.25 { p * 4.0 }
                else if p < 0.75 { 2.0 - p * 4.0 }
                else { p * 4.0 - 4.0 }
            }
            LfoWaveform::Saw => {
                2.0 * self.phase - 1.0
            }
            LfoWaveform::Square => {
                if self.phase < 0.5 { 1.0 } else { -1.0 }
            }
            LfoWaveform::SampleHold => {
                self.sh_value
            }
            LfoWaveform::Random => {
                // Smoothly interpolate toward random target
                let alpha = (inc * 4.0).min(1.0);
                self.random_current += (self.random_target - self.random_current) * alpha;
                self.random_current
            }
        };

        let out = raw * self.depth;
        self.last_value = out;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Golden hashes captured from the hand-rolled `PolarisLfo` before it moved
    /// here (rate 200, depth 0.8, enabled). Any arithmetic drift breaks them.
    #[test]
    fn is_byte_identical_to_the_legacy_nord_lfo() {
        let golden: [u64; 6] = [
            0xadac2e39ada080ab, // Sine
            0x6baa07e0fc75171e, // Triangle
            0x2d2d3c3cc09914a8, // Saw
            0x57c73fce56833f46, // Square
            0xa2e08a620a9d3168, // SampleHold
            0x025509523c5ad093, // Random
        ];
        for wf in 0..6 {
            let mut lfo = Lfo::new(44100.0);
            lfo.waveform = LfoWaveform::from_index(wf);
            lfo.enabled = true;
            lfo.rate = 200.0;
            lfo.depth = 0.8;
            let mut h = 0u64;
            for _ in 0..4000 { h = h.rotate_left(7) ^ lfo.tick().to_bits() as u64; }
            assert_eq!(h, golden[wf], "LFO waveform {wf} drifted from the legacy PolarisLfo");
        }
    }
}
