# vgoog

A blazing fast, multi-account terminal UI and JSON command line for managing your entire Google Workspace — Gmail, Calendar, Drive, Sheets, Docs, Slides, Forms, Tasks, Contacts, and Apps Script — across all your Google accounts. Run `vgoog` for the keystroke-driven interface, or `vgoog exec` to script any action and get JSON back.

One tool handles every account, switches between them instantly, and ships as a single **4.4MB** Rust binary.

```
  vgoog -- Google Workspace Manager                  Work Gmail
 ────────────────────────────────────────────────────────────────

    > Gmail
      Calendar
      Drive
      Sheets
      Docs
      Slides
      Forms
      Tasks
      Contacts
      Apps Script

 ────────────────────────────────────────────────────────────────
  Welcome to vgoog!      ↑↓ Navigate  ⏎ Select  ^A Account  q Quit
```

---

## Philosophy

**Your tools should be faster than your thoughts.**

Every second you spend clicking through browser tabs, waiting for web UIs to load, and context-switching between Google apps is a second you're not doing real work. vgoog exists because managing Google Workspace from a browser is an unacceptably slow way to operate.

We believe:

- **The terminal is the power user's home.** If you live in the terminal, your Google Workspace should live there too. No Electron apps. No browser tabs. No loading spinners.
- **One interface, ten services.** You shouldn't need ten different apps to manage ten Google services. You need one. With consistent navigation, consistent keybindings, and zero learning curve between services.
- **Speed is a feature.** vgoog is written in Rust with async I/O, compiles to a 4.4MB static binary, starts instantly, and renders at 60fps. The only bottleneck is Google's API latency — and we handle pagination and token refresh transparently so you never wait for anything we control.
- **Text is the universal interface.** Every API response is browsable as structured JSON. Every action is a form you can fill out with your keyboard. No mouse required. No GUIs. Just text, terminals, and keystrokes.
- **Completeness matters.** vgoog doesn't just list your emails and call it a day. It exposes 237 API methods across 10 services — messages, threads, labels, drafts, filters, settings, delegates, forwarding, send-as aliases, calendar events with attendees, drive file uploads with multipart encoding, spreadsheet cell manipulation, document formatting, presentation slide management, form question builders, task hierarchies, contact groups, Apps Script deployments and remote execution. If Google's API supports it, vgoog lets you do it.
- **Trust the operator.** vgoog gives you the raw power of Google's APIs without hiding behind "are you sure?" dialogs for every action. Destructive operations in the TUI get a single confirm prompt, and `vgoog exec` asks nothing at all. Everything else executes immediately. You're an adult. You know what you're doing. The one exception is mail: vgoog never sends from a person's own account, only from one marked as the assistant's (see [Trust tiers](#trust-tiers)).

---

## Install

### From source

vgoog lives in the `_cli` repo. Cargo builds a release binary and installs it into `~/.cargo/bin`:

```bash
git clone <repo-url> _cli
cd _cli/vgoog
cargo install --path .
```

### Requirements

- You need **Rust 1.85+** to build it, because the locked `uuid` crate requires that version.
- You need a Google Cloud project with a **Desktop app** OAuth client, or a Workspace service account key with domain-wide delegation.
- The following APIs must be enabled in your Google Cloud Console:
  - Gmail API
  - Google Calendar API
  - Google Drive API
  - Google Sheets API
  - Google Docs API
  - Google Slides API
  - Google Forms API
  - Google Tasks API
  - People API
  - Apps Script API

---

## Setup

On first launch, vgoog walks you through configuration. You name the account, then choose how it signs in:

```
  ╔═══════════════════════════════════════╗
  ║     vgoog — Google Workspace TUI      ║
  ║     First-time Setup                  ║
  ╚═══════════════════════════════════════╝

  Account name (e.g. work, personal): work
  Display label (e.g. Work Gmail): Work Gmail

  How should this account authenticate?

    1  Sign in with Google        — opens a browser, catches the callback (recommended)
    2  Service account            — Workspace only, impersonates a user, no browser ever
    3  Paste tokens by hand       — when you already have them

  Choose [1]:
```

The default, option 1, only asks for the **Client ID** and **Client Secret** of a Desktop app OAuth client. vgoog then opens your browser, catches Google's redirect itself, and stores the refresh token it gets back. See [Getting OAuth Credentials](#getting-oauth-credentials) for creating the client, and [Authentication](#authentication) for the other two options. Once configured, vgoog automatically refreshes your access token when it expires — you never paste tokens again.

