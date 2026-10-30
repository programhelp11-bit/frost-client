//! Liquid-glass theme helpers shared by the launcher and the HUD renderer.
//!
//! egui cannot blur what is *behind* the window by itself. The effect is built from:
//! 1. a transparent native window (`ViewportBuilder::with_transparent`),
//! 2. OS-level backdrop blur: Acrylic on Windows (`window-vibrancy`), the compositor's
//!    blur-behind on Linux (KWin / Hyprland / Picom rules),
//! 3. translucent tinted fills, a diagonal sheen and specular edge highlights painted here.

use eframe::egui::{self, Color32, Frame, Painter, Pos2, Rect, Rounding, Stroke, Ui};

pub const ACCENT: Color32 = Color32::from_rgb(120, 200, 255);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(150, 190, 255);
pub const TEXT: Color32 = Color32::from_rgb(235, 242, 255);
pub const TEXT_DIM: Color32 = Color32::from_rgb(160, 175, 200);

/// Dark, mostly transparent visuals. Panels use `TRANSPARENT` so the backdrop shows through.
pub fn apply_theme(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = Color32::TRANSPARENT;
    v.window_fill = Color32::from_rgba_unmultiplied(16, 22, 36, 200);
    v.override_text_color = Some(TEXT);
    v.window_rounding = Rounding::same(18.0);
    v.window_stroke = Stroke::new(1.0, Color32::from_white_alpha(40));
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(120, 200, 255, 90);
    v.selection.stroke = Stroke::new(1.0, ACCENT);

    let rounding = Rounding::same(10.0);
    v.widgets.noninteractive.bg_fill = Color32::from_white_alpha(10);
    v.widgets.noninteractive.rounding = rounding;
    v.widgets.inactive.bg_fill = Color32::from_white_alpha(16);
    v.widgets.inactive.weak_bg_fill = Color32::from_white_alpha(16);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_white_alpha(34));
    v.widgets.inactive.rounding = rounding;
    v.widgets.hovered.bg_fill = Color32::from_white_alpha(34);
    v.widgets.hovered.weak_bg_fill = Color32::from_white_alpha(34);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_white_alpha(80));
    v.widgets.hovered.rounding = rounding;
    v.widgets.active.bg_fill = Color32::from_rgba_unmultiplied(120, 200, 255, 70);
    v.widgets.active.weak_bg_fill = Color32::from_rgba_unmultiplied(120, 200, 255, 70);
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.active.rounding = rounding;
    v.extreme_bg_color = Color32::from_rgba_unmultiplied(8, 12, 20, 160);
    ctx.set_visuals(v);
}

/// Enable native backdrop blur where the OS supports it.
#[allow(unused_variables)]
pub fn enable_backdrop_blur(cc: &eframe::CreationContext<'_>) {
    #[cfg(windows)]
    {
        // Acrylic needs Windows 10 1809+; Mica would be Windows 11 only.
        let _ = window_vibrancy::apply_acrylic(cc, Some((14, 18, 30, 110)));
    }
    // Linux: nothing to call – the compositor blurs transparent windows when the user enables
    // blur-behind for the "frost-launcher" window class (see docs/ARCHITECTURE.md).
}

/// Frame for a glass card. `alpha` is the tint opacity (0..=255).
pub fn glass_frame(alpha: u8, rounding: f32) -> Frame {
    Frame::none()
        .fill(Color32::from_rgba_unmultiplied(18, 26, 42, alpha))
        .stroke(Stroke::new(1.0, Color32::from_white_alpha(46)))
        .rounding(Rounding::same(rounding))
        .inner_margin(egui::Margin::same(14.0))
}

/// Paint sheen + specular edge on top of an already drawn glass card.
pub fn decorate(painter: &Painter, rect: Rect, rounding: f32) {
    // Upper-left sheen: a soft lighter band across the top third.
    let sheen = Rect::from_min_max(
        rect.min + egui::vec2(1.0, 1.0),
        Pos2::new(rect.max.x - 1.0, rect.min.y + rect.height() * 0.38),
    );
    painter.rect_filled(
        sheen,
        Rounding { nw: rounding, ne: rounding, sw: 0.0, se: 0.0 },
        Color32::from_white_alpha(9),
    );
    // Specular highlight along the top edge, fading towards the corners.
    let y = rect.min.y + 1.0;
    let steps = 24;
    let x0 = rect.min.x + rounding;
    let x1 = rect.max.x - rounding;
    if x1 > x0 {
        for i in 0..steps {
            let t0 = i as f32 / steps as f32;
            let t1 = (i + 1) as f32 / steps as f32;
            let mid = (t0 + t1) * 0.5;
            let a = (1.0 - (mid - 0.5).abs() * 2.0).clamp(0.0, 1.0);
            painter.line_segment(
                [Pos2::new(x0 + (x1 - x0) * t0, y), Pos2::new(x0 + (x1 - x0) * t1, y)],
                Stroke::new(1.2, Color32::from_white_alpha((20.0 + 90.0 * a) as u8)),
            );
        }
    }
}

/// Draw a glass card around `add`.
pub fn glass_panel<R>(
    ui: &mut Ui,
    alpha: u8,
    rounding: f32,
    add: impl FnOnce(&mut Ui) -> R,
) -> egui::InnerResponse<R> {
    let out = glass_frame(alpha, rounding).show(ui, add);
    decorate(ui.painter(), out.response.rect, rounding);
    out
}

/// Vector snowflake (six arms with branches). Used for the tab-list icon preview and logo.
pub fn paint_snowflake(painter: &Painter, center: Pos2, radius: f32, color: Color32) {
    let stroke = Stroke::new((radius * 0.12).max(1.0), color);
    for i in 0..6 {
        let ang = i as f32 * std::f32::consts::TAU / 6.0;
        let dir = egui::vec2(ang.cos(), ang.sin());
        let tip = center + dir * radius;
        painter.line_segment([center, tip], stroke);
        for (frac, len) in [(0.55_f32, 0.35_f32), (0.8, 0.22)] {
            let base = center + dir * radius * frac;
            for side in [-1.0_f32, 1.0] {
                let b_ang = ang + side * std::f32::consts::FRAC_PI_3;
                let b_dir = egui::vec2(b_ang.cos(), b_ang.sin());
                painter.line_segment([base, base + b_dir * radius * len], stroke);
            }
        }
    }
}
