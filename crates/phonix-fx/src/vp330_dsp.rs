//! VP-330 DSP primitives
//!
//! Biquad filters, formant filter bank with vowel morphing,
//! BBD ensemble chorus, Schroeder reverb, and AR envelope.

use std::f64::consts::PI;

// ── Biquad Filter (Direct Form II Transposed) ───────────────

#[derive(Clone)]
pub struct BiquadFilter {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl BiquadFilter {
    pub fn new() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    pub fn lowpass(freq: f32, q: f32, sample_rate: f32) -> Self {
        let w0 = 2.0 * PI * freq as f64 / sample_rate as f64;
        let alpha = w0.sin() / (2.0 * q as f64);
        let cos_w0 = w0.cos();
        let a0 = 1.0 + alpha;
        Self {
            b0: ((1.0 - cos_w0) / 2.0) / a0,
            b1: (1.0 - cos_w0) / a0,
            b2: ((1.0 - cos_w0) / 2.0) / a0,
            a1: (-2.0 * cos_w0) / a0,
            a2: (1.0 - alpha) / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    pub fn bandpass(freq: f32, q: f32, sample_rate: f32) -> Self {
        let w0 = 2.0 * PI * freq as f64 / sample_rate as f64;
        let alpha = w0.sin() / (2.0 * q as f64);
        let a0 = 1.0 + alpha;
        Self {
            b0: alpha / a0,
            b1: 0.0,
            b2: -alpha / a0,
            a1: (-2.0 * w0.cos()) / a0,
            a2: (1.0 - alpha) / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let x = input as f64;
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y as f32
    }

    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

// ── Vowel Formant Data ──────────────────────────────────────

pub const VOWEL_COUNT: usize = 5;
pub const FORMANT_COUNT: usize = 5;

/// Male formant frequencies [vowel][formant F1-F5] (Peterson & Barney + extensions)
pub const MALE_FORMANTS: [[f32; FORMANT_COUNT]; VOWEL_COUNT] = [
    [730.0, 1090.0, 2440.0, 3400.0, 4100.0], // ah (open)
    [530.0, 1840.0, 2480.0, 3400.0, 4100.0], // eh (mid)
    [270.0, 2290.0, 3010.0, 3300.0, 3850.0], // ee (closed)
    [570.0, 840.0, 2410.0, 3400.0, 4100.0],  // oh (round)
    [300.0, 870.0, 2240.0, 3400.0, 4100.0],  // oo (closed round)
];

/// Female formant frequencies (~17% higher)
pub const FEMALE_FORMANTS: [[f32; FORMANT_COUNT]; VOWEL_COUNT] = [
    [850.0, 1270.0, 2810.0, 3700.0, 4500.0], // ah
    [610.0, 2150.0, 2900.0, 3700.0, 4500.0], // eh
    [310.0, 2680.0, 3520.0, 3800.0, 4600.0], // ee
    [660.0, 980.0, 2820.0, 3700.0, 4500.0],  // oh
    [350.0, 1010.0, 2620.0, 3700.0, 4500.0], // oo
];

pub const FORMANT_Q: [f32; FORMANT_COUNT] = [8.0, 12.0, 15.0, 18.0, 22.0];
pub const FORMANT_GAIN: [f32; FORMANT_COUNT] = [1.0, 0.5, 0.25, 0.12, 0.06];

pub const VOWEL_NAMES: [&str; VOWEL_COUNT] = ["Ah", "Eh", "Ee", "Oh", "Oo"];

// ── Formant Filter Bank ─────────────────────────────────────

pub struct FormantFilterBank {
    filters: [BiquadFilter; FORMANT_COUNT],
    gains: [f32; FORMANT_COUNT],
    sample_rate: f32,
}

impl FormantFilterBank {
    pub fn new(sample_rate: f32) -> Self {
        let mut bank = Self {
            filters: [BiquadFilter::new(), BiquadFilter::new(), BiquadFilter::new(), BiquadFilter::new(), BiquadFilter::new()],
            gains: FORMANT_GAIN,
            sample_rate,
        };
        bank.set_vowel(0.0, false);
        bank
    }

    /// Set vowel shape: 0.0=ah, 0.25=eh, 0.5=ee, 0.75=oh, 1.0=oo
    pub fn set_vowel(&mut self, vowel: f32, female: bool) {
        let formants = if female {
            &FEMALE_FORMANTS
        } else {
            &MALE_FORMANTS
        };
        let vowel = vowel.clamp(0.0, 1.0);
        let pos = vowel * (VOWEL_COUNT - 1) as f32;
        let idx_a = (pos as usize).min(VOWEL_COUNT - 2);
        let idx_b = idx_a + 1;
        let frac = pos - idx_a as f32;

        for f in 0..FORMANT_COUNT {
            let freq = formants[idx_a][f] * (1.0 - frac) + formants[idx_b][f] * frac;
            self.filters[f] = BiquadFilter::bandpass(freq, FORMANT_Q[f], self.sample_rate);
        }
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let mut out = 0.0;
        for f in 0..FORMANT_COUNT {
            out += self.filters[f].process(input) * self.gains[f];
        }
        out
    }

    pub fn reset(&mut self) {
        for f in &mut self.filters {
            f.reset();
        }
    }
}

// ── Ensemble Chorus (BBD simulation) ────────────────────────

const ENSEMBLE_BUFFER_MS: f32 = 20.0;
const ENSEMBLE_LFO_RATES: [f64; 3] = [0.513, 0.73, 1.07];
const ENSEMBLE_BASE_DELAYS: [f32; 3] = [4.3, 5.1, 6.7];
const ENSEMBLE_MOD_DEPTHS: [f32; 3] = [1.2, 1.5, 2.0];

#[derive(Debug, Clone)]
pub struct EnsembleChorus {
    buffer: Vec<f32>,
    write_pos: usize,
    lfo_phases: [f64; 3],
    sample_rate: f32,
    /// Sub-Hz LFO sins, refreshed every 32 samples so the per-sample
    /// path skips three `f64::sin` evaluations. 32 samples at 48 kHz
    /// = ~1.5 kHz refresh rate, well above the audible flutter
    /// threshold of any sub-2-Hz chorus LFO.
    cached_lfos: [f32; 3],
    ctrl_counter: u32,
}

impl EnsembleChorus {
    pub fn new(sample_rate: f32) -> Self {
        let buf_size = (ENSEMBLE_BUFFER_MS * sample_rate / 1000.0) as usize + 2;
        let lfo_phases = [0.0_f64, 2.094, 4.189];
        let cached_lfos = [
            lfo_phases[0].sin() as f32,
            lfo_phases[1].sin() as f32,
            lfo_phases[2].sin() as f32,
        ];
        Self {
            buffer: vec![0.0; buf_size],
            write_pos: 0,
            lfo_phases,
            sample_rate,
            cached_lfos,
            ctrl_counter: 0,
        }
    }

    #[inline]
    pub fn process(&mut self, input: f32, depth: f32) -> (f32, f32) {
        let buf_size = self.buffer.len();
        self.buffer[self.write_pos] = input;
        // Modulo by `buf_size` (non-power-of-2 ring buffer) was a
        // hardware integer divide per audio sample. Bounded reset
        // is two ops with branch prediction (almost always not
        // taken).
        self.write_pos += 1;
        if self.write_pos >= buf_size { self.write_pos = 0; }

        let refresh = self.ctrl_counter == 0;
        let mut taps = [0.0f32; 3];
        for i in 0..3 {
            if refresh {
                self.cached_lfos[i] = self.lfo_phases[i].sin() as f32;
            }
            let lfo = self.cached_lfos[i];
            let delay_ms = ENSEMBLE_BASE_DELAYS[i] + lfo * ENSEMBLE_MOD_DEPTHS[i] * depth;
            let delay_samples = delay_ms * self.sample_rate / 1000.0;

            let read_pos = self.write_pos as f32 - delay_samples - 1.0;
            let read_pos = if read_pos < 0.0 {
                read_pos + buf_size as f32
            } else {
                read_pos
            };
            let idx = read_pos as usize % buf_size;
            let frac = read_pos.fract();
            let next = (idx + 1) % buf_size;
            taps[i] = self.buffer[idx] * (1.0 - frac) + self.buffer[next] * frac;

            self.lfo_phases[i] +=
                2.0 * PI * ENSEMBLE_LFO_RATES[i] / self.sample_rate as f64;
            if self.lfo_phases[i] > 2.0 * PI {
                self.lfo_phases[i] -= 2.0 * PI;
            }
        }
        self.ctrl_counter = (self.ctrl_counter + 1) & 31;

        let left = input * 0.6 + taps[0] * 0.35 + taps[1] * 0.15 + taps[2] * 0.25;
        let right = input * 0.6 + taps[0] * 0.15 + taps[1] * 0.35 - taps[2] * 0.25;
        (left, right)
    }

    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
    }
}

// ── AR Envelope ─────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub enum EnvState {
    Idle,
    Attack,
    Sustain,
    Release,
}

#[derive(Clone)]
pub struct ArEnvelope {
    state: EnvState,
    pub level: f32,
    attack_rate: f32,
    release_rate: f32,
}

impl ArEnvelope {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            state: EnvState::Idle,
            level: 0.0,
            attack_rate: Self::time_to_rate(0.05, sample_rate),
            release_rate: Self::time_to_rate(0.3, sample_rate),
        }
    }

