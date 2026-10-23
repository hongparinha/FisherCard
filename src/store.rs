use std::fs;
use std::path::PathBuf;

use crate::model::{Card, CardState, Deck, Library, Rating, ReviewEvent, StudyMode};
use crate::schedule::next_intro_due;
use crate::timeutil;

pub struct Store {
    path: PathBuf,
    library: Library,
    last_error: Option<String>,
}

impl Store {
    pub fn load() -> Self {
        let dir = data_dir();
        let _ = fs::create_dir_all(&dir);
        Self::open(dir.join("library.json"))
    }

    pub fn open(path: PathBuf) -> Self {
        let mut library = match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Library>(&bytes) {
                Ok(library) => library,
                Err(_) => {
                    let _ = fs::copy(&path, path.with_extension("json.bak"));
                    Library::default()
                }
            },
            Err(_) => Library::default(),
        };
        repair(&mut library);
        Self {
            path,
            library,
            last_error: None,
        }
    }

    pub fn is_dark(&self) -> bool {
        self.library.settings.dark
    }

    pub fn error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn clear_error(&mut self) {
        self.last_error = None;
    }

    pub fn decks(&self) -> &[Deck] {
        &self.library.decks
    }

    pub fn deck(&self, id: u64) -> Option<&Deck> {
        self.library.decks.iter().find(|deck| deck.id == id)
    }

    pub fn cards_of(&self, deck_id: u64) -> Vec<Card> {
        self.library
            .cards
            .iter()
            .filter(|card| card.deck_id == deck_id)
            .cloned()
            .collect()
    }

    pub fn card(&self, id: u64) -> Option<&Card> {
        self.library.cards.iter().find(|card| card.id == id)
    }

    pub fn reviews(&self) -> &[ReviewEvent] {
        &self.library.reviews
    }

    pub fn set_dark(&mut self, dark: bool) {
        self.library.settings.dark = dark;
        self.persist();
    }

    pub fn create_deck(
        &mut self,
        name: String,
        color: u8,
        cover_weekly: bool,
        new_per_day: u32,
    ) -> Result<u64, String> {
        let name = tidy_name(&name)?;
        let id = self.alloc();
        self.library.decks.push(Deck {
            id,
            name,
            created_at: timeutil::now_ms(),
            cover_weekly,
            new_per_day: new_per_day.clamp(1, 200),
            color: color % 8,
        });
        self.persist();
        Ok(id)
    }

    pub fn update_deck(
        &mut self,
        id: u64,
        name: String,
        color: u8,
        cover_weekly: bool,
        new_per_day: u32,
    ) -> Result<(), String> {
        let name = tidy_name(&name)?;
        let deck = self
            .library
            .decks
            .iter_mut()
            .find(|deck| deck.id == id)
            .ok_or_else(|| "That deck is gone.".to_string())?;
        deck.name = name;
        deck.color = color % 8;
        deck.cover_weekly = cover_weekly;
        deck.new_per_day = new_per_day.clamp(1, 200);
        self.persist();
        Ok(())
    }

    pub fn delete_deck(&mut self, id: u64) {
        let card_ids: Vec<u64> = self
            .library
            .cards
            .iter()
            .filter(|card| card.deck_id == id)
            .map(|card| card.id)
            .collect();
        self.library.decks.retain(|deck| deck.id != id);
        self.library.cards.retain(|card| card.deck_id != id);
        self.library
            .reviews
            .retain(|review| review.deck_id != id && !card_ids.contains(&review.card_id));
        self.persist();
    }

    pub fn create_card(
        &mut self,
        deck_id: u64,
        front: String,
        back: String,
        question: String,
        answer: String,
    ) -> Result<(u64, i64), String> {
        let (front, back, question, answer) = validate_sides(front, back, question, answer)?;
        if self.deck(deck_id).is_none() {
            return Err("That deck is gone.".into());
        }
        let now = timeutil::now_ms();
        let weekly = self.deck(deck_id).unwrap().cover_weekly;
        let flash_intros = self.intro_dues(deck_id, StudyMode::Flash);
        let flash_intro = if weekly {
            next_intro_due(&flash_intros, now)
        } else {
            now
        };
        let typed_intro = if !question.is_empty() {
            let typed_intros = self.intro_dues(deck_id, StudyMode::Typed);
            if weekly {
                next_intro_due(&typed_intros, now)
            } else {
                now
            }
        } else {
            0
        };
        let id = self.alloc();
        let mut card = Card {
            id,
            deck_id,
            front,
            back,
            question,
            answer,
            created_at: now,
            updated_at: now,
            flash: crate::model::Srs::default(),
            typed: crate::model::Srs::default(),
            typed_avg: 0.0,
            typed_attempts: 0,
        };
        card.flash.intro_due = flash_intro;
        card.flash.due = flash_intro;
        if typed_intro != 0 {
            card.typed.intro_due = typed_intro;
            card.typed.due = typed_intro;
        }
        self.library.cards.push(card);
        self.persist();
        Ok((id, flash_intro))
    }

    pub fn update_card(
        &mut self,
        id: u64,
        front: String,
        back: String,
        question: String,
        answer: String,
    ) -> Result<(), String> {
        let (front, back, question, answer) = validate_sides(front, back, question, answer)?;
        let now = timeutil::now_ms();
        let (deck_id, was_typed, typed_is_new) = {
            let card = self
                .library
                .cards
                .iter()
                .find(|card| card.id == id)
                .ok_or_else(|| "That card is gone.".to_string())?;
            (
                card.deck_id,
                card.has_typed(),
                matches!(card.typed.state, CardState::New) && card.typed.reps == 0,
            )
        };
        let weekly = self
            .deck(deck_id)
            .map(|deck| deck.cover_weekly)
            .ok_or_else(|| "That deck is gone.".to_string())?;
        let now_typed = !question.is_empty() && !answer.is_empty();
        let typed_intro = if now_typed && !was_typed && typed_is_new {
            let existing = self.intro_dues(deck_id, StudyMode::Typed);
            Some(if weekly {
                next_intro_due(&existing, now)
            } else {
                now
            })
        } else {
            None
        };
        let card = self
            .library
            .cards
            .iter_mut()
            .find(|card| card.id == id)
            .unwrap();
        card.front = front;
        card.back = back;
        card.question = question;
        card.answer = answer;
        card.updated_at = now;
        if let Some(intro) = typed_intro {
            card.typed.intro_due = intro;
            card.typed.due = intro;
            card.typed.state = CardState::New;
        }
        self.persist();
        Ok(())
    }

    pub fn delete_card(&mut self, id: u64) {
        self.library.cards.retain(|card| card.id != id);
        self.library.reviews.retain(|review| review.card_id != id);
        self.persist();
    }

    pub fn review(
        &mut self,
        card_id: u64,
        mode: StudyMode,
        rating: Rating,
        similarity: Option<f32>,
        now: i64,
    ) -> Result<u64, String> {
        let deck_id = self
            .card(card_id)
            .map(|card| card.deck_id)
            .ok_or_else(|| "That card is gone.".to_string())?;
        let weekly = self
            .deck(deck_id)
            .map(|deck| deck.cover_weekly)
            .ok_or_else(|| "That deck is gone.".to_string())?;
        {
            let card = self
                .library
                .cards
                .iter_mut()
                .find(|card| card.id == card_id)
                .unwrap();
            crate::srs::apply_review(card.srs_mut(mode), card_id, rating, now, weekly);
            if let Some(score) = similarity {
                let score = score.clamp(0.0, 1.0);
                card.typed_attempts = card.typed_attempts.saturating_add(1);
                card.typed_avg = if card.typed_attempts == 1 {
                    score
                } else {
                    card.typed_avg * 0.65 + score * 0.35
                };
            }
            card.updated_at = now;
        }
        let id = self.alloc();
        self.library.reviews.push(ReviewEvent {
            id,
            card_id,
            deck_id,
            mode,
            rating,
            similarity,
            at: now,
        });
        self.persist();
        Ok(id)
    }

    pub fn restore_card(&mut self, card: Card, review_id: Option<u64>) -> Result<(), String> {
        if let Some(existing) = self.library.cards.iter_mut().find(|item| item.id == card.id) {
            *existing = card;
        } else {
            self.library.cards.push(card);
        }
        if let Some(review_id) = review_id {
            self.library.reviews.retain(|review| review.id != review_id);
        }
        self.persist();
        Ok(())
    }

    fn intro_dues(&self, deck_id: u64, mode: StudyMode) -> Vec<i64> {
        self.library
            .cards
            .iter()
            .filter(|card| card.deck_id == deck_id)
            .filter(|card| mode == StudyMode::Flash || card.has_typed())
            .filter(|card| matches!(card.srs(mode).state, CardState::New))
            .map(|card| card.srs(mode).intro_due)
            .collect()
    }

    fn alloc(&mut self) -> u64 {
        self.library.next_id += 1;
        self.library.next_id
    }

    fn persist(&mut self) {
        if let Err(error) = self.save() {
            self.last_error = Some(format!("Couldn't save the library. {error}"));
        }
    }

    fn save(&mut self) -> Result<(), String> {
        if self.library.reviews.len() > 8000 {
            let extra = self.library.reviews.len() - 8000;
            self.library.reviews.drain(0..extra);
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let json = serde_json::to_vec_pretty(&self.library).map_err(|error| error.to_string())?;
        let temp = self.path.with_extension("json.tmp");
        fs::write(&temp, &json).map_err(|error| error.to_string())?;
        if self.path.exists() {
            let backup = self.path.with_extension("json.bak");
            let _ = fs::copy(&self.path, backup);
            if fs::remove_file(&self.path).is_err() {
                fs::write(&self.path, &json).map_err(|error| error.to_string())?;
                let _ = fs::remove_file(&temp);
                self.last_error = None;
                return Ok(());
            }
        }
        if fs::rename(&temp, &self.path).is_err() {
            fs::write(&self.path, &json).map_err(|error| error.to_string())?;
            let _ = fs::remove_file(&temp);
        }
        self.last_error = None;
        Ok(())
    }
}

