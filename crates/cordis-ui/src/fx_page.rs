//! The effects page: the chain a patch carries, opened up.
//!
//! Which effects run and in what order is not editable here -- that is what
//! curated means, and the recipe lives with the patch. Everything inside each
//! effect is. Drawn with Cordis's own widgets, on the same lacquer, in the
//! same clusters as the instrument page, and each effect shows what it does:
//! the EQ its response, the compressor and the ceiling their transfer, from
//! the same numbers the audio thread runs.

use egui::{Align2, FontId, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use phonix_fx::effects::{EffectType, ParametricEqEffect, ReverbType};
use phonix_fx::fx_chain::{FxChainSpec, FxSlotSpec};
use phonix_fx::fx_params::{pid, FxParamAccess};

use crate::colors::{BG_DARK, BORDER, GOLD, GOLD_BRIGHT, TEXT_DIM};
use crate::preset_picker::{picker_ui, NamedPreset, PresetPickerState, PresetPickerStyle};
use crate::{theme, widgets};

/// What the page keeps between frames: the dropdown's own state.
#[derive(Default)]
pub struct FxPageState {
    reverb_picker: PresetPickerState,
}

/// One slot's sparse `(pid, value)` list, read and written as a dense bank.
/// A pid the slot has never carried reads as 0, and writing appends it.
struct SlotAccess<'a> {
    slot: &'a mut FxSlotSpec,
    touched: bool,
}

impl FxParamAccess for SlotAccess<'_> {
    fn get(&self, param_id: u8) -> f32 {
        self.slot
            .params
            .iter()
            .find(|(p, _)| *p == param_id)
            .map(|(_, v)| *v)
            .unwrap_or(0.0)
    }

    fn set(&mut self, param_id: u8, value: f32) {
        match self.slot.params.iter_mut().find(|(p, _)| *p == param_id) {
            Some(e) => {
                if e.1 != value {
                    e.1 = value;
                    self.touched = true;
                }
            }
            None => {
                self.slot.params.push((param_id, value));
                self.touched = true;
            }
        }
    }
}

impl SlotAccess<'_> {
    fn has(&self, p: u8) -> bool {
        self.slot.params.iter().any(|(k, _)| *k == p)
    }

    /// Materialise a value the effect would use anyway, without counting it
    /// as an edit: the page must show the number the effect runs on, and a
    /// recipe only names the values it changes.
    fn default_to(&mut self, p: u8, v: f32) {
        if !self.has(p) {
            self.slot.params.push((p, v));
        }
    }
}

const GAP: f32 = 20.0;
const KNOB: f32 = 40.0;
const PITCH_X: f32 = 66.0;
const PITCH_Y: f32 = KNOB + 26.0 + 12.0;
const PLOT_H: f32 = 110.0;
/// The rate the response curves are drawn at. A picture, not the audio: the
/// shape of a shelf at 90 Hz does not move between 44.1 and 96 kHz on a plot
/// that spans 20 Hz to 20 kHz.
const CURVE_SR: f32 = 48_000.0;
/// Vertical span of the EQ plot, in dB either side of unity.
const EQ_PLOT_DB: f32 = 18.0;

fn knob(
    ui: &mut Ui,
    at: Pos2,
    label: &str,
    access: &mut SlotAccess,
    p: u8,
    range: std::ops::RangeInclusive<f32>,
    fmt: impl Fn(f32) -> String,
) {
    let (lo, hi) = (*range.start(), *range.end());
    let value = access.get(p);
    let mut norm = ((value - lo) / (hi - lo)).clamp(0.0, 1.0);
    let old = norm;
    let cell = Rect::from_min_size(at, Vec2::new(widgets::KNOB_GROUP_W, KNOB + 26.0));
    let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(cell));
    cui.vertical_centered(|ui| {
        widgets::knob_fmt(ui, &mut norm, label, &fmt(value), KNOB, GOLD);
    });
    if (norm - old).abs() > 1e-6 {
        access.set(p, lo + norm * (hi - lo));
    }
}

