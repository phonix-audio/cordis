//! Per-sample stereo voice generators for the rock acoustic drum kit (the RockOpera
//! DrumKitStyle). Each struct synthesizes ONE drum sample-by-sample, carrying its loop
//! state so the cost spreads over the voice's lifetime instead of landing on the note-on
//! callback (real-time safe by construction). The voices output FULL baked stereo:
//! decorrelated L/R crash, panned toms, hi-hat / ride tilt -- a true-stereo acoustic kit.

use std::f32::consts::TAU;

/// Rock tom tunings (fundamental Hz, pan): high rack toms left, floor toms right.
pub const TOM_TUNINGS: [(f32, f32); 6] = [
    (220.0, -0.35), (180.0, -0.30), (150.0, -0.15),
    (120.0,  0.00), (95.0,   0.35), (78.0,   0.30),
];

/// One active drum voice. `step()` advances one sample and returns its stereo
/// contribution (pre patch-level); `done()` is true once the voice has fully decayed.
#[derive(Clone, Debug)]
pub enum Voice {
    Kick(Kick),
    Snare(Snare),
    Snare909(Snare909),
    Clap(Clap),
    Hat(Hat),
    Ride(Ride),
    Crash(Crash),
    Tom(Tom),
}

impl Voice {
    /// Build the rock-kit voice for a GM drum slot index (0=kick .. 23), at velocity
    /// `vel` (0..1), with FULL baked stereo (decorrelated L/R crash, panned toms, hi-hat/
    /// ride tilt). Returns None for slots with no rock voice. `seed` only affects the
    /// crash's stereo decorrelation.
    pub fn rock(sr: f32, slot: usize, vel: f32, seed: u32) -> Option<Voice> {
        let v = match slot {
            0      => Voice::Kick(Kick::new(sr, vel)),                       // BassDrum1
            1      => Voice::Snare(Snare::new(sr, vel, 1.0)),                // SideStick (rimshot)
            2      => Voice::Snare(Snare::new(sr, vel, 0.3)),                // AcousticSnare
            4      => Voice::Snare909(Snare909::new(sr, vel, 1.0, 7000.0)),  // ElectricSnare (909)
            3      => Voice::Clap(Clap::new(sr, vel)),                       // HandClap
            6 | 8  => Voice::Hat(Hat::new(sr, vel, false)),                  // Closed / Pedal HiHat
            10     => Voice::Hat(Hat::new(sr, vel, true)),                   // OpenHiHat
            15 | 23=> Voice::Ride(Ride::new(sr, vel, false)),               // Ride 1 / 2
            17     => Voice::Ride(Ride::new(sr, vel, true)),                 // RideBell
            // Crash family. The kit places them across the stereo field the way a
            // drummer sets cymbals up (and the way the validated rock kit recorded
            // them): crash 1 centred, the china left, crash 2 right. A track that
            // wants them elsewhere moves them with the per-sound pan.
            13 | 19 => Voice::Crash(Crash::new(sr, vel, 0.0, seed)),  // Crash 1 / Splash (centre)
            16      => Voice::Crash(Crash::new(sr, vel, -0.4, seed)), // Chinese (left)
            21      => Voice::Crash(Crash::new(sr, vel, 0.4, seed)),  // Crash 2 (right)
            5      => { let (hz, p) = TOM_TUNINGS[0]; Voice::Tom(Tom::new(sr, vel, hz, p)) }
            7      => { let (hz, p) = TOM_TUNINGS[1]; Voice::Tom(Tom::new(sr, vel, hz, p)) }
            9      => { let (hz, p) = TOM_TUNINGS[2]; Voice::Tom(Tom::new(sr, vel, hz, p)) }
            11     => { let (hz, p) = TOM_TUNINGS[3]; Voice::Tom(Tom::new(sr, vel, hz, p)) }
            12     => { let (hz, p) = TOM_TUNINGS[4]; Voice::Tom(Tom::new(sr, vel, hz, p)) }
            14     => { let (hz, p) = TOM_TUNINGS[5]; Voice::Tom(Tom::new(sr, vel, hz, p)) }
            _      => return None,
        };
        Some(v)
    }

