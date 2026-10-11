# ship

The half of every deploy that is the same in every repo, so each repo's own deploy script is only
the part that is different: where its build goes.

```sh
git add …
npm run deploy        # a Node repo
bin/deploy            # a Rust repo
```

1. **What is staged becomes a commit.** lgit writes the message, and it is accepted as written.
2. **The commit is pushed to the default branch on GitHub** — `main`, or `master` where that is the name. A deploy refuses when GitHub has commits this checkout
   does not, because shipping past them would take someone else's change off the live site.
3. **A clean copy of exactly that commit is built and shipped.** It lives in
   `~/.cache/ship/<repo>-<hash>`, a git worktree kept between deploys so build caches stay warm, and
   put at the commit each time. Changes you did not stage are never in it.

Nothing runs on GitHub. It only keeps the code.

## Rerunnable

- Each step is recorded against the commit in `.git/ship` once it succeeds. A rerun after a failure
  or a Ctrl-C does only what is left, and a rerun with nothing new says so and stops.
- A step that exits 75 was skipped — Docker not running, say. It is named at the end, and the next
  deploy tries it again.
- One deploy per repo at a time. A second one refuses and names the first; a lock left by a deploy
  that died is taken over.

## `--dry-run`

Commits, pushes and ships nothing. It shows what is staged and what would go out, and runs the
local builds, so a real deploy of the same commit afterwards reuses them.

## `--mac`

Builds and ships only what runs on a Mac. vaulty then builds its two Mac targets and leaves Linux and
the Raspberry Pis alone, and Studio skips its Linux `.deb`; a repo with nothing else to build ignores
it. A repo's script reads it as `SHIP_MAC=1`. The builds it leaves out stay as last published.

## Writing a repo's deploy

```bash
#!/usr/bin/env bash
set -euo pipefail
source "$(command -v ship)"

ship_begin "$@"
ship_build npm ship_npm                      # local: runs in a dry run too
ship_step fly flyctl deploy --remote-only    # reaches past this Mac: skipped in a dry run
ship_end
```

Steps run inside the deploy copy, `$SHIP_TREE`. `$SHIP_MAIN` is the checkout you work in, and
`ship_bring <path>` copies an ignored file a build needs from it (`.env`, `.vercel/`, a linked
Supabase project). `ship_npm` installs the copy's dependencies only when `package-lock.json` changed.

## Install

```sh
chmod +x ship/ship
ln -s "$PWD/ship/ship" /opt/homebrew/bin/ship
```

Symlinked, like ccx, so edits are live at once. It needs `git` and, to commit, `lgit`.
