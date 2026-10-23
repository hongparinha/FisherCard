use eframe::egui::{self, Align, CornerRadius, Layout, Rect, Sense, Stroke, Ui, vec2};

use crate::app::FisherApp;
use crate::model::{Card, StudyMode};
use crate::schedule::{self, Pulse, WeakNote};
use crate::theme::{self, Palette};
use crate::timeutil;
use crate::ui::widgets;

struct DeckRow {
    id: u64,
    name: String,
    color: u8,
    pulse: Pulse,
}

struct WeakRow {
    deck_id: u64,
    title: String,
    note: WeakNote,
}

pub fn show(app: &mut FisherApp, ui: &mut Ui, palette: &Palette, time: f64) {
    let now = timeutil::now_ms();
    let (rows, weak, weak_total, activity) = collect(app, now);
    if rows.is_empty() {
        empty(app, ui, palette);
        return;
    }

    let width = ui.available_width().min(920.0);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .id_salt("brain-scroll")
        .show(ui, |ui| {
            let extra = (ui.available_width() - width).max(0.0) * 0.5;
            ui.horizontal(|ui| {
                ui.add_space(extra);
                ui.vertical(|ui| {
                    ui.set_width(width);
                    body(
                        app, ui, palette, time, now, &rows, &weak, weak_total, activity,
                    );
                });
            });
        });
}

fn collect(app: &FisherApp, now: i64) -> (Vec<DeckRow>, Vec<WeakRow>, usize, [u32; 7]) {
    let mut rows = Vec::new();
    let mut weak = Vec::new();
    for deck in app.store.decks() {
        let cards = app.store.cards_of(deck.id);
        let refs: Vec<&Card> = cards.iter().collect();
        let pulse = schedule::deck_pulse(deck, &refs, now);
        for card in &cards {
            if let Some(note) = schedule::weak_note(card, now) {
                weak.push(WeakRow {
                    deck_id: deck.id,
                    title: widgets::shorten(&card.front, 72),
                    note,
                });
            }
        }
        rows.push(DeckRow {
            id: deck.id,
            name: deck.name.clone(),
            color: deck.color,
            pulse,
        });
    }
    let weak_total = weak.len();
    weak.sort_by(|a, b| b.note.score.total_cmp(&a.note.score));
    weak.truncate(6);
    let activity = schedule::reviews_by_weekday(app.store.reviews(), now);
    (rows, weak, weak_total, activity)
}

fn body(
    app: &mut FisherApp,
    ui: &mut Ui,
    palette: &Palette,
    time: f64,
    now: i64,
    rows: &[DeckRow],
    weak: &[WeakRow],
    weak_total: usize,
    activity: [u32; 7],
) {
    ui.add_space(6.0);
    widgets::heading(ui, palette, "Brain", 30.0);
    ui.add_space(2.0);
    widgets::left_text(
        ui,
        &timeutil::format_long_date(now),
        palette.secondary,
        14.0,
    );
    ui.add_space(14.0);

    let covered: usize = rows
        .iter()
        .map(|row| row.pulse.flash_covered + row.pulse.typed_covered)
        .sum();
    let total: usize = rows
        .iter()
        .map(|row| row.pulse.flash_total + row.pulse.typed_total)
        .sum();
    let flash_ready: usize = rows.iter().map(|row| row.pulse.ready_flash).sum();
    let typed_ready: usize = rows.iter().map(|row| row.pulse.ready_typed).sum();
    let mut planned = [0u32; 7];
    for row in rows {
        for (index, count) in row.pulse.planned.iter().enumerate() {
            planned[index] = planned[index].saturating_add(*count);
        }
    }

    let gap = 12.0;
    let card_w = (ui.available_width() - gap * 2.0) / 3.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        stat_coverage(ui, palette, card_w, covered, total);
        let due_label = flash_ready.to_string();
        let weak_label = weak_total.to_string();
        let question_label = format!(
            "{typed_ready} question{}",
            if typed_ready == 1 { "" } else { "s" }
        );
        stat_number(ui, palette, card_w, &due_label, "Due now", &question_label);
        stat_number(
            ui,
            palette,
            card_w,
            &weak_label,
            "Needs work",
            "Low ease, lapses, or misses",
        );
    });

    ui.add_space(18.0);
    widgets::heading(ui, palette, "This week", 18.0);
    ui.add_space(4.0);
    week_bars(ui, palette, now, planned, activity);
    ui.add_space(6.0);
    widgets::left_text(
        ui,
        "New cards spread across seven days. Weekly decks bring every card back before the week ends, and weak cards come back sooner.",
        palette.secondary,
        13.0,
    );

    if !weak.is_empty() {
        ui.add_space(18.0);
        widgets::heading(ui, palette, "Needs another look", 18.0);
        ui.add_space(4.0);
        let mut open = None;
        widgets::group(ui, palette, |ui| {
            for (index, row) in weak.iter().enumerate() {
                if index > 0 {
                    widgets::hairline(ui, palette);
                }
                if weak_line(ui, palette, row).clicked() {
                    open = Some(row.deck_id);
                }
            }
        });
        if let Some(id) = open {
            app.open_deck(id, time);
        }
    }

    let mut ready: Vec<&DeckRow> = rows
        .iter()
        .filter(|row| row.pulse.ready_flash + row.pulse.ready_typed > 0)
        .collect();
    ready.sort_by_key(|row| {
        std::cmp::Reverse(row.pulse.ready_flash + row.pulse.ready_typed)
    });
    ui.add_space(18.0);
    widgets::heading(ui, palette, "Ready", 18.0);
    ui.add_space(4.0);
    if ready.is_empty() {
        widgets::left_text(ui, "Nothing is due right now.", palette.secondary, 14.0);
    } else {
        let mut open = None;
        widgets::group(ui, palette, |ui| {
            for (index, row) in ready.iter().enumerate() {
                if index > 0 {
                    widgets::hairline(ui, palette);
                }
                if ready_line(ui, palette, row).clicked() {
                    open = Some(row.id);
                }
            }
        });
        if let Some(id) = open {
            app.open_deck(id, time);
        }
    }
    ui.add_space(12.0);
}

