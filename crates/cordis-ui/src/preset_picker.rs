//! Shared preset-picker widget.
//!
//! Plugin windows used to ship its own preset combo
//! + prev/next arrows, with slightly different label formatting,
//! widths, and id-salt conventions. Per the user's UX feedback
//! ("all plugins should use the same way to select presets") this
//! module owns the single canonical implementation; each plugin
//! calls `picker_ui` instead of building its own.
//!
//! **Design rule — no per-plugin layout drift.** Width, headers,
//! arrow semantics, label formatting are all fixed *inside* this
//! module. The caller only chooses identity (id salt + accent /
//! dim colour). Section-headers and category-scoped arrows are
//! turned on automatically when the preset bank carries categories;
//! no callsite flag controls them.
//!
//! Returns `Option<usize>` with the newly-selected index rather
//! than taking a callback, so the caller can do `self.send(...)`
//! without nested `&mut self` borrows.

use egui::{self, RichText, Ui};

/// Minimum number of distinct categories before the picker switches
/// from inline `"CATEGORY · name"` rows to a section-header combo
/// and category-scoped prev/next arrows.
const CATEGORY_GROUPING_THRESHOLD: usize = 2;

/// Fixed visual width of the combo box. Locked so every plugin
/// renders the picker at identical dimensions.
const COMBO_WIDTH_PX: f32 = 200.0;

/// Minimal contract a preset has to expose for the picker.
pub trait Preset {
    fn preset_name(&self) -> &str;
    /// Optional category tag. When the bank has 2+ distinct
    /// categories, the picker auto-enables section headers and
    /// category-scoped prev/next.
    fn preset_category(&self) -> Option<&str> { None }
    // `track_fx` is NOT part of this copy. Elsewhere a preset can describe an
    // insert-FX chain for the host to apply; that pulls in `fx_chain`, and a
    // plugin has no track inserts to apply it to.
}

/// Lightweight adapter for engines whose preset list is just
/// `(name, category)` pairs (e.g. Singer voice presets, drum
/// patterns, drum kits). Construct on the fly from any source and
/// hand to `picker_ui` without having to invent a wrapper type.
pub struct NamedPreset<'a> {
    pub name:     &'a str,
    pub category: Option<&'a str>,
}

impl<'a> Preset for NamedPreset<'a> {
    fn preset_name(&self) -> &str { self.name }
    fn preset_category(&self) -> Option<&str> { self.category }
}

/// Per-plugin state for the picker. Lives on the plugin app.
#[derive(Default)]
pub struct PresetPickerState {
    pub current_idx: usize,
}

impl PresetPickerState {
    /// Match the picker's selected index to the preset whose
    /// `preset_name()` equals `name`. Use this on session restore /
    /// patch-restore paths so the dropdown reflects the loaded patch
    /// instead of staying at index 0. Falls back to leaving the
    /// index untouched if no preset matches — the picker just won't
    /// highlight any entry, which is the desired "off-bank patch"
    /// behaviour.
    pub fn sync_to_name<P: Preset>(&mut self, presets: &[P], name: &str) {
        if let Some(i) = presets.iter().position(|p| p.preset_name() == name) {
            self.current_idx = i;
        }
    }
}

/// Identity-only style. Width, layout, section behaviour, arrow
/// semantics are fixed inside the widget; callers must not be able
/// to drift those. The only knobs are the unique id salt and the
/// two colours that carry the plugin's visual identity.
pub struct PresetPickerStyle<'a> {
    pub salt:   &'a str,            // unique id for ComboBox state
    pub accent: egui::Color32,      // plugin's accent colour
    pub dim:    egui::Color32,      // secondary colour
}

