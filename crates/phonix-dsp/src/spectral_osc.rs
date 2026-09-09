//! SpectralOsc — an evolving additive-spectrum oscillator.
//!
//! A bank of `NP` partials summed at a fundamental, with a spectral-tilt
//! envelope and a slow shimmer so the timbre keeps evolving on a held note.
//! Gives engines complex, living spectra beyond a plain sine/saw. The
//! powf/sin-heavy envelope is recomputed only at control rate (every 32
//! samples); per sample only the partial phases advance and sum.
//!
//! Deterministic (no RNG): the caller supplies `tilt` (brightness), `inharm`
//! (0 = harmonic, up = bell-like) and `evolve` (drift rate); a played-note
//! engine drives `f0` and can sweep `tilt` from an envelope for motion.

use std::f32::consts::TAU;

use crate::fastmath::fast_sin;

/// Partial count. 24 covers a rich spectrum while staying affordable.
pub const NP: usize = 24;

#[derive(Clone)]
pub struct SpectralOsc {
    phases: [f32; NP],
    amp: [f32; NP],
    shimmer_phase: f32,
    ctrl: u32,
    sample_rate: f32,
}

impl SpectralOsc {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            // Golden-ratio phase spread so the partials don't start in phase.
            phases: std::array::from_fn(|k| (k as f32 * 0.6180339) % 1.0),
            amp: [0.0; NP],
            shimmer_phase: 0.0,
            ctrl: 0,
            sample_rate: sample_rate.max(1.0),
        }
    }

    /// One stereo sample. `tilt`/`inharm`/`evolve` are 0..1; `f0` in Hz.
    #[inline]
    pub fn process(&mut self, f0: f32, tilt: f32, inharm: f32, evolve: f32) -> (f32, f32) {
        let sample_rate = self.sample_rate;
        let dt = 1.0 / sample_rate;
        let nyq = sample_rate * 0.45;
        let inh = inharm.clamp(0.0, 1.0) * 0.012;

        // Control-rate envelope refresh (powf + sin), every 32 samples.
        if self.ctrl % 32 == 0 {
            self.shimmer_phase += 32.0 * dt * (0.05 + evolve.clamp(0.0, 1.0) * 0.40);
            if self.shimmer_phase > 1024.0 { self.shimmer_phase -= 1024.0; }
            let sh = self.shimmer_phase * TAU;
            let tilt = tilt.clamp(0.0, 1.0);
            for k in 0..NP {
                let kk = (k + 1) as f32;
                let f = f0 * kk * (1.0 + inh * (kk - 1.0));
                if f >= nyq { self.amp[k] = 0.0; continue; }
                let env = (1.0 / kk).powf(1.7 - tilt); // brighter tilt = flatter spectrum
                let shimmer = 0.6 + 0.4 * (sh * (1.0 + k as f32 * 0.11) + k as f32 * 1.7).sin();
                self.amp[k] = env * shimmer;
            }
        }
        self.ctrl = self.ctrl.wrapping_add(1);

        let (mut l, mut r) = (0.0_f32, 0.0_f32);
        let mut norm = 0.0_f32;
        for k in 0..NP {
            let amp = self.amp[k];
            if amp <= 1e-5 { continue; }
            let kk = (k + 1) as f32;
            let f = f0 * kk * (1.0 + inh * (kk - 1.0));
            let ph = &mut self.phases[k];
            *ph += f * dt;
            if *ph >= 1.0 { *ph -= ph.floor(); }
            // Table-lookup sine: the per-sample cost is NP of these, so the fast
            // path matters. The tiny interp error is inaudible in an additive pad.
            let s = fast_sin(*ph * TAU) * amp;
            // Even partials lean left, odd lean right, for a wide spectral image.
            if k % 2 == 0 { l += s; r += s * 0.7; } else { l += s * 0.7; r += s; }
            norm += amp;
        }
        let g = 0.5 / norm.max(0.001);
        (l * g, r * g)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_bounded_audible_signal() {
        let mut osc = SpectralOsc::new(48_000.0);
        let mut peak = 0.0f32;
        for _ in 0..48_000 {
            let (l, r) = osc.process(220.0, 0.5, 0.2, 0.4);
            peak = peak.max(l.abs()).max(r.abs());
        }
        assert!(peak > 1e-3, "spectral osc silent (peak {peak})");
        assert!(peak < 2.0, "spectral osc too hot (peak {peak})");
    }

    #[test]
    fn tilt_changes_the_spectrum_energy() {
        let rms = |tilt: f32| {
            let mut osc = SpectralOsc::new(48_000.0);
            let mut acc = 0.0f64;
            let n = 24_000;
            for _ in 0..n {
                let (l, _) = osc.process(220.0, tilt, 0.0, 0.0);
                acc += (l * l) as f64;
            }
            (acc / n as f64).sqrt()
        };
        assert!((rms(0.9) - rms(0.1)).abs() > 1e-4, "tilt had no spectral effect");
    }
}
