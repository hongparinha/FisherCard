use std::time::Duration;

use eframe::egui::{
    self, Align, Color32, CornerRadius, CursorIcon, Id, Layout, Rect, ResizeDirection, Sense,
    Stroke, StrokeKind, Ui, UiBuilder, ViewportCommand, pos2, vec2,
};

use crate::app::{FisherApp, Route};
use crate::schedule::deck_pulse;
use crate::theme::{self, Palette};
use crate::timeutil;
use crate::ui::{deck, home, modal, study, widgets};

const TITLE_H: f32 = 48.0;
const SIDEBAR_W: f32 = 252.0;

fn keep_on_screen(ui: &Ui) -> bool {
    let (outer, monitor) = ui.ctx().input(|input| {
        let view = input.viewport();
        (view.outer_rect, view.monitor_size)
    });
    let (Some(outer), Some(monitor)) = (outer, monitor) else {
        return false;
    };
    if outer.width() < 200.0 || outer.height() < 200.0 || monitor.x < 200.0 || monitor.y < 200.0 {
        return false;
    }
    let margin = 8.0;
    let off = outer.top() < -margin
        || outer.left() < -margin
        || outer.bottom() > monitor.y - margin
        || outer.right() > monitor.x - margin;
    if off {
        let x = ((monitor.x - outer.width()) * 0.5).max(margin);
        let y = ((monitor.y - outer.height()) * 0.5).max(margin);
        ui.ctx()
            .send_viewport_cmd(ViewportCommand::OuterPosition(pos2(x, y)));
    }
    true
}

pub fn show(app: &mut FisherApp, ui: &mut Ui) {
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    ui.spacing_mut().window_margin = egui::Margin::ZERO;
    let palette = app.palette();
    let time = ui.ctx().input(|input| input.time);
    if app.route_at == f64::MIN {
        app.route_at = time - 1.0;
    }
    if !app.nudged {
        app.nudged = keep_on_screen(ui);
    }
    let full = ui.max_rect();
    ui.painter()
        .rect_filled(full, CornerRadius::ZERO, palette.bg);
    ui.painter().rect_stroke(
        full.shrink(0.5),
        CornerRadius::ZERO,
        Stroke::new(1.0, palette.line),
        StrokeKind::Inside,
    );

    if app.modal.is_none() && !widgets::typing(ui.ctx()) {
        let undo = ui
            .ctx()
            .input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z));
        if undo && app.session().is_some_and(|session| session.undo.is_some()) {
            app.undo();
        }
    }

    title_bar(app, ui, &palette, full);
    sidebar(app, ui, &palette, full, time);
    content(app, ui, &palette, full, time);
    modal::show(app, ui, &palette, full, time);
    toast(app, ui, &palette, full);
    resize_edges(ui, full);
    let busy = app.modal.is_some()
        || app.modal_draw.is_some()
        || app.toast.is_some()
        || time - app.route_at < 0.3;
    ui.ctx().request_repaint_after(if busy {
        Duration::from_millis(16)
    } else {
        Duration::from_secs(1)
    });
}

fn title_bar(app: &mut FisherApp, ui: &mut Ui, palette: &Palette, full: Rect) {
    let bar = Rect::from_min_max(full.min, pos2(full.right(), full.top() + TITLE_H));
    ui.painter().line_segment(
        [pos2(bar.left(), bar.bottom()), pos2(bar.right(), bar.bottom())],
        Stroke::new(1.0, palette.line),
    );
    let drag = Rect::from_min_max(
        pos2(bar.left() + 92.0, bar.top()),
        pos2(bar.right() - 56.0, bar.bottom()),
    );
    let drag_response = ui.interact(drag, Id::unique("title-drag"), Sense::click_and_drag());
    if drag_response.double_clicked() {
        let maximized = ui
            .ctx()
            .input(|input| input.viewport().maximized.unwrap_or(false));
        ui.ctx()
            .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    }
    if drag_response.drag_started() {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }

    let title = window_title(app);
    ui.ctx()
        .send_viewport_cmd(ViewportCommand::Title(title.clone()));
    ui.painter().text(
        bar.center(),
        egui::Align2::CENTER_CENTER,
        title,
        theme::semi(13.0),
        palette.secondary,
    );

    traffic_lights(ui, palette, bar);
    theme_button(app, ui, palette, bar);
}

fn window_title(app: &FisherApp) -> String {
    match &app.route {
        Route::Brain => "FisherCard".into(),
        Route::Deck(id) => app
            .store
            .deck(*id)
            .map(|deck| deck.name.clone())
            .unwrap_or_else(|| "FisherCard".into()),
        Route::Study(session) => {
            let name = app
                .store
                .deck(session.deck_id)
                .map(|deck| deck.name.as_str())
                .unwrap_or("Deck");
            format!("{} · {name}", session.mode.title())
        }
    }
}

