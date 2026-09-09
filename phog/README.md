# phog

CLI for [PostHog](https://posthog.com) — HogQL from the terminal, people and events by
fingerprint, and dashboards as files that apply the same way twice.

No AI anywhere. The same input always makes the same request.

## Install

```sh
cargo install --path .
```

Needs Rust 1.70+.

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

Load it from `~/.zshrc` (once, covers every tool):

```sh
[ -f ~/.config/secrets.env ] && source ~/.config/secrets.env
```

Then check:

```sh
phog config
```

### The alternative: a config file

Run `phog setup`, or just `phog` with nothing configured. It takes the key and the
project, proves them against the API, and writes **`~/.config/phog/config.toml`** `600`.
The environment wins over the file when both are set.

## Usage

Run `phog` alone for a menu. Every path in the menu is also a command.

```sh
phog query "select event, count() from events where timestamp > now() - interval 1 day group by event order by count() desc"
phog query --json < some.sql

phog events --fingerprint 3f9a…            # one person, across cookies and the agent
phog events --email someone@company.com --since -7d
phog persons "someone@"                      # search, plus a link to each person page

phog dashboards                              # list
phog dashboards show "Ask Croissant"
phog dashboards diff dashboards/ask-croissant.yaml
phog dashboards apply dashboards/ask-croissant.yaml
phog dashboards export 42 --out dashboards/growth.yaml

phog call GET insights/?limit=5              # anything the API has
phog call PATCH dashboards/42/ '{"pinned": true}'
```

`--json` on any command prints what the API returned instead of a table.

## Dashboards as files

A dashboard is one YAML file. `phog dashboards apply` makes PostHog match it: the dashboard is
created or updated, each insight is created or updated, insights that left the file are
removed, and tiles are placed. Run it again and nothing happens. That is the whole point —
the file is the truth and the UI is a view of it.

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
| `funnel` | two or more `series` steps, optional `window` | a step funnel |
| `hogql` | `query` | a table from SQL |
| `raw` | `raw`, a complete PostHog query node | anything the shorthands cannot say |

A series is `event`, plus `name`, `math` (`total`, `dau`, `unique_session`, `sum`, `avg`, `p90`…),
`property` for the math to run over, and `filters`. A filter is `key`, `value`, `operator`
and `type` (`event`, `person`, or `hogql` where `key` is the whole expression). `layout` is
`x`, `y`, `w`, `h` on a 12-column grid.

### How it finds things again

The dashboard is tagged `phog:<slug>` and every insight `phog:<slug>:<key>`. Those tags are the
identity: rename anything freely, the next apply still finds it. A dashboard with the same
`name` but no tag is adopted rather than duplicated, so a board somebody built by hand can be
taken under file control by exporting it, saving the file, and applying.

`export` turns a dashboard back into a file. Managed insights keep their keys; hand-made ones
get a key from their name. Anything the shorthands cannot express comes back as `raw`, which
round-trips exactly.

### What it never does

It does not delete dashboards on apply, only insights that carry its own tag and left the
file — and `--no-prune` keeps even those. It never touches insights it did not make. Deleting
a dashboard is its own command and asks first.

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
