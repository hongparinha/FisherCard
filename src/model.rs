use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StudyMode {
    Flash,
    Typed,
}

impl StudyMode {
    pub fn title(self) -> &'static str {
        match self {
            Self::Flash => "Study",
            Self::Typed => "Answer & Question",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rating {
    Again,
    Hard,
    Good,
    Easy,
}

impl Rating {
    pub const ALL: [Rating; 4] = [Rating::Again, Rating::Hard, Rating::Good, Rating::Easy];

    pub fn label(self) -> &'static str {
        match self {
            Self::Again => "Again",
            Self::Hard => "Hard",
            Self::Good => "Good",
            Self::Easy => "Easy",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::Again => 0,
            Self::Hard => 1,
            Self::Good => 2,
            Self::Easy => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardState {
    #[default]
    New,
    Learning,
    Review,
    Relearning,
}

fn default_ease() -> f32 {
    2.5
}

/// One scheduling track. Flashcards and typed questions each have their own.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Srs {
    #[serde(default = "default_ease")]
    pub ease: f32,
    #[serde(default)]
    pub interval_days: f32,
    #[serde(default)]
    pub due: i64,
    #[serde(default)]
    pub reps: u32,
    #[serde(default)]
    pub lapses: u32,
    #[serde(default)]
    pub step: u8,
    #[serde(default)]
    pub state: CardState,
    #[serde(default)]
    pub success_at: Option<i64>,
    #[serde(default)]
    pub fail_streak: u32,
    #[serde(default)]
    pub intro_due: i64,
}

impl Default for Srs {
    fn default() -> Self {
        Self {
            ease: 2.5,
            interval_days: 0.0,
            due: 0,
            reps: 0,
            lapses: 0,
            step: 0,
            state: CardState::New,
            success_at: None,
            fail_streak: 0,
            intro_due: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Deck {
    pub id: u64,
    pub name: String,
    pub created_at: i64,
    #[serde(default = "default_true")]
    pub cover_weekly: bool,
    #[serde(default = "default_new_per_day")]
    pub new_per_day: u32,
    #[serde(default)]
    pub color: u8,
}

fn default_true() -> bool {
    true
}

fn default_new_per_day() -> u32 {
    20
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Card {
    pub id: u64,
    pub deck_id: u64,
    pub front: String,
    pub back: String,
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub answer: String,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub flash: Srs,
    #[serde(default)]
    pub typed: Srs,
    #[serde(default)]
    pub typed_avg: f32,
    #[serde(default)]
    pub typed_attempts: u32,
}

impl Card {
    pub fn has_typed(&self) -> bool {
        !self.question.trim().is_empty() && !self.answer.trim().is_empty()
    }

    pub fn srs(&self, mode: StudyMode) -> &Srs {
        match mode {
            StudyMode::Flash => &self.flash,
            StudyMode::Typed => &self.typed,
        }
    }

    pub fn srs_mut(&mut self, mode: StudyMode) -> &mut Srs {
        match mode {
            StudyMode::Flash => &mut self.flash,
            StudyMode::Typed => &mut self.typed,
        }
    }

    pub fn normalize(&mut self) {
        for srs in [&mut self.flash, &mut self.typed] {
            if srs.ease < 1.3 {
                srs.ease = 2.5;
            } else if srs.ease > 3.0 {
                srs.ease = 3.0;
            }
        }
        self.typed_avg = self.typed_avg.clamp(0.0, 1.0);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewEvent {
    pub id: u64,
    pub card_id: u64,
    pub deck_id: u64,
    pub mode: StudyMode,
    pub rating: Rating,
    #[serde(default)]
    pub similarity: Option<f32>,
    pub at: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bucket {
    Learning,
    Review,
    Maintenance,
    New,
    Practice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Queued {
    pub card_id: u64,
    pub bucket: Bucket,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub dark: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { dark: false }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Library {
    #[serde(default)]
    pub next_id: u64,
    #[serde(default)]
    pub decks: Vec<Deck>,
    #[serde(default)]
    pub cards: Vec<Card>,
    #[serde(default)]
    pub reviews: Vec<ReviewEvent>,
    #[serde(default)]
    pub settings: Settings,
}
