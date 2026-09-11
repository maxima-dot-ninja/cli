# agree

CLI for the [Agree API](https://secure.agree.com/documentation) — invoices, agreements,
contacts, customers and reports.

Written in Rust. Works fully without AI; natural-language commands are optional.

## Install

```sh
cargo install --path .
```

Needs Rust 1.87 or newer.

## API key

**Put it in `~/.config/secrets.env`:**

```sh
mkdir -p ~/.config && chmod 700 ~/.config
touch ~/.config/secrets.env && chmod 600 ~/.config/secrets.env
```

Add the line:

```sh
export AGREE_API_KEY="agr_..."
```

Load it from `~/.zshrc` (once, covers every tool):

```sh
[ -f ~/.config/secrets.env ] && source ~/.config/secrets.env
```

Open a new terminal, then check it:

```sh
agree config
```

```
  Config file : /Users/you/.config/agree/config.toml
  Base URL    : https://secure.agree.com
  Currency    : USD
  API key     : set (57 chars)
  AI provider : anthropic
  AI model    : claude-sonnet-5
```

### The alternative: a config file

If you'd rather not use the environment, put the key in
**`~/.config/agree/config.toml`**. When `XDG_CONFIG_HOME` is set, agree uses
**`$XDG_CONFIG_HOME/agree/config.toml`** instead.

```toml
api_key = "agr_..."
currency = "USD"

[ai]
provider = "anthropic"
model = "claude-sonnet-5"
api_key = ""
```

`currency` is the default for amounts typed without one, and it falls back to `USD`.
`agree model` writes the `[ai]` table for you. **`AGREE_API_KEY` wins if both are set.**
Whenever agree writes the file itself, it sets it to `600`. A file you create by hand
keeps whatever permissions you give it.

> Don't `echo` a key into a file — it stays in `~/.zsh_history` forever. Use
> `pbpaste >` or an editor.

## Usage

There are three ways in, and none of them is the only way to reach anything.

**Shortcuts** for the things you do constantly:

```sh
agree config                      # where config lives, whether a key is set
agree invoices                    # recent invoices
agree invoices --status due       # filter: created, due, sent, paid, failed, ...
agree invoices --customer acme    # fuzzy company match
agree contacts                    # the first 50 contacts
agree contacts samir              # match name, email or company
agree customers                   # requires the customers feature on your org
agree agreements
agree templates
```

Add `--json` to any of them for raw JSON.

Every list shortcut except `templates` takes **`--limit`**. It defaults to 20 rows, or
50 for `contacts`, and **`--limit 0` fetches everything**. `--status` takes a
comma-separated list, so `--status due,failed` works too.

**Every API operation, directly.** Nothing is AI-only:

```sh
agree tools                                    # list all 38 operations
agree call get_invoice id=6d3571fa-…
agree call list_invoices statuses=due,failed
agree call delete_invoice id=6d3571fa-…        # asks before it runs
agree call mark_invoice_paid --yes id=…        # skip the confirmation
```

Arguments are `key=value`. Values that look like JSON are parsed as JSON, so numbers
and lists work: `page_size=100`, `events=["invoice.paid"]`. `agree call` always prints
the API's reply as JSON.

**Put `--yes` before the `key=value` pairs.** Everything after the first pair is read as
another pair, so a trailing `--yes` is skipped with a warning and you still get asked.
The confirmation needs a terminal to ask in, so a script that makes changes has to pass
`--yes`.

**Chat**, for anything that takes more than one call:

```sh
agree agent                       # open a conversation
agree 'how many invoices…'        # answer, then stay open for follow-ups
```

Both need an AI provider, which you pick with `agree model`. Press **Enter on an empty
line** to leave, or type `exit`, `quit` or `bye`.

## Natural language

```sh
agree 'invoice Samir $5000 every week'
agree 'show me the paid invoices'
agree 'find the contact at croissant'
```

> **Use single quotes.** In `"double quotes"` your shell expands `$5000` to nothing
> before agree ever runs, and the amount silently disappears:
>
> ```sh
> agree "invoice Samir $5000/week"   # shell sends: invoice Samir /week
> agree 'invoice Samir $5000/week'   # correct
> ```
>
> You'll be prompted for the amount either way, so nothing breaks — but single
> quotes save you the extra question.

### How it works

The model plans; it never touches the API itself. Each turn it names one tool, the
program runs it, and the result comes back for the next decision — up to 10 steps.
You see each step as it happens:

```
› how many invoices have i sent to samir? they're supposed to be weekly

  → Find Samir's contact — the API can't search by person name
  → List invoices for Treehaus LLC

  You've sent 1 invoice to Samir Rayani, due 2026-07-08, still unpaid.

  Counting Wednesdays from 2026-07-08 to today: Jul 8, 15, 22, 29, Aug 5, 12 —
  6 should exist. Only 1 was created, stuck at sequence 0.
  Gap: 5 missing invoices.
```

It is told to **diagnose, not just report** — count the periods, compare against
what exists, lead with the gap, and end with what to do about it. A readout of
correct-looking data is not useful if the data is wrong.

### What it is not allowed to do

- **It never sees your API key** and never makes an HTTP call. It names a tool; the
  program makes the call.
- **Every change stops for confirmation**, showing the exact request first.
- **It cannot invent an amount, an email address or an id.** An amount it cannot
  parse counts as missing, not guessed — and missing becomes a question.
- **Creating an invoice always goes through the form**, never a raw request. That is
  where dollars become cents, a name becomes a real contact, and a weekly repeat
  gets its weekday. A model writing `5000` where cents are expected is a 100×
  billing error, so that path stays guarded.
- **Dates are never accepted silently.** They are shown pre-filled with the weekday
  spelled out, because models resolve "next friday" wrong often enough to matter.
  When the model gives no date, the form suggests one 30 days from today.

### Pick a provider and model

Run **`agree model`** to choose one. It saves the choice under `[ai]` in the config file.

| Provider | Models it offers | Key it reads |
|---|---|---|
| Anthropic | `claude-sonnet-5` (recommended), `claude-opus-5`, `claude-haiku-4-5-20251001` | `ANTHROPIC_API_KEY` |
| OpenAI | `gpt-5.2` (recommended), `gpt-5-mini`, `gpt-5.2-pro` | `OPENAI_API_KEY` |
| Google Gemini | `gemini-3.1-pro` (recommended), `gemini-3-flash` | `GOOGLE_API_KEY` |
| Ollama | whatever `ollama list` shows, or a name you type | none |

Put the provider's key in `~/.config/secrets.env` next to `AGREE_API_KEY`. If that
variable is already set, `agree model` leaves the config key blank so the environment
stays the source of truth. Otherwise it asks for a key, and you can press Enter to skip.
**A key saved in the config file wins over the environment variable**, which is the
opposite of how `AGREE_API_KEY` works.

**Ollama runs locally and needs no key.** agree talks to it at `http://localhost:11434`,
so start it with `ollama serve` first. A smaller local model just means more questions
to fill in.

Set `AGREE_DEBUG=1` to see the raw model replies.

## Notes on the API

Things worth knowing, all of which agree handles for you:

- **Amounts are integer cents.** `$150.00` is `15000`. agree parses `5k`, `$5,000`
  and `1.5k` into cents and never uses a float, so nothing rounds wrong. An amount
  with three decimal places is rejected rather than rounded.
- **Contacts can't be searched by name.** The API filters on email and company only,
  so `agree contacts samir` fetches contacts and matches locally. It only searches the
  first 50 unless you pass **`--limit 0`**, which searches them all. The invoice form
  always searches every contact.
- **Agreements are always created from a template** — there's no freeform create.
  `agree templates` lists what's available.
- **Customers need a feature flag.** Without it the API returns 403 and agree says so
  plainly rather than showing an empty list.
- **PDFs are generated asynchronously** — the endpoint returns a download URL or a
  "not ready, retry" response.

The full vendored spec is in [`openapi.json`](openapi.json).

## Things the API does that will surprise you

Found by hitting them, not by reading:

- **A recurring invoice backdated to the past sends once, then stalls.** Set a weekly
  series starting five weeks ago and you get one invoice at `recurring_sequence 0`
  and nothing after it. There is no backfill.
- **`scheduled_at` in the past is accepted silently**, then sends at some later
  moment you did not pick. agree clamps the send date to the due date, which the API
  requires (`Invoice date cannot be after due date`), and shows both in the review.
- **A weekly repeat is rejected without `repeat_on_weekday`**, and a monthly one
  without `repeat_on_type` + `repeat_on_day`. agree fills these from the due date.

## Status

Working and verified against the live API: config, the read commands, all 38
operations through `agree call`, the conversation agent, and creating invoices
through the form.

Not done yet:

- **Re-prompting on a validation error.** A rejection goes back to the model as a
  failed step, so it can try again, but that reruns the whole form. The errors are
  already structured per field, so it should re-ask just the bad one.
- **Creating agreements.** The API can create an agreement from a template, but agree
  has no operation for it yet.
- **Ending a recurring series.** The form always creates a series that runs forever,
  and the review says so. There is no way yet to give it an end date or a count.
- **A non-interactive mode.** `agree agent` needs a real terminal; in a pipe it
  exits immediately.
