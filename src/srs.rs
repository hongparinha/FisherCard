//! Spaced repetition close to Anki's SM-2.
//! Weekly decks cap the gap at 7 days so the whole deck gets a turn.
//! Low ease, lapses, and a miss streak pull a card back sooner than that.

use crate::model::{CardState, Rating, Srs};
use crate::timeutil::{self, add_local_days};

const EASE_MIN: f32 = 1.3;
const EASE_MAX: f32 = 3.0;
const LEARN_STEPS_MS: [i64; 2] = [60_000, 600_000];
const RELEARN_MS: i64 = 600_000;

pub fn apply_review(srs: &mut Srs, card_id: u64, rating: Rating, now: i64, weekly: bool) {
    match srs.state {
        CardState::New | CardState::Learning => apply_fresh(srs, card_id, rating, now, weekly),
        CardState::Review => apply_review_state(srs, card_id, rating, now, weekly),
        CardState::Relearning => apply_relearn(srs, card_id, rating, now, weekly),
    }
}

pub fn preview_due(srs: &Srs, card_id: u64, rating: Rating, now: i64, weekly: bool) -> i64 {
    let mut copy = srs.clone();
    apply_review(&mut copy, card_id, rating, now, weekly);
    copy.due
}

pub fn preview_label(srs: &Srs, card_id: u64, rating: Rating, now: i64, weekly: bool) -> String {
    let due = preview_due(srs, card_id, rating, now, weekly);
    timeutil::format_span(now, due)
}

fn apply_fresh(srs: &mut Srs, card_id: u64, rating: Rating, now: i64, weekly: bool) {
    match rating {
        Rating::Again => {
            srs.fail_streak = srs.fail_streak.saturating_add(1);
            srs.state = CardState::Learning;
            srs.step = 0;
            srs.due = now + LEARN_STEPS_MS[0];
        }
        Rating::Hard => {
            let was_new = matches!(srs.state, CardState::New);
            let step = if was_new { 0 } else { srs.step as usize };
            let step = step.min(LEARN_STEPS_MS.len() - 1);
            let next = (step + 1).min(LEARN_STEPS_MS.len() - 1);
            let delay = (LEARN_STEPS_MS[step] + LEARN_STEPS_MS[next]) / 2;
            srs.fail_streak = 0;
            srs.state = CardState::Learning;
            srs.step = step as u8;
            srs.due = now + delay;
            srs.success_at = Some(now);
        }
        Rating::Good => {
            if matches!(srs.state, CardState::New) {
                srs.fail_streak = 0;
                srs.state = CardState::Learning;
                srs.step = 0;
                srs.due = now + LEARN_STEPS_MS[0];
                srs.success_at = Some(now);
            } else {
                let next = srs.step as usize + 1;
                if next >= LEARN_STEPS_MS.len() {
                    schedule(srs, card_id, 1.0, now, weekly, true);
                } else {
                    srs.fail_streak = 0;
                    srs.step = next as u8;
                    srs.state = CardState::Learning;
                    srs.due = now + LEARN_STEPS_MS[next];
                    srs.success_at = Some(now);
                }
            }
        }
        Rating::Easy => {
            schedule(srs, card_id, 4.0, now, weekly, true);
        }
    }
}

fn apply_review_state(srs: &mut Srs, card_id: u64, rating: Rating, now: i64, weekly: bool) {
    match rating {
        Rating::Again => {
            srs.lapses = srs.lapses.saturating_add(1);
            srs.fail_streak = srs.fail_streak.saturating_add(1);
            srs.ease = (srs.ease - 0.20).max(EASE_MIN);
            srs.interval_days = (srs.interval_days * 0.5).max(1.0);
            srs.state = CardState::Relearning;
            srs.step = 0;
            srs.due = now + RELEARN_MS;
        }
        Rating::Hard => {
            srs.ease = (srs.ease - 0.15).max(EASE_MIN);
            let ideal = (srs.interval_days * 1.2).max(1.0);
            schedule(srs, card_id, ideal, now, weekly, false);
        }
        Rating::Good => {
            let ideal = if srs.reps <= 1 {
                (srs.interval_days * srs.ease).max(6.0)
            } else {
                (srs.interval_days * srs.ease).max(srs.interval_days + 1.0)
            };
            schedule(srs, card_id, ideal, now, weekly, false);
        }
        Rating::Easy => {
            srs.ease = (srs.ease + 0.15).min(EASE_MAX);
            let ideal = (srs.interval_days.max(1.0) * srs.ease * 1.3).max(4.0);
            schedule(srs, card_id, ideal, now, weekly, false);
        }
    }
}

