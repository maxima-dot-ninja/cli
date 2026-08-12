use crate::config::Config;
use crate::providers::{anthropic, gemini, ollama, openai};
use anyhow::Result;

/// System prompt: role, format, and examples. Sent as a system message so it
/// outweighs the model's default assistant persona.
const SYSTEM_PROMPT: &str = r#"You are a git commit message generator. You reply with ONLY a raw Conventional Commits message — never commentary, analysis, greetings, or markdown.

Format:

<type>(<scope>): <short summary>

<optional body>

Rules:
- Allowed types: feat, fix, docs, style, refactor, test, chore, perf, ci, build, revert
- Include a (scope) when the change centers on one area — a module, file, or feature. Omit it only when the change spans many unrelated areas.
- Subject line under 72 chars, imperative mood, no trailing period
- Body: for a change with multiple distinct parts, add a blank line after the subject and list each part as a short bullet point starting with "- ". Only single-purpose changes should be subject-only.
- Plain text only: no markdown, no backticks, no ** bold, no code fences
- The FIRST character of your reply must be the first letter of the type. Never begin with phrases like "It looks like", "This commit", "Here is", or any explanation.
- Do not add anything after the body — no sign-off, no "let me know"

Example of a good reply:

feat(map-stage): add tile caching for zoom levels

- cache rendered tiles keyed by zoom and viewport
- evict least-recently-used tiles above the memory limit

Example of a BAD reply, never do this:

It looks like the provided code diff is a significant change that..."#;

/// Reminder appended on retry when the first reply failed validation
const RETRY_NOTE: &str = "\n\nIMPORTANT: your previous reply was rejected because it did not start with a Conventional Commits header like \"feat(scope): summary\". Reply with ONLY the commit message this time.";

/// Generate a commit message using the configured AI provider
pub async fn generate_commit(config: &Config, diff: &str) -> Result<String> {
    // Instructions closest to the model's reply carry the most weight, so the
    // diff goes first and the ask comes after it.
    let body_ask = if is_large_diff(diff) {
        "This is a large change, so after the subject line add a blank line and a body: one \"- \" bullet point per distinct change."
    } else {
        "Add a bullet-point body after the subject only if the change has multiple distinct parts."
    };
    let prompt = format!(
        "Read this git diff. Lines starting with \"+\" are code you added, lines starting with \"-\" are code you removed, and unprefixed lines are unchanged context — do not describe those as changes.\n\nGit diff:\n{}\n\nNow reply with ONLY the commit message for the changes above (no other text), starting directly with the type, e.g. \"feat(scope): ...\". {}",
        diff, body_ask
    );

    let mut last_output = String::new();

    for attempt in 0..2 {
        let prompt = if attempt == 0 {
            prompt.clone()
        } else {
            format!("{}{}", prompt, RETRY_NOTE)
        };

        let raw = dispatch(config, SYSTEM_PROMPT, &prompt).await?;

        if let Some(message) = extract_commit(&raw) {
            return Ok(message);
        }
        last_output = raw;
    }

    anyhow::bail!(
        "Model did not produce a valid Conventional Commits message after 2 attempts.\nLast output was:\n{}",
        last_output.trim()
    )
}

/// A diff is "large" when it touches several files or many lines — enough that
/// a subject line alone can't cover it.
fn is_large_diff(diff: &str) -> bool {
    let mut files = 0;
    let mut changed_lines = 0;
    for line in diff.lines() {
        if line.starts_with("diff --git") {
            files += 1;
        } else if (line.starts_with('+') && !line.starts_with("+++"))
            || (line.starts_with('-') && !line.starts_with("---"))
        {
            changed_lines += 1;
        }
    }
    files >= 3 || changed_lines >= 50
}

async fn dispatch(config: &Config, system: &str, prompt: &str) -> Result<String> {
    match config.provider.name.as_str() {
        "anthropic" => anthropic::generate(&config.provider, system, prompt).await,
        "openai" => openai::generate(&config.provider, system, prompt).await,
        "gemini" => gemini::generate(&config.provider, system, prompt).await,
        "ollama" => ollama::generate(&config.provider, system, prompt).await,
        other => anyhow::bail!("Unknown provider: {}", other),
    }
}

