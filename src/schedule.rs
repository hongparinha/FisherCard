//! Today's queue and the week plan.
//! New cards are spread over seven days. With weekly coverage on, a card that
//! would otherwise wait longer than this week is given a slot before Sunday.

use std::collections::{HashMap, HashSet};

use crate::model::{Bucket, Card, CardState, Deck, Queued, ReviewEvent, StudyMode};
use crate::srs::display_pressure;
use crate::timeutil::{self, add_local_days, end_of_day, end_of_week, start_of_day};

#[derive(Clone, Debug)]
pub struct WeakNote {
    pub score: f32,
    pub reason: String,
    pub mode: StudyMode,
}

#[derive(Clone, Debug)]
pub struct Pulse {
    pub cards: usize,
    pub typed_cards: usize,
    pub ready_flash: usize,
    pub ready_typed: usize,
    pub later_flash: usize,
    pub later_typed: usize,
    pub flash_covered: usize,
    pub flash_total: usize,
    pub typed_covered: usize,
    pub typed_total: usize,
    pub planned: [u32; 7],
}

pub fn next_intro_due(existing: &[i64], now: i64) -> i64 {
    let today = start_of_day(now);
    let mut loads = [0u32; 7];
    for due in existing {
        let day = timeutil::local_days_between(today, *due);
        if (0..7).contains(&day) {
            loads[day as usize] += 1;
        }
    }
    let best = (0..7).min_by_key(|&day| (loads[day], day)).unwrap_or(0);
    if best == 0 {
        now
    } else {
        add_local_days(today, best as i64)
    }
}

pub fn covered_this_week(srs: &crate::model::Srs, now: i64, weekly: bool) -> bool {
    let start = timeutil::start_of_week(now);
    let end = end_of_week(now);
    if let Some(when) = srs.success_at {
        if when >= start && when <= end {
            return true;
        }
    }
    if !weekly && matches!(srs.state, CardState::Review) && srs.due > end {
        return true;
    }
    false
}

pub fn in_scope(srs: &crate::model::Srs, weekly: bool, now: i64) -> bool {
    if weekly {
        return true;
    }
    match srs.state {
        CardState::New | CardState::Learning | CardState::Relearning => true,
        CardState::Review => srs.due <= end_of_week(now),
    }
}

pub fn maintenance_slot(card_id: u64, now: i64) -> i64 {
    let start = timeutil::start_of_week(now);
    add_local_days(start, (card_id % 7) as i64)
}

pub fn next_due(card_id: u64, srs: &crate::model::Srs, weekly: bool, now: i64) -> i64 {
    match srs.state {
        CardState::New => srs.intro_due,
        CardState::Learning | CardState::Relearning => {
            if srs.due < now {
                now
            } else {
                srs.due
            }
        }
        CardState::Review => {
            if srs.due <= now {
                now
            } else if weekly && !covered_this_week(srs, now, true) && srs.due > end_of_week(now) {
                let slot = maintenance_slot(card_id, now);
                if slot < start_of_day(now) {
                    now
                } else {
                    slot
                }
            } else {
                srs.due
            }
        }
    }
}

pub fn ready_bucket(deck: &Deck, card: &Card, mode: StudyMode, now: i64) -> Option<Bucket> {
    if mode == StudyMode::Typed && !card.has_typed() {
        return None;
    }
    let srs = card.srs(mode);
    match srs.state {
        CardState::New => {
            if srs.intro_due <= end_of_day(now) {
                Some(Bucket::New)
            } else {
                None
            }
        }
        CardState::Learning | CardState::Relearning => {
            if srs.due <= now {
                Some(Bucket::Learning)
            } else {
                None
            }
        }
        CardState::Review => {
            if srs.due <= now {
                Some(Bucket::Review)
            } else if deck.cover_weekly
                && !covered_this_week(srs, now, true)
                && srs.due > end_of_week(now)
                && maintenance_slot(card.id, now) <= now
            {
                Some(Bucket::Maintenance)
            } else {
                None
            }
        }
    }
}

