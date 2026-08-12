use crate::config::{Config, GitConfig, ProviderConfig, UiConfig};
use anyhow::{Context, Result};
use console::style;
use dialoguer::{Password, Select};
use std::process::Command;

/// Provider definitions with their available models
struct Provider {
    name: &'static str,
    display: &'static str,
    models: &'static [(&'static str, &'static str)], // (model_id, display_name)
    needs_key: bool,
    env_var: &'static str,
}

const PROVIDERS: &[Provider] = &[
    Provider {
        name: "anthropic",
        display: "Anthropic",
        models: &[
            ("claude-sonnet-4-20250514", "Claude Sonnet 4 (recommended)"),
            ("claude-opus-4-20250514", "Claude Opus 4"),
            ("claude-haiku-4-5-20251001", "Claude Haiku 4.5"),
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
            ("gpt-5-nano", "GPT-5 Nano (fastest)"),
            ("gpt-5.2-pro", "GPT-5.2 Pro (smartest)"),
            ("gpt-5", "GPT-5"),
            ("gpt-4.1", "GPT-4.1 (non-reasoning)"),
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
            ("gemini-3-pro", "Gemini 3 Pro"),
            ("gemini-2.5-flash", "Gemini 2.5 Flash"),
            ("gemini-2.5-flash-lite", "Gemini 2.5 Flash-Lite (fastest)"),
        ],
        needs_key: true,
        env_var: "GOOGLE_API_KEY",
    },
    Provider {
        name: "ollama",
        display: "Ollama (local)",
        models: &[], // Detected dynamically
        needs_key: false,
        env_var: "",
    },
];

/// Run the interactive setup wizard
pub fn run_setup() -> Result<()> {
    println!();
    println!("{}", style("┌─────────────────────────────────────────────┐").cyan());
    println!("{}", style("│  lgit — Setup Wizard                        │").cyan());
    println!("{}", style("└─────────────────────────────────────────────┘").cyan());
    println!();

    // Step 1: Select provider
    let provider_names: Vec<&str> = PROVIDERS.iter().map(|p| p.display).collect();
    let provider_idx = Select::new()
        .with_prompt("Select your AI provider")
        .items(&provider_names)
        .default(0)
        .interact()
        .context("Failed to get provider selection")?;

    let provider = &PROVIDERS[provider_idx];
    println!("  {} Selected: {}", style("✓").green(), provider.display);
    println!();

    // Step 2: Select model
    let model = if provider.name == "ollama" {
        select_ollama_model()?
    } else {
        let model_names: Vec<&str> = provider.models.iter().map(|(_, name)| *name).collect();
        let model_idx = Select::new()
            .with_prompt("Select a model")
            .items(&model_names)
            .default(0)
            .interact()
            .context("Failed to get model selection")?;

        let (model_id, _) = provider.models[model_idx];
        println!("  {} Selected: {}", style("✓").green(), model_id);
        model_id.to_string()
    };
    println!();

    // Step 3: Get API key if needed
    let api_key = if provider.needs_key {
        // Check environment variable first
        if let Ok(key) = std::env::var(provider.env_var) {
            println!(
                "  {} Found {} in environment",
                style("✓").green(),
                provider.env_var
            );
            key
        } else {
            let key: String = Password::new()
                .with_prompt(format!("Enter your {} (input hidden)", provider.env_var))
                .interact()
                .context("Failed to get API key")?;
            println!("  {} API key saved", style("✓").green());
            key
        }
    } else {
        String::new()
    };
    println!();

    // Build and save config
    let config = Config {
        provider: ProviderConfig {
            name: provider.name.to_string(),
            model,
            api_key,
        },
        git: GitConfig::default(),
        ui: UiConfig::default(),
    };

    crate::config::save_config(&config)?;

    let config_path = crate::config::config_path()?;
    println!(
        "{} Configuration saved to {}",
        style("✓").green().bold(),
        style(config_path.display()).dim()
    );
    println!();
    println!(
        "  Run {} to commit with AI, or {} to reconfigure.",
        style("lgit").cyan(),
        style("lgit --setup").cyan()
    );
    println!();

    Ok(())
}