/// A lamp with a word beside it, and the whole thing a switch.
fn lamp_switch(ui: &mut Ui, centre: Pos2, on: bool, label: &str, salt: impl std::hash::Hash) -> bool {
    theme::lamp(ui, centre, 4.0, on, GOLD_BRIGHT);
    ui.painter().text(
        centre + Vec2::new(10.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(9.0),
        if on { GOLD_BRIGHT } else { TEXT_DIM },
    );
    let hit = Rect::from_center_size(centre + Vec2::new(22.0, 0.0), Vec2::new(64.0, 18.0));
    ui.interact(hit, ui.id().with(salt), Sense::click()).clicked()
}

/// The dark inset every plot sits in, with a hairline at unity.
fn plot_frame(ui: &Ui, r: Rect) {
    ui.painter().rect_filled(r, 3.0, BG_DARK);
    ui.painter()
        .rect_stroke(r, 3.0, Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);
}

fn caption(ui: &Ui, at: Pos2, text: &str) {
    ui.painter().text(at, Align2::LEFT_TOP, text, FontId::proportional(10.0), TEXT_DIM);
}

fn ms(v: f32) -> String {
    format!("{:.0} ms", v * 1000.0)
}
fn pct(v: f32) -> String {
    format!("{:.0}%", v * 100.0)
}
fn db(v: f32) -> String {
    format!("{v:+.1} dB")
}
fn hz(v: f32) -> String {
    if v >= 1000.0 { format!("{:.1} kHz", v / 1000.0) } else { format!("{v:.0} Hz") }
}

/// The EQ's response, from the effect's own filters.
fn eq_plot(ui: &Ui, r: Rect, a: &SlotAccess) {
    plot_frame(ui, r);
    let mut eq = ParametricEqEffect::new(CURVE_SR);
    let mask = a.get(pid::EQ_ENABLE_MASK) as u32;
    for b in 0..4 {
        let (f, g, q) = (BAND_PIDS[b].0, BAND_PIDS[b].1, BAND_PIDS[b].2);
        eq.set_band_freq(b, a.get(f));
        eq.set_band_gain(b, a.get(g));
        eq.set_band_q(b, a.get(q));
        eq.set_band_enabled(b, mask & (1 << b) != 0);
    }

    // Log frequency across, dB up. Ticks where an ear counts: 100, 1k, 10k.
    let (f0, f1) = (20.0f32, 20_000.0f32);
    let x_of = |f: f32| r.left() + (f / f0).log10() / (f1 / f0).log10() * r.width();
    let y_of = |d: f32| r.center().y - (d / EQ_PLOT_DB).clamp(-1.0, 1.0) * (r.height() * 0.5 - 6.0);
    for f in [100.0, 1000.0, 10_000.0] {
        let x = x_of(f);
        ui.painter().line_segment([Pos2::new(x, r.top()), Pos2::new(x, r.bottom())], Stroke::new(1.0, BORDER.gamma_multiply(0.6)));
        ui.painter().text(Pos2::new(x + 3.0, r.bottom() - 2.0), Align2::LEFT_BOTTOM, hz(f), FontId::proportional(8.0), TEXT_DIM);
    }
    ui.painter().line_segment([Pos2::new(r.left(), y_of(0.0)), Pos2::new(r.right(), y_of(0.0))], Stroke::new(1.0, BORDER));

    let n = r.width() as usize;
    let mut line = Vec::with_capacity(n + 1);
    let mut area = Vec::with_capacity(n + 3);
    area.push(Pos2::new(r.left(), y_of(0.0)));
    for i in 0..=n {
        let f = f0 * (f1 / f0).powf(i as f32 / n as f32);
        let d = 20.0 * eq.magnitude_at(f).max(1e-6).log10();
        let p = Pos2::new(x_of(f), y_of(d));
        line.push(p);
        area.push(p);
    }
    area.push(Pos2::new(r.right(), y_of(0.0)));
    theme::fill_polygon(ui, &area, GOLD.gamma_multiply(0.18), GOLD.gamma_multiply(0.06));
    ui.painter().add(Shape::line(line, Stroke::new(1.5, GOLD)));
}

/// A transfer curve, input dB across and output dB up, for the dynamics
/// slots. `out` maps an input level to the level that leaves.
fn transfer_plot(ui: &Ui, r: Rect, out: impl Fn(f32) -> f32) {
    plot_frame(ui, r);
    let span = 60.0f32;
    let x_of = |d: f32| r.left() + (d + span) / span * r.width();
    let y_of = |d: f32| r.bottom() - (d + span) / span * r.height();
    // Unity, faint: what the signal would do if the slot did nothing.
    ui.painter().line_segment([Pos2::new(x_of(-span), y_of(-span)), Pos2::new(x_of(0.0), y_of(0.0))], Stroke::new(1.0, BORDER));
    for d in [-40.0, -20.0] {
        ui.painter().line_segment([Pos2::new(x_of(d), r.top()), Pos2::new(x_of(d), r.bottom())], Stroke::new(1.0, BORDER.gamma_multiply(0.5)));
        ui.painter().text(Pos2::new(x_of(d) + 3.0, r.bottom() - 2.0), Align2::LEFT_BOTTOM, format!("{d:.0}"), FontId::proportional(8.0), TEXT_DIM);
    }
    let n = r.width() as usize;
    let line: Vec<Pos2> = (0..=n)
        .map(|i| {
            let din = -span + span * i as f32 / n as f32;
            Pos2::new(x_of(din), y_of(out(din).clamp(-span, 0.0)))
        })
        .collect();
    ui.painter().add(Shape::line(line, Stroke::new(1.5, GOLD)));
}

/// The four bands' pids, in band order: freq, gain, q.
const BAND_PIDS: [(u8, u8, u8); 4] = [
    (pid::EQ_B0_FREQ, pid::EQ_B0_GAIN, pid::EQ_B0_Q),
    (pid::EQ_B1_FREQ, pid::EQ_B1_GAIN, pid::EQ_B1_Q),
    (pid::EQ_B2_FREQ, pid::EQ_B2_GAIN, pid::EQ_B2_Q),
    (pid::EQ_B3_FREQ, pid::EQ_B3_GAIN, pid::EQ_B3_Q),
];
/// What each band is under filter type AUTO, which the recipe never changes.
const BAND_NAMES: [&str; 4] = ["LOW\nSHELF", "PEAK 1", "PEAK 2", "HIGH\nSHELF"];

/// Draw the page. Returns true when anything was moved, so the caller knows
/// to publish the chain.
pub fn draw(ui: &mut Ui, r: Rect, spec: &mut FxChainSpec, state: &mut FxPageState) -> bool {
    let live: Vec<usize> = spec
        .slots
        .iter()
        .enumerate()
        .filter(|(_, s)| EffectType::from_index(s.effect_type as usize) != EffectType::None)
        .map(|(i, _)| i)
        .collect();
    if live.is_empty() {
        theme::engraved(ui, r.center(), "this patch carries no effects", FontId::proportional(12.0), TEXT_DIM, Align2::CENTER_CENTER);
        return false;
    }

    let mut changed = false;
    let col_w = (r.width() - GAP * (live.len() as f32 - 1.0)) / live.len() as f32;

    for (n, &i) in live.iter().enumerate() {
        let col = Rect::from_min_size(
            Pos2::new(r.left() + n as f32 * (col_w + GAP), r.top()),
            Vec2::new(col_w, r.height()),
        );
        let slot = &mut spec.slots[i];
        let ft = EffectType::from_index(slot.effect_type as usize);
        let mut y = col.top();

        theme::cluster_header(ui, Rect::from_min_size(Pos2::new(col.left(), y), Vec2::new(col.width(), 20.0)), &ft.display_name().to_uppercase());
        if lamp_switch(ui, Pos2::new(col.right() - 44.0, y + 10.0), slot.enabled, if slot.enabled { "ON" } else { "OFF" }, ("fx_on", i)) {
            slot.enabled = !slot.enabled;
            changed = true;
        }
        y += 30.0;

        let mut a = SlotAccess { slot, touched: false };
        let x0 = col.left() + 4.0;
        let plot = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(col.width() - 8.0, PLOT_H));

        match ft {
            EffectType::ParametricEq => {
                // Bands the recipe left alone still have values -- the
                // effect's own -- and the page shows those, not zeros.
                let fresh = ParametricEqEffect::new(CURVE_SR);
                for b in 0..4 {
                    a.default_to(BAND_PIDS[b].0, fresh.band_freq(b));
                    a.default_to(BAND_PIDS[b].1, fresh.band_gain(b));
                    a.default_to(BAND_PIDS[b].2, fresh.band_q(b));
                }
                a.default_to(pid::EQ_ENABLE_MASK, 1.0);
                a.default_to(pid::EQ_MIX, 1.0);

                eq_plot(ui, plot, &a);
                y += PLOT_H + 12.0;

                let mut mask = a.get(pid::EQ_ENABLE_MASK) as u32;
                for b in 0..4 {
                    let row_y = y + b as f32 * PITCH_Y;
                    let on = mask & (1 << b) != 0;
                    let c = Pos2::new(x0 + 8.0, row_y + 14.0);
                    theme::lamp(ui, c, 4.0, on, GOLD_BRIGHT);
                    ui.painter().text(Pos2::new(x0 + 2.0, row_y + 26.0), Align2::LEFT_TOP, BAND_NAMES[b], FontId::proportional(8.0), if on { GOLD } else { TEXT_DIM });
                    let hit = Rect::from_min_size(Pos2::new(x0, row_y), Vec2::new(48.0, KNOB + 26.0));
                    if ui.interact(hit, ui.id().with(("eq_band", b)), Sense::click()).clicked() {
                        mask ^= 1 << b;
                        a.set(pid::EQ_ENABLE_MASK, mask as f32);
                    }
                    let (f, g, q) = BAND_PIDS[b];
                    let at = |c: usize| Pos2::new(x0 + 50.0 + c as f32 * PITCH_X, row_y);
                    knob(ui, at(0), "Freq", &mut a, f, 20.0..=20_000.0, hz);
                    knob(ui, at(1), "Gain", &mut a, g, -18.0..=18.0, db);
                    knob(ui, at(2), "Q", &mut a, q, 0.1..=10.0, |v| format!("{v:.2}"));
                }
                y += 4.0 * PITCH_Y;
                caption(ui, Pos2::new(x0, y), "the shelf takes back what a modelled\nstring radiates below the soundboard");
            }
            EffectType::Compressor => {
                a.default_to(pid::COMP_MAKEUP, 0.0);
                let (t, ratio, makeup) = (a.get(pid::COMP_THRESH), a.get(pid::COMP_RATIO).max(1.0), a.get(pid::COMP_MAKEUP));
                // Bus mode, which this slot runs in: a hard knee.
                transfer_plot(ui, plot, |din| if din > t { t + (din - t) / ratio } else { din } + makeup);
                y += PLOT_H + 12.0;
                let at = |c: usize, row: usize| Pos2::new(x0 + c as f32 * PITCH_X, y + row as f32 * PITCH_Y);
                knob(ui, at(0, 0), "Thresh", &mut a, pid::COMP_THRESH, -60.0..=0.0, db);
                knob(ui, at(1, 0), "Ratio", &mut a, pid::COMP_RATIO, 1.0..=20.0, |v| format!("{v:.1}:1"));
                knob(ui, at(2, 0), "Makeup", &mut a, pid::COMP_MAKEUP, 0.0..=24.0, db);
                knob(ui, at(3, 0), "Mix", &mut a, pid::COMP_MIX, 0.0..=1.0, pct);
                knob(ui, at(0, 1), "Attack", &mut a, pid::COMP_ATTACK, 0.0001..=0.5, ms);
                knob(ui, at(1, 1), "Release", &mut a, pid::COMP_RELEASE, 0.01..=2.0, ms);
                y += 2.0 * PITCH_Y;
                caption(ui, Pos2::new(x0, y), "slow glue across the whole instrument,\nhard knee, no lookahead");
            }
            EffectType::Reverb => {
                // The type is a choice, not a quantity: a list, not a dial.
                let names: Vec<NamedPreset> = ReverbType::all().iter().map(|t| NamedPreset { name: t.display_name(), category: None }).collect();
                let idx = (a.get(pid::REVERB_TYPE).max(0.0) as usize).min(names.len() - 1);
                state.reverb_picker.current_idx = idx;
                let strip = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(col.width() - 8.0, 26.0));
                let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(strip));
                if let Some(new_idx) = picker_ui(&mut cui, &PresetPickerStyle { salt: "cordis_reverb_type", accent: GOLD, dim: TEXT_DIM }, &mut state.reverb_picker, &names) {
                    a.set(pid::REVERB_TYPE, new_idx as f32);
                }
                y += 40.0;
                let at = |c: usize, row: usize| Pos2::new(x0 + c as f32 * PITCH_X, y + row as f32 * PITCH_Y);
                knob(ui, at(0, 0), "Size", &mut a, pid::REVERB_SIZE, 0.0..=1.0, pct);
                knob(ui, at(1, 0), "Decay", &mut a, pid::REVERB_DECAY, 0.0..=1.0, pct);
                knob(ui, at(2, 0), "Damp", &mut a, pid::REVERB_DAMP, 0.0..=1.0, pct);
                knob(ui, at(3, 0), "Width", &mut a, pid::REVERB_WIDTH, 0.0..=1.0, pct);
                knob(ui, at(0, 1), "Pre", &mut a, pid::REVERB_PREDELAY, 0.0..=0.2, ms);
                knob(ui, at(1, 1), "Mix", &mut a, pid::REVERB_MIX, 0.0..=1.0, pct);
                y += 2.0 * PITCH_Y;
                caption(ui, Pos2::new(x0, y), "the room past the microphones: the\nmodel has no walls of its own");
            }
            EffectType::BrickwallLimiter => {
                let ceiling = a.get(pid::BRICK_CEILING_DB);
                transfer_plot(ui, plot, |din| din.min(ceiling));
                y += PLOT_H + 12.0;
                let at = |c: usize| Pos2::new(x0 + c as f32 * PITCH_X, y);
                knob(ui, at(0), "Ceiling", &mut a, pid::BRICK_CEILING_DB, -12.0..=0.0, db);
                knob(ui, at(1), "Release", &mut a, pid::BRICK_RELEASE_MS, 5.0..=1000.0, |v| format!("{v:.0} ms"));
                knob(ui, at(2), "Mix", &mut a, pid::BRICK_MIX, 0.0..=1.0, pct);
                y += PITCH_Y;
                caption(ui, Pos2::new(x0, y), "nothing leaves above the ceiling,\nwhatever the three slots before it add");
            }
            _ => {}
        }
        changed |= a.touched;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slot_reads_and_writes_through_the_access() {
        let mut slot = FxSlotSpec {
            effect_type: EffectType::Reverb.index() as u8,
            enabled: true,
            mix: 1.0,
            params: vec![(pid::REVERB_MIX, 0.22)],
            ..Default::default()
        };
        let mut a = SlotAccess { slot: &mut slot, touched: false };
        assert_eq!(a.get(pid::REVERB_MIX), 0.22);
        assert_eq!(a.get(pid::REVERB_SIZE), 0.0);
        a.set(pid::REVERB_MIX, 0.22);
        assert!(!a.touched);
        a.set(pid::REVERB_MIX, 0.40);
        assert!(a.touched);
        a.set(pid::REVERB_SIZE, 0.5);
        assert_eq!(slot.params.len(), 2);
    }

    /// Filling in an effect's own default is not an edit: it must not raise
    /// the flag that republishes the chain, or every frame would.
    #[test]
    fn materialising_a_default_is_not_an_edit() {
        let mut slot = FxSlotSpec { effect_type: EffectType::ParametricEq.index() as u8, ..Default::default() };
        let mut a = SlotAccess { slot: &mut slot, touched: false };
        a.default_to(pid::EQ_B1_FREQ, 1000.0);
        assert_eq!(a.get(pid::EQ_B1_FREQ), 1000.0);
        assert!(!a.touched);
        a.default_to(pid::EQ_B1_FREQ, 5.0);
        assert_eq!(a.get(pid::EQ_B1_FREQ), 1000.0, "a present value is never overwritten");
    }

    #[test]
    fn the_reverb_type_index_round_trips() {
        for (i, t) in ReverbType::all().iter().enumerate() {
            assert_eq!(ReverbType::from_index(i).index(), i);
            assert_eq!(t.index(), i);
        }
    }
}
