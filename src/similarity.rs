//! Scores a typed answer against the card.
//! The blend is spelling, weighted word coverage, and word order.
//! `|` separates other accepted answers.

use crate::model::Rating;
use unicode_normalization::UnicodeNormalization;

#[derive(Clone, Debug)]
pub struct RecallScore {
    pub similarity: f32,
    pub spelling: f32,
    pub coverage: f32,
    pub order: f32,
    pub rating: Rating,
    pub missing: Vec<String>,
}

impl RecallScore {
    pub fn percent(&self) -> i32 {
        (self.similarity * 100.0).round().clamp(0.0, 100.0) as i32
    }

    pub fn headline(&self) -> &'static str {
        match self.rating {
            Rating::Easy => "Very close",
            Rating::Good => "Close",
            Rating::Hard => "Partly there",
            Rating::Again => "Not yet",
        }
    }
}

pub fn rating_for(similarity: f32) -> Rating {
    if similarity >= 0.90 {
        Rating::Easy
    } else if similarity >= 0.74 {
        Rating::Good
    } else if similarity >= 0.50 {
        Rating::Hard
    } else {
        Rating::Again
    }
}

pub fn score_answer(expected: &str, given: &str) -> RecallScore {
    let options: Vec<&str> = if expected.contains('|') {
        expected
            .split('|')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect()
    } else {
        vec![expected]
    };
    if options.is_empty() {
        return blank();
    }
    options
        .into_iter()
        .map(|option| score_one(option, given))
        .max_by(|a, b| a.similarity.total_cmp(&b.similarity))
        .unwrap_or_else(blank)
}

fn blank() -> RecallScore {
    RecallScore {
        similarity: 0.0,
        spelling: 0.0,
        coverage: 0.0,
        order: 0.0,
        rating: Rating::Again,
        missing: Vec::new(),
    }
}

fn score_one(expected: &str, given: &str) -> RecallScore {
    let expected_norm = normalize(expected);
    let given_norm = normalize(given);
    if expected_norm.is_empty() {
        return blank();
    }
    let expected_tokens = tokenize(&expected_norm);
    if given_norm.is_empty() {
        return RecallScore {
            similarity: 0.0,
            spelling: 0.0,
            coverage: 0.0,
            order: 0.0,
            rating: Rating::Again,
            missing: content_words(&expected_tokens),
        };
    }
    if expected_norm == given_norm {
        return RecallScore {
            similarity: 1.0,
            spelling: 1.0,
            coverage: 1.0,
            order: 1.0,
            rating: Rating::Easy,
            missing: Vec::new(),
        };
    }

    let given_tokens = tokenize(&given_norm);
    let spelling_raw = char_ratio(&expected_norm, &given_norm);
    let spelling_plain = char_ratio(&strip_marks(&expected_norm), &strip_marks(&given_norm));
    let spelling = (0.70 * spelling_raw + 0.30 * spelling_plain).clamp(0.0, 1.0);
    let (f1, mut coverage, mut missing) = weighted_overlap(&expected_tokens, &given_tokens);
    let order = token_order(&expected_tokens, &given_tokens);

    let mut similarity = if expected_tokens.len() == 1 && given_tokens.len() <= 2 {
        let best = given_tokens
            .iter()
            .map(|token| {
                0.70 * char_ratio(token, &expected_tokens[0])
                    + 0.30 * char_ratio(&strip_marks(token), &strip_marks(&expected_tokens[0]))
            })
            .fold(0.0_f32, f32::max);
        let extra = given_tokens.len().saturating_sub(1) as f32 * 0.06;
        best * (1.0 - extra).max(0.0)
    } else if expected_tokens.len() <= 3 {
        0.62 * spelling + 0.38 * f1
    } else {
        0.26 * spelling + 0.52 * f1 + 0.22 * order
    };

    let expected_numbers = extract_numbers(&expected_norm);
    if !expected_numbers.is_empty() {
        let given_numbers = extract_numbers(&given_norm);
        let numeric = numeric_score(&expected_numbers, &given_numbers);
        if expected_tokens.len() <= 2 && expected_numbers.len() >= expected_tokens.len() {
            similarity = 0.82 * numeric + 0.18 * spelling;
            if numeric >= 0.99 {
                missing.clear();
                coverage = 1.0;
            }
        } else {
            similarity *= 0.75 + 0.25 * numeric;
        }
    }

    if coverage >= 0.999 && spelling >= 0.92 {
        similarity = similarity.max(0.96);
    }
    let similarity = similarity.clamp(0.0, 1.0);
    RecallScore {
        similarity,
        spelling,
        coverage,
        order,
        rating: rating_for(similarity),
        missing,
    }
}

