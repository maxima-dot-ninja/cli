#!/usr/bin/env python3
"""Rebuild openapi.json from Mercury's live documentation.

Mercury publishes no single spec file. Every page under docs.mercury.com/reference
embeds the OpenAPI fragment for its own operation, and llms.txt indexes the pages.
So: read the index, fetch each page, merge the fragments.

    python3 tools/fetch-spec.py            # writes ../openapi.json
    python3 tools/fetch-spec.py --check    # exit 1 if the vendored file is stale

Run it when Mercury ships new endpoints; `cargo build` turns the result into
commands with no further work.
"""

import argparse
import json
import os
import re
import sys
import urllib.request

INDEX = "https://docs.mercury.com/llms.txt"
HERE = os.path.dirname(os.path.abspath(__file__))
SPEC = os.path.join(HERE, "..", "openapi.json")


def get(url):
    # The docs host refuses urllib's default user-agent with a 403.
    request = urllib.request.Request(url, headers={"User-Agent": "merc-spec-fetch/1.0"})
    with urllib.request.urlopen(request, timeout=30) as response:
        return response.read().decode("utf-8")


def build():
    pages = sorted(set(re.findall(r"https://docs\.mercury\.com/reference/[a-z0-9_]+\.md", get(INDEX))))
    if not pages:
        sys.exit("llms.txt listed no reference pages — the docs layout changed.")

    spec = {
        "openapi": "3.0.0",
        "info": {"title": "Mercury API", "version": "1.0.0"},
        "servers": [],
        "paths": {},
        "components": {"schemas": {}, "securitySchemes": {}},
    }

    for url in pages:
        page = get(url)
        for block in re.findall(r"```json\n(.*?)\n```", page, re.S):
            try:
                fragment = json.loads(block)
            except ValueError:
                continue
            if not isinstance(fragment, dict) or not (fragment.keys() & {"paths", "components"}):
                continue
            for path, operations in fragment.get("paths", {}).items():
                spec["paths"].setdefault(path, {}).update(operations)
            components = fragment.get("components", {})
            spec["components"]["schemas"].update(components.get("schemas", {}))
            spec["components"]["securitySchemes"].update(components.get("securitySchemes", {}))
            for server in fragment.get("servers", []):
                if server not in spec["servers"]:
                    spec["servers"].append(server)
        print(f"  {os.path.basename(url)}", file=sys.stderr)

    # Mercury's docs illustrate the auth header with a real-looking token. It is
    # only an example in a description field, but it matches GitHub's secret
    # scanner exactly, which blocks the push of any repo that vendors this file.
    redacted = re.subn(r"mercury_(production|sandbox)_[A-Za-z0-9_]{6,}", "YOUR_TOKEN", json.dumps(spec))
    spec = json.loads(redacted[0])
    if redacted[1]:
        print(f"redacted {redacted[1]} example token(s)", file=sys.stderr)

    operations = sum(
        1
        for methods in spec["paths"].values()
        for method in methods
        if method in ("get", "post", "put", "patch", "delete")
    )
    if operations < 60:
        sys.exit(f"Only {operations} operations found — refusing to overwrite a good spec.")
    print(f"\n{len(pages)} pages, {operations} operations, {len(spec['components']['schemas'])} schemas", file=sys.stderr)
    return spec


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="report staleness instead of writing")
    args = parser.parse_args()

    fresh = json.dumps(build(), indent=2, sort_keys=True)

    if args.check:
        current = open(SPEC).read() if os.path.exists(SPEC) else ""
        if current.strip() == fresh.strip():
            print("openapi.json is up to date.")
            return
        sys.exit("openapi.json is STALE — run tools/fetch-spec.py")

    with open(SPEC, "w") as handle:
        handle.write(fresh + "\n")
    print(f"Wrote {os.path.relpath(SPEC)}")


if __name__ == "__main__":
    main()
