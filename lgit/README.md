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
- **Multiple AI providers** — your Claude Code subscription, Anthropic, OpenAI, Google Gemini, or local Ollama
- **GPG signing** — Sign commits with your GPG key, or commit unsigned
- **Auto push with smart retry** — Automatically pulls and retries if remote has new commits
- **PR link generation** — Get a quick link to create a PR on GitHub/GitLab
- **Many repos at once** — `lgit --root` stages and commits every repo in a folder, with all the messages written in parallel
- **Plain-English status** — `lgit status` tells you what you're in the middle of, what could bite you, and what to do next

## Installation

lgit lives in the [_cli](../README.md) repo:

```bash
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

lgit sends the model the list of staged files and the staged diff, and asks for a
**Conventional Commits** message. A diff over **30,000 characters** is trimmed so that
every file keeps an equal share, and the file list always goes through whole. If the
reply does not start with a valid header such as `feat(scope): ...`, lgit asks **once
more**, and then it stops and shows you what the model said.

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

**✎ Edit message** opens the message in your editor (`$VISUAL`, or `$EDITOR` if that
is unset), and whatever you save becomes the new suggestion.

### Asking about the change

Pick **? Ask about the changes** and type a question in plain English. lgit does one
of two things depending on what you asked.

**Ask about the code, get the real diff.** Not a summary — actual `git diff --cached`
output for whichever files hold what you asked about:

```
Ask: tell me about the new daily breakdown endpoint

📄 src/api/stats.rs

  diff --git a/src/api/stats.rs b/src/api/stats.rs
  index 3f2a1c0..8b9d4e2 100644
  --- a/src/api/stats.rs
  +++ b/src/api/stats.rs
  @@ -10,6 +10,18 @@
  +pub async fn daily_breakdown(range: DateRange) -> Result<Vec<DailyStat>> {
  +    let rows = db::query_daily(range).await?;
  +    Ok(rows.into_iter().map(DailyStat::from).collect())
  +}

  This adds the daily_breakdown handler, which queries per-day rows and
  maps them into DailyStat.
```

Each file shows at most **200 lines** of diff, and lgit prints the
`git diff --cached -- <path>` command that shows the rest.

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

### Many repos at once

From a folder that holds several repositories, `lgit --root` runs `git add -A` in
each one, asks the model for every commit message at the same time, and then walks
you through the repos in the order their messages come back. Each stop is the normal
review loop, so you can edit, ask, regenerate, or cancel per repo. Cancel skips that
repo and moves on. Repos with nothing to commit are listed and left alone, and a
summary at the end shows what was committed, skipped, or failed.

It looks **one level down** only, so neither the folder itself nor repos nested deeper
are included, while a worktree (a folder with a `.git` file) counts. Adding `--tag`
tags every repo that gets committed.

### What's going on here?

`lgit status` is `git status` written for a person. It reads the branch and its
upstream, any merge or rebase in progress, stashes, how far the branch is from
`main`, the staged and unstaged diffs, and the first lines of new files. Then it
answers three questions:

- **What are you in the middle of?** It describes the work by what it is for, not by which files changed.
- **What could bite you?** It looks for conflicts, a staged change that won't build without an unstaged or untracked file, secrets, leftover debug code, junk that belongs in `.gitignore`, a stale fetch, and forgotten stashes.
- **What should you do next?** It gives the commands, and says how to split the work when it should be more than one commit.

```
▸ lgit
  On main, tracking origin/main: 2 ahead (last fetched 3 hours ago).
  Files: 1 staged, 2 unstaged, 1 untracked.

Summary
  You're adding an `lgit status` command. Only the README is staged; the code is not.

Watch out
  • `src/main.rs` declares `mod status;`, but `src/status.rs` is untracked, so
    committing what's staged now would not build.

Next
  • Stage everything with `git add -A` and commit it as one feature.
