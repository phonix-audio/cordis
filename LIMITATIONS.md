# What does not work

A physical model is a claim, and a claim can be checked. Most of what follows
was found by checking, and most of it is written into the source at the place
that causes it. This file gathers it in one place so a reader does not have to
find out by playing.

Nothing here is a roadmap. Some of it has a known fix that nobody has had the
time for; some of it is genuinely open; some of it is a compromise that is
staying. Each item says which.

Dates in this file are the dates the measurement was taken, and they are the
dates in the source.

---

## The treble

**This is the instrument's largest fault and everything else in this section is
downstream of it.** The top of the compass is 15 to 16 dB under the rest of the
keyboard (notes 87-108), its second partial sits at −10 to +17 dB where the Iowa
Steinway sits at −30, and on some notes the fundamental is barely excited at
all. It is audible: it reads as thin, bell-like or plucked rather than as a
piano. Measured against a Gaussian pulse of this model's own contact duration
and impulse, the whole 25 to 30 dB is in the force pulse's *shape*, and no force
law reaches it; swept up and down with the stiffness bisected to hold the
contact duration, the felt's exponent moves that partial by five decibels at
best.

The cause is known, and it is not the string, the felt or the board. The
hammer-string loop closes once per audio sample. A real hammer is lifted off by
the wave that returns from the agraffe, and at A7 that round trip is
`2·x_H/c = 34 µs`; one and a half samples. So the model makes one clean
half-sine where Chaigne & Askenfelt's Fig. 2 shows a train of pulses returning
to zero between contacts, and a smooth single hump has deep spectral zeros. On
2026-08-12 the blow was measured to offer the note's own fundamental −0.5 dB in
the bass and **−46.1 dB at note 93**, with −22.0 to −35.7 either side of it. The
pulse falls at 25 dB/octave between 4 and 8 kHz where a half-sine falls at 12,
which is the 49 dB hole a middle C has there.

The reflection is not missing from the physics. A modal description *is* the
standing waves, so every reflection is in the bank already; what is missing is
its resolution in time, because the bank advances once per audio sample while
the felt is integrated twenty to eighty times inside it. There is exactly one
real fix and it is a change of structure: give the string bank a second set of
recursion coefficients at the sub-step rate and tick it inside the contact loop.
It would cost nothing outside contact, which is a thousandth of the engine's
work.

