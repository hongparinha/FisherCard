use eframe::egui;
use std::time::Instant;

use crate::model::{Bucket, Card, Queued, Rating, StudyMode};
use crate::schedule::{self, practice_queue};
use crate::similarity::{self, RecallScore};
use crate::store::Store;
use crate::theme::{self, Palette};
use crate::timeutil;

pub struct FisherApp {
    pub store: Store,
    pub route: Route,
    pub route_at: f64,
    pub modal: Option<Modal>,
    pub modal_draw: Option<Modal>,
    pub form: Form,
    pub toast: Option<Toast>,
    pub filter: String,
    pub nudged: bool,
}

pub enum Route {
    Brain,
    Deck(u64),
    Study(Session),
}

#[derive(Clone)]
pub enum Modal {
    Deck { editing: Option<u64> },
    Card { deck_id: u64, editing: Option<u64> },
    DeleteDeck(u64),
    DeleteCard(u64),
}

#[derive(Clone)]
pub struct Form {
    pub name: String,
    pub color: u8,
    pub cover_weekly: bool,
    pub new_per_day: u32,
    pub front: String,
    pub back: String,
    pub question: String,
    pub answer: String,
    pub error: Option<String>,
    pub focus: bool,
}

impl Default for Form {
    fn default() -> Self {
        Self {
            name: String::new(),
            color: 0,
            cover_weekly: true,
            new_per_day: 20,
            front: String::new(),
            back: String::new(),
            question: String::new(),
            answer: String::new(),
            error: None,
            focus: true,
        }
    }
}

#[derive(Clone)]
pub struct Toast {
    pub text: String,
    pub born: Instant,
}

#[derive(Clone)]
pub struct Session {
    pub deck_id: u64,
    pub mode: StudyMode,
    pub practice: bool,
    pub queue: Vec<Queued>,
    pub index: usize,
    pub revealed: bool,
    pub phase: Phase,
    pub counts: [u32; 4],
    pub typed_input: String,
    pub score: Option<RecallScore>,
    pub chosen: Option<Rating>,
    pub undo: Option<UndoSnap>,
    pub focus_answer: bool,
    pub hint: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Prompt,
    Feedback,
    Done,
}

#[derive(Clone)]
pub struct UndoSnap {
    card: Card,
    review_id: u64,
    index: usize,
    counts: [u32; 4],
    queue: Vec<Queued>,
    phase: Phase,
    revealed: bool,
    typed_input: String,
    score: Option<RecallScore>,
    chosen: Option<Rating>,
}

