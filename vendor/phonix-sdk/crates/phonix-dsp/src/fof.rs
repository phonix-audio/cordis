//! FOF choir synthesis — Fonctions d'Onde Formantique (Rodet / IRCAM
//! CHANT, 1984). The documented technique for convincing synthetic
//! singing voice / choir.
//!
//! Each vowel is built from several formants. Each formant is a stream
//! of *grains*: a sine at the formant centre frequency windowed by a
//! raised-cosine attack + exponential decay (the decay rate sets the
//! formant bandwidth). A new grain is triggered every glottal period
//! (1/f0); grains overlap-add. A real choir = several such voices, each
//! slightly detuned with its own vibrato, jitter (period wobble),
//! shimmer (amplitude wobble) and vocal-tract scaling.
//!
//! Performance: `vowel_formants()` (4 powf) and the window cosines are
//! the per-sample hot spots, so the formant set is cached (recomputed
//! only when vowel/brightness change) and the windows use a fast sine
//! approximation. Singer count is runtime-variable (1..MAX_SINGERS).

const MAX_SINGERS: usize = 6;
const NUM_FORMANTS: usize = 4;
const MAX_GRAINS: usize = 8;
const TAU: f32 = std::f32::consts::TAU;
const PI: f32 = std::f32::consts::PI;
const HALF_PI: f32 = std::f32::consts::FRAC_PI_2;

#[inline(always)]
fn fast_sin(x: f32) -> f32 {
    // Parabolic sine approximation, input in radians.
    let mut p = x * (1.0 / TAU);
    p -= p.floor();
    let q = p - 0.5;
    -16.0 * q * (0.5 - q.abs())
}

/// `0.5 * (1 - cos(pi*x))` for x in 0..1, via the fast sine (cos = sin+90deg).
#[inline(always)]
fn rcos_window(x: f32) -> f32 {
    0.5 * (1.0 - fast_sin(PI * x + HALF_PI))
}

// ── Vowel formant tables (alto choir): freq Hz, bandwidth Hz, amp dB ──
const VOWELS: usize = 5;
const F_FREQ: [[f32; NUM_FORMANTS]; VOWELS] = [
    [ 800.0, 1150.0, 2800.0, 3500.0], // Ah
    [ 400.0, 1600.0, 2700.0, 3300.0], // Eh
    [ 350.0, 1700.0, 2700.0, 3700.0], // Ee
    [ 450.0,  800.0, 2830.0, 3500.0], // Oh
    [ 325.0,  700.0, 2530.0, 3500.0], // Oo
];
const F_BW: [[f32; NUM_FORMANTS]; VOWELS] = [
    [80.0,  90.0, 120.0, 130.0],
    [60.0,  80.0, 120.0, 150.0],
    [50.0, 100.0, 120.0, 150.0],
    [70.0,  80.0, 100.0, 130.0],
    [50.0,  60.0, 170.0, 180.0],
];
const F_DB: [[f32; NUM_FORMANTS]; VOWELS] = [
    [0.0,  -4.0, -20.0, -36.0],
    [0.0, -24.0, -30.0, -35.0],
    [0.0, -20.0, -30.0, -36.0],
    [0.0,  -9.0, -16.0, -28.0],
    [0.0, -12.0, -30.0, -40.0],
];

#[inline]
fn db_to_lin(db: f32) -> f32 { 10.0f32.powf(db / 20.0) }