fn stat_coverage(ui: &mut Ui, palette: &Palette, width: f32, covered: usize, total: usize) {
    stat_shell(ui, palette, width, |ui| {
        let frac = if total == 0 {
            0.0
        } else {
            covered as f32 / total as f32
        };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            widgets::ring(ui, palette, frac);
            ui.vertical(|ui| {
                ui.add_space(12.0);
                widgets::heading(
                    ui,
                    palette,
                    &if total == 0 {
                        "—".into()
                    } else {
                        format!("{covered} of {total}")
                    },
                    18.0,
                );
                widgets::left_text(ui, "Reviewed this week", palette.secondary, 12.5);
            });
        });
    });
}

fn stat_number(ui: &mut Ui, palette: &Palette, width: f32, value: &str, title: &str, detail: &str) {
    stat_shell(ui, palette, width, |ui| {
        widgets::heading(ui, palette, value, 28.0);
        ui.add_space(2.0);
        widgets::left_text(ui, title, palette.text, 14.0);
        widgets::left_text(ui, detail, palette.secondary, 12.5);
    });
}

fn stat_shell(ui: &mut Ui, palette: &Palette, width: f32, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(palette.card)
        .stroke(Stroke::new(1.0, palette.line))
        .corner_radius(14)
        .inner_margin(egui::Margin::same(14))
        .shadow(egui::Shadow {
            offset: [0, 8],
            blur: 20,
            spread: 0,
            color: palette.shadow,
        })
        .show(ui, |ui| {
            ui.set_width(width - 28.0);
            ui.set_min_height(78.0);
            add(ui);
        });
}