Four cheaper things were tried instead and all four diverged or regressed: a
delay line for the returning wave (NaN on every note whose round trip fits the
buffer; a rigid termination reflects *inverted*, so added rather than
subtracted it is positive feedback), the residual compliance, a `t²` give
profile, and removing the compliance. A fifth, advancing the string sub-sample
through the contact, was retried on 2026-08-24 and parked again: it fixes the
diagnosis (note 99's second partial goes +23.4 → −5.0 dB at 192 kHz) but drops
the treble 9 dB where the middle loses 1, widens the second-partial spread from
39 to 52 dB, fails the published contact-duration guards (note 96 at 1.60 ms
against ≤ 1.2 ms; A4 at 1.05 string periods against Chaigne 2016's 0.88), and
doubles the error against the Iowa reference (13.5 → 26.4 dB rms) because the
sub-loop bypasses the contact-patch weighting.

Two more treble faults are separate but sit in the same register:

- **Inharmonicity above C6 is too high and cannot be fixed from the scale.**
  C7 at 8.8 cm gives B = 0.0106, which stretches its octave partial 36 cents ;
  harpsichord territory, and reported as such by ear. Lengthening the string
  halves it and breaks the contact-duration test at note 105; keeping the
  contact needs a 3.7 cm top string, which is not a string. Measured every way
  round on 2026-08-21. It has to come from the felt and the hammer mass at the
  top of the compass, not from the stringing.
- **The treble decays too slowly.** A real C7 loses 21 dB per second; this model
  loses 15.7. The string's own damping accounts for 4.3 dB of either, so the
  missing 5 dB is the bridge, whose losses in the treble are about half what
  they should be. That is its own piece of work. The margin the decay test
  asserts is what the model honestly has rather than what a piano has, and the
  net beside it says plainly that it cannot assert the treble outruns the bass,
  because it does not.

## Level and image

**Note-to-note level is uneven, and the two-point output is why.** Driven at its
own bridge point at its own fundamental, with no hammer and no string in the
path, the plate alone swings **24.8 dB from one semitone to the next**. Three
cures were built and measured and none was keepable: compressing the magnitudes
(24.8 → 30.6 dB; the nulls are made by the *signs*, not the magnitudes),
averaging the ear over eight points (→ 21.2 dB, the notches merely move), and
sharing the bridge's shape between the ears (worst jump 17.9 → 10.7 dB, at a
channel correlation of 0.966 against a real piano's 0.2; i.e. mono). Evenness
and stereo width are in direct conflict for a two-point output. The honest cure
is to read the far field as the modes' volume velocity rather than as two
samples of the plate, which is a different output model.

**The stereo image wanders.** Measured note by note it sits 4.7 dB left in the
bass, 6.1 right around D♯3, 3.8 left at D♯4, 5.3 left at D♯5 and 3.2 right at
the top. A scale drifts from side to side at random, and no piano does that.
Giving the ears a position on the bridge was written and measured on 2026-08-11
and reverted: above a few hundred hertz the positional cosine turns over many
times within a band and averages to nothing. The image cannot come from modal
phase alone.

**A flat +3.27 dB is given back after the plate's damping floor.** The per-note
make-up the floor actually needs runs 4.65 / 3.61 / 2.96 / 4.86 / 2.07 / 3.88 /
1.84 / 0.03 dB from E0 to E6, so a flat constant leaves the very top 3.3 dB
loud in relative terms. That happens to push against the treble deficit above
rather than with it; it is a level correction, not a treatment of that fault,
and it should not be read as one.

## Energy the model does not conserve

**At fortissimo the hammer leaves with more momentum than it arrived with.** The
top note delivers **2.19 times** the momentum it carries at 5 m/s. Two is the
perfect elastic rebound and the limit, so 1.99 at 0.5 m/s and 1.96 at 2 m/s are
right and 2.19 is not: the excess appears only at fortissimo and scales with the
felt's stiffness. Measured 2026-08-13, two cheap explanations were ruled out:
raising the contact's sub-steps from 20 to 80 leaves 2.19 unchanged to the
second decimal, and raising the Newton iterations from 8 to 40 does the same.
Both experiments were reverted rather than kept on the strength of a hypothesis.

The obvious explanation was that the string enters the contact solve as a
compliance `c_eff = h²/M + C/steps`, which is a lossless spring: it gives while
the felt presses and hands the work back at the end, where a real string would
have carried it away as travelling waves. That was tested the same day by
zeroing the compliance, and the answer is the opposite. Without it the ratio is
exactly 2.00 at 0.5 m/s and the scheme *runs away* at 2 and 5 m/s; ratios of
10⁹² and 10¹⁷⁰. The compliance is not the leak, it is what holds the scheme
together, and Bilbao's felt law is exactly conservative wherever the bare case
is stable at all.

So the 2.19 is not energy handed back by a spring. It is what is left of an
instability the compliance suppresses without quite cancelling, which is why it
appears at a threshold rather than drifting in with force. That is where the
question stands. In practical terms it is a ten percent excess of impulse; 0.8
dB; on the topmost note at fortissimo, which is not something anyone has
claimed to hear. What makes it worth writing down is that the guard which keeps
it honest asserts 2.2, so it sits immediately under its own limit rather than
comfortably inside it, and it is an instability rather than a bias.

Note that the source contradicts itself here. The comment beside `strike` still
calls the lossless-spring explanation "what has NOT been tested"; the test that
tested it is `a_hammer_gives_no_more_than_it_carries`, and it is the later of
the two.

Related, and bounded rather than fixed: the bridge stiffness the explicit scheme
uses is **not** the string's physical stiffness. The modal sum is 54 times the
derived value at A4 and 242 times in the bass. Substituting the physical `T/L`
was tried twice on 2026-08-07 and diverged both times (the tuning went 197 cents
out at A1; the pedal test returned `inf`). The term is holding the explicit
scheme together, not describing a string. The real fix is Chabassier's
continuity formulation with a Schur complement, which is a different scheme and
not a different constant. What can be said about the size of the error: the
product `S·C` measures 0.004 to 0.023 across the compass, and the cross-term the
formulation would add is 0.3% of the bridge force with one voice, 0.8% with
three and 1.4% with ten (2026-08-13).

## Modes the model does not carry

The string banks are truncated at Nyquist and the plate at 16 kHz. Both are
deliberate; both cost something specific.

**The treble delivers two fifths of the bridge force it should.** The
transmission sum converges like `1/k`, which is to say hardly at all: 420
partials reach it in the bass and **four** at the top of the compass, where the
partial sum is 0.0495 against a true 0.12. The missing three fifths are not
physics, they are arithmetic that was stopped early. The symptom is visible in
the bridge force, flat near 7 N from F1 to D♯5 and then 5.5, 2.3, 1.1, 1.0, 0.5
where first principles say it should rise.

The exact remainder is computable (`StringModes::residual_compliance`) and it
is **switched off**. Not for the reason one would guess: the discarded modes
start at 21.6 kHz, so over a millisecond contact they have responded many times
over, and the blow is not too short for them. Two things stand in the way. The
term is a *static* compliance where the one it must join is a *one-sample* one,
and at A5 it is about twice as large, so adding it whole roughly triples the
give the felt is pressed against. Ramping it in over the sub-steps, the obvious
repair, was checked on paper and **does not work, so it is not worth a cycle**:
`ω_c·h = 0.141`, so the ramp saturates by the twentieth sub-step and the term
would be applied whole over a contact that lasts twenty to a hundred and fifty
*samples*. That is the version that failed nine tests at once, among them the
one that exists for exactly this.

And underneath both: **the term and the felt stiffness are not independent.**
`K` above C7 is solved from the published contact duration, and every contact
duration this model matches was obtained with the residual absent; so `K` has
already absorbed the give the term describes, and switching it on double-counts
it. It cannot be switched on alone; it has to land together with a
re-derivation of the felt anchors against the same published durations, measured
as one change. The parameter is left plumbed through so the next attempt does
not have to re-thread it.

**The plate's modes are never retired.** The pruning is built and measured and
not wired in, because under a pedalled storm it retires *nothing*: 3618 of 3618
modes stay live from the first block to the last, since the plate is driven
broadband by every hammer and then held there by the ringing strings. Static
pruning at preset load would buy 17 to 25 percent and cost fidelity. So the
plate is the instrument's fixed floor: 3618 modes advanced every sample whatever
the music, 23% of a block's budget with nothing sounding at all and near 40% at
the clocks a loaded machine actually runs.

**The duplex/aliquot bank is inaudible and still computed.** Rendered with it
summed in and with it zeroed, the whole bank contributes between 62 and 132 dB
*under* the note across the treble. Three modal banks are built and advanced per
voice for it.

**Tension modulation discards every mode above `sr/4`.** Mode `k` contributes at
twice its own frequency, so above that the term aliases back as inharmonic
content spread from 4.8 kHz upward; heard as a metallic buzz through the bass
and middle. Discarding them is exact only because the stretch integral is
diagonal, but it does silence the top half of the modes for that term.

## Built, measured, and left off

These are wired into the code and inert. They are kept rather than deleted
because a reader who finds working code and a measurement is better served than
one who finds three dead fields.

- **Stulov's hereditary term** (`EPSILON = 0`). The felt's memory is implemented
  and shipping at zero. Swept from 0 to 0.9 the contact stays inside Chaigne's
  envelope and the peak force falls sensibly (52 → 42 N at C4), but the force's
  second partial barely moves at C4 and goes the *wrong way* at E♭6 (−11.1 →
  −6.0 dB). It is not the answer to the treble. Before this was measured the
  fields existed and were never read, which read as an implemented model and was
  not one.
- **The longitudinal pulse train**; the feature Chaigne names as missing from
  models like this one. Switched on, the compass sweep went from a peak of 0.44
  to 5.75: twenty-two decibels hot, clipping everywhere (2026-08-12). The source
  term is a quasi-static answer to a question that is not quasi-static.
- **The felt cap's own mass** (`FELT_SURFACE_HZ = 0`), built for the treble's
  pulse shape and not kept.
- **The exact intra-sample string trajectory**, enabled only above note 88. The
  12-to-24 dB gain it first appeared to produce **was a bug**; written correctly
  the result is modest and mixed, and below note 88 the felt anchors; fitted
  with the string held still; have already absorbed the error it corrects.
- **The curvature term**, the largest single measured gain of its day (note 99
  −33.8 → −15.7 dB, note 105 −32.5 → −12.3 dB), off because it broke three
  stability tests at once.
- **A per-voice implicit bridge solve.** Right for one voice and wrong for an
  instrument: it diverged at sixteen voices where the explicit scheme reaches a
  hundred and twenty-eight. The diagnosis it produced stands; the cure does not.
  A correct bridge solve is a genuine N×N system, not a scalar.

## What was chosen rather than measured

The README lists the sources the model is built from. This is the complement:
the places where no measurement existed and a number had to be picked. They are
marked as such in the source, at the constant.

- **The action's noise levels.** Timing and mechanism are published; the sound
  pressure is not. The levels of the key-bed thump, the escapement knock and the
  release noise are chosen. This is the one part of the model with no published
  figures behind it.
  There is an audible consequence, and it is open. A blind listening round
  cleared the action noise at forte on every pair, but its one chord-level
  "click" was a pianissimo chord: the strike sits +6.3 dB over the body at pp
  against +4.0 at ff, because the thump's level follows `velocity^0.25` and
  barely drops while the strings drop by twenty decibels. The A/B renders that
  would confirm the mechanics are the cause are in `chord_attack.rs`
  (`render_the_pp_mechanics_ab`).
- **The bass below C2.** Chaigne & Askenfelt's lowest anchor is C2, so an octave
  and a half is filled with the correct physical trend and confirmed by ear, not
  by measurement. Above C7 there is no published hammer at all, so `K` is solved
  from the contact duration, which is the observable that *is* measured.
- **The plate's mode shapes.** Nobody publishes the shape of the four-hundredth
  mode of a soundboard at the point a bridge pin sits. What is published is
  their statistics, and that is what is drawn here; deterministically, so an
  instrument always sounds like itself, but they are draws and not measurements.
  The bridge geometry (two bridges, the rim standoff, the near-rim ramp) is
  likewise invented, and one tenor mode at 161 Hz is invented outright to fill a
  gap between two of Ege's that otherwise leaves a 30 dB antiresonance in the
  bridge's mobility; the "electric piano" note.
- **`COUPLE_COMP`.** Decimating the board read starves the high partials of
  their energy path into the plate, so they hang; this term restores their decay
  and was calibrated by ear on held A2, A4 and C7. It is a non-physical damping
  term that exists to pay for a CPU optimisation, and it is zero when the
  optimisation is off.
- **The plate's low-frequency damping floor** is 80 Hz because 80 Hz is Ege's
  published figure and because 80 Hz is what was listened to and approved. Sixty
  would have been a fit. It buys a great deal; the fundamental's peak moves
  from 109 ms to 16 ms at E3 and 55 to 9 ms at E2; at a cost of 0.4 to 2.5 dB
  per note.
- **Several gains and thresholds are ear-set**, and say so: the duplex gain, the
  shared halo's gain, the retirement thresholds (80 dB under a note's own peak,
  chosen 2026-08-23), the register coupling coefficient. That last one is worth
  singling out: it is *kept under suspicion and not because it is understood*.
  It was fitted against a real grand's decay while the bridge was five to fifty
  times too stiff, so it is compensating for a fault that has since been fixed;
  removing it changed the decay curve almost not at all and pushed the top note
  out of range, so it is back until the real cause is found.

## Hybrid Preview is a sampler

The instrument plays the exact model by default. `Hybrid Preview` is a separate
mode, off by default, and it is not the physics: it renders each note once, at
four velocity layers, and replays it.

What it cannot do is reproduce a hammer meeting a string that is already moving,
which is why repeated notes click. The clicks are patched with a cross-fade and
an attack ramp, which is cosmetics rather than physics. Beyond that: velocity is
a staircase of four layers (eight was measured *worse*, 27 high-band artefacts
over a minute against 5, 2026-08-21); a note held or pedalled past the end of
its rendered sample simply stops, which is a note vanishing; a cached note
bypasses the soundboard entirely; the released note falls at a flat rate where
the model loses 17.7 / 22.1 / 8.0 dB in three bands, so the hybrid's release
differs from the model's by five to eight decibels somewhere in the spectrum
(shaping it into three bands to match was tried on 2026-08-21 and made the high
band twice as bad). Preparing a bank costs about 2.8 seconds per note on an idle
machine, once per parameter set.

## Sympathetic resonance is one shared bank, not 87 strings

With the pedal down a real piano wakes every other set of strings. Simulating
that costs a hundred and seventy voices where the music has six, so what ships
is a bank of resonators shaped by each note's attachment along the bridge.

What the measurement can say is that the bank delivers the same *average*
sympathetic energy; mean signed error +0.3 dB at a witness note across the
compass. What it cannot say is whether the evenness matters, and the honest
answer is that it is wrong in principle: the physical strings put +8.7, +8.0 and
**+36.3 dB** at three witness notes across the compass, an average the bank can
match and a spread it cannot, because one shared bank answers every note alike
where the plate's geometry answers each differently.

There is an open question attached to it. Pedalled forte chords have been
reported as sounding like an organ, and notes above 73 as synthetic, in a blind
listening round. Eighty-eight resonators driven by the output sum do ring like
pipes when that sum is a forte chord. Whether that is the cause is not settled;
the four-way render that would settle it is in `chord_attack.rs`.

The per-string model still exists, and is test-only. Even it is a budget rather
than a theory: what stops the engine simulating all 88 is cost, and if the
instrument is full, the sympathy is simply not there.

## Live is not the same render as offline

Two things are gated on the host reporting real-time processing, and both are
off in a bounce. This is deliberate; a dropout is worse than a note ending
early, and a bounce must render every voice it was given; but it does mean the
file is not bit-for-bit what was heard while playing.

**The governor sheds strings under load.** Not a fade and not a cut: the damper
is dropped on the string at the quickest rate the felt is given, over about
12 ms, which is a sound a piano makes; the note ends. The order of sacrifice is
sympathetic strings, then notes already releasing, then the quietest held note;
nothing struck in the last twentieth of a second is taken, and a key that is
physically down is never shed. So what a player loses first under load is the
pedal's halo and the faintest tails. The budget starts at 24 voices rather than
at the ceiling, because starting at the ceiling means one guaranteed dropout on
the first chord of every session. It cuts only after four blocks in a row over
75% of budget, and recovers one voice every eighth quiet block: down fast, up
slowly, because an xrun is heard and a note not restored for another second is
not.

**The board runs decoupled live.** Strings push the plate but never read it
back within the block; the read-back's drain is baked statically into the string
modes instead. That is what makes the block-major path legal, and it is an
approximation of the per-sample coupling the offline path uses.

Two more scheduling notes. The worker pool's real-time priority is Linux-only
and best-effort: on any other target it compiles to nothing, and on Linux
without `RLIMIT_RTPRIO` the request is simply refused and the pool runs at
ordinary priority. Measured in a host before the pool was scheduled properly, a
passage costing 20 ms of work spiked to 57 ms with xruns. And several instances
in one session oversubscribe the machine; six pianos meant up to fifty
real-time threads on twelve cores; so each instance's worker share is divided
by how many are sounding.

## Host and platform

- **Stereo out, no input, no MIDI out.** If a host gives fewer than two channels
  the output is downmixed.
- **Three MIDI messages are handled**: note on, note off, and CC 64. There is no
  MPE and no per-note expression; pitch bend, channel and poly pressure,
  sostenuto (CC 66) and una corda (CC 67) are ignored.
- The `MidiCCs` MIDI config that makes CC 64 arrive at all also declares 2080
  controller parameters. They are flagged hidden; a host that shows them anyway
  is not reading the flags.
- **CLAP has no piano category.** `plugin-features.h` defines none, so browsing
  a CLAP host by feature will not surface this; the word is carried in the
  description instead.
- **The editor is a fixed 1280×800 drawing and does not resize.** That is a
  choice (a column layout that reflows is the wrong tool for a picture) but it
  is a limit a user meets immediately.
- **The Windows preset directory keeps a deliberate wart**, documented in
  `COMPAT.md`: it is `%USERPROFILE%\Documents\VST3 Presets\...` because that is
  where MediaBay looks, and it was left untouched when the Linux and macOS
  branches were fixed so that nothing already indexed is orphaned.
- **Only Linux is built and tested here.** `scripts/build_plugins.sh --windows`
  cross-compiles through `cargo-xwin` and is exercised; macOS is not, and the
  bundle layout for it is untried.

## Running the tests

`cargo test --workspace` runs the assertions. Most of the measurement work in
this repository is `#[ignore]`d: those tests print numbers rather than asserting
them, and their attributes say which kind they are; "sweep for calibration",
"measures a constant", "renders for listening", "needs a rendering backend".
Several of them take minutes and two take half an hour. They are run
deliberately, one at a time, with `-- --ignored --nocapture`.

Three caveats for a contributor:

- **`the_pool_renders_what_one_thread_renders` is load-sensitive.** It renders
  the same notes once serially and once across eight workers and demands the
  difference stay 160 dB under the peak, which is double-precision rounding and
  not a physical difference. It already disables the process-wide instance cap
  for exactly this reason: the cap divides the workers by how many pianos are
  sounding, which in a full test run is whatever the other tests happen to be
  doing, and it "passed alone, failed in the suite, every time the suite grew".
  The pool also re-weights each participant's share from the throughput it
  actually achieved, so a saturated machine can tile the work differently and
  sum the same terms in a different order. If this test fails, run it alone
  before believing it.
- **The editor's snapshot tests need a rendering backend** and are ignored by
  default. Every structural test in the UI crate passes on a page of empty
  boxes; the only check that a piano was drawn is the picture, which needs
  software Vulkan (lavapipe) to produce.
- **Debug builds need a 16 MiB thread stack.** `.cargo/config.toml` sets
  `RUST_MIN_STACK` for this: a debug `cargo test` spawns each test on a 2 MiB
  thread and the engine's un-optimised call tree sums past that. Release inlines
  it away. Building from outside this repository's cargo config will hit stack
  overflows in debug tests, and that is the reason.

`.cargo/config.toml` also sets `-C target-cpu=x86-64-v3`, which assumes an
x86-64 host from roughly 2013 onward. On another architecture, or on an older
CPU, drop or change that line.

Budget real time for the suite. `cargo test` builds at `opt-level = 2` rather
than 3, several engine tests render seconds of audio through the full model, and
the pool's barriers *spin*; so two pool tests scheduled side by side put sixteen
busy-waiting threads on the machine at once and both slow down. Measured on a
twelve-core machine, the engine crate's 128 assertions took 4332 seconds that
way, and the last hour of it was two of those tests overlapping. `--release` is
far quicker, and `-- --test-threads=1` removes the contention if you would
rather watch it go through in order.

## Two tests that were green for the wrong reason

Worth knowing, because they say something about how much of this file can be
trusted and how it was arrived at.

A decay test read green while being false: it measured from each
note's peak rather than from the strike, which hid that the treble was not
outrunning the bass. And a unison-beating test fitted a straight line through
what is a two-stage decay; it was the ruler, not the instrument. A companion
assertion, "a wider unison must beat more", had to be deleted rather than fixed,
because at strong bridge coupling it is measurably false (1 cent wobbles
0.43 dB, 4 cents 0.34).

Where a measurement in this file contradicts an older comment in the source, the
measurement is the one to trust, and its date is given.
