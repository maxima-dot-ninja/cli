---
name: phog
description: >
  The owner's PostHog project — run HogQL, look people up by fingerprint or email, and
  build dashboards from files that apply idempotently.
binary: phog
version: 0.1.0
tools:
  - name: posthog_query
    description: >
      Run a HogQL query against the owner's PostHog project and get the rows back as JSON.
      Use it for any question about what visitors did: counts by event, one person's path,
      what the Croissant agent showed, model spend. The `events` table has `event`,
      `timestamp`, `distinct_id`, `properties.*` and `person.properties.*`. Website events carry
      `properties.fingerprint` (the device), `properties.plan_chat_id` (an Ask Croissant chat) and
      `properties.source` (web, api or agent). Keep a LIMIT on anything that lists rows.
    argv: [query, "{{hogql}}", --json]
    timeout_ms: 120000
    input:
      type: object
      required: [hogql]
      properties:
        hogql:
          type: string
          description: the HogQL, one statement

  - name: posthog_events
    description: >
      Everything PostHog holds on one person, newest first, by whichever handle you have: a
      device fingerprint (best — it reaches across cookies and the agent's server-side events),
      an email, or a distinct id. Answers with the rows and the person's PostHog URL.
    argv: [events, "{{fingerprint?:--fingerprint}}", "{{fingerprint?}}", "{{email?:--email}}", "{{email?}}", "{{distinct_id?:--distinct-id}}", "{{distinct_id?}}", --since, "{{since}}", --json]
    timeout_ms: 120000
    input:
      type: object
      properties:
        fingerprint:
          type: string
        email:
          type: string
        distinct_id:
          type: string
        since:
          type: string
          description: how far back, PostHog style — -7d, -24h, -90d
          default: "-90d"

  - name: posthog_dashboards
    description: >
      List every dashboard in the project, with its id, tile count and tags. Dashboards tagged
      `phog:<slug>` are managed from files and should be changed by editing the file and applying
      it, never by hand.
    argv: [dashboards, list, --json]
    timeout_ms: 60000
    input:
      type: object
      properties: {}

  - name: posthog_dashboard_apply
    description: >
      Make PostHog match a dashboard file. The file is YAML with `name`, `slug` and `insights`,
      each insight a `key`, `name`, `type` (trends | number | funnel | hogql | raw) and its
      series or query — `phog/dashboards/ask-croissant.yaml` in ~/dev/_cli is the worked example.
      Applying is idempotent: the same file twice changes nothing. Pass `dry_run` to see what would
      change first, and do that before the first real apply of a new file.
    argv: [dashboards, apply, "{{file}}", "{{dry_run?:--dry-run}}"]
    timeout_ms: 300000
    input:
      type: object
      required: [file]
      properties:
        file:
          type: string
          description: path to the dashboard YAML
        dry_run:
          type: boolean

  - name: posthog_journeys
    description: >
      Write a JSON snapshot of how many times every event fired per month, for the last N
      months, plus `$pageview` counts for any route patterns given (`:param` matches one
      segment). The website's /admin/journeys page reads this file; `npm run journeys:counts`
      in the website repo is the usual way to call it.
    argv: [journeys, --months, "{{months}}", "{{out?:--out}}", "{{out?}}"]
    timeout_ms: 120000
    input:
      type: object
      properties:
        months:
          type: integer
          default: 12
        out:
          type: string
          description: file to write; prints to stdout when omitted

  - name: posthog_dashboard_export
    description: >
      Write a dashboard PostHog already has to a YAML file that posthog_dashboard_apply can read
      back. Use it to bring a hand-built dashboard under file control, or to read one's shape.
    argv: [dashboards, export, "{{dashboard}}", --out, "{{out}}"]
    timeout_ms: 60000
    input:
      type: object
      required: [dashboard, out]
      properties:
        dashboard:
          type: string
          description: dashboard id or name
        out:
          type: string
          description: file to write
---

# phog

PostHog from the terminal. Reads are free; the only writes are dashboards, and those are
declared in files and applied like a migration.

## Asking about people

The website stamps three things on every event, and the agent and the API stamp the same
three on theirs, so a person can be followed across the site, the chat and the backend:

- `properties.fingerprint` — the device. Survives cleared cookies. This is the handle to prefer.
- `properties.plan_chat_id` — the Ask Croissant conversation an event belongs to.
- `properties.source` — `web`, `api` or `agent`, so you can tell who reported it.

Conversions are plain events: `lead_captured`, `user_signed_up`, `office_term_signed`,
`office_term_executed`. The agent's work is `agent_*` events plus one `$ai_generation` per turn
with `$ai_model`, `$ai_input_tokens`, `$ai_output_tokens` and `$ai_latency`.

## Dashboards are files

Never edit a managed dashboard in the PostHog UI; the next apply would put it back. Edit the
YAML, run a dry run, then apply. A new dashboard is a new file with a new `slug`; the slug is
what the tool finds it by, so pick it once.

## Finishing work

When you have applied a dashboard, say which insights were created, updated or removed and give
the dashboard URL the tool printed. When a query answers a question, give the answer in a
sentence and the count, not the table.
