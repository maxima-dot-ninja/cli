# merc

CLI for the [Mercury API](https://docs.mercury.com/reference) — accounts, transactions,
payments, cards, recipients, treasury, invoicing, webhooks.

**All 72 operations**, generated from Mercury's own OpenAPI spec. No AI, no guessing:
the same input always produces the same request.

## Install

```sh
cargo install --path .
```

It needs **Rust 1.88** or newer.

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

Then check it. `merc config` makes **one live read**, so it tells you whether Mercury
actually answers, not just whether a token is set:

```sh
merc config
```

```
  Config file : /Users/you/.config/merc/config.toml
  Environment : production
  Base URL    : https://api.mercury.com/api/v1
  API token   : set (76 chars)
  Read token  : not set — reads use the API token
  Operations  : 72

  ✓ Reaches Mercury — 3 accounts visible.
```

When the read fails, it prints the reason in place of that last line.

### Reads can use a second token

Mercury forces an **IP allow-list** onto any token that can write, so the main token stops
working the moment you change network. A **`Read Only`** token has no allow-list and works
from anywhere. Create one next to the main token and add it to `~/.config/secrets.env`:

```sh
export MERCURY_READ_KEY="secret-token:..."
```

merc then sends **every read** on that token and keeps `MERCURY_API_KEY` for anything that
changes data. `merc config` shows which token reads use.

### The alternative: a config file

Run `merc` with no token set and it offers to take one, asks whether it is for production
or the sandbox, checks it against the API, and writes **`~/.config/merc/config.toml`** `600`.
If `XDG_CONFIG_HOME` is set, the file goes in `$XDG_CONFIG_HOME/merc/` instead.

```toml
api_key = "secret-token:..."
read_key = ""
sandbox = false
client_id = ""
client_secret = ""
```

`read_key` holds the optional read token, and `client_id` and `client_secret` are only
used by OAuth2.

**The environment wins if both are set.** That holds for `MERCURY_API_KEY`,
`MERCURY_READ_KEY`, `MERCURY_CLIENT_ID` and `MERCURY_CLIENT_SECRET` alike. Don't `echo` a
token into a file — it stays in `~/.zsh_history` forever. Use `pbpaste >` or an editor.

### Sandbox

[Sandbox](https://docs.mercury.com/docs/using-mercury-sandbox) is a separate bank with
separate tokens. Add `--sandbox` to any command, set `sandbox = true` in the config, or
export `MERCURY_SANDBOX=1` (or `true`). Every prompt tells you which one you are on.

## Usage

Four ways in, and none of them is the only way to reach anything.

**Groups.** A group on its own runs its `list` command, and it takes that command's flags
directly:

```sh
merc accounts                     # every account and its balance
merc transactions                 # recent transactions
merc transactions -n 5 --status=pending
merc cards
merc recipients
merc treasury
```

The five groups with no `list` command are `oauth2`, `organization`, `send-money`,
`statements` and `vault`, and on their own they print their commands instead.

**Any operation**, as `<group> <command>`:

```sh
merc ops                                        # all 72, grouped; • marks a change
merc ops cards                                  # just one group
merc accounts get-statements --accountId=…
merc transactions list --status=pending --limit=50
merc cards freeze --cardId=…
merc cards list --status=active --status=frozen  # repeatable filters
merc cards list --status=active,frozen          # or comma-separated
merc statements get-pdf --statementId=… --out=july.pdf
merc transactions upload-attachment --transactionId=… --file=receipt.pdf
```

Flags are named exactly as Mercury names them, so anything in their docs can be typed
straight in. `--start-after` works as well as `--start_after`. `--limit`, `--search` and
`--file` also answer to `-n`, `-q` and `-f`, and a boolean flag given with no value
means `true`.

**By operation id**, the way the docs address it:

```sh
merc call getAccountCards accountId=…
merc call listTransactions status=sent limit=10
```

The id also works in kebab case, so `merc call get-account-cards` finds the same operation.

**The wizard**, when you'd rather not look anything up:

```sh
merc                              # pick a group, a command, then the arguments
merc send                         # send money, step by step
```

**You are never asked to paste an id.** Every id-shaped argument in the API — accounts,
cards, recipients, recipient invites, transactions, invoices, invoice attachments, customers,
categories, statements, treasury accounts, events, webhooks, users, SAFEs, approval
requests — is offered as a searchable list, labelled the way the tables are:

```
? statementId
> 2026-07-01  2026-07-31  $48,210.55
  2026-06-01  2026-06-30  $39,004.12
```

A list that needs an id of its own asks for that first, so `merc statements get-pdf` walks
you account → statement → file. A test fails if Mercury adds an id with no
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
| `--json` | Print Mercury's reply byte-for-byte, for `jq`; with `--all`, the pages merged into one list |
| `--all` | Follow every page of a list |
| `-y`, `--yes` | Skip the confirmation on a change |
| `--sandbox` | Use the sandbox |
| `--body '{…}'` | Send this as the whole request body — also `@file.json` or `-` for stdin |
| `--out PATH` | Where to write a PDF |

`--body` and flags combine: the body is the base, flags override single fields. That is
the escape hatch for anything the spec describes loosely, like an international wire's
routing block.

`--body` only reaches **JSON** requests, and form or multipart requests ignore it. In a
terminal, merc still asks for any required field you did not pass as a flag, even when the
body already holds it, and your answer overrides the body. In a pipe, the body counts
toward the required fields.

## Sending money

```sh
merc send                                        # guided
merc accounts create-transaction \
  --accountId=… --recipientId=… --amount=1234.56 --paymentMethod=ach
```

The guided flow picks the account and the recipient from lists, then asks for the amount,
the payment method and an optional memo. It does not ask for a wire purpose, so send a
**domestic wire** with `merc accounts create-transaction --purpose=…` instead.

Before anything is sent you see the exact request, the environment in red if it is
production, and a yes/no that defaults to no. Every change gets this confirmation, not only
a payment. `--yes` skips it.

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
- **Tokens are scoped.** A 403 means the token lacks the scope, and merc says so rather than
  showing an empty result.
- **Any token that can write is IP allow-listed.** A blocked IP comes back as a **401**,
  which looks exactly like a dead token. merc reads Mercury's `ipNotWhitelisted` code
  instead, says the token is fine, prints this machine's IP and tells you where to add it.
  When the blocked request was a read, it also points you at `MERCURY_READ_KEY`.
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
working help and validation, without a line being written. The hand-written parts are
seven naming overrides in `build.rs`, the display layouts in `src/view.rs`, and the table
in `src/wizard.rs` that says which listing answers each kind of id. A new kind of id needs
one row in that table, and `cargo test` names the missing one.

## Status

**Verified against the live API:** `merc accounts` against a real production token —
balances, statuses and ids render correctly. The command tree (all 72 operations build,
with unique names), request building against both hosts, and error handling.

**A rejected token says why.** Which of the two places the token came from, whether it is
missing the `secret-token:` prefix, whether it is a sandbox token aimed at production —
and, when it came from the environment, that a shell still holds the old value after the
file was fixed. That last one is the cause you will actually hit. A request blocked by the
IP allow-list is reported as exactly that, never as a bad token, even though Mercury sends
it as a 401. `merc config` runs this diagnosis against the live API.

**Verified by tests, not against live data:** amount parsing and formatting, argument
typing, required-field checks, idempotency-key filling, `--body` merging, pagination
choice, error classification including a blocked IP, the display layouts, that every id in
the API has a listing behind it, and that no prompt runs past one line. There are 50 tests,
run with `cargo test`.

**Not yet exercised with a real token:** every response body except accounts. The layouts
follow the schemas in the spec, so a field Mercury renamed without updating its docs would
print as `—` until the column is corrected.

**Not done:**

- **Multipart uploads and PDF downloads are untested** end to end — they need a real
  transaction to attach to.
- **OAuth2** is included for completeness (`merc oauth2 …`) but is for building an
  integration; a personal API token needs none of it. It talks to `oauth2.mercury.com`
  (or `oauth2-sandbox.mercury.com`) and authenticates with `MERCURY_CLIENT_ID` and
  `MERCURY_CLIENT_SECRET`. The spec marks all six token fields required when Mercury means
  "one of these two sets", and `--body` cannot fill them, because this request is a form.
  To send one set, pass its fields as flags plus `--body '{}'` to lift the check, and pipe
  the output so merc does not stop to ask for the rest:
  `merc oauth2 obtain-access-token --grant_type=refresh_token --refresh_token=… --body '{}' | cat`.
- **No default account.** Every command that needs one asks for it each time, either
  through `--accountId` or through the picker when you leave the flag off in a terminal.
