use crate::settings::{Rgba, Theme};
use eframe::egui::{self, Color32, FontId, Rounding, Stroke, TextStyle};

pub fn c(rgba: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3])
}

const fn hex(v: u32) -> Rgba {
    [(v >> 16) as u8, (v >> 8) as u8, v as u8, 255]
}

/// Linear blend between two colors (t = 0 → a, t = 1 → b).
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

#[allow(clippy::too_many_arguments)]
fn preset(
    name: &str,
    dark: bool,
    background: u32,
    panel: u32,
    text: u32,
    weak_text: u32,
    accent: u32,
    chart_secondary: u32,
) -> Theme {
    Theme {
        name: name.into(),
        dark,
        background: hex(background),
        panel: hex(panel),
        text: hex(text),
        weak_text: hex(weak_text),
        accent: hex(accent),
        chart_primary: hex(accent),
        chart_secondary: hex(chart_secondary),
        rounding: 8.0,
        font_size: 14.0,
        spacing: 1.0,
    }
}

pub fn presets() -> Vec<Theme> {
    vec![
        preset("Midnight", true, 0x14161b, 0x1d2027, 0xe6e8ee, 0x8a90a0, 0x5b9cff, 0xf0a35b),
        preset("Daylight", false, 0xf3f4f6, 0xffffff, 0x1d2128, 0x6b7280, 0x2563eb, 0xea580c),
        preset("Arctic", true, 0x2e3440, 0x3b4252, 0xeceff4, 0xa3acbd, 0x88c0d0, 0xebcb8b),
        preset("Violet Night", true, 0x1e1b2e, 0x2a2640, 0xf0edff, 0x9d97b8, 0xb48cff, 0xff79a8),
        preset("Forest", true, 0x15201a, 0x1e2c24, 0xe3efe6, 0x8ba796, 0x4ade80, 0xfacc15),
        preset("Sand", false, 0xfdf6e3, 0xfffbef, 0x3b3428, 0x857a64, 0xb5651d, 0x268bd2),
        preset("Ocean", true, 0x0b1d2a, 0x12293a, 0xe0f2fe, 0x7fa3bb, 0x38bdf8, 0xf472b6),
        preset("Ember", true, 0x1c1412, 0x2a1e1a, 0xf5e9e2, 0xa8928a, 0xff7a45, 0xffd166),
        preset("High Contrast", true, 0x000000, 0x111111, 0xffffff, 0xc8c8c8, 0xffff00, 0x00ffff),
    ]
}

pub fn apply(ctx: &egui::Context, t: &Theme) {
    let bg = c(t.background);
    let panel = c(t.panel);
    let text = c(t.text);
    let accent = c(t.accent);
    let border = mix(panel, text, 0.14);
    let rounding = Rounding::same(t.rounding);

    let mut v = if t.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.dark_mode = t.dark;
    v.override_text_color = None;
    v.panel_fill = bg;
    v.window_fill = panel;
    v.extreme_bg_color =
        mix(bg, if t.dark { Color32::BLACK } else { Color32::WHITE }, 0.35);
    v.faint_bg_color = mix(bg, panel, 0.6);
    v.code_bg_color = panel;
    v.hyperlink_color = accent;
    v.selection.bg_fill = mix(panel, accent, 0.55);
    v.selection.stroke = Stroke::new(1.0_f32, text);
    v.window_rounding = rounding;
    v.menu_rounding = rounding;
    v.window_stroke = Stroke::new(1.0_f32, border);

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = panel;
    w.noninteractive.weak_bg_fill = panel;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, border);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, text);
    w.noninteractive.rounding = rounding;

    w.inactive.bg_fill = mix(panel, text, 0.10);
    w.inactive.weak_bg_fill = mix(panel, text, 0.08);
    w.inactive.bg_stroke = Stroke::new(1.0_f32, border);
    w.inactive.fg_stroke = Stroke::new(1.0_f32, text);
    w.inactive.rounding = rounding;

    w.hovered.bg_fill = mix(panel, accent, 0.30);
    w.hovered.weak_bg_fill = mix(panel, accent, 0.25);
    w.hovered.bg_stroke = Stroke::new(1.0_f32, accent);
    w.hovered.fg_stroke = Stroke::new(1.5_f32, text);
    w.hovered.rounding = rounding;

    w.active.bg_fill = mix(panel, accent, 0.50);
    w.active.weak_bg_fill = mix(panel, accent, 0.45);
    w.active.bg_stroke = Stroke::new(1.0_f32, accent);
    w.active.fg_stroke = Stroke::new(2.0_f32, text);
    w.active.rounding = rounding;

    w.open = w.active;

    let fs = t.font_size;
    let mut style = (*ctx.style()).clone();
    style.visuals = v;
    style.text_styles = [
        (TextStyle::Small, FontId::proportional(fs * 0.8)),
        (TextStyle::Body, FontId::proportional(fs)),
        (TextStyle::Button, FontId::proportional(fs)),
        (TextStyle::Heading, FontId::proportional(fs * 1.45)),
        (TextStyle::Monospace, FontId::monospace(fs * 0.95)),
    ]
    .into();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0) * t.spacing;
    style.spacing.button_padding = egui::vec2(8.0, 4.0) * t.spacing;

    // Use the same style regardless of the OS light/dark preference.
    ctx.set_style_of(egui::Theme::Dark, style.clone());
    ctx.set_style_of(egui::Theme::Light, style);
    ctx.set_theme(if t.dark { egui::Theme::Dark } else { egui::Theme::Light });
}
