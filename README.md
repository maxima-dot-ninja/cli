# _cli

Personal command-line tools. One repo, four independent tools, no shared build.

| Tool | What it does | Stack |
|---|---|---|
| [**agree**](agree/README.md) | Invoices, agreements and contacts on the Agree API | Rust |
| [**pocket**](pocket/README.md) | Export and search recorded conversations | Bun / TypeScript |
| [**lgit**](lgit/README.md) | AI-written git commit messages | Rust |
| **ccx** | Claude Code launcher with auto-named sessions | Bash |

Each tool stands alone — install only what you want.

## agree

Everything the [Agree API](https://secure.agree.com/documentation) exposes — invoices,
agreements, contacts, reports — plus optional natural-language commands.

```sh
agree invoices --status due       # deterministic
agree contacts samir              # find a contact by any fragment
```

Amounts are handled in integer cents internally, because the API bills in cents and
sending `50` for "$50" charges 50 cents. Full docs: [agree/README.md](agree/README.md).

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

# agree — needs rust 1.70+
cargo install --path agree
```

Symlinks rather than copies, so edits to the source are live immediately.

## API keys

**All keys go in one file: `~/.config/secrets.env`.** Nothing else needs editing.

```sh
mkdir -p ~/.config && chmod 700 ~/.config
touch ~/.config/secrets.env && chmod 600 ~/.config/secrets.env
```

Put your keys in it:

```sh
export AGREE_API_KEY="agr_..."        # agree
export POCKET_APP_KEY="pk_..."        # pocket
export ANTHROPIC_API_KEY="sk-ant-..." # lgit, agree (AI features)
export OPENAI_API_KEY="sk-..."        # alternative AI provider
export GOOGLE_API_KEY="..."           # alternative AI provider
```

Load it once from `~/.zshrc`:

```sh
[ -f ~/.config/secrets.env ] && source ~/.config/secrets.env
```

Open a new terminal and every tool picks them up. **Environment always wins over a
tool's own config file**, so this one file overrides everything.

### Where each tool looks

| Tool | Environment variable | Config file fallback |
|---|---|---|
| **agree** | `AGREE_API_KEY` | `~/.config/agree/config.toml` |
| **pocket** | `POCKET_APP_KEY` | `~/.config/pocket/key` |
| **lgit** | provider's own var (`ANTHROPIC_API_KEY`, …) | `~/.config/lgit/config.toml` |
| **ccx** | — | — |

Every config file lives in `~/.config/<tool>/` and is written `600`.

### Rules

- **Never put a key in this repo.** `.gitignore` blocks `.env`, `*.key`, and `*.pem`
  as a backstop, but the rule is that credentials live in `~/.config/`.
- **Never `echo` a key into a file** — it lands in `~/.zsh_history` permanently.
  Use `pbpaste >` or an editor.
- **Keep `secrets.env` out of your dotfiles repo** if you sync those.
