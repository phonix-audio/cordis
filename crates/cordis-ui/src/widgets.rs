//! The widgets Cordis's editor needs.
//!
//! A COPY, deliberately. A shared UI crate would have to live somewhere both
//! repositories can reach, which reinstates exactly the coupling this split
//! exists to remove; and a host must keep drawing its own window with
//! its own widgets, or one editor in a stack of twenty-eight would wear a
//! different chrome from the rest.
//!
//! What is copied is the STABLE subset: knobs, the chrome bar, the meter. None
//! of the declarative renderer, the FX rack or the icon table comes with it.

use crate::colors::*;
use egui::*;

/// Inline SVG, kept because `plugin_chrome` renders an optional icon on each
/// mode pill. Cordis passes no pills, so this never runs for it — but it has
/// to compile, and stubbing it would silently break the first editor that does.
pub fn icon_inline(ui: &mut Ui, svg: &'static [u8], uri: &'static str, size: f32, tint: Color32) {
    let img = egui::Image::from_bytes(uri, svg)
        .fit_to_exact_size(Vec2::splat(size))
        .tint(tint);
    ui.add(img);
}

/// Register the image loaders the icons need.
pub fn install_image_loaders(ctx: &egui::Context) {
    egui_extras::install_image_loaders(ctx);
}

pub const GAP_SECTIONS: f32 = 16.0;
pub const GAP_KNOB_LABEL: f32 = 4.0;
/// UNIFORM knob-group width. Every knob is laid out this wide (or wider only if
/// its diameter exceeds it), INDEPENDENT of the label, so knob rows line up in
/// columns instead of each knob sizing to its own label and shoving neighbours.
/// Fits labels up to ~9 chars at 10pt; longer ones are painted centred and bleed
/// a couple px into the inter-knob gap (no layout displacement). Generalises the
/// TB-303 / Mellotron `set_min_width(50)` pattern to every editor.
pub const KNOB_GROUP_W: f32 = 52.0;

// ERGO: three visual states for a knob — idle, hover (accent glow ring) and
// dragging (full accent arc). The value shows as a tooltip so the idle state
// stays clean; the scroll wheel gives fine adjustment.

/// Modulation overlay passed to `draw_knob_body` when a knob is targeted by a
/// mod matrix. `modulated_value` is the live (base + routes) value in the same
/// 0..1 space as `value`; the source colours are drawn as small rim dots so it
/// is visible at a glance what is moving the knob.
///
/// Cordis has no mod matrix, so it always passes `None` — but the knob is
/// copied whole rather than stripped, because a knob that renders differently
/// from the shared one is exactly the drift this copy is meant to avoid.
pub struct KnobMod<'a> {
    pub modulated_value: f32,
    pub source_colors:   &'a [Color32],
}