pub fn build_queue(deck: &Deck, cards: &[&Card], mode: StudyMode, now: i64) -> Vec<Queued> {
    let by_id: HashMap<u64, &Card> = cards.iter().map(|card| (card.id, *card)).collect();
    let mut items = Vec::new();
    for card in cards {
        if let Some(bucket) = ready_bucket(deck, card, mode, now) {
            items.push(Queued {
                card_id: card.id,
                bucket,
            });
        }
    }
    if !deck.cover_weekly {
        let limit = deck.new_per_day.max(1) as usize;
        let mut fresh: Vec<u64> = items
            .iter()
            .filter(|item| item.bucket == Bucket::New)
            .map(|item| item.card_id)
            .collect();
        fresh.sort_by_key(|id| {
            let card = by_id[id];
            (card.srs(mode).intro_due, *id)
        });
        if fresh.len() > limit {
            let skip: HashSet<u64> = fresh[limit..].iter().copied().collect();
            items.retain(|item| item.bucket != Bucket::New || !skip.contains(&item.card_id));
        }
    }
    items.sort_by_key(|item| sort_key(item, by_id[&item.card_id], mode, now));
    if mode == StudyMode::Typed {
        let split = items
            .iter()
            .position(|item| item.bucket != Bucket::Learning)
            .unwrap_or(items.len());
        let seed = (now as u64).wrapping_mul(deck.id | 1);
        shuffle(&mut items[split..], seed);
    }
    items
}

fn sort_key(item: &Queued, card: &Card, mode: StudyMode, now: i64) -> (u8, i64, i64) {
    let srs = card.srs(mode);
    let weak = -(mode_weakness(card, mode, now) * 1000.0).round() as i64;
    match item.bucket {
        Bucket::Learning => (0, srs.due, card.id as i64),
        Bucket::Review => (1, weak, srs.due),
        Bucket::Maintenance => (2, weak, card.id as i64),
        Bucket::New => (3, srs.intro_due, card.id as i64),
        Bucket::Practice => (4, weak, card.id as i64),
    }
}

pub fn practice_queue(cards: &[&Card], limit: usize, seed: u64) -> Vec<Queued> {
    let mut items: Vec<Queued> = cards
        .iter()
        .filter(|card| card.has_typed())
        .map(|card| Queued {
            card_id: card.id,
            bucket: Bucket::Practice,
        })
        .collect();
    shuffle(&mut items, seed);
    items.truncate(limit);
    items
}

pub fn shuffle<T>(items: &mut [T], mut seed: u64) {
    if items.len() < 2 {
        return;
    }
    seed |= 1;
    for index in (1..items.len()).rev() {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let swap_with = (seed >> 33) as usize % (index + 1);
        items.swap(index, swap_with);
    }
}

pub fn mode_weakness(card: &Card, mode: StudyMode, now: i64) -> f32 {
    match mode {
        StudyMode::Flash => display_pressure(&card.flash, now),
        StudyMode::Typed => typed_weakness(card, now),
    }
}

pub fn typed_weakness(card: &Card, now: i64) -> f32 {
    if !card.has_typed() {
        return 0.0;
    }
    let mut score = display_pressure(&card.typed, now);
    if card.typed_attempts > 0 {
        let recall = (1.0 - card.typed_avg).clamp(0.0, 1.0);
        score = (score * 0.55 + recall * 0.45).clamp(0.0, 1.0);
        if card.typed_avg < 0.75 {
            score = score.max(0.40 + (0.75 - card.typed_avg) * 0.8);
        }
    }
    score.clamp(0.0, 1.0)
}

#[allow(dead_code)]
pub fn card_weakness(card: &Card, now: i64) -> f32 {
    display_pressure(&card.flash, now).max(typed_weakness(card, now))
}

pub fn weak_note(card: &Card, now: i64) -> Option<WeakNote> {
    let flash = display_pressure(&card.flash, now);
    let typed = typed_weakness(card, now);
    if flash < 0.40 && typed < 0.40 {
        return None;
    }
    if card.has_typed() && typed >= flash {
        Some(WeakNote {
            score: typed,
            reason: reason_for(&card.typed, now, Some((card.typed_avg, card.typed_attempts))),
            mode: StudyMode::Typed,
        })
    } else {
        Some(WeakNote {
            score: flash,
            reason: reason_for(&card.flash, now, None),
            mode: StudyMode::Flash,
        })
    }
}

