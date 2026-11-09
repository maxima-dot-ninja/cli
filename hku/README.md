# hku

CLI for the [Heroku Platform API](https://devcenter.heroku.com/articles/platform-api-reference).
It lists apps, dynos, formation, config vars, releases, add-ons and domains, fetches recent logs,
and calls any other endpoint directly.

No AI anywhere. The same input always makes the same request.

## Install

```sh
cargo install --path .
```

Run it from inside the `hku` folder. It needs **Rust 1.88** or newer, because the locked
dependencies require it.

## Token

Make a token that can read apps and config vars, and nothing else. It never expires:

```sh
heroku authorizations:create -d hku -s identity,read-protected
```

Then store it in one of two ways:

- `vaulty secrets set hku HEROKU_API_KEY <token>` puts it where vaulty and `hku` both read it.
- `hku setup` asks for it, checks it against `/account`, and writes `~/.config/hku/config.json`
  with mode `600`.

`HEROKU_TEAM` is optional. When it is set, `hku apps` lists that team's apps instead of every app
the token reaches. **The environment wins over the config file**, as it does for every tool here.
The Heroku CLI reads the same `HEROKU_API_KEY`, so one value works in both.

## Commands

```sh
hku                                   # a menu: pick an app from a list, then what to look at
hku apps                              # every app, with its generation, region, stack and team
hku apps show my-app                  # one app's details
hku dynos my-app                      # the dynos running now, and any that crashed
hku formation my-app                  # process types, quantity and size
hku config my-app                     # config var names, values masked
hku config my-app --reveal            # the same, with the values
hku releases my-app --limit 10        # the newest releases first
hku addons my-app                     # add-ons, plans and the config vars they set
hku domains my-app                    # domains and their DNS targets
hku logs my-app --lines 200           # recent lines; --dyno web.1, --type web, --source app
hku logs my-app --tail                # keep the stream open
hku call GET apps/my-app/builds       # any endpoint, as JSON, every page
hku call PATCH apps/my-app '{"maintenance":true}'   # asks first, unless you pass --yes
```

Every command takes `--json` for the raw API answer.

- **Config var values are masked everywhere**, `hku call` included, unless you pass `--reveal`.
- **Fir apps keep no log history.** `hku logs` says so on a Fir app, and `--tail` shows lines as
  they arrive.
- **Anything but a GET asks first.** With no keyboard to ask at, it stops and says to rerun with
  `--yes`.
