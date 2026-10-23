use eframe::egui::{
    self, Align2, Color32, CornerRadius, Id, Key, Modifiers, Rect, Sense, Stroke, Ui, pos2,
    vec2,
};

use crate::app::{FisherApp, Phase, Session};
use crate::model::{Bucket, Card, Rating, StudyMode};
use crate::schedule;
use crate::srs;
use crate::theme::{self, Palette};
use crate::timeutil;
use crate::ui::widgets;

pub fn show(app: &mut FisherApp, ui: &mut Ui, palette: &Palette, time: f64) {
    let Some(session) = app.session().cloned() else {
        return;
    };
    let now = timeutil::now_ms();
    let deck = app.store.deck(session.deck_id).cloned();
    let weekly = deck.as_ref().is_none_or(|deck| deck.cover_weekly);
    let width = ui.available_width().min(680.0);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .id_salt("study-scroll")
        .show(ui, |ui| {
            let extra = (ui.available_width() - width).max(0.0) * 0.5;
            ui.horizontal(|ui| {
                ui.add_space(extra);
                ui.vertical(|ui| {
                    ui.set_width(width);
                    session_body(app, ui, palette, &session, weekly, now, time);
                });
            });
        });
}

fn session_body(
    app: &mut FisherApp,
    ui: &mut Ui,
    palette: &Palette,
    session: &Session,
    weekly: bool,
    now: i64,
    time: f64,
) {
    header(app, ui, palette, session, time);
    ui.add_space(8.0);
    progress(ui, palette, session);
    ui.add_space(16.0);
    if session.phase != Phase::Feedback {
        ui.ctx()
            .animate_value_with_time(Id::unique("recall-percent"), 0.0, 0.2);
    }

    if session.phase == Phase::Done {
        finished(app, ui, palette, session, now, time);
        return;
    }

    let Some(item) = session.queue.get(session.index).copied() else {
        finished(app, ui, palette, session, now, time);
        return;
    };
    let Some(card) = app.store.card(item.card_id).cloned() else {
        widgets::left_text(ui, "That card is gone.", palette.secondary, 15.0);
        return;
    };
    chips(ui, palette, &card, session.mode, item.bucket, now);
    ui.add_space(10.0);

    let waiting = card.srs(session.mode).due > now + 1_500
        && !matches!(item.bucket, Bucket::New | Bucket::Practice);
    if waiting {
        waiting_card(app, ui, palette, &card, session.mode, now);
        return;
    }

    match session.mode {
        StudyMode::Flash => flash(app, ui, palette, session, &card, weekly, now),
        StudyMode::Typed => typed(app, ui, palette, session, &card, weekly, now),
    }
}

fn header(app: &mut FisherApp, ui: &mut Ui, palette: &Palette, session: &Session, time: f64) {
    ui.horizontal(|ui| {
        if widgets::action(
            ui,
            palette,
            widgets::quiet_style(palette),
            "‹  Deck",
            None,
            vec2(92.0, 32.0),
            true,
        )
        .clicked()
        {
            app.open_deck(session.deck_id, time);
        }
        ui.add_space(8.0);
        widgets::heading(ui, palette, session.mode.title(), 22.0);
        if session.undo.is_some() {
            ui.add_space(8.0);
            if widgets::action(
                ui,
                palette,
                widgets::quiet_style(palette),
                "Undo",
                None,
                vec2(72.0, 32.0),
                true,
            )
            .clicked()
            {
                app.undo();
            }
        }
    });
    if session.queue.is_empty() {
        return;
    }
    let place = if session.phase == Phase::Done {
        session.queue.len()
    } else {
        session.index + 1
    };
    widgets::left_text(
        ui,
        &format!("{place} of {}", session.queue.len()),
        palette.secondary,
        13.0,
    );
}

fn progress(ui: &mut Ui, palette: &Palette, session: &Session) {
    let total = session.queue.len().max(1) as f32;
    let done = if session.phase == Phase::Done {
        total
    } else {
        session.index as f32 + if session.revealed || session.phase == Phase::Feedback {
            0.45
        } else {
            0.0
        }
    };
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 4.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(2), palette.hover);
    let shown = ui.ctx().animate_value_with_time(
        Id::unique("study-progress"),
        (done / total).clamp(0.0, 1.0),
        0.2,
    );
    let mut fill = rect;
    fill.max.x = fill.min.x + rect.width() * shown;
    if fill.width() > 1.0 {
        ui.painter()
            .rect_filled(fill, CornerRadius::same(2), palette.accent);
    }
}