/// Render the picker. Returns `Some(new_idx)` if the user picked a
/// different preset this frame (combo pick OR arrow press), `None`
/// otherwise.
///
/// The returned index is always in *caller* order — same as how it
/// indexes into `presets`. Internally the widget may sort entries
/// by category for display; that's an implementation detail and
/// never leaks out.
pub fn picker_ui<P: Preset>(
    ui:      &mut Ui,
    style:   &PresetPickerStyle,
    state:   &mut PresetPickerState,
    presets: &[P],
) -> Option<usize> {
    if presets.is_empty() { return None; }

    // Defensive clamp: stale current_idx (e.g. a session saved
    // against a larger bank) would otherwise point past the end of
    // the current presets array. Without this, picker_ui still
    // worked (prev/next defaulted to render position 0) but the
    // displayed name was "(none)" and any subsequent sync_to_name
    // miss would keep it stale forever. Clamp once here and the
    // picker behaviour stays consistent.
    if state.current_idx >= presets.len() {
        state.current_idx = 0;
    }

    // Auto-detect grouping from the data. No caller flag.
    let mut distinct_cats: Vec<&str> = Vec::new();
    for p in presets {
        if let Some(c) = p.preset_category() {
            if !distinct_cats.iter().any(|x| *x == c) { distinct_cats.push(c); }
        }
    }
    let grouped = distinct_cats.len() >= CATEGORY_GROUPING_THRESHOLD;

    // Render order: when grouped, stable-sort caller indices so all
    // entries sharing a category appear contiguously. Categories
    // appear in the order they first occur in `presets` (preserving
    // caller intent for category ordering). Within a category, the
    // original relative order is kept. Without grouping, render
    // order equals caller order.
    let render_order: Vec<usize> = if grouped {
        let mut v: Vec<usize> = (0..presets.len()).collect();
        v.sort_by_key(|&i| {
            match presets[i].preset_category() {
                Some(c) => distinct_cats.iter().position(|x| *x == c).unwrap_or(usize::MAX),
                None    => usize::MAX,
            }
        });
        v
    } else {
        (0..presets.len()).collect()
    };

    // Caller-index ↔ render-position lookup. Used to (a) highlight
    // the current row, (b) step in visual order with the arrows,
    // (c) display the "N / total" counter at the picker's right.
    let cur_render_pos = render_order.iter()
        .position(|&i| i == state.current_idx)
        .unwrap_or(0);

    let mut newly_selected: Option<usize> = None;

    ui.horizontal(|ui| {
        let cur_label = presets.get(state.current_idx)
            .map(|p| p.preset_name())
            .unwrap_or("(none)");
        let cur_cat = presets.get(state.current_idx)
            .and_then(|p| p.preset_category());

        if grouped {
            // Fixed-width category chip: the category NAME changes width as the
            // user navigates presets (and vanishes for a None-category one), which
            // would shove the combo + arrows + preset counter that follow. Reserve
            // its slot so nothing after it moves. (Shared picker → fixes it for
            // every plugin + the FX rack headers at once.)
            crate::widgets::fixed_label(ui, 66.0,
                RichText::new(cur_cat.unwrap_or("")).color(style.dim).size(10.0).strong());
        }

        egui::ComboBox::from_id_salt(style.salt)
            .selected_text(RichText::new(cur_label).color(style.accent))
            .width(COMBO_WIDTH_PX)
            .show_ui(ui, |ui| {
                let mut last_cat: Option<&str> = None;
                for &orig_idx in &render_order {
                    let p = &presets[orig_idx];
                    let cat = p.preset_category();
                    if grouped && cat != last_cat {
                        // First entry: no separator above; subsequent
                        // category changes get a separator + chip.
                        if last_cat.is_some() { ui.separator(); }
                        if let Some(c) = cat {
                            ui.label(RichText::new(c).color(style.dim).size(10.0));
                        }
                        last_cat = cat;
                    }
                    let is_sel = orig_idx == state.current_idx;
                    let label = p.preset_name().to_string();
                    if ui.selectable_label(is_sel, label).clicked() {
                        newly_selected = Some(orig_idx);
                    }
                }
            });

        // Prev / next — walk render order so the user sees the same
        // sequence the dropdown displays. Wraps at the ends.
        if ui.small_button("<").on_hover_text("Previous preset").clicked() {
            let n = render_order.len();
            let next_pos = if cur_render_pos == 0 { n - 1 } else { cur_render_pos - 1 };
            newly_selected = Some(render_order[next_pos]);
        }
        if ui.small_button(">").on_hover_text("Next preset").clicked() {
            let next_pos = (cur_render_pos + 1) % render_order.len();
            newly_selected = Some(render_order[next_pos]);
        }

        // Counter follows render position so "12 / 64" matches what
        // the user sees when scrolling the dropdown.
        let counter = format!("{} / {}", cur_render_pos + 1, presets.len());
        ui.label(RichText::new(counter).color(style.dim).size(9.0));
    });

    if let Some(i) = newly_selected {
        state.current_idx = i;
    }
    newly_selected
}

// A test-only `render_order` helper and the picker's own tests stayed in
// elsewhere: they walk that repository's editor sources.