    #[inline]
    pub fn step(&mut self) -> (f32, f32) {
        match self {
            Voice::Kick(v)     => v.step(),
            Voice::Snare(v)    => v.step(),
            Voice::Snare909(v) => v.step(),
            Voice::Clap(v)     => v.step(),
            Voice::Hat(v)      => v.step(),
            Voice::Ride(v)     => v.step(),
            Voice::Crash(v)    => v.step(),
            Voice::Tom(v)      => v.step(),
        }
    }
    #[inline]
    pub fn done(&self) -> bool {
        match self {
            Voice::Kick(v)     => v.i >= v.n,
            Voice::Snare(v)    => v.i >= v.n,
            Voice::Snare909(v) => v.i >= v.n,
            Voice::Clap(v)     => v.i >= v.n,
            Voice::Hat(v)      => v.i >= v.n,
            Voice::Ride(v)     => v.i >= v.n,
            Voice::Crash(v)    => v.i >= v.n,
            Voice::Tom(v)      => v.i >= v.n,
        }
    }
}

#[inline]
fn fade(n: usize, i: usize, fade_n: usize) -> f32 {
    ((n - i) as f32 / fade_n as f32).min(1.0)
}

// ── Kick ────────────────────────────────────────────────────────────────────
#[derive(Clone, Debug)]
pub struct Kick {
    sr: f32, i: usize, n: usize, fade_n: usize, vel: f32,
    ph: f32, ph_sub: f32, ph_shell: f32, seed: u32, bp1: f32, bp2: f32,
    k_lo: f32, k_hi: f32,
}
impl Kick {
    pub fn new(sr: f32, vel: f32) -> Self {
        Kick {
            sr, i: 0, n: (0.40 * sr) as usize, fade_n: (0.020 * sr) as usize, vel,
            ph: 0.0, ph_sub: 0.0, ph_shell: 0.0, seed: 1, bp1: 0.0, bp2: 0.0,
            k_lo: 1.0 - (-TAU * 2200.0 / sr).exp(), k_hi: 1.0 - (-TAU * 5000.0 / sr).exp(),
        }
    }
    #[inline]
    fn step(&mut self) -> (f32, f32) {
        let i = self.i; let sr = self.sr; let ts = i as f32 / sr;
        let sweep = 1.0 + 0.88 * (-ts / 0.022).exp();
        self.ph += TAU * 56.0 * sweep / sr;
        let body = self.ph.sin() * (-ts / 0.13).exp();
        self.ph_sub += TAU * 40.0 / sr;
        let sub = self.ph_sub.sin() * (-ts / 0.19).exp() * 0.55;
        self.ph_shell += TAU * 110.0 / sr;
        let shell = self.ph_shell.sin() * (-ts / 0.05).exp() * 0.26;
        self.seed ^= self.seed << 13; self.seed ^= self.seed >> 17; self.seed ^= self.seed << 5;
        let w = (self.seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
        self.bp1 += (w - self.bp1) * self.k_hi;
        self.bp2 += (self.bp1 - self.bp2) * self.k_lo;
        let beater = (self.bp1 - self.bp2) * (-ts / 0.0035).exp() * 1.15;
        let f = fade(self.n, i, self.fade_n);
        let v = (((body + sub + shell) * 1.5).tanh() * 0.92 + beater) * self.vel * f;
        self.i += 1;
        (v, v)
    }
}

// ── Snare (rock) ──────────────────────────────────────────────────────────────
#[derive(Clone, Debug)]
pub struct Snare {
    sr: f32, i: usize, n: usize, fade_n: usize, vel: f32, rim: f32,
    p1: f32, p2: f32, p3: f32, seed: u32, hp_s: f32, lp_a: f32, lp_b: f32,
    k_hp3k: f32, k_lp65: f32, k_lp15: f32,
}
impl Snare {
    pub fn new(sr: f32, vel: f32, rim: f32) -> Self {
        Snare {
            sr, i: 0, n: (0.40 * sr) as usize, fade_n: (0.015 * sr) as usize, vel, rim,
            p1: 0.0, p2: 0.0, p3: 0.0, seed: 1, hp_s: 0.0, lp_a: 0.0, lp_b: 0.0,
            k_hp3k: 1.0 - (-TAU * 3000.0 / sr).exp(),
            k_lp65: 1.0 - (-TAU * 6500.0 / sr).exp(),
            k_lp15: 1.0 - (-TAU * 1500.0 / sr).exp(),
        }
    }
    #[inline]
    fn step(&mut self) -> (f32, f32) {
        let i = self.i; let sr = self.sr; let ts = i as f32 / sr;
        let settle = 1.0 + 0.4 * (-ts / 0.008).exp();
        self.p1 += TAU * 196.0 * settle / sr;
        self.p2 += TAU * 322.0 * settle / sr;
        let heads = self.p1.sin() * (-ts / 0.09).exp() + 0.6 * self.p2.sin() * (-ts / 0.055).exp();
        self.p3 += TAU * 453.0 / sr;
        let ring = self.p3.sin() * (-ts / 0.40).exp() * 0.14;
        self.seed ^= self.seed << 13; self.seed ^= self.seed >> 17; self.seed ^= self.seed << 5;
        let w = (self.seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
        self.hp_s += (w - self.hp_s) * self.k_hp3k;
        let crack = (w - self.hp_s) * (-ts / 0.013).exp() * (1.25 + self.rim * 0.5);
        self.lp_a += (w - self.lp_a) * self.k_lp65;
        self.lp_b += (self.lp_a - self.lp_b) * self.k_lp15;
        let wires = (self.lp_a - self.lp_b) * (-ts / 0.19).exp() * 0.95;
        let f = fade(self.n, i, self.fade_n);
        let v = ((heads * 1.0 + ring) * 1.45).tanh() * 0.74 + (crack + wires) * 0.85;
        let v = v * self.vel * f;
        self.i += 1;
        (v, v)
    }
}

// ── Snare 909 ────────────────────────────────────────────────────────────────
#[derive(Clone, Debug)]
pub struct Snare909 {
    sr: f32, i: usize, n: usize, fade_n: usize, vel: f32,
    f1: f32, f2: f32, seed: u32, p1: f32, p2: f32, lp_n: f32, hp_lp: f32,
    k_lp: f32, k_hp: f32,
}
impl Snare909 {
    pub fn new(sr: f32, vel: f32, tune: f32, noise_lp: f32) -> Self {
        Snare909 {
            sr, i: 0, n: (0.26 * sr) as usize, fade_n: (0.012 * sr) as usize, vel,
            f1: 180.0 * tune, f2: 330.0 * tune, seed: 1, p1: 0.0, p2: 0.0, lp_n: 0.0, hp_lp: 0.0,
            k_lp: 1.0 - (-TAU * noise_lp / sr).exp(), k_hp: 1.0 - (-TAU * 2500.0 / sr).exp(),
        }
    }
    #[inline]
    fn step(&mut self) -> (f32, f32) {
        let i = self.i; let sr = self.sr; let ts = i as f32 / sr;
        let sw = 1.0 + (-ts / 0.010).exp();
        self.p1 += TAU * self.f1 * sw / sr;
        self.p2 += TAU * self.f2 * sw / sr;
        let tone = (self.p1.sin() * 0.55 + self.p2.sin() * 0.45) * (-ts / 0.065).exp();
        self.seed ^= self.seed << 13; self.seed ^= self.seed >> 17; self.seed ^= self.seed << 5;
        let w = (self.seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
        self.lp_n += (w - self.lp_n) * self.k_lp;
        self.hp_lp += (w - self.hp_lp) * self.k_hp;
        let snappy = (w - self.hp_lp) * (-ts / 0.055).exp();
        let tail = self.lp_n * (-ts / 0.110).exp();
        let f = fade(self.n, i, self.fade_n);
        let x = ((tone * 0.5 + snappy * 0.8 + tail * 0.35) * self.vel * 1.6).tanh() * 0.8 * f;
        self.i += 1;
        (x, x)
    }
}

// ── Clap 909 ─────────────────────────────────────────────────────────────────
#[derive(Clone, Debug)]
pub struct Clap {
    sr: f32, i: usize, n: usize, fade_n: usize, vel: f32,
    seed: u32, x1: f32, x2: f32, y1: f32, y2: f32, hp_lp: f32,
    b0: f32, b1: f32, b2: f32, a1: f32, a2: f32, k_hp: f32,
}
impl Clap {
    pub fn new(sr: f32, vel: f32) -> Self {
        let w = TAU * 1100.0 / sr;
        let (cw, sw) = (w.cos(), w.sin());
        let alpha = sw / (2.0 * 2.0);
        let a0 = 1.0 + alpha;
        Clap {
            sr, i: 0, n: (0.24 * sr) as usize, fade_n: (0.012 * sr) as usize, vel,
            seed: 1, x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0, hp_lp: 0.0,
            b0: sw / 2.0 / a0, b1: 0.0, b2: -sw / 2.0 / a0,
            a1: (-2.0 * cw) / a0, a2: (1.0 - alpha) / a0,
            k_hp: 1.0 - (-TAU * 400.0 / sr).exp(),
        }
    }
    #[inline]
    fn step(&mut self) -> (f32, f32) {
        const BURSTS: [f32; 3] = [0.0, 0.010, 0.020];
        let i = self.i; let sr = self.sr; let ts = i as f32 / sr;
        let mut env = 0.0f32;
        for &b in &BURSTS {
            if ts >= b { env += (-(ts - b) / 0.008).exp(); }
        }
        if ts >= 0.028 { env += 0.9 * (-(ts - 0.028) / 0.120).exp(); }
        self.seed ^= self.seed << 13; self.seed ^= self.seed >> 17; self.seed ^= self.seed << 5;
        let w_n = (self.seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
        self.hp_lp += (w_n - self.hp_lp) * self.k_hp;
        let x = w_n - self.hp_lp;
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1; self.x1 = x; self.y2 = self.y1; self.y1 = y;
        let f = fade(self.n, i, self.fade_n);
        let v = ((y * env * self.vel) * 2.2).tanh() * 0.85 * f;
        self.i += 1;
        (v, v)
    }
}

// ── Hi-hat (closed/open) ─────────────────────────────────────────────────────
#[derive(Clone, Debug)]
pub struct Hat {
    sr: f32, i: usize, n: usize, fade_n: usize, vel: f32, t_dec: f32,
    phs: [f32; 6], ph_rm: f32, seed: u32, f_svf: f32, q: f32, lp: f32, bp: f32,
}
impl Hat {
    pub fn new(sr: f32, vel: f32, open: bool) -> Self {
        let dur = if open { 0.40 } else { 0.050 };
        Hat {
            sr, i: 0, n: (dur * sr) as usize, fade_n: (0.008 * sr) as usize, vel,
            t_dec: if open { 0.19 } else { 0.020 },
            phs: [0.0; 6], ph_rm: 0.0, seed: 1,
            f_svf: 2.0 * (std::f32::consts::PI * 7200.0 / sr).sin(), q: 0.9, lp: 0.0, bp: 0.0,
        }
    }
    #[inline]
    fn step(&mut self) -> (f32, f32) {
        const FREQS: [f32; 6] = [3613.0, 4327.0, 5219.0, 6131.0, 7459.0, 8923.0];
        let i = self.i; let sr = self.sr; let ts = i as f32 / sr;
        let mut m = 0.0f32;
        for (p, &fr) in self.phs.iter_mut().zip(FREQS.iter()) {
            *p += fr / sr;
            if *p >= 1.0 { *p -= 1.0; }
            m += if *p < 0.5 { 1.0 } else { -1.0 };
        }
        m /= 6.0;
        self.ph_rm += TAU * 5113.0 / sr;
        let metal = m * (0.5 + 0.5 * self.ph_rm.sin());
        self.seed ^= self.seed << 13; self.seed ^= self.seed >> 17; self.seed ^= self.seed << 5;
        let w = (self.seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
        let inp = metal * 0.75 + w * 0.5;
        self.lp += self.f_svf * self.bp;
        let hp = inp - self.lp - self.q * self.bp;
        self.bp += self.f_svf * hp;
        let f = fade(self.n, i, self.fade_n);
        let v = hp * (-ts / self.t_dec).exp() * self.vel * 0.7 * f;
        self.i += 1;
        (v * 0.94, v * 1.06)
    }
}

// ── Ride ─────────────────────────────────────────────────────────────────────
#[derive(Clone, Debug)]
pub struct Ride {
    sr: f32, i: usize, n: usize, fade_n: usize, vel: f32, bell: bool,
    pph: [f32; 9], bph: [f32; 6], seed: u32, hp: f32, k_hp: f32,
}
impl Ride {
    pub fn new(sr: f32, vel: f32, bell: bool) -> Self {
        Ride {
            sr, i: 0, n: (1.3 * sr) as usize, fade_n: (0.030 * sr) as usize, vel, bell,
            pph: [0.0; 9], bph: [0.0; 6], seed: 1, hp: 0.0, k_hp: 1.0 - (-TAU * 5000.0 / sr).exp(),
        }
    }
    #[inline]
    fn step(&mut self) -> (f32, f32) {
        const PING_F: [f32; 9] = [2950.0, 3417.0, 3881.0, 4730.0, 5203.0, 5879.0, 6451.0, 7321.0, 8243.0];
        const BELL_F: [f32; 6] = [1180.0, 1416.0, 1773.0, 2360.0, 2950.0, 3540.0];
        let i = self.i; let sr = self.sr; let ts = i as f32 / sr;
        let mut ping = 0.0f32;
        for (k, (p, &fr)) in self.pph.iter_mut().zip(PING_F.iter()).enumerate() {
            *p += TAU * fr / sr;
            ping += p.sin() * (-ts / (0.6 - 0.045 * k as f32).max(0.12)).exp();
        }
        ping *= 0.17;
        let bell_v = if self.bell {
            let mut b = 0.0f32;
            for (k, (p, &fr)) in self.bph.iter_mut().zip(BELL_F.iter()).enumerate() {
                *p += TAU * fr / sr;
                b += p.sin() * (-ts / (0.55 - 0.05 * k as f32).max(0.15)).exp();
            }
            b * (1.0 / BELL_F.len() as f32) * 1.4
        } else { 0.0 };
        self.seed ^= self.seed << 13; self.seed ^= self.seed >> 17; self.seed ^= self.seed << 5;
        let w = (self.seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
        self.hp += (w - self.hp) * self.k_hp;
        let onset = (ts / 0.025).min(1.0);
        let wash = (w - self.hp) * onset * (-ts / 0.45).exp() * 0.26;
        let f = fade(self.n, i, self.fade_n);
        let v = (ping + bell_v + wash) * self.vel * f;
        self.i += 1;
        (v * 0.92, v * 1.08)
    }
}

// ── Crash (per-hit seed, polyphonic) ─────────────────────────────────────────
#[derive(Clone, Debug)]
pub struct Crash {
    sr: f32, i: usize, n: usize, fade_n: usize, vel: f32,
    seed_l: u32, seed_r: u32, hp_l: f32, hp_r: f32, k_hp: f32,
    cph_l: [f32; 16], cph_r: [f32; 16], gl: f32, gr: f32,
}
impl Crash {
    pub fn new(sr: f32, vel: f32, pan: f32, seed: u32) -> Self {
        let angle = (pan.clamp(-1.0, 1.0) + 1.0) * 0.5 * std::f32::consts::FRAC_PI_2;
        Crash {
            sr, i: 0, n: (2.4 * sr) as usize, fade_n: (0.060 * sr) as usize, vel,
            seed_l: seed.wrapping_mul(747_796_405) | 1,
            seed_r: seed.wrapping_mul(2_891_336_453) | 1,
            hp_l: 0.0, hp_r: 0.0, k_hp: 1.0 - (-TAU * 900.0 / sr).exp(),
            cph_l: [0.0; 16], cph_r: [0.0; 16], gl: angle.cos(), gr: angle.sin(),
        }
    }
    #[inline]
    fn step(&mut self) -> (f32, f32) {
        const CF: [f32; 16] = [383.0, 547.0, 731.0, 932.0, 1187.0, 1453.0, 1789.0, 2143.0,
                               2591.0, 3067.0, 3593.0, 4231.0, 4937.0, 5683.0, 6521.0, 7309.0];
        let i = self.i; let sr = self.sr; let ts = i as f32 / sr;
        self.seed_l ^= self.seed_l << 13; self.seed_l ^= self.seed_l >> 17; self.seed_l ^= self.seed_l << 5;
        self.seed_r ^= self.seed_r << 13; self.seed_r ^= self.seed_r >> 17; self.seed_r ^= self.seed_r << 5;
        let wl = (self.seed_l as f32 / u32::MAX as f32) * 2.0 - 1.0;
        let wr = (self.seed_r as f32 / u32::MAX as f32) * 2.0 - 1.0;
        self.hp_l += (wl - self.hp_l) * self.k_hp;
        self.hp_r += (wr - self.hp_r) * self.k_hp;
        let (mut ml, mut mr) = (0.0f32, 0.0f32);
        for (k, &fr) in CF.iter().enumerate() {
            let e = (-ts / (1.25 - 0.05 * k as f32).max(0.18)).exp();
            self.cph_l[k] += TAU * fr / sr;
            self.cph_r[k] += TAU * fr * 1.006 / sr;
            ml += self.cph_l[k].sin() * e;
            mr += self.cph_r[k].sin() * e;
        }
        let mm = 1.0 / CF.len() as f32;
        let env = (-ts / 0.95).exp() + (-ts / 0.05).exp() * 0.8;
        let f = fade(self.n, i, self.fade_n);
        let l = ((wl - self.hp_l) * env * 0.45 + ml * mm * 0.55) * self.vel * f;
        let r = ((wr - self.hp_r) * env * 0.45 + mr * mm * 0.55) * self.vel * f;
        self.i += 1;
        (l * (0.5 + self.gl), r * (0.5 + self.gr))
    }
}

// ── Tom (research modal, RockOpera voicing) ──────────────────────────────────
#[derive(Clone, Debug)]
pub struct Tom {
    sr: f32, i: usize, n: usize, fade_n: usize, vel: f32, pitch_hz: f32,
    modes: [(f32, f32, f32); 6], base_tau: f32, glide_depth: f32, glide_tau: f32,
    split: f32, drive: f32, outg: f32, sub_amp: f32, sub_tau: f32,
    seed: u32, bp1: f32, bp2: f32, k_bp: f32, ph: [f32; 6], ph2: [f32; 6], sub_ph: f32,
    gl: f32, gr: f32,
}
impl Tom {
    pub fn new(sr: f32, vel: f32, pitch_hz: f32, pan: f32) -> Self {
        // RockOpera tom voicing constants (theatrical: deeper, longer, sub-octave thump).
        let (fund_amp, tau_mul, gd0, gdv, drive, outg, sub_amp) =
            (1.22f32, 1.35f32, 0.18f32, 0.09f32, 1.55f32, 0.88f32, 0.42f32);
        let modes: [(f32, f32, f32); 6] = [
            (1.000, 1.00 * fund_amp, 1.00),
            (1.594, 0.90, 0.74),
            (2.136, 0.52, 0.50),
            (2.296, 0.34, 0.44),
            (2.653, 0.22, 0.32),
            (2.918, 0.14, 0.24),
        ];
        let (bt0, dur0) = if pitch_hz < 110.0 { (0.62, 1.45) } else { (0.40, 0.92) };
        let base_tau = bt0 * tau_mul;
        let dur = dur0 * tau_mul;
        let angle = (pan.clamp(-1.0, 1.0) + 1.0) * 0.5 * std::f32::consts::FRAC_PI_2;
        Tom {
            sr, i: 0, n: (dur * sr) as usize, fade_n: (0.030 * sr) as usize, vel, pitch_hz,
            modes, base_tau, glide_depth: gd0 + gdv * vel.clamp(0.0, 1.0), glide_tau: base_tau * 0.5,
            split: 0.016, drive, outg, sub_amp, sub_tau: base_tau * 1.05,
            seed: 1, bp1: 0.0, bp2: 0.0,
            k_bp: 1.0 - (-TAU * (pitch_hz * 4.0).min(sr * 0.4) / sr).exp(),
            ph: [0.0; 6], ph2: [0.0; 6], sub_ph: 0.0,
            gl: angle.cos(), gr: angle.sin(),
        }
    }
    #[inline]
    fn step(&mut self) -> (f32, f32) {
        let i = self.i; let sr = self.sr; let ts = i as f32 / sr;
        let glide = 1.0 + self.glide_depth * (-ts / self.glide_tau).exp();
        let mut body = 0.0f32;
        for (mi, &(r, a, dm)) in self.modes.iter().enumerate() {
            let f = self.pitch_hz * r * glide;
            self.ph[mi] += TAU * f / sr;
            self.ph2[mi] += TAU * f * (1.0 + self.split) / sr;
            let env = (-ts / (self.base_tau * dm)).exp();
            body += a * env * (self.ph[mi].sin() + 0.6 * self.ph2[mi].sin());
        }
        body *= 0.42;
        if self.sub_amp > 0.0 {
            self.sub_ph += TAU * self.pitch_hz * 0.5 * glide / sr;
            body += self.sub_amp * self.sub_ph.sin() * (-ts / self.sub_tau).exp();
        }
        self.seed ^= self.seed << 13; self.seed ^= self.seed >> 17; self.seed ^= self.seed << 5;
        let w = (self.seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
        let click = w * (-ts / 0.0045).exp();
        self.bp1 += (w - self.bp1) * self.k_bp;
        self.bp2 += (self.bp1 - self.bp2) * self.k_bp;
        let thwack = (self.bp1 - self.bp2) * (-ts / 0.028).exp();
        let attack = click * 0.45 + thwack * 0.9;
        let f = fade(self.n, i, self.fade_n);
        let v = ((body * self.drive).tanh() * self.outg + attack * 0.5) * self.vel * f;
        self.i += 1;
        (v * self.gl, v * self.gr)
    }
}
