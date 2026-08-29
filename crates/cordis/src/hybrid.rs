//! Hybrid voice synthesis.
//!
//! The physical model is faithful but expensive: on a pedalled piece thirty to a
//! hundred strings ring at once, and every one of them reads the whole soundboard
//! (3618 modes) every sample. That is the real-time wall.
//!
//! The observation that breaks it: a *struck* note played on its own is a fixed
//! waveform. The strike, the string and the board colouring it are deterministic,
//! so the model can render that note once and every later strike is a buffer read.
//! What a sampler cannot do (the sympathetic halo under the pedal, the coupling
//! between simultaneously sounding notes) stays live as one shared layer rather
//! than as a hundred independent strings.
//!
//! The bank is filled lazily: the first strike of a note plays the live model
//! (correct, zero latency) while its sample renders in the background, and every
//! repeat reads the cache. The expensive per-voice coupling therefore runs only
//! while a note is being heard for the first time, never as a simultaneous crowd.
//!
//! This module is the bank itself: rendering one isolated note through the model,
//! the velocity layering, and the cache. The playback voice, the engine
//! integration and the shared resonance layer build on top of it.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;

use super::engine::{CordisCommand, CordisEngine};
use super::modal_bank::{Mode, ModalBank};
use super::patch::CordisPatch;

/// A request to render one note's velocity layer, tagged with the bank
/// generation it was made under and carrying the patch to render it with:
/// `(generation, note, layer velocity, patch)`. The generation lets a render
/// made against an old model or patch be discarded when it comes back stale.
pub type RenderRequest = FillerJob;

/// What the audio thread may ask of the filler.
pub enum FillerJob {
    /// Render (or disk-load) one `(note, layer)`.
    One(u64, u8, u8, CordisPatch),
    /// Prepare THE WHOLE KEYBOARD: 88 notes x every velocity layer.
    ///
    /// This is the user's specification for the hybrid, verbatim: "tout doit
    /// être prêt avant même de commencer à jouer, et de façon optimale". The
    /// optimal part: everything already on disk is loaded (file reads), and
    /// only the missing samples are rendered — in parallel, across the
    /// machine's cores, at normal priority. Each finished sample is sent back
    /// immediately, so the bank fills visibly and the meter can show progress.
    All(u64, CordisPatch),
}
/// A finished render on its way back to the audio thread, carrying its
/// generation: `(generation, note, layer velocity, sample)`.
pub type RenderDone = (u64, u8, u8, Arc<NoteSample>);

/// Spawn the background bank filler. It renders each requested `(note, layer)`
/// through the physical model OFF the audio thread and sends the sample back, so
/// the first strike of a note is covered live while its sample is prepared and
/// every later strike is a buffer read. It exits when the request channel closes
/// (the engine was dropped).
pub fn spawn_bank_filler(
    sr: f32,
    budget: RenderBudget,
    cache_root: Option<std::path::PathBuf>,
    reqs: Receiver<RenderRequest>,
    done: Sender<RenderDone>,
) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("cordis-bank-filler".into())
        .spawn(move || {
            let one = |note: u8, lv: u8, patch: &CordisPatch| -> Arc<NoteSample> {
                // Disk first: a bank rendered once per timbre is never
                // rendered again. Only filler/preparation threads touch the
                // disk — the audio thread asks and receives over channels.
                let dir = cache_root
                    .as_ref()
                    .map(|r| bank_dir(r, timbre_sig(patch), sr));
                let loaded = dir.as_ref().and_then(|d| load_sample(d, sr, note, lv));
                Arc::new(match loaded {
                    Some(s) => s,
                    None => {
                        let s = render_note_stereo(sr, note, lv, budget, patch);
                        if let Some(d) = &dir {
                            // Best effort: a full disk must not stop the music.
                            if let Err(e) = save_sample(d, sr, &s) {
                                eprintln!("cordis: bank save failed: {e}");
                            }
                        }
                        s
                    }
                })
            };
            // Behind a mutex so the preparation workers can also drain it
            // (Receiver is Send but not Sync; the outer loop and the workers
            // never contend — the outer recv only runs between jobs).
            let reqs = std::sync::Mutex::new(reqs);
            loop {
                let job = match reqs.lock().unwrap().recv() {
                    Ok(j) => j,
                    Err(_) => break,
                };
                match job {
                    FillerJob::One(gen, note, lv, patch) => {
                        let sample = one(note, lv, &patch);
                        if done.send((gen, note, lv, sample)).is_err() {
                            break;
                        }
                    }
                    FillerJob::All(gen, patch) => {
                        // The whole keyboard. Fan the MISSING renders across
                        // the cores (normal priority — the RT threads keep
                        // theirs); everything already on disk is a file read.
                        //
                        // ORDERED FOR A PLAYER, not for an iterator: the
                        // middle of the keyboard first, spreading outward, and
                        // the mf layer before the extremes — so a session
                        // becomes playable in the first minute of a
                        // first-ever preparation instead of after the last
                        // bass note. (A0-and-up order left the region every
                        // piece lives in for the end, behind the longest
                        // renders in the bank.)
                        let mut keys: Vec<(u8, u8)> = Vec::with_capacity(88 * VEL_LAYERS.len());
                        for &lv in &[88u8, 56, 120, 24] {
                            let mut lo = 60i16;
                            let mut hi = 61i16;
                            while lo >= 21 || hi <= 108 {
                                if lo >= 21 {
                                    keys.push((lo as u8, lv));
                                }
                                if hi <= 108 {
                                    keys.push((hi as u8, lv));
                                }
                                lo -= 1;
                                hi += 1;
                            }
                        }
                        let workers = std::thread::available_parallelism()
                            .map(|n| n.get().saturating_sub(2).max(1))
                            .unwrap_or(2);
                        let next = std::sync::atomic::AtomicUsize::new(0);
                        let stop = std::sync::atomic::AtomicBool::new(false);
                        // Notes the player touches DURING the preparation jump
                        // the queue: `One` requests arriving while `All` runs
                        // are drained here and served before the next scheduled
                        // key. Without this a played note's sample sat behind
                        // the whole remaining keyboard — minutes away — which
                        // is the difference between "preparing in the
                        // background" and "unusable while initialising".
                        let urgent: std::sync::Mutex<Vec<(u64, u8, u8, CordisPatch)>> =
                            std::sync::Mutex::new(Vec::new());
                        std::thread::scope(|sc| {
                            for _ in 0..workers {
                                sc.spawn(|| loop {
                                    if stop.load(std::sync::atomic::Ordering::Relaxed) {
                                        return;
                                    }
                                    if let Ok(g) = reqs.try_lock() {
                                        while let Ok(job) = g.try_recv() {
                                            if let FillerJob::One(gg, n, l, p) = job {
                                                urgent.lock().unwrap().push((gg, n, l, p));
                                            }
                                            // A nested All while one runs is
                                            // folded into this run (same keys).
                                        }
                                    }
                                    let hot = urgent.lock().unwrap().pop();
                                    if let Some((g, n, l, p)) = hot {
                                        let sample = one(n, l, &p);
                                        if done.send((g, n, l, sample)).is_err() {
                                            stop.store(true, std::sync::atomic::Ordering::Relaxed);
                                            return;
                                        }
                                        continue;
                                    }
                                    let i = next
                                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    let Some(&(note, lv)) = keys.get(i) else { return };
                                    let sample = one(note, lv, &patch);
                                    if done.send((gen, note, lv, sample)).is_err() {
                                        stop.store(true, std::sync::atomic::Ordering::Relaxed);
                                        return;
                                    }
                                });
                            }
                        });
                    }
                }
            }
        })
        .expect("spawn cordis bank filler thread")
}

