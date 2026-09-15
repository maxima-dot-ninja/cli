# pocket

pocket is a CLI for the [Pocket AI API](https://docs.heypocketai.com/docs/api). It lists
your recordings, exports each one as a transcript and a summary, and searches the exports
in natural language.

pocket is written in **Rust** and builds to a single binary. Nothing from the old
TypeScript version is left, so there is no Bun, no `package.json` and no `node_modules`.

Search still runs through **qmd**, and pocket starts qmd with `npx`, so you need
**Node 22 or newer** on your PATH. Exporting needs it too, because every export re-indexes
search when it finishes.

## Install

Run this from the `pocket/` folder. It needs **Rust 1.88+**, because some of reqwest's
dependencies require it:

```sh
cargo install --path .
```

That puts the binary in `~/.cargo/bin/`. pocket is compiled, so rerun it after you change
the code.

qmd has no install step of its own. The first time pocket runs it, `npx -y @tobilu/qmd`
fetches the package into npm's cache, and nothing is installed globally.

pocket also ships a `SKILL.md`, so an agent can use it as a skill. Link it into Claude
Code with `ln -s "$(pwd)" ~/.claude/skills/pocket`, or mount it in vaulty with
`vaulty skills add pocket`.

## Auth

Get an app key from your [Pocket AI account](https://docs.heypocketai.com/docs/api).
Keys look like `pk_…`.

**The recommended place is `~/.config/secrets.env`**, the one file every tool in this repo
shares:

```sh
mkdir -p ~/.config && chmod 700 ~/.config
touch ~/.config/secrets.env && chmod 600 ~/.config/secrets.env
```

Add the line:

```sh
export POCKET_APP_KEY="pk_..."
```

Load it from `~/.zshrc`. pocket never opens this file itself; it reads
`POCKET_APP_KEY` from the environment, so the file only works once your shell sources it:

```sh
[ -f ~/.config/secrets.env ] && source ~/.config/secrets.env
```

**The alternative is a key file at `~/.config/pocket/key`.** Copy the key first, then:

```sh
mkdir -p ~/.config/pocket
pbpaste > ~/.config/pocket/key
chmod 600 ~/.config/pocket/key
```

Piping from the clipboard keeps the key out of your shell history. Don't `echo` it
into either file — that leaves a copy in `~/.zsh_history` forever.

`POCKET_APP_KEY` wins whenever it is set and not empty. An empty variable counts as
unset, so pocket falls back to the key file. The **key file** is trimmed of whitespace
and trailing newlines, so a file written by `pbpaste` works as-is. The **environment
variable is used exactly as set**, with no trimming.

**Check that it worked:**

```sh
pocket list     # prints your recordings, or "No API key found."
```

A key the API rejects prints a `✗` line with the HTTP status, followed by `0 recordings`.

> **Never put the key in this repo.** It belongs in `~/.config/` or the environment.
> Exports, config, and credentials all live outside the project tree by design.

`search` and `index` need no key, because they only read already-exported files. The bare
`pocket` menu **does need the key**, even for its search and index entries, because
the key check runs before the menu opens.

## Usage

```sh
pocket                     # interactive menu
pocket list                # list recordings as <id>  <date>  <title>
pocket export              # pick one recording from a menu and export it
pocket export <id>         # export a specific recording
pocket export all          # export every recording that list returns
pocket search "..."        # natural-language search over exports
pocket --search "..."      # same thing
pocket index               # re-scan the exports and update the search index
```

The menu offers search, list, export one, export all, and rebuild index. You move with
**↑/↓ and Enter**, or press a **digit** to jump to an entry, and Ctrl-C leaves it. The
menu's search entry asks for the query on a plain line.

There is **no `--help` flag**. `pocket --help`, like any other command pocket does not
recognise, opens the menu, and so does `pocket search` with no words after it. The menu
needs a terminal, so scripts and agents use the commands above instead. Without one,
pocket prints an error that names those commands and exits with status 1.

`list` prints the request it makes (`→ GET /public/recordings?limit=100`) first, then one
line per recording, and it ends with the count. A recording with no title shows as
`(untitled)`.

**`list` and `export all` only see the first 100 recordings.** pocket asks the API for
a single page of 100 and does not paginate, so anything past that is never listed or
exported.

`search` joins every word after it into one query, so quotes are optional there.
`--search` takes only the next word, so a multi-word query needs quotes. Flags can go
anywhere on the line. A word that matches a flag (`--json`, `--fast`, `-n`, `--search`)
is always taken as that flag, so it never reaches the query.

There are three search flags. `-n <count>` sets how many **recordings** to return, and it
falls back to 5 when it is missing or not a positive number. `--json` prints
machine-readable output, and `--fast` skips LLM reranking, which is much faster but
ranks worse.

## Exports

Exports always land in `~/dev/pocket-exports/<title>-<date>/` as `transcript.txt` and
`summary.md`, plus `raw.json` when there's no transcript. Override the location
with `POCKET_EXPORT_DIR`; an empty value counts as unset.

The folder name starts with the title in lowercase. Accented letters keep their base
letter, every run of anything other than `a-z` and `0-9` becomes one hyphen, and the
result is cut to 60 characters. A title with no Latin letters or digits at all, such as
one written only in Hebrew, becomes `untitled`, the same as a missing title.

The date follows as `YYYY-MM-DD`. It is the **UTC** day of the recording time, and it
falls back to the creation time, so an evening recording in New York can carry the next
day's date. It becomes `unknown-date` when neither time exists.

`transcript.txt` has one line per segment, written as `Speaker: text`, or just the text
when a segment has no speaker. `summary.md` starts with the title, date, duration in
seconds and recording ID. Then it holds each Pocket summary, followed by an
`## Action items` checklist in which done items are ticked and due dates and context are
added. It says `_No summary available._` when Pocket has none. `raw.json` is the whole
API answer, pretty-printed, so you can see what came back when the transcript was empty.

Each export prints its request, the size of each file it wrote, and a `✓` line that names
the folder. That line is how you find the files.

**Re-exporting overwrites the files in place.** Two recordings that map to the same
folder name overwrite each other, and search only ever sees the last one written. That
happens when two recordings share a title on the same day, when two titles share their
first 60 characters, and for every untitled or non-Latin-titled recording on the same
day. The Rust rewrite did not change this. During `export all`, two such recordings can
be written at the same moment, so the folder can end up with the transcript of one and
the summary of the other.

`export all` fetches **four recordings at a time** and prints each one as a single block
when it lands, so the order changes from run to run. The last lines say how many were
exported and name any recording that failed, and rerunning `export all` retries it.
pocket **exits with status 0 even when an export fails**, so a script has to look for
the `✗` line. Each file is written under a temporary name and renamed into place, so an
interrupted export never leaves half a file for search to index.

Every export, whether of one recording or all of them, re-indexes search when it
finishes. If `npx` is missing, the files are still written, and pocket then stops with
`Could not run npx` and exits with status 1.

The path is fixed rather than relative to the current directory so the search index
— and anything else reading these files — can find them from anywhere.

If you use `POCKET_EXPORT_DIR`, set it for **every run**, search included, and set it
**before the first index is built**. The qmd collection remembers the folder it was
created with, and later runs only update that collection, so changing the variable
afterwards keeps indexing the old folder.

## Search

Search runs on-device via [qmd](https://github.com/tobi/qmd), which pocket starts with
`npx -y @tobilu/qmd`. qmd needs Node 22+. It does hybrid BM25 and vector search with LLM
reranking, so it matches meaning rather than keywords.

Exports are indexed as a qmd collection named `pocket`, with the mask
`**/*.{md,txt}` so transcripts are covered alongside summaries. Exporting re-indexes
automatically, and `pocket index` does it by hand. Each re-index runs `qmd update` (or
`qmd collection add` the first time) and then `qmd embed`, which only embeds files that
have no vectors yet, so it is cheap once the index is warm.

pocket decides whether the collection exists by looking for a `pocket:` entry in
`~/.config/qmd/index.yml`. When there isn't one, the first search builds the index
before it runs.

qmd downloads about **2.2GB of GGUF models** to `~/.cache/qmd/models/` the first time it
needs them. The first index fetches the embedding model, which is about 330MB, and the
first search fetches the rest. After that a default search takes ~30-60s on CPU, mostly
in reranking, and `--fast` drops that to a couple of seconds.

Results are grouped by recording (not by chunk) and ranked by the recording's best
matching chunk, since the usual question is "which conversation was that?". pocket asks
qmd for four chunks for every recording it wants, so a search can return fewer
recordings than `-n` asked for when one recording owns many of the matching chunks.

Each result shows its score as a percentage, the title and date read back from the
folder name, the folder path, and its two best chunks as `file:line` with a snippet cut
at 220 characters. The title is the folder name with its hyphens turned back into spaces,
so it is lowercase and has lost its punctuation.

`--json` prints the same recordings as `{folder, title, date, dir, score, hits}` objects.
There `score` is qmd's raw score, and `hits` holds every chunk qmd returned for that
recording rather than just two, with `file` cut down to the file name inside the folder.

If qmd fails, its error prints on stderr and pocket reports `No matching conversations.`,
or `[]` with `--json`. An empty result therefore does not always mean nothing matched.

### Why pocket pins the qmd index

pocket runs qmd with `--index index`, from inside the export folder, to pin the global
index at `~/.cache/qmd/index.sqlite`. Without the flag, qmd walks up from the current
directory looking for a project-local `.qmd/` and would silently search an unrelated
index.
