---
name: hku
description: >
  The owner's Heroku apps — dynos and their state, formation, config vars (masked), releases,
  add-ons, domains and recent logs, plus any Platform API endpoint through a raw call.
binary: hku
version: 0.1.0
tools:
  - name: heroku_apps
    description: >
      List the owner's Heroku apps with each one's generation (cedar or fir), region, stack, team,
      owner and when it last released. Lists the saved team's apps, or every app the token reaches
      when no team is saved. Call this first whenever you are not certain of an app's exact name.
    argv: [apps, [--team, "{{team?}}"]]
    timeout_ms: 60000
    input:
      type: object
      properties:
        team:
          type: string
          description: a team name, to list that team instead of the saved one

  - name: heroku_app_show
    description: >
      One app's details: id, generation, region, stack, team, owner, web URL, git URL, whether
      maintenance mode is on, ACM, and when it last released.
    argv: [apps, show, "{{app}}"]
    timeout_ms: 60000
    input:
      type: object
      required: [app]
      properties:
        app:
          type: string
          description: the app's name or id, as heroku_apps lists it

  - name: heroku_dynos
    description: >
      The dynos running on one app now, with each one's type, size, state, release and command.
      State is up, starting, idle, crashed or down, and any crashed dyno is named on the last line.
      Reach for this first when the owner says an app is down or erroring.
    argv: [dynos, "{{app}}"]
    timeout_ms: 60000
    input:
      type: object
      required: [app]
      properties:
        app:
          type: string

  - name: heroku_formation
    description: >
      One app's formation: each process type (web, worker, …), how many dynos it is set to run, their
      size and the command. This is what the app is meant to run; heroku_dynos is what runs now.
    argv: [formation, "{{app}}"]
    timeout_ms: 60000
    input:
      type: object
      required: [app]
      properties:
        app:
          type: string

  - name: heroku_config
    description: >
      One app's config vars. The names come back with each value masked down to its length, because
      the values are live secrets. Set reveal only when the owner has asked to see the values
      themselves in this conversation, and never repeat a revealed value back unless asked to.
    argv: [config, "{{app}}", "{{reveal?:--reveal}}"]
    timeout_ms: 60000
    input:
      type: object
      required: [app]
      properties:
        app:
          type: string
        reveal:
          type: boolean
          description: print the real values; only when the owner has asked for them

  - name: heroku_releases
    description: >
      One app's releases, newest first: version, status, whether it is current, who made it, when,
      and the description (a deploy, a config change, an add-on, a rollback). Reach for this to find
      what changed just before something broke.
    argv: [releases, "{{app}}", [--limit, "{{limit?}}"]]
    timeout_ms: 60000
    input:
      type: object
      required: [app]
      properties:
        app:
          type: string
        limit:
          type: integer
          description: how many releases, newest first
          default: 20

  - name: heroku_addons
    description: >
      One app's add-ons: name, service (heroku-postgresql, heroku-redis, …), plan, state, and the
      config var names each one sets, such as DATABASE_URL.
    argv: [addons, "{{app}}"]
    timeout_ms: 60000
    input:
      type: object
      required: [app]
      properties:
        app:
          type: string

  - name: heroku_domains
    description: >
      One app's domains: hostname, kind (heroku or custom), the DNS target a custom domain's CNAME
      must point at, its status and its ACM certificate status.
    argv: [domains, "{{app}}"]
    timeout_ms: 60000
    input:
      type: object
      required: [app]
      properties:
        app:
          type: string

  - name: heroku_logs
    description: >
      Recent log lines from one app, oldest first. Cedar apps keep the newest 1,500 lines; Fir apps
      keep none, and the tool says so instead of returning nothing. Narrow it with dyno (web.1),
      type (web) or source (app for the app's own output, heroku for the router and platform).
    argv:
      - logs
      - "{{app}}"
      - [--lines, "{{lines?}}"]
      - [--dyno, "{{dyno?}}"]
      - [--type, "{{type?}}"]
      - [--source, "{{source?}}"]
    timeout_ms: 90000
    input:
      type: object
      required: [app]
      properties:
        app:
          type: string
        lines:
          type: integer
          description: how many past lines, 1 to 1500
          default: 100
        dyno:
          type: string
          description: one dyno, such as web.1
        type:
          type: string
          description: one process type, such as web or worker
        source:
          type: string
          enum: [app, heroku]

  - name: heroku_call
    description: >
      Call any Heroku Platform API endpoint directly and get the JSON back. Reach for this when the
      named tools don't cover it: builds, pipelines, review apps, collaborators, log drains, SNI
      endpoints, features, rate limits. The path goes under https://api.heroku.com/, such as
      apps/my-app/builds or account/rate-limits. A GET follows every page, and config var values come
      back masked, so use heroku_config for those. ANYTHING ELSE CHANGES A LIVE
      APP and needs confirm true, which you may only set when the owner has just told you to do this
      exact thing — never on your own initiative, never to retry past a refusal.
    argv: [call, "{{method}}", "{{path}}", "{{body?}}", "{{confirm?:--yes}}"]
    timeout_ms: 60000
    error_hint: >
      If this failed asking for confirmation, that is because the call changes a live app and there
      is nobody at this keyboard to answer. Re-run it with confirm true — but only after the owner
      has said yes to this exact call in this conversation.
    input:
      type: object
      required: [method, path]
      properties:
        method:
          type: string
          enum: [GET, POST, PATCH, PUT, DELETE]
        path:
          type: string
          description: the path under https://api.heroku.com/, e.g. apps/my-app/builds
        body:
          type: object
          additionalProperties: true
          description: the JSON body, for POST, PATCH and PUT
        confirm:
          type: boolean
          description: >
            required for anything but GET; only ever true when the owner has just said yes
---

# hku

Heroku from the terminal. Every named tool only reads. The one way to change anything is
`heroku_call` with a method other than GET, and that waits for the owner's yes.

## Finding the problem

When the owner says an app is down or broken, read in this order:

1. `heroku_dynos` shows whether anything is crashed or missing right now.
2. `heroku_releases` shows what changed just before, such as a deploy, a config change or an add-on.
3. `heroku_logs` shows the error itself. Pass `source: heroku` to see the router's H-codes (H10 is
   a crashed app, H12 a timeout, H14 no web dynos).