impl FisherApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        let store = Store::load();
        theme::apply(&cc.egui_ctx, &theme::palette(store.is_dark()));
        Self {
            store,
            route: Route::Brain,
            route_at: f64::MIN,
            modal: None,
            modal_draw: None,
            form: Form::default(),
            toast: None,
            filter: String::new(),
            nudged: false,
        }
    }

    pub fn palette(&self) -> Palette {
        theme::palette(self.store.is_dark())
    }

    pub fn session(&self) -> Option<&Session> {
        match &self.route {
            Route::Study(session) => Some(session),
            _ => None,
        }
    }

    fn session_mut(&mut self) -> Option<&mut Session> {
        match &mut self.route {
            Route::Study(session) => Some(session),
            _ => None,
        }
    }

    pub fn toast(&mut self, text: impl Into<String>) {
        self.toast = Some(Toast {
            text: text.into(),
            born: Instant::now(),
        });
    }

    pub fn toggle_theme(&mut self, ctx: &egui::Context) {
        let dark = !self.store.is_dark();
        self.store.set_dark(dark);
        theme::apply(ctx, &theme::palette(dark));
    }

    pub fn open_brain(&mut self, time: f64) {
        self.route = Route::Brain;
        self.route_at = time;
    }

    pub fn open_deck(&mut self, id: u64, time: f64) {
        if self.store.deck(id).is_none() {
            return;
        }
        if !matches!(self.route, Route::Deck(current) if current == id) {
            self.filter.clear();
        }
        self.route = Route::Deck(id);
        self.route_at = time;
    }

    pub fn open_new_deck(&mut self) {
        let color = (self.store.decks().len() % 8) as u8;
        self.form = Form {
            color,
            focus: true,
            ..Form::default()
        };
        self.modal = Some(Modal::Deck { editing: None });
    }

    pub fn open_edit_deck(&mut self, id: u64) {
        let Some(deck) = self.store.deck(id).cloned() else {
            return;
        };
        self.form = Form {
            name: deck.name,
            color: deck.color,
            cover_weekly: deck.cover_weekly,
            new_per_day: deck.new_per_day,
            focus: true,
            ..Form::default()
        };
        self.modal = Some(Modal::Deck { editing: Some(id) });
    }

    pub fn open_new_card(&mut self, deck_id: u64) {
        self.form = Form {
            focus: true,
            ..Form::default()
        };
        self.modal = Some(Modal::Card {
            deck_id,
            editing: None,
        });
    }

    pub fn open_edit_card(&mut self, id: u64) {
        let Some(card) = self.store.card(id).cloned() else {
            return;
        };
        self.form = Form {
            front: card.front,
            back: card.back,
            question: card.question,
            answer: card.answer,
            focus: true,
            ..Form::default()
        };
        self.modal = Some(Modal::Card {
            deck_id: card.deck_id,
            editing: Some(id),
        });
    }

    pub fn ask_delete_deck(&mut self, id: u64) {
        self.modal = Some(Modal::DeleteDeck(id));
    }

    pub fn ask_delete_card(&mut self, id: u64) {
        self.modal = Some(Modal::DeleteCard(id));
    }

    pub fn close_modal(&mut self) {
        self.modal = None;
    }

    pub fn submit_modal(&mut self, time: f64) {
        let modal = self.modal.clone();
        match modal {
            Some(Modal::Deck { editing }) => self.submit_deck(editing, time),
            Some(Modal::Card { deck_id, editing }) => self.submit_card(deck_id, editing),
            Some(Modal::DeleteDeck(id)) => {
                let studying = matches!(&self.route, Route::Study(session) if session.deck_id == id);
                let viewing = matches!(self.route, Route::Deck(deck_id) if deck_id == id);
                self.store.delete_deck(id);
                if studying || viewing {
                    self.route = Route::Brain;
                    self.route_at = time;
                }
                self.modal = None;
                self.toast("Deck deleted");
            }
            Some(Modal::DeleteCard(id)) => {
                self.store.delete_card(id);
                self.modal = None;
                self.toast("Card deleted");
            }
            None => {}
        }
    }

    fn submit_deck(&mut self, editing: Option<u64>, time: f64) {
        let result = if let Some(id) = editing {
            self.store
                .update_deck(
                    id,
                    self.form.name.clone(),
                    self.form.color,
                    self.form.cover_weekly,
                    self.form.new_per_day,
                )
                .map(|()| id)
        } else {
            self.store.create_deck(
                self.form.name.clone(),
                self.form.color,
                self.form.cover_weekly,
                self.form.new_per_day,
            )
        };
        match result {
            Ok(id) => {
                self.modal = None;
                if editing.is_none() {
                    self.open_deck(id, time);
                    self.toast("Deck created");
                } else {
                    self.toast("Deck updated");
                }
            }
            Err(error) => self.form.error = Some(error),
        }
    }

    fn submit_card(&mut self, deck_id: u64, editing: Option<u64>) {
        let result = if let Some(id) = editing {
            self.store
                .update_card(
                    id,
                    self.form.front.clone(),
                    self.form.back.clone(),
                    self.form.question.clone(),
                    self.form.answer.clone(),
                )
                .map(|()| None)
        } else {
            self.store
                .create_card(
                    deck_id,
                    self.form.front.clone(),
                    self.form.back.clone(),
                    self.form.question.clone(),
                    self.form.answer.clone(),
                )
                .map(|(_id, intro)| Some(intro))
        };
        match result {
            Ok(Some(intro)) => {
                self.modal = None;
                self.toast(added_message(intro));
            }
            Ok(None) => {
                self.modal = None;
                self.toast("Card saved");
            }
            Err(error) => self.form.error = Some(error),
        }
    }

    pub fn start_study(&mut self, deck_id: u64, mode: StudyMode, practice: bool, time: f64) {
        let Some(deck) = self.store.deck(deck_id).cloned() else {
            return;
        };
        let cards = self.store.cards_of(deck_id);
        let refs: Vec<&Card> = cards.iter().collect();
        let now = timeutil::now_ms();
        let queue = if practice {
            practice_queue(&refs, 12, now as u64 ^ deck_id.wrapping_mul(97))
        } else {
            schedule::build_queue(&deck, &refs, mode, now)
        };
        self.route = Route::Study(Session {
            deck_id,
            mode,
            practice,
            queue,
            index: 0,
            revealed: false,
            phase: Phase::Prompt,
            counts: [0; 4],
            typed_input: String::new(),
            score: None,
            chosen: None,
            undo: None,
            focus_answer: true,
            hint: None,
        });
        self.route_at = time;
        self.settle_queue(now);
    }

    pub fn reveal(&mut self) {
        if let Some(session) = self.session_mut() {
            if session.mode == StudyMode::Flash && session.phase == Phase::Prompt {
                session.revealed = true;
            }
        }
    }

    pub fn rate_current(&mut self, rating: Rating) {
        let now = timeutil::now_ms();
        let Some(session) = self.session().cloned() else {
            return;
        };
        if session.mode != StudyMode::Flash
            || session.phase != Phase::Prompt
            || !session.revealed
        {
            return;
        }
        let Some(item) = session.queue.get(session.index).copied() else {
            return;
        };
        let Some(card) = self.store.card(item.card_id).cloned() else {
            return;
        };
        let mut snap = undo_from(&session, card);
        match self
            .store
            .review(item.card_id, StudyMode::Flash, rating, None, now)
        {
            Ok(review_id) => {
                snap.review_id = review_id;
                self.finish_review(snap, rating, item.card_id, now);
            }
            Err(error) => self.toast(error),
        }
    }

    pub fn submit_typed(&mut self) {
        let Some(session) = self.session().cloned() else {
            return;
        };
        if session.mode != StudyMode::Typed || session.phase != Phase::Prompt {
            return;
        }
        let Some(item) = session.queue.get(session.index).copied() else {
            return;
        };
        if session.typed_input.trim().is_empty() {
            if let Some(session) = self.session_mut() {
                session.hint = Some("Type an answer first.".into());
            }
            return;
        }
        let Some(card) = self.store.card(item.card_id).cloned() else {
            return;
        };
        let score = similarity::score_answer(&card.answer, &session.typed_input);
        let chosen = score.rating;
        if let Some(session) = self.session_mut() {
            session.score = Some(score);
            session.chosen = Some(chosen);
            session.phase = Phase::Feedback;
            session.hint = None;
        }
    }

    pub fn choose_rating(&mut self, rating: Rating) {
        if let Some(session) = self.session_mut() {
            if session.phase == Phase::Feedback {
                session.chosen = Some(rating);
            }
        }
    }

    pub fn continue_typed(&mut self) {
        let now = timeutil::now_ms();
        let Some(session) = self.session().cloned() else {
            return;
        };
        if session.mode != StudyMode::Typed || session.phase != Phase::Feedback {
            return;
        }
        let Some(item) = session.queue.get(session.index).copied() else {
            return;
        };
        let Some(card) = self.store.card(item.card_id).cloned() else {
            return;
        };
        let rating = session.chosen.unwrap_or(Rating::Again);
        let similarity = session.score.as_ref().map(|score| score.similarity);
        let mut snap = undo_from(&session, card);
        match self
            .store
            .review(item.card_id, StudyMode::Typed, rating, similarity, now)
        {
            Ok(review_id) => {
                snap.review_id = review_id;
                self.finish_review(snap, rating, item.card_id, now);
            }
            Err(error) => self.toast(error),
        }
    }

    pub fn undo(&mut self) {
        let Some(snap) = self.session().and_then(|session| session.undo.clone()) else {
            return;
        };
        if let Err(error) = self.store.restore_card(snap.card.clone(), Some(snap.review_id)) {
            self.toast(error);
            return;
        }
        if let Some(session) = self.session_mut() {
            session.queue = snap.queue;
            session.index = snap.index;
            session.counts = snap.counts;
            session.phase = snap.phase;
            session.revealed = snap.revealed;
            session.typed_input = snap.typed_input;
            session.score = snap.score;
            session.chosen = snap.chosen;
            session.hint = None;
            session.focus_answer = false;
            session.undo = None;
        }
    }

    pub fn skip_current(&mut self) {
        self.advance(timeutil::now_ms());
    }

    pub fn typed_input_mut(&mut self) -> Option<&mut String> {
        self.session_mut().map(|session| &mut session.typed_input)
    }

    pub fn take_answer_focus(&mut self) -> bool {
        let Some(session) = self.session_mut() else {
            return false;
        };
        if session.focus_answer {
            session.focus_answer = false;
            true
        } else {
            false
        }
    }

    fn finish_review(&mut self, snap: UndoSnap, rating: Rating, card_id: u64, now: i64) {
        if let Some(session) = self.session_mut() {
            session.undo = Some(snap);
            session.counts[rating.index()] = session.counts[rating.index()].saturating_add(1);
        }
        self.push_learning(card_id);
        self.advance(now);
    }

    fn push_learning(&mut self, card_id: u64) {
        let Some(mode) = self.session().map(|session| session.mode) else {
            return;
        };
        let Some(card) = self.store.card(card_id).cloned() else {
            return;
        };
        if !matches!(
            card.srs(mode).state,
            crate::model::CardState::Learning | crate::model::CardState::Relearning
        ) {
            return;
        }
        if let Some(session) = self.session_mut() {
            let already = session.queue[session.index + 1..]
                .iter()
                .any(|item| item.card_id == card_id);
            if !already {
                session.queue.push(Queued {
                    card_id,
                    bucket: Bucket::Learning,
                });
            }
        }
    }

    fn advance(&mut self, now: i64) {
        if let Some(session) = self.session_mut() {
            session.index += 1;
            session.revealed = false;
            session.typed_input.clear();
            session.score = None;
            session.chosen = None;
            session.hint = None;
            session.focus_answer = true;
            session.phase = Phase::Prompt;
        }
        self.settle_queue(now);
    }

    fn settle_queue(&mut self, now: i64) {
        loop {
            let probe = match &self.route {
                Route::Study(session) if session.phase != Phase::Feedback => {
                    if session.index >= session.queue.len() {
                        None
                    } else {
                        Some((
                            session.queue[session.index].card_id,
                            session.mode,
                            session.index,
                        ))
                    }
                }
                _ => return,
            };
            let Some((card_id, mode, index)) = probe else {
                if let Route::Study(session) = &mut self.route {
                    session.phase = Phase::Done;
                }
                return;
            };
            let skip = match self.store.card(card_id) {
                None => true,
                Some(card) => {
                    let srs = card.srs(mode);
                    matches!(
                        srs.state,
                        crate::model::CardState::Learning | crate::model::CardState::Relearning
                    ) && srs.due > now + 1_500
                }
            };
            if !skip {
                return;
            }
            if let Route::Study(session) = &mut self.route {
                if session.index == index {
                    session.index += 1;
                }
            }
        }
    }
}

fn undo_from(session: &Session, card: Card) -> UndoSnap {
    UndoSnap {
        card,
        review_id: 0,
        index: session.index,
        counts: session.counts,
        queue: session.queue.clone(),
        phase: session.phase,
        revealed: session.revealed,
        typed_input: session.typed_input.clone(),
        score: session.score.clone(),
        chosen: session.chosen,
    }
}

fn added_message(intro: i64) -> String {
    let now = timeutil::now_ms();
    if intro <= timeutil::end_of_day(now) {
        "Card added to today’s queue.".into()
    } else {
        format!(
            "Card added. It shows up {} so the week stays balanced.",
            timeutil::relative_day(intro, now)
        )
    }
}

impl eframe::App for FisherApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let color = self.palette().bg;
        [
            color.r() as f32 / 255.0,
            color.g() as f32 / 255.0,
            color.b() as f32 / 255.0,
            1.0,
        ]
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        crate::ui::show(self, ui);
    }
}
