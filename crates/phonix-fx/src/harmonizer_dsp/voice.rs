//! Per-voice state for harmony voices (optimized for real-time).

use super::pitch_shift::PitchShifter;
use std::f32::consts::PI;

/// Fast approximation of 2^x for small x (|x| < 1).
/// Max error ~0.3% in [-1, 1] range. Good enough for pitch ratios.
#[inline(always)]
fn fast_pow2(x: f32) -> f32 {
    // Attempt exact for common values
    if x == 0.0 { return 1.0; }
    // Polynomial approximation: 2^x ≈ 1 + 0.6931*x + 0.2402*x² + 0.0558*x³
    let x2 = x * x;
    1.0 + x * (0.6931472 + x * (0.2402265 + x * 0.0558206)) + x2 * x2 * 0.00898
}

/// State for a single harmony voice.
pub struct HarmonyVoice {
    pub enabled: bool,
    pub level: f32,
    pub pan: f32,
    pub detune_cents: f32,
    pub delay_ms: f32,
    pub gender: f32,
    pub vibrato_depth: f32,
    pub vibrato_rate: f32,
    pub octave: i8,

    pub pitch_shifter: PitchShifter,
    /// Second cascaded shifter for formant (gender) shifting: the first
    /// stage shifts by `ratio * g`, this one counter-shifts by `1/g`, so
    /// the net pitch stays `ratio` while the spectral envelope (formants)
    /// is scaled by `g`. Bypassed entirely when gender == 0 (g == 1.0),
    /// keeping the default path identical to the single-shifter version.
    /// Allocated at construction — never on the audio thread.
    formant_shifter: PitchShifter,
    delay_line: Vec<f32>,
    delay_write_pos: usize,
    lfo_phase: f32,

    current_pitch_ratio: f32,
    target_pitch_ratio: f32,
    ratio_smooth_coeff: f32,

    // Pre-computed pan gains (updated only when pan changes)
    pan_l: f32,
    pan_r: f32,
    cached_pan: f32,
    // Pre-computed detune ratio (updated only when detune changes)
    cached_detune_ratio: f32,
    cached_detune_cents: f32,
    // Pre-computed octave multiplier
    cached_octave_mult: f32,
    cached_octave: i8,
    // Pre-computed formant (gender) ratio + its inverse
    cached_formant_ratio: f32,
    cached_formant_inv: f32,
    cached_gender: f32,

    sample_rate: f32,
}

impl HarmonyVoice {
    pub fn new(sample_rate: f32) -> Self {
        let max_delay_samples = (sample_rate * 0.05) as usize + 1;
        let mut v = Self {
            enabled: false,
            level: 0.8,
            pan: 0.0,
            detune_cents: 0.0,
            delay_ms: 0.0,
            gender: 0.0,
            vibrato_depth: 0.0,
            vibrato_rate: 4.0,
            octave: 0,
            pitch_shifter: PitchShifter::new(sample_rate),
            formant_shifter: PitchShifter::new(sample_rate),
            delay_line: vec![0.0; max_delay_samples],
            delay_write_pos: 0,
            lfo_phase: 0.0,
            current_pitch_ratio: 1.0,
            target_pitch_ratio: 1.0,
            ratio_smooth_coeff: (-1.0 / (0.005 * sample_rate)).exp(), // 5ms smoothing
            pan_l: std::f32::consts::FRAC_1_SQRT_2,
            pan_r: std::f32::consts::FRAC_1_SQRT_2,
            cached_pan: 0.0,
            cached_detune_ratio: 1.0,
            cached_detune_cents: 0.0,
            cached_octave_mult: 1.0,
            cached_octave: 0,
            cached_formant_ratio: 1.0,
            cached_formant_inv: 1.0,
            cached_gender: 0.0,
            sample_rate,
        };
        v.update_pan();
        v
    }

    pub fn set_target_ratio(&mut self, ratio: f32) {
        self.target_pitch_ratio = ratio;
    }