    fn time_to_rate(seconds: f32, sample_rate: f32) -> f32 {
        if seconds <= 0.001 {
            return 1.0;
        }
        1.0 / (seconds * sample_rate)
    }

    pub fn set_attack(&mut self, seconds: f32, sample_rate: f32) {
        self.attack_rate = Self::time_to_rate(seconds, sample_rate);
    }

    pub fn set_release(&mut self, seconds: f32, sample_rate: f32) {
        self.release_rate = Self::time_to_rate(seconds, sample_rate);
    }

    pub fn gate_on(&mut self) {
        self.state = EnvState::Attack;
    }

    pub fn gate_off(&mut self) {
        if self.state != EnvState::Idle {
            self.state = EnvState::Release;
        }
    }

    pub fn is_idle(&self) -> bool {
        self.state == EnvState::Idle
    }

    #[inline]
    pub fn process(&mut self) -> f32 {
        match self.state {
            EnvState::Idle => {}
            EnvState::Attack => {
                self.level += self.attack_rate;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.state = EnvState::Sustain;
                }
            }
            EnvState::Sustain => {}
            EnvState::Release => {
                self.level -= self.release_rate;
                if self.level <= 0.0 {
                    self.level = 0.0;
                    self.state = EnvState::Idle;
                }
            }
        }
        self.level
    }
}