vgoog lowercases the account name and turns spaces into hyphens, and an empty name becomes `default`. The wizard is skipped entirely when `~/.config/secrets.env` already holds vgoog credentials (see [Restoring from the vault](#restoring-from-the-vault)).

### Multi-Account Support

vgoog supports multiple Google accounts. Add as many as you need — work, personal, client accounts, etc. The wizard only appears when no usable account exists, and the TUI has no add-account screen, so you add each further account with `vgoog login --account <name>` (see [Authentication](#authentication)). A login creates or replaces that account and makes it the active one.

Credentials and cached tokens are saved to `config.toml` in vgoog's config directory. That directory is `~/Library/Application Support/vgoog/` on macOS, `~/.config/vgoog/` on Linux and `%APPDATA%\vgoog\` on Windows, and setting **`VGOOG_CONFIG_DIR`** moves it anywhere else. The file holds refresh tokens and service account keys in plain text, and vgoog writes it with your default file permissions. It looks like this:

```toml
active_account = "work"
strategy = "auto"

[accounts.work]
label = "Work Gmail"
tier = "user"   # or "ai" — see Trust tiers

[accounts.work.auth]
client_id = "your-client-id.apps.googleusercontent.com"
client_secret = "GOCSPX-..."
access_token = "ya29.a0..."
refresh_token = "1//0e..."
token_expiry = 2026-02-20T12:00:00Z

[accounts.you]
label = "you@yourdomain.com (delegated)"
tier = "user"

[accounts.you.auth]
client_id = ""
client_secret = ""
access_token = "ya29.c0..."
refresh_token = ""
token_expiry = 2026-02-20T12:00:00Z

[accounts.you.service_account]
subject = "you@yourdomain.com"
key_json = "{ ...the whole key file... }"
scopes = ["https://www.googleapis.com/auth/gmail.modify", "..."]
```

An account with a `service_account` block signs in as that service account, acting as `subject`. Its `auth` fields then only cache the access token vgoog minted.

Switch between accounts instantly with `Ctrl+A` inside the TUI. The active account is displayed in the header bar. Switching resets your view back to service selection so you start fresh with the new account's data, and it saves that account as the active one, so later `vgoog exec` calls use it too.

vgoog automatically refreshes your access token when it expires (with a 2-minute safety buffer), saves the new token to disk, and never interrupts your workflow. A service account works the same way, except that each new token is minted from its key instead of a refresh token.

### Trust tiers

Every account has a tier. It says whose mailbox the account is, and so whether vgoog may send mail
from it.

| Tier | Whose mailbox | What vgoog does in it |
|---|---|---|
| `user` (the default) | a person's own | everything **except sending mail**: it reads, labels, archives, trashes and drafts |
| `ai` | the assistant's own | everything, sending included |

Google's scopes cannot draw this line, because `gmail.modify` — the scope that labels and archives —
also sends. So vgoog enforces it itself, at the one place every request passes through, before
anything leaves the machine. A send from a `user` account (`messages.send` or `drafts.send`) is
refused with `Denied:` and a pointer to save a draft instead, so the account's owner can send it.
The refusal deliberately does not say how to lift it: whoever reads it is usually the assistant, and
changing a tier is the owner's decision.

The scopes back this up wherever Google can. A `user` account is never granted `gmail.compose` or
`gmail.settings.sharing`, so it cannot touch forwarding, send-as or delegates either. That holds for
a delegated key too: even when the admin console authorised it for everything, the token vgoog mints
for a `user` account leaves those two out.

**Setting a tier.** The wizard asks "Whose mailbox is this?" when you add an account. On the command
line, `vgoog login --account <name> --tier ai` creates or replaces an account at that tier, and
`--tier` defaults to `user`. To change an existing account without signing in again, set
`tier = "ai"` or `tier = "user"` under its `[accounts.<name>]` table in `config.toml`; the next
vgoog command uses it. `vgoog accounts` and `vgoog status` print each account's tier.

An account saved before tiers existed has no `tier` line and loads as `user`. Nothing sends as a
person until someone has said it may.

### Getting OAuth Credentials

1. Go to [Google Cloud Console](https://console.cloud.google.com/apis/credentials)
2. Create a project (or select an existing one)
3. Enable the APIs listed above under **Library**
4. Go to **Credentials** → **Create Credentials** → **OAuth Client ID**
5. Application type → **Desktop app**
6. Note down your Client ID and Client Secret
7. Run `vgoog login --client-id ... --client-secret ...`, or pick option 1 in the wizard, and approve the consent screen in your browser. Skip the OAuth Playground: tokens minted there belong to Google's client rather than yours, so refreshing them fails with `unauthorized_client`.

### Authentication

Three ways in. `vgoog` with nothing configured runs the wizard and offers all three.

**1. Sign in with Google (recommended)**

```bash
vgoog login
```

Opens a browser, catches the redirect on a loopback port, exchanges the code itself. Create a
**Desktop app** OAuth client in the [Cloud console](https://console.cloud.google.com/apis/credentials)
and that is the whole setup — no redirect URIs to register, no OAuth Playground.

Credentials can also come from the environment, which is what makes this work straight after
`vaulty secrets pull`:

```bash
vgoog login                            # reads VGOOG_CLIENT_ID / VGOOG_CLIENT_SECRET, account "default"
vgoog login --client-id ... --client-secret ...
vgoog login --account personal         # adds or replaces the account named "personal"
```

vgoog listens on a random `127.0.0.1` port, prints the consent URL in case the browser does not open, and gives up after five minutes. It always forces Google's consent screen so that a refresh token comes back. If Google still returns none, revoke vgoog at myaccount.google.com/permissions and log in again. When the login succeeds, vgoog saves the account under the `--account` name, makes it active, and prints `{"account":"default","ok":true}`.

**2. Service account (Workspace only, deepest access)**

```bash
vgoog login --service-account ~/Downloads/key.json --subject you@yourdomain.com
```

No browser, no refresh token, works headless and in CI, and reaches Workspace-admin surfaces that
user consent cannot. Requires two things first:

- the service account has **domain-wide delegation** enabled, and you have its JSON key
- its **client id** is authorised in admin.google.com → Security → Access and data control →
  API controls → Domain-wide delegation, against the scopes below

> **Workspace access is not GCP IAM.** Giving the service account an IAM role in the Cloud console
> grants it nothing in Gmail, Drive or Calendar — those permissions live in the Workspace admin
> console and are granted separately. There are two ways to grant them:
>
> | | Direct sharing | Domain-wide delegation |
> |---|---|---|
> | How | Share one resource with the service account's email as Editor | Authorise its client id against a scope list |
> | Needs | Nothing special | A Workspace **Super Admin** |
> | Reaches | That one Drive folder / calendar | Every service, as any user in the domain |
> | Gmail | **No** — a mailbox cannot be shared with a robot | Yes |
>
> `vgoog` uses delegation, because Gmail is the point.

The delegated login requests **all 17 scopes** from `vgoog scopes --all`. Google rejects the whole token request if even one of them is missing from the admin console entry, so authorise every one. The login also accepts `--account` (default `default`), labels the account `<subject> (delegated)`, and stores the whole key file inside `config.toml`. Wizard option 2 does the same job interactively, and it prints the scopes and the key's client id for you to paste into the admin console.

**3. Paste tokens by hand** — offered by the wizard when you already have them.

### Restoring from the vault

`vgoog` rebuilds itself from `~/.config/secrets.env` when it has no usable config of its own —
written by `vaulty secrets pull`. That covers both a missing `config.toml` and one that holds no working account. vgoog reads the file itself, so the variables do not need to be exported in your shell, and **`VGOOG_SECRETS_ENV`** points it at a different file. Either credential kind restores:

```
VGOOG_SERVICE_ACCOUNT_KEY   the key file, minified to one line
VGOOG_SUBJECTS              comma-separated addresses — ONE ACCOUNT EACH, first is the default
VGOOG_AI_SUBJECTS           comma-separated addresses restored as ai accounts, the ones that may send
```

Addresses in `VGOOG_SUBJECTS` restore as `user` accounts and those in `VGOOG_AI_SUBJECTS` as `ai`
accounts (see [Trust tiers](#trust-tiers)). An address on both lists ends up `user`, so a mistake in
the file can never let vgoog send as a person.

Each subject becomes an account named after the part before the `@`, so `uri@yourdomain.com` becomes the account `uri`. For OAuth, the file needs these three values instead:

```
VGOOG_CLIENT_ID  VGOOG_CLIENT_SECRET  VGOOG_REFRESH_TOKEN
```

The OAuth values restore a single account named `default`, as a `user` account. A delegated key wins when both are present. Add `VGOOG_STRATEGY=oauth` to the file to restore the OAuth account instead, or `VGOOG_STRATEGY=service_account` to refuse the OAuth fallback when the key is missing or unreadable. Nothing to run: the next `vgoog` command picks it up
and writes its own config.

### Required Scopes

`vgoog scopes` prints them, ready to paste:

```bash
vgoog scopes         # the 15 an OAuth login requests
vgoog scopes --all   # 17, adding the Workspace-admin scopes a delegated key can use
```

A `user` account never holds two of them, `gmail.compose` and `gmail.settings.sharing`, whatever was
authorised (see [Trust tiers](#trust-tiers)).

### Checking it works

```bash
vgoog doctor
```

It prints one JSON object that lists every account, which kind of credential it holds, and whether it is usable. When a service account is configured, it also shows the key's email, client id and project. Last — the only check that means anything — it fetches the **active** account's Gmail profile to prove it can actually reach Google right now. `vgoog status` runs just that last check.

---

## Usage

```bash
vgoog
```

That's it. You'll see the service selection screen. Navigate with your keyboard. Everything is a keystroke away. Adding any command, such as `vgoog exec`, skips the TUI and prints JSON instead (see [Using the Command Line](#using-the-command-line)).

### Screenshots

**Action Select** — pick an action after choosing a service:

```
  vgoog > Gmail                                      Work Gmail
 ────────────────────────────────────────────────────────────────

    > Inbox
      Search
      Compose
      Labels
      Drafts
      Threads
      Filters
      Settings
      Forwarding
      Send-As
      Delegates
      Unified Search

 ────────────────────────────────────────────────────────────────
  Selected Gmail.   ↑↓ Navigate  ⏎ Select  ^A Account  Esc Back  q Quit
```

**Action View** — list + JSON preview side by side:

```
  vgoog > Gmail                                      Work Gmail
 ────────────────────────────────────────────────────────────────
  Items (more available)         | Preview
                                 |
  > Message 18e4a2b3c5d6         | id: 18e4a2b3c5d6f7e8
    Message 18e4a1f09b2c         | threadId: 18e4a2b3c5d6f7e8
    Message 18e49d7e4a10         |
    Message 18e49c02f3b7         |
    Message 18e49a55c8e1         |

 ────────────────────────────────────────────────────────────────
  20 messages loaded   ↑↓ Navigate  ⏎ Detail  d Delete  n Next  ^A Account  Esc Back
```

Gmail's list call returns only message and thread ids, so each row shows an id until you press `Enter` to open the full message.

**Input Form** — compose an email:

```
  vgoog > Gmail                                      Work Gmail
 ────────────────────────────────────────────────────────────────

    To*
    alice@company.com

    Subject*
    Re: Q3 Planning

    CC
    bob@company.com

    BCC

    Body*
    Sounds good, let's sync Thursday.█

 ────────────────────────────────────────────────────────────────
  Fill in fields              Tab Next Field  ⏎ Submit  Esc Cancel
```

**Account Switcher** — `Ctrl+A` overlay:

```
  vgoog -- Google Workspace Manager                  Work Gmail
 ────────────────────────────────────────────────────────────────
                                     ┌ Switch Account (^A) ────┐
    > Gmail                          │ > work *                │
      Calendar                       │   personal              │
      Drive                          │   client-acme           │
      Sheets                         └─────────────────────────┘
      Docs
      Slides
      Forms
      Tasks
      Contacts
      Apps Script

 ────────────────────────────────────────────────────────────────
  Switch account: ↑↓ select, Enter confirm, Esc cancel
```

**Detail View** — full JSON with syntax coloring:

```
  vgoog > Calendar                                   Work Gmail
 ────────────────────────────────────────────────────────────────

  {
    "kind": "calendar#event",
    "id": "abc123def456",
    "status": "confirmed",
    "summary": "Weekly Standup",
    "start": {
      "dateTime": "2026-02-20T10:00:00-05:00",
      "timeZone": "America/New_York"
    },
    "end": {
      "dateTime": "2026-02-20T10:30:00-05:00"
    },
    "attendees": [

 ────────────────────────────────────────────────────────────────
  Loaded event detail   ↑↓ Navigate  ⏎ Detail  d Delete  n Next  ^A Account  Esc Back
```

### Navigation Model

vgoog uses a simple hierarchical navigation:

```
Service Select  →  Action Select  →  Action View / Input Form
    (10 services)     (6-12 actions)    (list + preview | detail | form)
```

You can always go back with `Esc`, or with `q` outside input forms. You never get lost.

### Keybindings

#### Global

| Key | Action |
|-----|--------|
| `Ctrl+C` | Quit immediately |
| `Ctrl+A` | Toggle account switcher overlay (not in input forms) |
| `q` | Go back / quit (disabled in input forms) |

#### Service & Action Selection

| Key | Action |
|-----|--------|
| `↑` / `k` | Move up |
| `↓` / `j` | Move down |
| `Enter` | Select |
| `Esc` | Go back, or quit from the service list |

#### Action View (Lists & Detail)

| Key | Action |
|-----|--------|
| `↑` / `k` | Navigate list or scroll detail |
| `↓` / `j` | Navigate list or scroll detail |
| `Enter` | Open detail view for selected item |
| `d` | Delete selected item (with confirmation) |
| `n` | Load next page of results |
| `Esc` | Close detail / go back |

#### Input Forms

| Key | Action |
|-----|--------|
| `Tab` | Next field |
| `Shift+Tab` | Previous field |
| `Enter` | Submit form once every required (*) field is filled |
| `Backspace` | Delete character |
| `Esc` | Cancel |

#### Confirmation Dialogs

| Key | Action |
|-----|--------|
| `y` | Confirm |
| `n` / `Esc` | Cancel |

---

## Using the Command Line

Every command runs once, prints JSON, and exits, so scripts and agents can drive vgoog without the TUI.

- **`vgoog exec <service> <action> ['<json>'] [--account <name>]`** runs one action and prints the result.
- **`vgoog list`** prints every service and its action names as JSON.
- **`vgoog status`** fetches the active account's Gmail profile, which proves the credentials work.
- **`vgoog login`** adds or replaces an account without the wizard (see [Authentication](#authentication)).
- **`vgoog doctor`** reports every account and whether the active one can reach Google (see [Checking it works](#checking-it-works)).
- **`vgoog strategy [auto|service_account|oauth]`** prints the stored credential preference, or saves a new one (`sa` and `delegated` also mean `service_account`).
- **`vgoog scopes [--all]`** prints the scopes to authorise as one space-separated line.
- **`vgoog --version`** prints the version, which vaulty uses to check the installed binary against its skill manifest.

### Running actions

```bash
vgoog list                                   # every service and action
vgoog exec gmail list_messages '{"query":"from:samir is:unread","max_results":20}'
vgoog exec gmail modify_message '{"id":"18f...","add_labels":["Label_12"]}'
vgoog exec calendar list_events '{"timeMin":"2026-08-16T00:00:00Z","maxResults":10}'
vgoog exec tasks list_task_lists --account personal
```

The service is one of `gmail`, `calendar`, `drive`, `sheets`, `docs`, `slides`, `forms`, `tasks`, `contacts` or `apps_script`. The JSON argument is optional and defaults to `{}`.

A success prints `{"data": ..., "ok": true}` on stdout. A failure prints `{"error": "...", "ok": false}` on stderr, and vgoog exits with status 1.

Arguments use snake_case names, but Google's camelCase spellings work too. vgoog converts `maxResults` to `max_results` before dispatch and keeps the original key as well. A few Google names map to different vgoog names: `addLabelIds` becomes `add_labels`, `removeLabelIds` becomes `remove_labels`, `messageIds` becomes `ids`, `labelIds` becomes `labels`, `q` becomes `query` and `userId` becomes `id`.

`--account` runs that one call as a named account. If the call has to refresh that account's token, vgoog saves the config with that account marked active, so later calls without `--account` use it too.

Sending mail needs the message already built. `send_message`, `create_draft` and `update_draft` take a `raw` field holding the full RFC 2822 message, base64url-encoded, and the TUI's Compose form is what builds that for you.

`modify_message`, `modify_thread` and `batch_modify_messages` refuse to run without `add_labels` or `remove_labels`. Gmail's own error for that case reads like a missing permission, so vgoog names the missing argument instead.

`exec` reaches every API method except Drive's `upload_file`, `update_file_content`, `download_file` and `export_file`, so file uploads still go through the TUI's Upload action.

`vgoog strategy` saves its value in `config.toml`, and `doctor` reports it. No other code reads that saved value today. Which credential an account uses depends only on whether it has a `service_account` block, and what a vault restore builds depends on `VGOOG_STRATEGY` in secrets.env.

The `SKILL.md` beside this README turns these commands into agent tools. vaulty mounts `google` (which runs `exec`), `google_actions` (which runs `list`) and `google_status` (which runs `status`), and Claude Code reads the same file as a skill.

---

## Services

### 📧 Gmail — 12 actions, 55 API methods

| Action | What it does |
|--------|-------------|
| **Inbox** | List inbox messages with snippets, paginated |
| **Search** | Full Gmail search syntax (`from:`, `subject:`, `has:attachment`, etc.) |
| **Compose** | Send email with To, Subject, CC, BCC, and body |
| **Labels** | List all labels (system + user) |
| **Drafts** | List, view, and manage draft messages |
| **Threads** | Browse conversation threads |
| **Filters** | View Gmail filters |
| **Settings** | View vacation/auto-reply settings |
| **Forwarding** | Manage forwarding addresses |
| **Send-As** | Manage send-as aliases |
| **Delegates** | View and manage account delegates |
| **Unified Search** | Run one Gmail search across every connected account, up to 10 hits each, tagged with the account |

Full API coverage: messages (CRUD, batch modify, batch delete, attachments), threads (CRUD, modify), labels (CRUD), drafts (CRUD, send), settings (vacation, auto-forwarding, IMAP, POP, language), filters (CRUD), forwarding addresses, send-as aliases (CRUD, verify), delegates, profile, history.

---

### 📅 Calendar — 9 actions, 26 API methods

| Action | What it does |
|--------|-------------|
| **Today** | Events happening today |
| **Week View** | Next 7 days of events |
| **Events** | All upcoming events |
| **Quick Add** | Natural language event creation ("Meeting with Bob tomorrow at 3pm") |
| **Calendars** | List all calendars you have access to |
| **Create Event** | Full event creation with attendees, location, description |
| **ACL/Sharing** | View calendar sharing rules |
| **Settings** | Calendar settings |
| **Free/Busy** | Query free/busy time for a calendar |

Full API coverage: calendar list (CRUD), calendars (CRUD, clear), events (CRUD, move, quick add, instances), ACL rules (CRUD), settings, colors, free/busy queries.

---

### 📁 Drive — 10 actions, 34 API methods

| Action | What it does |
|--------|-------------|
| **My Files** | Root directory, sorted by last modified |
| **Search** | Drive search queries (`name contains 'report'`, etc.) |
| **Upload** | Upload a local file to Drive with mime detection |
| **Create Folder** | Create a new folder, optionally inside a parent |
| **Shared** | Files shared with you |
| **Recent** | Recently viewed files |
| **Starred** | Your starred files |
| **Trash** | Trashed files |
| **Storage Info** | Account storage quota |
| **Shared Drives** | List team/shared drives |

Full API coverage: files (CRUD, upload with multipart, copy, download, export, move), folders, permissions (CRUD), comments (CRUD), replies, revisions (list, get, delete), changes tracking, storage info, shared drives (CRUD).

---

### 📊 Sheets — 8 actions, 17 API methods

| Action | What it does |
|--------|-------------|
| **Open Sheet** | Browse spreadsheets from Drive |
| **Create Sheet** | Create a new spreadsheet |
| **Read Range** | Read cell values from any range (`Sheet1!A1:Z100`) |
| **Write Range** | Write values to a range (JSON array input) |
| **Append Data** | Append rows to a sheet |
| **Manage Sheets** | Add new sheets/tabs to a spreadsheet |
| **Named Ranges** | Create named ranges |
| **Sort** | Sort a range by column |

Full API coverage: spreadsheets (create, get, get with ranges), values (get, batch get, update, append, clear, batch update, batch clear), structural batch updates, sheet management (add, delete, rename), auto-resize columns, sort, named ranges.

---

### 📄 Docs — 7 actions, 14 API methods

| Action | What it does |
|--------|-------------|
| **Open Doc** | Browse documents from Drive |
| **Create Doc** | Create a new document |
| **Insert Text** | Insert text at a specific index |
| **Replace Text** | Find and replace across entire document |
| **Formatting** | Bold, italic, font size on a text range |
| **Headers/Footers** | Create document headers or footers |
| **Tables** | Insert a table at a specific position |

Full API coverage: documents (create, get), batch updates, text insertion/deletion, table insertion, inline images, text style updates (bold, italic, underline, font size), paragraph styles, find/replace, named ranges, page breaks, headers, footers.

---

### 📽 Slides — 8 actions, 20 API methods

| Action | What it does |
|--------|-------------|
| **Open Presentation** | Browse presentations from Drive |
| **Create Presentation** | Create a new presentation |
| **Add Slide** | Add a slide with a chosen layout |
| **Edit Text** | Find and replace text across all slides |
| **Shapes** | Create shapes (text boxes, rectangles, etc.) on a slide |
| **Images** | Insert images from URL onto a slide |
| **Tables** | Create tables on a slide |
| **Notes** | Add speaker notes to a slide |

Full API coverage: presentations (create, get), pages (get, thumbnail), batch updates, slides (create, delete, duplicate, move), text (insert, delete, replace all), shapes, images, tables, text style updates, shape properties, replace shapes with images, page properties, speaker notes.

---

### 📝 Forms — 7 actions, 21 API methods

| Action | What it does |
|--------|-------------|
| **Open Form** | Browse forms from Drive |
| **Create Form** | Create a new form with title |
| **Add Question** | Add text, choice, scale, date, or time questions |
| **Responses** | View form responses |
| **Watches** | View notification watches |
| **Settings** | Update form title and description |
| **Grid Questions** | Add grid/matrix questions with rows and columns |

Full API coverage: forms (create, get, batch update), responses (list, get), watches (CRUD, renew), question types (text, paragraph, multiple choice, checkbox, dropdown, scale, date, time, file upload, grid), section headers, item management (delete, move), form info updates, settings updates.

---

### ✅ Tasks — 6 actions, 14 API methods

| Action | What it does |
|--------|-------------|
| **Task Lists** | View all task lists |
| **View Tasks** | List tasks in a specific task list |
| **Create Task** | Create a task with title, notes, and due date |
| **Complete/Toggle** | Mark a task complete or incomplete |
| **Move Task** | Reorder or re-parent a task |
| **Clear Completed** | Remove all completed tasks from a list |

Full API coverage: task lists (CRUD), tasks (CRUD, complete, uncomplete, move, clear completed), subtask hierarchy support, due date filtering.

---

### 👥 Contacts — 6 actions, 20 API methods

| Action | What it does |
|--------|-------------|
| **Contacts** | List all contacts sorted by last name |
| **Search** | Search contacts by name, email, etc. |
| **Create Contact** | Create a contact with name, email, phone, organization |
| **Groups** | View contact groups and member counts |
| **Other Contacts** | Browse "Other contacts" (auto-saved from interactions) |
| **Directory** | Search organization directory |

Full API coverage: people (get, get me, batch get), contacts (list, search, CRUD, batch create/update/delete), contact groups (CRUD, modify members), other contacts (list, copy to contacts), directory search.

---

### ⚡ Apps Script — 7 actions, 16 API methods

| Action | What it does |
|--------|-------------|
| **Projects** | Browse script projects from Drive |
| **Create Project** | Create a new Apps Script project |
| **Edit Code** | Upload code files to a project |
| **Versions** | View project versions |
| **Deployments** | View project deployments |
| **Run Function** | Execute a function remotely with parameters |
| **Processes** | View running/completed script processes |

Full API coverage: projects (create, get, content get/update, metrics), versions (CRUD), deployments (CRUD), remote execution with parameters and dev mode, process monitoring, file helpers (server JS, HTML, manifest).

---

## Architecture

```
src/
├── main.rs              Entry point, setup wizard, TUI event loop, CLI command handling
├── config.rs            TOML config, accounts, strategy, restore from ~/.config/secrets.env
├── client.rs            HTTP client (GET/POST/PUT/PATCH/DELETE/multipart/download)
├── error.rs             Error types (API, Auth, HTTP, RateLimit, NotFound, Config)
├── auth/
│   ├── mod.rs           ensure_token — picks the OAuth or service account path per account
│   ├── oauth.rs         Token refresh for both paths (2-min buffer, auto-save)
│   ├── callback.rs      Browser sign-in: loopback redirect, code exchange
│   ├── service_account.rs  Signed JWT assertion → access token (domain-wide delegation)
│   └── scopes.rs        Every scope by service, shared by OAuth login and delegation
├── cli/
│   ├── mod.rs           clap commands: exec, list, status, login, doctor, strategy, scopes
│   ├── exec.rs          Routes exec calls to a service, and the action map for `list`
│   ├── args.rs          camelCase → snake_case argument normalising, plus aliases
│   └── gmail.rs …       One JSON-args dispatcher per service (10 files)
├── api/
│   ├── mod.rs           Module registry
│   ├── gmail.rs         Gmail API v1 — 55 methods
│   ├── calendar.rs      Calendar API v3 — 26 methods
│   ├── drive.rs         Drive API v3 — 34 methods
│   ├── sheets.rs        Sheets API v4 — 17 methods
│   ├── docs.rs          Docs API v1 — 14 methods
│   ├── slides.rs        Slides API v1 — 20 methods
│   ├── forms.rs         Forms API v1 — 21 methods
│   ├── tasks.rs         Tasks API v1 — 14 methods
│   ├── people.rs        People API v1 — 20 methods
│   └── apps_script.rs   Apps Script API v1 — 16 methods
└── ui/
    ├── mod.rs           UI module registry
    ├── app.rs           App state, 10 services, 5 screens, navigation
    └── views/
        ├── mod.rs       View module registry
        ├── render.rs    TUI rendering (header, status, lists, detail, forms, dialogs)
        └── handlers.rs  Action dispatch for all 10 services
```

### Key Design Decisions

- **No generated client code.** Every API call is hand-written with the exact URL, method, and JSON body. This means no OpenAPI codegen bloat, no version mismatches, and total control over every request.
- **`serde_json::Value` as the lingua franca.** API responses are returned as raw JSON values. This keeps the type surface small, avoids 500 struct definitions for 10 different APIs, and lets users browse the actual API response in the detail view.
- **Async all the way down.** Tokio runtime, async HTTP client, async token refresh. The TUI never blocks on I/O.
- **Multi-account from day one.** Every account is a named profile in a single TOML config file. Switching accounts is a single `Ctrl+A` hotkey. Token refresh happens per-account transparently.
- **One-time setup, zero friction.** Sign in once in the browser, or hand vgoog a service account key once. OAuth tokens refresh automatically forever after, and a service account mints its own, so you never touch tokens again.
- **Single binary, zero runtime dependencies.** Compiles with rustls (no OpenSSL), LTO, single codegen unit, stripped symbols. The result is a static 4.4MB binary that runs anywhere.
- **One scope list for both sign-in paths.** `auth/scopes.rs` feeds the OAuth consent URL and the admin console's delegation entry, so the two can never disagree.

---

## Error Handling

vgoog surfaces errors transparently in the status bar:

| Error Type | When |
|-----------|------|
| `Auth error` | Token refresh fails (invalid credentials, revoked access) |
| `API error (4xx/5xx)` | Google API returns an error (quota, permission, bad request) |
| `Rate limited` | 429 Too Many Requests with retry-after |
| `Not found` | Resource doesn't exist |
| `Denied` | vgoog refused the request itself, before sending it: mail sent from a `user` account, or from an `ai` account with no scope that can send |
| `Config error` | Missing or malformed config file |
| `HTTP error` | Network connectivity issues |

Errors are never swallowed. If something fails, you see exactly what Google told us.

In command-line mode the error arrives as `{"error":"...","ok":false}` on stderr, and vgoog exits with status 1. Service account token failures also carry a hint: `unauthorized_client` means the admin console has not authorised the key's client id for the requested scopes, and `invalid_grant` means delegation is off for the key or the user is not in the Workspace.

---

## Build

```bash
# Development (fast compile, debug symbols)
cargo build

# Release (optimized, LTO, stripped — 4.4MB)
cargo build --release

# Run directly
cargo run --release
```

### Build Profile

```toml
[profile.release]
opt-level = 3       # Maximum optimization
lto = true          # Link-time optimization
codegen-units = 1   # Single codegen unit for best optimization
strip = true        # Strip debug symbols
```

---

## Dependencies

| Crate | Purpose |
|-------|---------|
| `tokio` | Async runtime |
| `reqwest` | HTTP client (rustls TLS, multipart uploads) |
| `ratatui` + `crossterm` | Terminal UI framework |
| `serde` + `serde_json` + `toml` | Serialization |
| `chrono` | Date/time handling |
| `anyhow` + `thiserror` | Error handling |
| `base64` | RFC 2822 email encoding |
| `urlencoding` | URL parameter encoding |
| `uuid` | Generate unique IDs for Slides/Drive objects |
| `mime_guess` | Auto-detect file MIME types for uploads |
| `dirs` | Cross-platform config directory resolution |
| `clap` | Command-line parsing for `exec`, `login` and the other commands |
| `jsonwebtoken` | Signs the JWT assertion a service account trades for a token |

Cargo.toml also declares `arboard`, `tui-textarea`, `textwrap` and `unicode-width`, but no code uses them yet.

---

## Platform Support

vgoog runs on **Windows, Linux, and macOS**. Its only platform-specific code picks the command that opens your browser during sign-in: `open` on macOS, `xdg-open` on Linux and `explorer` on Windows.

| Component | Windows | Linux | macOS |
|-----------|---------|-------|-------|
| Terminal UI | Console API via crossterm | termios | termios |
| Config path | `%APPDATA%\vgoog` | `~/.config/vgoog` | `~/Library/Application Support/vgoog` |
| TLS / HTTP | rustls (pure Rust, no OpenSSL) | rustls | rustls |
| File paths | Handled by `PathBuf` | POSIX | POSIX |

Every dependency was chosen to be cross-platform. `reqwest` uses `rustls-tls` so there is no OpenSSL dependency to wrestle with on any OS. `crossterm` talks directly to the Windows console API — no WSL or MSYS2 required. `dirs` resolves config directories correctly per-platform.

> **Note for Windows users:** The service menu uses emoji icons (📧 📅 📁 etc.). These render correctly in **Windows Terminal** and the default terminal on **Windows 11**. Legacy `cmd.exe` on older Windows 10 builds may display them as boxes. This is purely cosmetic — all functionality works regardless. We recommend [Windows Terminal](https://aka.ms/terminal) for the best experience.

---

## Stats

| Metric | Value |
|--------|-------|
| Lines of Rust | 8,748 |
| API methods | 237 |
| Google services | 10 |
| TUI actions | 80 |
| CLI actions (`vgoog exec`) | 233 |
| Auth methods | Browser sign-in, service account with domain-wide delegation, or pasted tokens |
| Multi-account | Yes (unlimited accounts) |
| Binary size (release) | ~4.4 MB |
| Dependencies | 21 direct |
| Compiler warnings | 0 |

---

## License

MIT
