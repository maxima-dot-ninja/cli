# vgoog

A blazing fast, multi-account terminal UI for managing your entire Google Workspace — Gmail, Calendar, Drive, Sheets, Docs, Slides, Forms, Tasks, Contacts, and Apps Script — across all your Google accounts, from a single keystroke-driven interface.

Multi-account. Instant switching. One tool for every account. Built in Rust. 3.5MB binary. Zero bloat.

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
  Welcome to vgoog!             up/dn Select  ^A Account  q Q
```

---

## Philosophy

**Your tools should be faster than your thoughts.**

Every second you spend clicking through browser tabs, waiting for web UIs to load, and context-switching between Google apps is a second you're not doing real work. vgoog exists because managing Google Workspace from a browser is an unacceptably slow way to operate.

We believe:

- **The terminal is the power user's home.** If you live in the terminal, your Google Workspace should live there too. No Electron apps. No browser tabs. No loading spinners.
- **One interface, ten services.** You shouldn't need ten different apps to manage ten Google services. You need one. With consistent navigation, consistent keybindings, and zero learning curve between services.
- **Speed is a feature.** vgoog is written in Rust with async I/O, compiles to a 3.4MB static binary, starts instantly, and renders at 60fps. The only bottleneck is Google's API latency — and we handle pagination and token refresh transparently so you never wait for anything we control.
- **Text is the universal interface.** Every API response is browsable as structured JSON. Every action is a form you can fill out with your keyboard. No mouse required. No GUIs. Just text, terminals, and keystrokes.
- **Completeness matters.** vgoog doesn't just list your emails and call it a day. It exposes 219 API methods across 10 services — messages, threads, labels, drafts, filters, settings, delegates, forwarding, send-as aliases, calendar events with attendees, drive file uploads with multipart encoding, spreadsheet cell manipulation, document formatting, presentation slide management, form question builders, task hierarchies, contact groups, Apps Script deployments and remote execution. If Google's API supports it, vgoog lets you do it.
- **Trust the operator.** vgoog gives you the raw power of Google's APIs without hiding behind "are you sure?" dialogs for every action. Destructive operations get a single confirm prompt. Everything else executes immediately. You're an adult. You know what you're doing.

---

## Install

### From source

```bash
git clone <repo-url>
cd vgoog
cargo build --release
cp target/release/vgoog ~/.local/bin/  # or wherever your PATH looks
```

### Requirements

- Rust 1.70+ (build only)
- A Google Cloud project with OAuth2 credentials
- The following APIs enabled in your Google Cloud Console:
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

On first launch, vgoog walks you through configuration. You name the account and paste your OAuth credentials:

```
  ╔═══════════════════════════════════════╗
  ║     vgoog — Google Workspace TUI      ║
  ║          First-time Setup              ║
  ╚═══════════════════════════════════════╝

  Account name (e.g. work, personal): work
  Display label (e.g. Work Gmail): Work Gmail

  ── Manual Token Entry ──

  Get tokens from: https://console.cloud.google.com/apis/credentials
  Or use the OAuth Playground: https://developers.google.com/oauthplayground/

  GOOGLE_OAUTH_CLIENT_ID: ****
  GOOGLE_OAUTH_CLIENT_SECRET: ****
  GOOGLE_OAUTH_ACCESS_TOKEN: ****
  GOOGLE_OAUTH_REFRESH_TOKEN: ****
```

You need a Client ID, Client Secret, Access Token, and Refresh Token. See [Getting OAuth Credentials](#getting-oauth-credentials) below for step-by-step instructions. Once configured, vgoog automatically refreshes your access token when it expires — you never paste tokens again.

### Multi-Account Support

vgoog supports multiple Google accounts. Add as many as you need — work, personal, client accounts, etc.

Credentials are saved to `~/.config/vgoog/config.toml`:

```toml
active_account = "work"

[accounts.work]
label = "Work Gmail"

[accounts.work.auth]
client_id = "your-client-id.apps.googleusercontent.com"
client_secret = "GOCSPX-..."
access_token = "ya29.a0..."
refresh_token = "1//0e..."
token_expiry = 2026-02-20T12:00:00Z

[accounts.personal]
label = "Personal"

