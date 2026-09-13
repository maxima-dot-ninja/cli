# phog

CLI for [PostHog](https://posthog.com) — HogQL from the terminal, people and events by
fingerprint, and dashboards as files that apply the same way twice.

No AI anywhere. The same input always makes the same request.

## Install

```sh
cargo install --path .
```

Run it from inside the `phog` folder. It needs **Rust 1.88** or newer, because the locked
dependencies require it.

## Keys

Two things from PostHog: a **personal API key** (Settings → Personal API keys, starts
with `phx_`; the project key that begins `phc_` can only send events and is no use
here) and the **project id** (the number in the project's URL).

**Put them in `~/.config/secrets.env`:**

```sh
export POSTHOG_PERSONAL_API_KEY="phx_..."
export POSTHOG_PROJECT_ID="12345"
export POSTHOG_APP_HOST="https://us.posthog.com"   # or https://eu.posthog.com
```

**`POSTHOG_APP_HOST`** is optional. It defaults to `https://us.posthog.com`, so you only set it
for the EU cloud or a self-hosted instance.

Load it from `~/.zshrc` (once, covers every tool):

```sh
[ -f ~/.config/secrets.env ] && source ~/.config/secrets.env
```

Then check:

```sh
phog config
```

### The alternative: a config file

Run `phog setup`, or just `phog` with nothing configured. It asks for the app host, the key
and the project id, proves them against the API, and writes **`~/.config/phog/config.toml`**
with mode `600`. When **`XDG_CONFIG_HOME`** is set, the file goes to
`$XDG_CONFIG_HOME/phog/config.toml` instead. The environment wins over the file when both are
set, one value at a time.

## Usage

Run `phog` alone for a menu. Every path in the menu is also a command. The menu's file picker
lists the YAML files in **`dashboards/`** under the current directory, so run it from the folder
that holds them.

```sh
phog query "select event, count() from events where timestamp > now() - interval 1 day group by event order by count() desc"
phog query --json < some.sql

phog events --fingerprint 3f9a…            # one person, across cookies and the agent
phog events --email someone@company.com --since -7d
phog events --distinct-id abc123 --limit 50
phog persons "someone@"                      # search, plus a link to each person page

phog dashboards                              # lists them, same as `dashboards list`
phog dashboards show "Ask Croissant"
phog dashboards check dashboards/ask-croissant.yaml   # parses the file and needs no key
phog dashboards diff dashboards/ask-croissant.yaml
phog dashboards apply dashboards/ask-croissant.yaml
phog dashboards export 42 --out dashboards/growth.yaml
phog dashboards delete 42                    # asks first, and --yes skips the question

phog journeys --months 12 --path /pricing --path '/careers/:id' --out counts.json

phog call GET 'insights/?limit=5'            # anything the API has
phog call PATCH dashboards/42/ '{"pinned": true}'
```

**`--json`** prints JSON instead of a table for `query`, `events`, `persons`, `dashboards list`,
`dashboards show` and `dashboards check`. For `query` and `events`, each row comes out as an
object keyed by column name. `call` always prints JSON, and `export` always writes YAML.

`events` needs one of **`--fingerprint`**, **`--email`** or **`--distinct-id`**. When you pass more
than one, the fingerprint wins over the email and the email wins over the distinct id. A
fingerprint matches events that carry it as a property and events from the distinct id
`fp:<fingerprint>`. **`--since`** reads like PostHog's own ranges (`-24h`, `-7d`, `-2w`, `-3m` for
months, `-1y`) and defaults to `-90d`. **`--limit`** defaults to 200 rows. Under the table, it
prints a person link for up to five of the distinct ids it found.

`persons` runs two lookups and merges them, and **`--limit`** (default 20) caps each one.

`show`, `export` and `delete` find a dashboard by its id, its name or its slug, and otherwise by
the first name that contains what you typed. Name matching ignores case. `export` prints the
YAML to stdout when you leave out **`--out`**.

`call` takes a path under `/api/projects/<id>/`, or a full path that starts with `/api/`. It asks
before it sends anything other than a GET, and **`--yes`** skips that question.

## Journey counts

`phog journeys` writes one JSON document with a count per event per month, for a page that
wants to show numbers without holding a key. It runs two HogQL queries: every named event
(anything not starting with `$`) grouped by month, and `$pageview` grouped by month for each
`--path` you pass. A path is a route pattern: `:param` matches one segment, a trailing slash is
tolerated, and the rest is literal. **`--months`** (default 12) counts back from the current
month, so the last month is a month to date. Without **`--out`** it prints to stdout; with it the
file is written whole, through a sibling and a rename, so a reader never sees half of it.

```json
{
  "generated_at": "2026-09-11T18:34:13Z",
  "months": ["2025-10", "…", "2026-09"],
  "events": {"astronaut_opened": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 62]},
  "pageviews": {"/careers/:id": [0, 0, 0, 0, 0, 0, 0, 610, 66, 59, 68, 19]}
}
```

Every array lines up with `months`. The website's `npm run journeys:counts` wraps this with the
patterns taken from its own journey map.

## Dashboards as files

A dashboard is one YAML file. `phog dashboards apply` makes PostHog match it: the dashboard is
created or updated, each insight is created or updated, insights that left the file are
removed, and tiles are placed. Run it again and nothing happens. That is the whole point —
the file is the truth and the UI is a view of it.

`diff` (or `apply --dry-run`) prints the changes apply would make and stops. `check` reads the
file without contacting PostHog and needs no key, and with `--json` it prints every query node
apply would send.

```yaml
name: Ask Croissant
slug: ask-croissant           # how phog finds it again; pick once, never change
description: What the chat does for visitors.
tags: [croissant]
date_from: -30d               # default for every insight below

insights:
  - key: chats-opened         # stable per insight, same rule as slug
    name: Chats opened
    type: number
    series:
      - event: astronaut_opened
        math: unique_session
    layout: {x: 0, y: 0, w: 3, h: 3}

  - key: funnel
    name: From opening the chat to a signed term
    type: funnel
    window: 30d
    series:
      - {event: astronaut_opened, name: Opened the chat}
      - {event: agent_results_shown, name: Saw spaces}
      - {event: lead_captured, name: Left an email}
      - {event: office_term_signed, name: Signed a term}

  - key: skills
    name: What the agent reaches for
    type: trends
    display: ActionsBarValue
    breakdown: skill
    series:
      - event: agent_skill_called

  - key: recent
    name: Recent conversations
    type: hogql
    query: |
      SELECT properties.plan_chat_id AS chat, min(timestamp) AS started, count() AS events
      FROM events WHERE properties.plan_chat_id IS NOT NULL
      GROUP BY chat ORDER BY started DESC LIMIT 100
```

Five insight types:

| `type` | What it needs | What it makes |
|---|---|---|
| `trends` | `series`, optional `interval`, `display`, `breakdown` | a line, bar, pie or table over time |
| `number` | one `series` | a single big number |
| `funnel` | two or more `series` steps, optional `window` and `breakdown` | a step funnel |
| `hogql` | `query` | a table from SQL |
| `raw` | `raw`, a complete PostHog query node | anything the shorthands cannot say |

A series is `event`, plus `name`, `math` (`total`, `dau`, `unique_session`, `sum`, `avg`, `p90`…),
`property` for the math to run over, and `filters`. A filter is `key`, `value`, `operator`
and `type` (`event`, `person`, or `hogql` where `key` is the whole expression). `layout` is
`x`, `y`, `w`, `h` on a 12-column grid.

Every insight can also set a `description`, its own `date_from`, and `filters` that apply to the
whole insight. An insight without a `date_from` uses the dashboard's, and `-30d` when neither is
set. A filter's `operator` defaults to `exact` and its `type` defaults to `event`. A funnel
`window` is a number and a unit (`30m`, `2h`, `14d`, `1w` or `1M`) and defaults to `14d`. A
trends `interval` defaults to `day`.

### How it finds things again

The dashboard is tagged `phog` and `phog:<slug>`, and every insight is tagged `phog`,
`phog:<slug>:<key>` and the dashboard's own `tags`. Those tags are the identity: rename
anything freely, the next apply still finds it. A dashboard with the same `name` but no tag
is adopted rather than duplicated. Its hand-made insights are not adopted, because they carry
no tag. If you export a hand-built board and apply the file, apply creates tagged copies of
those insights beside the originals and leaves the originals alone, so you delete the
originals by hand once the copies look right.

`export` turns a dashboard back into a file. Managed insights keep their keys; hand-made ones
get a key from their name. Anything the shorthands cannot express comes back as `raw`, which
round-trips exactly.

### What it never does

It does not delete dashboards on apply, only insights that carry its own tag and left the
file — and `--no-prune` keeps even those. It never touches insights it did not make. Deleting
a dashboard is its own command. It asks first unless you pass `--yes`, and it leaves the
dashboard's insights in place. Both kinds of removal work by setting PostHog's `deleted` flag.

## Notes on the API

- The private API lives on the app host (`us.posthog.com`), not the ingest host
  (`us.i.posthog.com`). Pointing the key at the wrong one is a 404 on everything.
- HogQL goes through `POST /api/projects/:id/query/` with `{"kind": "HogQLQuery"}`. It sees
  every event whichever person it landed on, which is what makes lookups by a property such as
  `fingerprint` possible when the persons endpoint cannot.
- Person search does not look at person properties. `phog persons` runs the search and a
  property filter on `fingerprint` and merges the two.
- A stored insight comes back with defaults the file never set. Apply compares only the keys
  the file set, so those defaults do not read as drift.

## Status

Written against the API as of September 2026. The dashboard `layouts` shape and the funnel
filter names are the parts most likely to move if PostHog changes its insight schema; `raw`
is the escape hatch when they do.
