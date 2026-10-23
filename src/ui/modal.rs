use eframe::egui::{
    self, Align, Color32, CornerRadius, Id, Key, Layout, Modifiers, Rect, RichText, Sense,
    StrokeKind, Ui, UiBuilder, vec2,
};

use crate::app::{FisherApp, Modal};
use crate::theme::{self, Palette};
use crate::ui::widgets;

pub fn show(app: &mut FisherApp, ui: &mut Ui, palette: &Palette, full: egui::Rect, time: f64) {
    if app.modal.is_some() {
        app.modal_draw = app.modal.clone();
    }
    let open = app.modal.is_some();
    let t = ui.ctx().animate_bool_with_time(Id::unique("modal"), open, 0.16);
    if !open && t < 0.02 {
        app.modal_draw = None;
        return;
    }
    let Some(modal) = app.modal_draw.clone() else {
        return;
    };

    let mut confirm_enter = false;
    let screen = Rect::from_min_size(full.min, full.size());
    let card_w = 460.0_f32.min((full.width() - 48.0).max(280.0));
    let card_h = 540.0_f32.min((full.height() - 64.0).max(320.0));
    let card = Rect::from_center_size(
        screen.center() + vec2(0.0, (1.0 - t) * 12.0),
        vec2(card_w, card_h),
    );
    egui::Area::new(Id::unique("modal-layer"))
        .order(egui::Order::Foreground)
        .fixed_pos(full.min)
        .constrain(false)
        .interactable(true)
        .show(ui.ctx(), |ui| {
            ui.set_opacity(t.clamp(0.0, 1.0));
            ui.painter().rect_filled(
                screen,
                CornerRadius::ZERO,
                Color32::from_black_alpha((120.0 * t) as u8),
            );
            ui.painter().rect(
                card,
                CornerRadius::same(16),
                palette.card,
                egui::Stroke::new(1.0, palette.line),
                StrokeKind::Inside,
            );
            let dim = ui.interact(screen, Id::unique("modal-dim"), Sense::click());
            let _card_hit = ui.interact(card, Id::unique("modal-card"), Sense::click());
            let mut body = ui.new_child(
                UiBuilder::new()
                    .max_rect(card.shrink(18.0))
                    .layout(Layout::top_down(Align::Min))
                    .id_salt("modal-body"),
            );
            body.set_clip_rect(card.shrink(8.0));
            egui::ScrollArea::vertical()
                .max_height((card.height() - 36.0).max(80.0))
                .auto_shrink([false, false])
                .id_salt("modal-scroll")
                .show(&mut body, |ui| {
                    confirm_enter = match &modal {
                        Modal::Deck { editing } => deck_form(app, ui, palette, *editing),
                        Modal::Card { editing, .. } => {
                            card_form(app, ui, palette, editing.is_some())
                        }
                        Modal::DeleteDeck(_) => confirm_form(
                            app,
                            ui,
                            palette,
                            "Delete this deck?",
                            "Its cards are deleted too.",
                            time,
                        ),
                        Modal::DeleteCard(_) => confirm_form(
                            app,
                            ui,
                            palette,
                            "Delete this card?",
                            "This can't be undone.",
                            time,
                        ),
                    };
                });
            if dim.clicked() && dim.interact_pointer_pos().is_some_and(|pos| !card.contains(pos)) {
                app.close_modal();
            }
        });

    if app.modal.is_none() {
        return;
    }
    let escape = ui
        .ctx()
        .input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape));
    if escape {
        app.close_modal();
        return;
    }
    let command_enter = ui.ctx().input(|input| {
        input.key_pressed(Key::Enter) && input.modifiers.command
    });
    if confirm_enter || command_enter {
        app.submit_modal(time);
    }
}

fn deck_form(app: &mut FisherApp, ui: &mut Ui, palette: &Palette, editing: Option<u64>) -> bool {
    widgets::heading(
        ui,
        palette,
        if editing.is_some() {
            "Deck settings"
        } else {
            "New deck"
        },
        22.0,
    );
    ui.add_space(10.0);
    widgets::caption(ui, palette, "NAME");
    let name = widgets::field(ui, palette, &mut app.form.name, "deck-name", "Deck name", 1);
    if app.form.focus {
        name.request_focus();
        app.form.focus = false;
    }
    ui.add_space(8.0);
    widgets::caption(ui, palette, "COLOR");
    color_row(ui, &mut app.form.color);
    ui.add_space(10.0);
    widgets::toggle(
        ui,
        palette,
        &mut app.form.cover_weekly,
        "Cover every card each week",
    );
    if app.form.cover_weekly {
        ui.add_space(4.0);
        widgets::left_text(
            ui,
            "New cards spread across seven days. Intervals stay inside the week, and weak cards return sooner.",
            palette.secondary,
            13.0,
        );
    } else {
        ui.add_space(8.0);
        stepper(ui, palette, &mut app.form.new_per_day);
        widgets::left_text(
            ui,
            "Classic spacing. Intervals can grow past a week.",
            palette.secondary,
            13.0,
        );
    }
    error_line(ui, palette, app.form.error.as_deref());
    ui.add_space(12.0);
    let save = buttons(
        app,
        ui,
        palette,
        if editing.is_some() { "Save" } else { "Create" },
        time_unused(),
    );
    name.lost_focus() && ui.ctx().input(|input| input.key_pressed(Key::Enter)) || save
}

