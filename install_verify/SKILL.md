---
name: install_verify
description: >
  Prove the vaulty daemon actually restarted onto the freshly built binary after `./bin/install`.
  One read-only call that checks the PID, the binary it runs, when it started, and whether it
  answers a ping — and says so in JSON.
binary: install_verify
version: 0.1.0
tools:
  - name: install_verify
    description: >
      Verify the vaulty daemon after an install. Finds the daemon's PID through launchd (falling
      back to a single `vaulty serve` process), confirms it is running `~/.cargo/bin/vaulty`,
      confirms it started AFTER `install_time`, and runs `ping_command` to prove it is
      responsive. Pass `install_time` as ISO-8601 — the instant `./bin/install` was invoked, NOT
      now — or the start-time check is meaningless and always passes. Leave `ping_command` alone
      unless the default `vaulty status` is the wrong thing to ask. The reply is always JSON, and
      a non-zero exit is a failed check with the reasons in `error`, not a crash.
    argv:
      - ["--install-time", "{{install_time?}}"]
      - ["--ping-command", "{{ping_command?}}"]
    timeout_ms: 30000
    error_hint: >
      A non-zero exit still prints the full JSON report. Read it: `error` lists every check that
      failed, joined by semicolons. `binary_ok` false means the daemon is running a different
      binary than the one just built. `start_after_install` false means launchd never restarted
      it. `ping_ok` false means it is up but not answering, and `ping_output` has what it said.
      No PID at all means the daemon is not running. Report which one it is — do not re-run hoping
      for a different answer.
    input:
      type: object
      properties:
        install_time:
          type: string
          description: >
            when `./bin/install` was invoked, ISO-8601, e.g. 2026-09-15T11:17:00+02:00. A naive
            timestamp is read in the local timezone. Defaults to now, which makes the start-time
            check trivially pass — always pass the real install time.
        ping_command:
          type: string
          description: >
            the command that proves the daemon is responsive, split on whitespace and run with a
            15 second timeout. Defaults to `vaulty status`.
---

# install_verify

The daemon that vaulty runs under launchd is easy to install and hard to trust. `./bin/install`
can succeed, the new binary can land in `~/.cargo/bin`, and the old process can still be the one
answering — because launchd never bounced it, or because it bounced onto a stale path. This tool
exists so that "installed" means something you checked rather than something you assumed.

It is **read-only**. It runs `launchctl list`, `ps`, and the ping command. It does not restart,
kill, or reinstall anything. If it says the install went wrong, fixing that is a separate act.

## What it checks

- **There is exactly one daemon.** The PID comes from `launchctl list com.vaulty.daemon`. If
  launchd has no PID it falls back to `pgrep -f "vaulty serve"`, and zero or several matches is
  an error, not a guess.
- **It runs the right binary.** The process's executable is resolved through symlinks and
  compared to the resolved `~/.cargo/bin/vaulty`. `binary_ok` is that comparison.
- **It started after the install.** The process start time is compared to `install_time`.
  `start_after_install` is true only if the daemon is younger than the install.
- **It answers.** `ping_command` is run and must exit zero within 15 seconds. `ping_ok` says
  whether it did, and `ping_output` holds whatever it printed on stdout and stderr.

The exit code is zero only when all three boolean checks pass. When any fail, the JSON still comes
out in full and `error` names every failure, so one call gives the whole picture.

## Using it

Record the time before you run the install, then pass it in:

```
install_verify  install_time=2026-09-15T11:17:00+02:00
```

Calling it with no `install_time` is fine for "is the daemon up and answering" but proves nothing
about whether the install took — the start-time check compares against now and always passes.

A useful reply looks like this:

```json
{
  "pid": 48213,
  "binary_path": "/Users/a32/.cargo/bin/vaulty",
  "binary_resolved": "/Users/a32/.cargo/bin/vaulty",
  "binary_ok": true,
  "start_time": "2026-09-15T11:17:04+02:00",
  "install_time": "2026-09-15T11:17:00+02:00",
  "start_after_install": true,
  "ping_command": "vaulty status",
  "ping_ok": true,
  "ping_output": "vaulty 0.4.2 — serving on 127.0.0.1:7878"
}
```

## Reading a failure

Each failing check points at one cause, so say which it is instead of "verification failed":

- **`binary_ok` false** — the daemon is running something other than `~/.cargo/bin/vaulty`.
  `binary_path` and `binary_resolved` show what it is actually running.
- **`start_after_install` false** — the binary is right but the process predates the install, so
  launchd never restarted it and the old code is still serving.
- **`ping_ok` false** — the process is there but did not answer, or answered with a non-zero
  exit. `ping_output` has what it said, including a timeout note if it hung.
- **No `pid` and an `error`** — there is no daemon to check.

## Finish it — never hand it back

The deliverable is the work, done. Not a plan, not a status report, not a menu.

- **Never end with a question you could have answered by acting.** "I can do A or B — which?"
  is a wasted turn. Pick the obvious one and do it.
- **Never report what you were about to do.** He asked for the thing, not the approach.
- **"To the best of your abilities" means do ALL of it** — it is not permission to assess and
  report back.
- **Work in bounded batches and land each one before starting the next**, so running out of time
  leaves real work done instead of a half-built plan.
- **If you cannot finish, land what you can** and say exactly where you stopped. Never a question
  about how to continue.
- **Anything urgent leads** — money lost, something broken, a deadline. Put it first, before any
  account of what you did.

For this tool, done means one thing: a plain verdict — the daemon is running the new binary and
answering, or it is not — with the failing check named and its value from the JSON quoted, never
"here is the report" with the reading left to him.

Stop ONLY for the four things that cannot be taken back: destroying data, showing something to
anyone who is not him, sending a message to another person, or spending his money.
