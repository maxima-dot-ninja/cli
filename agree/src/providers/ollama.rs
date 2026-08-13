use crate::config::AiConfig;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const API_URL: &str = "http://localhost:11434/api/generate";

#[derive(Serialize)]
struct Request {
    model: String,
    system: String,
    prompt: String,
    stream: bool,
    /// Ollama honours this per-request; low temperature keeps structured
    /// output stable on smaller local models.
    options: Options,
}

#[derive(Serialize)]
struct Options {
    temperature: f32,
}

#[derive(Deserialize)]
struct Response {
    response: String,
}

pub async fn generate(config: &AiConfig, _key: &str, system: &str, prompt: &str) -> Result<String> {
    let request = Request {
        model: config.model.clone(),
        system: system.to_string(),
        prompt: prompt.to_string(),
        stream: false,
        options: Options { temperature: 0.0 },
    };

    let response = reqwest::Client::new()
        .post(API_URL)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .context("Failed to reach Ollama. Is it running? Try `ollama serve`.")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Ollama API error ({status}): {body}");
    }

    let result: Response = response.json().await.context("Failed to parse Ollama response")?;
    Ok(result.response)
}
