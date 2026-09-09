//! Shared anti-aliased virtual-analog oscillator (PolyBLEP), with hard-sync
//! reporting, FM, and continuous waveform morphing.
//!
//! Hoisted out of the Polaris engine, which Solstice already imported. The
//! generator is engine-neutral; keep engine-specific waveform *sets* or routing
//! in the engine.

use std::f32::consts::{PI, TAU};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waveform {
    Saw,
    Pulse,
    Triangle,
    Sine,
    Noise,
}

impl Waveform {
    pub const ALL: &'static [Waveform] = &[
        Waveform::Saw,
        Waveform::Pulse,
        Waveform::Triangle,
        Waveform::Sine,
        Waveform::Noise,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Waveform::Saw => "Saw",
            Waveform::Pulse => "Pulse",
            Waveform::Triangle => "Tri",
            Waveform::Sine => "Sine",
            Waveform::Noise => "Noise",
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Waveform::Saw,
            1 => Waveform::Pulse,
            2 => Waveform::Triangle,
            3 => Waveform::Sine,
            4 => Waveform::Noise,
            _ => Waveform::Saw,
        }
    }

    pub fn index(&self) -> usize {
        match self {
            Waveform::Saw => 0,
            Waveform::Pulse => 1,
            Waveform::Triangle => 2,
            Waveform::Sine => 3,
            Waveform::Noise => 4,
        }
    }
}

/// PolyBLEP anti-aliasing correction
#[inline(always)]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        2.0 * t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + 2.0 * t + 1.0
    } else {
        0.0
    }
}

/// Fast sine approximation (4th order polynomial)
#[inline(always)]
fn fast_sin(x: f32) -> f32 {
    // Normalize to -PI..PI
    let mut x = x % TAU;
    if x > PI { x -= TAU; }
    if x < -PI { x += TAU; }
    // Bhaskara-style approximation
    let b = 4.0 / PI;
    let c = -4.0 / (PI * PI);
    let y = b * x + c * x * x.abs();
    // Extra precision
    let p = 0.225;
    p * (y * y.abs() - y) + y
}

#[derive(Debug, Clone)]
pub struct Oscillator {
    pub phase: f32,
    pub sample_rate: f32,
    noise_state: u32,
}

