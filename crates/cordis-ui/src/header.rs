//! The fascia strip: maker's mark, nameplate, and the two lamps.
//!
//! Replaces the shared plugin chrome, which is a good DAW header and the wrong
//! object here — a row of pill buttons over a grand piano. The preset is an
//! engraved brass plate because that is what a piano has where a synth has a
//! patch list.

use egui::{Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

use crate::colors::*;
use crate::theme;

/// What the header wants done, after the fact.
#[derive(Default)]
pub struct HeaderResult {
    /// Index into the factory bank.
    pub preset_selected: Option<usize>,
    pub save_clicked: bool,
    pub load_clicked: bool,
    /// The preview lamp was clicked; the caller flips the engine's hybrid mode.
    pub preview_toggled: bool,
}

/// Draw the strip. `preset_name` is shown on the plate, `preset_names` fills
/// the popup the plate opens.
pub fn draw(
    ui: &mut Ui,
    rect: Rect,
    preset_name: &str,
    preset_names: &[String],
    voices: u8,
    preview_on: bool,
    bank_fill: (u16, u16),
) -> HeaderResult {
    let mut out = HeaderResult::default();
    let mid = rect.center().y;

    // A hairline under the whole strip, separating fascia from instrument.
    ui.painter().line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, BORDER),
    );

    wordmark(ui, Pos2::new(rect.left() + 16.0, mid));

    // ── Nameplate ────────────────────────────────────────────────────
    let plate = Rect::from_center_size(Pos2::new(rect.center().x, mid), Vec2::new(300.0, 28.0));
    theme::gradient_plate(ui, plate);
    ui.painter().rect_stroke(
        plate,
        4.0,
        Stroke::new(1.0, Color32::from_rgb(90, 70, 36)),
        egui::StrokeKind::Inside,
    );
    theme::engraved(
        ui,
        plate.center(),
        preset_name,
        theme::display_font(ui.ctx(), 13.0),
        PLATE_ENGRAVED,
        Align2::CENTER_CENTER,
    );

    let plate_resp = ui.interact(plate, ui.id().with("plate"), Sense::click());
    // The plate is painted, so without this nothing tells the accessibility
    // tree — or a test — that this rectangle is what you press to change preset.
    plate_resp.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            format!("Preset: {preset_name}"),
        )
    });
    // The bank, hung under the plate.
    //
    // Through `Popup`, not the `Memory::*_popup` calls: those are deprecated in
    // egui 0.34, and they are deprecated because they only work if the caller
    // also calls `keep_popup_open` every single frame. Without it the entry
    // evaporates at the end of the frame after it opened, so the next press
    // "toggled" a popup egui already considered closed and re-opened it — the
    // list flashed once and then would not stay up. `Popup` owns that
    // bookkeeping, including not closing on the very click that opened it.
    let popup_id = ui.id().with("plate_popup");
    egui::Popup::from_toggle_button_response(&plate_resp)
        .id(popup_id)
        .align(egui::RectAlign::BOTTOM_START)
        .gap(4.0)
        .width(plate.width())
        .frame(
            egui::Frame::popup(ui.style())
                .fill(BG_PANEL)
                .stroke(Stroke::new(1.0, BORDER)),
        )
        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
        .show(|ui| {
            for (i, name) in preset_names.iter().enumerate() {
                let selected = name == preset_name;
                if ui.selectable_label(selected, name).clicked() {
                    out.preset_selected = Some(i);
                }
            }
        });

    // Chevrons step through the bank without opening it.
    let here = preset_names.iter().position(|n| n == preset_name);
    if chevron(ui, Pos2::new(plate.left() - 18.0, mid), false).clicked() {
        out.preset_selected = Some(here.map_or(0, |i| i.saturating_sub(1)));
    }
    if chevron(ui, Pos2::new(plate.right() + 18.0, mid), true).clicked() {
        let last = preset_names.len().saturating_sub(1);
        out.preset_selected = Some(here.map_or(0, |i| (i + 1).min(last)));
    }

    // ── Save / Load ──────────────────────────────────────────────────
    let mut x = plate.right() + 40.0;
    for (label, flag) in [("SAVE", &mut out.save_clicked), ("LOAD", &mut out.load_clicked)] {
        let r = Rect::from_min_size(Pos2::new(x, mid - 9.0), Vec2::new(46.0, 18.0));
        let resp = ui.interact(r, ui.id().with(label), Sense::click());
        let col = if resp.hovered() { GOLD_BRIGHT } else { GOLD.gamma_multiply(0.8) };
        ui.painter()
            .rect_stroke(r, 2.0, Stroke::new(1.0, col.gamma_multiply(0.6)), egui::StrokeKind::Inside);
        ui.painter()
            .text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(9.0), col);
        if resp.clicked() {
            *flag = true;
        }
        x += 54.0;
    }

    // ── Right tail: voices, then the preview lamp ────────────────────
    ui.painter().text(
        Pos2::new(rect.right() - 16.0, mid),
        Align2::RIGHT_CENTER,
        format!("{voices} voices"),
        FontId::monospace(10.0),
        TEXT_DIM,
    );

    let lamp_c = Pos2::new(rect.right() - 170.0, mid);
    theme::lamp(ui, lamp_c, 4.0, preview_on, GOLD_BRIGHT);
    // The label is the bank's own state: preparing, ready, or off. The whole
    // keyboard is rendered before the mode is worth its name, and the count
    // is the honest thing to show while that happens.
    let label = if !preview_on {
        "PREVIEW".to_string()
    } else if bank_fill.0 < bank_fill.1 {
        format!("PREP {}/{}", bank_fill.0, bank_fill.1)
    } else {
        "HYBRID".to_string()
    };
    ui.painter().text(
        lamp_c + Vec2::new(10.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(9.0),
        if preview_on { TEXT_PRIMARY } else { TEXT_DIM },
    );
    let lamp_hit = Rect::from_center_size(lamp_c + Vec2::new(24.0, 0.0), Vec2::new(80.0, 18.0));
    if ui.interact(lamp_hit, ui.id().with("preview"), Sense::click()).clicked() {
        out.preview_toggled = true;
    }

    out
}