4. `heroku_formation` shows what the app is meant to run, when the dyno list looks short.

## Cedar and Fir

An app's `generation` is `cedar` or `fir`, and `heroku_apps` shows which. Cedar keeps the newest
1,500 log lines. **Fir keeps no log history at all**, so `heroku_logs` on a Fir app says so rather
than returning nothing. Lasting logs on Fir need a telemetry drain, which
`heroku_call GET apps/<app>/telemetry-drains` lists.

## Config vars

The values are live secrets: database URLs, API keys and signing secrets. `heroku_config` masks
them unless `reveal` is set. Reveal a value only when the owner asked for it, and quote it back
only when the owner asked for that too. Saying a var is set, and how long it is, answers most
questions.

## The rest of the API

`heroku_call` reaches every endpoint in the Platform API. These are the reads people want most:

- `apps/<app>/builds` lists builds, and `apps/<app>/builds/<id>` shows one.
- `pipelines`, then `pipelines/<id>/pipeline-couplings` and `pipelines/<id>/review-apps`, cover
  pipelines and review apps. Most pipeline paths take the pipeline's id, not its name.
- `apps/<app>/collaborators`, `apps/<app>/features` and `apps/<app>/sni-endpoints` cover access,
  features and certificates.
- `apps/<app>/log-drains` lists Cedar's drains, and `apps/<app>/telemetry-drains` lists Fir's.
- `account/rate-limits` reports how many calls are left. The pool holds 4,500 and refills at
  about 75 a minute.

The full list is at https://devcenter.heroku.com/articles/platform-api-reference.

## Finish it — never hand it back

A question about an app is answered with what a tool returned in THIS turn, with the app named and
any crashed dyno or failed release called out — never with "I can't reach Heroku." Every named tool
only reads. A `heroku_call` with POST, PATCH, PUT or DELETE changes a live app and needs the owner's
say-so first: deleting an app, an add-on or a domain cannot be undone, a dyno restart drops live
requests, and a config var change restarts every dyno.