// ── Simple Schroeder Reverb ─────────────────────────────────

#[derive(Debug, Clone)]
struct CombFilter {
    buffer: Vec<f32>,
    pos: usize,
    feedback: f32,
    damp: f32,
    damp_state: f32,
}

impl CombFilter {
    pub fn set_feedback(&mut self, fb: f32) {
        self.feedback = fb.clamp(0.0, 0.99);
    }
    pub fn set_damp(&mut self, d: f32) {
        self.damp = d.clamp(0.0, 0.95);
    }

    fn new(delay_samples: usize, feedback: f32, damp: f32) -> Self {
        Self {
            buffer: vec![0.0; delay_samples.max(1)],
            pos: 0,
            feedback,
            damp,
            damp_state: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.buffer[self.pos];
        self.damp_state = delayed * (1.0 - self.damp) + self.damp_state * self.damp;
        self.buffer[self.pos] = input + self.damp_state * self.feedback;
        self.pos = (self.pos + 1) % self.buffer.len();
        delayed
    }
}

#[derive(Debug, Clone)]
struct AllPassFilter {
    buffer: Vec<f32>,
    pos: usize,
    feedback: f32,
}

impl AllPassFilter {
    fn new(delay_samples: usize, feedback: f32) -> Self {
        Self {
            buffer: vec![0.0; delay_samples.max(1)],
            pos: 0,
            feedback,
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.buffer[self.pos];
        let output = -input + delayed;
        self.buffer[self.pos] = input + delayed * self.feedback;
        self.pos = (self.pos + 1) % self.buffer.len();
        output
    }
}

#[derive(Debug, Clone)]
pub struct SimpleReverb {
    combs_l: [CombFilter; 4],
    combs_r: [CombFilter; 4],
    allpasses_l: [AllPassFilter; 2],
    allpasses_r: [AllPassFilter; 2],
    /// Output gain to match convolution reverb's perceived loudness.
    /// Without this the algorithmic branch was ~3× quieter than
    /// convolution at the same `reverb_mix` knob position. Plexus
    /// reference: `src/plexus/bus.rs::ReverbBus`.
    output_gain: f32,
}

impl SimpleReverb {
    pub fn new(sample_rate: f32) -> Self {
        let ms = |t: f32| (t * sample_rate / 1000.0) as usize;
        let fb = 0.75;
        let damp = 0.4;

        Self {
            combs_l: [
                CombFilter::new(ms(29.7), fb, damp),
                CombFilter::new(ms(37.1), fb, damp),
                CombFilter::new(ms(41.1), fb, damp),
                CombFilter::new(ms(43.7), fb, damp),
            ],
            combs_r: [
                CombFilter::new(ms(31.3), fb, damp),
                CombFilter::new(ms(38.9), fb, damp),
                CombFilter::new(ms(42.3), fb, damp),
                CombFilter::new(ms(45.1), fb, damp),
            ],
            allpasses_l: [
                AllPassFilter::new(ms(5.0), 0.5),
                AllPassFilter::new(ms(1.7), 0.5),
            ],
            allpasses_r: [
                AllPassFilter::new(ms(5.3), 0.5),
                AllPassFilter::new(ms(1.9), 0.5),
            ],
            output_gain: 3.0,
        }
    }