/// Pull a valid commit message out of the model's reply, or None if there isn't one.
/// Drops any preamble before the Conventional Commits header and strips markdown
/// the model added despite instructions.
fn extract_commit(raw: &str) -> Option<String> {
    let cleaned = strip_code_fences(raw.trim());
    let lines: Vec<&str> = cleaned.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let header = clean_header(line);
        if is_valid_header(&header) {
            let mut out = vec![header];
            // Backticks in the body are always stray markdown; leave "**" alone
            // there since it can be legitimate code (e.g. char **argv)
            out.extend(lines[i + 1..].iter().map(|l| l.replace('`', "")));
            return Some(out.join("\n").trim().to_string());
        }
    }
    None
}

fn strip_code_fences(message: &str) -> &str {
    let message = message
        .strip_prefix("```plaintext")
        .or_else(|| message.strip_prefix("```text"))
        .or_else(|| message.strip_prefix("```"))
        .unwrap_or(message);
    message.strip_suffix("```").unwrap_or(message).trim()
}

/// Undo markdown decoration on a header candidate: `**feat:**`, `# feat: ...`, backticks
fn clean_header(line: &str) -> String {
    line.trim()
        .trim_start_matches('#')
        .trim()
        .replace("**", "")
        .replace('`', "")
}

/// Check a line against `<type>(<scope>)?!?: <summary>`
fn is_valid_header(line: &str) -> bool {
    const TYPES: &[&str] = &[
        "feat", "fix", "docs", "style", "refactor", "test", "chore", "perf", "ci", "build",
        "revert",
    ];

    let Some((head, summary)) = line.split_once(':') else {
        return false;
    };
    let head = head.strip_suffix('!').unwrap_or(head);

    let (ty, scope_ok) = match head.split_once('(') {
        Some((ty, rest)) => {
            let ok = rest.len() > 1
                && rest.ends_with(')')
                && !rest[..rest.len() - 1].contains(['(', ')']);
            (ty, ok)
        }
        None => (head, true),
    };

    TYPES.contains(&ty) && scope_ok && summary.starts_with(' ') && summary.trim().len() >= 3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_and_scoped_headers() {
        assert!(is_valid_header("feat: add thing"));
        assert!(is_valid_header("feat(map-stage): add tile caching"));
        assert!(is_valid_header("fix(ui)!: drop legacy flag"));
    }

    #[test]
    fn rejects_meta_commentary() {
        assert!(!is_valid_header(
            "It looks like the provided code diff is a significant change"
        ));
        assert!(!is_valid_header("Here is the commit message:"));
        assert!(!is_valid_header("feature: wrong type"));
    }

    #[test]
    fn extracts_message_after_preamble() {
        let raw = "Sure! Here is a commit message for your diff:\n\nfeat(map-stage): add tile caching\n\n- cache tiles by zoom";
        assert_eq!(
            extract_commit(raw).unwrap(),
            "feat(map-stage): add tile caching\n\n- cache tiles by zoom"
        );
    }

    #[test]
    fn strips_markdown_bold_header() {
        let raw = "**feat:** add tag support";
        assert_eq!(extract_commit(raw).unwrap(), "feat: add tag support");
    }

    #[test]
    fn classifies_diff_size() {
        let small = "diff --git a/f.rs b/f.rs\n--- a/f.rs\n+++ b/f.rs\n+one added line";
        assert!(!is_large_diff(small));

        let many_lines = format!(
            "diff --git a/f.rs b/f.rs\n--- a/f.rs\n+++ b/f.rs\n{}",
            "+line\n".repeat(60)
        );
        assert!(is_large_diff(&many_lines));

        let many_files = "diff --git a/a.rs b/a.rs\n+x\ndiff --git a/b.rs b/b.rs\n+x\ndiff --git a/c.rs b/c.rs\n+x";
        assert!(is_large_diff(many_files));
    }

    #[test]
    fn rejects_pure_rambling() {
        assert!(extract_commit("It looks like the provided code diff is a significant change that touches several files.").is_none());
    }
}
