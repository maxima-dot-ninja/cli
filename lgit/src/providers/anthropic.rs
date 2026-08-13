use crate::config::ProviderConfig;
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
/// carries no `text` field. Requiring `text` fails the whole parse; taking the
/// first block returns the reasoning instead of the reply.
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

/// Generate a commit message using Anthropic's API
pub async fn generate(config: &ProviderConfig, system: &str, prompt: &str) -> Result<String> {
    let client = reqwest::Client::new();

    let request = Request {
        model: config.model.clone(),
        max_tokens: 1024,
        system: system.to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: prompt.to_string(),
        }],
    };

    let response = client
        .post(API_URL)
        .header("x-api-key", &config.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await
        .context("Failed to send request to Anthropic")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Anthropic API error ({}): {}", status, body);
    }

    // Read as text first so a parse failure can show what actually came back.
    let body = response
        .text()
        .await
        .context("Could not read Anthropic response")?;
    let result: Response = serde_json::from_str(&body)
        .with_context(|| format!("Unexpected Anthropic response shape: {body}"))?;

    result.answer().context("Anthropic returned no text block")
}
