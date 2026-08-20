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

One binary over the whole of Google Workspace. It holds the owner's OAuth tokens itself, and every
call returns JSON.

## How to use it

Three tools, and the order matters:

1. **`google_actions`** — the map. Ten services, each with its own action list. Read it before
   reaching for anything you have not used in this conversation.
2. **`google`** — the work. `service` + `action` + `args`.
3. **`google_status`** — the alibi. Run it before reporting a failure, so you can tell him whether
   the problem is the request or the connection.

## Worked examples

```
google  service=calendar  action=list_events   args={ "timeMin": "2026-08-16T00:00:00Z", "maxResults": 10 }
google  service=gmail     action=list_messages args={ "q": "from:samir is:unread" }
google  service=drive     action=search_files  args={ "q": "name contains 'invoice'" }
```

Arguments are passed through to Google's own API, so its parameter names are the ones that work —
`timeMin`, not `start_time`; `q`, not `query`. `google_actions` tells you the action names;
Google's API docs are the truth for the arguments inside `args`.

## Gotchas

- **Multiple accounts.** He has more than one connected. Anything ambiguous — "my calendar", "check
  my email" — is the active account. Pass `account` only when he names one.
- **Auth errors are not your fault and not his.** `Missing required parameter: refresh_token` means
  the account needs reconnecting, not that the request was wrong. Say that plainly.
- **Read before you write.** Deleting an event or sending a message is not reversible from here.
  Confirm with him first unless he has just told you to do exactly that thing.
- Empty results are a real answer. An empty `list_events` means nothing is scheduled — say so,
  rather than trying three more variations of the query.
