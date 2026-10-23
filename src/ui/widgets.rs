use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Frame, Id, Margin, Pos2, Rect, Response, Sense,
    Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};

use crate::model::Rating;
use crate::theme::{self, Palette};

pub struct BtnStyle {
    pub fill: Color32,
    pub hover: Color32,
    pub text: Color32,
    pub stroke: Color32,
}

pub fn primary_style(palette: &Palette) -> BtnStyle {
    BtnStyle {
        fill: palette.accent,
        hover: palette.accent_hover,
        text: palette.on_accent,
        stroke: Color32::TRANSPARENT,
    }
}

pub fn quiet_style(palette: &Palette) -> BtnStyle {
    BtnStyle {
        fill: palette.card,
        hover: palette.hover,
        text: palette.text,
        stroke: palette.line,
    }
}

pub fn solid_style(color: Color32) -> BtnStyle {
    BtnStyle {
        fill: color,
        hover: theme::mix(color, Color32::WHITE, 0.14),
        text: Color32::WHITE,
        stroke: Color32::TRANSPARENT,
    }
}

pub fn soft_style(fill: Color32, text: Color32) -> BtnStyle {
    BtnStyle {
        fill,
        hover: theme::mix(fill, text, 0.12),
        text,
        stroke: Color32::TRANSPARENT,
    }
}

pub fn action(
    ui: &mut Ui,
    palette: &Palette,
    style: BtnStyle,
    title: &str,
    subtitle: Option<&str>,
    size: Vec2,
    enabled: bool,
) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let hot = ui.ctx().animate_bool_with_time(
        response.id,
        enabled && response.hovered(),
        0.12,
    );
    let mut fill = theme::mix(style.fill, style.hover, hot);
    let mut text = style.text;
    if !enabled {
        fill = theme::mix(style.fill, palette.bg, 0.45);
        text = palette.tertiary;
    }
    let draw = if response.is_pointer_button_down_on() && enabled {
        rect.shrink(1.0)
    } else {
        rect
    };
    ui.painter().rect(
        draw,
        CornerRadius::same(9),
        fill,
        Stroke::new(if style.stroke.a() == 0 { 0.0 } else { 1.0 }, style.stroke),
        StrokeKind::Inside,
    );
    let sub_color = if text.r() > 210 && text.g() > 210 && text.b() > 210 {
        Color32::from_white_alpha(210)
    } else {
        palette.secondary
    };
    let title_galley =
        ui.painter()
            .layout_no_wrap(title.to_owned(), theme::semi(13.5), text);
    let sub_galley = subtitle.filter(|text| !text.is_empty()).map(|text| {
        ui.painter()
            .layout_no_wrap(text.to_owned(), theme::body(11.0), sub_color)
    });
    let sub_h = sub_galley.as_ref().map(|g| g.size().y + 1.0).unwrap_or(0.0);
    let block = title_galley.size().y + sub_h;
    let mut y = draw.center().y - block * 0.5;
    let title_x = draw.center().x - title_galley.size().x * 0.5;
    ui.painter().galley(pos2(title_x, y), title_galley, text);
    if let Some(sub) = sub_galley {
        y += block - sub.size().y;
        let x = draw.center().x - sub.size().x * 0.5;
        ui.painter().galley(pos2(x, y), sub, sub_color);
    }
    if enabled && response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

pub fn field(
    ui: &mut Ui,
    palette: &Palette,
    text: &mut String,
    id: &str,
    hint: &'static str,
    rows: usize,
) -> Response {
    let width = ui.available_width();
    let frame = Frame::new()
        .fill(palette.field)
        .stroke(Stroke::new(1.0, palette.line))
        .corner_radius(8)
        .inner_margin(Margin::symmetric(10, 8));
    let font = FontId::new(15.0, egui::FontFamily::Proportional);
    let edit = if rows <= 1 {
        egui::TextEdit::singleline(text)
    } else {
        egui::TextEdit::multiline(text).desired_rows(rows)
    };
    ui.add(
        edit.id(Id::unique(id))
            .hint_text(hint)
            .font(font)
            .desired_width(width)
            .frame(frame)
            .text_color(palette.text)
            .char_limit(5000),
    )
}

pub fn caption(ui: &mut Ui, palette: &Palette, text: &str) {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), theme::semi(12.0), palette.secondary);
    let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter().galley(rect.min, galley, palette.secondary);
}

pub fn heading(ui: &mut Ui, palette: &Palette, text: &str, size: f32) {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), theme::semi(size), palette.text);
    let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter().galley(rect.min, galley, palette.text);
}

pub fn paragraph(ui: &mut Ui, text: &str, color: Color32, size: f32) {
    let width = ui.available_width();
    let galley = ui.painter().layout(
        text.to_owned(),
        FontId::new(size, egui::FontFamily::Proportional),
        color,
        width,
    );
    let (rect, _) = ui.allocate_exact_size(vec2(width, galley.size().y), Sense::hover());
    ui.painter()
        .galley(pos2(rect.center().x - galley.size().x * 0.5, rect.top()), galley, color);
}

pub fn left_text(ui: &mut Ui, text: &str, color: Color32, size: f32) {
    let width = ui.available_width();
    let galley = ui.painter().layout(
        text.to_owned(),
        FontId::new(size, egui::FontFamily::Proportional),
        color,
        width,
    );
    let (rect, _) = ui.allocate_exact_size(vec2(width, galley.size().y), Sense::hover());
    ui.painter().galley(rect.min, galley, color);
}