[accounts.personal.auth]
client_id = "..."
client_secret = "..."
access_token = "..."
refresh_token = "..."
token_expiry = 2026-02-20T12:00:00Z
```

Switch between accounts instantly with `Ctrl+A` inside the TUI. The active account is displayed in the header bar. Switching resets your view back to service selection so you start fresh with the new account's data.

vgoog automatically refreshes your access token when it expires (with a 2-minute safety buffer), saves the new token to disk, and never interrupts your workflow.

### Getting OAuth Credentials

1. Go to [Google Cloud Console](https://console.cloud.google.com/apis/credentials)
2. Create a project (or select an existing one)
3. Enable the APIs listed above under **Library**
4. Go to **Credentials** → **Create Credentials** → **OAuth Client ID**
5. Application type → **Desktop app**
6. Note down your Client ID and Client Secret
7. Use the [OAuth Playground](https://developers.google.com/oauthplayground/) to obtain access + refresh tokens with the scopes below

### Required Scopes

Request these scopes when generating tokens:

```
https://www.googleapis.com/auth/gmail.modify
https://www.googleapis.com/auth/gmail.compose
https://www.googleapis.com/auth/gmail.settings.basic
https://www.googleapis.com/auth/gmail.settings.sharing
https://www.googleapis.com/auth/calendar
https://www.googleapis.com/auth/drive
https://www.googleapis.com/auth/spreadsheets
https://www.googleapis.com/auth/documents
https://www.googleapis.com/auth/presentations
https://www.googleapis.com/auth/forms.body
https://www.googleapis.com/auth/forms.responses.readonly
https://www.googleapis.com/auth/tasks
https://www.googleapis.com/auth/contacts
https://www.googleapis.com/auth/script.projects
https://www.googleapis.com/auth/script.processes
```

---

## Usage

```bash
vgoog
```

That's it. You'll see the service selection screen. Navigate with your keyboard. Everything is a keystroke away.

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

 ────────────────────────────────────────────────────────────────
  Selected Gmail.               up/dn Select  ^A Account  Esc
```

**Action View** — list + JSON preview side by side:

```
  vgoog > Gmail                                      Work Gmail
 ────────────────────────────────────────────────────────────────
  Items (1-25)                   | Preview
                                 |
  > Weekly standup notes         | id: 18e4a2b3c5d6f7e8
    from: alice@company.com      | threadId: 18e4a2b3c5d6...
    Q3 Planning Doc - Review     | snippet: Hey team, here...
    from: bob@company.com        | labelIds: [2 items]
    Invoice #4521                | internalDate: 170801280...
    from: billing@vendor.io      | sizeEstimate: 4521
    Re: API migration timeline   | payload: {...}
    from: charlie@company.com    |
    Your flight confirmation     |
    from: noreply@airline.com    |

 ────────────────────────────────────────────────────────────────
  Loaded 25 messages         up/dn Detail  d Del  n Next  Esc
```

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

    Body*
    Sounds good, let's sync Thursday.█

 ────────────────────────────────────────────────────────────────
  Fill in fields              Tab Next  Enter Submit  Esc Cxl
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
  Switch account: up/dn select, Enter confirm, Esc cancel
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
  Loaded event detail             up/dn scroll  Esc Back
```

### Navigation Model

vgoog uses a simple hierarchical navigation:

```
Service Select  →  Action Select  →  Action View / Input Form
    (10 services)     (6-11 actions)    (list + preview | detail | form)
