//! Dollars, without a float in the middle.
//!
//! Mercury talks in decimal dollars — `10.20` means ten dollars twenty. That is
//! friendlier than integer cents until you do arithmetic on it, at which point
//! binary floating point starts losing pennies. So an amount is parsed straight
//! from its digits into an integer number of cents, and only turned back into a
//! decimal at the edges: once for display, once when writing the request.

use anyhow::{bail, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Money(i64);

impl Money {
    pub fn cents(&self) -> i64 {
        self.0
    }

    /// Parse what a person types: `1234.56`, `$1,234.56`, `5k`, `1.5m`.
    ///
    /// Three decimal places is a typo, not a rounding instruction, so it fails.
    pub fn parse(input: &str) -> Result<Self> {
        let cleaned: String = input.trim().trim_start_matches("USD").replace(['$', ',', '_', ' '], "");
        let negative = cleaned.starts_with('-');
        let cleaned = cleaned.trim_start_matches(['-', '+']).to_string();
        if cleaned.is_empty() {
            bail!("no amount given");
        }

        let (digits, multiplier) = match cleaned.chars().last() {
            Some('k') | Some('K') => (&cleaned[..cleaned.len() - 1], 1_000),
            Some('m') | Some('M') => (&cleaned[..cleaned.len() - 1], 1_000_000),
            _ => (cleaned.as_str(), 1),
        };

        let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
        if fraction.len() > 2 {
            bail!("'{}' has more than 2 decimal places", input.trim());
        }
        let valid = |part: &str| part.chars().all(|c| c.is_ascii_digit());
        if !valid(whole) || !valid(fraction) || (whole.is_empty() && fraction.is_empty()) {
            bail!("'{}' is not an amount", input.trim());
        }

        let dollars: i64 = match whole.is_empty() {
            true => 0,
            false => whole.parse()?,
        };
        let cents: i64 = match fraction.len() {
            0 => 0,
            1 => fraction.parse::<i64>()? * 10,
            _ => fraction.parse()?,
        };
        let total = (dollars * 100 + cents) * multiplier;
        Ok(Self(match negative {
            true => -total,
            false => total,
        }))
    }

    /// Whatever Mercury sent, read back exactly. Its JSON numbers are decimal
    /// dollars with at most two places, so the printed form is the honest one.
    pub fn from_json(value: &serde_json::Value) -> Option<Self> {
        match value {
            serde_json::Value::Number(number) => Self::parse(&number.to_string()).ok(),
            serde_json::Value::String(text) => Self::parse(text).ok(),
            _ => None,
        }
    }

    /// `10.20` — the decimal the API wants, built from the integer, never a float.
    pub fn to_api(self) -> serde_json::Value {
        let sign = match self.0 < 0 {
            true => "-",
            false => "",
        };
        let literal = format!("{sign}{}.{:02}", (self.0 / 100).abs(), (self.0 % 100).abs());
        serde_json::from_str(&literal).unwrap_or(serde_json::Value::Null)
    }

    /// `$1,234.56` — for reading, never for sending.
    pub fn display(&self) -> String {
        let whole = (self.0 / 100).abs().to_string();
        let mut grouped = String::new();
        for (index, digit) in whole.chars().enumerate() {
            if index > 0 && (whole.len() - index).is_multiple_of(3) {
                grouped.push(',');
            }
            grouped.push(digit);
        }
        let sign = match self.0 < 0 {
            true => "-",
            false => "",
        };
        format!("{sign}${grouped}.{:02}", (self.0 % 100).abs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn typed_amounts_become_exact_cents() {
        assert_eq!(Money::parse("10.20").unwrap().cents(), 1020);
        assert_eq!(Money::parse("$1,234.56").unwrap().cents(), 123_456);
        assert_eq!(Money::parse("5000").unwrap().cents(), 500_000);
        assert_eq!(Money::parse("5k").unwrap().cents(), 500_000);
        assert_eq!(Money::parse("1.5k").unwrap().cents(), 150_000);
        assert_eq!(Money::parse(".99").unwrap().cents(), 99);
        assert_eq!(Money::parse("-42.50").unwrap().cents(), -4250);
    }

    #[test]
    fn a_typo_is_refused_rather_than_rounded() {
        for input in ["", "abc", "1.234", "1.2.3", "10 dollars"] {
            assert!(Money::parse(input).is_err(), "{input:?} should not parse");
        }
    }

    #[test]
    fn the_wire_format_keeps_both_decimal_places() {
        // "10.2" would also be valid JSON, but a payment file people read
        // should say what it means.
        assert_eq!(Money::parse("10.20").unwrap().to_api().to_string(), "10.20");
        assert_eq!(Money::parse("0.99").unwrap().to_api().to_string(), "0.99");
        assert_eq!(Money::parse("5k").unwrap().to_api().to_string(), "5000.00");
    }

    #[test]
    fn amounts_survive_the_round_trip_through_json() {
        // 0.1 + 0.2 territory: these are the values a float would mangle.
        for input in ["0.01", "0.99", "10.20", "1234.56", "999999.99"] {
            let original = Money::parse(input).unwrap();
            let returned = Money::from_json(&original.to_api()).unwrap();
            assert_eq!(returned, original, "{input} did not survive");
        }
        assert_eq!(Money::from_json(&json!(0.29)).unwrap().cents(), 29);
        assert_eq!(Money::from_json(&json!(1e3)).unwrap().cents(), 100_000);
    }

    #[test]
    fn display_is_grouped_and_signed() {
        assert_eq!(Money::parse("1234.56").unwrap().display(), "$1,234.56");
        assert_eq!(Money::parse("-42.50").unwrap().display(), "-$42.50");
        assert_eq!(Money::parse("0.99").unwrap().display(), "$0.99");
        assert_eq!(Money::parse("0").unwrap().display(), "$0.00");
        assert_eq!(Money::parse("1234567.89").unwrap().display(), "$1,234,567.89");
    }
}
