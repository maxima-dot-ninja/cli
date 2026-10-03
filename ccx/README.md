# ccx

**ccx is how you start Claude Code every day.** It runs `claude` on Opus 5.5 at xhigh effort, in auto permission mode, with
remote control on. **By default every session is a ghost**, which means it is deleted, with every
trace of it, as soon as Claude exits. `ccx --persist` keeps sessions instead: it picks up the
folder's last session that is no longer running, or starts a new kept one named after the folder.
A few subcommands list, summarize and delete kept sessions. ccx is one bash script, and it lives in
the [_cli](../README.md) repo.

```sh
ccx                          # a new session that is deleted, with every trace, when claude exits
ccx --persist                # resume this folder's last ended session, or start a new kept one
ccx new                      # always start a new kept session
ccx --model sonnet           # any claude flag passes straight through, and yours win
ccx new --name spike         # a new kept session under your own name, with no numbering
ccx details                  # every session of this folder, each with a one-sentence summary
ccx delete croissant-api-002 # delete a session by its name, from any folder
ccx clear-all                # delete every ended session of this folder
ccx help                     # print the command list
```

**`ccx cleanup` is listed in the help but does not work right now.** The section on
[cleanup](#ccx-cleanup-is-broken) says why.

## What plain `ccx` does

Plain `ccx` starts a **new session** with the same defaults as `ccx new`, and **deletes it with
every trace** as soon as Claude exits. This throwaway session is called a ghost. Any claude flag
works, the same as with `ccx new`, but a ghost never resumes anything: Claude refuses to start
when `--resume` or `--continue` comes with the `--session-id` ccx passes, so use
`ccx --persist --resume ID` for that. ccx runs this command line and stays running underneath it
until Claude exits:

```sh
claude --model claude-opus-5-5 --effort xhigh --permission-mode auto --remote-control NAME --name NAME --session-id ID --settings HOOK [your args]
```

The name is the folder's base name with `-ghost` and a counter, such as `dev-cli-ghost-000`. It
only has to be free among running sessions, because a ghost's title is deleted along with it. So
ccx skips the transcript scan, and a ghost starts in about **50 ms** instead of 2.5 seconds. Your
own `--name` replaces it, just as with `ccx new`.

ccx keeps a **list of the ghost's session ids** in `~/.config/ccx/ghosts/<pid>`, named after its
own pid. It picks the first id itself and passes it as `--session-id`. A `/clear` inside a ghost
starts a new session with a new id, so ccx also passes a `SessionStart` hook through `--settings`
that adds every new session's id to the list. A session you open with `/resume` from inside a
ghost stays off the list, because it was a real session before the ghost opened it, so it is
never deleted.

When Claude exits, ccx deletes every session on the list and prints one line for each. For each
session it removes these:

- It removes the transcript and its `<id>/` folder from whichever project holds them, and that
  project's `sessions-index.json`.
- It removes the `/rewind` file history in `~/.claude/file-history/<id>/` and the saved shell env
  in `~/.claude/session-env/<id>/`.
- It removes the session's prompts from `~/.claude/history.jsonl`, which is the up-arrow list,
  and its cached summary, if one exists.

Every session appends to `history.jsonl`, so ccx filters the ghost's lines out of it instead of
deleting the file. It writes the result to a private temp file and renames that over the original,
and it skips the rewrite when the session has no lines in it. Each session takes about **45 ms**
to delete, and almost all of that is the history rewrite.

The cleanup also runs when you **close the terminal window** or stop ccx with a plain `kill`. On a
closed terminal, ccx sends its own output to `/dev/null` first. Nobody can see it then anyway, and
in macOS's bash 3.2 one write that fails on the dead terminal makes the next session's history
filter silently skip.

When ccx dies in a way it cannot catch, such as `kill -9`, its list stays behind. **Every ccx run
sweeps those lists first**, so any list whose ccx is gone gets deleted the same way before ccx does
anything else. A session on such a list that a live Claude still holds is reported as
`(running, kept)`, and its list stays until a later run finds it ended. `ccx --persist` also never
resumes a session that a running ghost has on its list.

Remote Control stays on, as with every ccx launch, and ccx only deletes what is on this machine.

## What `ccx --persist` does

`ccx --persist` is how you get a session that stays. It first looks for a session to resume. It
reads the transcripts Claude keeps for the
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
starts a new kept session.

To start a fresh kept session, run **`ccx new`**. Passing your own `--name` to `ccx --persist` also
skips the resume and starts a new kept session.

The folder is resolved with `pwd -P`, so a symlinked folder maps to its real path. Claude uses the
real path for its own folder names too, so the two agree.

## What it passes to `claude`

`ccx --persist` and `ccx new` use `exec`, so ccx replaces itself with `claude` and leaves nothing
running behind it. A ghost runs `claude` as a child instead, because ccx has to outlive it to
delete it, and its command line is in the section on [plain `ccx`](#what-plain-ccx-does). The kept
launches run one of these three command lines, and your own arguments always go at the end:

```sh
# resuming an ended session
claude --model claude-opus-5-5 --effort xhigh --permission-mode auto --remote-control NAME --name NAME --resume SESSION_ID [your args]

# starting a new session
claude --model claude-opus-5-5 --effort xhigh --permission-mode auto --remote-control NAME --name NAME [your args]

# starting a new session when you passed -n, --name or --name=...
claude --model claude-opus-5-5 --effort xhigh --permission-mode auto --remote-control [your args]
```

`--model claude-opus-5-5 --effort xhigh` pins the model and the effort level. Because your
arguments come last, passing your own `--model` or `--effort` overrides them.
`--permission-mode auto` lets a classifier approve safe actions on its own and stop to ask before
risky ones, instead of skipping every check. `--remote-control NAME` opens
the session to Remote Control under the same name that `--name` puts in the prompt box, the
`/resume` picker and the terminal title.

**Arguments you give `ccx --persist` go to the resumed session** when there is one, because they
land after `--resume`. Use `ccx new` when you want them on a fresh session instead. Any first word
that is not a subcommand counts as a `claude` argument, so a typo such as `ccx detail` starts a
ghost and hands it `detail` as a prompt.

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
  dash. ccx reads transcripts for titles and timestamps, and `delete`, `clear-all` and a ghost's
  cleanup delete them.
- **`~/.claude/projects/<slug>/<session-id>/`** holds a session's tool results and subagent
  transcripts, and `delete`, `clear-all` and a ghost's cleanup remove it along with the transcript.
- **`~/.claude/projects/<slug>/sessions-index.json`** is a cache Claude rebuilds, and `delete`,
  `clear-all` and a ghost's cleanup remove it.
- **`~/.claude/projects/<slug>/memory/`** is never read or written by ccx.
- **`~/.claude/file-history/<session-id>/`** and **`~/.claude/session-env/<session-id>/`** hold a
  session's `/rewind` snapshots and its saved shell env. Only a ghost's cleanup removes them.
- **`~/.claude/history.jsonl`** is the up-arrow list of prompts that every session appends to. Only
  a ghost's cleanup touches it, and it removes only the ghost's own lines.
- **`~/.config/ccx/summaries/<session-id>`** is the summary cache. `details` creates it, and
  `delete`, `clear-all` and a ghost's cleanup remove it.
- **`~/.config/ccx/ghosts/<pid>`** is a ghost's list of session ids. Plain `ccx` creates it, and
  the ghost itself or the next ccx run removes it once every session on it is deleted.

ccx needs **bash**, **Claude Code** (`claude`) on your `PATH`, and **jq** for `details` and
for ghosts. macOS ships jq in `/usr/bin`, and the other tools it uses, such as grep, sed, sort and
uuidgen, come with the system. There is nothing to configure and no key to set.

## Install

Run this from the root of the _cli repo:

```sh
chmod +x ccx/ccx
ln -s "$PWD/ccx/ccx" /opt/homebrew/bin/ccx
```

ccx is symlinked rather than copied, so edits to the script are live immediately.
