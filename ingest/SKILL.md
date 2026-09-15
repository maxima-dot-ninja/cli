---
name: ingest
description: >
  Read the owner's local project files — list a folder, read one file, grep for text, or pull a
  whole folder in at once. Use whenever he points you at code or files on this machine and you
  need to see what is in them.
binary: ingest
# No requires_env: every setting has a working default. INGEST_ROOT only changes where "here" is.
tools:
  - name: ingest_list
    description: >
      Every regular file under a folder, as a JSON array of absolute paths, sorted. Symlinks,
      dotfiles, secrets and the usual junk folders (.git, node_modules, target, dist, build, …)
      are left out. Start here to learn the shape of a project before reading anything. `depth`
      keeps it shallow; `include` and `exclude` are comma-separated globs where `*` crosses `/`,
      so `*.rs` means every Rust file at any depth.
    argv: [list, "{{root}}", --json, [--depth, "{{depth?}}"], [--include, "{{include?}}"], [--exclude, "{{exclude?}}"], "{{hidden?:--hidden}}"]
    timeout_ms: 60000
    input:
      type: object
      required: [root]
      properties:
        root:
          type: string
          description: absolute path of the folder to list
        depth:
          type: integer
          description: how many levels down to go; 1 means the folder's own entries only
        include:
          type: string
          description: comma-separated globs a file must match, e.g. "*.ts,*.tsx"
        exclude:
          type: string
          description: comma-separated globs to skip, added on top of the defaults
        hidden:
          type: boolean
          description: include dotfiles and dot-folders

  - name: ingest_read
    description: >
      One file as JSON: path, size, offset, bytes_read, truncated, binary, encoding and content.
      Text comes back as text; anything with a NUL byte comes back base64 with binary true. At
      most 256KB is served per call — when `truncated` is true, call again with `offset` set to
      the previous offset plus bytes_read to read on. The path must sit inside `root`; anything
      that resolves outside it is refused, as are dotfiles and files that look like secrets (.env,
      keys, certificates, credentials) unless the matching flag is set. Only set `allow_secrets`
      when the owner has asked to see that specific file.
    argv: [read, "{{path}}", --root, "{{root}}", --json, [--offset, "{{offset?}}"], [--max-bytes, "{{max_bytes?}}"], "{{binary?:--binary}}", "{{hidden?:--hidden}}", "{{allow_secrets?:--allow-secrets}}"]
    timeout_ms: 60000
    error_hint: >
      A "refused" error means the path is outside the root, hidden, or looks like a secret. Check
      the root is the project folder; pass hidden for dotfiles like .gitignore; pass allow_secrets
      only when he asked for that file by name.
    input:
      type: object
      required: [path, root]
      properties:
        path:
          type: string
          description: the file, absolute or relative to root
        root:
          type: string
          description: absolute path of the folder the file must live under
        offset:
          type: integer
          description: byte offset to start from (default 0)
        max_bytes:
          type: integer
          description: maximum bytes to return (default 262144)
        binary:
          type: boolean
          description: force base64 output even for text
        hidden:
          type: boolean
          description: allow a dotfile or a file inside a dot-folder
        allow_secrets:
          type: boolean
          description: allow env files, keys, certificates and credential folders

  - name: ingest_grep
    description: >
      Search file contents under a folder for a regex (or a literal string with `literal` true).
      Returns a JSON array of hits, each with path, line and snippet, in path order, at most
      `max` of them (default 500). Binary files, files over 8MB, dotfiles, secrets and the
      default junk folders are skipped. Use this to find where something is defined or used
      before reading the file.
    argv: [grep, "{{pattern}}", "{{root}}", --json, "{{literal?:--literal}}", "{{ignore_case?:-i}}", [--max, "{{max?}}"], [--include, "{{include?}}"], [--exclude, "{{exclude?}}"], "{{hidden?:--hidden}}"]
    timeout_ms: 120000
    input:
      type: object
      required: [pattern, root]
      properties:
        pattern:
          type: string
          description: a Rust-flavoured regex, or plain text when literal is true
        root:
          type: string
          description: absolute path of the folder to search
        literal:
          type: boolean
          description: match the pattern as plain text, not a regex
        ignore_case:
          type: boolean
          description: case-insensitive match
        max:
          type: integer
          description: stop after this many hits (default 500)
        include:
          type: string
          description: comma-separated globs a file must match, e.g. "*.py"
        exclude:
          type: string
          description: comma-separated globs to skip, added on top of the defaults
        hidden:
          type: boolean
          description: include dotfiles and dot-folders

  - name: ingest_directory
    description: >
      A whole folder in one call: a JSON array of {path, content, binary, truncated}, one per file,
      text as text and binary as base64. Same rules as ingest_list for what is left out, and each
      file is cut at `max_bytes` bytes (default 256KB) with truncated true. This is the fast way to load
      a small project or one subfolder; for a big tree, narrow it with `depth` or `include` first or
      the reply will be huge.
    argv: [ingest, "{{root}}", --json, [--depth, "{{depth?}}"], [--include, "{{include?}}"], [--exclude, "{{exclude?}}"], [--max-bytes, "{{max_bytes?}}"], "{{hidden?:--hidden}}"]
    timeout_ms: 120000
    input:
      type: object
      required: [root]
      properties:
        root:
          type: string
          description: absolute path of the folder to ingest
        depth:
          type: integer
          description: how many levels down to go
        include:
          type: string
          description: comma-separated globs a file must match
        exclude:
          type: string
          description: comma-separated globs to skip, added on top of the defaults
        max_bytes:
          type: integer
          description: maximum bytes per file (default 262144)
        hidden:
          type: boolean
          description: include dotfiles and dot-folders
---

# ingest

Local files, read-only, inside one root. Nothing here writes, and nothing here can reach past the
folder you name as the root.

## Which tool for which question

- **"What is in this project?"** → `ingest_list` with a `depth` of 2 or 3 first. The paths alone
  usually tell you where to look next.
- **"Where is X defined?"** → `ingest_grep`. Use `literal` when the text has dots, brackets or
  slashes in it, so it is not read as a regex.
- **"Show me that file"** → `ingest_read`. Watch `truncated`; a big file needs more than one call.
- **"Read the whole thing"** → `ingest_directory`, narrowed with `include` or `depth`. It returns
  every file's content at once, so on a big tree it is the wrong first move.

## Gotchas

- Every path is checked against `root`. A path that resolves outside it, through `..` or a
  symlink, is refused. Use the project folder as the root, not `/`.
- Dotfiles and dot-folders are skipped everywhere unless `hidden` is set. `.gitignore` and
  `.github/` need it; `.env` needs `allow_secrets` on top, and only when he asked for that file.
- The default excludes (`.git`, `node_modules`, `target`, `dist`, `build`, …) apply to every walk.
  `exclude` adds to them. They do not stop `ingest_read` from opening a file named outright.
- Globs match the path relative to the root and `*` crosses `/`. `src/*.rs` matches everything
  under `src` at any depth; `*.test.ts` matches every test file in the tree.

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

Done means you have read what he pointed you at and answered from it. Nothing this tool does can be
undone, because it never writes; the one thing to hold back on is `allow_secrets`, which shows you
his credentials, so only set it when he named the file.