fn apply_relearn(srs: &mut Srs, card_id: u64, rating: Rating, now: i64, weekly: bool) {
    match rating {
        Rating::Again => {
            srs.fail_streak = srs.fail_streak.saturating_add(1);
            srs.state = CardState::Relearning;
            srs.step = 0;
            srs.due = now + RELEARN_MS;
        }
        Rating::Hard => {
            srs.fail_streak = 0;
            srs.state = CardState::Relearning;
            srs.due = now + RELEARN_MS;
            srs.success_at = Some(now);
        }
        Rating::Good => {
            let days = srs.interval_days.max(1.0);
            schedule(srs, card_id, days, now, weekly, false);
        }
        Rating::Easy => {
            srs.ease = (srs.ease + 0.15).min(EASE_MAX);
            let days = (srs.interval_days.max(1.0) * 1.3).max(1.0);
            schedule(srs, card_id, days, now, weekly, false);
        }
    }
}

fn schedule(srs: &mut Srs, card_id: u64, ideal_days: f32, now: i64, weekly: bool, first_graduate: bool) {
    let pressure = schedule_pressure(srs);
    let days = finalize_days(card_id, ideal_days, weekly, pressure);
    srs.interval_days = days;
    srs.due = add_local_days(now, days as i64);
    srs.state = CardState::Review;
    srs.step = 0;
    srs.success_at = Some(now);
    srs.fail_streak = 0;
    if first_graduate {
        srs.reps = 1;
    } else {
        srs.reps = srs.reps.saturating_add(1);
    }
}

/// How hard this card is pressing on the schedule. 0 is comfortable, 1 is shaky.
pub fn schedule_pressure(srs: &Srs) -> f32 {
    let ease_part = ((2.5 - srs.ease) / 1.2).clamp(0.0, 1.0);
    let streak_part = (srs.fail_streak as f32 / 2.0).clamp(0.0, 1.0);
    let lapse_part = (srs.lapses as f32 / 5.0).clamp(0.0, 1.0);
    0.50 * streak_part + 0.30 * ease_part + 0.20 * lapse_part
}

fn finalize_days(card_id: u64, ideal: f32, weekly: bool, pressure: f32) -> f32 {
    let capped = apply_caps(ideal.max(1.0), weekly, pressure);
    let fuzzed = apply_caps(fuzz_days(card_id, capped), weekly, pressure);
    fuzzed.round().max(1.0)
}

fn apply_caps(days: f32, weekly: bool, pressure: f32) -> f32 {
    let mut days = days;
    if weekly {
        days = days.min(7.0);
    }
    if pressure >= 0.65 {
        days = days.min(1.0);
    } else if pressure >= 0.30 {
        days = days.min(3.0);
    }
    days
}

fn fuzz_days(card_id: u64, days: f32) -> f32 {
    if days < 2.5 {
        return days;
    }
    let hash = card_id.wrapping_mul(2654435761) % 1000;
    let factor = 0.95 + (hash as f32 / 1000.0) * 0.10;
    days * factor
}

