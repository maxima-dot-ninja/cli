# merc

CLI for the [Mercury API](https://docs.mercury.com/reference) — accounts, transactions,
payments, cards, recipients, treasury, invoicing, webhooks.

**All 72 operations**, generated from Mercury's own OpenAPI spec. No AI, no guessing:
the same input always produces the same request.

## Install

```sh
cargo install --path .
```

Needs Rust 1.70+.

## API token

Create one at **mercury.com → Settings → API Tokens**. Keep the `secret-token:` prefix.

**Put it in `~/.config/secrets.env`:**

```sh
mkdir -p ~/.config && chmod 700 ~/.config
touch ~/.config/secrets.env && chmod 600 ~/.config/secrets.env
```

Add the line:

```sh
export MERCURY_API_KEY="secret-token:..."
```

Load it from `~/.zshrc` (once, covers every tool):

```sh
[ -f ~/.config/secrets.env ] && source ~/.config/secrets.env
```

Then check it:

```sh
merc config
```

```
  Config file : /Users/you/.config/merc/config.toml
  Environment : production
  Base URL    : https://api.mercury.com/api/v1
  API token   : set (76 chars)
  Operations  : 72
```

### The alternative: a config file

Run `merc` with no token set and it offers to take one, checks it against the API,
and writes **`~/.config/merc/config.toml`** `600`:

```toml
api_key = "secret-token:..."
sandbox = false
```

**`MERCURY_API_KEY` wins if both are set.** Don't `echo` a token into a file — it
stays in `~/.zsh_history` forever. Use `pbpaste >` or an editor.

### Sandbox

[Sandbox](https://docs.mercury.com/docs/using-mercury-sandbox) is a separate bank with
separate tokens. Add `--sandbox` to any command, set `sandbox = true` in the config, or
export `MERCURY_SANDBOX=1`. Every prompt tells you which one you are on.

## Usage

Four ways in, and none of them is the only way to reach anything.

**Groups.** A group on its own lists:

```sh
merc accounts                     # every account and its balance
merc transactions                 # recent transactions
merc cards
merc recipients
merc treasury
```

**Any operation**, as `<group> <command>`:

```sh
merc ops                                        # all 72, grouped
merc ops cards                                  # just one group
merc accounts get-statements --accountId=…
merc transactions list --status=pending --limit=50
merc cards freeze --cardId=…
merc cards list --status=active --status=frozen  # repeatable filters
merc statements get-pdf --statementId=… --out=july.pdf
merc transactions upload-attachment --transactionId=… --file=receipt.pdf
```

Flags are named exactly as Mercury names them, so anything in their docs can be typed
straight in. `--start-after` works as well as `--start_after`.

**By operation id**, the way the docs address it:

```sh
merc call getAccountCards accountId=…
merc call listTransactions status=sent limit=10
```

**The wizard**, when you'd rather not look anything up:

```sh
merc                              # pick a group, a command, then the arguments
merc send                         # send money, step by step
```

**You are never asked to paste an id.** Every id-shaped argument in the API — accounts,
cards, recipients, transactions, invoices, customers, statements, webhooks, users, SAFEs,
approval requests — is offered as a searchable list, labelled the way the tables are:

```
? statementId
> 2026-07-01  2026-07-31  $48,210.55
  2026-06-01  2026-06-30  $39,004.12
```

A list that needs an id of its own asks for that first, so `merc statements get-pdf` walks
you account → statement → file. A test fails the build if Mercury adds an id with no
listing behind it.

**This works outside the wizard too.** Leave a required argument off any command and, if
there is a terminal to ask, you get the same picker instead of an error:

```sh
merc statements get-pdf           # asks which account, then which statement
```

In a pipe it stays an error — a script blocked on a prompt nobody can see is worse than
a script that stops.

### Global flags

| Flag | Does |
|---|---|
| `--json` | Print Mercury's reply byte-for-byte, for `jq` |
| `--all` | Follow every page of a list |
| `--yes` | Skip the confirmation on a change |
| `--sandbox` | Use the sandbox |
| `--body '{…}'` | Send this as the whole request body — also `@file.json` or `-` for stdin |
| `--out PATH` | Where to write a PDF |

`--body` and flags combine: the body is the base, flags override single fields. That is
the escape hatch for anything the spec describes loosely, like an international wire's
routing block.

## Sending money

```sh
merc send                                        # guided
merc accounts create-transaction \
  --accountId=… --recipientId=… --amount=1234.56 --paymentMethod=ach
```

Before anything is sent you see the exact request, the environment in red if it is
production, and a yes/no. `--yes` skips it.

**The idempotency key is filled in for you** and shown in that confirmation. Pass
`--idempotencyKey=payroll-august` to choose your own — a meaningful one is what stops a
retry becoming a second payment.

## Notes on the API

Things worth knowing, all of which merc handles:

- **Amounts are decimal dollars**, not cents — `10.20` is ten dollars twenty. merc parses
  `$1,234.56`, `5k` and `1.5m` into integer cents, and never puts a float in the middle.
  Three decimal places is refused rather than rounded.
- **Repeating a send is safe.** Every payment carries an idempotency key; Mercury answers a
  repeat with the original transaction, and merc says so instead of reporting an error.
- **A duplicate payment is blocked for 24 hours** — same recipient, same account, same
  amount — even with a different idempotency key. That comes back as a 400.
- **Domestic wires need a purpose.** `--purpose '{"simple":{"category":"Vendor",
  "additionalInfo":"Acme"}}'`. Mercury's own note is in `merc accounts create-transaction --help`.
- **Tokens are scoped**, and Send Money additionally needs its IP allow-listed. That is what
  a 403 means; merc says so rather than showing an empty result.
- **Pagination is by cursor**, `start_after` and `page.nextPage`, except account transactions
  which use `offset`. `--all` follows whichever one applies.
- **PDFs come back as bytes**, not JSON, and are written to a file named after the
  `Content-Disposition` header unless you pass `--out`.

## How the commands are generated

Mercury publishes no single spec file. Each page under `docs.mercury.com/reference` embeds
the OpenAPI fragment for its own operation, and `llms.txt` indexes the pages, so:

```sh
python3 tools/fetch-spec.py          # merge all 77 pages into openapi.json
python3 tools/fetch-spec.py --check  # is the vendored copy stale?
cargo build                          # spec -> command table
```

`build.rs` turns [`openapi.json`](openapi.json) into a static Rust table: every path,
parameter, type, enum, required flag and description. It also decides the *shape* — a
group and a verb per operation, which responses are lists, how each one paginates — and
**fails the build** if two operations would answer to the same command.

So when Mercury ships an endpoint, refetch and rebuild; it appears as a command with
working help and validation, without a line being written. The only hand-written parts
are seven naming overrides in `build.rs` and the display layouts in `src/view.rs`.

## Status

**Verified against the live API:** `merc accounts` against a real production token —
balances, statuses and ids render correctly. The command tree (all 72 operations build,
with unique names), request building against both hosts, and error handling.

**A rejected token says why.** Which of the two places the token came from, whether it is
missing the `secret-token:` prefix, whether it is a sandbox token aimed at production —
and, when it came from the environment, that a shell still holds the old value after the
file was fixed. That last one is the cause you will actually hit.

**Verified by tests, not against live data:** amount parsing and formatting, argument
typing, required-field checks, idempotency-key filling, `--body` merging, pagination
choice, the display layouts, that every id in the API has a listing behind it, and that
no prompt runs past one line. 49 tests, `cargo test`.

**Not yet exercised with a real token:** every response body except accounts. The layouts
follow the schemas in the spec, so a field Mercury renamed without updating its docs would
print as `—` until the column is corrected.

**Not done:**

- **Multipart uploads and PDF downloads are untested** end to end — they need a real
  transaction to attach to.
- **OAuth2** is included for completeness (`merc oauth2 …`) but is for building an
  integration; a personal API token needs none of it. It is the one place the spec marks
  every field required when Mercury means "one of these two sets", so pass `--body`.
- **No default account.** Every command that needs one still wants `--accountId`; the
  wizard offers a list, the flag does not.