pub fn chip(ui: &mut Ui, text: &str, fg: Color32, bg: Color32) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), theme::semi(11.0), fg);
    let size = galley.size() + vec2(16.0, 6.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(20), bg);
    ui.painter().galley(
        pos2(rect.left() + 8.0, rect.center().y - galley.size().y * 0.5),
        galley,
        fg,
    );
    response
}

pub fn toggle(ui: &mut Ui, palette: &Palette, value: &mut bool, label: &str) -> Response {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 28.0), Sense::hover());
    let switch = Rect::from_min_size(rect.min, vec2(44.0, 26.0));
    let response = ui.interact(switch, Id::unique(("toggle", label)), Sense::click());
    if response.clicked() {
        *value = !*value;
    }
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id, *value, 0.14);
    let track = theme::mix(palette.line, palette.good, t);
    ui.painter()
        .rect_filled(switch, CornerRadius::same(13), track);
    let knob_x = switch.left() + 2.0 + t * (switch.width() - 26.0);
    let knob = Rect::from_min_size(pos2(knob_x, switch.top() + 2.0), vec2(22.0, 22.0));
    ui.painter()
        .rect_filled(knob, CornerRadius::same(11), Color32::WHITE);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::body(14.0), palette.text);
    ui.painter().galley(
        pos2(switch.right() + 10.0, rect.center().y - galley.size().y * 0.5),
        galley,
        palette.text,
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

pub fn meter(ui: &mut Ui, palette: &Palette, id: Id, label: &str, value: f32, color: Color32) {
    caption(ui, palette, label);
    let shown = ui.ctx().animate_value_with_time(id, value.clamp(0.0, 1.0), 0.35);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 7.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(4), palette.hover);
    let mut fill = rect;
    fill.max.x = fill.min.x + rect.width() * shown;
    if fill.width() > 1.0 {
        ui.painter()
            .rect_filled(fill, CornerRadius::same(4), color);
    }
}

pub fn paint_row(
    ui: &Ui,
    rect: Rect,
    response: &Response,
    selected: bool,
    palette: &Palette,
    base: Color32,
) {
    let hot = ui
        .ctx()
        .animate_bool_with_time(response.id, response.hovered() && !selected, 0.12);
    let fill = if selected {
        palette.selected
    } else {
        theme::mix(base, palette.hover, hot)
    };
    if selected || hot > 0.02 {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(8), fill);
    }
}

pub fn group<R>(ui: &mut Ui, palette: &Palette, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(palette.card)
        .stroke(Stroke::new(1.0, palette.line))
        .corner_radius(12)
        .inner_margin(Margin::symmetric(4, 4))
        .shadow(egui::Shadow {
            offset: [0, 8],
            blur: 22,
            spread: 0,
            color: palette.shadow,
        })
        .show(ui, add)
        .inner
}

pub fn hairline(ui: &mut Ui, palette: &Palette) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment(
        [
            pos2(rect.left() + 12.0, rect.center().y),
            pos2(rect.right() - 12.0, rect.center().y),
        ],
        Stroke::new(1.0, palette.line),
    );
}

pub fn ring(ui: &mut Ui, palette: &Palette, frac: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(72.0, 72.0), Sense::hover());
    let center = rect.center();
    let radius = 28.0;
    ui.painter()
        .circle_stroke(center, radius, Stroke::new(5.0, palette.hover));
    arc(ui, center, radius, frac.clamp(0.0, 1.0), palette.accent);
    let label = format!("{}%", (frac.clamp(0.0, 1.0) * 100.0).round() as i32);
    ui.painter().text(
        center,
        Align2::CENTER_CENTER,
        label,
        theme::semi(13.0),
        palette.text,
    );
}

fn arc(ui: &Ui, center: Pos2, radius: f32, frac: f32, color: Color32) {
    if frac <= 0.001 {
        return;
    }
    let steps = ((48.0 * frac).ceil() as i32).max(2);
    let start = -std::f32::consts::FRAC_PI_2;
    let sweep = std::f32::consts::TAU * frac;
    let mut points = Vec::with_capacity(steps as usize + 1);
    for step in 0..=steps {
        let angle = start + sweep * (step as f32 / steps as f32);
        points.push(center + vec2(angle.cos(), angle.sin()) * radius);
    }
    ui.painter().line(points, Stroke::new(5.0, color));
}

pub fn rating_color(palette: &Palette, rating: Rating) -> Color32 {
    match rating {
        Rating::Again => palette.again,
        Rating::Hard => palette.hard,
        Rating::Good => palette.good,
        Rating::Easy => palette.easy,
    }
}

pub fn rating_soft(palette: &Palette, rating: Rating) -> Color32 {
    match rating {
        Rating::Again => palette.again_soft,
        Rating::Hard => palette.hard_soft,
        Rating::Good => palette.good_soft,
        Rating::Easy => palette.easy_soft,
    }
}

pub fn shorten(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        text.to_string()
    } else {
        let trimmed: String = text.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{trimmed}…")
    }
}

pub fn typing(ctx: &egui::Context) -> bool {
    const IDS: &[&str] = &[
        "deck-name",
        "card-front",
        "card-back",
        "card-question",
        "card-answer",
        "typed-answer",
        "deck-filter",
    ];
    ctx.memory(|memory| IDS.iter().any(|id| memory.has_focus(Id::unique(*id))))
}

pub fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
