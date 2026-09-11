# pocket

CLI for the [Pocket AI API](https://docs.heypocketai.com/docs/api) — list and export recordings (transcript + summary), and search them in natural language.

Runs on [bun](https://bun.sh), no dependencies, no build step. Search also needs
**Node's `npx`** on your PATH, because that is how pocket runs qmd.

## Install

Run this from the `pocket/` folder:

```sh
chmod +x pocket.ts
ln -s "$(pwd)/pocket.ts" /opt/homebrew/bin/pocket
```

The script starts with `#!/usr/bin/env bun`, so `bun` has to be on your PATH as well.

pocket also ships a `SKILL.md`, so an agent can use it as a skill. Link it into Claude
Code with `ln -s "$(pwd)" ~/.claude/skills/pocket`, or mount it in vaulty with
`vaulty skills add pocket`.

## Auth

Get an app key from your [Pocket AI account](https://docs.heypocketai.com/docs/api).
Keys look like `pk_…`.

**Recommended — `~/.config/secrets.env`**, the one file every tool in this repo shares:

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

**Alternative — a key file at `~/.config/pocket/key`.** Copy the key first, then:

```sh
mkdir -p ~/.config/pocket
pbpaste > ~/.config/pocket/key
chmod 600 ~/.config/pocket/key
```

Piping from the clipboard keeps the key out of your shell history. Don't `echo` it
into either file — that leaves a copy in `~/.zsh_history` forever.

`POCKET_APP_KEY` wins if both are set. The **key file** is trimmed of whitespace and
trailing newlines, so a file written by `pbpaste` works as-is. The **environment
variable is used exactly as set**, with no trimming.

**Check it worked:**

```sh
pocket list     # prints your recordings, or "No API key found."
```

A key the API rejects prints a `✗` line with the HTTP status, followed by `0 recordings`.

> **Never put the key in this repo.** It belongs in `~/.config/` or the environment.
> Exports, config, and credentials all live outside the project tree by design.

`search` and `index` need no key — they only read already-exported files. The bare
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
**↑/↓ and Enter**, or press a **digit** to jump to an entry. Any command pocket does not
recognise also opens the menu.

**`list` and `export all` only see the first 100 recordings.** pocket asks the API for
a single page of 100 and does not paginate, so anything past that is never listed or
exported.

`search` joins every word after it into one query, so quotes are optional there.
`--search` takes only the next word, so a multi-word query needs quotes.

Search flags: `-n <count>` sets how many **recordings** to return (default 5), `--json`
prints machine-readable output, and `--fast` skips LLM reranking (much faster, worse
ranking).

## Exports

Exports always land in `~/dev/pocket-exports/<title>-<date>/` as `transcript.txt` +
`summary.md` (plus `raw.json` when there's no transcript). Override the location
with `POCKET_EXPORT_DIR`.

The folder name is the title in lowercase, with every run of other characters turned
into a hyphen and the result cut to 60 characters, followed by the date as
`YYYY-MM-DD`. The date comes from the recording time, falls back to the creation time,
and becomes `unknown-date` when neither exists.

`transcript.txt` has one line per segment, written as `Speaker: text`. `summary.md`
starts with the title, date, duration and recording ID, and then holds each Pocket
summary with its action items as a checklist.

**Re-exporting overwrites the files in place.** Two recordings with the same title on
the same day map to the same folder, so the second overwrites the first and search
only ever sees one of them.

The path is fixed rather than relative to the current directory so the search index
— and anything else reading these files — can find them from anywhere.

If you use `POCKET_EXPORT_DIR`, set it for **every run**, search included, and set it
**before the first index is built**. The qmd collection remembers the folder it was
created with, and later runs only update that collection, so changing the variable
afterwards keeps indexing the old folder.

## Search

Search is on-device via [qmd](https://github.com/tobi/qmd), run through
`npx -y @tobilu/qmd` (nothing installed globally). Hybrid BM25 + vector search with
LLM reranking, so it matches meaning rather than keywords.

Exports are indexed as a qmd collection named `pocket`, with the mask
`**/*.{md,txt}` so transcripts are covered alongside summaries. Exporting re-indexes
automatically; `pocket index` does it by hand. Each re-index runs `qmd update` (or
`qmd collection add` the first time) and then `qmd embed`, which only embeds files that
have no vectors yet, so it is cheap once the index is warm.

pocket decides whether the collection exists by looking for a `pocket:` entry in
`~/.config/qmd/index.yml`. When there isn't one, the first search builds the index
before it runs.

The first search downloads about **2GB of GGUF models** to `~/.cache/qmd/` and builds
the index. After that a default search takes ~30-60s on CPU, mostly reranking —
`--fast` drops that to a couple of seconds.

Results are grouped by recording (not by chunk) and ranked by the recording's best
matching chunk, since the usual question is "which conversation was that?".

Each result shows its score as a percentage, the title and date read back from the
folder name, the folder path, and its two best chunks as `file:line` with a snippet.
`--json` prints the same recordings as `{folder, title, date, dir, score, hits}` objects,
and there `hits` holds every matching chunk rather than just two.

### Note on `--index`

qmd is invoked with `--index index` to pin the global index at
`~/.cache/qmd/index.sqlite`. Without it, qmd walks up from the current directory
looking for a project-local `.qmd/` and would silently search an unrelated index.