/// The five parameters a rendered sample bakes in — the bank's identity.
/// (`mechanics` is rendered at zero and fired live, `gain` multiplies the
/// players live: neither belongs here.)
pub fn timbre_sig(p: &CordisPatch) -> [f32; 5] {
    [p.voicing, p.unison_detune, p.width, p.damper, p.tune]
}

/// The velocities the model is rendered at. A played note picks the nearest
/// layer and scales the amplitude for the remainder, so the felt's velocity
/// dependence is sampled coarsely and the loudness filled in continuously.
pub const VEL_LAYERS: [u8; 4] = [24, 56, 88, 120];

/// The layer index whose velocity is nearest `vel`.
pub fn layer_index(vel: u8) -> usize {
    let mut best = 0;
    let mut best_d = u16::MAX;
    for (i, &lv) in VEL_LAYERS.iter().enumerate() {
        let d = (lv as i16 - vel as i16).unsigned_abs();
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    best
}

/// The representative velocity of the layer a played `vel` maps to.
pub fn layer_vel(vel: u8) -> u8 {
    VEL_LAYERS[layer_index(vel)]
}

/// The two layers a played velocity sits BETWEEN, and how far it is between
/// them: `(lower, upper, mix)` with `mix` 0 at the lower layer and 1 at the upper.
///
/// Snapping each note to its nearest layer makes the timbre a staircase: two
/// notes a few velocity units apart, but on either side of a boundary, are
/// rendered from different strikes and jump in brightness. In a passage whose
/// velocities hover around a boundary — which is most of a nocturne's left hand —
/// consecutive notes then alternate between two timbres, and that alternation is
/// heard as a click on the repeated note. Measured 2026-08-21: going from four
/// layers to eight made it WORSE (5 high-band artefacts to 27 over a minute),
/// which is the signature of boundary crossings rather than of layer coarseness,
/// since eight layers are individually CLOSER to the played velocity.
///
/// Both samples are the same note struck from rest, so they are phase-aligned and
/// a linear crossfade between them interpolates the strike rather than smearing
/// it.
pub fn layer_pair(vel: u8) -> (u8, u8, f32) {
    let v = vel as f32;
    if v <= VEL_LAYERS[0] as f32 {
        return (VEL_LAYERS[0], VEL_LAYERS[0], 0.0);
    }
    let last = VEL_LAYERS[VEL_LAYERS.len() - 1];
    if v >= last as f32 {
        return (last, last, 0.0);
    }
    for w in VEL_LAYERS.windows(2) {
        let (a, b) = (w[0], w[1]);
        if v >= a as f32 && v <= b as f32 {
            let mix = (v - a as f32) / ((b - a) as f32).max(1.0);
            return (a, b, mix);
        }
    }
    (last, last, 0.0)
}

/// One note rendered by the model, stereo, as the board radiates it.
pub struct NoteSample {
    pub note: u8,
    pub layer_vel: u8,
    pub left: Vec<f32>,
    pub right: Vec<f32>,
}

impl NoteSample {
    pub fn frames(&self) -> usize {
        self.left.len()
    }
}

/// How far to let a rendered note run.
#[derive(Clone, Copy)]
pub struct RenderBudget {
    /// Never render longer than this, whatever the note (a hard memory cap).
    pub max_secs: f32,
    /// Never render shorter than this, so an onset is always complete.
    pub min_secs: f32,
    /// Stop once a window has fallen this far below the note's own peak.
    pub tail_db: f32,
    /// When set, use this as the length cap for EVERY note instead of the
    /// note-dependent one. The offline bounce sets it per note to how long the
    /// piece actually holds that note, so a briefly-struck note is not rendered
    /// out to its full free decay.
    pub cap_secs: Option<f32>,
}

impl Default for RenderBudget {
    fn default() -> Self {
        RenderBudget {
            max_secs: 14.0,
            min_secs: 0.5,
            tail_db: -72.0,
            cap_secs: None,
        }
    }
}

impl RenderBudget {
    /// A note-dependent cap: the bass rings far longer than the treble, so there
    /// is no reason to hold fourteen seconds of a top-octave note in memory. An
    /// explicit `cap_secs` overrides it (still bounded by the hard `max_secs`).
    pub fn max_secs_for(&self, note: u8) -> f32 {
        if let Some(c) = self.cap_secs {
            return c.clamp(self.min_secs, self.max_secs);
        }
        // ~14 s at A0, tapering to ~2.5 s at the top of the compass.
        let t = ((note.max(21).min(108) - 21) as f32) / (108.0 - 21.0);
        let secs = 14.0 * (1.0 - t) + 2.5 * t;
        secs.min(self.max_secs)
    }

    /// A copy with an explicit length cap.
    pub fn with_cap(self, secs: f32) -> Self {
        RenderBudget { cap_secs: Some(secs), ..self }
    }
}

/// Render one isolated note through the physical model to a stereo buffer,
/// stopping once its tail has decayed (or the budget's cap is reached).
///
/// This is the expensive step the hybrid pays once per `(note, layer)`. The note
/// is rendered under the CURRENT `patch` so the bank always matches the piano,
/// but at unity gain: the patch's own gain is applied live to the player, so it
/// stays automatable and does not bake into the sample.
pub fn render_note_stereo(
    sr: f32,
    note: u8,
    vel: u8,
    budget: RenderBudget,
    patch: &CordisPatch,
) -> NoteSample {
    let (mut eng, tx, _mr) = CordisEngine::new_for_plugin(sr);
    let mut p = patch.clone();
    p.gain = 1.0;
    // ── The mechanism must NOT be baked in ────────────────────────────────
    //
    // The action's noise and the body's thump are STOCHASTIC per event: each key
    // let-off has its own few-ms delay and its own jitter, which is why eight keys
    // struck together come out 2.8 times one key rather than eight times over.
    // Frozen into a sample that same realisation replays identically at every
    // strike, so notes struck close together add their thumps COHERENTLY — a low
    // frequency bump at the 100/250/1000 Hz resonances that is heard as a click on
    // the second and third notes of an arpeggio. Measured: leaving it out of the
    // sample drops the hybrid's onset transient excess over the plain model by 29%.
    // The engine fires the mechanism live on every sampled note-on instead, so it
    // keeps its randomisation and costs nothing extra.
    p.mechanics = 0.0;
    let _ = tx.send(CordisCommand::LoadPatch(Box::new(p)));
    let _ = tx.send(CordisCommand::NoteOn(note, vel));

    let max_frames = (sr * budget.max_secs_for(note)) as usize;
    let min_frames = (sr * budget.min_secs) as usize;
    // A ~50 ms window to judge the tail against the running peak.
    let win = ((sr * 0.05) as usize).max(1);
    let tail_ratio = 10f32.powf(budget.tail_db / 20.0);

    let mut left = Vec::with_capacity(max_frames.min(1 << 20));
    let mut right = Vec::with_capacity(max_frames.min(1 << 20));
    let mut buf = vec![0.0f32; 256 * 2];

    let mut peak = 0.0f32; // the note's own peak, to measure the tail against
    let mut win_peak = 0.0f32; // peak within the current window
    let mut win_count = 0usize;

    'render: while left.len() < max_frames {
        buf.iter_mut().for_each(|x| *x = 0.0);
        eng.process_audio(&mut buf, 2);
        for fr in buf.chunks(2) {
            let (l, r) = (fr[0], fr[1]);
            left.push(l);
            right.push(r);
            let a = l.abs().max(r.abs());
            peak = peak.max(a);
            win_peak = win_peak.max(a);
            win_count += 1;
            if win_count >= win {
                // Decide at each window boundary, once past the onset.
                if left.len() >= min_frames && peak > 0.0 && win_peak <= peak * tail_ratio {
                    break 'render;
                }
                win_peak = 0.0;
                win_count = 0;
            }
        }
    }

    // A short fade at the end so a sample that was length-capped (or hit the
    // hard cap) runs out smoothly instead of stepping to silence: a click. On a
    // naturally decayed tail it lands below -72 dB and is inaudible.
    let len = left.len();
    let fade = ((sr * 0.02) as usize).min(len);
    for k in 0..fade {
        let idx = len - fade + k;
        let g = 1.0 - (k as f32 + 1.0) / fade as f32;
        left[idx] *= g;
        right[idx] *= g;
    }

    NoteSample {
        note,
        layer_vel: vel,
        left,
        right,
    }
}