/// Interpolated, choir-widened formant set for vowel 0..1 with a
/// brightness control (0.5 = neutral) lifting the upper formants.
fn vowel_formants(v: f32, brightness: f32) -> [(f32, f32, f32); NUM_FORMANTS] {
    let pos = v.clamp(0.0, 1.0) * (VOWELS - 1) as f32;
    let a = (pos as usize).min(VOWELS - 2);
    let b = a + 1;
    let t = pos - a as f32;
    let up = 0.4 + brightness.clamp(0.0, 1.0) * 1.2; // upper-formant gain
    let mut out = [(0.0, 0.0, 0.0); NUM_FORMANTS];
    for f in 0..NUM_FORMANTS {
        let freq = F_FREQ[a][f] * (1.0 - t) + F_FREQ[b][f] * t;
        // Choir = many singers -> effective formants far wider than a
        // single voice, so 2-3 harmonics pass under each.
        let bw = (F_BW[a][f] * (1.0 - t) + F_BW[b][f] * t) * 2.6;
        let mut amp = db_to_lin((F_DB[a][f] * (1.0 - t) + F_DB[b][f] * t) * 0.5);
        if f >= 1 { amp *= up; }
        out[f] = (freq, bw, amp);
    }
    out
}

#[derive(Clone, Copy, Default)]
struct Grain {
    active: bool,
    phase: f32,
    phase_inc: f32,
    env: f32,
    decay: f32,
    amp: f32,
    age: u32,
    kris: u32,
    kdur: u32,
    kdec: u32,
}

impl Grain {
    fn trigger(&mut self, fc: f32, bw: f32, amp: f32, sr: f32) {
        self.active = true;
        self.phase = 0.0;
        self.phase_inc = TAU * fc / sr;
        self.env = 1.0;
        self.decay = (-PI * bw / sr).exp();
        self.amp = amp;
        self.age = 0;
        self.kris = (0.003 * sr) as u32;
        let dur_s = (4.0 / (PI * bw)).clamp(0.004, 0.022);
        self.kdur = (dur_s * sr) as u32;
        self.kdec = (0.003 * sr) as u32;
    }

    #[inline(always)]
    fn process(&mut self) -> f32 {
        if !self.active { return 0.0; }
        let s = fast_sin(self.phase);
        self.phase += self.phase_inc;
        if self.phase >= TAU { self.phase -= TAU; }

        let mut a = self.env;
        if self.age < self.kris {
            a *= rcos_window(self.age as f32 / self.kris.max(1) as f32);
        }
        if self.age + self.kdec > self.kdur {
            let rem = self.kdur.saturating_sub(self.age) as f32 / self.kdec.max(1) as f32;
            a *= rcos_window(rem);
        }
        self.env *= self.decay;
        self.age += 1;
        if self.age >= self.kdur { self.active = false; }
        s * a * self.amp
    }
}

#[derive(Clone)]
struct Singer {
    sr: f32,
    grains: [[Grain; MAX_GRAINS]; NUM_FORMANTS],
    rr: [usize; NUM_FORMANTS],
    samples_to_trigger: f32,
    detune: f32,
    formant_scale: f32,
    vib_phase: f32,
    vib_rate: f32,
    human_vib: crate::vibrato::HumanVibrato,
    pan_l: f32,
    pan_r: f32,
    rng: u32,
    jitter: f32,
    shimmer: f32,
    breath_lp: f32,
}

impl Singer {
    fn new(sr: f32, idx: usize) -> Self {
        let mut s = Self {
            sr,
            grains: [[Grain::default(); MAX_GRAINS]; NUM_FORMANTS],
            rr: [0; NUM_FORMANTS],
            samples_to_trigger: 0.0,
            detune: 1.0,
            formant_scale: 1.0,
            vib_phase: (idx as f32) * 0.37,
            vib_rate: 5.2,
            human_vib: crate::vibrato::HumanVibrato::default(),
            pan_l: 0.707,
            pan_r: 0.707,
            rng: 0x1234_5677u32.wrapping_add((idx as u32 + 1).wrapping_mul(2654435761)),
            jitter: 0.0,
            shimmer: 1.0,
            breath_lp: 0.0,
        };
        s.configure(idx, 4);
        s
    }

