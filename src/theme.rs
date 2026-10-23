use eframe::egui::{
    self, Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin,
    Shadow, Stroke, Style, TextStyle, Visuals,
};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub card: Color32,
    pub text: Color32,
    pub secondary: Color32,
    pub tertiary: Color32,
    pub line: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_soft: Color32,
    pub good: Color32,
    pub hard: Color32,
    pub again: Color32,
    pub easy: Color32,
    pub selected: Color32,
    pub hover: Color32,
    pub field: Color32,
    pub shadow: Color32,
    pub on_accent: Color32,
    pub again_soft: Color32,
    pub hard_soft: Color32,
    pub good_soft: Color32,
    pub easy_soft: Color32,
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            dark: true,
            bg: Color32::from_rgb(28, 28, 30),
            card: Color32::from_rgb(44, 44, 46),
            text: Color32::from_rgb(245, 245, 247),
            secondary: Color32::from_rgb(152, 152, 157),
            tertiary: Color32::from_rgb(124, 124, 128),
            line: Color32::from_rgb(58, 58, 60),
            accent: Color32::from_rgb(10, 132, 255),
            accent_hover: Color32::from_rgb(64, 156, 255),
            accent_soft: Color32::from_rgb(28, 58, 92),
            good: Color32::from_rgb(48, 209, 88),
            hard: Color32::from_rgb(255, 159, 10),
            again: Color32::from_rgb(255, 69, 58),
            easy: Color32::from_rgb(10, 132, 255),
            selected: Color32::from_rgb(64, 64, 68),
            hover: Color32::from_rgb(54, 54, 58),
            field: Color32::from_rgb(36, 36, 38),
            shadow: Color32::from_black_alpha(110),
            on_accent: Color32::WHITE,
            again_soft: Color32::from_rgb(72, 36, 34),
            hard_soft: Color32::from_rgb(74, 52, 24),
            good_soft: Color32::from_rgb(28, 64, 40),
            easy_soft: Color32::from_rgb(28, 52, 84),
        }
    } else {
        Palette {
            dark: false,
            bg: Color32::from_rgb(245, 245, 247),
            card: Color32::WHITE,
            text: Color32::from_rgb(29, 29, 31),
            secondary: Color32::from_rgb(110, 110, 115),
            tertiary: Color32::from_rgb(142, 142, 147),
            line: Color32::from_rgb(229, 229, 234),
            accent: Color32::from_rgb(0, 122, 255),
            accent_hover: Color32::from_rgb(0, 113, 227),
            accent_soft: Color32::from_rgb(232, 242, 255),
            good: Color32::from_rgb(52, 199, 89),
            hard: Color32::from_rgb(255, 149, 0),
            again: Color32::from_rgb(255, 59, 48),
            easy: Color32::from_rgb(0, 122, 255),
            selected: Color32::WHITE,
            hover: Color32::from_rgb(232, 232, 237),
            field: Color32::WHITE,
            shadow: Color32::from_black_alpha(28),
            on_accent: Color32::WHITE,
            again_soft: Color32::from_rgb(255, 236, 235),
            hard_soft: Color32::from_rgb(255, 244, 229),
            good_soft: Color32::from_rgb(232, 248, 237),
            easy_soft: Color32::from_rgb(232, 242, 255),
        }
    }
}

const DECK_COLORS: [[u8; 3]; 8] = [
    [0, 122, 255],
    [52, 199, 89],
    [255, 149, 0],
    [255, 59, 48],
    [175, 82, 222],
    [90, 200, 250],
    [255, 45, 85],
    [255, 204, 0],
];

pub fn deck_color(index: u8) -> Color32 {
    let rgb = DECK_COLORS[index as usize % DECK_COLORS.len()];
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

pub fn semi(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("Semibold".into()))
}

pub fn body(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgba_unmultiplied(
        lerp_u8(a.r(), b.r(), t),
        lerp_u8(a.g(), b.g(), t),
        lerp_u8(a.b(), b.b(), t),
        lerp_u8(a.a(), b.a(), t),
    )
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round().clamp(0.0, 255.0) as u8
}

pub fn soft_shadow() -> Shadow {
    Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(22),
    }
}

pub fn install_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    let regular = std::fs::read(r"C:\Windows\Fonts\segoeui.ttf").ok();
    let semibold = std::fs::read(r"C:\Windows\Fonts\seguisb.ttf").ok();
    let has_regular = regular.is_some();
    if let Some(bytes) = regular {
        fonts
            .font_data
            .insert("Segoe".into(), Arc::new(FontData::from_owned(bytes)));
        if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
            family.insert(0, "Segoe".into());
        }
    }
    let fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    if let Some(bytes) = semibold {
        fonts.font_data.insert(
            "SegoeSemibold".into(),
            Arc::new(FontData::from_owned(bytes)),
        );
        let mut family = vec!["SegoeSemibold".into()];
        if has_regular {
            family.push("Segoe".into());
        }
        family.extend(fallback);
        fonts
            .families
            .insert(FontFamily::Name("Semibold".into()), family);
    } else {
        fonts
            .families
            .insert(FontFamily::Name("Semibold".into()), fallback);
    }
    ctx.set_fonts(fonts);
}

pub fn apply(ctx: &Context, palette: &Palette) {
    let mut visuals = if palette.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    visuals.window_fill = palette.bg;
    visuals.panel_fill = palette.bg;
    visuals.faint_bg_color = palette.hover;
    visuals.extreme_bg_color = palette.field;
    visuals.code_bg_color = palette.field;
    visuals.override_text_color = Some(palette.text);
    visuals.hyperlink_color = palette.accent;
    visuals.selection.bg_fill = mix(palette.accent, palette.card, 0.82);
    visuals.selection.stroke = Stroke::new(1.0, palette.accent);
    visuals.warn_fg_color = palette.hard;
    visuals.error_fg_color = palette.again;
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.menu_corner_radius = CornerRadius::same(10);
    visuals.window_stroke = Stroke::new(1.0, palette.line);
    visuals.window_shadow = soft_shadow();
    visuals.popup_shadow = soft_shadow();
    let radius = CornerRadius::same(8);
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = radius;
    }
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.line);
    visuals.widgets.noninteractive.bg_fill = palette.card;
    visuals.widgets.inactive.bg_fill = palette.field;
    visuals.widgets.inactive.weak_bg_fill = palette.field;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, palette.line);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.hovered.bg_fill = palette.field;
    visuals.widgets.hovered.weak_bg_fill = palette.hover;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, palette.accent);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.active.bg_fill = palette.field;
    visuals.widgets.active.weak_bg_fill = palette.accent_soft;
    visuals.widgets.active.bg_stroke = Stroke::new(1.5, palette.accent);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, palette.text);

    let mut style = Style {
        visuals: visuals.clone(),
        ..(*ctx.global_style()).clone()
    };
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.window_margin = Margin::ZERO;
    style.text_styles.insert(TextStyle::Heading, semi(26.0));
    style.text_styles.insert(TextStyle::Body, body(15.0));
    style.text_styles.insert(TextStyle::Button, semi(14.0));
    style.text_styles.insert(TextStyle::Small, body(12.0));
    style
        .text_styles
        .insert(TextStyle::Monospace, FontId::monospace(13.0));
    ctx.set_global_style(style);
}