    /// `size` 0..1 → comb feedback 0.5..0.9. Drives the apparent
    /// room size / tail length.
    pub fn set_size(&mut self, size: f32) {
        let fb = 0.5 + size.clamp(0.0, 1.0) * 0.4;
        for c in &mut self.combs_l { c.set_feedback(fb); }
        for c in &mut self.combs_r { c.set_feedback(fb); }
    }

    /// Per-IR high-frequency damping (0..1). Higher = darker.
    /// Lets the IR-kind dropdown sculpt timbre on the algorithmic
    /// branch instead of being inert. Cathedral=0.05, Wooden Hall
    /// =0.80, etc. — see `IrKind::algorithmic_params`.
    pub fn set_damping(&mut self, damp: f32) {
        let d = damp.clamp(0.0, 0.95);
        for c in &mut self.combs_l { c.set_damp(d); }
        for c in &mut self.combs_r { c.set_damp(d); }
    }

    #[inline]
    pub fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let mut out_l = 0.0;
        let mut out_r = 0.0;
        for comb in &mut self.combs_l {
            out_l += comb.process(left);
        }
        for comb in &mut self.combs_r {
            out_r += comb.process(right);
        }
        out_l *= 0.25;
        out_r *= 0.25;

        for ap in &mut self.allpasses_l {
            out_l = ap.process(out_l);
        }
        for ap in &mut self.allpasses_r {
            out_r = ap.process(out_r);
        }
        (out_l * self.output_gain, out_r * self.output_gain)
    }
}

// ── PolyBLEP sawtooth helper ────────────────────────────────

#[inline]
pub fn saw_polyblep(phase: &mut f64, freq: f64, sample_rate: f64) -> f32 {
    let dt = freq / sample_rate;
    let naive = 2.0 * *phase - 1.0;
    let mut out = naive;
    let t = *phase;
    if t < dt {
        let t = t / dt;
        out -= t + t - t * t - 1.0;
    } else if t > 1.0 - dt {
        let t = (t - 1.0 + dt) / dt;
        out -= t * t + t + t - 1.0;
    }
    *phase += dt;
    if *phase >= 1.0 {
        *phase -= 1.0;
    }
    out as f32
}

// ── Envelope Follower ─────────────────────────────────────────

pub struct EnvelopeFollower {
    level: f32,
    attack_coeff: f32,
    release_coeff: f32,
}

impl EnvelopeFollower {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            level: 0.0,
            attack_coeff: (-1.0f32 / (0.001 * sample_rate)).exp(),  // 1ms attack
            release_coeff: (-1.0f32 / (0.008 * sample_rate)).exp(), // 8ms release
        }
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let rect = input.abs();
        let coeff = if rect > self.level {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.level = coeff * self.level + (1.0 - coeff) * rect;
        self.level
    }

