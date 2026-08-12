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

#[derive(Deserialize)]
struct ContentBlock {
    text: String,
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

    let result: Response = response
        .json()
        .await
        .context("Failed to parse Anthropic response")?;

    result
        .content
        .first()
        .map(|c| c.text.clone())
        .context("No content in Anthropic response")
}