fn draw_knob_body(
    ui: &mut Ui,
    value: &mut f32,
    label: &str,
    display_text: &str,
    size: f32,
    accent: Color32,
    mod_overlay: Option<&KnobMod<'_>>,
) -> (egui::Response, f32) {
    let s = size;

    let label_height = 14.0;
    let total_height = s + label_height + GAP_KNOB_LABEL;
    // UNIFORM group width (independent of the label): previously group_w grew to
    // the label, so a long label ("Sub Drone", "Vib Depth", "Destruct") made its
    // knob wider and shoved the following knobs right / broke column alignment
    // across rows. Now every knob of a given size is the same width, so knob rows
    // form aligned columns. Labels up to ~9 chars fit centred; longer ones bleed
    // a couple px into the inter-knob gap (painted, no layout displacement).
    let label_up = label.to_uppercase();
    let group_w = s.max(KNOB_GROUP_W);
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(group_w, total_height), Sense::click_and_drag());
    // The label is painted, not laid out, so without this a knob is invisible to
    // the accessibility tree — and to the tests that assert every parameter has
    // a control. A rotary is a slider as far as a screen reader is concerned.
    response.widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), *value as f64, label));
    let painter = ui.painter();
    let center = Pos2::new(rect.center().x, rect.top() + s / 2.0);
    let radius = s / 2.0 - 4.0;

    let hovered = response.hovered() || response.dragged();
    let dragging = response.dragged();

    // ERGO: 3-state knob — idle (subtle), hover (glow ring + value), active/drag (bright glow + value)
    if hovered {
        let glow_alpha = if dragging { 140u8 } else { 80 };
        let glow_width = if dragging { 2.5 } else { 1.5 };
        painter.circle_stroke(
            center,
            radius + 5.0,
            Stroke::new(glow_width, Color32::from_rgba_unmultiplied(
                accent.r(), accent.g(), accent.b(), glow_alpha,
            )),
        );
    }

    // Outer ring delimiter
    painter.circle_stroke(center, radius + 3.0, Stroke::new(1.0, BORDER));
    painter.circle_filled(center, radius + 2.0, BG_DARK);

    // Background circle
    painter.circle_filled(center, radius, BG_RAISED);
    painter.circle_stroke(center, radius, Stroke::new(1.0, BORDER));

    // Inner circle (recessed look — darker center)
    painter.circle_filled(center, radius - 3.0, BG_DARK);

    // Arc indicator (270 degree sweep)
    let start_angle = std::f32::consts::PI * 0.75;
    let sweep = std::f32::consts::PI * 1.5;
    let segments = 48;
    let active_end = start_angle + sweep * *value;

    // Background arc (thicker track)
    for i in 0..segments {
        let t0 = i as f32 / segments as f32;
        let t1 = (i + 1) as f32 / segments as f32;
        let a0 = start_angle + sweep * t0;
        let a1 = start_angle + sweep * t1;
        let r = radius + 1.0;
        let p0 = center + Vec2::new(a0.cos(), a0.sin()) * r;
        let p1 = center + Vec2::new(a1.cos(), a1.sin()) * r;
        painter.line_segment([p0, p1], Stroke::new(3.0, Color32::from_rgb(34, 30, 24)));
    }

    // Active arc — accent color with glow on hover
    for i in 0..segments {
        let t0 = i as f32 / segments as f32;
        let t1 = (i + 1) as f32 / segments as f32;
        let a0 = start_angle + sweep * t0;
        let a1 = start_angle + sweep * t1;
        if a1 <= active_end {
            let r = radius + 1.0;
            let p0 = center + Vec2::new(a0.cos(), a0.sin()) * r;
            let p1 = center + Vec2::new(a1.cos(), a1.sin()) * r;
            // Thicker active arc + glow when hovered
            let arc_width = if hovered { 3.5 } else { 3.0 };
            painter.line_segment([p0, p1], Stroke::new(arc_width, accent));
            // Soft glow behind active arc on hover
            if hovered {
                let glow_r = radius + 1.0;
                let gp0 = center + Vec2::new(a0.cos(), a0.sin()) * glow_r;
                let gp1 = center + Vec2::new(a1.cos(), a1.sin()) * glow_r;
                painter.line_segment([gp0, gp1], Stroke::new(6.0, Color32::from_rgba_unmultiplied(
                    accent.r(), accent.g(), accent.b(), 30,
                )));
            }
        }
    }

    // ── Modulation overlay (if any) ────────────────────────────────────
    // Draw a thin differential arc between the base value and the live
    // modulated value, plus small colored dots at the outer rim — one
    // per modulation source — so the user can read "this knob is being
    // modulated by LFO 1 and Env 2" at a glance.
    if let Some(m) = mod_overlay {
        let mv = m.modulated_value.clamp(0.0, 1.0);
        let a0 = start_angle + sweep * (*value).clamp(0.0, 1.0);
        let a1 = start_angle + sweep * mv;
        // Differential arc on the outside of the main ring.
        let mod_r = radius + 5.5;
        let seg = 24;
        let (from, to) = if a0 <= a1 { (a0, a1) } else { (a1, a0) };
        for i in 0..seg {
            let t0 = i as f32 / seg as f32;
            let t1 = (i + 1) as f32 / seg as f32;
            let p0 = center + Vec2::new((from + (to - from) * t0).cos(),
                                        (from + (to - from) * t0).sin()) * mod_r;
            let p1 = center + Vec2::new((from + (to - from) * t1).cos(),
                                        (from + (to - from) * t1).sin()) * mod_r;
            let col = Color32::from_rgba_unmultiplied(255, 255, 255, 200);
            painter.line_segment([p0, p1], Stroke::new(2.0, col));
        }
        // Small filled dot at the live-value tip for extra readability.
        let tip = center + Vec2::new(a1.cos(), a1.sin()) * mod_r;
        painter.circle_filled(tip, 2.5, Color32::WHITE);

        // Rim source indicators: evenly-spaced colored dots at the top
        // of the knob so they don't clash with the arc.
        let n = m.source_colors.len().min(6);
        if n > 0 {
            let spread = 0.55; // radians
            let base_a = -std::f32::consts::PI / 2.0;
            let step = if n == 1 { 0.0 } else { spread / (n - 1) as f32 };
            for (i, c) in m.source_colors.iter().take(n).enumerate() {
                let a = base_a - spread / 2.0 + step * i as f32;
                let p = center + Vec2::new(a.cos(), a.sin()) * (radius + 8.0);
                painter.circle_filled(p, 2.0, *c);
            }
        }
    }

    // Pointer line
    let pointer_angle = start_angle + sweep * *value;
    let pointer_start = center + Vec2::new(pointer_angle.cos(), pointer_angle.sin()) * (radius * 0.3);
    let pointer_end = center + Vec2::new(pointer_angle.cos(), pointer_angle.sin()) * (radius - 5.0);
    let pointer_color = if hovered { Color32::WHITE } else { Color32::from_rgb(228, 218, 198) };
    painter.line_segment([pointer_start, pointer_end], Stroke::new(2.0, pointer_color));

    // Pointer dot at tip
    let dot_pos = center + Vec2::new(pointer_angle.cos(), pointer_angle.sin()) * (radius - 5.0);
    painter.circle_filled(dot_pos, if hovered { 2.5 } else { 2.0 }, pointer_color);

    // ERGO: Value shown directly in knob center on hover (no tooltip required)
    if hovered {
        // Show value in center of knob
        let font_size = (s * 0.22).max(8.0).min(12.0);
        painter.text(
            center,
            Align2::CENTER_CENTER,
            display_text,
            FontId::monospace(font_size),
            TEXT_PRIMARY,
        );
    }

    // Label: 10px, TEXT_DIM, uppercase (doc/GUI.md § Typography)
    painter.text(
        Pos2::new(center.x, rect.top() + s + GAP_KNOB_LABEL),
        Align2::CENTER_TOP,
        &label_up,
        FontId::proportional(10.0),
        if hovered { TEXT_PRIMARY } else { TEXT_DIM },
    );

    // Handle drag — supports both vertical and horizontal movement.
    // Uses the LARGER of |dx| and |dy| so horizontal sweeps work too,
    // solving the "cursor hits window top/bottom edge" problem.
    // Shift = fine control (5x precision).
    if response.dragged() {
        let drag = response.drag_delta();
        // Use vertical primarily, but if horizontal motion is larger, use that instead
        // (right = increase, left = decrease — like a horizontal slider)
        let delta = if drag.x.abs() > drag.y.abs() {
            drag.x
        } else {
            -drag.y
        };
        let sensitivity = if ui.input(|i| i.modifiers.shift) { 500.0 } else { 100.0 };
        *value = (*value + delta / sensitivity).clamp(0.0, 1.0);
    }

    // Handle scroll wheel (doc/GUI.md § Knob)
    if hovered {
        let scroll = ui.input(|i| i.smooth_scroll_delta().y);
        if scroll != 0.0 {
            let sensitivity = if ui.input(|i| i.modifiers.shift) { 2000.0 } else { 400.0 };
            let delta = scroll / sensitivity;
            *value = (*value + delta).clamp(0.0, 1.0);
        }
    }

    // ERGO: Double-click resets knob to center (50%)
    if response.double_clicked() {
        *value = 0.5;
    }

    (response, *value)
}

