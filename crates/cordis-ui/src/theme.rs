//! The instrument's look: lacquer, brass and engraved type.
//!
//! Everything here is painted rather than loaded. There is no image asset in
//! this crate, because every surface the editor draws is also a surface it has
//! to reason about: the microphones sit at a computed point on the soundboard,
//! a sounding string lights along its own line. A bitmap would duplicate that
//! geometry and drift from it.

use egui::{Align2, Color32, FontFamily, FontId, Mesh, Pos2, Rect, Shape, Stroke, Ui, Vec2};

use crate::colors::*;

/// The display face, used for the wordmark, cluster headings and the nameplate.
///
/// Body text stays on egui's default sans: a serif at 9 pt is harder to read
/// than it is handsome, and the caption under a knob has to be read.
pub const DISPLAY: &str = "display";

/// Register the display face. Idempotent, but call it once — `set_fonts`
/// rebuilds every atlas.
///
/// Falls back silently to the default proportional family when the file is
/// absent, and `display_font()` then returns that instead, so every call site
/// keeps working with slightly plainer letters.
pub fn install_fonts(ctx: &egui::Context) {
    const TTF: &[u8] = include_bytes!("../assets/NotoSerifDisplay-Regular.ttf");

    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        DISPLAY.to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(TTF)),
    );
    fonts
        .families
        .entry(FontFamily::Name(DISPLAY.into()))
        .or_default()
        .insert(0, DISPLAY.to_owned());
    ctx.set_fonts(fonts);
}

/// A font id in the display face at `size`, or the default proportional face
/// when that family is not bound.
///
/// The check is not defensive padding: `set_fonts` only takes effect on the
/// pass *after* it is called, so the very first frame of an editor asks for a
/// family that does not exist yet, and epaint panics rather than falling back.
pub fn display_font(ctx: &egui::Context, size: f32) -> FontId {
    let bound = ctx.fonts(|f| {
        f.families()
            .iter()
            .any(|fam| matches!(fam, FontFamily::Name(n) if &**n == DISPLAY))
    });
    if bound {
        FontId::new(size, FontFamily::Name(DISPLAY.into()))
    } else {
        FontId::proportional(size)
    }
}

/// Dark, warm, and low-contrast enough that the brass reads as the bright
/// thing on the panel.
pub fn apply_visuals(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG_LACQUER;
    v.window_fill = BG_LACQUER;
    v.extreme_bg_color = BG_DARK;
    v.widgets.noninteractive.bg_fill = BG_PANEL;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_DIM);
    v.widgets.inactive.bg_fill = BG_RAISED;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_DIM);
    v.widgets.hovered.bg_fill = BG_RAISED;
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, GOLD.gamma_multiply(0.5));
    v.widgets.active.bg_fill = BG_RAISED;
    v.widgets.active.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    v.widgets.active.bg_stroke = Stroke::new(1.0, GOLD);
    v.selection.bg_fill = GOLD.gamma_multiply(0.25);
    v.selection.stroke = Stroke::new(1.0, GOLD);
    v.window_stroke = Stroke::new(1.0, BORDER);
    ctx.set_visuals(v);
}

/// A vertical two-stop gradient, as one mesh quad.
///
/// The only way to get a gradient out of egui: four vertices, a colour each,
/// two triangles. The same shape an envelope display uses to fill under
/// its curve.
pub fn gradient_v(ui: &Ui, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    ui.painter().add(Shape::mesh(mesh));
}

/// Fill a closed outline with a vertical gradient, as a triangle fan from its
/// centroid.
///
/// `Shape::convex_polygon` is the obvious call and the wrong one: a grand's
/// outline is concave along the bentside, and epaint fills it as though it were
/// not, spilling wood outside the rim. A fan is correct for any shape that can
/// see its own centroid, which this one can, and it carries a colour per vertex
/// so the fill and the gradient are the same pass.
pub fn fill_polygon(ui: &Ui, pts: &[Pos2], top: Color32, bottom: Color32) {
    if pts.len() < 3 {
        return;
    }
    let n = pts.len() as f32;
    let c = Pos2::new(
        pts.iter().map(|p| p.x).sum::<f32>() / n,
        pts.iter().map(|p| p.y).sum::<f32>() / n,
    );
    let y0 = pts.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    let y1 = pts.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    let shade = |p: Pos2| {
        let t = if y1 > y0 { (p.y - y0) / (y1 - y0) } else { 0.0 };
        Color32::from_rgba_premultiplied(
            (top.r() as f32 + (bottom.r() as f32 - top.r() as f32) * t) as u8,
            (top.g() as f32 + (bottom.g() as f32 - top.g() as f32) * t) as u8,
            (top.b() as f32 + (bottom.b() as f32 - top.b() as f32) * t) as u8,
            (top.a() as f32 + (bottom.a() as f32 - top.a() as f32) * t) as u8,
        )
    };

    let mut mesh = Mesh::default();
    mesh.colored_vertex(c, shade(c));
    for &p in pts {
        mesh.colored_vertex(p, shade(p));
    }
    for i in 1..pts.len() as u32 {
        mesh.add_triangle(0, i, i + 1);
    }
    mesh.add_triangle(0, pts.len() as u32, 1);
    ui.painter().add(Shape::mesh(mesh));
}