/// The maker's mark, letterspaced and struck into the lacquer.
fn wordmark(ui: &Ui, left: Pos2) {
    let w = theme::tracked_text(
        ui,
        left,
        "CORDIS",
        theme::display_font(ui.ctx(), 21.0),
        GOLD,
        3.0,
        true,
    );
    ui.painter().line_segment(
        [
            Pos2::new(left.x, left.y + 14.0),
            Pos2::new(left.x + w, left.y + 14.0),
        ],
        Stroke::new(1.0, GOLD.gamma_multiply(0.45)),
    );
}

/// A painted triangle. Not a glyph: the default font has no dependable
/// arrowheads, and this editor ships no icon set.
fn chevron(ui: &mut Ui, centre: Pos2, right: bool) -> Response {
    let hit = Rect::from_center_size(centre, Vec2::splat(20.0));
    let resp = ui.interact(hit, ui.id().with(("chev", right)), Sense::click());
    let col = if resp.hovered() { GOLD_BRIGHT } else { GOLD.gamma_multiply(0.7) };
    let s = 5.0;
    let pts = if right {
        vec![
            centre + Vec2::new(-s * 0.5, -s),
            centre + Vec2::new(s * 0.7, 0.0),
            centre + Vec2::new(-s * 0.5, s),
        ]
    } else {
        vec![
            centre + Vec2::new(s * 0.5, -s),
            centre + Vec2::new(-s * 0.7, 0.0),
            centre + Vec2::new(s * 0.5, s),
        ]
    };
    ui.painter()
        .add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    fn bank() -> Vec<String> {
        vec!["Concert Grand".to_string(), "Felted".to_string(), "Honky".to_string()]
    }

    fn harness() -> Harness<'static> {
        let names = bank();
        Harness::builder()
            .with_size(egui::vec2(1280.0, 44.0))
            .build(move |ctx| {
                crate::theme::apply_visuals(ctx);
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| {
                        let r = ui.max_rect();
                        draw(ui, r, &names[0], &names, 0, false, (0, 0));
                    });
            })
    }

    /// Pressing the nameplate opens the bank and LEAVES it open.
    ///
    /// It did not: the click that opened the list landed on the plate, which is
    /// outside the popup, so the same frame's click-away test shut it again.
    /// All the user saw was a rectangle appearing and vanishing.
    #[test]
    fn the_bank_stays_open_when_the_nameplate_is_pressed() {
        let mut h = harness();
        h.run_steps(2);
        assert!(
            h.query_by_label("Felted").is_none(),
            "the bank was already open before anything was pressed"
        );

        h.get_by_label("Preset: Concert Grand").click();
        h.run_steps(2);
        assert!(
            h.query_by_label("Felted").is_some(),
            "the bank closed in the same frame it opened"
        );

        // Pressing the plate again puts it away.
        h.get_by_label("Preset: Concert Grand").click();
        h.run_steps(2);
        assert!(h.query_by_label("Felted").is_none(), "the bank would not close");

        // And so does choosing something from it.
        h.get_by_label("Preset: Concert Grand").click();
        h.run_steps(2);
        h.get_by_label("Felted").click();
        h.run_steps(2);
        assert!(
            h.query_by_label("Felted").is_none(),
            "the bank stayed open after a preset was chosen"
        );
    }
}
