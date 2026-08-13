use crate::config::{self, Config};
use anyhow::{Context, Result};
use console::style;
use dialoguer::{Input, Password, Select};
use std::process::Command;

struct Provider {
    name: &'static str,
    display: &'static str,
    models: &'static [(&'static str, &'static str)],
    needs_key: bool,
    env_var: &'static str,
}

const PROVIDERS: &[Provider] = &[
    Provider {
        name: "anthropic",
        display: "Anthropic",
        models: &[
            ("claude-sonnet-5", "Claude Sonnet 5 (recommended)"),
            ("claude-opus-5", "Claude Opus 5"),
            ("claude-haiku-4-5-20251001", "Claude Haiku 4.5 (fastest)"),
        ],
        needs_key: true,
        env_var: "ANTHROPIC_API_KEY",
    },
    Provider {
        name: "openai",
        display: "OpenAI",
        models: &[
            ("gpt-5.2", "GPT-5.2 (recommended)"),
            ("gpt-5-mini", "GPT-5 Mini"),
            ("gpt-5.2-pro", "GPT-5.2 Pro (smartest)"),
        ],
        needs_key: true,
        env_var: "OPENAI_API_KEY",
    },
    Provider {
        name: "gemini",
        display: "Google Gemini",
        models: &[
            ("gemini-3.1-pro", "Gemini 3.1 Pro (recommended)"),
            ("gemini-3-flash", "Gemini 3 Flash"),
        ],
        needs_key: true,
        env_var: "GOOGLE_API_KEY",
    },
    Provider {
        name: "ollama",
        display: "Ollama (local, no key)",
        models: &[],
        needs_key: false,
        env_var: "",
    },
];

/// Whatever `ollama list` reports, so a beefier machine offers its own models.
fn installed_ollama_models() -> Vec<String> {
    let Ok(output) = Command::new("ollama").arg("list").output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .skip(1)
        .filter_map(|line| line.split_whitespace().next())
        .map(String::from)
        .collect()
}

/// Pick the AI provider and model. Model choice is always the user's — a small
/// local model is a supported answer, not a blocked one.
pub fn choose_model() -> Result<()> {
    let mut config = config::load()?;

    println!();
    let provider_idx = Select::new()
        .with_prompt("AI provider")
        .items(&PROVIDERS.iter().map(|p| p.display).collect::<Vec<_>>())
        .default(0)
        .interact()
        .context("No provider chosen")?;
    let provider = &PROVIDERS[provider_idx];

    let model = match provider.name {
        "ollama" => choose_ollama_model()?,
        _ => {
            let labels: Vec<&str> = provider.models.iter().map(|(_, label)| *label).collect();
            let idx = Select::new()
                .with_prompt("Model")
                .items(&labels)
                .default(0)
                .interact()
                .context("No model chosen")?;
            provider.models[idx].0.to_string()
        }
    };

    config.ai.provider = provider.name.to_string();
    config.ai.model = model.clone();

    if provider.needs_key {
        config.ai.api_key = ask_for_key(provider, &config)?;
    }

    config::save(&config)?;

    println!();
    println!("{} {} · {}", style("✓").green().bold(), provider.display, style(&model).cyan());
    println!("  saved to {}", config::config_path()?.display());
    warn_if_small(&config);
    println!();
    Ok(())
}

fn choose_ollama_model() -> Result<String> {
    let installed = installed_ollama_models();

    if installed.is_empty() {
        println!(
            "  {}",
            style("No local models found. Is ollama running? Enter a name manually.").dim()
        );
        let name: String = Input::new()
            .with_prompt("Model name")
            .interact_text()
            .context("No model given")?;
        return Ok(name.trim().to_string());
    }

    let idx = Select::new()
        .with_prompt("Local model")
        .items(&installed)
        .default(0)
        .interact()
        .context("No model chosen")?;
    Ok(installed[idx].clone())
}

/// An existing environment variable is the better home for a key, so offer to
/// keep using it rather than writing a second copy into the config file.
fn ask_for_key(provider: &Provider, config: &Config) -> Result<String> {
    let from_env = std::env::var(provider.env_var).unwrap_or_default();

    if !from_env.is_empty() {
        println!(
            "  {} found in {} — leaving the config file empty so it stays the source of truth.",
            style("key").green(),
            style(provider.env_var).cyan()
        );
        return Ok(String::new());
    }

    if !config.ai.api_key.is_empty() {
        println!("  {}", style("keeping the existing key in your config").dim());
        return Ok(config.ai.api_key.clone());
    }

    println!(
        "  {}",
        style(format!(
            "Tip: export {} in ~/.config/secrets.env instead, and leave this blank.",
            provider.env_var
        ))
        .dim()
    );
    let key: String = Password::new()
        .with_prompt(format!("{} API key (enter to skip)", provider.display))
        .allow_empty_password(true)
        .interact()
        .context("No key entered")?;
    Ok(key)
}

/// Small models handle loose phrasing less reliably. Say so once, then get out of
/// the way — the form catches whatever the model misses either way.
pub fn warn_if_small(config: &Config) {
    if config.ai.provider != "ollama" {
        return;
    }
    println!(
        "  {}",
        style("Local model: expect it to miss details on loose phrasing. You'll be asked to fill in anything it can't work out.")
            .dim()
    );
}

pub fn show_ai(config: &Config) {
    let provider = match config.ai.provider.is_empty() {
        true => "not set — run `agree model`".to_string(),
        false => config.ai.provider.clone(),
    };
    println!("  AI provider : {provider}");
    if !config.ai.model.is_empty() {
        println!("  AI model    : {}", config.ai.model);
    }
}
