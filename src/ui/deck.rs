use eframe::egui::{self, Color32, Rect, Sense, Ui, vec2};

use crate::app::FisherApp;
use crate::model::{Card, StudyMode};
use crate::schedule::{self, Pulse};
use crate::theme::{self, Palette};
use crate::timeutil;
use crate::ui::widgets;

pub fn show(app: &mut FisherApp, ui: &mut Ui, palette: &Palette, deck_id: u64, time: f64) {
    let now = timeutil::now_ms();
    let Some(deck) = app.store.deck(deck_id).cloned() else {
        widgets::left_text(ui, "That deck is gone.", palette.secondary, 15.0);
        return;
    };
    let mut cards = app.store.cards_of(deck_id);
    let refs: Vec<&Card> = cards.iter().collect();
    let pulse = schedule::deck_pulse(&deck, &refs, now);
    cards.sort_by_key(|card| std::cmp::Reverse(card.updated_at));

    ui.add_space(4.0);
    header(ui, palette, &deck.name, deck.color, &pulse, deck.cover_weekly, deck.new_per_day);
    ui.add_space(12.0);
    actions(app, ui, palette, deck_id, &pulse, time);
    if pulse.typed_cards == 0 {
        ui.add_space(2.0);
        widgets::left_text(
            ui,
            "Add both a question and an answer to use Answer & Question. Cards with only a front and back stay in Study.",
            palette.secondary,
            13.0,
        );
    }
    ui.add_space(10.0);
    widgets::field(
        ui,
        palette,
        &mut app.filter,
        "deck-filter",
        "Filter cards",
        1,
    );
    let filter = app.filter.trim().to_lowercase();
    let visible: Vec<Card> = cards
        .into_iter()
        .filter(|card| {
            filter.is_empty()
                || card.front.to_lowercase().contains(&filter)
                || card.back.to_lowercase().contains(&filter)
                || card.question.to_lowercase().contains(&filter)
                || card.answer.to_lowercase().contains(&filter)
        })
        .collect();
    ui.add_space(8.0);

    let list_h = ui.available_height().max(120.0);
    egui::ScrollArea::vertical()
        .max_height(list_h)
        .auto_shrink([false, false])
        .id_salt(("deck-cards", deck_id))
        .show(ui, |ui| {
            if visible.is_empty() {
                let message = if filter.is_empty() {
                    "No cards yet. Add one and it joins the week."
                } else {
                    "No cards match that filter."
                };
                widgets::left_text(ui, message, palette.secondary, 14.0);
                return;
            }
            widgets::group(ui, palette, |ui| {
                for (index, card) in visible.iter().enumerate() {
                    if index > 0 {
                        widgets::hairline(ui, palette);
                    }
                    card_row(app, ui, palette, card, now);
                }
            });
        });
}

fn header(
    ui: &mut Ui,
    palette: &Palette,
    name: &str,
    color: u8,
    pulse: &Pulse,
    weekly: bool,
    new_per_day: u32,
) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(14.0, 28.0), Sense::hover());
        ui.painter().circle_filled(
            egui::pos2(rect.center().x, rect.center().y),
            6.0,
            theme::deck_color(color),
        );
        widgets::heading(ui, palette, name, 28.0);
    });
    let detail = if weekly {
        format!(
            "{} cards · {} with a question. Every card comes back within the week.",
            pulse.cards, pulse.typed_cards
        )
    } else {
        format!(
            "{} cards · {} with a question. Up to {new_per_day} new cards a day.",
            pulse.cards, pulse.typed_cards
        )
    };
    widgets::left_text(ui, &detail, palette.secondary, 13.5);
}

fn actions(
    app: &mut FisherApp,
    ui: &mut Ui,
    palette: &Palette,
    deck_id: u64,
    pulse: &Pulse,
    time: f64,
) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        let study = if pulse.ready_flash > 0 {
            format!("Study  {}", pulse.ready_flash)
        } else {
            "Study".into()
        };
        if widgets::action(
            ui,
            palette,
            widgets::primary_style(palette),
            &study,
            None,
            vec2(148.0, 36.0),
            true,
        )
        .clicked()
        {
            app.start_study(deck_id, StudyMode::Flash, false, time);
        }
        let questions = if pulse.ready_typed > 0 {
            format!("Answer & Question  {}", pulse.ready_typed)
        } else {
            "Answer & Question".into()
        };
        if widgets::action(
            ui,
            palette,
            widgets::quiet_style(palette),
            &questions,
            None,
            vec2(210.0, 36.0),
            pulse.typed_cards > 0,
        )
        .clicked()
        {
            app.start_study(deck_id, StudyMode::Typed, false, time);
        }
        if widgets::action(
            ui,
            palette,
            widgets::quiet_style(palette),
            "Add card",
            None,
            vec2(108.0, 36.0),
            true,
        )
        .clicked()
        {
            app.open_new_card(deck_id);
        }
        if widgets::action(
            ui,
            palette,
            widgets::quiet_style(palette),
            "Deck settings",
            None,
            vec2(132.0, 36.0),
            true,
        )
        .clicked()
        {
            app.open_edit_deck(deck_id);
        }
        if widgets::action(
            ui,
            palette,
            widgets::soft_style(Color32::TRANSPARENT, palette.again),
            "Delete deck",
            None,
            vec2(118.0, 36.0),
            true,
        )
        .clicked()
        {
            app.ask_delete_deck(deck_id);
        }
    });
}

fn card_row(app: &mut FisherApp, ui: &mut Ui, palette: &Palette, card: &Card, now: i64) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 64.0), Sense::click());
    widgets::paint_row(ui, rect, &response, false, palette, palette.card);
    let title = ui.painter().layout(
        widgets::shorten(&card.front, 90),
        theme::semi(15.0),
        palette.text,
        rect.width() - 150.0,
    );
    ui.painter()
        .galley(egui::pos2(rect.left() + 12.0, rect.top() + 10.0), title, palette.text);
    let mut meta = schedule::card_status(card, StudyMode::Flash, now);
    if card.has_typed() {
        meta.push_str("  ·  A&Q");
    }
    let meta_galley = ui.painter().layout_no_wrap(meta, theme::body(12.0), palette.secondary);
    ui.painter().galley(
        egui::pos2(rect.left() + 12.0, rect.bottom() - 10.0 - meta_galley.size().y),
        meta_galley,
        palette.secondary,
    );

    let delete = Rect::from_min_size(
        egui::pos2(rect.right() - 72.0, rect.center().y - 14.0),
        vec2(60.0, 28.0),
    );
    let mut buttons = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(delete)
            .id_salt(("delete-card", card.id)),
    );
    let delete_clicked = widgets::action(
        &mut buttons,
        palette,
        widgets::soft_style(Color32::TRANSPARENT, palette.again),
        "Delete",
        None,
        vec2(60.0, 28.0),
        true,
    )
    .clicked();
    if delete_clicked {
        app.ask_delete_card(card.id);
    } else if response.clicked() {
        app.open_edit_card(card.id);
    }
}
