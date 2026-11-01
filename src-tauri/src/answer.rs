//! Omnibox math, unit conversion and currency. Currency rates are fetched once a day.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OmniboxAnswer {
    pub title: String,
    pub url: String,
    /// True when Enter should leave the text in the address bar instead of navigating.
    pub answer: bool,
}

struct Rates {
    fetched: Instant,
    /// Units of this currency per 1 EUR. EUR itself is 1.
    per_eur: Vec<(String, f64)>,
}

static RATES: Mutex<Option<Rates>> = Mutex::new(None);

pub fn answer(query: &str) -> Option<OmniboxAnswer> {
    let query = query.trim();
    if query.is_empty() {
        return None;
    }
    if let Some(value) = eval_math(query) {
        let text = format_number(value);
        return Some(OmniboxAnswer {
            title: format!("{query} = {text}"),
            url: text,
            answer: true,
        });
    }
    if let Some((value, unit)) = convert_units(query) {
        let text = format!("{} {unit}", format_number(value));
        return Some(OmniboxAnswer {
            title: text.clone(),
            url: text,
            answer: true,
        });
    }
    if let Some((amount, from, to)) = parse_currency(query) {
        ensure_rates();
        let value = convert_currency(amount, &from, &to)?;
        let text = format!("{} {}", format_number(value), to.to_uppercase());
        return Some(OmniboxAnswer {
            title: format!("{amount} {} = {text}", from.to_uppercase()),
            url: text,
            answer: true,
        });
    }
    None
}

fn ensure_rates() {
    let fresh = RATES
        .lock()
        .expect("rates")
        .as_ref()
        .is_some_and(|rates| rates.fetched.elapsed() < Duration::from_secs(24 * 60 * 60));
    if fresh {
        return;
    }
    let Ok(body) = ureq::get("https://api.frankfurter.app/latest").call() else {
        return;
    };
    let Ok(body) = body.into_string() else {
        return;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) else {
        return;
    };
    let Some(map) = json.get("rates").and_then(|value| value.as_object()) else {
        return;
    };
    let mut per_eur = vec![("EUR".into(), 1.0)];
    for (code, value) in map {
        if let Some(rate) = value.as_f64() {
            per_eur.push((code.to_uppercase(), rate));
        }
    }
    *RATES.lock().expect("rates") = Some(Rates {
        fetched: Instant::now(),
        per_eur,
    });
}

fn convert_currency(amount: f64, from: &str, to: &str) -> Option<f64> {
    let rates = RATES.lock().expect("rates");
    let rates = rates.as_ref()?;
    let from = rates.per_eur.iter().find(|(code, _)| code == &from.to_uppercase())?.1;
    let to = rates.per_eur.iter().find(|(code, _)| code == &to.to_uppercase())?.1;
    if from == 0.0 {
        return None;
    }
    Some(amount * to / from)
}

fn parse_currency(query: &str) -> Option<(f64, String, String)> {
    let mut parts = query.split_whitespace();
    let amount: f64 = parts.next()?.parse().ok()?;
    let from = parts.next()?.to_string();
    let mid = parts.next()?;
    if !matches!(mid.to_ascii_lowercase().as_str(), "to" | "in") {
        return None;
    }
    let to = parts.next()?.to_string();
    if parts.next().is_some() || from.len() != 3 || to.len() != 3 {
        return None;
    }
    if !from.chars().all(|ch| ch.is_ascii_alphabetic()) || !to.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return None;
    }
    Some((amount, from, to))
}

fn convert_units(query: &str) -> Option<(f64, String)> {
    let mut parts = query.split_whitespace();
    let amount: f64 = parts.next()?.parse().ok()?;
    let from = parts.next()?.to_ascii_lowercase();
    let mid = parts.next()?.to_ascii_lowercase();
    if mid != "to" && mid != "in" {
        return None;
    }
    let to = parts.next()?.to_ascii_lowercase();
    if parts.next().is_some() {
        return None;
    }
    let (from_base, from_factor, from_offset) = unit(&from)?;
    let (to_base, to_factor, to_offset) = unit(&to)?;
    if from_base != to_base {
        return None;
    }
    let base = amount * from_factor + from_offset;
    let value = (base - to_offset) / to_factor;
    Some((value, to))
}

