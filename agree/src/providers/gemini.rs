use crate::config::AiConfig;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

#[derive(Serialize)]
struct Request {
    #[serde(rename = "systemInstruction")]
    system_instruction: Content,
    contents: Vec<Content>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
}

#[derive(Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Serialize)]
struct Part {
    text: String,
}

#[derive(Serialize)]
struct GenerationConfig {
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,
}

#[derive(Deserialize)]
struct Response {
    candidates: Vec<Candidate>,
}

#[derive(Deserialize)]
struct Candidate {
    content: ResponseContent,
}

#[derive(Deserialize)]
struct ResponseContent {
    parts: Vec<ResponsePart>,
}

#[derive(Deserialize)]
struct ResponsePart {
    text: String,
}

pub async fn generate(config: &AiConfig, key: &str, system: &str, prompt: &str) -> Result<String> {
    let url = format!("{}/{}:generateContent?key={}", API_BASE, config.model, key);

    let request = Request {
        system_instruction: Content { parts: vec![Part { text: system.into() }] },
        contents: vec![Content { parts: vec![Part { text: prompt.into() }] }],
        generation_config: GenerationConfig { max_output_tokens: 1024 },
    };

    let response = reqwest::Client::new()
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .context("Failed to reach Gemini")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Gemini API error ({status}): {body}");
    }

    let result: Response = response.json().await.context("Failed to parse Gemini response")?;
    result
        .candidates
        .first()
        .and_then(|c| c.content.parts.first())
        .map(|p| p.text.clone())
        .context("Empty Gemini response")
}