/// Draw a rotary knob with LED ring. Value shown on hover as tooltip.
pub fn knob(ui: &mut Ui, value: &mut f32, label: &str, size: f32, accent: Color32) -> f32 {
    let display = format!("{:.0}%", *value * 100.0);
    draw_knob_body(ui, value, label, &display, size, accent, None).1
}

/// Draw a knob (or any small control) CENTERED in a fixed cell of `width` x
/// `height`. Wrap each knob in a row/grid with the SAME width and their centres
/// line up regardless of label length -- the clean way to get an aligned knob
/// grid without the shared knob auto-sizing to each label. Opt-in per call:
/// pick a `width` >= the widest label in the group; only worth it where the
/// layout has the horizontal room (roomy editors), so tight ones are untouched.
///
/// `height` MUST be the full knob footprint (`size + GAP_KNOB_LABEL + 14`, i.e.
/// `size + 18` for the standard label band). A zero/short height leaves the cell
/// sized only by its content, and inside a `horizontal_wrapped` row that makes
/// egui's cross-axis centring stagger the knobs vertically (each cell centres by
/// its own height). A fixed height makes every cell identical so the row stays
/// flat. Use [`knob_cell_for`] to derive it from a knob `size`.
pub fn knob_cell<R>(ui: &mut Ui, width: f32, height: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.allocate_ui_with_layout(
        Vec2::new(width, height),
        Layout::top_down(Align::Center),
        add,
    ).inner
}