fn chips(ui: &mut Ui, palette: &Palette, card: &Card, mode: StudyMode, bucket: Bucket, now: i64) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (label, fg, bg) = match bucket {
            Bucket::New => ("New", palette.accent, palette.accent_soft),
            Bucket::Learning => ("Learning", palette.hard, palette.hard_soft),
            Bucket::Maintenance => ("Due this week", palette.accent, palette.accent_soft),
            Bucket::Practice => ("Practice", palette.secondary, palette.hover),
            Bucket::Review => ("", Color32::TRANSPARENT, Color32::TRANSPARENT),
        };
        if !label.is_empty() {
            widgets::chip(ui, label, fg, bg);
        }
        if schedule::mode_weakness(card, mode, now) >= 0.48 {
            widgets::chip(ui, "Needs work", palette.again, palette.again_soft);
        }
    });
}

fn flash(
    app: &mut FisherApp,
    ui: &mut Ui,
    palette: &Palette,
    session: &Session,
    card: &Card,
    weekly: bool,
    now: i64,
) {
    let flip = ui.ctx().animate_bool_with_time(
        Id::unique(("flip", card.id)),
        session.revealed,
        0.22,
    );
    let scale = (flip * std::f32::consts::PI).cos().abs().max(0.04);
    let show_back = flip >= 0.5;
    face_card(
        ui,
        palette,
        if show_back { "Back" } else { "Front" },
        if show_back { &card.back } else { &card.front },
        scale,
    );
    ui.add_space(14.0);
    if !session.revealed {
        if widgets::action(
            ui,
            palette,
            widgets::primary_style(palette),
            "Show answer",
            None,
            vec2(ui.available_width(), 42.0),
            true,
        )
        .clicked()
        {
            app.reveal();
        }
        widgets::paragraph(
            ui,
            "Space or Enter shows the answer",
            palette.tertiary,
            12.5,
        );
        keys_flash(app, ui, false);
    } else {
        ratings(app, ui, palette, card, StudyMode::Flash, weekly, now, None, true);
        widgets::paragraph(ui, "1 Again   2 Hard   3 Good   4 Easy", palette.tertiary, 12.5);
        keys_flash(app, ui, true);
    }
}

fn typed(
    app: &mut FisherApp,
    ui: &mut Ui,
    palette: &Palette,
    session: &Session,
    card: &Card,
    weekly: bool,
    now: i64,
) {
    face_card(ui, palette, "Question", &card.question, 1.0);
    ui.add_space(14.0);
    if session.phase == Phase::Prompt {
        let response = {
            let Some(text) = app.typed_input_mut() else {
                return;
            };
            widgets::field(ui, palette, text, "typed-answer", "Type the answer", 1)
        };
        if app.take_answer_focus() {
            response.request_focus();
        }
        if let Some(hint) = &session.hint {
            widgets::left_text(ui, hint, palette.again, 13.0);
        }
        ui.add_space(8.0);
        let check = widgets::action(
            ui,
            palette,
            widgets::primary_style(palette),
            "Check",
            None,
            vec2(ui.available_width(), 42.0),
            true,
        );
        let enter = response.lost_focus()
            && ui.ctx().input(|input| input.key_pressed(Key::Enter))
            && !ui.ctx().input(|input| input.modifiers.command);
        if check.clicked() || enter {
            app.submit_typed();
        }
        widgets::paragraph(ui, "Enter checks the answer", palette.tertiary, 12.5);
    } else {
        feedback(app, ui, palette, session, card, weekly, now);
    }
}

