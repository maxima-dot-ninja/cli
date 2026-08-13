# agree

CLI for the [Agree API](https://secure.agree.com/documentation) — invoices, agreements,
contacts, customers and reports.

Written in Rust. Works fully without AI; natural-language commands are optional.

## Install

```sh
cargo install --path .
```

Needs Rust 1.70+.

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
  API key     : set (57 chars)
```

### The alternative: a config file

If you'd rather not use the environment, put the key in
**`~/.config/agree/config.toml`**:

```toml
api_key = "agr_..."
currency = "USD"
```

agree writes that file `600` automatically. **`AGREE_API_KEY` wins if both are set.**

> Don't `echo` a key into a file — it stays in `~/.zsh_history` forever. Use
> `pbpaste >` or an editor.

## Usage

```sh
agree config                      # where config lives, whether a key is set
agree invoices                    # recent invoices
agree invoices --status due       # filter: created, due, sent, paid, failed, ...
agree invoices --customer acme    # fuzzy company match
agree contacts                    # all contacts
agree contacts samir              # match name, email or company
agree customers                   # requires the customers feature on your org
agree agreements
agree templates
```

Add `--json` to any command for raw JSON.

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

The model only ever produces a JSON description of what you asked for. It never
calls the API, never sees a key, and never invents an email address. Everything it
returns is re-validated: an amount it cannot parse counts as missing rather than
guessed, and anything missing becomes a prompt. Nothing is sent until the full
invoice is shown back to you and confirmed.

Dates are never accepted silently — they're shown pre-filled with the weekday
spelled out, because models resolve "next friday" wrong often enough to matter.

Pick your provider and model with `agree model`. Ollama runs locally and needs no
key; smaller local models simply mean more prompts to fill in.

## Notes on the API

Things worth knowing, all of which agree handles for you:

- **Amounts are integer cents.** `$150.00` is `15000`. agree parses `5k`, `$5,000`
  and `1.5k` into cents and never uses a float, so nothing rounds wrong. An amount
  with three decimal places is rejected rather than rounded.
- **Contacts can't be searched by name.** The API filters on email and company only,
  so `agree contacts samir` pages through and matches locally.
- **Agreements are always created from a template** — there's no freeform create.
  `agree templates` lists what's available.
- **Customers need a feature flag.** Without it the API returns 403 and agree says so
  plainly rather than showing an empty list.
- **PDFs are generated asynchronously** — the endpoint returns a download URL or a
  "not ready, retry" response.

The full vendored spec is in [`openapi.json`](openapi.json).

## Status

Working today: config, and read commands for invoices, contacts, customers,
agreements and templates — all verified against the live API.

Planned: writes (create/send invoices, agreements) behind a confirmation step, a
schema-driven form for filling gaps, and an optional natural-language layer that
turns "recurring invoice for Samir, $5000/week" into a reviewed, typed request.