/// [`knob_cell`] with the height derived from the knob `size` (the standard
/// `size + GAP_KNOB_LABEL + label band`). Prefer this so callers never mis-size
/// the cell height and reintroduce the wrapped-row stagger.
pub fn knob_cell_for<R>(ui: &mut Ui, width: f32, size: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    knob_cell(ui, width, size + GAP_KNOB_LABEL + 14.0, add)
}

/// Vertically centre a SHORT control (label, checkbox, combo, selector) on the
/// knob-CIRCLE line: reserve a `size`-tall cell (the circle height, NOT the full
/// knob-plus-label footprint) sized to its own content width, and centre the
/// content in it. Use this inside a `horizontal_top` row so short controls line
/// up with the knobs' circles instead of dropping to the row's overall centre
/// (which sits below the circle because of the knob's label band). This is the
/// SAME primitive the declarative renderer uses; hand-drawn Custom closures must
/// use it too so alignment never drifts between the two paths.
pub fn vcell<R>(ui: &mut Ui, size: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.allocate_ui_with_layout(
        Vec2::new(1.0, size),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_height(size);
            add(ui)
        },
    ).inner
}

/// Knob with custom value display text.
pub fn knob_fmt(ui: &mut Ui, value: &mut f32, label: &str, display_text: &str, size: f32, accent: Color32) -> f32 {
    draw_knob_body(ui, value, label, display_text, size, accent, None).1
}

/// Draw a label in a FIXED-WIDTH slot so its content NEVER reflows the widgets
/// that follow it in a horizontal row. Live-updating status text (a current
/// phoneme, a value readout, an active-voice / step counter) changes width as it
/// updates; placed bare in a `ui.horizontal(..)` it shoves everything after it
/// sideways. This reserves exactly `width` and truncates overflow, so neighbours
/// never move. The shared fix for "text blocks that displace the rest" — use it
/// for any runtime-varying text sitting before other widgets on a row.
pub fn fixed_label(ui: &mut Ui, width: f32, text: RichText) {
    let h = ui.spacing().interact_size.y;
    ui.allocate_ui_with_layout(
        Vec2::new(width, h),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_width(width);
            ui.set_max_width(width);
            ui.add(egui::Label::new(text).truncate());
        },
    );
}

/// Place `add` in a fixed-width cell as tall as a knob's CIRCLE (`knob_size`),
/// vertically centred. A knob reserves a value band UNDER its circle, so a plain
/// label / dropdown / text field beside it (centred on the full row height)
/// drops below the circle. Wrapping the non-knob elements with this, inside a

// proportionally to peak, red beyond 0.9. Caller passes the size +
// accent so each plugin keeps its identity colour.

/// Draw a horizontal peak meter. `peak` is clamped to 0..1 and used
/// as the fill width fraction. Above 0.9 the fill turns red — the
/// universal convention every plugin already used.
pub fn peak_meter(ui: &mut Ui, peak: f32, size: Vec2, accent: Color32) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let bg = Color32::from_rgb(30, 30, 35);
    ui.painter().rect_filled(rect, 2u8, bg);
    let fill_w = peak.clamp(0.0, 1.0) * size.x;
    let fill_rect = Rect::from_min_size(rect.min, Vec2::new(fill_w, size.y));
    let fill_colour = if peak > 0.9 { Color32::RED } else { accent };
    ui.painter().rect_filled(fill_rect, 2u8, fill_colour);
}

/// Vertical peak meter — fills from the bottom up. Used in mixer
/// strips alongside the volume fader so the strip has a real
/// channel-meter look. `peak` is clamped to 0..1; above 0.9 the fill
/// turns red. Includes a faint 0 dB tick at the 0.79 mark (≈ -1 dB
/// from clip) to anchor the eye.

//     if let Some(i) = res.pill_clicked   { ... }
//     if let Some(i) = res.preset_selected { ... }

use super::preset_picker;

/// One selectable pill in the header's mode strip.
pub struct PillEntry<'a> {
    pub label:    &'a str,
    pub selected: bool,
}