/// What the Brain screen uses. Includes overdue days, unlike the schedule cap.
pub fn display_pressure(srs: &Srs, now: i64) -> f32 {
    if matches!(srs.state, CardState::New) && srs.lapses == 0 && srs.fail_streak == 0 {
        return 0.0;
    }
    let ease_part = ((2.5 - srs.ease) / 1.2).clamp(0.0, 1.0);
    let lapse_part = (srs.lapses as f32 / 3.0).clamp(0.0, 1.0);
    let streak_part = (srs.fail_streak as f32 / 2.0).clamp(0.0, 1.0);
    let overdue = if matches!(srs.state, CardState::Review) {
        (timeutil::local_days_between(srs.due, now).max(0) as f32 / 5.0).clamp(0.0, 1.0)
    } else if srs.fail_streak > 0 {
        0.25
    } else {
        0.0
    };
    (0.30 * ease_part + 0.28 * lapse_part + 0.24 * streak_part + 0.18 * overdue).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn review_card() -> Srs {
        Srs {
            state: CardState::Review,
            ease: 2.5,
            interval_days: 10.0,
            reps: 4,
            due: 1_700_000_000_000,
            ..Srs::default()
        }
    }

    #[test]
    fn three_goods_graduate_a_new_card() {
        let mut srs = Srs::default();
        let now = 1_700_000_000_000;
        apply_review(&mut srs, 1, Rating::Good, now, true);
        assert_eq!(srs.state, CardState::Learning);
        let due = srs.due;
        apply_review(&mut srs, 1, Rating::Good, due, true);
        assert_eq!(srs.state, CardState::Learning);
        let due = srs.due;
        apply_review(&mut srs, 1, Rating::Good, due, true);
        assert_eq!(srs.state, CardState::Review);
        assert!(srs.interval_days >= 1.0);
        assert!(srs.success_at.is_some());
    }

    #[test]
    fn easy_graduates_immediately() {
        let mut srs = Srs::default();
        let now = 1_700_000_000_000;
        apply_review(&mut srs, 3, Rating::Easy, now, true);
        assert_eq!(srs.state, CardState::Review);
        assert!((1.0..=7.0).contains(&srs.interval_days));
    }

    #[test]
    fn again_on_a_new_card_does_not_count_as_a_lapse() {
        let mut srs = Srs::default();
        apply_review(&mut srs, 1, Rating::Again, 1_700_000_000_000, true);
        assert_eq!(srs.state, CardState::Learning);
        assert_eq!(srs.lapses, 0);
        assert_eq!(srs.fail_streak, 1);
    }

    #[test]
    fn again_on_a_review_drops_ease_and_relearns() {
        let mut srs = review_card();
        let now = srs.due;
        apply_review(&mut srs, 9, Rating::Again, now, true);
        assert_eq!(srs.state, CardState::Relearning);
        assert_eq!(srs.lapses, 1);
        assert!((srs.ease - 2.3).abs() < 0.001);
        assert!(srs.due > now);
    }

    #[test]
    fn ease_never_falls_below_the_floor() {
        let mut srs = review_card();
        let now = 1_700_000_000_000;
        for _ in 0..30 {
            srs.state = CardState::Review;
            srs.interval_days = 4.0;
            apply_review(&mut srs, 2, Rating::Again, now, true);
        }
        assert!((srs.ease - 1.3).abs() < 0.001);
    }

    #[test]
    fn weekly_coverage_caps_a_healthy_card_at_seven_days() {
        let mut srs = review_card();
        let now = 1_700_000_000_000;
        apply_review(&mut srs, 11, Rating::Good, now, true);
        assert_eq!(srs.interval_days, 7.0);
        assert_eq!(timeutil::local_days_between(now, srs.due), 7);
    }

    #[test]
    fn weekly_coverage_can_be_turned_off() {
        let mut srs = review_card();
        let now = 1_700_000_000_000;
        apply_review(&mut srs, 11, Rating::Good, now, false);
        assert!((20.0..=30.0).contains(&srs.interval_days));
    }

    #[test]
    fn a_shaky_card_comes_back_within_three_days() {
        let mut srs = Srs {
            state: CardState::Review,
            ease: 1.3,
            interval_days: 10.0,
            reps: 5,
            lapses: 6,
            fail_streak: 0,
            due: 1_700_000_000_000,
            ..Srs::default()
        };
        let due = srs.due;
        apply_review(&mut srs, 4, Rating::Good, due, true);
        assert_eq!(srs.interval_days, 3.0);
    }

    #[test]
    fn a_miss_streak_comes_back_tomorrow() {
        let mut srs = Srs {
            state: CardState::Review,
            ease: 1.3,
            interval_days: 10.0,
            reps: 5,
            lapses: 6,
            fail_streak: 3,
            due: 1_700_000_000_000,
            ..Srs::default()
        };
        let due = srs.due;
        apply_review(&mut srs, 4, Rating::Good, due, true);
        assert_eq!(srs.interval_days, 1.0);
    }

    #[test]
    fn preview_matches_the_applied_due_time() {
        let srs = review_card();
        let now = 1_700_000_000_000;
        let preview = preview_due(&srs, 7, Rating::Hard, now, true);
        let mut applied = srs.clone();
        apply_review(&mut applied, 7, Rating::Hard, now, true);
        assert_eq!(preview, applied.due);
    }
}
