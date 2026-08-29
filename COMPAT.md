# What must not change

Everything below is a wire format. It is written into files other programs read,
so it is not a naming choice and not refactorable. Each item has a test; the
tests exist because a comment alone loses to a rename.

## Identifiers a host resolves the plugin by

    VST3 class id   PxCordisPiano001            (16 ASCII bytes, exactly)
    CLAP id         com.phonix-audio.cordis
    Plugin NAME     Cordis
    Plugin VENDOR   Phonix Audio

The class id goes into every exported DAWproject `Vst3Plugin` device and into the
header of every `.vstpreset`. NAME and VENDOR compose the preset directory
Cubase's MediaBay indexes, `<presets>/Phonix Audio/Cordis/`. Change any of them
and existing projects and preset banks point at a plugin no host can find.

They were SET, once, at first publication under the phonix-audio organisation.
Nothing had shipped before that, so there was exactly one moment in which
choosing them was free. That moment is over.

NAME does not repeat the vendor. A host prints VENDOR beside it already, so
"Phonix Cordis" would only spend the plugin list's width saying it twice.

A host application that resolves plugins by class id holds a copy of it in its
own DAWproject export. A test used to assert the two matched, which is no longer
possible now that the dependency runs the other way; both sides assert against
the literal instead, so a drift still fails a build, just two builds instead of
one. That is a note for whoever maintains that side: its copy has to be moved to
the value above in the same change, or its export names a plugin no host can
find. Nothing in this repository depends on it.

Test: `cordis-plugin`, `frozen_identifiers`.

## Parameter ids and persistence keys

    persist   patch  editor-state
    params    preset voicing unison width damper action release tune gain
              hybrid maxhold

Three of the ids deliberately differ from what they drive: `unison` sets
`unison_detune`, `action` sets `mechanics`, and `width` is labelled "Spread" in
the editor. They are frozen as they are.

These strings are JSON keys inside every saved project's plugin state and inside
every generated `.vstpreset`.

`hybrid` and `maxhold` were appended after the split from that host, which used
to switch the sample cache on through an engine method that no longer crosses
the boundary. Appending is safe: a project saved before they existed simply has
no key for them, and nice-plug's restore only visits the keys it finds. Adding is
always allowed; renaming and reordering are not.

## The patch's serde shape

    name voicing unison_detune width damper mechanics release_noise tune gain

In that order, and with the `#[serde(default)]` fallbacks intact: `mechanics`
defaults to 0.35 and `release_noise` to 0.5. A session written before those two
fields existed relies on them, so removing a default is a silent data change
rather than a compile error.

This JSON is what a host's session file stores for a Cordis track.

Tests: `cordis`, `the_patch_field_names_and_their_order_are_a_wire_format` and
`a_session_written_before_those_fields_still_loads`.

## The factory bank, names AND order

    Concert Grand · Concert Grand, Bright · Concert Grand, Mellow · Close Mics
    Player's Seat · Salon · Tuned Dead · Wide Unison · Long Dampers · Tight Dampers

The names are looked up as strings. The ORDER matters too: the plugin's integer
`preset` parameter indexes this vector, and that integer is what a host's
automation lane and every generated `.vstpreset` store. Reordering the bank
silently repoints saved projects at a different piano.

Append only.

Test: `cordis`, `the_factory_bank_is_a_wire_format`.

## The `.vstpreset` byte layout

48-byte header, `Comp`/`Info`/`List` chunk ids, uppercase-hex class id, and the
`MetaInfo` XML shape. Third-party hosts parse these.

Note one deliberate wart carried over unchanged: on Windows the preset directory
is `%USERPROFILE%\Documents\VST3 Presets\<vendor>\<plugin>`, because that is
where MediaBay looks. It used to be that path on *every* platform, with a literal
`C:\Users\Public\Documents` fallback, which on Linux is not an absolute path but
a single directory name containing backslashes; so every instantiation wrote its
whole bank into a junk folder in the current directory. The Linux and macOS
branches were fixed; the Windows one was not touched, so nothing already indexed
is orphaned.