    /// Recompute the ensemble spread for this singer given the active count.
    fn configure(&mut self, idx: usize, count: usize) {
        let frac = if count > 1 { idx as f32 / (count - 1) as f32 * 2.0 - 1.0 } else { 0.0 };
        let pan = frac * 0.8;
        let p = (pan + 1.0) * 0.5 * HALF_PI;
        self.detune = 2.0f32.powf(frac * 0.09 / 12.0); // up to ~9 cents
        self.formant_scale = 1.0 + frac * 0.04;        // +/-4% tract length
        self.vib_rate = 5.2 + frac * 0.8;              // 4.4..6.0 Hz
        self.pan_l = p.cos();
        self.pan_r = p.sin();
    }

    #[inline(always)]
    fn next_rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng & 0xFFFF) as f32 / 65535.0 * 2.0 - 1.0
    }

    /// A fresh note. Each singer's first grain lands at its own moment
    /// inside the first few milliseconds: singers that all started on the
    /// same sample summed coherently, then beat against each other into a
    /// dip a tenth of a second in.
    fn reset(&mut self) {
        for fo in &mut self.grains { for g in fo.iter_mut() { g.active = false; } }
        self.samples_to_trigger = self.next_rand().abs() * self.sr * 0.005;
        self.breath_lp = 0.0;
        self.vib_phase = self.next_rand().abs();
        self.human_vib = crate::vibrato::HumanVibrato::default();
        self.human_vib.vib_phase = self.vib_phase;
    }

    #[inline]
    fn process(&mut self, f0: f32, vib_depth: f32, breath_amt: f32,
               formants: &[(f32, f32, f32); NUM_FORMANTS]) -> (f32, f32) {
        // Human vibrato per singer (flutter + per-cycle wander + non-sinusoidal
        // shape) instead of a plain sine; output kept as a small pitch ratio to
        // match the former magnitude. rng decoupled into a local for the closure.
        let vib = {
            let mut r = self.rng;
            let (v, _s, _a) = self.human_vib.step(
                self.sr, 0.0, self.vib_rate, 1.0, vib_depth, 1.0, 1.0, 0.0, 0.0,
                || { r ^= r << 13; r ^= r >> 17; r ^= r << 5; (r & 0xFFFF) as f32 / 65535.0 * 2.0 - 1.0 },
            );
            self.rng = r;
            v
        };
        let f = f0 * self.detune * (1.0 + vib + self.jitter * 0.01);

        if self.samples_to_trigger <= 0.0 {
            self.jitter = self.next_rand() * 0.3;
            self.shimmer = 1.0 + self.next_rand() * 0.12;
            for fi in 0..NUM_FORMANTS {
                let (fc, bw, amp) = formants[fi];
                let slot = self.rr[fi];
                self.grains[fi][slot].trigger(fc * self.formant_scale, bw, amp * self.shimmer, self.sr);
                self.rr[fi] = (slot + 1) % MAX_GRAINS;
            }
            let period = (self.sr / f.max(20.0)).max(1.0);
            self.samples_to_trigger += period;
        }
        self.samples_to_trigger -= 1.0;

        let mut mono = 0.0f32;
        for fi in 0..NUM_FORMANTS {
            for g in self.grains[fi].iter_mut() {
                mono += g.process();
            }
        }
        if breath_amt > 0.0 {
            let n = self.next_rand();
            self.breath_lp += 0.35 * (n - self.breath_lp);
            mono += (n - self.breath_lp) * 0.05 * breath_amt;
        }
        (mono * self.pan_l, mono * self.pan_r)
    }
}

/// A choir of `count` decorrelated FOF singers (1..MAX_SINGERS).
#[derive(Clone)]
pub struct FofChoir {
    singers: Vec<Singer>,
    count: usize,
    cached: [(f32, f32, f32); NUM_FORMANTS],
    c_vowel: f32,
    c_bright: f32,
    c_valid: bool,
}

impl FofChoir {
    pub fn new(sr: f32) -> Self {
        Self {
            singers: (0..MAX_SINGERS).map(|i| Singer::new(sr, i)).collect(),
            count: 4,
            cached: [(0.0, 0.0, 0.0); NUM_FORMANTS],
            c_vowel: -1.0,
            c_bright: -1.0,
            c_valid: false,
        }
    }

