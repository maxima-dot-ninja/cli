use crate::config::AiConfig;
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

pub async fn generate(config: &AiConfig, key: &str, system: &str, prompt: &str) -> Result<String> {
    let request = Request {
        model: config.model.clone(),
        messages: vec![
            Message { role: "system".into(), content: system.into() },
            Message { role: "user".into(), content: prompt.into() },
        ],
        max_completion_tokens: 4096,
    };

    let response = reqwest::Client::new()
        .post(API_URL)
        .header("Authorization", format!("Bearer {key}"))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .context("Failed to reach OpenAI")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("OpenAI API error ({status}): {body}");
    }

    let result: Response = response.json().await.context("Failed to parse OpenAI response")?;
    result.choices.first().map(|c| c.message.content.clone()).context("Empty OpenAI response")
}
