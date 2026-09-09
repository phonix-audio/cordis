//! Clean autotune via pitch-synchronous granular pitch correction.
//!
//! Uses the same dual-head delay technique as the pitch shifter, but with
//! grain size locked to the detected pitch period. This means crossfades
//! always happen at the same phase of the waveform → transparent correction.
//!
//! This is the technique used by Antares Auto-Tune and similar products.

const BUFFER_SIZE: usize = 8192;
const BUFFER_MASK: usize = BUFFER_SIZE - 1;
const XFADE_TABLE_SIZE: usize = 256;
const DEFAULT_GRAIN: f32 = 512.0;
const MIN_GRAIN: f32 = 40.0;   // ~1200Hz max pitch
const MAX_GRAIN: f32 = 800.0;  // ~60Hz min pitch

fn build_xfade_table() -> [f32; XFADE_TABLE_SIZE] {
    let mut table = [0.0f32; XFADE_TABLE_SIZE];
    let mut i = 0;
    while i < XFADE_TABLE_SIZE {
        let phase = i as f64 / XFADE_TABLE_SIZE as f64;
        let s = (std::f64::consts::PI * phase).sin();
        table[i] = (s * s) as f32;
        i += 1;
    }
    table
}

/// Pitch-synchronous autotune processor.
pub struct AutotuneProcessor {
    buffer: [f32; BUFFER_SIZE],
    write_pos: usize,
    delay_a: f32,
    delay_b: f32,
    grain_size: f32,
    xfade_table: [f32; XFADE_TABLE_SIZE],
    sample_rate: f32,
    /// How fast the correction ratio responds (0=off, 1=instant)
    speed: f32,
    /// Smoothed ratio
    current_ratio: f32,
    ratio_smooth: f32,
}

impl AutotuneProcessor {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            buffer: [0.0; BUFFER_SIZE],
            write_pos: 0,
            delay_a: 0.0,
            delay_b: DEFAULT_GRAIN * 0.5,
            grain_size: DEFAULT_GRAIN,
            xfade_table: build_xfade_table(),
            sample_rate,
            speed: 1.0,
            current_ratio: 1.0,
            ratio_smooth: (-1.0 / (0.005 * sample_rate)).exp(), // 5ms
        }
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.clamp(0.0, 1.0);
        let smooth_time = 0.002 + (1.0 - speed) * 0.03; // 2ms (hard) to 32ms (gentle)
        self.ratio_smooth = (-1.0 / (smooth_time * self.sample_rate)).exp();
    }

    /// Set grain size from detected pitch (call this from engine when pitch updates).
    /// `pitch_hz`: detected fundamental frequency.
    #[inline]
    pub fn set_pitch_period(&mut self, pitch_hz: f32) {
        if pitch_hz > 0.0 {
            let period = self.sample_rate / pitch_hz;
            // Use 2x the pitch period for smoother grains
            let new_grain = (period * 2.0).clamp(MIN_GRAIN, MAX_GRAIN);
            // Smooth grain size changes to avoid clicks
            self.grain_size += (new_grain - self.grain_size) * 0.05;
        }
    }

    /// Process one sample. `target_ratio`: pitch multiplier (1.0 = no change).
    #[inline]
    pub fn process(&mut self, input: f32, target_ratio: f32) -> f32 {
        self.buffer[self.write_pos] = input;

        // Smooth the ratio
        self.current_ratio = self.current_ratio * self.ratio_smooth
            + target_ratio * (1.0 - self.ratio_smooth);

        // Sweep delays
        let sweep = 1.0 - self.current_ratio;
        self.delay_a = fast_rem(self.delay_a + sweep, self.grain_size);
        self.delay_b = fast_rem(self.delay_b + sweep, self.grain_size);

        // Ensure head B stays half a grain offset from A
        let ideal_b = fast_rem(self.delay_a + self.grain_size * 0.5, self.grain_size);
        self.delay_b += (ideal_b - self.delay_b) * 0.01; // gently track
        self.delay_b = fast_rem(self.delay_b, self.grain_size);

        // Read both heads
        let out_a = self.read_cubic(self.delay_a);
        let out_b = self.read_cubic(self.delay_b);

        // Crossfade via table
        let inv_g = 1.0 / self.grain_size;
        let win_a = self.xfade_lookup(self.delay_a * inv_g);
        let win_b = self.xfade_lookup(self.delay_b * inv_g);
        let sum = win_a + win_b;
        let output = if sum > 1e-6 {
            (out_a * win_a + out_b * win_b) / sum
        } else {
            (out_a + out_b) * 0.5
        };

        self.write_pos = (self.write_pos + 1) & BUFFER_MASK;
        output
    }

    pub fn reset(&mut self) {
        self.buffer = [0.0; BUFFER_SIZE];
        self.write_pos = 0;
        self.delay_a = 0.0;
        self.delay_b = self.grain_size * 0.5;
        self.current_ratio = 1.0;
    }

    #[inline(always)]
    fn read_cubic(&self, delay: f32) -> f32 {
        let pos = (self.write_pos as f32 - delay).rem_euclid(BUFFER_SIZE as f32);
        let idx = pos as usize;
        let frac = pos - pos.floor();

        let im1 = self.buffer[(idx + BUFFER_SIZE - 1) & BUFFER_MASK];
        let i0 = self.buffer[idx & BUFFER_MASK];
        let i1 = self.buffer[(idx + 1) & BUFFER_MASK];
        let i2 = self.buffer[(idx + 2) & BUFFER_MASK];

        let c1 = 0.5 * (i1 - im1);
        let c2 = im1 - 2.5 * i0 + 2.0 * i1 - 0.5 * i2;
        let c3 = 0.5 * (i2 - im1) + 1.5 * (i0 - i1);
        ((c3 * frac + c2) * frac + c1) * frac + i0
    }

    #[inline(always)]
    fn xfade_lookup(&self, phase: f32) -> f32 {
        let p = phase.clamp(0.0, 0.9999) * XFADE_TABLE_SIZE as f32;
        let idx = p as usize;
        let frac = p - idx as f32;
        if idx >= XFADE_TABLE_SIZE - 1 {
            return self.xfade_table[XFADE_TABLE_SIZE - 1];
        }
        self.xfade_table[idx] + (self.xfade_table[idx + 1] - self.xfade_table[idx]) * frac
    }
}

