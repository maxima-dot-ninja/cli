use crate::config::Config;
use crate::providers::{anthropic, claude_cli, gemini, ollama, openai};
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
- Describe what happened to the FILES, never the subject matter written inside them. Moving, deleting, or adding a document that describes an API is not a change to that API. If a file was moved or deleted, say it was moved or deleted.
- The file list you are given is the authoritative account of the change. Never claim a change to anything absent from it, and never let the prose inside a file override what the file list says happened to it.

Example of a good reply:

feat(map-stage): add tile caching for zoom levels

- cache rendered tiles keyed by zoom and viewport
- evict least-recently-used tiles above the memory limit

Example of a BAD reply, never do this:

It looks like the provided code diff is a significant change that..."#;

/// Reminder appended on retry when the first reply failed validation
const RETRY_NOTE: &str = "\n\nIMPORTANT: your previous reply was rejected because it did not start with a Conventional Commits header like \"feat(scope): summary\". Reply with ONLY the commit message this time.";

/// System prompt for follow-up questions. Unlike SYSTEM_PROMPT this one is allowed
/// to talk back — but only in two fixed shapes, so the reply stays parseable.
const FOLLOWUP_SYSTEM: &str = r#"You are helping a developer review a staged git change before they commit it.

You receive the staged files, the diff, the commit message currently proposed, and the conversation so far. Reply in ONE of exactly two formats and nothing else.

FORMAT 1 — show the developer real code. Use this when they ask about a file, a feature, an endpoint, or say things like "show me", "tell me about", "what changed in", "why".

SHOW: exact/path/one.rs, exact/path/two.md
NOTE: one to three sentences answering them in plain language.

FORMAT 2 — rewrite the commit message. Use this when they give guidance on wording, type, or scope, or tell you to write/finalize the message.

MESSAGE:
<the full Conventional Commits message, same rules as normal: type(scope): summary, optional "- " bullet body, plain text only>

Rules:
- Paths on the SHOW line must be copied exactly from the staged file list. Never invent a path, never guess at one that isn't listed.
- Pick the files that actually contain what they asked about — read the diff to decide.
- If their question needs no file, still use FORMAT 1 with an empty "SHOW:" line and answer in NOTE.
- Never emit both formats. Never add anything outside the format."#;

/// System prompt for `lgit status`. The reply is light markdown that
/// `ui::print_status_report` turns into terminal styling.
const STATUS_SYSTEM: &str = r###"You are looking over a developer's shoulder at their git repository and telling them what is going on, in plain English. They are a senior engineer, so skip git basics: never explain what staging, rebasing, or upstreams are.

You get the facts git reports, the staged and unstaged diffs, and the first lines of any new files. Use them to answer three questions: what am I in the middle of, is anything about to bite me, and what do I do next.

Reply with these sections as "## " headings, in this order. Leave a section out when it has nothing real to say.

## Summary
One to three sentences. Describe the work by what it is for (the feature, the fix, the refactor), not by which files changed, and say where it stands: clean, half-staged, ready to commit, mid-rebase, unpushed, and so on. If a merge, rebase, cherry-pick, or bisect is unfinished, lead with that.

## Changes
One "- " bullet per group of related changes, grouped by purpose rather than by folder. Say what each group does and whether it is staged, unstaged, untracked, or a mix. Name a file only when the name helps.

## Watch out
Only things that can really cause harm. Check for each of these:
- conflicts, and an unfinished merge, rebase, cherry-pick, or bisect
- a commit of only the staged part would be broken: staged code that uses something that exists only in an unstaged or untracked file, or a partly staged file whose staged half does not work alone
- secrets, keys, tokens, or passwords in the diffs or new files, and secret-looking files that are untracked and not ignored
- leftover debug output, conflict markers, or big commented-out blocks added in this change
- build output, dependencies, logs, or OS junk that belongs in .gitignore
- large or binary files about to be committed
- the upstream has commits this branch lacks, or the last fetch is more than a few days old so the ahead/behind counts can't be trusted
- the base branch has moved far past this branch
- a detached HEAD, a branch that was never pushed, or an upstream that no longer exists
- work left uncommitted for days, and stashes old enough to look forgotten
Leave this section out when nothing applies. Every point must name a problem that exists right now; if a point needs "worth knowing", "just in case", or "no risk" to make sense, drop it.

