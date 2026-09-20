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
| [**ccx**](ccx/README.md) | Claude Code launcher that resumes a folder's last session and names new ones | Bash | — |

Each tool stands alone — install only what you want.

## Installing vaulty

`install.sh` at the top of this repo is what a brand-new machine runs. vaulty's source is private,
so only the compiled binary is published here, attached to a release:

```sh
curl -fsSL https://raw.githubusercontent.com/maxima-dot-ninja/cli/main/install.sh | sh
vaulty login <code>
```

That has to be public: a machine you are setting up holds no credentials to authenticate with, and
a compiled binary carries no secrets.

The script picks the build for the machine's architecture, checks its SHA-256, and installs to
`~/.local/bin`. It writes no config and starts nothing — a fresh vaulty has nothing to run with,
and `vaulty login <code>` is what fixes that, with a code from `/spawn` on a machine that already
works. Releases are published by `./bin/release` in the vaulty repo.

## Skills

A tool with a `SKILL.md` beside it is also an **agent skill**, and that one file is read by two
harnesses:

- **vaulty** reads the YAML frontmatter — tool names, JSON-Schema inputs and argv templates — and
  mounts each tool for its agent. `vaulty skills add <tool>` brings a skill in, and it is live on
  the next message without a restart.
- **Claude Code** reads the same file's `name` and `description` and renders its markdown body. You
  add a tool there by linking it: `ln -s "$PWD/<tool>" ~/.claude/skills/<tool>`.

```
<tool>/
  src/          the CLI — you run it directly, as always
  SKILL.md      frontmatter = the machine contract, body = the prose
  README.md     for humans
```

The format is documented in vaulty at `src/core/skills/innate/SKILLS_GUIDE.md`, and
`vaulty skills check <tool>` reports every problem in a manifest at once. Two rules catch people out:

- **Every manifest must end with the `## Finish it — never hand it back` section**, copied from the
  guide, followed by one line saying what "done" means for that tool and which of its actions cannot
  be undone. vaulty refuses a manifest without it.
- **`vaulty skills add` copies a manifest from this repo only once.** After that, vaulty's own copy
  in its `skills/` folder is the source of truth, and a later edit here does not reach it until you
  copy the file over and run `vaulty skills add <tool>` again. A rejected `add` leaves its copy
  behind too, so fix that copy rather than the one here.

**Nothing about a skill changes how the tool works in a terminal.** The manifest only adds to it.

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
waspy ask what did jen last say to me   # a plain-English question; Claude answers, formatted
waspy status                   # where it reads from and how fresh that is
```

`search` matches words, not meaning. A question in plain English goes to `ask`, which has Claude
read your chats through waspy and prints the answer formatted, with bold, lists and tables.

Run in a terminal, waspy reads WhatsApp's database directly, so results are live. Anything in the
background, such as vaulty or a script, reads a copy instead and never touches the original, and
every reply says how old the copy is. A launchd job checks every two minutes and copies whenever
WhatsApp has changed, but only once `~/.cargo/bin/waspy` has **Full Disk Access**: without it,
macOS asks "would like to access data from other apps" on every run, because it does not remember
"Allow" for a command-line tool. Full docs: [waspy/README.md](waspy/README.md).

## vgoog

All of Google Workspace — Gmail, Calendar, Drive, Sheets, Docs, Slides, Forms, Tasks,
Contacts and Apps Script — as a terminal UI and a JSON command line, across several
Google accounts.

```sh
vgoog                  # open the TUI, or the setup wizard if nothing is configured
vgoog login            # browser sign-in with a Desktop OAuth client
vgoog accounts         # every account, its trust tier, and the address it acts as
vgoog doctor           # accounts, credential kinds, and whether Google is reachable
vgoog list             # every service and its action names
vgoog exec gmail list_messages '{"query":"is:unread","max_results":20}'   # one action, JSON back
```

It needs a one-time setup in Google Cloud: enable the Workspace APIs, create a **Desktop
app** OAuth client, and run `vgoog login`. On Workspace you can use a service account
with domain-wide delegation instead. vgoog can also rebuild its accounts from `VGOOG_*`
keys that it reads straight out of `~/.config/secrets.env`.

Every account has a **trust tier**. A `user` account is a person's own: vgoog reads, labels and
drafts in it but **never sends mail from it**. Only an `ai` account, the assistant's own mailbox,
can send. New accounts are `user` unless you sign them in with `vgoog login --tier ai`. See
[Trust tiers](vgoog/README.md#trust-tiers). Full docs: [vgoog/README.md](vgoog/README.md).

## pocket

Pulls your recordings from the [Pocket AI API](https://docs.heypocketai.com/docs/api) to
disk, as a transcript and a summary each, and searches them in natural language.

```sh
pocket                                  # arrow-key menu
pocket list                             # id, date and title of each recording
pocket export all                       # export recordings and re-index search
pocket search what did we decide about pricing
pocket search "pricing" -n 10 --fast --json   # skip reranking, print JSON for scripts
```

Search runs **entirely on-device** via [qmd](https://github.com/tobi/qmd), which does
hybrid keyword and vector search with local reranking, so nothing is uploaded. pocket
itself is one Rust binary, but it starts qmd through `npx`, so **search and export both
need Node 22+** (every export re-indexes). The first index and the first search download
about 2.2GB of models.

**`list` and `export all` only see the first 100 recordings**, because pocket reads a
single page from the API. **Two recordings can also land in the same folder** and
overwrite each other: a folder is named from the title and the day, so untitled
recordings from one day, titles with no Latin letters, and titles that match in their
first 60 characters all collide.

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

Launches Claude Code with permissions bypassed and Remote Control on. By default it **resumes this
folder's last ended session**; otherwise it starts a new one named after the folder, so parallel
sessions are easy to tell apart.

```sh
ccx                   # resume this folder's last ended session, or start a new one
ccx new               # always start a new, auto-named session
ccx details [dir]     # every session of a folder, each with a one-sentence summary
ccx delete <name...>  # delete sessions by name, from any folder
ccx clear-all [dir]   # delete every ended session of a folder
```

Plain `ccx` looks through this folder's transcripts in `~/.claude/projects/`, skips any session a
running Claude still holds, and resumes the one with the newest message under its own title. When
there is nothing to resume, or you pass your own `--name`, it starts a new session, and `ccx new`
always does. Every launch runs `claude --dangerously-skip-permissions --remote-control <name> --name
<name>`, adds `--resume <id>` when it resumes, and passes your own arguments on at the end.

A new session is named after the **last two parts of its path** plus the lowest free three-digit
number, so `~/dev/_www/croissant/api` starts at `croissant-api-000`. A name counts as taken while a
running session holds it or while any transcript still carries it as a title, so a number only
comes free again once its transcript is deleted.

`ccx details` prints each session's start date and name with a one-sentence summary written by
`claude -p --model haiku` on your normal login, so no API key is needed. Summaries are cached in
`~/.config/ccx/summaries/` and redone only when a session has grown. `delete` and `clear-all` never
touch a running session, `clear-all` keeps the folder's `memory/`, and neither asks before deleting.

**`ccx cleanup` does not work right now.** The script calls a function that was never written, so
it stops with `cleanup: command not found` and renames nothing. Full docs: [ccx/README.md](ccx/README.md).

## Install

```sh
# ccx — needs bash, claude, and jq (for ccx details)
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

# waspy — builds, signs, and schedules its sync job; safe to rerun. Then give
# ~/.cargo/bin/waspy Full Disk Access (it opens the pane) so the job can keep the copy fresh.
waspy/bin/install

# pocket — needs rust 1.88+, and node 22+ for search and export
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
