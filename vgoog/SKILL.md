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
          description: >
            which connected account, by the name google_accounts lists, when he has more than one
            (defaults to the active one)

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

  - name: google_accounts
    description: >
      List every connected Google account: its name, the address it acts as, and its tier — `user`
      is a person's mailbox and never sends mail, `ai` is the assistant's own and may. Reads the
      local config only, so it is instant. Call it before passing `account` to `google`.
    argv: [accounts]
    timeout_ms: 30000
    input:
      type: object
      properties: {}
---

# vgoog

One binary over the whole of Google Workspace. It holds the owner's credentials itself, and every
call returns JSON.

## Two ways in, same actions

**Inside vaulty** — four tools, and the order matters:

1. **`google_actions`** — the map. Ten services, each with its own action list. Read it before
   reaching for anything you have not used in this conversation.
2. **`google`** — the work. `service` + `action` + `args`.
3. **`google_status`** — the alibi. Run it before reporting a failure, so you can say whether the
   problem is the request or the connection.
4. **`google_accounts`** — who is connected. It gives you the names `account` accepts, and which
   of them may send mail.

**Anywhere with a shell** — Claude Code, a script, CI — it is a CLI, and the same actions:

```bash
vgoog list                                   # every service and action
vgoog doctor                                 # is it connected, and as whom
vgoog accounts                               # every account, its tier, and the address it acts as
vgoog switch <account>                       # make that account the default for later calls
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

## Scope: the newest 100, and no listing of the whole mailbox

**"Process my emails" means the newest ~100.** Not the inbox. The cap goes on the LIST call —
`max_results: 100` — so 100 is all that is ever fetched.

Never enumerate the mailbox to find out how big it is. A harvest of every id and header is not
progress, it is the thing that eats the turn and produces nothing: three thousand messages cannot be
reasoned about, and a run that dies during the harvest has done zero work he can use. There is no
question the full list answers that the newest hundred does not.

A number he names wins. A query that narrows things ("from Samir", "unread this week") sets its own
scope — run it, take what it returns, still capped at ~100.

## Land the work as you go

Classify and label in **batches of about 20, applying each batch before fetching the next.** Never
classify everything first and apply at the end.

The reason is failure: a run that stops halfway must leave 60 messages genuinely handled, not a
half-built plan for 3,000. Batching turns a timeout into partial success instead of total loss.

If you run out of room, stop cleanly and say exactly where the line fell — "worked the newest 100,
oldest untouched is 12 Aug". That is a finished piece of work with a known edge.

## Finish it — never hand it back

The deliverable is the work, done. "I did the harvesting, which of these two things do you want
next?" is the failure this whole section exists to prevent. He asked for emails to be processed;
processed emails are the deliverable.

- **Never end with a question you could have answered by acting.** Pick the obvious one and do it.
- **Never report what you were *about* to do** — not a classifier, not a plan, not your progress.
- **Never offer him a menu of ways to continue.**
- **"To the best of your abilities" means do ALL of it** — not assess and report back.
- **If you cannot finish, land what you can** and say exactly where you stopped.

**Done here** means messages labelled, archived, trashed or drafted — the inbox measurably worked.
**Irreversible here**, and the only things to stop for: actually sending mail to another person,
sharing anything with anyone who is not him, and permanent `delete_message` (trashing is reversible
and needs no permission).

**Anything urgent goes at the TOP, before any account of what you did.** A failed payment, an expired
domain, a suspended account, a legal or tax deadline — that is the message, and the triage summary is
the footnote. Burying "Stripe payment failed" under a list of completed steps is worse than not
running at all, because it reads as handled.

## Time is a triage input, not a detail

Your system prompt carries the current date and time. **Use it on every message.** An email's own
date changes what should happen to it, often more than its subject line does.

For each message, ask what its age did to it:

- **Still live** — the date is recent or the deadline is ahead. Triage on content as normal.
- **Expired and harmless** → archive or trash it. A sale that ended, a webinar that happened, a
  flight that already flew, "24 hours only" from three weeks ago, a meeting invite for last Tuesday.
  These are noise now no matter how loud they were then. Do not surface them; do not ask.
- **Expired and HARMFUL** → this is the one that matters. Age made it worse, not moot. A failed
  payment, a card about to expire, a domain lapsing, an account suspension, a legal, tax or filing
  deadline, a renewal that silently auto-charges. Every day that passed raised the cost. **These go
  to the top of the report even when they are the oldest thing in the pile** — especially then,
  because nobody caught them.

**Loudness in the email is not urgency.** "URGENT", "FINAL NOTICE" and "ACTION REQUIRED" are
marketing on most messages. The date decides urgency, not the subject line. Equally, a quiet
automated receipt from six weeks ago saying a charge failed outranks anything shouting today.

**When something old is still unresolved, say how long it has been sitting.** "Stripe payment failed
— 23 days ago, still unpaid" is actionable; "Stripe payment failed" is not.

**Old threads he replied to are done.** If he already answered, the thread is not outstanding no
matter how old. Check for his reply before flagging anything as needing him.

Bulk age rules that need no thought: automated notifications, newsletters, social and marketing
older than ~30 days with no reply from him are archive-on-sight.

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
  email" — is the active account. Pass `account` only when he names one. `vgoog accounts` lists
  the names. Use `--account` for a single call rather than `vgoog switch`, because switching moves
  his default for everything that comes after.
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
