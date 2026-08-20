---
name: pocket
description: >
  Search and read the owner's recorded conversations and meetings. Use whenever he refers to
  something that was said, heard, discussed, agreed or decided out loud rather than written down.
binary: pocket
# No requires_env: pocket takes its key from POCKET_APP_KEY *or* ~/.config/pocket/key, so an
# unset variable does not mean a broken tool. Only list a var whose absence guarantees failure.
tools:
  - name: pocket_list
    description: >
      List the owner's Pocket AI recordings — his recorded conversations and meetings, newest
      first, with the id, title and date of each. Start here when he refers to something he said
      or was told in a meeting; then pocket_export the one that matches to read it.
    argv: [list]
    timeout_ms: 60000
    input:
      type: object
      properties: {}

  - name: pocket_export
    description: >
      Write a recording's transcript and summary to disk, then read them. Pass the recording id
      from pocket_list, or "all" to export everything. Files land in
      ~/dev/pocket-exports/<title>-<date>/ as transcript.txt and summary.md — the reply names the
      paths, and reading them is a coding or shell job from there. Exporting also re-indexes
      search, so a recording must be exported before pocket_search can find it.
    argv: [export, "{{id}}"]
    timeout_ms: 600000
    input:
      type: object
      required: [id]
      properties:
        id:
          type: string
          description: recording id from pocket_list, or "all" for every recording

  - name: pocket_search
    description: >
      Search everything already exported from Pocket in natural language — meaning, not keywords
      (on-device hybrid search with LLM reranking). Use it for "what did we decide about X", "when
      did I talk to Y". Results are grouped by recording, since the real question is usually which
      conversation it was. A normal search takes 30-60 seconds; set fast for a couple of seconds
      and worse ranking. It only sees recordings that have been exported.
    argv: [search, "{{query}}", --json, [-n, "{{count?}}"], "{{fast?:--fast}}"]
    timeout_ms: 300000
    input:
      type: object
      required: [query]
      properties:
        query:
          type: string
          description: what to look for, in plain words
        count:
          type: integer
          description: how many recordings to return (default 5)
        fast:
          type: boolean
          description: skip the LLM reranking — much faster, worse ranking
---

# pocket

Pocket AI records the owner's conversations and meetings. This skill is how you hear them.

## The shape of the thing

Search does **not** reach the Pocket API. It reads a local index built from **exported** files, and
nothing else. So the loop is always:

1. `pocket_list` — what recordings exist
2. `pocket_export <id>` — pull one down (or `all`)
3. `pocket_search` — ask questions across everything exported

A recording that has never been exported is invisible to search. If a search comes back empty and
the owner is sure the conversation happened, the answer is usually `pocket_export all`, not a
better query.

## Which tool for which question

- **"What did we decide about pricing?"** → `pocket_search`. It is a meaning search; write the
  question the way he asked it, not a keyword soup.
- **"Pull up the call with Samir"** → `pocket_list`, match on title and date, then `pocket_export`.
- **"What were my action items from Tuesday's standup?"** → export it, then read `summary.md` with
  `coding`. The summary is usually enough; the transcript is there when it isn't.

## Gotchas

- `pocket_export all` re-downloads and re-indexes everything. It takes minutes, not seconds. Only
  reach for it when a targeted export has already failed to find the conversation.
- Search runs entirely on this machine. Nothing is uploaded, and it works with no network.
- `fast: true` skips the reranking step. It is the right call when you are checking whether a topic
  exists at all, and the wrong call when the ranking is the answer.
- The output of `pocket_export` names the files it wrote. Read them; do not guess the path.