fn feedback(
    app: &mut FisherApp,
    ui: &mut Ui,
    palette: &Palette,
    session: &Session,
    card: &Card,
    weekly: bool,
    now: i64,
) {
    let Some(score) = session.score.clone() else {
        return;
    };
    let shown = ui.ctx().animate_value_with_time(
        Id::unique("recall-percent"),
        score.percent() as f32,
        0.45,
    );
    let color = widgets::rating_color(palette, score.rating);
    ui.horizontal(|ui| {
        widgets::heading(ui, palette, &format!("{}%", shown.round() as i32), 36.0);
        ui.add_space(12.0);
        ui.vertical(|ui| {
            ui.add_space(8.0);
            widgets::heading(ui, palette, score.headline(), 20.0);
            widgets::left_text(
                ui,
                "Compared using spelling, words covered, and word order.",
                palette.secondary,
                12.5,
            );
        });
    });
    let _ = color;
    ui.add_space(8.0);
    widgets::meter(
        ui,
        palette,
        Id::unique(("meter", card.id, "spell")),
        "Spelling",
        score.spelling,
        palette.accent,
    );
    widgets::meter(
        ui,
        palette,
        Id::unique(("meter", card.id, "cover")),
        "Words",
        score.coverage,
        palette.good,
    );
    widgets::meter(
        ui,
        palette,
        Id::unique(("meter", card.id, "order")),
        "Order",
        score.order,
        palette.hard,
    );
    if !score.missing.is_empty() {
        ui.add_space(6.0);
        widgets::caption(ui, palette, "MISSING");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            for word in &score.missing {
                widgets::chip(ui, word, palette.again, palette.again_soft);
            }
        });
    }
    ui.add_space(8.0);
    widgets::caption(ui, palette, "YOU WROTE");
    widgets::left_text(ui, &session.typed_input, palette.text, 15.0);
    ui.add_space(4.0);
    widgets::caption(ui, palette, "ANSWER");
    widgets::left_text(ui, &card.answer, palette.text, 15.0);
    ui.add_space(10.0);
    ratings(
        app,
        ui,
        palette,
        card,
        StudyMode::Typed,
        weekly,
        now,
        session.chosen,
        false,
    );
    ui.add_space(8.0);
    if widgets::action(
        ui,
        palette,
        widgets::primary_style(palette),
        "Continue",
        None,
        vec2(ui.available_width(), 42.0),
        true,
    )
    .clicked()
        || ui.ctx().input_mut(|input| {
            input.consume_key(Modifiers::NONE, Key::Enter)
                || input.consume_key(Modifiers::NONE, Key::Space)
        })
    {
        app.continue_typed();
    }
    if let Some(rating) = number_key(ui) {
        app.choose_rating(rating);
    }
}

fn ratings(
    app: &mut FisherApp,
    ui: &mut Ui,
    palette: &Palette,
    card: &Card,
    mode: StudyMode,
    weekly: bool,
    now: i64,
    chosen: Option<Rating>,
    apply_now: bool,
) {
    let gap = 8.0;
    let width = (ui.available_width() - gap * 3.0) / 4.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for rating in Rating::ALL {
            let label = srs::preview_label(card.srs(mode), card.id, rating, now, weekly);
            let suggested = chosen == Some(rating) || (chosen.is_none() && apply_now);
            let style = if !apply_now && chosen == Some(rating) {
                widgets::solid_style(widgets::rating_color(palette, rating))
            } else if apply_now {
                widgets::solid_style(widgets::rating_color(palette, rating))
            } else {
                widgets::soft_style(
                    widgets::rating_soft(palette, rating),
                    widgets::rating_color(palette, rating),
                )
            };
            let _ = suggested;
            if widgets::action(
                ui,
                palette,
                style,
                rating.label(),
                Some(&label),
                vec2(width, 52.0),
                true,
            )
            .clicked()
            {
                if apply_now {
                    app.rate_current(rating);
                } else {
                    app.choose_rating(rating);
                }
            }
        }
    });
}

fn face_card(ui: &mut Ui, palette: &Palette, caption: &str, text: &str, scale: f32) {
    let full = vec2(ui.available_width(), 300.0);
    let (rect, _) = ui.allocate_exact_size(full, Sense::hover());
    let card = Rect::from_center_size(rect.center(), vec2(rect.width() * scale, rect.height()));
    let painter = ui.painter_at(card);
    painter.rect(
        card,
        CornerRadius::same(18),
        palette.card,
        Stroke::new(1.0, palette.line),
        egui::StrokeKind::Inside,
    );
    if scale < 0.2 {
        return;
    }
    painter.text(
        pos2(card.left() + 22.0, card.top() + 18.0),
        Align2::LEFT_TOP,
        caption,
        theme::semi(12.0),
        palette.tertiary,
    );
    let inner_w = (card.width() - 44.0).max(20.0);
    let size = face_size(text.chars().count());
    let galley = painter.layout(
        text.to_owned(),
        theme::semi(size),
        palette.text,
        inner_w,
    );
    let top = card.top() + 48.0;
    let room = card.bottom() - 20.0 - top;
    let y = if galley.size().y < room {
        top + (room - galley.size().y) * 0.5
    } else {
        top
    };
    painter.galley(pos2(card.center().x - galley.size().x * 0.5, y), galley, palette.text);
}