fn reason_for(srs: &crate::model::Srs, now: i64, typed: Option<(f32, u32)>) -> String {
    let mut best = ("Recall needs another pass".to_string(), 0.0_f32);
    if srs.fail_streak >= 2 {
        let score = srs.fail_streak as f32 + 1.0;
        best = (
            format!("Missed {} times in a row", srs.fail_streak),
            score,
        );
    }
    if srs.lapses >= 2 {
        let score = srs.lapses as f32 + 0.4;
        if score > best.1 {
            best = (format!("Lapsed {} times", srs.lapses), score);
        }
    }
    if srs.ease < 2.0 {
        let score = (2.5 - srs.ease) * 4.0;
        if score > best.1 {
            best = (format!("Ease has fallen to {:.1}", srs.ease), score);
        }
    }
    if matches!(srs.state, CardState::Review) {
        let days = timeutil::local_days_between(srs.due, now);
        if days >= 2 {
            let score = days as f32;
            if score > best.1 {
                best = (format!("Overdue by {days} days"), score);
            }
        }
    }
    if let Some((average, attempts)) = typed {
        if attempts > 0 && average < 0.75 {
            let score = (1.0 - average) * 8.0;
            if score > best.1 {
                best = (
                    format!("Typed answers average {}%", (average * 100.0).round() as i32),
                    score,
                );
            }
        }
    }
    best.0
}

pub fn later_today(deck: &Deck, card: &Card, mode: StudyMode, now: i64) -> bool {
    if mode == StudyMode::Typed && !card.has_typed() {
        return false;
    }
    if ready_bucket(deck, card, mode, now).is_some() {
        return false;
    }
    let due = next_due(card.id, card.srs(mode), deck.cover_weekly, now);
    due > now && due <= end_of_day(now)
}

pub fn deck_pulse(deck: &Deck, cards: &[&Card], now: i64) -> Pulse {
    let mut pulse = Pulse {
        cards: cards.len(),
        typed_cards: cards.iter().filter(|card| card.has_typed()).count(),
        ready_flash: build_queue(deck, cards, StudyMode::Flash, now).len(),
        ready_typed: build_queue(deck, cards, StudyMode::Typed, now).len(),
        later_flash: 0,
        later_typed: 0,
        flash_covered: 0,
        flash_total: 0,
        typed_covered: 0,
        typed_total: 0,
        planned: [0; 7],
    };
    for card in cards {
        if later_today(deck, card, StudyMode::Flash, now) {
            pulse.later_flash += 1;
        }
        if later_today(deck, card, StudyMode::Typed, now) {
            pulse.later_typed += 1;
        }
        if in_scope(&card.flash, deck.cover_weekly, now) {
            pulse.flash_total += 1;
            if covered_this_week(&card.flash, now, deck.cover_weekly) {
                pulse.flash_covered += 1;
            }
            note_plan(&mut pulse.planned, card.id, &card.flash, deck.cover_weekly, now);
        }
        if card.has_typed() && in_scope(&card.typed, deck.cover_weekly, now) {
            pulse.typed_total += 1;
            if covered_this_week(&card.typed, now, deck.cover_weekly) {
                pulse.typed_covered += 1;
            }
        }
    }
    pulse
}

fn note_plan(planned: &mut [u32; 7], card_id: u64, srs: &crate::model::Srs, weekly: bool, now: i64) {
    let covered = covered_this_week(srs, now, weekly);
    let due = next_due(card_id, srs, weekly, now);
    if covered && due > end_of_week(now) {
        return;
    }
    let index = if due <= now {
        timeutil::weekday_index(now, now)
    } else {
        timeutil::weekday_index(due, now)
    };
    if let Some(index) = index {
        planned[index] = planned[index].saturating_add(1);
    }
}

pub fn reviews_by_weekday(events: &[ReviewEvent], now: i64) -> [u32; 7] {
    let mut counts = [0u32; 7];
    for event in events {
        if let Some(index) = timeutil::weekday_index(event.at, now) {
            counts[index] = counts[index].saturating_add(1);
        }
    }
    counts
}

pub fn card_status(card: &Card, mode: StudyMode, now: i64) -> String {
    let srs = card.srs(mode);
    match srs.state {
        CardState::New => {
            if srs.intro_due <= end_of_day(now) {
                "New".into()
            } else {
                format!("New · {}", timeutil::relative_day(srs.intro_due, now))
            }
        }
        CardState::Learning | CardState::Relearning => {
            if srs.due <= now {
                "Learning".into()
            } else {
                format!("Learning · {}", timeutil::format_span(now, srs.due))
            }
        }
        CardState::Review => timeutil::format_when(srs.due, now),
    }
}