/// `(family, multiply-to-base, add-after-multiply)`.
fn unit(name: &str) -> Option<(&'static str, f64, f64)> {
    Some(match name {
        "m" => ("m", 1.0, 0.0),
        "cm" => ("m", 0.01, 0.0),
        "mm" => ("m", 0.001, 0.0),
        "km" => ("m", 1000.0, 0.0),
        "ft" => ("m", 0.3048, 0.0),
        "in" => ("m", 0.0254, 0.0),
        "mi" => ("m", 1609.344, 0.0),
        "g" => ("g", 1.0, 0.0),
        "kg" => ("g", 1000.0, 0.0),
        "lb" => ("g", 453.59237, 0.0),
        "oz" => ("g", 28.349523125, 0.0),
        "ml" => ("ml", 1.0, 0.0),
        "l" => ("ml", 1000.0, 0.0),
        "gal" => ("ml", 3785.411784, 0.0),
        "km/h" | "kph" => ("kph", 1.0, 0.0),
        "mph" => ("kph", 1.609344, 0.0),
        "c" => ("temp", 1.0, 0.0),
        "f" => ("temp", 5.0 / 9.0, -32.0 * 5.0 / 9.0),
        _ => return None,
    })
}

fn format_number(value: f64) -> String {
    if !value.is_finite() {
        return "0".into();
    }
    let rounded = (value * 1_000_000.0).round() / 1_000_000.0;
    let text = format!("{rounded}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn eval_math(query: &str) -> Option<f64> {
    if !query.chars().any(|ch| "+-*/^%()".contains(ch)) {
        return None;
    }
    if query.chars().any(|ch| !(ch.is_ascii_digit() || ch == '.' || ch.is_whitespace() || "+-*/^%()".contains(ch))) {
        return None;
    }
    let mut parser = Parser { input: query.as_bytes(), pos: 0 };
    let value = parser.parse_expr()?;
    parser.skip();
    if parser.pos != parser.input.len() {
        return None;
    }
    value.is_finite().then_some(value)
}

struct Parser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn parse_expr(&mut self) -> Option<f64> {
        let mut value = self.parse_term()?;
        loop {
            self.skip();
            match self.peek() {
                Some(b'+') => {
                    self.pos += 1;
                    value += self.parse_term()?;
                }
                Some(b'-') => {
                    self.pos += 1;
                    value -= self.parse_term()?;
                }
                _ => return Some(value),
            }
        }
    }

    fn parse_term(&mut self) -> Option<f64> {
        let mut value = self.parse_power()?;
        loop {
            self.skip();
            match self.peek() {
                Some(b'*') => {
                    self.pos += 1;
                    value *= self.parse_power()?;
                }
                Some(b'/') => {
                    self.pos += 1;
                    let divisor = self.parse_power()?;
                    if divisor == 0.0 {
                        return None;
                    }
                    value /= divisor;
                }
                _ => return Some(value),
            }
        }
    }

    fn parse_power(&mut self) -> Option<f64> {
        let mut value = self.parse_unary()?;
        self.skip();
        if self.peek() == Some(b'^') {
            self.pos += 1;
            let exp = self.parse_power()?;
            value = value.powf(exp);
        }
        Some(value)
    }

    fn parse_unary(&mut self) -> Option<f64> {
        self.skip();
        if self.peek() == Some(b'-') {
            self.pos += 1;
            return self.parse_unary().map(|value| -value);
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Option<f64> {
        self.skip();
        if self.peek() == Some(b'(') {
            self.pos += 1;
            let value = self.parse_expr()?;
            self.skip();
            if self.peek() != Some(b')') {
                return None;
            }
            self.pos += 1;
            return Some(value);
        }
        let start = self.pos;
        while matches!(self.peek(), Some(b'0'..=b'9' | b'.')) {
            self.pos += 1;
        }
        if start == self.pos {
            return None;
        }
        let text = std::str::from_utf8(&self.input[start..self.pos]).ok()?;
        let mut value: f64 = text.parse().ok()?;
        self.skip();
        if self.peek() == Some(b'%') {
            self.pos += 1;
            value /= 100.0;
        }
        Some(value)
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn skip(&mut self) {
        while self.peek() == Some(b' ') {
            self.pos += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiplication_and_parentheses() {
        let answer = answer("2*(3+4)").unwrap();
        assert_eq!(answer.url, "14");
        assert!(answer.answer);
    }

    #[test]
    fn kilometres_to_miles() {
        let answer = answer("10 km to mi").unwrap();
        assert!(answer.url.starts_with("6.213"));
        assert!(answer.url.ends_with(" mi"));
    }

    #[test]
    fn a_plain_sentence_is_not_math() {
        assert!(answer("react hooks").is_none());
        assert!(eval_math("https://example.com").is_none());
    }

    #[test]
    fn fahrenheit_to_celsius() {
        let (value, unit) = convert_units("32 f to c").expect("conversion");
        assert!(value.abs() < 0.001, "{value} {unit}");
        assert_eq!(unit, "c");
    }
}
