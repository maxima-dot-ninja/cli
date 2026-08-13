use crate::config::AiConfig;
use crate::money::Money;
use crate::providers::{anthropic, gemini, ollama, openai};
use anyhow::{bail, Result};
use serde::Deserialize;

/// Fields the form does not read are still parsed, so a model that fills them in
/// does not fail deserialisation.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Intent {
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub amount: Option<String>,
    #[serde(default)]
    pub payee: Option<String>,
    #[serde(default)]
    pub repeat_unit: Option<String>,
    #[serde(default)]
    pub repeat_frequency: Option<i64>,
    #[serde(default)]
    pub ends: Option<String>,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub memo: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub unclear: Option<String>,
}

impl Intent {
    /// Cents, or None when the model gave nothing usable. A malformed amount is
    /// treated as missing rather than an error — the form will ask for it.
    pub fn money(&self, currency: &str) -> Option<Money> {
        let raw = self.amount.as_ref()?;
        Money::parse(raw, currency).ok()
    }

    /// Only "week" and "month" exist in the API; anything else is dropped.
    pub fn unit(&self) -> Option<String> {
        match self.repeat_unit.as_deref() {
            Some("week") | Some("weekly") => Some("week".into()),
            Some("month") | Some("monthly") => Some("month".into()),
            _ => None,
        }
    }

    pub fn frequency(&self) -> i64 {
        self.repeat_frequency.filter(|n| *n > 0).unwrap_or(1)
    }
}

/// Which key a provider needs, and where it comes from if the config is blank.
fn resolve_key(ai: &AiConfig) -> String {
    if !ai.api_key.is_empty() {
        return ai.api_key.clone();
    }
    let var = match ai.provider.as_str() {
        "anthropic" => "ANTHROPIC_API_KEY",
        "openai" => "OPENAI_API_KEY",
        "gemini" => "GOOGLE_API_KEY",
        _ => return String::new(),
    };
    std::env::var(var).unwrap_or_default()
}

pub async fn dispatch(ai: &AiConfig, system: &str, prompt: &str) -> Result<String> {
    let key = resolve_key(ai);
    match ai.provider.as_str() {
        "anthropic" => anthropic::generate(ai, &key, system, prompt).await,
        "openai" => openai::generate(ai, &key, system, prompt).await,
        "gemini" => gemini::generate(ai, &key, system, prompt).await,
        "ollama" => ollama::generate(ai, &key, system, prompt).await,
        "" => bail!("No AI provider configured. Run `agree model` to pick one."),
        other => bail!("Unknown AI provider: {other}. Run `agree model` to pick one."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &str) -> Intent {
        serde_json::from_str(raw).expect("should have parsed")
    }

    #[test]
    fn amounts_become_cents() {
        assert_eq!(parse(r#"{"amount":"5000"}"#).money("USD").unwrap().amount, 500_000);
        assert_eq!(parse(r#"{"amount":"$1.5k"}"#).money("USD").unwrap().amount, 150_000);
    }

    #[test]
    fn a_bad_amount_reads_as_missing_not_wrong() {
        // A weak model writing nonsense must not produce a wrong invoice.
        assert!(parse(r#"{"amount":"about five grand"}"#).money("USD").is_none());
        assert!(parse(r#"{}"#).money("USD").is_none());
    }

    #[test]
    fn normalises_loose_cadence_words() {
        assert_eq!(parse(r#"{"repeat_unit":"weekly"}"#).unit().unwrap(), "week");
        assert_eq!(parse(r#"{"repeat_unit":"monthly"}"#).unit().unwrap(), "month");
        assert!(parse(r#"{"repeat_unit":"fortnight"}"#).unit().is_none());
        assert!(parse(r#"{"repeat_unit":null}"#).unit().is_none());
    }

    #[test]
    fn frequency_defaults_sanely() {
        assert_eq!(parse(r#"{"repeat_frequency":2}"#).frequency(), 2);
        assert_eq!(parse(r#"{"repeat_frequency":0}"#).frequency(), 1);
        assert_eq!(parse(r#"{}"#).frequency(), 1);
    }
}
