---
name: merc
description: >
  The owner's Mercury bank account — balances, transactions, cards, recipients, statements,
  treasury, invoicing and webhooks. All 72 API operations.
binary: merc
version: 0.1.0
requires_env: [MERCURY_API_KEY]
tools:
  - name: bank_operations
    description: >
      List every Mercury operation, grouped. Each line is a group and a command — that pair is
      exactly what `bank` takes. Call this first whenever you are not certain of an operation's
      name; a guessed name wastes a turn. Operations marked with a bullet change data or move money.
    argv: [ops]
    timeout_ms: 60000
    input:
      type: object
      properties: {}

  - name: bank
    description: >
      Run one Mercury operation. Pass `group` and `command` exactly as bank_operations lists them
      ("accounts" + "list", "cards" + "freeze", "accounts" + "list-transactions"), and that
      operation's arguments in `args`, named as Mercury names them. Reading is free. ANYTHING THAT
      MOVES MONEY or changes data needs confirm true, and you may only set that when the owner has
      just told you to do this specific thing — never on your own initiative, never to retry past a
      refusal. Amounts are decimal dollars, not cents. `all` follows every page of a list;
      `sandbox` uses the test bank instead of real money.
    argv:
      - "{{group}}"
      - "{{command}}"
      - "{{@pairs:args:--}}"
      - --json
      - "{{all?:--all}}"
      - "{{sandbox?:--sandbox}}"
      - "{{confirm?:--yes}}"
    timeout_ms: 60000
    error_hint: >
      If this failed asking for confirmation, that is because the operation changes data and there
      is nobody at this keyboard to answer. Re-run it with confirm true — but only after the owner
      has said yes to this exact operation in this conversation.
    input:
      type: object
      required: [group, command]
      properties:
        group:
          type: string
          description: the group, as bank_operations lists it — accounts, cards, transactions, …
        command:
          type: string
          description: the command within that group, e.g. "list" or "get-cards"
        args:
          type: object
          additionalProperties: true
          description: >
            the operation's arguments, named as Mercury names them — accountId, limit, start, end
        confirm:
          type: boolean
          description: >
            required for anything that changes data or moves money; only ever true when the owner
            has just said yes
        all:
          type: boolean
          description: follow every page of a list
        sandbox:
          type: boolean
          description: use the sandbox bank, not real money
---

# merc

The whole Mercury API, deterministically. No AI anywhere in the tool — the same input always makes
the same request.

## The two-step

`bank_operations` then `bank`. The listing is the contract: it prints a group and a command, and
those are the two fields `bank` takes. Do not invent an operation name from the docs and hope.

```
bank  group=accounts      command=list
bank  group=accounts      command=list-transactions  args={ "accountId": "…", "limit": 50 }
bank  group=cards         command=get-cards          args={ "accountId": "…" }
```

## Money rules — read these before doing anything that moves money

- **Amounts are decimal dollars here.** `50` means fifty dollars. (Agree, the invoicing tool, is
  the opposite — integer cents. Do not carry a habit from one to the other.)
- **`confirm: true` is the owner's word, not yours.** Set it only when he has just asked for this
  specific transfer, this specific card freeze. Never to get past a refusal, never on a hunch that
  he meant it.
- **Say the amount back to him in words before you send anything.** "Sending $1,250.00 to Acme from
  the operating account — yes?"
- **`sandbox: true` is the test bank.** Use it to check the shape of a call you are unsure of.

## Gotchas

- The bulleted operations in `bank_operations` are the ones that change things. Treat an unbulleted
  operation as safe to run without asking.
- `all: true` on a long list can return a lot of output. Prefer a filter and a limit first.
- A confirmation failure is not a bug — see the hint the tool returns.
