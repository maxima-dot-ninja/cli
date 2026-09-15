# waspy

**WhatsApp Spy.** waspy reads your WhatsApp from the terminal, straight out of the desktop app's own
database. It is read-only: it opens that database read-only and has no way to send, edit or delete
anything.

```sh
waspy                        # arrow-key menu
waspy chats                  # recent chats: when, unread count, name, last message
waspy unread                 # every chat with unread messages, and those messages
waspy read mum               # a chat's last 50 messages; any part of its name works
waspy read team -n 200 --since 2d
waspy search invoice march   # messages containing every word, newest first
waspy search lunch --chat sam
waspy ask what did jen last say to me   # a question in plain words, answered by Claude
waspy sync                   # refresh the copy now
waspy status                 # where it reads from, how fresh, whether the sync job runs
```

The four reads and `ask` take **`--json`** for scripts and agents, and with it a failure prints
`{"error": "..."}` too. `chats`, `read` and `search` take **`-n`** for how many results, and their
defaults are 30, 50 and 20. `read --since` takes an age such as `30m`, `6h`, `2d` or `1w`.
`read`, `search` and `ask` join every word after them, so quotes are optional.

**`search` matches words, not meaning.** It finds messages that contain every word you type, so
"last thing jen said to me?" finds nothing. A question in plain English, such as "which group is
the noisiest" or "what did jen last say to me", goes to **`ask`**.

You name a chat by its id, its exact name, or any part of its name that fits only one chat. When
several chats fit, waspy lists them and asks you to be more specific.

Bare `waspy` opens a menu with seven entries: recent chats, unread, read a chat, search, ask Claude,
sync now and status. **Search** is the word match, and **Ask Claude** takes a question in plain
English. "Read a chat" lets you type to filter your 300 most recent chats. The menu works
like pocket's: **↑/↓** (or j/k) and **Enter**, or a **digit** to jump to an entry, and **Esc** or
**q** to leave. It comes back after each entry. In a terminal, a command waspy does not know also
opens the menu.

## Asking in plain words

`waspy ask` hands your question to Claude Code (`claude -p`) running on your own login, so it is
billed to your Claude subscription and needs no API key. If `ANTHROPIC_API_KEY` is set in your
environment, Claude Code uses that key instead and the question is billed to it.

```sh
waspy ask what did jen last say to me
waspy ask when did sam send the invoice --model sonnet
waspy ask who asked about lunch on friday --json
```

Before Claude starts, waspy puts the **names of your 200 most recent chats** into its instructions,
each with whether it is a person or a group and the date of its last message. With the names in hand,
Claude goes straight to the right chat. Claude then reads the messages by running this same waspy
binary with `--json`. Each lookup it makes is printed as it happens, then the answer, then a line with
the model, the number of lookups and the seconds it took.

**The answer is printed formatted.** Claude is told it may use light markdown, and waspy renders it:
bold, italics, inline code, bullet and numbered lists, tables, quotes and links are styled and
word-wrapped to your window, so you never see raw `**`. Piped into a file or another command, the
answer keeps the same layout without colour, and `--json` returns the raw markdown for scripts.

Claude gets a single tool, Bash, and Claude Code refuses every command except this waspy binary's
four reads: `chats`, `unread`, `read` and `search`. So it cannot run `sync` or `ask`, pipe into
another program, read files or send anything, and any command it tries that is refused is printed in
yellow. waspy also starts Claude with no settings files, no MCP servers and no saved session, so every
question starts fresh and a follow-up has to say again who it means.

It uses **haiku** at low effort unless you pass `--model sonnet` or `--model opus`. With `--json` it
prints the question, the answer, the lookups, any refused commands, the model and the seconds.

Asked from a terminal, the chat list and every lookup read the live database like any other command.
Asked from the background, they all read the copy. It needs Claude Code installed and signed in. The
chat names and the messages Claude reads go to Anthropic, the same as anything else you show Claude.

## How it reads WhatsApp

The WhatsApp app for Mac keeps every chat in a plain SQLite database,
`~/Library/Group Containers/group.net.whatsapp.WhatsApp.shared/ChatStorage.sqlite`. Your terminal
may read it; a background process may not, because macOS protects other apps' data. So there are
two ways in, and waspy picks for you:

- **Live.** When you run waspy in a terminal, it reads the original, and results are current to
  the second. It counts as being in a terminal when its input, its output or its error stream is
  one, so `waspy chats --json | jq` still reads live. If macOS refuses the original anyway, waspy
  falls back to the copy.
- **The copy.** Anything running in the background, such as vaulty or a script, reads the copy in
  `~/.local/share/waspy/` (or `$XDG_DATA_HOME/waspy/`) and never touches the original. A launchd
  job checks every two minutes and copies again only when WhatsApp's database has changed.

Every reply says which one it read and how old the copy is. Text output has a line at the top, and
JSON output starts with `source` (`live` or `snapshot`) and `as_of`. Because an unchanged database is
not copied again, an old copy can simply mean nothing new has arrived.

The background never touches WhatsApp's folder because doing so without Full Disk Access makes
macOS ask "would like to access data from other apps", and for a command-line tool its **Allow is
not remembered**: the question comes back every run. Full Disk Access is the grant macOS keeps.

**`WASPY_LIVE`** is the one way around that rule. When it is set to anything, waspy tries the
original even with no terminal attached. `waspy ask` sets it for the lookups Claude runs, and only
when you asked from a terminal, because those lookups still run inside your terminal session. Set it
only for a process that also runs inside a terminal session. A real background process that sets
it, such as a launchd job or vaulty's daemon, opens WhatsApp's folder from the background, and that
is exactly what brings the dialog back.

The copy is made with SQLite's backup API, so it is consistent even while WhatsApp is writing, and
it is renamed into place, so a reader never sees half a copy. Every sync records its result in
`sync.json` next to the copy. `waspy status` shows whether the original is readable from where you
ran it, how old the copy is, when the job last copied and last checked, and whether the job is
loaded. The last check is what tells a quiet day from a dead job, and a sync that failed says why.

## Install

```sh
cd waspy && bin/install
```

It builds and installs `waspy` with `cargo install`, signs it with your Apple Development identity
(or the one named in `WASPY_SIGN_IDENTITY`), and schedules the sync job (`ninja.maxima.waspy.sync`),
which runs at login and every two minutes. It then waits for the job's first run. If that run could
copy WhatsApp, the grant is already in place and install stops there. Otherwise it makes a first copy
from your terminal, shows `waspy status`, and opens the Full Disk Access pane. It is safe to run
again.

**One step is yours:** add `~/.cargo/bin/waspy` under **System Settings → Privacy & Security → Full
Disk Access**. In the file picker, press ⌘⇧G and paste the path, because `~/.cargo` is hidden. Only
the waspy binary needs it; vaulty never does.

Because the binary is signed with a real identity, the grant survives rebuilds, as long as you
rebuild with `bin/install`. A plain `cargo install` leaves the binary ad-hoc signed, and so does
install itself when it finds no Apple Development identity. Either way the grant stops matching and
has to be given again.

Until then the sync job checks for the grant silently — the privacy database only opens with it,
and a refusal there puts up no dialog — and does nothing else, so it never pops anything up.
`waspy status` shows it waiting.

## As a vaulty skill

`SKILL.md` gives vaulty four read-only tools: `waspy_chats`, `waspy_unread`, `waspy_read` and
`waspy_search`. Add it with `vaulty skills add waspy`. vaulty runs in the background, so these tools
always read the copy.

Anything vaulty reads through waspy goes to its model, and that includes other people's messages to
you.