/// Inputs to `plugin_chrome`. Builder-style — set only what the
/// plugin actually has. Empty `mode_pills` skips the strip, `None`
/// status_right skips the status block.
pub struct PluginChrome<'a> {
    pub title:        &'a str,
    pub accent:       Color32,
    pub dim:          Color32,
    pub peak:         f32,
    /// CPU load 0..1, shown as a FIXED-WIDTH `CPU nnn%` readout in the right tail
    /// (uniform place + constant width across every plugin, so it never shifts the
    /// header). `None` = not shown.
    pub cpu:          Option<f32>,
    pub preset_salt:  &'a str,
    pub mode_pills:   &'a [PillEntry<'a>],
    pub status_right: Option<&'a str>,
}

/// What the chrome reports back to the plugin. The plugin reacts by
/// sending an engine command + updating its mirror.
pub struct ChromeResult {
    pub pill_clicked:    Option<usize>,
    pub preset_selected: Option<usize>,
    /// The "Save" disk button was clicked — the plugin should write its patch
    /// to disk (e.g. via `crate::preset_io::save_patch_to_disk`).
    pub save_clicked:    bool,
    /// The "Load" disk button was clicked — the plugin should load a patch from
    /// disk (e.g. via `crate::preset_io::load_patch_from_disk`).
    pub load_clicked:    bool,
}

/// Render the standard plugin header. Returns indices of any UI
/// events the plugin should react to.
///
/// Alignment discipline (the pre-fix chrome was visibly mismatched):
/// - Title uses `ui.label(...).size(CHROME_TITLE_PT)` not `ui.heading()`
///   (heading's default 17 pt towered over the 11 pt pills, creating
///   baseline drift and a tall row in pill-less plugins).
/// - Every text element on the row uses the same `Align::Center`
///   vertical alignment so pills, picker chrome and status text sit
///   on one horizontal line regardless of font size.
/// - The row height is pinned to `CHROME_ROW_H` so a pill-less
///   plugin (Atmosphera) and a pill plugin (Bass) get identical
///   header heights and the outer frame doesn't jump.
/// - 12 px gap between every logical block (title | pills | picker)
///   plus a guaranteed 12 px gap between picker and the right-aligned
///   tail.
pub fn plugin_chrome<P: preset_picker::Preset>(
    ui:           &mut Ui,
    chrome:       &PluginChrome,
    preset_state: &mut preset_picker::PresetPickerState,
    presets:      &[P],
) -> ChromeResult {
    let mut pill_clicked = None;
    let mut preset_selected = None;
    let mut save_clicked = false;
    let mut load_clicked = false;

    ui.horizontal(|ui| {
        ui.set_min_height(CHROME_ROW_H);
        // Title — uses a fixed-size label rather than ui.heading() so
        // the row keeps a predictable baseline / height.
        ui.label(RichText::new(chrome.title)
            .color(chrome.accent).strong().size(CHROME_TITLE_PT));
        ui.add_space(CHROME_GAP);

        // Optional mode pills.
        for (i, pill) in chrome.mode_pills.iter().enumerate() {
            let col = if pill.selected { chrome.accent } else { chrome.dim };
            let text = RichText::new(pill.label).color(col).strong().size(CHROME_PILL_PT);
            if ui.selectable_label(pill.selected, text).clicked() {
                pill_clicked = Some(i);
            }
        }
        if !chrome.mode_pills.is_empty() { ui.add_space(CHROME_GAP); }

        // Preset picker — fixed 200 px wide widget that handles its
        // own internal layout. Identity-only style.
        let style = preset_picker::PresetPickerStyle {
            salt: chrome.preset_salt, accent: chrome.accent, dim: chrome.dim,
        };
        preset_selected = preset_picker::picker_ui(ui, &style, preset_state, presets);

        // Disk save/load, next to the factory-preset picker.
        ui.add_space(CHROME_GAP);
        if ui.small_button(RichText::new("Save").color(chrome.dim).size(CHROME_PILL_PT)).clicked() {
            save_clicked = true;
        }
        if ui.small_button(RichText::new("Load").color(chrome.dim).size(CHROME_PILL_PT)).clicked() {
            load_clicked = true;
        }

        // Right-aligned tail: status text (if any) then meter.
        // Guaranteed gap from the picker is enforced by the RTL block
        // starting only after `add_space` — picker ↔ tail can't collide.
        ui.add_space(CHROME_GAP);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            peak_meter(ui, chrome.peak, Vec2::new(60.0, 8.0), chrome.accent);
            // Fixed-width CPU readout, uniform across every plugin. `{:>3.0}` +
            // monospace keeps `CPU   5%` and `CPU 100%` the SAME width, so the
            // value never shifts the header (the Polaris "blocks move" class).
            if let Some(cpu) = chrome.cpu {
                ui.add_space(8.0);
                let pct = (cpu * 100.0).clamp(0.0, 999.0);
                let col = if cpu > 0.85 { Color32::from_rgb(255, 70, 50) }
                    else if cpu > 0.6 { Color32::from_rgb(230, 180, 60) }
                    else { Color32::from_rgb(120, 190, 120) };
                ui.label(RichText::new(format!("CPU {pct:>3.0}%"))
                    .monospace().size(CHROME_STATUS_PT).color(col));
            }
            if let Some(s) = chrome.status_right {
                ui.add_space(8.0);
                ui.label(RichText::new(s).color(chrome.dim).size(CHROME_STATUS_PT));
            }
            // a standalone build draws an audio-device status and control here, from a slot
            // its standalone binary publishes into the egui context. A plugin
            // never owns the device, so that block does not come with us.
        });
    });

    ChromeResult { pill_clicked, preset_selected, save_clicked, load_clicked }
}

