# _cli

Personal command-line tools. One repo, eight independent tools, no shared build.

| Tool | What it does | Stack | Skill |
|---|---|---|---|
| [**merc**](merc/README.md) | The whole Mercury banking API | Rust | ✅ |
| [**agree**](agree/README.md) | Invoices, agreements and contacts on the Agree API | Rust | ✅ |
| [**vgoog**](vgoog/README.md) | All of Google Workspace — Gmail, Calendar, Drive, Sheets, Docs | Rust | ✅ |
| [**pocket**](pocket/README.md) | Export and search recorded conversations | Rust | ✅ |
| [**waspy**](waspy/README.md) | Read your WhatsApp from the terminal, read-only | Rust | ✅ |
| [**phog**](phog/README.md) | PostHog — HogQL, people by fingerprint, dashboards as files | Rust | ✅ |
| [**lgit**](lgit/README.md) | AI-written commit messages, and a plain-English `status` | Rust | — |
| **ccx** | Claude Code launcher with auto-named sessions | Bash | — |

Each tool stands alone — install only what you want.

## Skills

A tool with a `SKILL.md` beside it is also an **agent skill**. One file, two readers:

- **[vaulty](https://github.com/…/vaulty)** parses the YAML frontmatter — tool names, JSON-Schema
  inputs and argv templates — and mounts them as tools. `vaulty skills add <tool>`. No restart.
- **Claude Code** reads the same file's `name` + `description` and renders the markdown body.
  `ln -s "$PWD/<tool>" ~/.claude/skills/<tool>`.

```
<tool>/
  src/          the CLI — you run it directly, as always
  SKILL.md      frontmatter = the machine contract, body = the prose
  README.md     for humans
```

Writing one: the format is documented in vaulty at `src/core/organs/SKILLS_GUIDE.md`, and
`vaulty skills check <tool>` reports every problem in a manifest at once.

**Nothing about a skill changes how the tool works in a terminal.** The manifest is additive.

## merc

Every one of the [Mercury API](https://docs.mercury.com/reference)'s 72 operations —
accounts, transactions, payments, cards, recipients, treasury, invoicing, webhooks.
Deterministic: no AI anywhere, the same input always makes the same request.

```sh
merc accounts                              # every account and its balance
merc transactions -n 20 --status=pending   # a group runs its list command, with its flags
merc call getAccountCards accountId=…      # or any operation by Mercury's own id
merc send                                  # guided payment, confirmed before it sends
merc config                                # where the token comes from, checked live
```

The commands are **generated from Mercury's OpenAPI spec at build time**, so coverage is
a fact rather than a promise, and a new Mercury endpoint becomes a working command by
refetching the spec and rebuilding. Amounts are parsed into integer cents and never
touch a float. An optional read-only token in `MERCURY_READ_KEY` handles every read,
so the main token is only used to move money. Full docs: [merc/README.md](merc/README.md).

## agree

Everything the [Agree API](https://secure.agree.com/documentation) exposes — invoices,
agreements, contacts, customers, templates, webhooks and reports. You can reach it three
ways: shortcuts, direct calls to all 38 operations, and plain English.

```sh
agree invoices --status due,failed        # shortcut, filtered by status
agree call mark_invoice_paid --yes id=…   # any operation directly
agree 'invoice Samir $5000 every week'    # plain English, then open for follow-ups
agree agent                               # open a conversation
agree model                               # pick the AI provider and model
```

**`--yes` must come before the `key=value` pairs**, because `call` reads everything after
the first pair as another pair. Scripts need `--yes`, since the confirmation prompt needs
a terminal.

The agent plans and calls tools one at a time, but never touches the API itself and
never sees your key — and every change stops for confirmation. Amounts are handled
in integer cents throughout, because the API bills in cents and sending `50` for
"$50" charges 50 cents. Full docs: [agree/README.md](agree/README.md).

## waspy

WhatsApp Spy. It reads your WhatsApp straight out of the desktop app's own database, and it is
read-only: it opens that database read-only and has no way to send, edit or delete anything.

```sh
waspy                          # arrow-key menu
waspy chats                    # recent chats: when, unread count, name, last message
waspy unread                   # every chat with unread messages, and those messages
waspy read mum --since 2d      # a chat's messages; any part of its name works
waspy search invoice march     # messages containing every word, newest first
waspy ask what did jen last say to me   # plain English, answered by Claude on your subscription
waspy status                   # where it reads from and how fresh that is
```

Your terminal can read WhatsApp's database directly, so results there are live. A process macOS
will not let in, such as vaulty's daemon, reads a copy that a launchd job refreshes every two
minutes, and every reply says how old it is. Full docs: [waspy/README.md](waspy/README.md).

## vgoog

All of Google Workspace — Gmail, Calendar, Drive, Sheets, Docs, Slides, Forms, Tasks,
Contacts and Apps Script — as a terminal UI and a JSON command line, across several
Google accounts.

```sh
vgoog                  # open the TUI, or the setup wizard if nothing is configured
vgoog login            # browser sign-in with a Desktop OAuth client
vgoog doctor           # accounts, credential kinds, and whether Google is reachable
vgoog list             # every service and its action names
vgoog exec gmail list_messages '{"query":"is:unread","max_results":20}'   # one action, JSON back
```

It needs a one-time setup in Google Cloud: enable the Workspace APIs, create a **Desktop
app** OAuth client, and run `vgoog login`. On Workspace you can use a service account
with domain-wide delegation instead. vgoog can also rebuild its accounts from `VGOOG_*`
keys that it reads straight out of `~/.config/secrets.env`.
Full docs: [vgoog/README.md](vgoog/README.md).

## pocket

Pulls recordings from the [Pocket AI API](https://docs.heypocketai.com/docs/api) and
searches them in natural language.

```sh
pocket                                  # arrow-key menu
pocket list                             # id, date and title of each recording
pocket export all                       # export recordings and re-index search
pocket search what did we decide about pricing
pocket search "pricing" --fast --json   # skip reranking, print JSON for scripts
```

Search runs **entirely on-device** via [qmd](https://github.com/tobi/qmd), which does
hybrid keyword and vector search with local reranking. Nothing is uploaded. qmd runs
through `npx`, so search needs Node, and the first search downloads about 2GB of
models.

**`list` and `export all` only see the first 100 recordings**, because pocket reads a
single page from the API.

Exports live in `~/dev/pocket-exports/`, one folder per recording, unless you set
`POCKET_EXPORT_DIR`. Full docs: [pocket/README.md](pocket/README.md).

## phog

[PostHog](https://posthog.com/docs/api) from the terminal. It runs HogQL queries, looks
people up by fingerprint, email or distinct id, and keeps dashboards as YAML files that
apply the same way every time.

```sh
phog query "select event, count() from events group by event"  # HogQL, printed as a table
phog events --fingerprint 3f9a…                                # one person's timeline across cookies
phog dashboards diff dashboards/ask-croissant.yaml             # what apply would change
phog dashboards apply dashboards/ask-croissant.yaml            # make PostHog match the file
phog call GET 'insights/?limit=5'                              # any API endpoint directly
```

`call` asks before any write unless you pass `--yes`, and deleting only sets PostHog's
`deleted` flag. Full docs: [phog/README.md](phog/README.md).

## lgit

Stage your changes and let an AI write the commit message. You can accept it, edit it,
ask about the change, or regenerate it, and then lgit commits, pushes and prints a PR
link. It works with your Claude Code subscription, Anthropic, OpenAI, Gemini, or a
local Ollama, and it can sign commits with GPG.

```sh
git add -A && lgit    # write the message for what's staged, then commit and push
lgit status           # what's going on here, what could bite you, what to do next
lgit --root           # commit every repo in this folder, messages written in parallel
lgit --tag v1.0.0     # commit, then tag and push the tag
lgit --model          # switch provider or model
```

Full docs: [lgit/README.md](lgit/README.md).

## ccx

Claude Code with permissions bypassed and remote control on. It also names each
session automatically, so parallel sessions are easy to tell apart.

```sh
cd dev/_www/croissant/api && ccx
# session is named croissant-api-000
```

The name is the **last two path components** plus a counter. A second session in the
same folder becomes `croissant-api-001`, and a third becomes `002`.

**A number is never reused.** ccx treats a name as taken when a running session holds
it (from `~/.claude/sessions/`) or when it has ever appeared as a title in any
transcript under `~/.claude/projects/`. Ended sessions keep their names in the
`/resume` picker without collisions, and a number only comes free again when its
transcript is deleted.

Pass your own `--name` to opt out. All other args go straight through to `claude`.

```sh
ccx details [dir]    # every session of a folder (default: here), with a summary
```

Each session gets its start date and name on one line, with a one-sentence summary
indented under it. The summary comes from `claude -p --model haiku` on your normal
login, so no API key is needed. Running sessions are listed but not summarized,
because they are still changing.

Summaries are cached in `~/.config/ccx/summaries/`, keyed on the transcript's last
timestamp. A resumed session is summarized again, and a failed call is not cached,
so the next run retries it. The first run takes a while and reruns are instant.

```sh
ccx cleanup [dir]     # renumber ended sessions so no two share a title
ccx cleanup --all     # the same, for every project Claude knows about
ccx delete <name...>  # delete sessions by name, from any folder
ccx clear-all [dir]   # delete every ended session of a folder
ccx help              # print this list
```

**Running sessions are never touched.** `cleanup` skips them and keeps their names
reserved, `delete` leaves them alone, and `clear-all` keeps them along with the
folder's `memory/`. Rerunning `cleanup` on a folder that is already tidy writes nothing.

## Install

```sh
# ccx — needs bash and claude
chmod +x ccx/ccx
ln -s "$PWD/ccx/ccx" /opt/homebrew/bin/ccx

# lgit — needs rust 1.83+, and gpg installed even for unsigned commits
cargo install --path lgit

# agree — needs rust 1.87+
cargo install --path agree

# merc — needs rust 1.88+
cargo install --path merc

# vgoog — needs rust 1.85+
cargo install --path vgoog

# phog — needs rust 1.88+
cargo install --path phog

# waspy — builds, signs, and schedules its sync job; safe to rerun
waspy/bin/install

# pocket — needs rust 1.88+, plus node for search
cargo install --path pocket
```

ccx is symlinked rather than copied, so edits to it are live immediately.
The Rust tools are compiled, so rerun `cargo install` after you change one.

## API keys

**All keys go in one file: `~/.config/secrets.env`.** Nothing else needs editing.

```sh
mkdir -p ~/.config && chmod 700 ~/.config
touch ~/.config/secrets.env && chmod 600 ~/.config/secrets.env
```

Put your keys in it:

```sh
export MERCURY_API_KEY="secret-token:..."   # merc
export MERCURY_READ_KEY="secret-token:..."  # merc, optional read-only token for reads
export AGREE_API_KEY="agr_..."              # agree
export POCKET_APP_KEY="pk_..."              # pocket
export POSTHOG_PERSONAL_API_KEY="phx_..."   # phog, the personal key, not the project key
export POSTHOG_PROJECT_ID="12345"           # phog, the number in the project's URL
export ANTHROPIC_API_KEY="sk-ant-..."       # lgit, agree (AI features)
export OPENAI_API_KEY="sk-..."              # alternative AI provider
export GOOGLE_API_KEY="..."                 # alternative AI provider
```

Load it once from `~/.zshrc`:

```sh
[ -f ~/.config/secrets.env ] && source ~/.config/secrets.env
```

Open a new terminal and every tool picks them up. **Environment wins over a tool's own
config file**, so this one file overrides almost everything. There are two exceptions:

- **agree's AI key** set as `api_key` under `[ai]` in agree's config beats the environment variable.
- **lgit only reads its key from its own config.** `lgit --setup` copies the environment
  variable in, so rerun it after you change a key. The Claude Code provider needs no key.

### Where each tool looks

| Tool | Environment variable | Config file fallback |
|---|---|---|
| **merc** | `MERCURY_API_KEY`, `MERCURY_READ_KEY` (optional), `MERCURY_SANDBOX` | `~/.config/merc/config.toml` |
| **agree** | `AGREE_API_KEY` | `~/.config/agree/config.toml` |
| **pocket** | `POCKET_APP_KEY` | `~/.config/pocket/key` |
| **phog** | `POSTHOG_PERSONAL_API_KEY`, `POSTHOG_PROJECT_ID`, `POSTHOG_APP_HOST` (optional) | `~/.config/phog/config.toml` |
| **vgoog** | reads its `VGOOG_*` keys from `secrets.env` itself; `VGOOG_CONFIG_DIR` moves its config | `~/Library/Application Support/vgoog/config.toml` on macOS |
| **lgit** | none when committing; `lgit --setup` copies `ANTHROPIC_API_KEY`, … into the config | `~/.config/lgit/config.toml` |
| **waspy** | — | — (reads WhatsApp's own database; nothing to configure) |
| **ccx** | — | — |

Every config file lives in `~/.config/<tool>/`, or under `$XDG_CONFIG_HOME` when that is set.
The tools write their config files `600`; a file you create by hand keeps whatever mode you give it.

**vgoog is the exception to both.** Its config sits in the OS config directory, which is
`~/Library/Application Support/vgoog/` on macOS, and it is written with default
permissions even though it holds refresh tokens and the service-account key.

### Rules

- **Never put a key in this repo.** `.gitignore` blocks `.env`, `*.key`, and `*.pem`
  as a backstop, but the rule is that credentials live in `~/.config/`.
- **Never `echo` a key into a file** — it lands in `~/.zsh_history` permanently.
  Use `pbpaste >` or an editor.
- **Keep `secrets.env` out of your dotfiles repo** if you sync those.
