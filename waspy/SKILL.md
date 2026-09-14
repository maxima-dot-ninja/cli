---
name: waspy
description: >
  Read the owner's WhatsApp: his chats, their messages, what is unread, and a search across all of
  it. Read-only. Use whenever he refers to something said or sent on WhatsApp, asks what someone
  messaged him, or wants to catch up on what he has not read.
binary: waspy
version: 0.1.0
tools:
  - name: waspy_chats
    description: >
      List the owner's WhatsApp chats, newest first. Each has its name, whether it is a person or a
      group, how many messages are unread, and the last message. Start here to find a chat's exact
      name before reading it.
    argv: [chats, --json, [-n, "{{count?}}"]]
    timeout_ms: 30000
    input:
      type: object
      properties:
        count:
          type: integer
          description: how many chats to list (default 30)

  - name: waspy_unread
    description: >
      Every WhatsApp chat with unread messages, each with those messages. Use it for "what did I
      miss", "any new messages", or a morning catch-up.
    argv: [unread, --json]
    timeout_ms: 30000
    input:
      type: object
      properties: {}

  - name: waspy_read
    description: >
      Read one WhatsApp chat's recent messages, oldest first, with who sent each and when. Name the
      chat by any part of its name; if several chats match, the reply lists them so you can pick
      one. Media comes through as a label such as [photo] or [voice note], with any caption.
    argv: [read, "{{chat}}", --json, [-n, "{{count?}}"], [--since, "{{since?}}"]]
    timeout_ms: 30000
    input:
      type: object
      required: [chat]
      properties:
        chat:
          type: string
          description: the chat's name, or any part of it, or its id from waspy_chats
        count:
          type: integer
          description: how many messages (default 50)
        since:
          type: string
          description: only messages newer than this, like 30m, 6h, 2d or 1w

  - name: waspy_search
    description: >
      Find WhatsApp messages containing every word given, across all chats or one, newest first.
      It is a word match, not a meaning search, so try the words someone would actually have typed.
    argv: [search, "{{query}}", --json, [-n, "{{count?}}"], [--chat, "{{chat?}}"]]
    timeout_ms: 30000
    input:
      type: object
      required: [query]
      properties:
        query:
          type: string
          description: the words to look for
        count:
          type: integer
          description: how many messages to return (default 20)
        chat:
          type: string
          description: only search this chat (any part of its name)
---

# waspy

waspy reads the owner's WhatsApp straight out of the desktop app's own database. It never sends,
edits or deletes anything, and it cannot: it opens the database read-only.

## How fresh it is

Every reply starts with `source` and `as_of`. `live` means it read WhatsApp's database directly and
is current to the second. `snapshot` means macOS would not let this process read the original —
which is always the case inside vaulty — so it read the copy a launchd job refreshes every two
minutes, and `as_of` says how old that copy is. Mention the age when it matters, for example when
the owner asks about something that happened in the last few minutes.

## What it cannot do

It cannot see photos, voice notes or files, only that they were sent and any caption. It cannot
reply. If a reply says there is no copy yet, or the copy is hours old, the sync job has stopped:
tell the owner to run `waspy status`.

## Privacy

These are other people's messages to the owner. Read what the task needs, and do not repeat a
third party's words anywhere they did not send them.

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

Stop ONLY for the four things that cannot be taken back: destroying data, showing something to
anyone who is not him, sending a message to another person, or spending his money.

Done here means he has the answer out of his WhatsApp in a sentence — who said what, and when —
with the message quoted when the wording matters. waspy is read-only: it cannot send, edit or
delete anything, so nothing it does needs asking first. The one brake that applies is the second
above: other people's messages to him are never shown to anyone else.