fn week_bars(ui: &mut Ui, palette: &Palette, now: i64, planned: [u32; 7], activity: [u32; 7]) {
    let today = timeutil::weekday_index(now, now).unwrap_or(0);
    let max = planned.iter().copied().max().unwrap_or(0).max(1) as f32;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 118.0), Sense::hover());
    ui.painter().rect(
        rect,
        CornerRadius::same(14),
        palette.card,
        Stroke::new(1.0, palette.line),
        egui::StrokeKind::Inside,
    );
    let inner = rect.shrink2(vec2(16.0, 14.0));
    let gap = 10.0;
    let col_w = (inner.width() - gap * 6.0) / 7.0;
    for index in 0..7 {
        let x = inner.left() + index as f32 * (col_w + gap);
        let label = timeutil::weekday_short(index);
        let color = if index == today {
            palette.accent
        } else {
            palette.tertiary
        };
        ui.painter().text(
            egui::pos2(x + col_w * 0.5, inner.bottom() - 4.0),
            egui::Align2::CENTER_BOTTOM,
            label,
            theme::semi(11.0),
            color,
        );
        let track_bottom = inner.bottom() - 22.0;
        let track_top = inner.top() + 8.0;
        let track = Rect::from_min_max(
            egui::pos2(x + col_w * 0.5 - 7.0, track_top),
            egui::pos2(x + col_w * 0.5 + 7.0, track_bottom),
        );
        ui.painter()
            .rect_filled(track, CornerRadius::same(7), palette.hover);
        let frac = planned[index] as f32 / max;
        let shown = ui.ctx().animate_value_with_time(
            egui::Id::unique(("week-bar", index)),
            track.height() * frac,
            0.4,
        );
        if shown > 1.0 {
            let mut bar = track;
            bar.min.y = track.max.y - shown;
            let fill = if index == today {
                palette.accent
            } else {
                theme::mix(palette.accent, palette.card, 0.45)
            };
            ui.painter()
                .rect_filled(bar, CornerRadius::same(7), fill);
        }
        if activity[index] > 0 {
            ui.painter().circle_filled(
                egui::pos2(x + col_w * 0.5, inner.top() + 3.0),
                2.4,
                palette.good,
            );
        }
    }
}

fn weak_line(ui: &mut Ui, palette: &Palette, row: &WeakRow) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), 52.0), Sense::click());
    widgets::paint_row(ui, rect, &response, false, palette, palette.card);
    let mode = match row.note.mode {
        StudyMode::Flash => "Flashcard",
        StudyMode::Typed => "Question",
    };
    let title = ui.painter().layout_no_wrap(
        row.title.clone(),
        theme::semi(14.0),
        palette.text,
    );
    ui.painter().galley(
        egui::pos2(rect.left() + 12.0, rect.top() + 8.0),
        title,
        palette.text,
    );
    let detail = format!("{mode} · {}", row.note.reason);
    let detail = ui.painter().layout(
        detail,
        theme::body(12.0),
        palette.secondary,
        rect.width() - 24.0,
    );
    ui.painter().galley(
        egui::pos2(rect.left() + 12.0, rect.top() + 28.0),
        detail,
        palette.secondary,
    );
    if response.hovered() {
        ui.ctx()
            .set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn ready_line(ui: &mut Ui, palette: &Palette, row: &DeckRow) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), 48.0), Sense::click());
    widgets::paint_row(ui, rect, &response, false, palette, palette.card);
    ui.painter().circle_filled(
        egui::pos2(rect.left() + 18.0, rect.center().y),
        5.0,
        theme::deck_color(row.color),
    );
    let title = ui
        .painter()
        .layout_no_wrap(row.name.clone(), theme::semi(14.0), palette.text);
    ui.painter().galley(
        egui::pos2(rect.left() + 32.0, rect.center().y - title.size().y * 0.5),
        title,
        palette.text,
    );
    let detail = format!(
        "{} to study · {} questions",
        row.pulse.ready_flash, row.pulse.ready_typed
    );
    let galley = ui
        .painter()
        .layout_no_wrap(detail, theme::body(12.0), palette.secondary);
    ui.painter().galley(
        egui::pos2(
            rect.right() - 12.0 - galley.size().x,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        palette.secondary,
    );
    if response.hovered() {
        ui.ctx()
            .set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn empty(app: &mut FisherApp, ui: &mut Ui, palette: &Palette) {
    ui.vertical_centered(|ui| {
        ui.add_space(120.0);
        widgets::heading(ui, palette, "Create a deck", 28.0);
        ui.add_space(6.0);
        widgets::paragraph(
            ui,
            "Cards you add are spread across the week, so the whole deck gets a turn.",
            palette.secondary,
            14.0,
        );
        ui.add_space(16.0);
        ui.allocate_ui_with_layout(
            vec2(180.0, 38.0),
            Layout::top_down(Align::Center),
            |ui| {
                if widgets::action(
                    ui,
                    palette,
                    widgets::primary_style(palette),
                    "New deck",
                    None,
                    vec2(160.0, 38.0),
                    true,
                )
                .clicked()
                {
                    app.open_new_deck();
                }
            },
        );
    });
}
