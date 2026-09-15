# ccx

**ccx is how you start Claude Code every day.** It runs `claude` with permissions bypassed and
remote control on, it names each session after the folder it runs in, and by default it picks up
the last session of that folder that is no longer running. A few subcommands list, summarize and
delete past sessions. ccx is one bash script, and it lives in the [_cli](../README.md) repo.

```sh
ccx                          # resume this folder's last ended session, or start a new one
ccx new                      # always start a new session
ccx --model opus             # any claude flag passes straight through
ccx new --name spike         # a new session under your own name, with no numbering
ccx details                  # every session of this folder, each with a one-sentence summary
ccx delete croissant-api-002 # delete a session by its name, from any folder
ccx clear-all                # delete every ended session of this folder
ccx help                     # print the command list
```

**`ccx cleanup` is listed in the help but does not work right now.** The section on
[cleanup](#ccx-cleanup-is-broken) says why.

## What plain `ccx` does

Running `ccx` first looks for a session to resume. It reads the transcripts Claude keeps for the
current folder in `~/.claude/projects/<slug>/`, and it picks the one whose **newest message** is
the most recent. It sorts on the timestamp of that last message rather than on the file's modified
time, because appending a title to a transcript would otherwise bump it to the top. A transcript
with no messages in it yet is skipped.

A session only counts as ended when **no live Claude process holds it**. ccx reads every record in
`~/.claude/sessions/<pid>.json`, keeps the ones whose pid is still alive, and skips any transcript
whose session id appears in one of them. A session sitting idle in another terminal is still
running, so ccx never resumes it out from under that terminal. The check covers every running
Claude on the machine, not only the ones in this folder.

When ccx finds a session, it resumes it **under that session's own title**, so the Remote Control
name matches what the `/resume` picker shows. A session started by plain `claude` has no title, and
ccx gives it a fresh name the same way it names a new session. When nothing is left to resume, ccx
starts a new session.

To start fresh, run **`ccx new`**. Passing your own `--name` to plain `ccx` also skips the resume
and starts a new session.

The folder is resolved with `pwd -P`, so a symlinked folder maps to its real path. Claude uses the
real path for its own folder names too, so the two agree.

## What it passes to `claude`

Every launch uses `exec`, so ccx replaces itself with `claude` and leaves nothing running behind
it. It runs one of these three command lines, and your own arguments always go at the end:

```sh
# resuming an ended session
claude --dangerously-skip-permissions --remote-control NAME --name NAME --resume SESSION_ID [your args]

# starting a new session
claude --dangerously-skip-permissions --remote-control NAME --name NAME [your args]

# starting a new session when you passed -n, --name or --name=...
claude --dangerously-skip-permissions --remote-control [your args]
```

`--dangerously-skip-permissions` turns off every permission prompt. `--remote-control NAME` opens
the session to Remote Control under the same name that `--name` puts in the prompt box, the
`/resume` picker and the terminal title.

**Arguments you give plain `ccx` go to the resumed session** when there is one, because they land
after `--resume`. Use `ccx new` when you want them on a fresh session instead. Any first word that
is not a subcommand counts as a `claude` argument, so a typo such as `ccx detail` resumes the last
session and hands it `detail` as a prompt.

With your own `--name`, ccx passes `--remote-control` with no value, so Claude Code picks the
Remote Control name itself. `--remote-control` takes an optional value, so **put a flag first** in
that case: in `ccx new "fix the build" --name build`, Claude reads `fix the build` as the Remote
Control name.

## How sessions are named

The name is the **last two parts of the folder path** plus a three-digit counter. Both parts are
lowercased, every run of characters other than letters and digits becomes one dash, and dashes at
the ends are trimmed. So `~/dev/_www/croissant/api` gives `croissant-api-000`, and `~/dev/_cli`
gives `dev-cli-000`. A path whose parts slug to nothing, such as `/`, gives `claude-000`.

The counter is the **lowest number from 000 to 999 that is not taken**. A name is taken when a
live session record in `~/.claude/sessions/` holds it, or when it has ever appeared as a title in
any transcript of any project under `~/.claude/projects/`. Claude writes the title into the
transcript as a `custom-title` line and repeats it every few turns, so an ended session keeps
holding its name. If all thousand numbers are taken, ccx falls back to the bare name with no
counter.

So **ccx never hands out a number while a transcript still carries it**, and the names it hands
out are unique across every project, not just within one folder. A number only comes free again
when every transcript that ever carried it is deleted. The next new session in that folder then
fills the gap, because ccx always takes the lowest free number.

Titles can still end up shared, because ccx only checks when it picks a name. A `/rename` can set
any title, for example. A resume keeps the transcript's title without checking it, so resuming a
session whose title a running session also holds gives two live sessions the same Remote Control
name. `cleanup` is the command meant to fix shared titles, but it is broken.

Picking a fresh name costs **one scan of every transcript**, and that wait comes before Claude
starts. With 303 transcripts totalling about 313 MB, the scan takes about **2.5 seconds**. Resuming
a session that already has a title skips the scan.

Pass **`--name`** (or `-n`) with your own name, and ccx does no naming at all.

## What each subcommand does

In every subcommand that takes `dir`, it means the folder you work in, such as `~/dev/_cli`, and it
defaults to the current folder. It never means the folder under `~/.claude/projects/`. A folder
with no transcripts prints `no transcripts for <path>`.

### `ccx details [dir]` lists sessions with a summary each

`details` lists every session of a folder, oldest first by first message. Each session gets one
line with its start date and its title in bold, or its session id when it has no title, and a
one-sentence summary indented under it. Running sessions are marked `(running)` and are not
summarized, since they are still changing. Each block prints as soon as its summary is ready, and
the last line counts the sessions and says how many are running.

To make a summary, ccx pulls **only what was said** out of the transcript, which means the user's
turns and the assistant's prose. It drops tool calls, tool results, injected skill text, subagent
chatter and any text that starts with `<`. It cuts each message to 400 characters and the whole
thing to 12,000 bytes, since the opening turns carry the topic. This step needs `jq`.

It sends that text to **`claude -p --model haiku`** with no tools and a short system prompt that
asks for one past-tense sentence under 20 words. It passes `--no-session-persistence`, so the
summarizer never shows up as a session itself. It runs from the cache folder, so it cannot pick up
a project's `CLAUDE.md`. It uses your **normal Claude Code login**, so it needs no API key and is
billed to your Claude subscription like any other Claude Code use.

Each summary is cached in **`~/.config/ccx/summaries/<session-id>`**. The first line of that file
is the timestamp of the transcript's last message, and the rest is the summary. A rerun reuses the
file while that timestamp still matches, so it costs nothing. A session you have resumed since has
a newer last message, so it is summarized again. Summaries run one at a time, so the first run over
a busy folder takes a while and reruns are instant.

A transcript with nothing said in it prints `(empty)`. A call that returns nothing prints
`(summary failed)`, and Claude's error output is thrown away, so the reason is not shown. **Neither
result is cached**, so the next run tries again.

### `ccx delete <name...>` deletes sessions by name

`delete` takes one or more titles, such as `ccx delete dev-cli-003 dev-cli-004`. It works from any
folder, because it searches the transcripts of every project for one whose **latest title**
matches. For each match it deletes the transcript, the session's `<id>/` folder of tool results and
subagent transcripts, its cached summary, and that project's `sessions-index.json`, which is a
cache Claude rebuilds from the transcripts.

A running session is reported as `(running, kept)` and left alone. A name that no transcript has
prints `not found`. **There is no confirmation prompt.** Rerunning with the same names only
reports what is already gone.

Each name stops at the **first transcript that matches**. When two transcripts share a title, one
run deletes only one of them, and if the first one found is running, the other is never reached.
Each name costs a full scan of every transcript, which takes about 3 seconds at the size above.

### `ccx clear-all [dir]` deletes every ended session of a folder

`clear-all` removes each ended session's transcript, its `<id>/` folder and its cached summary. It
also removes a leftover `<id>/` folder whose transcript is already gone. Then it removes the
project's `sessions-index.json`. **There is no confirmation prompt**, so it deletes as soon as you
press Enter.

**Running sessions are kept** and reported as `(running, kept)`. **`memory/` is never touched**,
and neither is anything else in the project folder that is not a session. Rerunning it deletes
nothing and only lists what is still running.

### `ccx cleanup` is broken

`ccx cleanup [dir]` and `ccx cleanup --all` appear in the help, but the script sends them to a
`cleanup` function that it never defines. Running either one makes bash report
`cleanup: command not found`, and nothing changes. The code meant to do the work,
`cleanup_project`, is in the script, but nothing calls it, and nothing handles `--all` at all.

`cleanup` is meant to renumber a folder's ended sessions so that no two share a title.
`cleanup_project` walks a project's sessions oldest first. A title stays when no earlier session
and no running session uses it, and otherwise the session gets the lowest free number for its base
name. The rename is one more `custom-title` line appended to the end of the transcript, which is
the same write `/rename` does, so nothing already in the file changes. Running sessions are
skipped and their names are reserved, since a live Claude would write its own name back over
anything appended. A second run finds every title unique and writes nothing.

### `ccx help` prints the command list

`help` prints the usage block from the script's header comment, so the help text and the header
are the same lines. `-h` and `--help` do the same thing.

## Which files it touches

- **`~/.claude/sessions/<pid>.json`** is Claude's live record of each running session. ccx only
  reads it, for the session id and the name, and it ignores records whose pid is gone.
- **`~/.claude/projects/<slug>/<session-id>.jsonl`** is a session's transcript. The slug is the
  folder's absolute path with every character other than letters, digits and dashes turned into a
  dash. ccx reads transcripts for titles and timestamps, and `delete` and `clear-all` delete them.
- **`~/.claude/projects/<slug>/<session-id>/`** holds a session's tool results and subagent
  transcripts, and `delete` and `clear-all` remove it along with the transcript.
- **`~/.claude/projects/<slug>/sessions-index.json`** is a cache Claude rebuilds, and `delete` and
  `clear-all` remove it.
- **`~/.claude/projects/<slug>/memory/`** is never read or written by ccx.
- **`~/.config/ccx/summaries/<session-id>`** is the summary cache. `details` creates it, and
  `delete` and `clear-all` remove it.

ccx needs **bash**, **Claude Code** (`claude`) on your `PATH`, and **jq** for `details`. macOS
ships jq in `/usr/bin`, and the other tools it uses, such as grep, sed and sort, come with the
system. There is nothing to configure and no key to set.

## Install

Run this from the root of the _cli repo:

```sh
chmod +x ccx/ccx
ln -s "$PWD/ccx/ccx" /opt/homebrew/bin/ccx
```

ccx is symlinked rather than copied, so edits to the script are live immediately.
