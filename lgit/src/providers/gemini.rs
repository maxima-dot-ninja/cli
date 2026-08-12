use crate::config::ProviderConfig;
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

/// Generate a commit message using Google Gemini API
pub async fn generate(config: &ProviderConfig, system: &str, prompt: &str) -> Result<String> {
    let client = reqwest::Client::new();

    let url = format!(
        "{}/{}:generateContent?key={}",
        API_BASE, config.model, config.api_key
    );

    let request = Request {
        system_instruction: Content {
            parts: vec![Part {
                text: system.to_string(),
            }],
        },
        contents: vec![Content {
            parts: vec![Part {
                text: prompt.to_string(),
            }],
        }],
        generation_config: GenerationConfig {
            max_output_tokens: 1024,
        },
    };

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .context("Failed to send request to Gemini")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Gemini API error ({}): {}", status, body);
    }

    let result: Response = response
        .json()
        .await
        .context("Failed to parse Gemini response")?;

    result
        .candidates
        .first()
        .and_then(|c| c.content.parts.first())
        .map(|p| p.text.clone())
        .context("No content in Gemini response")
}
