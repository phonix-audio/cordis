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

The contact is integrated with the string advanced. From C5 up, the transverse
banks tick at the hammer's sub-step spacing while the felt is on them, and the
felt is solved at each tick against the string's real position, so the wave
returning from the agraffe, which is what lifts the hammer off, is resolved
where its round trip is shorter than a sample. The once-a-sample scheme stays
below C5, where the two agree within a decibel and a bass string carries four
hundred modes. Measured 2026-09-02 at 4 m/s, the bridge force from C6 to the
top sits 8 dB under A4 on average with a 5 dB spread, where it sat 20 dB under
with an 18 dB spread. C7 is strung at 9.8 cm, which halves its inharmonicity
(B 0.0054, the octave partial 19 cents sharp) and lands its contact at 1.23
periods against the 1.26 Chaigne and Askenfelt measured at 2.5 m/s.

Two things stay open:

- **The partials above the fundamental are too strong.** Through the output at
  about 5 m/s, the second, third and fourth partials of C6, D#6 and C7 sit 8
  to 19, 18 to 26 and up to 30 dB above the Iowa Steinway's (measured
  2026-09-02, 20.9 dB rms over the nine figures). No lever on the felt reaches
  it: the contact patch's weighting, the felt's stiffness and its exponent,
  Stulov's memory and Hunt-Crossley's factor were each swept through the same
  measure, and none lowers the third partial without lengthening the contact
  past the published band and taking the top octave's level with it. A taper
  on the coupling for the bridge's own mass (`BRIDGE_MASS_HZ`) brings the error
  to 12 dB at a corner of 1.8 kHz and costs the top note 11 dB; it is built and
  off. Whatever makes a real treble pulse that smooth is not a Hertz felt on a
  string.
- **A chord's first block costs.** Nine treble notes struck inside one
  64-frame block cost it 2.2 ms against a budget of 1.3, ten bass notes 2.0;
  the contact's sub-step solve is the cost, and it runs only on the samples
  the felt is on. It was 4.9 ms before the exact free trajectory, a
  transcendental per mode per sub-step, stopped being evaluated for a scheme
  that never read it.

Measured and closed on 2026-09-02: the fundamental barely excited at the top
(the contact above), the treble's decay (below), the excess momentum (below).

## Level and image

The output is not two samples of the plate. A note's direct sound is its own
bridge force above 250 Hz, heard by a near-coincident pair of cardioids from
where the note is pinned on the bridge; the two listening points carry the
board's modes below 250 Hz in full and the rest at a reduced share, the same on
both sides and as a side signal at a quarter of the width. Above 250 Hz the
bridge's admittance is flat, so the board radiates in proportion to the force
on it, and a note's level no longer depends on the signs a listening point
happens to draw. Measured 2026-09-02 across the compass at velocity 90: the
level varies by 2.8 dB rms and the worst semitone step is 4 dB, where it was
16 dB (24.8 dB at the plate alone); a chord counts each note once. The image
follows the pitch, 4 dB left at the bottom to 6 dB right at the top.

**A single note is nearly mono.** The pair is coincident within 5 cm, so a held
note's two channels correlate at 0.9 through the middle of the keyboard, where
the two-point output gave 0.2. Width between notes comes from their positions;
width within a note would have to come from the board's difference signal, and
raising that share brings the wandering image back (measured 2026-09-02: at a
share of one the mid-keyboard image alternates 4 dB left and 6 dB right from
one note to the next). The side share is held at a quarter.

**The treble's decay was double-counted, and is not now.** The termination
loss written into the treble string modes was applied to the vertical motion
the bridge already drains dynamically, so a C7 fell 1.6 times faster than the
law it cites. The vertical modes now carry only what the termination takes
beyond the plate's mean admittance; the published law acts on the unison's
antisymmetric motion, which pushes no net force on the bridge and was never
drained, through a cross-string damper, and in full on the horizontal bank.
Pedalled tails measured 2026-09-02: A6 21 dB/s and D7 18 against anchors of 18
and 21; D#6 loses 25 dB in its first second against the Iowa recording's 28.

**A flat +3.27 dB is given back after the plate's damping floor.** The per-note
make-up the floor actually needs runs 4.65 / 3.61 / 2.96 / 4.86 / 2.07 / 3.88 /
1.84 / 0.03 dB from E0 to E6, so a flat constant leaves the very top 3.3 dB
loud in relative terms. That happens to push against the treble deficit above
rather than with it; it is a level correction, not a treatment of that fault,
and it should not be read as one.

## Energy the model does not conserve

**The hammer no longer leaves with more than it brought.** With the string
banks and the hammer sharing one sub-step spacing, the momentum ratio at the
top note is 1.93 at 5 m/s and 2.00 at 0.5 m/s, under the elastic limit at
every dynamic (measured 2026-09-02). The 2.19 recorded before came from the
banks being spaced at twenty sub-steps while the hammer's inertia was taken at
forty to eighty: the felt met a hammer several times its mass, and that is also
why the contact's length looked insensitive to the felt's stiffness.

Related, and bounded rather than fixed: the bridge stiffness the explicit scheme
uses is **not** the string's physical stiffness. The modal sum is 54 times the
derived value at A4 and 242 times in the bass. Substituting the physical `T/L`
was tried twice on 2026-08-07 and diverged both times (the tuning went 197 cents
out at A1; the pedal test returned `inf`). The term is holding the explicit
scheme together, not describing a string. The real fix is Chabassier's
continuity formulation with a Schur complement, which is a different scheme and
not a different constant. What can be said about the size of the error: the
product `S*C` measures 0.004 to 0.023 across the compass, and the cross-term the
formulation would add is 0.3% of the bridge force with one voice, 0.8% with
three and 1.4% with ten (2026-08-13).

## Modes the model does not carry

The string banks are truncated at Nyquist and the plate at 16 kHz. Both are
deliberate; both cost something specific.

**Both remainders of the truncation are added.** The
transmission sum converges like `1/k`: 420 partials reach the bridge in the
bass and four at the top, where the partial sum is 0.0495 against a true 0.12.
The exact remainder for the bridge (`residual_bridge`) is added at the force of
the sample, so the treble's bridge force no longer stops at two fifths. The
remainder of the string's give under the felt (`StringModes::residual_compliance`)
joins the sub-stepped contact as a massless spring in series, its deflection
carried from one sub-step to the next: the discarded modes start at 21.6 kHz
and answer within a few sub-steps of a contact that lasts hundreds. With it C7's
contact lands on the 1.26 periods Chaigne and Askenfelt measured; the top
string is 40 percent softer under the felt than the bank alone made it, and the
top note sits 14 dB under the compass mean, which is what a 3.5 g hammer on a
5 cm string gives.

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
  envelope and the peak force falls sensibly (52 -> 42 N at C4), but the force's
  second partial barely moves at C4 and goes the *wrong way* at E♭6 (-11.1 ->
  -6.0 dB). It is not the answer to the treble. Before this was measured the
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
  -33.8 -> -15.7 dB, note 105 -32.5 -> -12.3 dB), off because it broke three
  stability tests at once.
- **A per-voice implicit bridge solve.** Right for one voice and wrong for an
  instrument: it diverged at sixteen voices where the explicit scheme reaches a
  hundred and twenty-eight. The diagnosis it produced stands; the cure does not.
  A correct bridge solve is a genuine NxN system, not a scalar.

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
- **The editor is a fixed 1280x800 drawing and does not resize.** That is a
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