```

You can always go back with `Esc` or `q`. You never get lost.

### Keybindings

#### Global

| Key | Action |
|-----|--------|
| `Ctrl+C` | Quit immediately |
| `Ctrl+A` | Toggle account switcher overlay |
| `q` | Go back / quit (disabled in input forms) |

#### Service & Action Selection

| Key | Action |
|-----|--------|
| `↑` / `k` | Move up |
| `↓` / `j` | Move down |
| `Enter` | Select |
| `Esc` | Go back |

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
| `Enter` | Submit form |
| `Backspace` | Delete character |
| `Esc` | Cancel |

#### Confirmation Dialogs

| Key | Action |
|-----|--------|
| `y` | Confirm |
| `n` / `Esc` | Cancel |

---

## Services

### 📧 Gmail — 11 actions, 39 API methods

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

Full API coverage: messages (CRUD, batch modify, batch delete, attachments), threads (CRUD, modify), labels (CRUD), drafts (CRUD, send), settings (vacation, auto-forwarding, IMAP, POP, language), filters (CRUD), forwarding addresses, send-as aliases (CRUD, verify), delegates, profile, history.

---

### 📅 Calendar — 9 actions, 24 API methods

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

### 📁 Drive — 10 actions, 33 API methods

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

### 📊 Sheets — 8 actions, 19 API methods

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

### ✅ Tasks — 6 actions, 15 API methods

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

### 👥 Contacts — 6 actions, 21 API methods

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

### ⚡ Apps Script — 7 actions, 21 API methods

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
├── main.rs              Entry point, setup wizard, TUI event loop
├── config.rs            TOML config management (~/.config/vgoog/)
├── auth.rs              OAuth2 token refresh (2-min buffer, auto-save)
├── client.rs            HTTP client (GET/POST/PUT/PATCH/DELETE/multipart/download)
├── error.rs             Error types (API, Auth, HTTP, RateLimit, NotFound)
├── api/
│   ├── mod.rs           Module registry
│   ├── gmail.rs         Gmail API v1 — 39 methods
│   ├── calendar.rs      Calendar API v3 — 24 methods
│   ├── drive.rs         Drive API v3 — 33 methods
│   ├── sheets.rs        Sheets API v4 — 19 methods
│   ├── docs.rs          Docs API v1 — 14 methods
│   ├── slides.rs        Slides API v1 — 20 methods
│   ├── forms.rs         Forms API v1 — 21 methods
│   ├── tasks.rs         Tasks API v1 — 15 methods
│   ├── people.rs        People API v1 — 21 methods
│   └── apps_script.rs   Apps Script API v1 — 21 methods
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
- **One-time setup, zero friction.** Paste your OAuth credentials once during setup. Token refresh happens automatically forever after — you never touch tokens again.
- **Single binary, zero runtime dependencies.** Compiles with rustls (no OpenSSL), LTO, single codegen unit, stripped symbols. The result is a static 3.4MB binary that runs anywhere.

---

## Error Handling

vgoog surfaces errors transparently in the status bar:

| Error Type | When |
|-----------|------|
| `Auth error` | Token refresh fails (invalid credentials, revoked access) |
| `API error (4xx/5xx)` | Google API returns an error (quota, permission, bad request) |
| `Rate limited` | 429 Too Many Requests with retry-after |
| `Not found` | Resource doesn't exist |
| `Config error` | Missing or malformed config file |
| `HTTP error` | Network connectivity issues |

Errors are never swallowed. If something fails, you see exactly what Google told us.

---

## Build

```bash
# Development (fast compile, debug symbols)
cargo build

# Release (optimized, LTO, stripped — 3.4MB)
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
| `arboard` | Clipboard support |

---

## Platform Support

vgoog runs on **Windows, Linux, and macOS** with no platform-specific code.

| Component | Windows | Linux | macOS |
|-----------|---------|-------|-------|
| Terminal UI | Console API via crossterm | termios | termios |
| Config path | `%APPDATA%\vgoog` | `~/.config/vgoog` | `~/Library/Application Support/vgoog` |
| TLS / HTTP | rustls (pure Rust, no OpenSSL) | rustls | rustls |
| Clipboard | Native Win32 | X11 / Wayland | Native AppKit |
| File paths | Handled by `PathBuf` | POSIX | POSIX |

Every dependency was chosen to be cross-platform. `reqwest` uses `rustls-tls` so there is no OpenSSL dependency to wrestle with on any OS. `crossterm` talks directly to the Windows console API — no WSL or MSYS2 required. `dirs` resolves config directories correctly per-platform.

> **Note for Windows users:** The service menu uses emoji icons (📧 📅 📁 etc.). These render correctly in **Windows Terminal** and the default terminal on **Windows 11**. Legacy `cmd.exe` on older Windows 10 builds may display them as boxes. This is purely cosmetic — all functionality works regardless. We recommend [Windows Terminal](https://aka.ms/terminal) for the best experience.

---

## Stats

| Metric | Value |
|--------|-------|
| Lines of Rust | 6,692 |
| API methods | 219 |
| Google services | 10 |
| TUI actions | 79 |
| Auth method | Manual Token Entry (auto-refresh) |
| Multi-account | Yes (unlimited accounts) |
| Binary size (release) | ~3.5 MB |
| Dependencies | 12 direct |
| Compiler warnings | 0 |

---

## License

MIT
