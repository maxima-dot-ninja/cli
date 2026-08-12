# _cli

Personal command-line tools. One repo, three independent tools, no shared build.

| Tool | What it does | Stack |
|---|---|---|
| [**pocket**](pocket/README.md) | Export and search recorded conversations | Bun / TypeScript |
| [**lgit**](lgit/README.md) | AI-written git commit messages | Rust |
| **ccx** | Claude Code launcher with auto-named sessions | Bash |

Each tool stands alone — install only what you want.

## pocket

Pulls recordings from the [Pocket AI API](https://docs.heypocketai.com/docs/api) and
searches them in natural language.

```sh
pocket export all              # pull everything down
pocket --search "what did we decide about pricing"
```

Search runs **entirely on-device** via [qmd](https://github.com/tobi/qmd) — hybrid
keyword + vector search with local reranking. Nothing is uploaded.

Exports live in `~/dev/pocket-exports/`. Full docs: [pocket/README.md](pocket/README.md).

## lgit

Stage your changes, let an AI write the commit message. Supports Anthropic, OpenAI,
Gemini, and local Ollama, with optional GPG signing and auto-push.

```sh
git add -A
lgit
```

Full docs: [lgit/README.md](lgit/README.md).

## ccx

Claude Code with permissions bypassed and remote control on, plus automatic session
naming so parallel sessions are tellable apart.

```sh
cd dev/_www/croissant/api && ccx
# session is named croissant-api-000
```

The name is the **last two path components** plus a counter. A second session in the
same folder becomes `croissant-api-001`, a third `002`.

Numbers come from Claude's live session registry at `~/.claude/sessions/`, so:

- Closing `000` frees that number for the next session — they stay small
- A session killed without cleanup has its number reclaimed automatically

Pass your own `--name` to opt out. All other args go straight through to `claude`.

## Install

```sh
# pocket — needs bun
chmod +x pocket/pocket.ts
ln -s "$PWD/pocket/pocket.ts" /opt/homebrew/bin/pocket

# ccx — needs bash and claude
chmod +x ccx/ccx
ln -s "$PWD/ccx/ccx" /opt/homebrew/bin/ccx

# lgit — needs rust 1.70+
cargo install --path lgit
```

Symlinks rather than copies, so edits to the source are live immediately.

## Credentials

No secrets live in this repo. Every tool reads its keys from outside it:

| Tool | Reads from |
|---|---|
| pocket | `POCKET_APP_KEY`, else `~/.config/pocket/key` |
| lgit | `~/.config/lgit/config.toml`, or the provider's env var |
| ccx | — |

Keep it that way. `.gitignore` blocks `.env`, `*.key`, and `*.pem` as a backstop,
but the rule is that config belongs in `~/.config/`, never in the tree.
