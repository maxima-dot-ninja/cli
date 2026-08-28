---
name: vgoog
description: >
  The owner's Google Workspace — Gmail, Calendar, Drive, Sheets, Docs, Slides, Forms, Tasks and
  Contacts — across every connected account.
binary: vgoog
version: 0.1.0
tools:
  - name: google
    description: >
      Do one thing in the owner's Google Workspace. Pass a service (gmail, calendar, drive, sheets,
      docs, slides, forms, tasks, contacts, apps_script), an action, and that action's arguments as
      an object. Returns JSON. If you are not certain of the exact action name or the arguments it
      takes, call google_actions FIRST — guessing an action name wastes a turn.
    argv: [exec, "{{service}}", "{{action}}", "{{args?}}", [--account, "{{account?}}"]]
    timeout_ms: 120000
    input:
      type: object
      required: [service, action]
      properties:
        service:
          type: string
          enum: [gmail, calendar, drive, sheets, docs, slides, forms, tasks, contacts, apps_script]
          description: which Google service
        action:
          type: string
          description: snake_case action name, exactly as google_actions lists it
        args:
          type: object
          description: the action's arguments, as an object
        account:
          type: string
          description: which connected account, when he has more than one (defaults to the active one)

  - name: google_actions
    description: >
      List every Google service and the actions available on each. Cheap, and the right first move
      whenever you are unsure what `google` will accept.
    argv: [list]
    timeout_ms: 30000
    input:
      type: object
      properties: {}

  - name: google_status
    description: >
      Check whether Google access is actually working, and for which account. Call this when a
      `google` call fails with an auth error, before telling the owner anything is broken.
    argv: [status]
    timeout_ms: 30000
    input:
      type: object
      properties: {}
---

# vgoog

One binary over the whole of Google Workspace. It holds the owner's credentials itself, and every
call returns JSON.

## Two ways in, same actions

**Inside vaulty** — three tools, and the order matters:

1. **`google_actions`** — the map. Ten services, each with its own action list. Read it before
   reaching for anything you have not used in this conversation.
2. **`google`** — the work. `service` + `action` + `args`.
3. **`google_status`** — the alibi. Run it before reporting a failure, so you can say whether the
   problem is the request or the connection.

**Anywhere with a shell** — Claude Code, a script, CI — it is a CLI, and the same actions:

```bash
vgoog list                                   # every service and action
vgoog doctor                                 # is it connected, and as whom
vgoog exec <service> <action> '<json args>'  # the work
```

```bash
vgoog exec gmail list_messages '{"query":"from:samir is:unread","max_results":20}'
vgoog exec gmail modify_message '{"id":"18f...","add_labels":["Label_12"]}'
vgoog exec calendar list_events '{"timeMin":"2026-08-16T00:00:00Z","maxResults":10}'
```

Every response is `{"data": ..., "ok": true}` or `{"error": "...", "ok": false}`. Pipe it through
`jq` or python and read `.data`.

## Arguments

Google's own parameter names work, and so do snake_case ones — `maxResults` and `max_results` both
land. `vgoog list` gives you the action names; Google's API docs are the truth for what goes inside.

## Doing a lot at once

Anything touching more than a handful of messages is a script, not a sequence of calls. One `vgoog`
call per email is slow, easy to lose track of, and impossible to re-run. Write the loop, run it,
report what it did:

```bash
vgoog exec gmail list_messages '{"query":"from:notifications@github.com older_than:7d","max_results":200}' \
  | jq -r '.data.messages[].id' \
  | while read id; do vgoog exec gmail modify_message "{\"id\":\"$id\",\"add_labels\":[\"Label_12\"]}"; done
```

Batch endpoints exist and are better still where they fit: `batch_modify_messages` takes an `ids`
array and does the lot in one request.

## Just do it

The owner asked for the thing because he wants it done. Labelling, archiving, marking read,
trashing, moving, searching, reading, creating drafts, creating events — do them, all of them, in
bulk, without checking in. A wrong label is undone in one more call.

Stop and ask ONLY where the action cannot be taken back:

- actually sending a message to another person — mail, a reply, an invite
- showing anything to anyone who is not him: sharing a file, a calendar, a doc
- permanent delete (`delete_message`; trashing is reversible and needs no permission)
- deleting an event other people are already invited to

That is the whole list. Report what you did when you are finished, not what you are about to do.

## Gotchas

- **Multiple accounts.** More than one is connected. Anything ambiguous — "my calendar", "check my
  email" — is the active account. Pass `account` only when he names one.
- **Auth errors are not your fault and not his.** `unauthorized_client` means the service account's
  scopes are not authorised in the admin console; `invalid_grant` means delegation is off or the
  user is wrong. `vgoog doctor` tells you which. Say that plainly rather than guessing at the
  request.
- **An argument error can look like a permissions error.** "no label updates were provided" means
  the labels did not reach the call, not that a scope is missing. Check `vgoog doctor` before
  concluding anything about scopes.
- Empty results are a real answer. An empty `list_events` means nothing is scheduled — say so,
  rather than trying three more variations of the query.

## The mailbox is evidence, not a feed

Most questions that reach this tool are not "what is in my inbox" — they are "what happened", and
the mailbox is where the proof of it landed. Treat it that way.

**Search for the thing.** A question about a payment, a booking, an order, a person: search the
sender, the amount, the reference, the word. Listing the twenty newest messages and summarising
them answers a question nobody asked, and buries the receipt that was three weeks down.

**Read the message, not the preview.** Preview text is the first bytes of the HTML, which on any
templated receipt is boilerplate — often a hidden fragment that flatly contradicts the real content.
An amount or a balance is only real once it has been read out of the message body.

**This is the second source.** When the bank, the invoicing tool or the calendar cannot be reached
or does not have it, the confirmation email almost always does. And when they CAN be reached, the
email is what confirms them — an amount that appears in two places independently is a fact, one that
appears in a single place is a lead.