```

It never changes anything, and it never takes git's index lock, so it is safe to
run in the middle of anything. The first lines come straight from git and print
immediately; the explanation follows when the model replies. A clean repo that is
in sync gets a one-line answer and no model call at all.

Untracked files whose names look like secrets (`.env`, `*.pem`, `id_rsa`, and
so on) are listed by name only. Their contents are never sent to the model.

### Commands

```bash
lgit                  # Run the commit flow
lgit status           # Explain what's going on in this repo, in plain English
lgit --tag v1.0.0     # Commit, then tag it
lgit --root           # Commit every repo in this folder, one after another
lgit --setup          # Re-run setup wizard
lgit --model          # Change AI model (can switch providers)
lgit --key            # Manage API keys
lgit --config         # Show current configuration
lgit --gpginfo        # Show GPG key setup instructions
lgit --version        # Print the version
```

## Configuration

### Where to put your API key

lgit makes every request with the key saved in its **config file**. It reads your
provider's environment variable (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, or
`GOOGLE_API_KEY`) only while `lgit --setup`, `lgit --model`, or `lgit --key` is
running, and never when it commits.

**The easiest way is `lgit --setup`.** If the variable is set in your shell, setup
copies its value into the config file. If it isn't, setup asks for the key with the
input hidden. Don't `echo` a key into the config by hand, because that leaves a copy
in `~/.zsh_history` forever.

If you keep keys in `~/.config/secrets.env`, use your provider's standard variable:

```sh
mkdir -p ~/.config && chmod 700 ~/.config
touch ~/.config/secrets.env && chmod 600 ~/.config/secrets.env
```

```sh
export ANTHROPIC_API_KEY="sk-ant-..."   # or OPENAI_API_KEY / GOOGLE_API_KEY
```

Load it from `~/.zshrc`:

```sh
[ -f ~/.config/secrets.env ] && source ~/.config/secrets.env
```

Then run `lgit --setup` so the key is copied into the config.

**`lgit --model` does not copy the key.** If you switch to a provider whose variable is
set, lgit leaves `api_key` empty and that provider rejects every request. Use
`lgit --setup` to switch in that case. When the variable is not set, `--model` asks for
the key and saves it.

**`lgit --key`** shows, for each provider, whether its key is set in the environment,
set in the config, or not set. It saves a new key only for the provider you are using
now. For any other provider it prints an `export` line for your shell profile, and you
then switch with `lgit --setup` so the key lands in the config. It won't change a key
whose variable is already set in your environment.

Ollama runs locally and needs no key at all. The Claude Code provider needs no key either: it runs `claude -p` and bills your Claude subscription, so it works anywhere you are logged into Claude Code.

`lgit --config` shows whether a key is saved, masked as `••••••••`, but never the key
itself.

### Config file

Config lives at `~/.config/lgit/config.toml`, or `$XDG_CONFIG_HOME/lgit/` if you set
that. Same place on every platform, alongside `git`, `gh`, and everything else.

The file is written `600` — it holds an API key.

> **Moved in 0.2.1.** Config used to live in the OS config directory, which on macOS
> meant `~/Library/Application Support/lgit/`. That path is still read as a fallback
> when `~/.config/lgit/config.toml` is absent, so an old install keeps working. Copy
> your file over and delete the old one when convenient.

Run `lgit --config` to print the path actually in use.

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

`provider.name` is one of `claude_cli`, `anthropic`, `openai`, `gemini`, or `ollama`,
and `provider.model` is the model id that provider expects. `api_key` stays empty for
Claude Code and Ollama. `git.auto_push` pushes after every commit and `git.pr_link`
prints the pull request link, and both default to `true`.

**`ui.color` has no effect** yet, since nothing reads it. To turn colors off, set
`NO_COLOR=1` or `CLICOLOR=0` in your shell instead.

## Supported Providers

| Provider | Config name | Models (first is the default) | API Key Env Var |
|----------|-------------|--------|-----------------|
| Claude Code | `claude_cli` | sonnet, opus, haiku (whatever your subscription serves) | — (uses your Claude Code login) |
| Anthropic | `anthropic` | Claude Sonnet 4, Opus 4, Haiku 4.5 | `ANTHROPIC_API_KEY` |
| OpenAI | `openai` | GPT-5.2, 5 Mini, 5 Nano, 5.2 Pro, 5, 4.1 | `OPENAI_API_KEY` |
| Google Gemini | `gemini` | Gemini 3.1 Pro, 3 Flash, 3 Pro, 2.5 Flash, 2.5 Flash-Lite | `GOOGLE_API_KEY` |
| Ollama | `ollama` | Any installed model | — |

`lgit --model` lists exactly these and switches provider at the same time.

The Claude Code provider runs `claude -p` with its tools, settings files, MCP servers,
and memory turned off, so each call is a plain model call. It needs `claude` on your
`PATH` and a logged-in session. Ollama is reached at `http://localhost:11434`, and
setup offers whatever models `ollama list` reports.

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

lgit pushes only when `auto_push` is on. A branch with no upstream gets
`git push --set-upstream origin <branch>` on its first push. The pull is a plain
`git pull`, so your own merge or rebase setting applies, and if the push is still
rejected after it, lgit warns you and leaves it to you. With `--tag`, lgit pushes the
commit first and then runs `git push --tags`.

The PR link appears only for an `origin` on **github.com** or **gitlab.com**. On
GitLab it opens a new merge request.

## GPG Signing

lgit supports GPG-signed commits. On each commit, you choose whether to sign and which key to use. lgit finds your keys with `gpg --list-secret-keys` and signs with `git commit -S --gpg-sign=<key id>`.

```
? Select signing option
❯ 🔐 Personal <me@personal.com> (ABC123)
  🔐 Work <me@company.com> (XYZ789)
  📝 Commit without signing
```

### No GPG Keys?

**The `gpg` program itself must be installed**, even if you never sign. lgit runs it
before every commit to look for keys, and when it isn't on your `PATH` the commit stops
with `Failed to execute gpg`.

If `gpg` is installed but you don't have GPG keys set up, lgit will offer to create an unsigned commit:

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

- Building needs **Rust 1.83 or newer** and a C compiler, because cargo builds libgit2 from source.
- On Linux the build also needs the **OpenSSL** development headers and `pkg-config`.
- **Git** must be installed, since lgit runs the `git` command to commit, push, pull, and diff.
- **GPG** must be installed even if you never sign, because lgit runs `gpg` before every commit to list your keys.
- You need an API key for Anthropic, OpenAI, or Gemini, or else a logged-in **Claude Code** or a running **Ollama**.

## License

MIT
