use crate::config::ProviderConfig;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::io::Write;
use std::process::{Command, Stdio};

/// The `claude -p` reply in `--output-format json`. Only the fields lgit reads.
#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    result: String,
    #[serde(default)]
    is_error: bool,
}

/// Generate a commit message through the Claude Code CLI, billed to the
/// developer's Claude subscription instead of an API key.
///
/// The child is run with `std::process` on a blocking thread rather than
/// `tokio::process` on purpose. tokio's process driver installs a SIGCHLD
/// handler, and that signal interrupts the blocking terminal read behind the
/// interactive menu. The `console` crate treats an interrupted read as Ctrl-C
/// and raises SIGINT, so under `--root` lgit would kill itself the moment any
/// other repo's `claude` finished while the developer was at the prompt.
pub async fn generate(config: &ProviderConfig, system: &str, prompt: &str) -> Result<String> {
    let model = config.model.clone();
    let system = system.to_string();
    let prompt = prompt.to_string();
    tokio::task::spawn_blocking(move || run(&model, &system, &prompt))
        .await
        .context("claude task failed")?
}

/// The flags strip Claude Code down to a one-shot model call: no tools, no
/// settings files, no MCP servers, no CLAUDE.md or memory sections, and no
/// session written to disk. Without them the full agent system prompt is sent
/// along (tens of thousands of tokens and ten seconds per commit message).
/// `--bare` is deliberately absent: it also skips loading the login, so every
/// call would fail with "Not logged in".
fn run(model: &str, system: &str, prompt: &str) -> Result<String> {
    let mut child = Command::new("claude")
        .args([
            "-p",
            "--model",
            model,
            "--tools",
            "",
            "--setting-sources",
            "",
            "--strict-mcp-config",
            "--exclude-dynamic-system-prompt-sections",
            "--no-session-persistence",
            "--system-prompt",
            system,
            "--output-format",
            "json",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to run 'claude'. Is Claude Code installed and on your PATH?")?;

    // The prompt goes over stdin so its size never hits the argv limit.
    let mut stdin = child.stdin.take().context("Could not open stdin for claude")?;
    stdin
        .write_all(prompt.as_bytes())
        .context("Could not send prompt to claude")?;
    drop(stdin);

    let output = child
        .wait_with_output()
        .context("Failed waiting for claude")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("claude exited with {}: {}{}", output.status, stdout.trim(), stderr.trim());
    }

    let result: Response = serde_json::from_str(&stdout)
        .with_context(|| format!("Unexpected claude response shape: {}", stdout.trim()))?;

    if result.is_error {
        anyhow::bail!("claude reported an error: {}", result.result.trim());
    }
    if result.result.trim().is_empty() {
        anyhow::bail!("claude returned no text");
    }

    Ok(result.result)
}
