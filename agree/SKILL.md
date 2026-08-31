---
name: agree
description: >
  The owner's Agree account — invoices, agreements, contacts, customers, templates and reports.
  All 38 API operations.
binary: agree
version: 0.1.0
requires_env: [AGREE_API_KEY]
tools:
  - name: billing_operations
    description: >
      List every Agree operation, with the arguments each one takes and whether it changes data.
      Call this first when you are not certain of an operation's name or its argument names.
    argv: [tools]
    timeout_ms: 60000
    input:
      type: object
      properties: {}

  - name: billing
    description: >
      Run one Agree operation. Pass `op` as billing_operations lists it (list_invoices,
      get_invoice, mark_invoice_paid) and that operation's arguments in `args`; the reply is JSON.
      Reading is free, and so is anything that stays inside his own account — drafting an invoice,
      editing one, adding a contact, renaming a template. Do those and report them. `confirm` is
      for the two things that cannot be taken back: anything that REACHES A CUSTOMER (sending or
      issuing an invoice, emailing an agreement) and anything that DELETES. Those need him to have
      just asked for that specific action. AMOUNTS ARE INTEGER CENTS — $150.00 is
      15000, and getting that wrong is a 100x billing error. Contacts cannot be searched by name,
      only by email or company.
    argv: [call, "{{op}}", "{{@pairs:args}}", "{{confirm?:--yes}}"]
    timeout_ms: 60000
    error_hint: >
      If this failed asking for confirmation, the operation either reaches a customer or deletes
      something. Re-run with confirm true only after the owner has said yes to this exact action —
      not to the general topic.
    input:
      type: object
      required: [op]
      properties:
        op:
          type: string
          description: operation name, as billing_operations lists it
        args:
          type: object
          additionalProperties: true
          description: the operation's arguments
        confirm:
          type: boolean
          description: >
            required for anything that creates, changes or deletes; only ever true when the owner
            has just said yes
---

# agree

Invoicing and agreements. This is the tool that bills people, so it is the one to be slowest with.

## Cents, not dollars

**Every amount is an integer number of cents.** $150.00 is `15000`. Sending `150` charges someone
a dollar fifty; sending `15000` when you meant $150 in the *other* direction is a $15,000 invoice.

There is a guarded path that converts dollars to cents and shows a form — but it only exists when
the owner runs `agree` himself in a terminal. **This tool posts the raw request.** So for anything
that bills someone:

> State the amount back to him in **both** dollars and cents, and get a yes, before you send it.

"That's $2,400.00 — 240000 cents — to Croissant, due Sept 1. Confirm?"

## The two-step

`billing_operations` lists every operation *with its argument names*, which are not always what you
would guess. Read it before composing a call you have not made before.

```
billing  op=list_invoices  args={ "statuses": "due,sent", "date_start": "2026-08-01" }
billing  op=get_invoice    args={ "id": "…" }
```

## Gotchas

- **Contacts cannot be searched by name** — only by email or company. Looking someone up by their
  first name will come back empty and it is not a bug.
- `statuses` takes a comma-separated string, not an array.
- Operations tagged `[CHANGES DATA]` in the listing need `confirm: true`.
- An invoice that has been sent is visible to the customer. Fix a mistake by talking to the owner,
  not by quietly issuing a correction.

## Cross-check against the bank

An invoice here records what was AGREED. It is not proof that money moved. "Paid" in Agree and a
matching credit in Mercury are two different claims, and only the second one is cash.

When either question comes up — was this paid, how much came in, what is outstanding — check both
and say which one you read. Where they disagree, say so plainly rather than picking the friendlier
number; a mismatch between the ledger and the bank is the most useful thing you can surface.

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