    pub fn reset(&mut self) {
        self.level = 0.0;
    }
}

// ── Vocoder Bank (10-band analysis/synthesis) ─────────────────

pub const VOCODER_BANDS: usize = 10;

/// VP-330 style vocoder band center frequencies
pub const VOCODER_FREQS: [f32; VOCODER_BANDS] = [
    200.0, 350.0, 510.0, 700.0, 1000.0,
    1400.0, 2000.0, 2800.0, 4000.0, 5500.0,
];

const VOCODER_Q: f32 = 5.0;

pub struct VocoderBank {
    analysis: [BiquadFilter; VOCODER_BANDS],
    synthesis: [BiquadFilter; VOCODER_BANDS],
    envelopes: [EnvelopeFollower; VOCODER_BANDS],
    sibilance_filter: BiquadFilter,
}

impl VocoderBank {
    pub fn new(sample_rate: f32) -> Self {
        let mut analysis = [
            BiquadFilter::new(), BiquadFilter::new(), BiquadFilter::new(),
            BiquadFilter::new(), BiquadFilter::new(), BiquadFilter::new(),
            BiquadFilter::new(), BiquadFilter::new(), BiquadFilter::new(),
            BiquadFilter::new(),
        ];
        let mut synthesis = [
            BiquadFilter::new(), BiquadFilter::new(), BiquadFilter::new(),
            BiquadFilter::new(), BiquadFilter::new(), BiquadFilter::new(),
            BiquadFilter::new(), BiquadFilter::new(), BiquadFilter::new(),
            BiquadFilter::new(),
        ];
        let envelopes = [
            EnvelopeFollower::new(sample_rate), EnvelopeFollower::new(sample_rate),
            EnvelopeFollower::new(sample_rate), EnvelopeFollower::new(sample_rate),
            EnvelopeFollower::new(sample_rate), EnvelopeFollower::new(sample_rate),
            EnvelopeFollower::new(sample_rate), EnvelopeFollower::new(sample_rate),
            EnvelopeFollower::new(sample_rate), EnvelopeFollower::new(sample_rate),
        ];

        for i in 0..VOCODER_BANDS {
            analysis[i] = BiquadFilter::bandpass(VOCODER_FREQS[i], VOCODER_Q, sample_rate);
            synthesis[i] = BiquadFilter::bandpass(VOCODER_FREQS[i], VOCODER_Q, sample_rate);
        }

        Self {
            analysis,
            synthesis,
            envelopes,
            sibilance_filter: BiquadFilter::bandpass(8000.0, 2.0, sample_rate),
        }
    }

    /// Process one sample: modulator (mic) controls carrier (oscillators)
    /// Returns vocoder output + sibilance level for display
    #[inline]
    pub fn process(&mut self, carrier: f32, modulator: f32, sibilance: f32) -> f32 {
        let mut output = 0.0;
        for i in 0..VOCODER_BANDS {
            let mod_band = self.analysis[i].process(modulator);
            let env = self.envelopes[i].process(mod_band);
            let car_band = self.synthesis[i].process(carrier);
            output += car_band * env;
        }

        // Add high-frequency sibilance from modulator for consonant clarity
        if sibilance > 0.001 {
            let sib = self.sibilance_filter.process(modulator);
            output += sib * sibilance;
        }

        output
    }

    /// Get per-band envelope levels for display
    pub fn band_levels(&self) -> [f32; VOCODER_BANDS] {
        let mut levels = [0.0f32; VOCODER_BANDS];
        for i in 0..VOCODER_BANDS {
            levels[i] = self.envelopes[i].level;
        }
        levels
    }

    pub fn reset(&mut self) {
        for i in 0..VOCODER_BANDS {
            self.analysis[i].reset();
            self.synthesis[i].reset();
            self.envelopes[i].reset();
        }
    }
}