fn normalize(input: &str) -> String {
    let folded = input.nfkc().collect::<String>().to_lowercase();
    let chars: Vec<char> = folded.chars().collect();
    let mut out = String::new();
    let mut pending_space = false;
    for (index, ch) in chars.iter().copied().enumerate() {
        if ch == '\'' || ch == '’' {
            continue;
        }
        if ch == '.' {
            let prev_digit = out.chars().last().is_some_and(|prev| prev.is_ascii_digit());
            let next_digit = chars
                .get(index + 1)
                .is_some_and(|next| next.is_ascii_digit());
            if prev_digit && next_digit {
                pending_space = false;
                out.push('.');
                continue;
            }
        }
        if ch.is_alphanumeric() {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            out.push(ch);
        } else {
            pending_space = true;
        }
    }
    out
}

fn tokenize(normalized: &str) -> Vec<String> {
    normalized.split_whitespace().map(str::to_string).collect()
}

fn token_weight(token: &str) -> f32 {
    match token {
        "the" | "a" | "an" | "of" | "to" | "and" | "or" | "in" | "on" | "for" | "is" | "are"
        | "was" | "were" | "be" | "it" | "that" | "this" | "with" | "as" | "at" | "by" | "from"
        | "its" => 0.25,
        _ => 1.0,
    }
}

fn content_words(tokens: &[String]) -> Vec<String> {
    let mut missing = Vec::new();
    for token in tokens {
        if token_weight(token) >= 1.0 && !missing.contains(token) {
            missing.push(token.clone());
        }
    }
    missing.truncate(12);
    missing
}

fn weighted_overlap(expected: &[String], given: &[String]) -> (f32, f32, Vec<String>) {
    let mut available: Vec<(String, i32)> = Vec::new();
    for token in expected {
        if let Some(slot) = available.iter_mut().find(|(name, _)| name == token) {
            slot.1 += 1;
        } else {
            available.push((token.clone(), 1));
        }
    }
    let expected_weight: f32 = expected.iter().map(|token| token_weight(token)).sum();
    let given_weight: f32 = given.iter().map(|token| token_weight(token)).sum();
    let mut matched = 0.0;
    for token in given {
        if let Some(slot) = available.iter_mut().find(|(name, count)| *count > 0 && name == token) {
            slot.1 -= 1;
            matched += token_weight(token);
            continue;
        }
        if let Some(slot) = available.iter_mut().find(|(name, count)| {
            *count > 0 && strip_marks(name) == strip_marks(token)
        }) {
            slot.1 -= 1;
            matched += token_weight(token) * 0.9;
        }
    }
    let mut missing = Vec::new();
    for (token, count) in &available {
        if *count > 0 && token_weight(token) >= 1.0 && !missing.contains(token) {
            missing.push(token.clone());
        }
    }
    missing.truncate(12);
    let precision = if given_weight <= f32::EPSILON {
        0.0
    } else {
        matched / given_weight
    };
    let recall = if expected_weight <= f32::EPSILON {
        0.0
    } else {
        matched / expected_weight
    };
    let f1 = if precision + recall <= f32::EPSILON {
        0.0
    } else {
        2.0 * precision * recall / (precision + recall)
    };
    (f1.clamp(0.0, 1.0), recall.clamp(0.0, 1.0), missing)
}

fn token_order(expected: &[String], given: &[String]) -> f32 {
    if expected.is_empty() {
        return 0.0;
    }
    lcs_len(expected, given) as f32 / expected.len() as f32
}

fn lcs_len(a: &[String], b: &[String]) -> usize {
    let mut previous = vec![0usize; b.len() + 1];
    let mut current = vec![0usize; b.len() + 1];
    for left in a {
        for (index, right) in b.iter().enumerate() {
            if left == right || strip_marks(left) == strip_marks(right) {
                current[index + 1] = previous[index] + 1;
            } else {
                current[index + 1] = current[index].max(previous[index + 1]);
            }
        }
        std::mem::swap(&mut previous, &mut current);
        current.fill(0);
    }
    previous[b.len()]
}