#[inline(always)]
fn fast_rem(val: f32, m: f32) -> f32 {
    let r = val % m;
    if r < 0.0 { r + m } else { r }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_passthrough() {
        let sr = 48000.0;
        let mut at = AutotuneProcessor::new(sr);
        let freq = 440.0;
        at.set_pitch_period(freq);
        let mut output = Vec::new();
        for i in 0..16384 {
            let input = (2.0 * PI * freq * i as f32 / sr).sin() * 0.5;
            output.push(at.process(input, 1.0));
        }
        let tail = &output[2048..];
        let rms: f32 = (tail.iter().map(|x| x * x).sum::<f32>() / tail.len() as f32).sqrt();
        assert!(rms > 0.2, "Passthrough RMS = {}", rms);
    }

    #[test]
    fn test_small_correction() {
        use crate::harmonizer_dsp::pitch_detect::YinDetector;

        let sr = 48000.0;
        let mut at = AutotuneProcessor::new(sr);
        at.set_speed(1.0);
        let mut detector = YinDetector::default_for_sample_rate(sr);

        let sharp_freq = 440.0 * 2.0f32.powf(30.0 / 1200.0); // 30 cents sharp
        at.set_pitch_period(sharp_freq);
        let ratio = 440.0 / sharp_freq;

        let mut output = Vec::new();
        for i in 0..24000 {
            let input = (2.0 * PI * sharp_freq * i as f32 / sr).sin() * 0.5;
            output.push(at.process(input, ratio));
        }

        for &s in &output[8000..] {
            detector.push_sample(s);
        }

        let detected = detector.pitch();
        let error = 1200.0 * (detected / 440.0).log2().abs();
        assert!(error < 20.0, "Expected ~440Hz, got {}Hz ({:.0}ct off)", detected, error);
    }
}
