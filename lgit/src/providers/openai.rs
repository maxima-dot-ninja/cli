use crate::config::ProviderConfig;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const API_URL: &str = "https://api.openai.com/v1/chat/completions";

#[derive(Serialize)]
struct Request {
    model: String,
    messages: Vec<Message>,
    max_completion_tokens: u32,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct Response {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

/// Generate a commit message using OpenAI's API
pub async fn generate(config: &ProviderConfig, system: &str, prompt: &str) -> Result<String> {
    let client = reqwest::Client::new();

    let request = Request {
        model: config.model.clone(),
        messages: vec![
            Message {
                role: "system".to_string(),
                content: system.to_string(),
            },
            Message {
                role: "user".to_string(),
                content: prompt.to_string(),
            },
        ],
        max_completion_tokens: 1024,
    };

    let response = client
        .post(API_URL)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .context("Failed to send request to OpenAI")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("OpenAI API error ({}): {}", status, body);
    }

    let result: Response = response
        .json()
        .await
        .context("Failed to parse OpenAI response")?;

    result
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .context("No content in OpenAI response")
}