/// Three stops, for the nameplate: dark edge, bright face, dark edge again.
/// That is what makes a flat rectangle read as rolled brass.
pub fn gradient_plate(ui: &Ui, rect: Rect) {
    let mid = Rect::from_min_max(
        Pos2::new(rect.left(), rect.center().y - rect.height() * 0.5),
        Pos2::new(rect.right(), rect.center().y),
    );
    let low = Rect::from_min_max(mid.left_bottom(), rect.right_bottom());
    gradient_v(ui, mid, PLATE_EDGE, PLATE_FACE);
    gradient_v(ui, low, PLATE_FACE, PLATE_EDGE);
}

/// Text cut INTO a surface: a light line below, the letters above it.
///
/// Reads as engraved because the eye takes the pale offset for the far wall of
/// the groove catching the light.
pub fn engraved(
    ui: &Ui,
    pos: Pos2,
    text: &str,
    font: FontId,
    ink: Color32,
    anchor: Align2,
) {
    let lip = Color32::from_rgba_unmultiplied(255, 255, 255, 40);
    ui.painter()
        .text(pos + Vec2::new(0.0, 1.0), anchor, text, font.clone(), lip);
    ui.painter().text(pos, anchor, text, font, ink);
}

/// Text raised OFF a dark surface: a black line below, gold above.
pub fn embossed(ui: &Ui, pos: Pos2, text: &str, font: FontId, ink: Color32, anchor: Align2) {
    ui.painter().text(
        pos + Vec2::new(0.0, 1.0),
        anchor,
        text,
        font.clone(),
        Color32::from_rgb(8, 8, 8),
    );
    ui.painter().text(pos, anchor, text, font, ink);
}

/// Letters spaced apart, painted one at a time.
///
/// egui has no letter-spacing, and a wordmark without it looks like a label.
/// Returns the total width so a caller can centre or underline it.
pub fn tracked_text(
    ui: &Ui,
    left: Pos2,
    text: &str,
    font: FontId,
    ink: Color32,
    tracking: f32,
    emboss: bool,
) -> f32 {
    let mut x = left.x;
    for ch in text.chars() {
        let s = ch.to_string();
        let w = ui.painter().layout_no_wrap(s.clone(), font.clone(), ink).size().x;
        let at = Pos2::new(x, left.y);
        if emboss {
            embossed(ui, at, &s, font.clone(), ink, Align2::LEFT_CENTER);
        } else {
            ui.painter()
                .text(at, Align2::LEFT_CENTER, &s, font.clone(), ink);
        }
        x += w + tracking;
    }
    (x - tracking) - left.x
}

/// A lamp: halo, body, and the small white glint that sells it as glass.
pub fn lamp(ui: &Ui, centre: Pos2, radius: f32, on: bool, tint: Color32) {
    let p = ui.painter();
    if on {
        p.circle_filled(centre, radius * 2.2, tint.gamma_multiply(0.20));
        p.circle_filled(centre, radius * 1.5, tint.gamma_multiply(0.35));
    }
    let body = if on { tint } else { BG_DARK };
    p.circle_filled(centre, radius, body);
    p.circle_stroke(centre, radius, Stroke::new(1.0, BORDER));
    if on {
        p.circle_filled(
            centre - Vec2::splat(radius * 0.35),
            radius * 0.28,
            Color32::from_rgba_unmultiplied(255, 255, 255, 150),
        );
    }
}

/// A cluster heading: the display face, tracked, in gold, over a hairline.
pub fn cluster_header(ui: &Ui, rect: Rect, title: &str) {
    let baseline = Pos2::new(rect.left(), rect.top() + 7.0);
    let w = tracked_text(ui, baseline, title, display_font(ui.ctx(), 11.0), GOLD, 1.6, false);
    ui.painter().line_segment(
        [
            Pos2::new(rect.left(), baseline.y + 9.0),
            Pos2::new(rect.left() + w.max(40.0), baseline.y + 9.0),
        ],
        Stroke::new(1.0, BORDER),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The palette has to stay dark enough for brass to be the bright thing.
    /// A regression here means someone lightened the lacquer and the whole
    /// instrument goes flat.
    #[test]
    fn the_lacquer_is_darker_than_the_brass() {
        let lum = |c: Color32| {
            0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32
        };
        assert!(lum(BG_LACQUER) < 30.0, "the lacquer is not dark");
        assert!(lum(GOLD) > 3.0 * lum(BG_LACQUER), "brass does not stand out");
        assert!(lum(LACQUER_TOP) > lum(LACQUER_BOTTOM), "the sheen is upside down");
    }

    /// The microphone colours and the meter needles are one idea; if they ever
    /// diverge the stereo metaphor breaks silently.
    #[test]
    fn the_microphones_and_the_needles_agree() {
        assert_eq!(MIC_LEFT, Color32::from_rgb(212, 175, 55));
        assert_eq!(MIC_RIGHT, Color32::from_rgb(192, 196, 204));
        assert_ne!(MIC_LEFT, MIC_RIGHT, "left and right must be tellable apart");
    }
}