/// Change the model (can also switch providers)
pub fn change_model() -> Result<()> {
    let mut config = crate::config::load_config()?;

    println!();
    println!(
        "{} Current: {} / {}",
        style("ℹ").cyan(),
        style(&config.provider.name).green(),
        style(&config.provider.model).green()
    );
    println!();

    // Step 1: Select provider
    let provider_names: Vec<&str> = PROVIDERS.iter().map(|p| p.display).collect();
    let current_provider_idx = PROVIDERS
        .iter()
        .position(|p| p.name == config.provider.name)
        .unwrap_or(0);

    let provider_idx = Select::new()
        .with_prompt("Select provider")
        .items(&provider_names)
        .default(current_provider_idx)
        .interact()
        .context("Failed to get provider selection")?;

    let provider = &PROVIDERS[provider_idx];
    let switching_provider = provider.name != config.provider.name;

    // Step 2: Select model
    let new_model = if provider.name == "ollama" {
        select_ollama_model()?
    } else {
        let model_names: Vec<&str> = provider.models.iter().map(|(_, name)| *name).collect();
        let current_idx = if switching_provider {
            0
        } else {
            provider
                .models
                .iter()
                .position(|(id, _)| *id == config.provider.model)
                .unwrap_or(0)
        };

        let model_idx = Select::new()
            .with_prompt("Select a model")
            .items(&model_names)
            .default(current_idx)
            .interact()
            .context("Failed to get model selection")?;

        let (model_id, _) = provider.models[model_idx];
        model_id.to_string()
    };

    // Step 3: Handle API key if switching to a new provider that needs one
    if switching_provider && provider.needs_key {
        // Check environment variable first
        let has_env_key = std::env::var(provider.env_var).is_ok();

        if has_env_key {
            println!(
                "  {} Found {} in environment",
                style("✓").green(),
                provider.env_var
            );
            config.provider.api_key = String::new(); // Use env var
        } else {
            println!();
            println!(
                "{} {} requires an API key",
                style("!").yellow(),
                provider.display
            );
            let key: String = Password::new()
                .with_prompt(format!("Enter your {} (input hidden)", provider.env_var))
                .interact()
                .context("Failed to get API key")?;
            config.provider.api_key = key;
        }
    }

    // Update config
    config.provider.name = provider.name.to_string();
    config.provider.model = new_model.clone();
    crate::config::save_config(&config)?;

    println!();
    println!(
        "{} Switched to {} / {}",
        style("✓").green().bold(),
        style(provider.display).cyan(),
        style(&new_model).cyan()
    );
    println!();

    Ok(())
}

/// Manage API keys for providers
pub fn manage_api_key() -> Result<()> {
    let mut config = crate::config::load_config()?;

    println!();
    println!("{}", style("API Key Management").bold().cyan());
    println!();

    // Show current status for each provider
    for provider in PROVIDERS.iter().filter(|p| p.needs_key) {
        let env_status = std::env::var(provider.env_var).is_ok();
        let config_status = provider.name == config.provider.name && !config.provider.api_key.is_empty();

        let status = if env_status {
            style("✓ set via environment").green()
        } else if config_status {
            style("✓ set in config").green()
        } else {
            style("✗ not set").dim()
        };

        println!("  {} {}: {}", provider.display, style(provider.env_var).dim(), status);
    }
    println!();

    // Select which provider's key to manage
    let key_providers: Vec<&Provider> = PROVIDERS.iter().filter(|p| p.needs_key).collect();
    let provider_names: Vec<String> = key_providers
        .iter()
        .map(|p| format!("{} ({})", p.display, p.env_var))
        .collect();
    let provider_refs: Vec<&str> = provider_names.iter().map(|s| s.as_str()).collect();

    let provider_idx = Select::new()
        .with_prompt("Select provider to update")
        .items(&provider_refs)
        .default(0)
        .interact()
        .context("Failed to get provider selection")?;

    let provider = key_providers[provider_idx];

    // Check if env var is set
    if std::env::var(provider.env_var).is_ok() {
        println!();
        println!(
            "{} {} is set via environment variable.",
            style("ℹ").cyan(),
            provider.env_var
        );
        println!(
            "  To change it, update your shell profile or unset it to use config instead."
        );
        println!();
        return Ok(());
    }

    // Prompt for new key
    println!();
    let key: String = Password::new()
        .with_prompt(format!("Enter {} (input hidden)", provider.env_var))
        .interact()
        .context("Failed to get API key")?;

    // If this is for the current provider, save to config
    if provider.name == config.provider.name {
        config.provider.api_key = key;
        crate::config::save_config(&config)?;
        println!();
        println!(
            "{} API key saved to config",
            style("✓").green().bold()
        );
    } else {
        // For non-current providers, suggest environment variable
        println!();
        println!(
            "{} To use this key, add to your shell profile:",
            style("ℹ").cyan()
        );
        println!();
        println!("  export {}=\"{}...\"", provider.env_var, &key[..key.len().min(8)]);
        println!();
        println!(
            "  Or switch to {} with {} and re-run this command.",
            provider.display,
            style("lgit --model").cyan()
        );
    }
    println!();

    Ok(())
}

/// Detect and select from installed Ollama models
fn select_ollama_model() -> Result<String> {
    // Try to get list of installed models
    let output = Command::new("ollama")
        .arg("list")
        .output()
        .context("Failed to run 'ollama list'. Is Ollama installed?")?;

    if !output.status.success() {
        anyhow::bail!("Failed to list Ollama models. Make sure Ollama is running.");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let models: Vec<String> = stdout
        .lines()
        .skip(1) // Skip header line
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            parts.first().map(|s| s.to_string())
        })
        .collect();

    if models.is_empty() {
        anyhow::bail!("No Ollama models found. Install one with 'ollama pull <model>'");
    }

    let model_refs: Vec<&str> = models.iter().map(|s| s.as_str()).collect();
    let idx = Select::new()
        .with_prompt("Select an Ollama model")
        .items(&model_refs)
        .default(0)
        .interact()
        .context("Failed to get model selection")?;

    println!("  {} Selected: {}", style("✓").green(), models[idx]);
    Ok(models[idx].clone())
}