impl Oscillator {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            phase: 0.0,
            sample_rate,
            noise_state: 0xDEAD_BEEF,
        }
    }

    /// Reset phase (for key sync or hard sync)
    pub fn reset_phase(&mut self) {
        self.phase = 0.0;
    }

    /// Generate one sample with continuous waveform morphing.
    /// shape: 0.0=Saw, 0.25=Pulse, 0.5=Triangle, 0.75=Sine, 1.0=Saw (wraps)
    /// When shape is between two waveforms, crossfades between them.
    pub fn process_shape(
        &mut self,
        frequency: f32,
        pulse_width: f32,
        shape: f32,
    ) -> (f32, bool) {
        let dt = frequency / self.sample_rate;
        let dt = dt.clamp(0.0, 0.5);

        // 4 waveforms equally spaced: Saw(0), Pulse(0.25), Tri(0.5), Sine(0.75)
        let shape = shape.fract(); // wrap 0..1
        let idx = shape * 4.0;
        let base = idx as usize; // 0..3
        let frac = idx - base as f32; // blend factor

        let waveforms = [Waveform::Saw, Waveform::Pulse, Waveform::Triangle, Waveform::Sine];
        let wf_a = waveforms[base % 4];
        let wf_b = waveforms[(base + 1) % 4];

        let sample_a = self.raw_waveform(wf_a, self.phase, dt, pulse_width);
        let sample_b = self.raw_waveform(wf_b, self.phase, dt, pulse_width);
        let sample = sample_a * (1.0 - frac) + sample_b * frac;

        self.phase += dt;
        let wrapped = self.phase >= 1.0;
        if wrapped { self.phase -= 1.0; }

        (sample, wrapped)
    }

    /// Raw waveform sample at given phase (no phase advance)
    #[inline]
    fn raw_waveform(&self, waveform: Waveform, phase: f32, dt: f32, pulse_width: f32) -> f32 {
        match waveform {
            Waveform::Saw => {
                let naive = 2.0 * phase - 1.0;
                naive - poly_blep(phase, dt)
            }
            Waveform::Pulse => {
                let duty = pulse_width.clamp(0.05, 0.95);
                let mut s = if phase < duty { 1.0 } else { -1.0 };
                s += poly_blep(phase, dt);
                s -= poly_blep((phase - duty + 1.0) % 1.0, dt);
                s
            }
            Waveform::Triangle => {
                if phase < 0.25 { phase * 4.0 }
                else if phase < 0.75 { 2.0 - phase * 4.0 }
                else { phase * 4.0 - 4.0 }
            }
            Waveform::Sine => fast_sin(phase * TAU),
            Waveform::Noise => 0.0, // Noise handled separately
        }
    }

    /// Generate one sample. Returns (sample, phase_wrapped) where phase_wrapped
    /// is true if the oscillator completed a cycle (used for hard sync).
    pub fn process(
        &mut self,
        frequency: f32,
        waveform: Waveform,
        pulse_width: f32,
    ) -> (f32, bool) {
        let dt = frequency / self.sample_rate;
        let dt = dt.clamp(0.0, 0.5); // Nyquist limit

        let sample = match waveform {
            Waveform::Saw => {
                let naive = 2.0 * self.phase - 1.0;
                naive - poly_blep(self.phase, dt)
            }
            Waveform::Pulse => {
                let duty = pulse_width.clamp(0.05, 0.95);
                let mut sample = if self.phase < duty { 1.0 } else { -1.0 };
                sample += poly_blep(self.phase, dt);
                sample -= poly_blep((self.phase - duty + 1.0) % 1.0, dt);
                sample
            }
            Waveform::Triangle => {
                // Integrated square approach for band-limited triangle
                let p = self.phase;
                if p < 0.25 {
                    p * 4.0
                } else if p < 0.75 {
                    2.0 - p * 4.0
                } else {
                    p * 4.0 - 4.0
                }
            }
            Waveform::Sine => {
                fast_sin(self.phase * TAU)
            }
            Waveform::Noise => {
                self.noise_state ^= self.noise_state << 13;
                self.noise_state ^= self.noise_state >> 17;
                self.noise_state ^= self.noise_state << 5;
                (self.noise_state as f32 / u32::MAX as f32) * 2.0 - 1.0
            }
        };

        // Advance phase
        self.phase += dt;
        let wrapped = self.phase >= 1.0;
        if wrapped {
            self.phase -= 1.0;
        }

        (sample, wrapped)
    }

    /// Process with FM modulation (modulator output modulates this oscillator's phase)
    pub fn process_fm(
        &mut self,
        frequency: f32,
        waveform: Waveform,
        pulse_width: f32,
        fm_input: f32,
        fm_amount: f32,
    ) -> (f32, bool) {
        let dt = frequency / self.sample_rate;
        let dt = dt.clamp(0.0, 0.5);

        // FM: modulate phase by fm_input
        let fm_phase = (self.phase + fm_input * fm_amount * 0.5).fract();
        let fm_phase = if fm_phase < 0.0 { fm_phase + 1.0 } else { fm_phase };

        let sample = match waveform {
            Waveform::Saw => {
                let naive = 2.0 * fm_phase - 1.0;
                naive - poly_blep(fm_phase, dt)
            }
            Waveform::Pulse => {
                let duty = pulse_width.clamp(0.05, 0.95);
                let mut s = if fm_phase < duty { 1.0 } else { -1.0 };
                s += poly_blep(fm_phase, dt);
                s -= poly_blep((fm_phase - duty + 1.0) % 1.0, dt);
                s
            }
            Waveform::Triangle => {
                if fm_phase < 0.25 { fm_phase * 4.0 }
                else if fm_phase < 0.75 { 2.0 - fm_phase * 4.0 }
                else { fm_phase * 4.0 - 4.0 }
            }
            Waveform::Sine => {
                fast_sin(fm_phase * TAU)
            }
            Waveform::Noise => {
                self.noise_state ^= self.noise_state << 13;
                self.noise_state ^= self.noise_state >> 17;
                self.noise_state ^= self.noise_state << 5;
                (self.noise_state as f32 / u32::MAX as f32) * 2.0 - 1.0
            }
        };

        self.phase += dt;
        let wrapped = self.phase >= 1.0;
        if wrapped {
            self.phase -= 1.0;
        }

        (sample, wrapped)
    }

    /// Process with FM and continuous shape morphing
    pub fn process_fm_shape(
        &mut self,
        frequency: f32,
        pulse_width: f32,
        fm_input: f32,
        fm_amount: f32,
        shape: f32,
    ) -> (f32, bool) {
        let dt = frequency / self.sample_rate;
        let dt = dt.clamp(0.0, 0.5);

        let fm_phase = (self.phase + fm_input * fm_amount * 0.5).fract();
        let fm_phase = if fm_phase < 0.0 { fm_phase + 1.0 } else { fm_phase };

        let shape = shape.fract();
        let idx = shape * 4.0;
        let base = idx as usize;
        let frac = idx - base as f32;

        let waveforms = [Waveform::Saw, Waveform::Pulse, Waveform::Triangle, Waveform::Sine];
        let wf_a = waveforms[base % 4];
        let wf_b = waveforms[(base + 1) % 4];

        let sample_a = self.raw_waveform(wf_a, fm_phase, dt, pulse_width);
        let sample_b = self.raw_waveform(wf_b, fm_phase, dt, pulse_width);
        let sample = sample_a * (1.0 - frac) + sample_b * frac;

        self.phase += dt;
        let wrapped = self.phase >= 1.0;
        if wrapped { self.phase -= 1.0; }

        (sample, wrapped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Golden hashes captured from the hand-rolled `PolarisOscillator` before it
    /// moved here (process + process_fm + process_shape + process_fm_shape mixed
    /// per waveform). Any arithmetic drift breaks them.
    #[test]
    fn is_byte_identical_to_the_legacy_nord_oscillator() {
        let golden: [u64; 5] = [
            0x08b626240c6ee28a, // Saw
            0xb70b0e3ed828e782, // Pulse
            0xb38e160d02979a54, // Triangle
            0x131329d8b020d450, // Sine
            0x87227b3358a1cab5, // Noise
        ];
        for wf in 0..5 {
            let w = Waveform::from_index(wf);
            let mut o = Oscillator::new(44100.0);
            let mut h = 0u64;
            for i in 0..3000 {
                let f = 110.0 + i as f32 * 0.5;
                let (s, _) = o.process(f, w, 0.35);
                h = h.rotate_left(7) ^ s.to_bits() as u64;
            }
            let mut o2 = Oscillator::new(44100.0);
            for i in 0..3000 {
                let f = 220.0 + i as f32 * 0.3;
                let (a, _) = o2.process_fm(f, w, 0.4, (i as f32 * 0.01).sin(), 0.6);
                h = h.rotate_left(7) ^ a.to_bits() as u64;
            }
            let mut o3 = Oscillator::new(44100.0);
            for i in 0..3000 {
                let (a, _) = o3.process_shape(180.0, 0.3, i as f32 / 3000.0);
                let (b, _) = o3.process_fm_shape(180.0, 0.3, 0.2, 0.5, i as f32 / 3000.0);
                h = h.rotate_left(7) ^ a.to_bits() as u64;
                h = h.rotate_left(3) ^ b.to_bits() as u64;
            }
            assert_eq!(h, golden[wf], "oscillator waveform {wf} drifted from the legacy PolarisOscillator");
        }
    }
}