    pub fn note_on(&mut self) {
        for s in &mut self.singers { s.reset(); }
    }

    fn set_count(&mut self, n: usize) {
        let n = n.clamp(1, MAX_SINGERS);
        if n != self.count {
            self.count = n;
            for i in 0..n { self.singers[i].configure(i, n); }
        }
    }

    /// Render one stereo sample.
    /// * `f0` note frequency, `vowel` 0..1, `vib_depth` ~0.01-0.03,
    /// * `breath` 0..1, `brightness` 0..1 (0.5 neutral), `n_singers` 1..6.
    #[inline]
    pub fn process(&mut self, f0: f32, vowel: f32, vib_depth: f32,
                   breath: f32, brightness: f32, n_singers: usize) -> (f32, f32) {
        self.set_count(n_singers);
        // Cache the (powf-heavy) formant set; recompute only on change.
        if !self.c_valid || (vowel - self.c_vowel).abs() > 1e-4 || (brightness - self.c_bright).abs() > 1e-4 {
            self.cached = vowel_formants(vowel, brightness);
            self.c_vowel = vowel;
            self.c_bright = brightness;
            self.c_valid = true;
        }
        let mut l = 0.0f32;
        let mut r = 0.0f32;
        for i in 0..self.count {
            let (sl, sr_) = self.singers[i].process(f0, vib_depth, breath, &self.cached);
            l += sl;
            r += sr_;
        }
        let g = 2.6 / self.count as f32;
        (l * g, r * g)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn produces_audio() {
        let mut c = FofChoir::new(48_000.0);
        c.note_on();
        let mut peak = 0.0f32;
        for _ in 0..48_000 {
            let (l, _r) = c.process(261.6, 0.0, 0.02, 0.3, 0.5, 4);
            peak = peak.max(l.abs());
        }
        assert!(peak > 0.02, "FOF choir silent (peak {peak})");
    }

    /// Five singers hold their level from the first moments: the onset is
    /// not a coherent burst followed by a dip.
    #[test]
    fn the_singers_do_not_start_phase_locked() {
        let sr = 48_000.0f32;
        let mut c = FofChoir::new(sr);
        c.note_on();
        let rms = |c: &mut FofChoir, n: usize| {
            let mut acc = 0.0f64;
            for _ in 0..n {
                let (l, _) = c.process(261.6, 0.0, 0.01, 0.3, 0.5, 5);
                acc += (l as f64) * (l as f64);
            }
            (acc / n as f64).sqrt()
        };
        let _ = rms(&mut c, (0.02 * sr) as usize);
        let early = rms(&mut c, (0.03 * sr) as usize);
        let dip = rms(&mut c, (0.15 * sr) as usize);
        let later = rms(&mut c, (0.3 * sr) as usize);
        assert!(early < 2.0 * later, "coherent burst: {early} vs {later}");
        assert!(dip > 0.5 * later, "dip: {dip} vs {later}");
    }

    /// cargo test --release --lib fof::tests::profile -- --ignored --nocapture
    #[test]
    #[ignore = "diagnostic"]
    fn profile() {
        let sr = 48_000.0f32;
        for n in [1usize, 4, 6] {
            let mut c = FofChoir::new(sr);
            c.note_on();
            // warm up
            for _ in 0..2048 { let _ = c.process(261.6, 0.0, 0.02, 0.3, 0.5, n); }
            let blocks = 200_000;
            let t0 = Instant::now();
            for _ in 0..blocks { let _ = c.process(261.6, 0.0, 0.02, 0.3, 0.5, n); }
            let ns = t0.elapsed().as_nanos() as f64 / blocks as f64;
            eprintln!("{} singers: {:.1} ns/sample ({:.2}% of one core)",
                      n, ns, ns * sr as f64 / 1e9 * 100.0);
        }
    }
}