## Next
One to three concrete steps in order, with the exact command in backticks where one helps. When the work should be more than one commit, say which files go in each. Skip generic hygiene such as fetching, running tests, or re-reading code unless a specific fact calls for it.

Rules:
- Write full sentences, and keep it short. A typical reply is under 20 lines; a quiet repo needs three or four.
- State only what the facts and diffs show. When a diff or file was cut short, don't guess at the missing part and don't mention that it was cut.
- Ages in the facts are relative to right now.
- Use "- " bullets, `backticks` for paths and commands, and **bold** only for the one thing they must not miss. No other markdown: no tables, no code blocks, no numbered lists.
- Never end with a closing remark or an offer to help."###;

/// Generate a commit message using the configured AI provider.
///
/// `history` carries anything the developer said in follow-up turns, so a
/// regenerate after "call this a fix, not a feat" respects that instruction.
pub async fn generate_commit(
    config: &Config,
    summary: &str,
    diff: &str,
    history: &[String],
) -> Result<String> {
    // Instructions closest to the model's reply carry the most weight, so the
    // diff goes first and the ask comes after it.
    let body_ask = if is_large_diff(diff) {
        "This is a large change, so after the subject line add a blank line and a body: one \"- \" bullet point per distinct change."
    } else {
        "Add a bullet-point body after the subject only if the change has multiple distinct parts."
    };
    let feedback = if history.is_empty() {
        String::new()
    } else {
        format!(
            "\n\nThe developer has already discussed this change with you. Honour what they asked for:\n{}\n",
            history.join("\n")
        )
    };
    let prompt = format!(
        "Every file in this change, and what happened to it:\n{}\n\nThat list is complete and authoritative — the change is those files and nothing else.\n\nNow the diff. Lines starting with \"+\" are added, lines starting with \"-\" are removed, and unprefixed lines are unchanged context — do not describe those as changes. The diff may be trimmed for length; the file list above is not.\n\nGit diff:\n{}\n{}\nNow reply with ONLY the commit message for the changes above (no other text), starting directly with the type, e.g. \"feat(scope): ...\". Describe what happened to the files — if they were moved or deleted, say so. {}",
        summary, diff, feedback, body_ask
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

/// What the model decided to do with a follow-up question.
pub enum FollowUp {
    /// Show the real git diff for these files, plus a short spoken answer.
    Show { files: Vec<String>, note: String },
    /// Replace the proposed commit message.
    Message(String),
}

/// Answer a free-text follow-up about the staged change.
pub async fn follow_up(
    config: &Config,
    diff: &str,
    current: &str,
    staged_files: &[String],
    history: &[String],
) -> Result<FollowUp> {
    let prompt = format!(
        "Staged files — the only paths you may name:\n{}\n\nGit diff:\n{}\n\nCurrently proposed commit message:\n{}\n\nConversation so far:\n{}\n\nReply now, in FORMAT 1 or FORMAT 2.",
        staged_files.join("\n"),
        diff,
        current,
        history.join("\n"),
    );

    let raw = dispatch(config, FOLLOWUP_SYSTEM, &prompt).await?;
    Ok(parse_follow_up(&raw, staged_files))
}

/// Explain the state of a repository in plain English. `facts` is the report
/// `status.rs` builds from git.
pub async fn explain_status(config: &Config, facts: &str) -> Result<String> {
    let prompt = format!(
        "{facts}\n\nThat is everything git reports about this repository. Now tell me what is going on, using the sections from your instructions."
    );
    let raw = dispatch(config, STATUS_SYSTEM, &prompt).await?;
    Ok(strip_code_fences(raw.trim()).to_string())
}

/// A rewritten message wins if there is one; otherwise treat the reply as a
/// request to show files. An unparseable reply degrades to a note-only answer
/// rather than an error — the developer still sees what the model said.
fn parse_follow_up(raw: &str, staged_files: &[String]) -> FollowUp {
    let cleaned = strip_code_fences(raw.trim());

    if let Some((_, rest)) = cleaned.split_once("MESSAGE:") {
        if let Some(message) = extract_commit(rest) {
            return FollowUp::Message(message);
        }
    }

    let files = match tag_value(cleaned, "SHOW:") {
        Some(list) => resolve_files(&list, staged_files),
        None => Vec::new(),
    };
    let note = match cleaned.split_once("NOTE:") {
        Some((_, rest)) => rest.trim().to_string(),
        None if files.is_empty() => cleaned.to_string(),
        None => String::new(),
    };

    FollowUp::Show { files, note }
}

/// The remainder of the first line beginning with `tag`
fn tag_value(text: &str, tag: &str) -> Option<String> {
    text.lines()
        .map(str::trim_start)
        .find(|line| line.starts_with(tag))
        .map(|line| line[tag.len()..].trim().to_string())
}

/// Keep only paths that are genuinely staged. A model that answers "git.rs" or
/// "./src/git.rs" still resolves to the real "src/git.rs"; anything invented is
/// dropped, so we never try to diff a file that isn't there.
fn resolve_files(list: &str, staged: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    for raw in list.split(',') {
        let name = raw.trim().trim_matches('`').trim_matches('"').trim_matches('\'');
        if name.is_empty() {
            continue;
        }
        let hit = staged
            .iter()
            .find(|path| path.as_str() == name)
            .or_else(|| staged.iter().find(|path| path.ends_with(name)))
            .or_else(|| staged.iter().find(|path| base_name(path) == base_name(name)));

        if let Some(path) = hit {
            if !out.contains(path) {
                out.push(path.clone());
            }
        }
    }
    out
}

fn base_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
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
        "claude_cli" => claude_cli::generate(&config.provider, system, prompt).await,
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

    fn staged() -> Vec<String> {
        vec![
            "src/api/stats.rs".to_string(),
            "docs/README.md".to_string(),
        ]
    }

    #[test]
    fn follow_up_resolves_show_files_and_note() {
        let raw = "SHOW: src/api/stats.rs\nNOTE: The daily breakdown endpoint was added here.";
        match parse_follow_up(raw, &staged()) {
            FollowUp::Show { files, note } => {
                assert_eq!(files, vec!["src/api/stats.rs"]);
                assert!(note.starts_with("The daily breakdown"));
            }
            _ => panic!("expected Show"),
        }
    }

    #[test]
    fn follow_up_resolves_partial_and_decorated_paths() {
        // bare file name, ./ prefix, and backticks all resolve to the staged path
        let raw = "SHOW: `stats.rs`, ./docs/README.md";
        match parse_follow_up(raw, &staged()) {
            FollowUp::Show { files, .. } => {
                assert_eq!(files, vec!["src/api/stats.rs", "docs/README.md"]);
            }
            _ => panic!("expected Show"),
        }
    }

    #[test]
    fn follow_up_drops_invented_paths() {
        let raw = "SHOW: src/does/not/exist.rs\nNOTE: here you go";
        match parse_follow_up(raw, &staged()) {
            FollowUp::Show { files, .. } => assert!(files.is_empty()),
            _ => panic!("expected Show"),
        }
    }

    #[test]
    fn follow_up_takes_a_rewritten_message() {
        let raw = "MESSAGE:\nfix(api): correct daily breakdown totals\n\n- clamp the range";
        match parse_follow_up(raw, &staged()) {
            FollowUp::Message(m) => {
                assert!(m.starts_with("fix(api): correct daily breakdown totals"));
            }
            _ => panic!("expected Message"),
        }
    }

    #[test]
    fn follow_up_answers_questions_with_no_files() {
        let raw = "SHOW:\nNOTE: It is a feat because the endpoint is new.";
        match parse_follow_up(raw, &staged()) {
            FollowUp::Show { files, note } => {
                assert!(files.is_empty());
                assert!(note.contains("feat"));
            }
            _ => panic!("expected Show"),
        }
    }

    #[test]
    fn follow_up_degrades_to_a_note_when_format_is_ignored() {
        let raw = "That change adds pagination to the stats route.";
        match parse_follow_up(raw, &staged()) {
            FollowUp::Show { files, note } => {
                assert!(files.is_empty());
                assert_eq!(note, "That change adds pagination to the stats route.");
            }
            _ => panic!("expected Show"),
        }
    }

    #[test]
    fn rejects_pure_rambling() {
        assert!(extract_commit("It looks like the provided code diff is a significant change that touches several files.").is_none());
    }
}
