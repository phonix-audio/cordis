//! One effect's parameters, drawn from its `EffectSpec`.

use egui::{Align2, FontId, Pos2, Rect, Sense, Ui, Vec2};
use phonix_fx::{Curve, EffectSpec, ParamFlags, ParamKind, ParamSpec, SlotSpec, SpecValue, Value};

use super::RackStyle;
use crate::preset_picker::{picker_ui, NamedPreset, PresetPickerState, PresetPickerStyle};
use crate::theme::{lamp, Palette};
use crate::widgets;

/// A slot's written parameters read against what its kind declares: an id
/// the slot never wrote reads as the kind's default, and writing it adds it.
pub struct SlotAccess<'a> {
    slot: &'a mut SlotSpec,
    spec: &'static EffectSpec,
    touched: bool,
}

impl<'a> SlotAccess<'a> {
    pub fn new(slot: &'a mut SlotSpec, spec: &'static EffectSpec) -> Self {
        SlotAccess { slot, spec, touched: false }
    }

    pub fn spec(&self) -> &'static EffectSpec {
        self.spec
    }

    pub fn slot(&mut self) -> &mut SlotSpec {
        self.slot
    }

    /// Whether a write changed anything since construction.
    pub fn touched(&self) -> bool {
        self.touched
    }

    pub fn value(&self, p: &ParamSpec) -> Value {
        match self.slot.get(p.id) {
            Some(v) => v.resolve(p).0,
            None => p.default,
        }
    }

    pub fn set(&mut self, p: &ParamSpec, value: impl Into<SpecValue>) {
        let value = value.into();
        if self.slot.get(p.id) != Some(&value) {
            self.slot.set(p.id, value);
            self.touched = true;
        }
    }
}

/// Pickers for the enums with many variants, one per parameter index.
#[derive(Default)]
pub struct PanelState {
    pickers: Vec<PresetPickerState>,
}

impl PanelState {
    fn picker(&mut self, index: usize) -> &mut PresetPickerState {
        if self.pickers.len() <= index {
            self.pickers.resize_with(index + 1, Default::default);
        }
        &mut self.pickers[index]
    }
}

/// Up to this many variants, an enum is a row of pills; past it, a list.
const PILLS_UP_TO: usize = 5;
const CELL_H: f32 = 26.0;

fn to_norm(p: &ParamSpec, v: f32) -> f32 {
    match p.kind {
        ParamKind::Float { min, max, curve } => match curve {
            Curve::Log if min > 0.0 => ((v / min).ln() / (max / min).ln()).clamp(0.0, 1.0),
            Curve::Pow(k) => ((v - min) / (max - min)).clamp(0.0, 1.0).powf(1.0 / k),
            _ => ((v - min) / (max - min)).clamp(0.0, 1.0),
        },
        ParamKind::Int { min, max } => ((v - min as f32) / (max - min).max(1) as f32).clamp(0.0, 1.0),
        _ => 0.0,
    }
}

fn from_norm(p: &ParamSpec, n: f32) -> f32 {
    match p.kind {
        ParamKind::Float { min, max, curve } => match curve {
            Curve::Log if min > 0.0 => min * (max / min).powf(n),
            Curve::Pow(k) => min + n.powf(k) * (max - min),
            _ => min + n * (max - min),
        },
        ParamKind::Int { min, max } => (min as f32 + n * (max - min) as f32).round(),
        _ => 0.0,
    }
}

/// The mix knob every slot has, beside its parameters.
fn mix_knob(ui: &mut Ui, slot: &mut SlotSpec, style: &RackStyle) -> bool {
    let mut norm = slot.mix.clamp(0.0, 1.0);
    let old = norm;
    widgets::knob_cell_for(ui, widgets::KNOB_GROUP_W, style.knob, |ui| {
        widgets::knob_fmt(ui, &mut norm, "Mix", &format!("{:.0}%", slot.mix * 100.0), style.knob, style.accent);
    });
    if (norm - old).abs() > 1e-6 {
        slot.mix = norm;
        return true;
    }
    false
}

