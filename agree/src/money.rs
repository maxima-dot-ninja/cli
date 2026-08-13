use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// An amount in the smallest currency unit, exactly as the API wants it.
///
/// The API takes integer cents and says so loudly, because sending 50 for "$50"
/// bills a customer 50 cents. Dollars never exist as a number in this program —
/// they are parsed into cents at the edge and formatted back for display, so
/// there is no float in the middle to round wrong.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    /// Integer amount in the smallest currency unit. $150.00 => 15000
    pub amount: i64,
    /// ISO 4217 code
    pub currency: String,
}

impl Money {
    pub fn from_cents(cents: i64, currency: &str) -> Self {
        Self { amount: cents, currency: currency.to_uppercase() }
    }

    /// Parse human input: "5000", "$5,000", "5000.50", "$1.5k", "USD 20".
    ///
    /// Rejects more than two decimal places rather than rounding, so a typo
    /// becomes an error instead of a silently wrong invoice.
    pub fn parse(input: &str, currency: &str) -> Result<Self> {
        let cleaned: String = input
            .trim()
            .trim_start_matches("USD")
            .trim_start_matches("usd")
            .replace(['$', ',', '_', ' '], "");

        if cleaned.is_empty() {
            bail!("no amount given");
        }

        let (digits, multiplier) = match cleaned.chars().last() {
            Some('k') | Some('K') => (&cleaned[..cleaned.len() - 1], 1_000),
            Some('m') | Some('M') => (&cleaned[..cleaned.len() - 1], 1_000_000),
            _ => (cleaned.as_str(), 1),
        };

        let (whole, frac) = match digits.split_once('.') {
            Some((w, f)) => (w, f),
            None => (digits, ""),
        };

        if frac.len() > 2 {
            bail!("'{}' has more than 2 decimal places", input.trim());
        }
        if !whole.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) {
            bail!("'{}' is not a valid amount", input.trim());
        }

        let whole: i64 = if whole.is_empty() { 0 } else { whole.parse()? };
        let cents_frac: i64 = match frac.len() {
            0 => 0,
            1 => frac.parse::<i64>()? * 10,
            _ => frac.parse::<i64>()?,
        };

        // Multiplier applies to the whole value, so "1.5k" is 150000 cents.
        let total = (whole * 100 + cents_frac) * multiplier;
        Ok(Self::from_cents(total, currency))
    }

    /// "$5,000.00" for display only — never sent to the API
    pub fn display(&self) -> String {
        let negative = self.amount < 0;
        let abs = self.amount.abs();
        let whole = abs / 100;
        let cents = abs % 100;

        let mut grouped = String::new();
        let digits = whole.to_string();
        for (i, c) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i).is_multiple_of(3) {
                grouped.push(',');
            }
            grouped.push(c);
        }

        let symbol = if self.currency == "USD" { "$" } else { "" };
        let suffix = if symbol.is_empty() { format!(" {}", self.currency) } else { String::new() };
        let sign = if negative { "-" } else { "" };
        format!("{sign}{symbol}{grouped}.{cents:02}{suffix}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dollars_become_cents() {
        assert_eq!(Money::parse("5000", "USD").unwrap().amount, 500_000);
        assert_eq!(Money::parse("$5,000", "USD").unwrap().amount, 500_000);
        assert_eq!(Money::parse("50", "USD").unwrap().amount, 5_000);
        assert_eq!(Money::parse("1.00", "USD").unwrap().amount, 100);
    }

    #[test]
    fn handles_partial_cents() {
        assert_eq!(Money::parse("15.5", "USD").unwrap().amount, 1_550);
        assert_eq!(Money::parse("15.05", "USD").unwrap().amount, 1_505);
        assert_eq!(Money::parse(".99", "USD").unwrap().amount, 99);
    }

    #[test]
    fn handles_shorthand() {
        assert_eq!(Money::parse("5k", "USD").unwrap().amount, 500_000);
        assert_eq!(Money::parse("1.5k", "USD").unwrap().amount, 150_000);
        assert_eq!(Money::parse("2M", "USD").unwrap().amount, 200_000_000);
    }

    #[test]
    fn rejects_junk_rather_than_guessing() {
        assert!(Money::parse("", "USD").is_err());
        assert!(Money::parse("abc", "USD").is_err());
        assert!(Money::parse("1.234", "USD").is_err(), "3 decimals must error, not round");
        assert!(Money::parse("1.2.3", "USD").is_err());
    }

    #[test]
    fn formats_for_humans() {
        assert_eq!(Money::from_cents(500_000, "USD").display(), "$5,000.00");
        assert_eq!(Money::from_cents(99, "USD").display(), "$0.99");
        assert_eq!(Money::from_cents(1_234_567, "USD").display(), "$12,345.67");
        assert_eq!(Money::from_cents(100, "EUR").display(), "1.00 EUR");
    }

    #[test]
    fn round_trips() {
        for input in ["5000", "0.01", "1234.56", "1,000,000"] {
            let m = Money::parse(input, "USD").unwrap();
            let again = Money::parse(m.display().trim_start_matches('$'), "USD").unwrap();
            assert_eq!(m.amount, again.amount, "round trip failed for {input}");
        }
    }
}
