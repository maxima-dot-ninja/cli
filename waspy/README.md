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

Every read command takes **`--json`** for scripts and agents, and **`-n`** for how many results.
`read` and `search` join every word after them, so quotes are optional.

The menu works like pocket's: **↑/↓ and Enter**, or a **digit** to jump to an entry, and **Esc** or
**q** to leave. It comes back after each entry. A command waspy does not know also opens the menu.

## Asking in plain words

`waspy ask` hands your question to Claude Code (`claude -p`) running on your own login, so it is
billed to your Claude subscription and needs no API key. Claude gets a single tool, Bash, and Claude
Code refuses every command except this waspy binary's four reads. So it can list chats, read them and
search them, and nothing else. Each lookup it makes is printed as it happens, then the answer.

```sh
waspy ask what did jen last say to me
waspy ask when did sam send the invoice --model sonnet
waspy ask who asked about lunch on friday --json
```

It uses haiku unless you pass `--model`. Asked from a terminal, its lookups read the live database
like every other command. It needs Claude Code installed and signed in. The messages Claude reads go
to Anthropic, the same as anything else you show Claude.

## How it reads WhatsApp

The WhatsApp app for Mac keeps every chat in a plain SQLite database,
`~/Library/Group Containers/group.net.whatsapp.WhatsApp.shared/ChatStorage.sqlite`. Your terminal
may read it; a background process may not, because macOS protects other apps' data. So there are
two ways in, and waspy picks for you:

- **Live.** When you run waspy in a terminal, it reads the original, and results are current to
  the second.
- **The copy.** Anything running in the background, such as vaulty or a script, reads the copy in
  `~/.local/share/waspy/` and never touches the original. A launchd job refreshes it every two
  minutes, and every reply says how old it is.

The background never touches WhatsApp's folder because doing so without Full Disk Access makes
macOS ask "would like to access data from other apps", and for a command-line tool its **Allow is
not remembered**: the question comes back every run. Full Disk Access is the grant macOS keeps.

The copy is made with SQLite's backup API, so it is consistent even while WhatsApp is writing, and
it is renamed into place, so a reader never sees half a copy. `waspy status` shows the last sync,
and a sync that failed says why.

## Install

```sh
cd waspy && bin/install
```

It builds and installs `waspy`, signs it with your Apple Development identity, schedules the sync
job (`ninja.maxima.waspy.sync`), makes a first copy, and opens the Full Disk Access pane. It is safe
to run again.

**One step is yours:** add `~/.cargo/bin/waspy` under **System Settings → Privacy & Security → Full
Disk Access** (in the file picker, ⌘⇧G and paste the path; `~/.cargo` is hidden). Only the waspy
binary needs it; vaulty never does. Because the binary is signed with a real identity, the grant
survives rebuilds.

Until then the sync job checks for the grant silently — the privacy database only opens with it,
and a refusal there puts up no dialog — and does nothing else, so it never pops anything up.
`waspy status` shows it waiting.

## As a vaulty skill

`SKILL.md` gives vaulty four read-only tools: `waspy_chats`, `waspy_unread`, `waspy_read` and
`waspy_search`. Add it with `vaulty skills add waspy`.

Anything vaulty reads through waspy goes to its model, and that includes other people's messages to
you.