fn traffic_lights(ui: &mut Ui, palette: &Palette, bar: Rect) {
    let colors = [
        Color32::from_rgb(255, 95, 87),
        Color32::from_rgb(254, 188, 46),
        Color32::from_rgb(40, 200, 64),
    ];
    let mut rects = Vec::new();
    let y = bar.center().y;
    let mut x = bar.left() + 18.0;
    for _ in 0..3 {
        rects.push(pos2(x, y));
        x += 20.0;
    }
    let _ = y;
    let cluster = Rect::from_min_max(
        pos2(bar.left() + 8.0, bar.top() + 8.0),
        pos2(bar.left() + 78.0, bar.bottom() - 8.0),
    );
    let hovered = ui
        .ctx()
        .input(|input| input.pointer.hover_pos())
        .is_some_and(|pos| cluster.contains(pos));
    let glyph = Color32::from_rgb(60, 40, 30);
    for (index, center) in rects.into_iter().enumerate() {
        let rect = Rect::from_center_size(center, vec2(12.0, 12.0));
        let response = ui.interact(rect, Id::unique(("traffic", index)), Sense::click());
        ui.painter().circle_filled(center, 6.0, colors[index]);
        if hovered {
            let stroke = Stroke::new(1.15, glyph);
            match index {
                0 => {
                    ui.painter().line_segment(
                        [center + vec2(-3.2, -3.2), center + vec2(3.2, 3.2)],
                        stroke,
                    );
                    ui.painter().line_segment(
                        [center + vec2(3.2, -3.2), center + vec2(-3.2, 3.2)],
                        stroke,
                    );
                }
                1 => {
                    ui.painter().line_segment(
                        [center + vec2(-3.4, 0.0), center + vec2(3.4, 0.0)],
                        stroke,
                    );
                }
                _ => {
                    ui.painter().rect_stroke(
                        Rect::from_center_size(center, vec2(6.5, 6.5)),
                        CornerRadius::same(1),
                        stroke,
                        StrokeKind::Inside,
                    );
                }
            }
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if response.clicked() {
            match index {
                0 => ui.ctx().send_viewport_cmd(ViewportCommand::Close),
                1 => ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true)),
                _ => {
                    let maximized = ui
                        .ctx()
                        .input(|input| input.viewport().maximized.unwrap_or(false));
                    ui.ctx()
                        .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                }
            }
        }
        let _ = palette;
    }
}

fn theme_button(app: &mut FisherApp, ui: &mut Ui, palette: &Palette, bar: Rect) {
    let rect = Rect::from_center_size(pos2(bar.right() - 36.0, bar.center().y), vec2(28.0, 28.0));
    let response = ui.interact(rect, Id::unique("theme-toggle"), Sense::click());
    let hot = ui
        .ctx()
        .animate_bool_with_time(response.id, response.hovered(), 0.12);
    if hot > 0.02 {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(8),
            theme::mix(palette.bg, palette.hover, hot),
        );
    }
    let center = rect.center();
    let color = palette.text;
    if palette.dark {
        ui.painter()
            .circle_stroke(center, 5.0, Stroke::new(1.5, color));
        for step in 0..8 {
            let angle = step as f32 * std::f32::consts::TAU / 8.0;
            let dir = vec2(angle.cos(), angle.sin());
            ui.painter().line_segment(
                [center + dir * 7.0, center + dir * 9.0],
                Stroke::new(1.4, color),
            );
        }
    } else {
        ui.painter()
            .circle_filled(center + vec2(-1.2, 0.4), 6.2, color);
        ui.painter()
            .circle_filled(center + vec2(2.4, -1.2), 5.4, palette.bg);
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if response.clicked() {
        app.toggle_theme(ui.ctx());
    }
}