const CHROME_ROW_H:    f32 = 26.0;
const CHROME_TITLE_PT: f32 = 14.0;
const CHROME_PILL_PT:  f32 = 11.0;
const CHROME_STATUS_PT:f32 = 10.0;
const CHROME_GAP:      f32 = 12.0;

/// Plugin chrome wrapped in a uniform `Panel::top`. Use this from
/// `draw_ui(ctx)` BEFORE the plugin's `CentralPanel::show` so every
/// plugin window gets identical chrome framing (fill, border stroke,
/// inner margin, height) regardless of what theme the plugin uses
/// in its body. Replaces the per-plugin pattern of calling
/// `plugin_chrome` inline inside a CentralPanel — which inherited
/// whatever frame the CentralPanel was using and made the chrome
/// look different from plugin to plugin.
///
/// Returns the same `ChromeResult` as `plugin_chrome`. Caller still
/// reacts to `pill_clicked` / `preset_selected`.
pub fn plugin_chrome_panel<P: preset_picker::Preset>(
    ctx:          &egui::Context,
    chrome:       &PluginChrome,
    preset_state: &mut preset_picker::PresetPickerState,
    presets:      &[P],
) -> ChromeResult {
    use super::colors::{BG_RAISED, BORDER};
    egui::Panel::top("plugin_chrome_panel")
        .frame(
            egui::Frame::NONE
                .fill(BG_RAISED)
                .inner_margin(egui::Margin::symmetric(10i8, 5i8))
                .stroke(egui::Stroke::new(1.0, BORDER)),
        )
        .show(ctx, |ui| plugin_chrome(ui, chrome, preset_state, presets))
        .inner
}

// ── Empty-state CTA ───────────────────────────────────────────────────────
//
// Shown in session view / arrangement view / mixer when there are zero
// tracks — gives the user a single obvious "what now" instead of a blank
// panel. The "+ Add your first track" control opens the SAME category-grouped
// Add-Track picker as the sidebar menu, supplied by the caller as `add_menu`
// (it has the session/undo/tx in scope). Previously this was a plain button
// that only set an unread memory flag, so it did nothing.

/// ~10 plugins whose per-knob helpers bypassed the cooldown-arming `send()`).
///
/// Instead of plumbing a cooldown through every knob helper, this gate blocks
/// adoption centrally whenever ANY pointer button is down or a text field has
/// keyboard focus, and for `HOLD` frames after the interaction ends (covering
/// the engine's publish round-trip). Call once per frame from `draw_ui` and
/// only adopt when it returns true. Per-command `sync_cooldown`s remain useful
/// for slow round-trips (preset loads) and compose with this gate.
pub fn mirror_adopt_gate(ctx: &egui::Context, hold_frames: &mut u8) -> bool {
    const HOLD: u8 = 20;
    let interacting = ctx.input(|i| i.pointer.any_down()) || ctx.wants_keyboard_input();
    if interacting {
        *hold_frames = HOLD;
        return false;
    }
    if *hold_frames > 0 {
        *hold_frames -= 1;
        return false;
    }
    true
}
