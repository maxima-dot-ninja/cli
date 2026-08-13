# lgit

AI-powered git commits. Stage your changes, let AI write the message.

```
┌─────────────────────────────────────────────┐
│  lgit — AI-powered commits                  │
└─────────────────────────────────────────────┘
```

## Features

- **AI-generated commit messages** — Analyzes your diff and writes conventional commit messages
- **Ask follow-up questions** — Ask about the change in plain English and get the real diff back, or tell it how to reword the message
- **Multiple AI providers** — Anthropic, OpenAI, Google Gemini, or local Ollama
- **GPG signing** — Sign commits with your GPG key, or commit unsigned
- **Auto push with smart retry** — Automatically pulls and retries if remote has new commits
- **PR link generation** — Get a quick link to create a PR on GitHub/GitLab

## Installation

```bash
# Clone and install
git clone https://github.com/a32ninja/lgit.git
cd lgit
cargo install --path .
```

## Quick Start

```bash
# First run — interactive setup
lgit

# Or explicitly run setup
lgit --setup
```

## Usage

### Basic Flow

```bash
# 1. Stage your changes (required!)
git add -A

# 2. Let lgit do the rest
lgit
```

> **Note:** lgit only commits staged changes. You must run `git add` first to stage the files you want to include. Unstaged changes will not be committed.

### Example Session

```
┌─────────────────────────────────────────────┐
│  lgit — AI-powered commits                  │
└─────────────────────────────────────────────┘

📁 Staged changes (3 files):

  added      src/new_feature.rs                  +142  -0
  modified   src/main.rs                         +12   -3
  modified   Cargo.toml                          +2    -0

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📝 Suggested commit:

  feat(core): add user authentication module

  - Implement JWT-based auth flow
  - Add login/logout endpoints
  - Update dependencies for jsonwebtoken crate

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

? What would you like to do?
❯ ✓ Accept and commit
  ✎ Edit message
  ? Ask about the changes
  ↻ Regenerate
  ✕ Cancel

? Select signing option
❯ 🔐 John Doe <john@example.com> (ABCD1234EFGH5678)
  🔐 Work Key <john@work.com> (WXYZ9876STUV5432)
  📝 Commit without signing

✓ Committed successfully (signed)!
ℹ Pushing to remote...
✓ Pushed successfully!

🔗 Create a pull request:
  https://github.com/user/repo/compare/feature-branch?expand=1
```

### Asking about the change

Pick **? Ask about the changes** and type a question in plain English. lgit does one
of two things depending on what you asked.

**Ask about the code, get the real diff.** Not a summary — actual `git diff --cached`
output for whichever files hold what you asked about:

```
Ask: tell me about the new daily breakdown endpoint

📄 src/api/stats.rs

  @@ -10,6 +10,18 @@
  +pub async fn daily_breakdown(range: DateRange) -> Result<Vec<DailyStat>> {
  +    let rows = db::query_daily(range).await?;
  +    Ok(rows.into_iter().map(DailyStat::from).collect())
  +}

  This adds the daily_breakdown handler, which queries per-day rows and
  maps them into DailyStat.
```

**Say how to change the message, get it rewritten:**

```
Ask: call this a fix, not a feat, and mention the migration

📝 Suggested commit:

  fix(api): correct daily breakdown totals and add migration
```

Questions stack up, and **Regenerate honours everything you've said** — so you can
narrow the message over a few turns instead of editing it by hand.

Two things worth knowing:

- It can only show files that are actually staged. Invented paths are dropped rather
  than guessed at, so it will never show you a diff for a file that isn't in the commit.
- A bare file name works — `stats.rs` resolves to `src/api/stats.rs`.

### Commands

```bash
lgit            # Run the commit flow
lgit --setup    # Re-run setup wizard
lgit --model    # Change AI model
lgit --config   # Show current configuration
lgit --gpginfo  # Show GPG key setup instructions
```

## Configuration

Config lives in your platform's config directory:

| Platform | Path |
|---|---|
| macOS | `~/Library/Application Support/lgit/config.toml` |
| Linux | `~/.config/lgit/config.toml` (or `$XDG_CONFIG_HOME/lgit/`) |

Run `lgit --config` to print the exact path on your machine.

```toml
[provider]
name = "anthropic"
model = "claude-sonnet-4-20250514"
api_key = "sk-ant-..."

[git]
auto_push = true
pr_link = true

[ui]
color = true
```

## Supported Providers

| Provider | Models | API Key Env Var |
|----------|--------|-----------------|
| Anthropic | Claude Sonnet 4, Opus 4, Haiku 3 | `ANTHROPIC_API_KEY` |
| OpenAI | GPT-5.2, GPT-5.1, GPT-5 Mini, GPT-4.1, GPT-4o | `OPENAI_API_KEY` |
| Google Gemini | Gemini 2.5 Pro, 2.5 Flash, 2.0 Flash | `GOOGLE_API_KEY` |
| Ollama | Any installed model | — |

## Smart Push

If the remote has commits you don't have locally, lgit automatically:

1. Detects the rejection
2. Pulls the latest changes
3. Retries the push

```
ℹ Pushing to remote...
ℹ Remote has new changes, pulling...
ℹ Retrying push...
✓ Pushed successfully!
```

## GPG Signing

lgit supports GPG-signed commits. On each commit, you choose whether to sign and which key to use:

```
? Select signing option
❯ 🔐 Personal <me@personal.com> (ABC123)
  🔐 Work <me@company.com> (XYZ789)
  📝 Commit without signing
```

### No GPG Keys?

If you don't have GPG keys set up, lgit will offer to create an unsigned commit:

```
⚠ No GPG keys found. Run `lgit --gpginfo` for setup instructions.
? No GPG keys found. What would you like to do?
❯ 📝 Commit without signing
  ✕ Cancel
```

### Setting Up GPG

Quick setup:

```bash
# Generate a new key
gpg --full-generate-key

# List your keys to get the key ID
gpg --list-secret-keys --keyid-format LONG
```

For detailed instructions, run:

```bash
lgit --gpginfo
```

Or see [docs/GPG_SETUP.md](docs/GPG_SETUP.md) for the complete guide.

## Requirements

- Rust 1.70+
- Git
- GPG (optional, for commit signing)
- API key for your chosen provider (or Ollama installed locally)

## License

MIT
