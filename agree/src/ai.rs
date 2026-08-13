use crate::config::{AiConfig, Config};
use crate::money::Money;
use crate::providers::{anthropic, gemini, ollama, openai};
use anyhow::{bail, Result};
use serde::Deserialize;

/// The model's job is to fill in this shape and nothing else. It never calls the
/// API, never sees a key, and never picks an email address — every value it
/// produces is re-validated in Rust and anything missing becomes a form field.
const SYSTEM: &str = r#"You turn a person's request about invoicing into ONE JSON object. You reply with raw JSON only — no prose, no markdown, no code fences.

Shape:

{
  "action": "create_invoice" | "list_invoices" | "find_contact" | "unknown",
  "amount": "the amount exactly as the person wrote it, e.g. \"5000\" or \"$1.5k\", or null",
  "payee": "who is being billed, as they said it, e.g. \"Samir\", or null",
  "repeat_unit": "week" | "month" | null,
  "repeat_frequency": 1,
  "ends": "never" | "date" | "count",
  "due_date": "YYYY-MM-DD or null",
  "memo": "short description or null",
  "status": "for list_invoices only: paid|due|sent|failed|draft, or null",
  "unclear": "anything you could not work out, in a few words, or null"
}

Rules:
- Copy the amount as written. Do NOT convert to cents, do NOT strip the currency symbol. The program handles that.
- Never invent an email address, a contact, or an ID. Put the name in "payee" exactly as spoken.
- "every week" means repeat_unit "week" and repeat_frequency 1. "every other week" is repeat_unit "week", repeat_frequency 2.
- If the request only asks to see or find something, use list_invoices or find_contact.
- If a value was not stated, use null. Guessing is worse than null — a null becomes a question for the user, a wrong guess becomes a wrong invoice.
- Reply with the JSON object alone."#;

const RETRY: &str = "\n\nYour previous reply was not valid JSON. Reply with ONLY the JSON object this time, starting with { and ending with }.";

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

async fn dispatch(ai: &AiConfig, system: &str, prompt: &str) -> Result<String> {
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

/// Read a request into an Intent. Two attempts, then give up — the caller falls
/// back to asking for everything by hand, which still gets the job done.
pub async fn read_intent(config: &Config, request: &str) -> Result<Intent> {
    let prompt = format!(
        "Today is {}. Turn this request into the JSON object:\n\n{}",
        chrono::Local::now().format("%Y-%m-%d"),
        request
    );

    let mut last = String::new();
    for attempt in 0..2 {
        let full = match attempt {
            0 => prompt.clone(),
            _ => format!("{prompt}{RETRY}"),
        };
        let raw = dispatch(&config.ai, SYSTEM, &full).await?;
        if let Some(intent) = extract(&raw) {
            return Ok(intent);
        }
        last = raw;
    }

    bail!(
        "The model did not return usable JSON after 2 tries.\nLast reply was:\n{}",
        last.trim()
    )
}

/// Pull the JSON object out of a reply, tolerating fences and stray prose that
/// smaller local models tend to add despite instructions.
fn extract(raw: &str) -> Option<Intent> {
    let text = raw.trim();
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str::<Intent>(&text[start..=end]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &str) -> Intent {
        extract(raw).expect("should have parsed")
    }

    #[test]
    fn reads_the_worked_example() {
        let raw = r#"{"action":"create_invoice","amount":"5000","payee":"Samir",
                     "repeat_unit":"week","repeat_frequency":1,"ends":"never",
                     "due_date":null,"memo":null,"status":null,"unclear":null}"#;
        let intent = parse(raw);
        assert_eq!(intent.action, "create_invoice");
        assert_eq!(intent.money("USD").unwrap().amount, 500_000, "$5000 must become cents");
        assert_eq!(intent.unit().unwrap(), "week");
        assert_eq!(intent.frequency(), 1);
        assert_eq!(intent.payee.unwrap(), "Samir");
    }

    #[test]
    fn survives_fences_and_chatter() {
        let raw = "Sure! Here is the JSON:\n```json\n{\"action\":\"list_invoices\",\"status\":\"paid\"}\n```\nHope that helps!";
        let intent = parse(raw);
        assert_eq!(intent.action, "list_invoices");
        assert_eq!(intent.status.unwrap(), "paid");
    }

    #[test]
    fn a_bad_amount_reads_as_missing_not_wrong() {
        // A weak model writing nonsense must not produce a wrong invoice.
        let intent = parse(r#"{"action":"create_invoice","amount":"about five grand"}"#);
        assert!(intent.money("USD").is_none(), "unparseable amount must be None, not a guess");
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

    #[test]
    fn missing_fields_are_never_fatal() {
        // Any subset must parse; the form fills the rest.
        let intent = parse(r#"{"action":"create_invoice"}"#);
        assert!(intent.amount.is_none() && intent.payee.is_none());
    }

    #[test]
    fn rejects_replies_with_no_object() {
        assert!(extract("I'm not sure what you mean.").is_none());
        assert!(extract("").is_none());
    }
}
