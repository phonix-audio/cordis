//! Shared disk save/load for plugin patches.
//!
//! Every engine's patch is serde-serializable (that's how the session persists
//! engine state), so a per-plugin "save/load to disk" is just a file picker plus
//! JSON. These two helpers centralise that so each editor only needs a Save and
//! a Load button, instead of re-deriving the dialog + serialization wiring.
//!
//! Both dialogs default to a per-engine user-preset directory,
//! `<data_dir>/presets/<engine>/`, where `<data_dir>` is the platform's own
//! application data location for this vendor and product (see `user_preset_dir`).
//! That gives saved patches an organised home and a known place to drop generated
//! preset packs so the Load dialog finds them immediately, instead of scattering
//! wherever the picker last happened to be.

use std::path::PathBuf;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Path-safe folder name for an engine: keep ASCII alphanumerics, fold anything
/// else (spaces, hyphens, separators) to `_` so no path component can escape.
fn preset_slug(engine: &str) -> String {
    engine
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// Per-engine user-preset directory, created on demand. `None` only if the
/// platform has no resolvable data directory (headless/sandboxed), in which
/// case the dialogs simply fall back to the picker's own default location.
fn user_preset_dir(engine: &str) -> Option<PathBuf> {
    // Vendor and product, so the editor's own patches sit beside nothing else.
    // Not a wire format: nothing outside this crate reads the path, and a preset
    // the user saved is reachable through the Load dialog wherever it lands.
    let dirs = directories::ProjectDirs::from("com", "Phonix Audio", "Cordis")?;
    let dir = dirs.data_dir().join("presets").join(preset_slug(engine));
    let _ = std::fs::create_dir_all(&dir);
    Some(dir)
}

/// Open a save dialog and write `patch` as pretty JSON. `engine` names the
/// instrument (dialog title + preset folder), `default_name` seeds the filename.
/// Returns true when a file was actually written.
pub fn save_patch_to_disk<P: Serialize>(patch: &P, engine: &str, default_name: &str) -> bool {
    let mut dialog = rfd::FileDialog::new()
        .set_title(format!("Save {engine} Patch"))
        .add_filter("JSON Patch", &["json"])
        .set_file_name(format!("{default_name}.json"));
    if let Some(dir) = user_preset_dir(engine) {
        dialog = dialog.set_directory(dir);
    }
    if let Some(path) = dialog.save_file() {
        if let Ok(json) = serde_json::to_string_pretty(patch) {
            return std::fs::write(path, json).is_ok();
        }
    }
    false
}

/// Open a load dialog, read and deserialize a patch. Returns `None` if the user
/// cancelled or the file could not be read/parsed as `P`.
pub fn load_patch_from_disk<P: DeserializeOwned>(engine: &str) -> Option<P> {
    let mut dialog = rfd::FileDialog::new()
        .set_title(format!("Load {engine} Patch"))
        .add_filter("JSON Patch", &["json"]);
    if let Some(dir) = user_preset_dir(engine) {
        dialog = dialog.set_directory(dir);
    }
    let path = dialog.pick_file()?;
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_slug_is_path_safe() {
        assert_eq!(preset_slug("Techno Kick"), "Techno_Kick");
        assert_eq!(preset_slug("VP330"), "VP330");
        assert_eq!(preset_slug("TB-303"), "TB_303");
        // No path separator can survive into a folder name.
        let s = preset_slug("a/b\\c..d");
        assert!(!s.contains('/') && !s.contains('\\') && !s.contains('.'), "got {s}");
    }

    #[test]
    fn user_preset_dir_is_per_engine_under_presets() {
        // ProjectDirs may be absent in a sandbox; only assert when it resolves.
        if let Some(dir) = user_preset_dir("Techno Kick") {
            assert!(dir.ends_with("Techno_Kick"), "leaf must be the engine slug: {dir:?}");
            assert!(dir.parent().map_or(false, |p| p.ends_with("presets")),
                "parent must be the shared presets/ folder: {dir:?}");
        }
    }
}

/// The shared Save / Load pair, for an editor whose chrome is hand-drawn.
///
/// `plugin_chrome_panel` already offers these, but several editors keep a
/// bespoke top bar (the VP-330's instrument panel, TechnoKick's) and so had NO
/// on-disk user presets at all: the factory bank and nothing else. Polaris had
/// them but hand-rolled its own `rfd` dialogs, which meant its files did not
/// land in the same per-engine directory as everyone else's.
///
/// Returns the loaded patch when the user picked one.
pub fn disk_preset_buttons<P>(
    ui: &mut egui::Ui,
    engine: &str,
    current: &P,
    default_name: &str,
) -> Option<P>
where
    P: Serialize + DeserializeOwned,
{
    let mut loaded = None;
    if ui.add(egui::Button::new(egui::RichText::new("Save").size(10.0))
        .min_size(egui::vec2(38.0, 20.0)))
        .on_hover_text("Save this patch to disk")
        .clicked()
    {
        save_patch_to_disk(current, engine, default_name);
    }
    if ui.add(egui::Button::new(egui::RichText::new("Load").size(10.0))
        .min_size(egui::vec2(38.0, 20.0)))
        .on_hover_text("Load a patch from disk")
        .clicked()
    {
        loaded = load_patch_from_disk::<P>(engine);
    }
    loaded
}

// a project-wide test lived here: it walked that
// repository's `src/gui` looking for an editor with a factory bank and no way
// to save a tweaked patch. There is one editor here, and it has Save and Load,
// so the sweep has nothing to sweep. It stayed behind with the directory it
// reads.