pub fn next_future_due(cards: &[&Card], mode: StudyMode, now: i64) -> Option<i64> {
    cards
        .iter()
        .filter_map(|card| {
            if mode == StudyMode::Typed && !card.has_typed() {
                return None;
            }
            let srs = card.srs(mode);
            let due = match srs.state {
                CardState::New => srs.intro_due,
                _ => srs.due,
            };
            (due > now).then_some(due)
        })
        .min()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Srs;

    fn deck(weekly: bool) -> Deck {
        Deck {
            id: 1,
            name: "Test".into(),
            created_at: 0,
            cover_weekly: weekly,
            new_per_day: 20,
            color: 0,
        }
    }

    fn card(id: u64) -> Card {
        Card {
            id,
            deck_id: 1,
            front: format!("front {id}"),
            back: "back".into(),
            question: String::new(),
            answer: String::new(),
            created_at: 0,
            updated_at: 0,
            flash: Srs::default(),
            typed: Srs::default(),
            typed_avg: 0.0,
            typed_attempts: 0,
        }
    }

    #[test]
    fn new_cards_spread_across_seven_days() {
        let now = timeutil::now_ms();
        let mut dues = Vec::new();
        for _ in 0..7 {
            dues.push(next_intro_due(&dues, now));
        }
        let mut days: Vec<i64> = dues
            .iter()
            .map(|due| timeutil::local_days_between(now, *due))
            .collect();
        days.sort();
        assert_eq!(days, vec![0, 1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn typed_queue_ignores_cards_without_a_question() {
        let now = timeutil::now_ms();
        let mut plain = card(1);
        plain.flash.intro_due = now;
        plain.flash.due = now;
        let mut typed = card(2);
        typed.question = "Hola?".into();
        typed.answer = "Hello".into();
        typed.typed.intro_due = now;
        typed.typed.due = now;
        let mut half = card(3);
        half.question = "Only a question".into();
        half.typed.intro_due = now;
        let cards = [&plain, &typed, &half];
        let queue = build_queue(&deck(true), &cards, StudyMode::Typed, now);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].card_id, 2);
        let flash = build_queue(&deck(true), &cards, StudyMode::Flash, now);
        assert_eq!(flash.len(), 3);
    }

    #[test]
    fn a_future_new_card_waits_for_its_day() {
        let now = timeutil::now_ms();
        let mut today = card(1);
        today.flash.intro_due = now;
        let mut later = card(2);
        later.flash.intro_due = add_local_days(start_of_day(now), 3);
        let cards = [&today, &later];
        let queue = build_queue(&deck(true), &cards, StudyMode::Flash, now);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].card_id, 1);
    }

    #[test]
    fn weekly_mode_pulls_a_far_card_into_this_week() {
        let now = timeutil::now_ms();
        let mut card = card(3);
        card.flash.state = CardState::Review;
        card.flash.interval_days = 40.0;
        card.flash.reps = 5;
        card.flash.due = add_local_days(now, 40);
        card.flash.success_at = Some(add_local_days(now, -20));
        assert!(!covered_this_week(&card.flash, now, true));
        let due = next_due(card.id, &card.flash, true, now);
        assert!(due <= end_of_week(now));
    }

    #[test]
    fn a_card_reviewed_this_week_can_rest_past_sunday() {
        let now = timeutil::now_ms();
        let mut card = card(8);
        card.flash.state = CardState::Review;
        card.flash.reps = 4;
        card.flash.interval_days = 40.0;
        card.flash.due = add_local_days(now, 40);
        card.flash.success_at = Some(now);
        assert!(covered_this_week(&card.flash, now, true));
        let due = next_due(card.id, &card.flash, true, now);
        assert!(due > end_of_week(now));
    }

    #[test]
    fn lapses_show_up_as_a_weak_card() {
        let now = timeutil::now_ms();
        let mut card = card(4);
        card.flash.state = CardState::Review;
        card.flash.ease = 1.6;
        card.flash.lapses = 4;
        card.flash.reps = 4;
        card.flash.due = now;
        let note = weak_note(&card, now).expect("weak");
        assert!(note.score >= 0.40);
        assert!(!note.reason.is_empty());
    }

    #[test]
    fn daily_new_card_cap_applies_when_weekly_coverage_is_off() {
        let now = timeutil::now_ms();
        let mut deck = deck(false);
        deck.new_per_day = 2;
        let mut cards = Vec::new();
        for id in 1..=5 {
            let mut card = card(id);
            card.flash.intro_due = now;
            cards.push(card);
        }
        let refs: Vec<&Card> = cards.iter().collect();
        let queue = build_queue(&deck, &refs, StudyMode::Flash, now);
        assert_eq!(queue.len(), 2);
    }
}