fn sidebar(app: &mut FisherApp, ui: &mut Ui, palette: &Palette, full: Rect, time: f64) {
    let sidebar = Rect::from_min_max(
        pos2(full.left(), full.top() + TITLE_H),
        pos2(full.left() + SIDEBAR_W, full.bottom()),
    );
    ui.painter().line_segment(
        [
            pos2(sidebar.right(), sidebar.top()),
            pos2(sidebar.right(), sidebar.bottom()),
        ],
        Stroke::new(1.0, palette.line),
    );
    let mut side = ui.new_child(
        UiBuilder::new()
            .max_rect(sidebar.shrink2(vec2(10.0, 10.0)))
            .layout(Layout::top_down(Align::Min))
            .id_salt("sidebar"),
    );
    side.set_clip_rect(sidebar);
    side.spacing_mut().item_spacing = vec2(4.0, 4.0);

    if nav_row(
        &mut side,
        palette,
        "brain",
        "Brain",
        None,
        None,
        matches!(app.route, Route::Brain),
    )
    .clicked()
    {
        app.open_brain(time);
    }
    side.add_space(8.0);
    widgets::caption(&mut side, palette, "DECKS");
    side.add_space(2.0);

    let now = timeutil::now_ms();
    let rows: Vec<(u64, String, u8, usize, bool)> = app
        .store
        .decks()
        .iter()
        .map(|deck| {
            let cards = app.store.cards_of(deck.id);
            let refs: Vec<_> = cards.iter().collect();
            let pulse = deck_pulse(deck, &refs, now);
            let selected = match &app.route {
                Route::Deck(id) => *id == deck.id,
                Route::Study(session) => session.deck_id == deck.id,
                Route::Brain => false,
            };
            (
                deck.id,
                deck.name.clone(),
                deck.color,
                pulse.ready_flash.max(pulse.ready_typed),
                selected,
            )
        })
        .collect();

    let scroll_h = (side.available_height() - 48.0).max(80.0);
    egui::ScrollArea::vertical()
        .max_height(scroll_h)
        .auto_shrink([false, false])
        .id_salt("deck-list")
        .show(&mut side, |ui| {
            if rows.is_empty() {
                ui.add_space(8.0);
                widgets::left_text(ui, "No decks yet", palette.tertiary, 13.0);
            }
            for (id, name, color, due, selected) in &rows {
                let badge = (*due > 0).then(|| due.to_string());
                if nav_row(
                    ui,
                    palette,
                    &format!("deck-{id}"),
                    name,
                    Some(theme::deck_color(*color)),
                    badge,
                    *selected,
                )
                .clicked()
                {
                    app.open_deck(*id, time);
                }
            }
        });

    side.add_space(8.0);
    let new_width = side.available_width();
    if widgets::action(
        &mut side,
        palette,
        widgets::quiet_style(palette),
        "New deck",
        None,
        vec2(new_width, 34.0),
        true,
    )
    .clicked()
    {
        app.open_new_deck();
    }
}

fn nav_row(
    ui: &mut Ui,
    palette: &Palette,
    salt: &str,
    label: &str,
    dot: Option<Color32>,
    badge: Option<String>,
    selected: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
    let _ = salt;
    widgets::paint_row(ui, rect, &response, selected, palette, palette.bg);
    let mut x = rect.left() + 10.0;
    if let Some(color) = dot {
        ui.painter()
            .circle_filled(pos2(x + 4.0, rect.center().y), 4.0, color);
        x += 16.0;
    }
    let right_pad = if badge.is_some() { 36.0 } else { 10.0 };
    let galley = ui.painter().layout(
        label.to_owned(),
        theme::body(14.0),
        palette.text,
        (rect.right() - right_pad - x).max(20.0),
    );
    ui.painter().galley(
        pos2(x, rect.center().y - galley.size().y.min(18.0) * 0.5),
        galley,
        palette.text,
    );
    if let Some(badge) = badge {
        let galley = ui
            .painter()
            .layout_no_wrap(badge, theme::semi(11.0), palette.accent);
        let pill = Rect::from_center_size(
            pos2(rect.right() - 18.0, rect.center().y),
            galley.size() + vec2(12.0, 4.0),
        );
        ui.painter()
            .rect_filled(pill, CornerRadius::same(10), palette.accent_soft);
        ui.painter().galley(
            pos2(
                pill.center().x - galley.size().x * 0.5,
                pill.center().y - galley.size().y * 0.5,
            ),
            galley,
            palette.accent,
        );
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    response
}

fn content(app: &mut FisherApp, ui: &mut Ui, palette: &Palette, full: Rect, time: f64) {
    let mut page = Rect::from_min_max(
        pos2(full.left() + SIDEBAR_W, full.top() + TITLE_H),
        full.right_bottom(),
    );
    let error = app.store.error().map(|error| error.to_string());
    if error.is_some() {
        let banner = Rect::from_min_max(page.min, pos2(page.right(), page.top() + 44.0));
        page.min.y += 44.0;
        paint_error(app, ui, palette, banner, error.unwrap_or_default());
    }
    let fade = widgets::ease(((time - app.route_at) / 0.18) as f32);
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(page.shrink2(vec2(28.0, 22.0)))
            .layout(Layout::top_down(Align::Min))
            .id_salt("page"),
    );
    child.set_clip_rect(page);
    child.set_opacity(fade);
    child.spacing_mut().item_spacing = vec2(8.0, 8.0);
    let kind = match &app.route {
        Route::Brain => Page::Brain,
        Route::Deck(id) => Page::Deck(*id),
        Route::Study(_) => Page::Study,
    };
    match kind {
        Page::Brain => home::show(app, &mut child, palette, time),
        Page::Deck(id) => deck::show(app, &mut child, palette, id, time),
        Page::Study => study::show(app, &mut child, palette, time),
    }
}

