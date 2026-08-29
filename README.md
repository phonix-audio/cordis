# Cordis

A grand piano built from the physics, not from samples.

The hammer's force is computed against the string rather than assumed. The
strings of a unison are coupled through a real bridge. The soundboard is *in the
loop*, loading the strings, rather than a reverb hung on the output. The decay of
every note, the double decay, the beating of a unison, sympathetic resonance, the
sustain pedal and the stereo image are all consequences of that chain rather than
settings inside it.

Sources: Humbert (IRCAM/ATIAM 2002) for the modal formulation and the real-time
contact scheme; Chabassier (2012) for the string, the felt and the measured
stringing of a Steinway D; Ege (2009, 2013) for the soundboard's measured modes
and damping; Chaigne & Askenfelt (JASA 1994) for the hammer felt.

## What does not work

`REFERENCES.md` lists the published work this implements, paper by paper,
with the module that follows each one.

`LIMITATIONS.md`. It is long, and it is the honest half of this README: the
treble deficit and its cause, the energy the hammer hands back at fortissimo,
the modes the banks do not carry, what was built and measured and left switched
off, which constants were set by ear because nothing is published, and what the
instrument gives up when the machine runs out of time. Read it before deciding
whether this model is fit for what you want.

## Layout

    crates/cordis            the engine. serde, and libc on Linux.
    crates/cordis-ui         the egui editor
    crates/cordis-plugin     VST3 / CLAP, via nice-plug
    crates/cordis-research   measurement tools

The editor is a separate crate rather than a feature of the engine, and that is
load-bearing. Cargo unifies features across a resolved graph, so an optional
`egui` inside `cordis` would be switched on for the engine's own tests by any
`cargo test --workspace`. A crate boundary is the only thing that makes "the
engine never sees egui" true rather than merely intended:

    cargo tree -p cordis --edges normal   # serde; plus libc on Linux

The engine's dependency list is deliberately that short. `libc` is there for one
thing and one thing only; putting the worker threads at the audio callback's
real-time priority, which is a Linux syscall; and nothing in the DSP touches
it; on every other target the engine is serde alone. There is no `[features]`
table in `crates/cordis/Cargo.toml`, and the absence is the contract.

## Building

    cargo test                      # 152 tests, plus 136 measurement probes
    scripts/build_plugins.sh        # VST3 + CLAP bundle for this platform
    scripts/build_plugins.sh --windows

The bundle lands in `target/bundled/<platform>/`, laid out as the VST3 spec
wants. Any host that scans a directory will find it there; for a development
tree, point the host's search path at it rather than installing.

The measurement probes are `#[ignore]`d and do not run there; the 152 that do
take the better part of an hour in a debug build, and far less with `--release`.
`LIMITATIONS.md` has the details, including one test that is load-sensitive.

Two things the build assumes about this machine, both in `.cargo/config.toml`
and both easy to change. It compiles for `x86-64-v3`, so it needs an x86-64 CPU
from roughly 2013 onward and will want that line dropped or replaced on another
architecture. And it raises `RUST_MIN_STACK` to 16 MiB, because a debug
`cargo test` gives each test a 2 MiB thread and the engine's un-optimised call
tree sums past it; building from outside this config will hit that as a stack
overflow rather than as a test failure. The bundling script wants `bash` and
`python3`, and the Windows cross-build additionally wants `cargo-xwin` and
`clang-cl`. Only Linux is built and tested here; macOS is not.

## Measurement tools

    cargo run --release -p cordis-research --bin cordis_bench
    cargo run --release -p cordis-research --bin board_tf

`cordis_bench` reports the soundboard's mode count, where the coupling energy
sits, and the realtime factor at several polyphonies. `board_tf` prints the
plate's radiated magnitude response driven at a bridge point.

They need `hound` and `rustfft` as real dependencies, which is why they are a
crate of their own: a `[[bin]]` cannot see dev-dependencies, and putting a WAV
writer in the engine's graph would break the dependency contract above.

## Hearing it

There is no audio in this repository, and that is on purpose: a render is made
to be judged and thrown away, not kept in the history, and the scores that were
used to judge it are other people's music. So the way to hear the instrument is
to build the bundle above and play it, or to render your own material from the
tests.

The listening renders the model was developed against are still here as
`#[ignore]`d tests. They write 48 kHz WAVs under `renders/`, which is ignored:

    cargo test -p cordis --release --lib -- --ignored --nocapture \
        render_the_chord_attack

`render_the_sympathy_ab` and `render_the_pp_mechanics_ab` are the same shape.
Each renders one passage with one thing changed, which is what they are for:
they answer a question rather than showing the instrument off.

## Compatibility

`COMPAT.md` lists what cannot change: the VST3 class id, the parameter ids, the
patch's serde field names and their defaults, and the factory bank's names *and
order*. All of them are written into files other programs read, and all of them
are covered by a test.

## History

The engine began as a research project on piano physics
rather than a plugin. That host now loads it as an ordinary scanned VST3, like
any third-party plugin, which is what the split was for. The DAW is not public
and nothing here depends on it; comments in the source that name it are recording
where a decision came from, not pointing at code you can read.

## Licence

MIT or Apache-2.0, at your option; `LICENSE-MIT` and `LICENSE-APACHE` are both
here. The editor bundles Noto Serif Display under the SIL Open Font License,
whose text sits beside it in `crates/cordis-ui/assets/OFL.txt`.

One thing to know before redistributing a **built** plugin. The VST3 wrapper
comes from nice-plug, which reaches VST3 through `vst3-sys`, and `vst3-sys` is
GPLv3; nice-plug's own manifest says its `vst3` feature "exists mostly for
GPL-compliance reasons". So the source in this repository is MIT/Apache-2.0, but
a compiled `.vst3` (and the `.clap` built alongside it, which is the same shared
object) is a combined work with GPLv3 code and carries GPL-3.0 obligations.
`scripts/build_plugins.sh` builds both from one cdylib with nice-plug's default
features, which include `vst3`. A CLAP-only build with that feature turned off
would link `clap-sys`, which is MIT/Apache-2.0, and nothing GPL; that is the
switch to reach for if the GPL terms are not wanted.