fn char_ratio(a: &str, b: &str) -> f32 {
    if a == b {
        return 1.0;
    }
    let a: Vec<char> = a.chars().take(4000).collect();
    let b: Vec<char> = b.chars().take(4000).collect();
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let distance = levenshtein(&a, &b);
    let scale = a.len().max(b.len()) as f32;
    (1.0 - distance as f32 / scale).clamp(0.0, 1.0)
}

fn levenshtein(a: &[char], b: &[char]) -> usize {
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0; b.len() + 1];
    for (i, left) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, right) in b.iter().enumerate() {
            let cost = if left == right { 0 } else { 1 };
            current[j + 1] = (previous[j + 1] + 1)
                .min(current[j] + 1)
                .min(previous[j] + cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

fn strip_marks(input: &str) -> String {
    input.nfd().filter(|ch| !is_combining(*ch)).collect()
}

fn is_combining(ch: char) -> bool {
    matches!(
        ch as u32,
        0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F
    )
}

fn extract_numbers(input: &str) -> Vec<f64> {
    let chars: Vec<char> = input.chars().collect();
    let mut numbers = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let start_digit = chars[index].is_ascii_digit()
            || (chars[index] == '.'
                && index + 1 < chars.len()
                && chars[index + 1].is_ascii_digit());
        if !start_digit {
            index += 1;
            continue;
        }
        let start = index;
        while index < chars.len() && (chars[index].is_ascii_digit() || chars[index] == '.') {
            index += 1;
        }
        let token: String = chars[start..index].iter().collect();
        if let Ok(value) = token.parse::<f64>() {
            if value.is_finite() {
                numbers.push(value);
            }
        }
    }
    numbers
}

fn numeric_score(expected: &[f64], given: &[f64]) -> f32 {
    if expected.is_empty() {
        return 1.0;
    }
    if given.is_empty() {
        return 0.0;
    }
    let mut total = 0.0;
    for value in expected {
        let best = given
            .iter()
            .map(|candidate| {
                let relative = (candidate - value).abs() / value.abs().max(1.0);
                if relative < 0.005 {
                    1.0
                } else if relative < 0.02 {
                    0.85
                } else if relative < 0.10 {
                    0.55
                } else {
                    0.0
                }
            })
            .fold(0.0_f32, f32::max);
        total += best;
    }
    total / expected.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_and_punctuation_are_exact() {
        let score = score_answer("Mitochondria", "mitochondria!");
        assert!(score.similarity >= 0.99);
        assert_eq!(score.rating, Rating::Easy);
    }

    #[test]
    fn a_one_letter_typo_still_counts_as_close() {
        let score = score_answer("Paris", "Pari");
        assert!(score.similarity >= 0.74, "similarity {}", score.similarity);
        assert!(matches!(score.rating, Rating::Good | Rating::Easy));
    }

    #[test]
    fn an_unrelated_answer_is_a_miss() {
        let score = score_answer("Paris", "Tokyo");
        assert!(score.similarity < 0.50, "similarity {}", score.similarity);
        assert_eq!(score.rating, Rating::Again);
    }

    #[test]
    fn an_empty_answer_is_a_miss() {
        let score = score_answer("Paris", "   ");
        assert_eq!(score.rating, Rating::Again);
        assert!(score.missing.iter().any(|word| word == "paris"));
    }

    #[test]
    fn a_missing_keyword_is_called_out() {
        let score = score_answer(
            "Mitochondria is the powerhouse of the cell",
            "it is the powerhouse of the cell",
        );
        assert!(score.similarity < 0.90, "similarity {}", score.similarity);
        assert!(score.missing.iter().any(|word| word == "mitochondria"));
        assert_ne!(score.rating, Rating::Easy);
    }

    #[test]
    fn the_full_sentence_scores_as_easy() {
        let score = score_answer(
            "Mitochondria is the powerhouse of the cell",
            "mitochondria is the powerhouse of the cell",
        );
        assert_eq!(score.rating, Rating::Easy);
    }

    #[test]
    fn either_alternative_is_accepted() {
        let color = score_answer("color|colour", "colour");
        let colour = score_answer("color | colour", "color");
        assert_eq!(color.rating, Rating::Easy);
        assert_eq!(colour.rating, Rating::Easy);
    }

    #[test]
    fn close_numbers_score_well() {
        let score = score_answer("3.14", "3.140");
        assert!(score.similarity >= 0.90, "similarity {}", score.similarity);
        assert_eq!(score.rating, Rating::Easy);
    }
}