/// The lazy bank: rendered notes keyed by `(note, layer velocity)`.
///
/// Phase 1 is the storage and the synchronous render. The background thread that
/// fills it while the live model covers the first strike is a later phase; the
/// cache is shaped for it now (shared, cheap to clone out an `Arc`).
#[derive(Default)]
pub struct SampleBank {
    samples: HashMap<(u8, u8), Arc<NoteSample>>,
}

impl SampleBank {
    pub fn new() -> Self {
        SampleBank {
            samples: HashMap::new(),
        }
    }

    /// The rendered sample for the layer this velocity maps to, if present.
    pub fn get(&self, note: u8, vel: u8) -> Option<Arc<NoteSample>> {
        self.samples.get(&(note, layer_vel(vel))).cloned()
    }

    /// Whether the layer this velocity maps to is already rendered.
    pub fn has(&self, note: u8, vel: u8) -> bool {
        self.samples.contains_key(&(note, layer_vel(vel)))
    }

    /// Insert an already-rendered sample under its own layer velocity.
    pub fn insert(&mut self, sample: Arc<NoteSample>) {
        self.samples.insert((sample.note, sample.layer_vel), sample);
    }

    /// Render the layer for `(note, vel)` synchronously and cache it. Returns the
    /// cached sample (existing or freshly rendered). This is what the background
    /// job will call off the audio thread; on the audio thread the live model is
    /// used instead and this is never touched.
    pub fn get_or_render(
        &mut self,
        sr: f32,
        note: u8,
        vel: u8,
        budget: RenderBudget,
        patch: &CordisPatch,
    ) -> Arc<NoteSample> {
        let lv = layer_vel(vel);
        if let Some(s) = self.samples.get(&(note, lv)) {
            return s.clone();
        }
        let s = Arc::new(render_note_stereo(sr, note, lv, budget, patch));
        self.samples.insert((note, lv), s.clone());
        s
    }

