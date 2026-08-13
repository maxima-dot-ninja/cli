# pocket

CLI for the [Pocket AI API](https://docs.heypocketai.com/docs/api) — list and export recordings (transcript + summary), and search them in natural language.

Runs on [bun](https://bun.sh), no dependencies, no build step.

## Install

```sh
chmod +x pocket.ts
ln -s "$(pwd)/pocket.ts" /opt/homebrew/bin/pocket
```

## Auth

Get an app key from your [Pocket AI account](https://docs.heypocketai.com/docs/api).
Keys look like `pk_…`.

**Recommended — `~/.config/secrets.env`**, the one file every tool in this repo reads:

```sh
mkdir -p ~/.config && chmod 700 ~/.config
touch ~/.config/secrets.env && chmod 600 ~/.config/secrets.env
```

Add the line:

```sh
export POCKET_APP_KEY="pk_..."
```

Load it from `~/.zshrc`:

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

`POCKET_APP_KEY` wins if both are set. Whitespace and trailing newlines are trimmed,
so a file written by `pbpaste` works as-is.

**Check it worked:**

```sh
pocket list     # prints your recordings, or "No API key found."
```

> **Never put the key in this repo.** It belongs in `~/.config/` or the environment.
> Exports, config, and credentials all live outside the project tree by design.

`search` and `index` need no key — they only read already-exported files.

## Usage

```sh
pocket                     # interactive menu
pocket list                # list recordings
pocket export              # pick one recording to export
pocket export <id>         # export a specific recording
pocket export all          # export everything
pocket search "..."        # natural-language search over exports
pocket --search "..."      # same thing
pocket index               # rebuild the search index
```

Search flags: `-n <count>` (default 5), `--json` for machine-readable output,
`--fast` to skip LLM reranking (much faster, worse ranking).

## Exports

Exports always land in `~/dev/pocket-exports/<title>-<date>/` as `transcript.txt` +
`summary.md` (plus `raw.json` when there's no transcript). Override the location
with `POCKET_EXPORT_DIR`.

The path is fixed rather than relative to the current directory so the search index
— and anything else reading these files — can find them from anywhere.

## Search

Search is on-device via [qmd](https://github.com/tobi/qmd), run through
`npx -y @tobilu/qmd` (nothing installed globally). Hybrid BM25 + vector search with
LLM reranking, so it matches meaning rather than keywords.

Exports are indexed as a qmd collection named `pocket`, with the mask
`**/*.{md,txt}` so transcripts are covered alongside summaries. Exporting re-indexes
automatically; `pocket index` does it by hand.

The first search downloads ~1GB of GGUF models to `~/.cache/qmd/` and builds the
index. After that a default search takes ~30-60s on CPU, mostly reranking — `--fast`
drops that to a couple of seconds.

Results are grouped by recording (not by chunk) and ranked by the recording's best
matching chunk, since the usual question is "which conversation was that?".

### Note on `--index`

qmd is invoked with `--index index` to pin the global index at
`~/.cache/qmd/index.sqlite`. Without it, qmd walks up from the current directory
looking for a project-local `.qmd/` and would silently search an unrelated index.