/// Draw every parameter the kind declares, in the order it declares them,
/// wrapped in rows. Returns true when the mix moved; parameter writes are
/// reported by the access.
pub fn draw_panel(ui: &mut Ui, access: &mut SlotAccess, style: &RackStyle, state: &mut PanelState, slot_index: usize) -> bool {
    let spec = access.spec();
    let pal = Palette::of(ui.ctx());
    let mut mix_moved = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 6.0);
        for (index, p) in spec.params.iter().enumerate() {
            if p.flags.contains(ParamFlags::READ_ONLY) {
                continue;
            }
            match p.kind {
                ParamKind::Float { .. } | ParamKind::Int { .. } => {
                    let value = access.value(p).as_f32();
                    let mut norm = to_norm(p, value);
                    let old = norm;
                    let text = match p.kind {
                        ParamKind::Int { .. } => format!("{}", value as i32),
                        _ => p.unit.format(value),
                    };
                    widgets::knob_cell_for(ui, widgets::KNOB_GROUP_W, style.knob, |ui| {
                        widgets::knob_fmt(ui, &mut norm, p.short, &text, style.knob, style.accent);
                    });
                    if (norm - old).abs() > 1e-6 {
                        let v = from_norm(p, norm);
                        match p.kind {
                            ParamKind::Int { .. } => access.set(p, v as i64),
                            _ => access.set(p, v),
                        }
                    }
                }
                ParamKind::Bool => {
                    let on = access.value(p).as_bool();
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(widgets::KNOB_GROUP_W, CELL_H), Sense::click());
                    let centre = Pos2::new(rect.left() + 8.0, rect.center().y);
                    lamp(ui, centre, 4.0, on, style.accent);
                    ui.painter().text(centre + Vec2::new(10.0, 0.0), Align2::LEFT_CENTER, p.short, FontId::proportional(9.0), if on { style.accent } else { pal.text_dim });
                    if resp.clicked() {
                        access.set(p, !on);
                    }
                }
                ParamKind::Enum { variants, labels } => {
                    let current = access.value(p).as_variant().unwrap_or("");
                    let idx = variants.iter().position(|v| *v == current).unwrap_or(0);
                    if variants.len() <= PILLS_UP_TO {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(p.short).color(pal.text_dim).size(9.0));
                            for (i, label) in labels.iter().enumerate() {
                                if widgets::selector_pill(ui, label, i == idx, style.accent).clicked() && i != idx {
                                    access.set(p, variants[i]);
                                }
                            }
                        });
                    } else {
                        let names: Vec<NamedPreset> = labels.iter().map(|l| NamedPreset { name: l, category: None }).collect();
                        let picker = state.picker(index);
                        picker.current_idx = idx;
                        let strip = ui.allocate_exact_size(Vec2::new(150.0, CELL_H), Sense::hover()).0;
                        let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(strip.min, strip.size())));
                        let salt = format!("{}_{}_{}", style.salt, slot_index, p.id);
                        if let Some(new_idx) = picker_ui(&mut cui, &PresetPickerStyle { salt: &salt, accent: style.accent, dim: style.dim }, picker, &names) {
                            if let Some(v) = variants.get(new_idx) {
                                access.set(p, *v);
                            }
                        }
                    }
                }
                ParamKind::Bits { .. } => {}
            }
        }
        mix_moved = mix_knob(ui, access.slot(), style);
    });
    mix_moved
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;
    use phonix_fx::Registry;

    /// Every float and int a kind declares is a knob the accessibility tree
    /// sees, for every kind this build ships.
    #[test]
    fn every_declared_number_is_a_knob() {
        for entry in Registry::builtin().entries() {
            let spec = entry.spec;
            let mut slot = SlotSpec::new(spec.kind);
            let mut state = PanelState::default();
            let mut harness = Harness::builder().with_size(egui::vec2(900.0, 400.0)).build_ui(|ui| {
                let mut access = SlotAccess::new(&mut slot, spec);
                let style = RackStyle { accent: egui::Color32::WHITE, dim: egui::Color32::GRAY, knob: 32.0, salt: "t" };
                draw_panel(ui, &mut access, &style, &mut state, 0);
            });
            harness.run_steps(2);
            for p in spec.params.iter().filter(|p| matches!(p.kind, ParamKind::Float { .. } | ParamKind::Int { .. })) {
                assert!(harness.query_by_label(p.short).is_some(), "{}: no knob for {:?}", spec.kind, p.id);
            }
            assert!(harness.query_by_label("Mix").is_some(), "{}: no mix knob", spec.kind);
        }
    }

    /// A value written by the panel lands in the slot under the spec's id,
    /// and an unwritten one reads as the kind's default.
    #[test]
    fn the_access_reads_defaults_and_writes_ids() {
        let reg = Registry::builtin();
        let entry = reg.get("delay").unwrap();
        let mut slot = SlotSpec::new("delay");
        let mut a = SlotAccess::new(&mut slot, entry.spec);
        let time = entry.spec.param("time").unwrap();
        assert_eq!(a.value(time), Value::F(0.3));
        a.set(time, 0.5_f32);
        assert!(a.touched());
        assert_eq!(slot.f32("time"), Some(0.5));
    }
}