fn data_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("FisherCard")
}

fn repair(library: &mut Library) {
    for card in &mut library.cards {
        card.normalize();
    }
    for deck in &mut library.decks {
        if deck.new_per_day == 0 {
            deck.new_per_day = 20;
        }
        deck.color %= 8;
    }
    let max_id = library
        .decks
        .iter()
        .map(|deck| deck.id)
        .chain(library.cards.iter().map(|card| card.id))
        .chain(library.reviews.iter().map(|review| review.id))
        .max()
        .unwrap_or(0);
    if library.next_id < max_id {
        library.next_id = max_id;
    }
}

fn tidy_name(name: &str) -> Result<String, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Name the deck.".into());
    }
    if name.chars().count() > 80 {
        return Err("Use a shorter deck name.".into());
    }
    Ok(name)
}

fn validate_sides(
    front: String,
    back: String,
    question: String,
    answer: String,
) -> Result<(String, String, String, String), String> {
    let front = front.trim().to_string();
    let back = back.trim().to_string();
    let question = question.trim().to_string();
    let answer = answer.trim().to_string();
    if front.is_empty() || back.is_empty() {
        return Err("Add both a front and a back.".into());
    }
    if question.is_empty() != answer.is_empty() {
        return Err("Add both a question and an answer, or leave both empty.".into());
    }
    for (label, text) in [
        ("Front", front.as_str()),
        ("Back", back.as_str()),
        ("Question", question.as_str()),
        ("Answer", answer.as_str()),
    ] {
        if text.chars().count() > 5000 {
            return Err(format!("{label} is too long. Keep it under 5000 characters."));
        }
    }
    Ok((front, back, question, answer))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(name: &str) -> Store {
        let dir = std::env::temp_dir().join(format!("fishercard-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Store::open(dir.join("library.json"))
    }

    #[test]
    fn cards_round_trip_and_join_the_scheduler() {
        let mut store = temp_store("roundtrip");
        let deck_id = store.create_deck("Spanish".into(), 1, true, 20).unwrap();
        let (card_id, intro) = store
            .create_card(
                deck_id,
                "hola".into(),
                "hello".into(),
                "How do you greet someone?".into(),
                "hola".into(),
            )
            .unwrap();
        assert!(intro > 0);
        let reloaded = Store::open(store.path.clone());
        assert_eq!(reloaded.decks().len(), 1);
        assert_eq!(reloaded.cards_of(deck_id).len(), 1);
        let card = reloaded.card(card_id).unwrap();
        assert!(card.has_typed());
        assert_eq!(card.flash.intro_due, intro);
        let _ = fs::remove_dir_all(store.path.parent().unwrap());
    }

    #[test]
    fn a_question_without_an_answer_is_rejected() {
        let mut store = temp_store("partial");
        let deck_id = store.create_deck("Bio".into(), 0, true, 20).unwrap();
        let error = store
            .create_card(deck_id, "cell".into(), "unit of life".into(), "What is a cell?".into(), " ".into())
            .unwrap_err();
        assert!(error.contains("question"));
        let _ = fs::remove_dir_all(store.path.parent().unwrap());
    }

    #[test]
    fn reviews_move_the_due_date() {
        let mut store = temp_store("review");
        let deck_id = store.create_deck("Geo".into(), 2, true, 20).unwrap();
        let (card_id, _) = store
            .create_card(deck_id, "capital".into(), "Paris".into(), String::new(), String::new())
            .unwrap();
        let now = timeutil::now_ms();
        store
            .review(card_id, StudyMode::Flash, Rating::Easy, None, now)
            .unwrap();
        let card = store.card(card_id).unwrap();
        assert_eq!(card.flash.state, CardState::Review);
        assert!(card.flash.due > now);
        assert_eq!(store.reviews().len(), 1);
        let _ = fs::remove_dir_all(store.path.parent().unwrap());
    }
}