fn card_form(app: &mut FisherApp, ui: &mut Ui, palette: &Palette, editing: bool) -> bool {
    widgets::heading(
        ui,
        palette,
        if editing { "Edit card" } else { "New card" },
        22.0,
    );
    ui.add_space(10.0);
    widgets::caption(ui, palette, "FRONT");
    let front = widgets::field(ui, palette, &mut app.form.front, "card-front", "Front", 3);
    if app.form.focus {
        front.request_focus();
        app.form.focus = false;
    }
    ui.add_space(6.0);
    widgets::caption(ui, palette, "BACK");
    widgets::field(ui, palette, &mut app.form.back, "card-back", "Back", 3);
    ui.add_space(6.0);
    widgets::caption(ui, palette, "QUESTION");
    let question = widgets::field(
        ui,
        palette,
        &mut app.form.question,
        "card-question",
        "Optional question",
        2,
    );
    ui.add_space(6.0);
    widgets::caption(ui, palette, "ANSWER");
    let answer = widgets::field(
        ui,
        palette,
        &mut app.form.answer,
        "card-answer",
        "Separate other accepted answers with |",
        2,
    );
    ui.add_space(4.0);
    widgets::left_text(
        ui,
        "Leave both question and answer empty for a flashcard only. Fill both to also practice by typing.",
        palette.secondary,
        12.5,
    );
    error_line(ui, palette, app.form.error.as_deref());
    ui.add_space(12.0);
    let save = buttons(app, ui, palette, if editing { "Save" } else { "Add card" }, 0.0);
    let enter = ui.ctx().input(|input| input.key_pressed(Key::Enter));
    let command = ui.ctx().input(|input| input.modifiers.command);
    save || ((question.lost_focus() || answer.lost_focus()) && enter && !command)
}

fn confirm_form(
    app: &mut FisherApp,
    ui: &mut Ui,
    palette: &Palette,
    title: &str,
    body: &str,
    time: f64,
) -> bool {
    widgets::heading(ui, palette, title, 22.0);
    ui.add_space(6.0);
    widgets::left_text(ui, body, palette.secondary, 14.0);
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if widgets::action(
            ui,
            palette,
            widgets::quiet_style(palette),
            "Cancel",
            None,
            vec2(110.0, 36.0),
            true,
        )
        .clicked()
        {
            app.close_modal();
        }
        if widgets::action(
            ui,
            palette,
            widgets::solid_style(palette.again),
            "Delete",
            None,
            vec2(110.0, 36.0),
            true,
        )
        .clicked()
        {
            app.submit_modal(time);
        }
    });
    ui.ctx().input(|input| input.key_pressed(Key::Enter))
}

fn buttons(app: &mut FisherApp, ui: &mut Ui, palette: &Palette, save: &str, _time: f64) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if widgets::action(
            ui,
            palette,
            widgets::quiet_style(palette),
            "Cancel",
            None,
            vec2(110.0, 36.0),
            true,
        )
        .clicked()
        {
            app.close_modal();
        }
        if widgets::action(
            ui,
            palette,
            widgets::primary_style(palette),
            save,
            None,
            vec2(140.0, 36.0),
            true,
        )
        .clicked()
        {
            clicked = true;
        }
    });
    clicked
}

fn color_row(ui: &mut Ui, selected: &mut u8) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        for index in 0..8u8 {
            let (rect, response) =
                ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
            let color = theme::deck_color(index);
            ui.painter().circle_filled(rect.center(), 8.0, color);
            if *selected == index {
                ui.painter().circle_stroke(
                    rect.center(),
                    11.0,
                    egui::Stroke::new(2.0, color),
                );
            }
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if response.clicked() {
                *selected = index;
            }
        }
    });
}

fn stepper(ui: &mut Ui, palette: &Palette, value: &mut u32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if widgets::action(
            ui,
            palette,
            widgets::quiet_style(palette),
            "−",
            None,
            vec2(36.0, 32.0),
            true,
        )
        .clicked()
        {
            *value = value.saturating_sub(5).max(1);
        }
        ui.label(
            RichText::new(format!("{value} new a day"))
                .size(14.0)
                .color(palette.text),
        );
        if widgets::action(
            ui,
            palette,
            widgets::quiet_style(palette),
            "+",
            None,
            vec2(36.0, 32.0),
            true,
        )
        .clicked()
        {
            *value = (*value + 5).min(100);
        }
    });
}

fn error_line(ui: &mut Ui, palette: &Palette, error: Option<&str>) {
    if let Some(error) = error {
        ui.add_space(8.0);
        widgets::left_text(ui, error, palette.again, 13.0);
    }
}

fn time_unused() -> f64 {
    0.0
}