    /// Drop every cached sample. Called when the model or the patch changes, so
    /// the bank never serves a note rendered by a piano that no longer exists.
    pub fn clear(&mut self) {
        self.samples.clear();
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

/// How many sample voices can sound at once. Samples are cheap (a buffer read
/// each), so this is generous: a pedalled passage can pile up far more ringing
/// notes than the physical model could afford, and here they cost almost nothing.
pub const HYBRID_PLAYERS: usize = 256;

/// A cached note being played back: a read cursor into a `NoteSample`, a velocity
/// amplitude, and a damper-release envelope for when the key lifts with the pedal
/// up. The sample already carries the strike, the string, the board colour and
/// the action noise, all at final output level, so playback OUTPUTS IT DIRECTLY
/// and never touches the soundboard (driving the board would double its colour).
pub struct SamplePlayer {
    sample: Option<Arc<NoteSample>>,
    pos: usize,
    pub note: u8,
    gain: f32,
    pub active: bool,
    pub held: bool,
    /// Release multiplier and its per-sample decay (1.0 = not releasing).
    rel_mul: f32,
    rel_coeff: f32,
    /// Per-sample LINEAR decrement of `rel_mul`, used to cross-fade a player out
    /// on a restrike instead of the exponential damper: an exponential drops
    /// fastest at the start, which ducks a still-ringing note the instant the new
    /// strike lands (heard as a click on a repeated note). 0 = not cross-fading.
    rel_lin: f32,
    /// The damper, split three ways. A piano's damper is a felt pad, and the
    /// measurement of what the model does when one lands is not what a flat gain
    /// does at all: 100 ms after the key is released the model has lost 17.7 dB
    /// between 180 and 600 Hz, 22.1 dB between 600 and 1800, and only 8.0 dB
    /// between 1.8 and 5 kHz — the MIDDLE goes first and the top rings on, partly
    /// because the plate and the action noise are not what the damper is touching.
    /// A sample faded by one gain loses all three together, so every note-off in
    /// the hybrid differs from the model by five to eight decibels somewhere in
    /// the spectrum, and that difference arrives as a step: measured, the note-off
    /// is where the hybrid's audible clicks come from (slowing this damper down
    /// drops the click count from 30 to 11 over thirty seconds of the nocturne).
    /// Three bands with their own measured decay rates cost two one-pole filters
    /// per channel and reconstruct EXACTLY when the gains are equal, so a note
    /// that is not being damped passes through untouched.
    d_lo: [f32; 2],
    d_mid: [f32; 2],
    g_lo: f32,
    g_mid: f32,
    g_hi: f32,
    k_lo: f32,
    k_mid: f32,
    k_hi: f32,
    /// Splitter coefficients, from the sample rate.
    c_lo: f32,
    c_mid: f32,
    damping: bool,
    split_primed: bool,
    /// Set when the player stopped because it reached the END of its sample
    /// while it was still meant to be sounding. Diagnostic: the model would have
    /// kept ringing, so this is a note vanishing.
    pub exhausted: bool,
    /// Attack gain ramp (0..1) and its per-sample rise. A fresh sample carries a
    /// full strike transient; on a RESTRIKE the real string is already moving, so
    /// the hammer adds a much softer onset. Playing the full fresh transient over
    /// a ringing note is the click on repeated notes, so a restrike ramps the new
    /// sample in over a few ms. 1.0 / 0.0 = no ramp (a first strike).
    atk_mul: f32,
    atk_step: f32,
}

impl Default for SamplePlayer {
    fn default() -> Self {
        SamplePlayer {
            sample: None,
            pos: 0,
            note: 0,
            gain: 1.0,
            active: false,
            held: false,
            rel_mul: 1.0,
            rel_coeff: 1.0,
            rel_lin: 0.0,
            d_lo: [0.0; 2],
            d_mid: [0.0; 2],
            g_lo: 1.0,
            g_mid: 1.0,
            g_hi: 1.0,
            k_lo: 1.0,
            k_mid: 1.0,
            k_hi: 1.0,
            c_lo: 0.0,
            c_mid: 0.0,
            damping: false,
            split_primed: false,
            exhausted: false,
            atk_mul: 1.0,
            atk_step: 0.0,
        }
    }
}

/// Where the three damper bands are split, Hz.
const DAMP_SPLIT_LO: f32 = 600.0;
const DAMP_SPLIT_HI: f32 = 1800.0;
/// The decay each band takes when the damper lands, in seconds, fitted to the
/// model measured note-off band by band (see `SamplePlayer::d_lo`).
const DAMP_TAU_LO: f32 = 0.050;
const DAMP_TAU_MID: f32 = 0.045;
const DAMP_TAU_HI: f32 = 0.095;

impl SamplePlayer {
    /// The damper fall. A piano damper quenches a note fast: this reaches
    /// inaudibility (-60 dB) in ~0.5 s, the order a real damper takes.
    fn damper_coeff_ms(sr: f32, ms: f32) -> f32 {
        (-1.0 / (sr * ms.max(1.0) * 1.0e-3)).exp()
    }
    fn damper_coeff(sr: f32) -> f32 {
        Self::damper_coeff_ms(sr, 80.0)
    }

    pub fn start(&mut self, sample: Arc<NoteSample>, note: u8, gain: f32, attack_ms: f32, sr: f32) {
        self.sample = Some(sample);
        self.pos = 0;
        self.note = note;
        self.gain = gain;
        self.active = true;
        self.held = true;
        self.rel_mul = 1.0;
        self.rel_coeff = 1.0;
        // Reset the cross-fade too: a slot reused from a player that was mid
        // cross-fade would otherwise inherit its linear fade-out and the new note
        // would decay to nothing on the spot (heard as a pizzicato).
        self.rel_lin = 0.0;
        self.damping = false;
        self.split_primed = false;
        self.exhausted = false;
        self.g_lo = 1.0;
        self.g_mid = 1.0;
        self.g_hi = 1.0;
        // A restrike softens the fresh transient by ramping the sample in.
        if attack_ms > 0.0 {
            let n = (attack_ms * 1.0e-3 * sr).max(1.0);
            self.atk_mul = 0.0;
            self.atk_step = 1.0 / n;
        } else {
            self.atk_mul = 1.0;
            self.atk_step = 0.0;
        }
    }

    /// The key was released. With the pedal down the string keeps ringing (the
    /// sample plays its full tail); with it up the damper falls.
    pub fn release(&mut self, sustain: bool, sr: f32) {
        self.release_ms(sustain, sr, 80.0)
    }

    /// Release with an explicit damper time constant. `ms` scales the measured
    /// per-band rates (80 ms is the calibration point, so 80 keeps them exactly);
    /// it exists so the damper can be swept while chasing a difference.
    pub fn release_ms(&mut self, sustain: bool, sr: f32, ms: f32) {
        self.held = false;
        if !sustain {
            // MEASURED 2026-08-21: shaping this damper into three bands to match
            // the model's felt (which takes the middle first and leaves the top
            // ringing) made the high band WORSE, not better — twice the artefact
            // transients above 2 kHz. The flat fall is kept until there is a
            // measurement that says otherwise.
            self.rel_coeff = Self::damper_coeff_ms(sr, ms);
        }
    }

    /// Land the damper: three bands, each with its own measured decay.
    fn arm_damper(&mut self, sr: f32, scale: f32) {
        let sc = scale.max(0.01);
        let k = |tau: f32| (-1.0 / (sr * tau * sc).max(1.0)).exp();
        self.k_lo = k(DAMP_TAU_LO);
        self.k_mid = k(DAMP_TAU_MID);
        self.k_hi = k(DAMP_TAU_HI);
        self.g_lo = 1.0;
        self.g_mid = 1.0;
        self.g_hi = 1.0;
        self.damping = true;
        // The splitters start from the signal that is already flowing, not from
        // zero: a filter charging from rest is itself a step.
        self.c_lo = 1.0 - (-2.0 * std::f32::consts::PI * DAMP_SPLIT_LO / sr).exp();
        self.c_mid = 1.0 - (-2.0 * std::f32::consts::PI * DAMP_SPLIT_HI / sr).exp();
        self.d_lo = [0.0; 2];
        self.d_mid = [0.0; 2];
        self.split_primed = false;
    }

    /// The pedal came up: a note no finger is holding gets its damper back.
    pub fn pedal_up(&mut self, still_held: bool, sr: f32) {
        if !still_held && !self.held {
            self.rel_coeff = Self::damper_coeff(sr);
        }
    }

    /// Sounding with its damper still off the string: the key is down, or the
    /// pedal is. The hybrid's answer to `Voice::undamped`, and what an editor
    /// lights a key from.
    pub fn undamped(&self) -> bool {
        self.active && self.rel_coeff >= 1.0
    }

    /// A struck note of the same pitch takes over this string.
    pub fn is_note(&self, note: u8) -> bool {
        self.active && self.note == note
    }

    /// How far into its sample this player has run, 0..1. An inactive player is
    /// fully spent, so it is stolen first. Falling release counts as more spent.
    pub fn progress(&self) -> f32 {
        if !self.active {
            return f32::INFINITY;
        }
        let frac = match &self.sample {
            Some(s) if !s.left.is_empty() => self.pos as f32 / s.left.len() as f32,
            _ => 1.0,
        };
        // A releasing player is nearer the end than its cursor alone says.
        frac + (1.0 - self.rel_mul)
    }

    /// One stereo frame, advancing the cursor and the release envelope.
    pub fn tick(&mut self) -> (f32, f32) {
        if !self.active {
            return (0.0, 0.0);
        }
        let s = match &self.sample {
            Some(s) => s,
            None => {
                self.active = false;
                return (0.0, 0.0);
            }
        };
        if self.pos >= s.left.len() {
            self.active = false;
            // Held (or pedalled) notes that outlast their rendered sample simply
            // stop, where the model would still be ringing.
            self.exhausted = self.held || self.rel_mul > 0.5;
            return (0.0, 0.0);
        }
        let g = self.gain * self.rel_mul * self.atk_mul;
        let (mut ol, mut orr) = (s.left[self.pos] * g, s.right[self.pos] * g);
        if self.damping {
            // Prime the splitters on the first damped sample: a filter charging
            // from silence is a step, which is the very thing being fixed.
            if !self.split_primed {
                self.d_lo = [ol, orr];
                self.d_mid = [0.0, 0.0];
                self.split_primed = true;
            }
            let (a_lo, a_mid) = (self.c_lo, self.c_mid);
            let (g_lo, g_mid, g_hi) = (self.g_lo, self.g_mid, self.g_hi);
            for (ch, x) in [&mut ol, &mut orr].into_iter().enumerate() {
                self.d_lo[ch] += (*x - self.d_lo[ch]) * a_lo;
                let lo = self.d_lo[ch];
                let rest = *x - lo;
                self.d_mid[ch] += (rest - self.d_mid[ch]) * a_mid;
                let mid = self.d_mid[ch];
                let hi = rest - mid;
                *x = lo * g_lo + mid * g_mid + hi * g_hi;
            }
            self.g_lo *= self.k_lo;
            self.g_mid *= self.k_mid;
            self.g_hi *= self.k_hi;
            if self.g_lo.max(self.g_mid).max(self.g_hi) < 1.0e-3 {
                self.active = false;
            }
        }
        let out = (ol, orr);
        self.pos += 1;
        if self.atk_step > 0.0 && self.atk_mul < 1.0 {
            self.atk_mul = (self.atk_mul + self.atk_step).min(1.0);
        }
        if self.rel_lin > 0.0 {
            self.rel_mul -= self.rel_lin;
            if self.rel_mul <= 0.0 {
                self.active = false;
            }
        } else if self.rel_coeff < 1.0 {
            self.rel_mul *= self.rel_coeff;
            if self.rel_mul < 1.0e-3 {
                self.active = false;
            }
        }
        out
    }

    /// The velocity layer this player is sounding, so a restrike can reuse it and
    /// avoid two same-note samples at different tunings beating against each other.
    pub fn layer_vel(&self) -> Option<u8> {
        self.sample.as_ref().map(|s| s.layer_vel)
    }

    /// Cross-fade this player out linearly over `ms`, from wherever its level is
    /// now. Used on a restrike so the old tail gives way smoothly under the new
    /// strike instead of ducking.
    pub fn crossfade_out(&mut self, ms: f32, sr: f32) {
        let n = (ms * 1.0e-3 * sr).max(1.0);
        self.rel_lin = (self.rel_mul / n).max(1.0e-6);
    }
}

/// The sympathetic halo, shared.
///
/// With the sustain pedal down every string is free, and a struck note's
/// partials that land on another string's modes set it ringing. In the physical
/// engine that is one live Voice per undamped string (thirty to a hundred of
/// them), which is the cost the hybrid removes. Here it is ONE shared modal bank:
/// a resonator at every note's fundamental, driven by the struck output, so a
/// note excites the resonators its partials fall on and they ring on together.
///
/// It is a halo, not the tone: the engine measures the pedalled sympathetic
/// addition at only 0 to 1.2 dB, so this is summed in low and gated on the pedal.
/// When the pedal lifts, every damper lands, so the output fades over a damper's
/// fall and the bank is then cleared.
pub struct SympatheticBank {
    bank: ModalBank,
    shape: Vec<f64>,
    coupling: f64,
    gain: f32,
    env: f32,
    env_fall: f32,
    live: bool,
}

impl SympatheticBank {
    /// The lowest and highest MIDI notes with strings.
    const LO: u8 = 21;
    const HI: u8 = 108;

    pub fn new(sr: f32) -> Self {
        let mut modes = Vec::with_capacity((Self::HI - Self::LO + 1) as usize);
        for note in Self::LO..=Self::HI {
            let f = 440.0 * 2f64.powf((note as f64 - 69.0) / 12.0);
            // Undamped strings ring long: ~15 s in the bass down to ~3 s at the
            // top, the order of the free decays the model itself produces.
            let t = (note - Self::LO) as f64 / (Self::HI - Self::LO) as f64;
            let t60 = 15.0 * (1.0 - t) + 3.0 * t;
            modes.push(Mode::from_t60(f, t60));
        }
        let n = modes.len();
        let mut bank = ModalBank::new();
        bank.set_modes(&modes, sr);
        // A full damper shape so `damp(0.0)` can hard-clear the bank when the
        // pedal lifts and the halo has faded.
        bank.set_damper_shape(&vec![1.0; n]);
        SympatheticBank {
            bank,
            shape: vec![1.0; n],
            coupling: 1.0,
            // ModalBank works in tiny physical units (the modal coordinates are
            // displacements, the same the soundboard's output is scaled up from),
            // so the readout needs a large factor to reach audio level. Calibrated
            // against the live model: with this the pedalled tail regains the ~2.8
            // dB of sympathetic energy the physical engine puts there (a held
            // C-major chord, Close Mics), matching its measured 0 to 1.2 dB add.
            // MEASURED 2026-08-23, against the per-string model it replaces.
            //
            // The obvious target is the wrong one and the measurement says so:
            // matched on TOTAL energy the bank would be silent, because eighty-
            // seven free strings LOAD the bridge and the physical model comes
            // out 2 dB QUIETER with the pedal down than without.
            //
            // What a listener calls the pedal is the energy that appears where
            // the struck note has none. Measured at a witness note a tritone
            // above the one struck, the physical strings put +8.7, +8.0 and
            // +36.3 dB there for three pairs across the compass — an average
            // the bank can match and a SPREAD it cannot, since one shared bank
            // answers every note alike where the plate's geometry answers each
            // differently. At this gain the mean signed error over the three is
            // +0.3 dB: the same sympathetic energy, spread evenly instead of
            // unevenly. See `print_the_pedalled_add_to_calibrate_the_halo`.
            gain: 200.0,
            env: 0.0,
            env_fall: 1.0 / (sr * 0.15),
            live: false,
        }
    }

    /// One sample of halo to add to the output. `drive` is the struck signal
    /// (mono), `pedal` whether the sustain pedal is down.
    pub fn process(&mut self, drive: f32, pedal: bool) -> f32 {
        if pedal {
            self.env = 1.0;
            self.live = true;
            self.bank.add_force(&self.shape, drive as f64 * self.coupling);
            self.bank.tick();
            return self.bank.read(&self.shape) as f32 * self.gain;
        }
        if !self.live {
            return 0.0;
        }
        // Pedal up: the dampers land. Fade the halo over a damper's fall, then
        // clear the bank so nothing stale survives to the next pedal.
        self.env -= self.env_fall;
        if self.env <= 0.0 {
            self.env = 0.0;
            self.bank.damp(0.0);
            self.live = false;
            return 0.0;
        }
        self.bank.tick();
        self.bank.read(&self.shape) as f32 * self.gain * self.env
    }

    /// Whether the halo still has anything to contribute (so the engine keeps
    /// running the block).
    pub fn is_active(&self) -> bool {
        self.live
    }

    /// Give each resonator the coupling its string actually has to the plate.
    ///
    /// The bank shipped with a flat shape — every note driven and read equally
    /// — and measured against the physical model that is 32 dB out between one
    /// note and another: the strings a struck note really shakes are the ones
    /// the bridge carries to, and the plate's geometry decides which. Handing
    /// the same weights the board uses puts the halo where the instrument puts
    /// it. `w[i]` is the coupling strength of note `LO + i`, in any consistent
    /// unit; the set is normalised here, so only the SHAPE matters and the
    /// overall level stays with `gain`.
    pub fn set_shape(&mut self, w: &[f64]) {
        let n = self.shape.len().min(w.len());
        let mean: f64 = w[..n].iter().sum::<f64>() / n.max(1) as f64;
        if mean <= 0.0 {
            return;
        }
        for i in 0..n {
            self.shape[i] = w[i] / mean;
        }
    }

    /// Stop dead, without the damper's fall.
    ///
    /// For an all-notes-off, which is a panic and not a pedal lift: the
    /// hundred and fifty milliseconds this otherwise takes to fade are exactly
    /// what someone hitting panic does not want to hear.
    pub fn reset(&mut self) {
        self.bank.damp(0.0);
        self.env = 0.0;
        self.live = false;
    }

    /// The halo depends on nothing but the note grid, but the gain is the one
    /// tunable, exposed for calibration against the model.
    pub fn set_gain(&mut self, g: f32) {
        self.gain = g;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    #[test]
    fn layer_mapping_picks_nearest() {
        assert_eq!(layer_vel(1), 24);
        assert_eq!(layer_vel(40), 24); // 40 is 16 from 24, 16 from 56 -> ties to first
        assert_eq!(layer_vel(56), 56);
        assert_eq!(layer_vel(70), 56);
        assert_eq!(layer_vel(100), 88);
        assert_eq!(layer_vel(127), 120);
    }

    #[test]
    fn rendered_note_has_an_onset_and_a_tail() {
        let s = render_note_stereo(SR, 60, 88, RenderBudget::default(), &CordisPatch::default());
        assert!(s.frames() > (SR * 0.2) as usize, "too short: {}", s.frames());

        let peak = s
            .left
            .iter()
            .zip(&s.right)
            .fold(0.0f32, |m, (l, r)| m.max(l.abs()).max(r.abs()));
        assert!(peak > 1e-4, "silent render, peak {peak}");

        // The strike is near the start: the peak lands in the first 150 ms.
        let onset = (SR * 0.15) as usize;
        let early = s.left[..onset.min(s.left.len())]
            .iter()
            .fold(0.0f32, |m, &x| m.max(x.abs()));
        assert!(early >= peak * 0.5, "no strike transient: early {early} vs peak {peak}");
    }

    #[test]
    fn bass_renders_longer_than_treble() {
        let b = RenderBudget::default();
        assert!(b.max_secs_for(28) > b.max_secs_for(100));
    }

    #[test]
    fn player_plays_then_stops_and_damps() {
        let mut bank = SampleBank::new();
        let s = bank.get_or_render(SR, 60, 88, RenderBudget::default(), &CordisPatch::default());
        let mut p = SamplePlayer::default();
        assert!(!p.active);
        p.start(s.clone(), 60, 1.0, 0.0, 48_000.0);
        assert!(p.active && p.is_note(60));

        // It produces sound.
        let mut e = 0.0f32;
        for _ in 0..(SR as usize / 10) {
            let (l, r) = p.tick();
            e += l * l + r * r;
        }
        assert!(e > 1e-6, "player silent");

        // A damper release quenches it fast (well under a second).
        p.release(false, SR);
        let mut n = 0;
        while p.active && n < SR as usize {
            p.tick();
            n += 1;
        }
        assert!(!p.active, "damper never quenched the player");
        assert!(n < (SR * 0.6) as usize, "damper too slow: {n} samples");
    }

    #[test]
    fn engine_hybrid_plays_cached_note() {
        let (mut eng, tx, _mr) = CordisEngine::new_for_plugin(SR);
        eng.set_hybrid(true);
        eng.prewarm(60, 88);
        assert_eq!(eng.bank_len(), 1);

        let _ = tx.send(CordisCommand::NoteOn(60, 90));
        let mut buf = vec![0.0f32; 512 * 2];
        let mut peak = 0.0f32;
        for _ in 0..30 {
            buf.iter_mut().for_each(|x| *x = 0.0);
            eng.process_audio(&mut buf, 2);
            for &x in &buf {
                peak = peak.max(x.abs());
            }
        }
        assert!(peak > 1e-4, "hybrid engine silent on a cached note, peak {peak}");

        // Releasing then letting the damper fall brings it to silence.
        let _ = tx.send(CordisCommand::NoteOff(60));
        let mut tail = 0.0f32;
        for _ in 0..60 {
            buf.iter_mut().for_each(|x| *x = 0.0);
            eng.process_audio(&mut buf, 2);
        }
        for _ in 0..30 {
            buf.iter_mut().for_each(|x| *x = 0.0);
            eng.process_audio(&mut buf, 2);
            for &x in &buf {
                tail = tail.max(x.abs());
            }
        }
        assert!(tail < peak, "note did not decay after release: tail {tail} peak {peak}");
    }

    #[test]
    fn background_filler_populates_bank_on_miss() {
        let (mut eng, tx, _mr) = CordisEngine::new_for_plugin(SR);
        eng.set_hybrid(true);
        assert_eq!(eng.bank_len(), 0);

        // First strike: a miss. The live model covers it and a background render
        // is requested.
        let _ = tx.send(CordisCommand::NoteOn(60, 90));
        let mut buf = vec![0.0f32; 256 * 2];
        let mut filled = false;
        // Sixty seconds, not twelve.
        //
        // The question is WHETHER the filler populates the bank, never how fast.
        // But the wait was a wall-clock deadline, and a wall clock measures the
        // machine as much as the code: one note costs the background renderer
        // about 2.8 s on an idle machine, and run under a full suite with a
        // compile alongside it went past twelve. This failed in the suite and
        // passed on its own. A deadline generous enough to be about the code
        // costs nothing whenever the answer is yes, which is every green run.
        for _ in 0..6000 {
            buf.iter_mut().for_each(|x| *x = 0.0);
            eng.process_audio(&mut buf, 2); // drains finished renders into the bank
            if eng.bank_len() > 0 {
                filled = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(filled, "background filler never populated the bank");

        // The repeat now finds the sample and plays it.
        let _ = tx.send(CordisCommand::NoteOn(60, 90));
        let mut peak = 0.0f32;
        for _ in 0..8 {
            buf.iter_mut().for_each(|x| *x = 0.0);
            eng.process_audio(&mut buf, 2);
            for &x in &buf {
                peak = peak.max(x.abs());
            }
        }
        assert!(peak > 0.0, "cached repeat produced nothing");
    }

    #[test]
    fn patch_change_invalidates_bank_but_gain_does_not() {
        let (mut eng, tx, _mr) = CordisEngine::new_for_plugin(SR);
        eng.set_hybrid(true);
        eng.prewarm(60, 88);
        assert_eq!(eng.bank_len(), 1);

        // A timbre change (tune) clears the bank on the next drain.
        let _ = tx.send(CordisCommand::SetTune(50.0));
        let mut buf = vec![0.0f32; 64 * 2];
        eng.process_audio(&mut buf, 2);
        assert_eq!(eng.bank_len(), 0, "tune change did not invalidate the bank");

        // A gain change does not: gain is applied live to the players.
        eng.prewarm(60, 88);
        assert_eq!(eng.bank_len(), 1);
        let _ = tx.send(CordisCommand::SetGain(0.5));
        eng.process_audio(&mut buf, 2);
        assert_eq!(eng.bank_len(), 1, "gain change wrongly invalidated the bank");
    }

    #[test]
    fn sympathetic_halo_rings_under_pedal() {
        let mut sb = SympatheticBank::new(SR);
        let mut ring = 0.0f32;
        for i in 0..(SR as usize) {
            let t = i as f32 / SR;
            // A short 440 Hz burst, then silence.
            let drive = if t < 0.05 {
                (core::f32::consts::TAU * 440.0 * t).sin()
            } else {
                0.0
            };
            let h = sb.process(drive, true);
            if t > 0.1 {
                ring += h * h; // energy AFTER the burst is the resonators ringing on
            }
        }
        assert!(ring > 1e-9, "no sympathetic ring under pedal: {ring}");
    }

    #[test]
    fn sympathetic_halo_silent_with_pedal_up() {
        let mut sb = SympatheticBank::new(SR);
        let mut e = 0.0f32;
        for i in 0..(SR as usize / 2) {
            let t = i as f32 / SR;
            let drive = (core::f32::consts::TAU * 440.0 * t).sin();
            e += sb.process(drive, false).abs(); // pedal up: never driven
        }
        assert_eq!(e, 0.0, "halo should be silent with the pedal up, got {e}");
    }

    #[test]
    fn halo_fades_and_clears_on_pedal_up() {
        let mut sb = SympatheticBank::new(SR);
        for i in 0..(SR as usize / 10) {
            let t = i as f32 / SR;
            sb.process((core::f32::consts::TAU * 440.0 * t).sin(), true);
        }
        assert!(sb.is_active());
        let mut n = 0;
        while sb.is_active() && n < SR as usize {
            sb.process(0.0, false);
            n += 1;
        }
        assert!(!sb.is_active(), "halo never cleared after pedal up");
        assert!(n < (SR * 0.3) as usize, "halo cleared too slowly: {n} samples");
    }

    #[test]
    fn player_runs_out_at_sample_end() {
        let s = Arc::new(NoteSample {
            note: 60,
            layer_vel: 88,
            left: vec![0.5; 100],
            right: vec![0.5; 100],
        });
        let mut p = SamplePlayer::default();
        p.start(s, 60, 1.0, 0.0, 48_000.0);
        for _ in 0..100 {
            p.tick();
        }
        let (l, r) = p.tick();
        assert!(!p.active && l == 0.0 && r == 0.0);
    }

    #[test]
    fn bank_caches_by_layer() {
        let mut bank = SampleBank::new();
        assert!(!bank.has(60, 90));
        let pat = CordisPatch::default();
        let a = bank.get_or_render(SR, 60, 90, RenderBudget::default(), &pat);
        assert!(bank.has(60, 88)); // 90 mapped to layer 88
        assert!(bank.has(60, 90)); // and 90 still resolves to it
        let b = bank.get_or_render(SR, 60, 85, RenderBudget::default(), &pat);
        // 85 and 90 share layer 88: same Arc, no re-render.
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(bank.len(), 1);
    }
}


// ── Disk persistence of the bank ─────────────────────────────────────────────
//
// The user's specification for the hybrid, verbatim: "tout doit être prêt
// avant même de commencer à jouer, et de façon optimale". The optimal part is
// this file format: a bank rendered once per (patch timbre, sample rate) is
// never rendered again — session opens become file reads.
//
// One file per (note, layer) under a directory named by the render signature,
// so a preparation interrupted half-way resumes where it stopped, and a
// timbre change simply addresses a different directory without destroying the
// old one (switching back is free). Only the FILLER/PREPARATION threads touch
// the disk; the audio thread never does.
//
// Format, little-endian, hand-rolled (no serde in this crate's contract):
//   magic "CRDB" | u32 version | f32 sr | u8 note | u8 layer_vel |
//   u32 frames | frames × f32 left | frames × f32 right
//
// The version is bumped whenever the RENDERER changes audibly (it is the
// engine's sound, frozen), so stale banks self-invalidate.

/// Bump this when the physical model's sound changes: every persisted bank
/// with an older version is ignored and re-rendered.
pub const BANK_FORMAT_VERSION: u32 = 1;

/// The directory a given timbre's bank lives in, under `root`.
///
/// The signature folds the five timbre parameters and the sample rate; two
/// pianos with the same preset share the directory (and tracks with the same
/// preset share the rendering work across sessions).
pub fn bank_dir(root: &std::path::Path, sig: [f32; 5], sr: f32) -> std::path::PathBuf {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut fold = |v: u32| {
        h = (h ^ v as u64).wrapping_mul(0x1_0000_0001_b3);
    };
    for v in sig {
        fold(v.to_bits());
    }
    fold(sr.to_bits());
    fold(BANK_FORMAT_VERSION);
    root.join(format!("cordis-bank-{h:016x}"))
}

fn sample_path(dir: &std::path::Path, note: u8, layer_vel: u8) -> std::path::PathBuf {
    dir.join(format!("n{note:03}_v{layer_vel:03}.crdb"))
}

/// Write one rendered sample. Written to a temp name then renamed, so a crash
/// mid-write can never leave a half sample that later loads as a truncated
/// note.
pub fn save_sample(
    dir: &std::path::Path,
    sr: f32,
    s: &NoteSample,
) -> std::io::Result<()> {
    use std::io::Write;
    std::fs::create_dir_all(dir)?;
    let path = sample_path(dir, s.note, s.layer_vel);
    let tmp = path.with_extension("tmp");
    {
        let mut w = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
        w.write_all(b"CRDB")?;
        w.write_all(&BANK_FORMAT_VERSION.to_le_bytes())?;
        w.write_all(&sr.to_le_bytes())?;
        w.write_all(&[s.note, s.layer_vel])?;
        w.write_all(&(s.left.len() as u32).to_le_bytes())?;
        for v in &s.left {
            w.write_all(&v.to_le_bytes())?;
        }
        for v in &s.right {
            w.write_all(&v.to_le_bytes())?;
        }
        w.flush()?;
    }
    std::fs::rename(&tmp, &path)
}

/// Load one sample if it is present and sound; `None` means "render it".
///
/// Any inconsistency — wrong magic, version, rate, note, or a short file —
/// reads as absence rather than as an error: the render path is always a
/// correct fallback and a corrupt file must never become a corrupt note.
pub fn load_sample(
    dir: &std::path::Path,
    sr: f32,
    note: u8,
    layer_vel: u8,
) -> Option<NoteSample> {
    let bytes = std::fs::read(sample_path(dir, note, layer_vel)).ok()?;
    if bytes.len() < 18 || &bytes[0..4] != b"CRDB" {
        return None;
    }
    let ver = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    let fsr = f32::from_le_bytes(bytes[8..12].try_into().ok()?);
    if ver != BANK_FORMAT_VERSION || (fsr - sr).abs() > 0.5 {
        return None;
    }
    if bytes[12] != note || bytes[13] != layer_vel {
        return None;
    }
    let frames = u32::from_le_bytes(bytes[14..18].try_into().ok()?) as usize;
    let need = 18 + frames * 8;
    if bytes.len() < need {
        return None;
    }
    let read_f32s = |off: usize| -> Vec<f32> {
        bytes[off..off + frames * 4]
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    };
    Some(NoteSample {
        note,
        layer_vel,
        left: read_f32s(18),
        right: read_f32s(18 + frames * 4),
    })
}

#[cfg(test)]
mod prepare_tests {
    use super::*;
    use crate::engine::{CordisCommand, CordisEngine};

    /// "Tout doit être prêt avant même de commencer à jouer" — enabling the
    /// hybrid asks for the WHOLE keyboard, and a persisted bank makes that a
    /// file load, not a render. Pre-populate the disk with every (note,
    /// layer), enable, and the bank must fill to 88 x layers without a single
    /// note being rendered (the fake samples are 100 frames; a render is
    /// never that short — its minimum is half a second).
    #[test]
    fn enabling_hybrid_prepares_the_whole_keyboard_from_disk() {
        let root = std::env::temp_dir().join(format!(
            "cordis_prepare_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let sr = 48_000.0f32;
        let (mut eng, tx, mr) = CordisEngine::new_for_plugin(sr);
        eng.set_bank_cache_root(root.clone());
        // The engine's default patch decides the bank directory.
        let sig = timbre_sig(&eng.patch);
        let dir = bank_dir(&root, sig, sr);
        for note in 21u8..=108 {
            for &lv in &VEL_LAYERS {
                let s = NoteSample {
                    note,
                    layer_vel: lv,
                    left: vec![0.01; 100],
                    right: vec![0.01; 100],
                };
                save_sample(&dir, sr, &s).expect("prepopulate");
            }
        }

        eng.set_hybrid(true);
        let want = 88 * VEL_LAYERS.len();
        let mut buf = vec![0.0f32; 256 * 2];
        let mut ready = false;
        for _ in 0..1200 {
            buf.iter_mut().for_each(|x| *x = 0.0);
            eng.process_audio(&mut buf, 2); // drains finished loads
            if eng.bank_len() == want {
                ready = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            ready,
            "the whole-keyboard preparation never completed: {} of {want}",
            eng.bank_len()
        );
        // And the meter says so, so a host can show readiness. (Republishes
        // at 30 Hz; run past one period to force a tick first.)
        for _ in 0..(sr as usize / 30 / 256 + 2) {
            buf.iter_mut().for_each(|x| *x = 0.0);
            eng.process_audio(&mut buf, 2);
        }
        let fill = {
            let mut g = mr.try_lock().expect("the meter reader is free in a test");
            g.read().cloned().unwrap_or_default().bank_fill
        };
        assert_eq!(fill, (want as u16, want as u16));

        std::fs::remove_dir_all(&root).ok();
    }
}

#[cfg(test)]
mod prepare_cost_tests {
    use super::*;
    use crate::engine::CordisEngine;

    /// Prepare the banks of every preset Ressac plays (plus the grand the
    /// other demos use), sequentially — each preset's 352 renders fan across
    /// the cores internally. Run once; afterwards every session open of these
    /// timbres is a file load.
    ///
    ///   cargo test -p cordis --release --lib prepare_the_ressac_banks -- --ignored --nocapture
    #[test]
    #[ignore = "renders seven full keyboards; ~half an hour once"]
    fn prepare_the_ressac_banks() {
        let root = std::env::var_os("XDG_CACHE_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|h| std::path::PathBuf::from(h).join(".cache"))
            })
            .expect("a home")
            .join("phonix-audio")
            .join("cordis-banks");
        let sr = 48_000.0f32;
        let want = 88 * VEL_LAYERS.len();
        for name in [
            "Tight Dampers",
            "Close Mics",
            "Player's Seat",
            "Long Dampers",
            "Concert Grand, Mellow",
            "Wide Unison",
            "Concert Grand, Bright",
        ] {
            let preset = crate::patch::factory_presets_tagged()
                .into_iter()
                .find(|p| p.name.contains(name))
                .unwrap_or_else(|| panic!("preset {name} not found"));
            let (mut eng, tx, _mr) = CordisEngine::new_for_plugin(sr);
            eng.set_bank_cache_root(root.clone());
            let _ = tx.send(crate::engine::CordisCommand::LoadPatch(Box::new(preset)));
            let mut buf = vec![0.0f32; 256 * 2];
            // Drain the patch load BEFORE enabling, so the preparation renders
            // the right timbre.
            eng.process_audio(&mut buf, 2);
            let t0 = std::time::Instant::now();
            eng.set_hybrid(true);
            loop {
                buf.iter_mut().for_each(|x| *x = 0.0);
                eng.process_audio(&mut buf, 2);
                if eng.bank_len() == want {
                    break;
                }
                assert!(t0.elapsed().as_secs() < 3600, "{name}: over an hour");
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            eprintln!("  {name:24} : {want} entrees en {:6.1} s", t0.elapsed().as_secs_f64());
        }
        eprintln!("  banques: {}", root.display());
    }

    /// The REAL first-ever preparation: the whole keyboard rendered through
    /// the model, in parallel, persisted. Prints the wall time — the number
    /// the "minutes, pas des dizaines de minutes" gate is judged on — and
    /// leaves the bank on disk, so running this once also warms the user's
    /// cache for the default patch.
    ///
    ///   cargo test -p cordis --release --lib prepare_the_default_bank -- --ignored --nocapture
    #[test]
    #[ignore = "renders the full keyboard; minutes of CPU"]
    fn prepare_the_default_bank_and_time_it() {
        let root = std::env::var_os("XDG_CACHE_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|h| std::path::PathBuf::from(h).join(".cache"))
            })
            .expect("a home")
            .join("phonix-audio")
            .join("cordis-banks");
        let sr = 48_000.0f32;
        let (mut eng, _tx, _mr) = CordisEngine::new_for_plugin(sr);
        eng.set_bank_cache_root(root.clone());
        let want = 88 * VEL_LAYERS.len();
        let t0 = std::time::Instant::now();
        eng.set_hybrid(true);
        let mut buf = vec![0.0f32; 256 * 2];
        loop {
            buf.iter_mut().for_each(|x| *x = 0.0);
            eng.process_audio(&mut buf, 2);
            if eng.bank_len() == want {
                break;
            }
            assert!(
                t0.elapsed().as_secs() < 3600,
                "preparation ran over an hour: {} of {want}",
                eng.bank_len()
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        eprintln!(
            "  preparation complete : {want} entrees en {:.1} s (banque: {})",
            t0.elapsed().as_secs_f64(),
            root.display()
        );
    }
}

#[cfg(test)]
mod persist_tests {
    use super::*;

    #[test]
    fn a_sample_survives_the_disk_round_trip() {
        let dir = std::env::temp_dir().join(format!(
            "cordis_persist_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let s = NoteSample {
            note: 60,
            layer_vel: 88,
            left: (0..1000).map(|i| (i as f32 * 0.001).sin()).collect(),
            right: (0..1000).map(|i| (i as f32 * 0.002).cos()).collect(),
        };
        save_sample(&dir, 48_000.0, &s).expect("save");
        let back = load_sample(&dir, 48_000.0, 60, 88).expect("load");
        assert_eq!(back.left, s.left);
        assert_eq!(back.right, s.right);
        // The wrong rate reads as absence, never as a wrong-pitch note.
        assert!(load_sample(&dir, 44_100.0, 60, 88).is_none());
        // A truncated file reads as absence.
        let p = dir.join("n060_v088.crdb");
        let bytes = std::fs::read(&p).unwrap();
        std::fs::write(&p, &bytes[..bytes.len() / 2]).unwrap();
        assert!(load_sample(&dir, 48_000.0, 60, 88).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn two_timbres_get_two_directories_and_the_same_one_returns() {
        let root = std::path::Path::new("/tmp");
        let a = bank_dir(root, [0.5, 1.0, 0.7, 0.5, 0.0], 48_000.0);
        let b = bank_dir(root, [0.6, 1.0, 0.7, 0.5, 0.0], 48_000.0);
        let a2 = bank_dir(root, [0.5, 1.0, 0.7, 0.5, 0.0], 48_000.0);
        assert_ne!(a, b, "different timbres must not share a bank");
        assert_eq!(a, a2, "the same timbre must come back to its bank");
        let c = bank_dir(root, [0.5, 1.0, 0.7, 0.5, 0.0], 44_100.0);
        assert_ne!(a, c, "a different rate is a different bank");
    }
}
