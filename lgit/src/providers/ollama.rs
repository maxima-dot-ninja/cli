use crate::config::ProviderConfig;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const API_URL: &str = "http://localhost:11434/api/generate";

#[derive(Serialize)]
struct Request {
    model: String,
    system: String,
    prompt: String,
    stream: bool,
}

#[derive(Deserialize)]
struct Response {
    response: String,
}

/// Generate a commit message using local Ollama
pub async fn generate(config: &ProviderConfig, system: &str, prompt: &str) -> Result<String> {
    let client = reqwest::Client::new();

    let request = Request {
        model: config.model.clone(),
        system: system.to_string(),
        prompt: prompt.to_string(),
        stream: false,
    };

    let response = client
        .post(API_URL)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .context("Failed to send request to Ollama. Is Ollama running?")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Ollama API error ({}): {}", status, body);
    }

    let result: Response = response
        .json()
        .await
        .context("Failed to parse Ollama response")?;

    Ok(result.response)
}