enum Page {
    Brain,
    Deck(u64),
    Study,
}

fn paint_error(app: &mut FisherApp, ui: &mut Ui, palette: &Palette, banner: Rect, error: String) {
    ui.painter().rect_filled(banner, CornerRadius::ZERO, palette.again_soft);
    ui.painter().text(
        pos2(banner.left() + 28.0, banner.center().y),
        egui::Align2::LEFT_CENTER,
        error,
        theme::body(13.0),
        palette.again,
    );
    let button = Rect::from_min_size(pos2(banner.right() - 78.0, banner.center().y - 12.0), vec2(64.0, 24.0));
    let response = ui.interact(button, Id::unique("dismiss-error"), Sense::click());
    if response.clicked() {
        app.store.clear_error();
    }
    ui.painter().text(
        button.center(),
        egui::Align2::CENTER_CENTER,
        "Dismiss",
        theme::semi(12.0),
        palette.again,
    );
}

fn toast(app: &mut FisherApp, ui: &mut Ui, palette: &Palette, full: Rect) {
    let Some(toast) = app.toast.clone() else {
        return;
    };
    let age = toast.born.elapsed().as_secs_f32();
    if age > 2.8 {
        app.toast = None;
        return;
    }
    let fade = if age < 0.16 {
        age / 0.16
    } else if age > 2.35 {
        (2.8 - age) / 0.45
    } else {
        1.0
    };
    let rise = (1.0 - widgets::ease((age / 0.2).min(1.0))) * 10.0;
    egui::Area::new(Id::unique("toast"))
        .order(egui::Order::Foreground)
        .anchor(
            egui::Align2::CENTER_BOTTOM,
            vec2(SIDEBAR_W * 0.5, -28.0 - rise),
        )
        .interactable(false)
        .show(ui.ctx(), |ui| {
            ui.set_opacity(fade.clamp(0.0, 1.0));
            FrameToast::paint(ui, palette, &toast.text);
            let _ = full;
        });
}

struct FrameToast;

impl FrameToast {
    fn paint(ui: &mut Ui, palette: &Palette, text: &str) {
        egui::Frame::new()
            .fill(palette.card)
            .stroke(Stroke::new(1.0, palette.line))
            .corner_radius(10)
            .inner_margin(egui::Margin::symmetric(14, 10))
            .shadow(egui::Shadow {
                offset: [0, 10],
                blur: 24,
                spread: 0,
                color: palette.shadow,
            })
            .show(ui, |ui| {
                ui.label(egui::RichText::new(text).size(13.5).color(palette.text));
            });
    }
}

fn resize_edges(ui: &mut Ui, full: Rect) {
    let edge = 6.0;
    let corner = 14.0;
    let zones = [
        (
            Rect::from_min_max(full.left_top(), pos2(full.left() + corner, full.top() + corner)),
            CursorIcon::ResizeNorthWest,
            ResizeDirection::NorthWest,
            "nw",
        ),
        (
            Rect::from_min_max(pos2(full.right() - corner, full.top()), pos2(full.right(), full.top() + corner)),
            CursorIcon::ResizeNorthEast,
            ResizeDirection::NorthEast,
            "ne",
        ),
        (
            Rect::from_min_max(pos2(full.left(), full.bottom() - corner), pos2(full.left() + corner, full.bottom())),
            CursorIcon::ResizeSouthWest,
            ResizeDirection::SouthWest,
            "sw",
        ),
        (
            Rect::from_min_max(
                pos2(full.right() - corner, full.bottom() - corner),
                full.right_bottom(),
            ),
            CursorIcon::ResizeSouthEast,
            ResizeDirection::SouthEast,
            "se",
        ),
        (
            Rect::from_min_max(full.left_top(), pos2(full.right(), full.top() + edge)),
            CursorIcon::ResizeNorth,
            ResizeDirection::North,
            "n",
        ),
        (
            Rect::from_min_max(pos2(full.left(), full.bottom() - edge), full.right_bottom()),
            CursorIcon::ResizeSouth,
            ResizeDirection::South,
            "s",
        ),
        (
            Rect::from_min_max(full.left_top(), pos2(full.left() + edge, full.bottom())),
            CursorIcon::ResizeWest,
            ResizeDirection::West,
            "w",
        ),
        (
            Rect::from_min_max(pos2(full.right() - edge, full.top()), full.right_bottom()),
            CursorIcon::ResizeEast,
            ResizeDirection::East,
            "e",
        ),
    ];
    for (rect, cursor, direction, salt) in zones {
        let response = ui.interact(rect, Id::unique(("resize", salt)), Sense::drag());
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(cursor);
        }
        if response.drag_started() {
            ui.ctx()
                .send_viewport_cmd(ViewportCommand::BeginResize(direction));
        }
    }
}
