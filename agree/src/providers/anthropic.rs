use crate::config::AiConfig;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const API_URL: &str = "https://api.anthropic.com/v1/messages";

#[derive(Serialize)]
struct Request {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<Message>,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct Response {
    content: Vec<ContentBlock>,
}

/// Reasoning models put a `thinking` block ahead of the answer, and that block
/// carries no `text` field at all. Requiring `text` here fails the whole parse;
/// taking the first block returns the reasoning instead of the reply.
#[derive(Deserialize)]
struct ContentBlock {
    #[serde(default)]
    text: Option<String>,
}

impl Response {
    fn answer(&self) -> Option<String> {
        self.content.iter().find_map(|block| block.text.clone())
    }
}

pub async fn generate(config: &AiConfig, key: &str, system: &str, prompt: &str) -> Result<String> {
    let request = Request {
        model: config.model.clone(),
        max_tokens: 1024,
        system: system.to_string(),
        messages: vec![Message { role: "user".into(), content: prompt.into() }],
    };

    let response = reqwest::Client::new()
        .post(API_URL)
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await
        .context("Failed to reach Anthropic")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Anthropic API error ({status}): {body}");
    }

    // Read as text first so a parse failure can show what actually came back.
    let body = response.text().await.context("Could not read Anthropic response")?;
    let result: Response = serde_json::from_str(&body)
        .with_context(|| format!("Unexpected Anthropic response shape: {body}"))?;

    result.answer().context("Anthropic returned no text block")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_the_thinking_block() {
        // Exactly what claude-sonnet-5 returns: reasoning first, answer second.
        let body = r#"{"content":[
            {"type":"thinking","thinking":"weighing it up","signature":"abc"},
            {"type":"text","text":"{\"action\":\"create_invoice\"}"}
        ]}"#;
        let parsed: Response = serde_json::from_str(body).expect("must parse despite thinking block");
        assert_eq!(parsed.answer().unwrap(), "{\"action\":\"create_invoice\"}");
    }

    #[test]
    fn still_reads_a_plain_reply() {
        let body = r#"{"content":[{"type":"text","text":"hello"}]}"#;
        let parsed: Response = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.answer().unwrap(), "hello");
    }

    #[test]
    fn reasoning_only_reply_is_not_an_answer() {
        let body = r#"{"content":[{"type":"thinking","thinking":"...","signature":"x"}]}"#;
        let parsed: Response = serde_json::from_str(body).unwrap();
        assert!(parsed.answer().is_none());
    }
}