    /// Process one sample. Returns (left, right).
    #[inline]
    pub fn process(&mut self, input: f32) -> (f32, f32) {
        // Smooth pitch ratio
        self.current_pitch_ratio = self.current_pitch_ratio * self.ratio_smooth_coeff
            + self.target_pitch_ratio * (1.0 - self.ratio_smooth_coeff);

        // Update cached values only when parameters change
        if self.octave != self.cached_octave {
            self.cached_octave = self.octave;
            self.cached_octave_mult = 2.0f32.powi(self.octave as i32);
        }
        if self.detune_cents != self.cached_detune_cents {
            self.cached_detune_cents = self.detune_cents;
            self.cached_detune_ratio = fast_pow2(self.detune_cents / 1200.0);
        }
        if self.pan != self.cached_pan {
            self.update_pan();
        }
        if self.gender != self.cached_gender {
            self.cached_gender = self.gender;
            // Bipolar gender knob (-1..1, 0 = neutral) -> formant scale.
            // ±1 maps to ±6 semitones of spectral-envelope shift.
            self.cached_formant_ratio = fast_pow2(self.gender * 0.5);
            self.cached_formant_inv = 1.0 / self.cached_formant_ratio;
        }

        let mut ratio = self.current_pitch_ratio * self.cached_octave_mult * self.cached_detune_ratio;

        // Vibrato (only if active)
        if self.vibrato_depth > 0.0 {
            // Fast triangle-ish LFO instead of sin()
            let phase = self.lfo_phase;
            let lfo = if phase < 0.25 {
                phase * 4.0
            } else if phase < 0.75 {
                2.0 - phase * 4.0
            } else {
                phase * 4.0 - 4.0
            };
            self.lfo_phase += self.vibrato_rate / self.sample_rate;
            if self.lfo_phase >= 1.0 { self.lfo_phase -= 1.0; }
            ratio *= fast_pow2(lfo * self.vibrato_depth / 12.0);
        }

        // Pitch shift (+ optional formant/gender shift). When gender is
        // neutral the cascade is bypassed and this is the exact legacy
        // single-shifter path.
        let shifted = if (self.cached_formant_ratio - 1.0).abs() < 1e-3 {
            self.pitch_shifter.process(input, ratio, 1.0)
        } else {
            // Stage 1 over-shifts by ratio*g, stage 2 counter-shifts by
            // 1/g: net pitch = ratio, formants scaled by g.
            let g = self.cached_formant_ratio;
            let mid = self.pitch_shifter.process(input, ratio * g, 1.0);
            self.formant_shifter.process(mid, self.cached_formant_inv, 1.0)
        };

        // Delay line (skip if delay is 0)
        let out = if self.delay_ms > 0.1 {
            let delay_samples = ((self.delay_ms * self.sample_rate * 0.001) as usize)
                .min(self.delay_line.len() - 1);
            self.delay_line[self.delay_write_pos] = shifted;
            self.delay_write_pos += 1;
            if self.delay_write_pos >= self.delay_line.len() {
                self.delay_write_pos = 0;
            }
            let read_pos = if self.delay_write_pos >= delay_samples {
                self.delay_write_pos - delay_samples
            } else {
                self.delay_write_pos + self.delay_line.len() - delay_samples
            };
            self.delay_line[read_pos]
        } else {
            shifted
        };

        let sample = out * self.level;
        (sample * self.pan_l, sample * self.pan_r)
    }

    fn update_pan(&mut self) {
        self.cached_pan = self.pan;
        let angle = (0.5 + self.pan * 0.5) * (PI / 2.0);
        self.pan_l = angle.cos() * std::f32::consts::SQRT_2;
        self.pan_r = angle.sin() * std::f32::consts::SQRT_2;
    }

    pub fn reset(&mut self) {
        self.pitch_shifter.reset();
        self.formant_shifter.reset();
        self.delay_line.fill(0.0);
        self.delay_write_pos = 0;
        self.lfo_phase = 0.0;
        self.current_pitch_ratio = 1.0;
        self.target_pitch_ratio = 1.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_voice(gender: f32, n: usize) -> Vec<f32> {
        let sr = 48000.0;
        let mut v = HarmonyVoice::new(sr);
        v.enabled = true;
        v.gender = gender;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let input = (2.0 * PI * 220.0 * i as f32 / sr).sin() * 0.5;
            out.push(v.process(input).0);
        }
        out
    }

    #[test]
    fn gender_neutral_matches_single_shifter() {
        // gender == 0 must bypass the formant cascade and be exactly the
        // legacy single-shifter path (same smoothing/level/pan math).
        let sr = 48000.0;
        let out = run_voice(0.0, 8192);

        let mut reference = PitchShifter::new(sr);
        let coeff = (-1.0f32 / (0.005 * sr)).exp();
        let mut cur = 1.0f32;
        let angle = 0.5 * (PI / 2.0);
        let pan_l = angle.cos() * std::f32::consts::SQRT_2;
        for (i, &got) in out.iter().enumerate() {
            cur = cur * coeff + 1.0 * (1.0 - coeff);
            let expected = reference.process((2.0 * PI * 220.0 * i as f32 / sr).sin() * 0.5, cur, 1.0)
                * 0.8 * pan_l;
            assert!(
                (got - expected).abs() < 1e-7,
                "sample {}: gender=0 diverged from legacy path ({} vs {})", i, got, expected
            );
        }
    }

    #[test]
    fn gender_shift_changes_output() {
        let neutral = run_voice(0.0, 8192);
        let shifted = run_voice(0.6, 8192);

        let rms: f32 = (shifted[2048..].iter().map(|x| x * x).sum::<f32>()
            / (shifted.len() - 2048) as f32).sqrt();
        assert!(rms > 0.05, "gender-shifted output is near-silent (rms {})", rms);

        let max_diff = neutral.iter().zip(&shifted)
            .skip(2048)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(max_diff > 1e-3, "gender=0.6 output identical to neutral (max diff {})", max_diff);
    }
}