fn face_size(len: usize) -> f32 {
    if len > 280 {
        16.0
    } else if len > 140 {
        20.0
    } else if len > 60 {
        24.0
    } else {
        30.0
    }
}

fn waiting_card(app: &mut FisherApp, ui: &mut Ui, palette: &Palette, card: &Card, mode: StudyMode, now: i64) {
    face_card(ui, palette, "Front", &card.front, 1.0);
    ui.add_space(12.0);
    widgets::paragraph(
        ui,
        &format!(
            "This one comes back in {}.",
            timeutil::format_span(now, card.srs(mode).due)
        ),
        palette.secondary,
        14.0,
    );
    if widgets::action(
        ui,
        palette,
        widgets::quiet_style(palette),
        "Skip for now",
        None,
        vec2(160.0, 36.0),
        true,
    )
    .clicked()
    {
        app.skip_current();
    }
}

fn finished(app: &mut FisherApp, ui: &mut Ui, palette: &Palette, session: &Session, now: i64, time: f64) {
    let reviewed: u32 = session.counts.iter().sum();
    ui.add_space(36.0);
    if reviewed == 0 {
        widgets::heading(
            ui,
            palette,
            if session.mode == StudyMode::Flash {
                "Nothing is due"
            } else {
                "No questions are due"
            },
            28.0,
        );
        ui.add_space(6.0);
        let cards = app.store.cards_of(session.deck_id);
        let refs: Vec<&Card> = cards.iter().collect();
        if let Some(due) = schedule::next_future_due(&refs, session.mode, now) {
            widgets::left_text(
                ui,
                &format!("Next up {}", timeutil::format_when(due, now).to_lowercase()),
                palette.secondary,
                14.0,
            );
        } else {
            widgets::left_text(ui, "The deck is clear for now.", palette.secondary, 14.0);
        }
        ui.add_space(16.0);
        if session.mode == StudyMode::Typed && refs.iter().any(|card| card.has_typed()) {
            if widgets::action(
                ui,
                palette,
                widgets::primary_style(palette),
                "Practice random questions",
                None,
                vec2(240.0, 40.0),
                true,
            )
            .clicked()
            {
                app.start_study(session.deck_id, StudyMode::Typed, true, time);
            }
            ui.add_space(8.0);
        }
    } else {
        widgets::heading(ui, palette, "That's the queue", 28.0);
        ui.add_space(8.0);
        widgets::left_text(
            ui,
            &format!(
                "Again {}   Hard {}   Good {}   Easy {}",
                session.counts[0], session.counts[1], session.counts[2], session.counts[3]
            ),
            palette.secondary,
            15.0,
        );
        if session.practice {
            ui.add_space(4.0);
            widgets::left_text(
                ui,
                "Practice still updates the schedule.",
                palette.tertiary,
                13.0,
            );
        }
        ui.add_space(16.0);
    }
    if widgets::action(
        ui,
        palette,
        widgets::quiet_style(palette),
        "Back to deck",
        None,
        vec2(140.0, 36.0),
        true,
    )
    .clicked()
    {
        app.open_deck(session.deck_id, time);
    }
}

fn keys_flash(app: &mut FisherApp, ui: &Ui, revealed: bool) {
    if widgets::typing(ui.ctx()) || app.modal.is_some() {
        return;
    }
    let reveal = ui.ctx().input_mut(|input| {
        input.consume_key(Modifiers::NONE, Key::Space) || input.consume_key(Modifiers::NONE, Key::Enter)
    });
    if !revealed && reveal {
        app.reveal();
        return;
    }
    if revealed {
        if let Some(rating) = number_key(ui) {
            app.rate_current(rating);
        }
    }
}

fn number_key(ui: &Ui) -> Option<Rating> {
    if widgets::typing(ui.ctx()) {
        return None;
    }
    let key = ui.ctx().input_mut(|input| {
        if input.consume_key(Modifiers::NONE, Key::Num1) {
            Some(Rating::Again)
        } else if input.consume_key(Modifiers::NONE, Key::Num2) {
            Some(Rating::Hard)
        } else if input.consume_key(Modifiers::NONE, Key::Num3) {
            Some(Rating::Good)
        } else if input.consume_key(Modifiers::NONE, Key::Num4) {
            Some(Rating::Easy)
        } else {
            None
        }
    });
    key
}
