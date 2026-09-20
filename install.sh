#!/bin/sh
# vaulty installer.
#
#   curl -fsSL https://raw.githubusercontent.com/maxima-dot-ninja/cli/main/install.sh | sh
#
# vaulty's source is private; only the built binary is published here, which is what makes this
# fetchable at all — a machine you are setting up for the first time has no credentials to
# authenticate with, and a binary carries no secrets.
#
# This installs the binary and NOTHING else. No config is written and no daemon is started,
# because a fresh vaulty has nothing to run with. The one command that changes that is:
#
#   vaulty login <code>
#
# where <code> comes from `/spawn` on a machine that already works. That claims the token and
# pulls every secret in one step.
#
# Overridable: VAULTY_VERSION=vaulty-v0.1.0, VAULTY_BIN_DIR=/usr/local/bin

set -eu

REPO="maxima-dot-ninja/cli"
BIN_DIR="${VAULTY_BIN_DIR:-$HOME/.local/bin}"

say() { printf '  %s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

# ── which build ──────────────────────────────────────────────────────────────────────────
os=$(uname -s)
arch=$(uname -m)
case "$os-$arch" in
  Darwin-arm64)   target="aarch64-apple-darwin" ;;
  Darwin-x86_64)  target="x86_64-apple-darwin" ;;
  Linux-x86_64)   target="x86_64-unknown-linux-gnu" ;;
  *) die "no vaulty build for $os $arch" ;;
esac

echo "vaulty → $target"

# ── which version ────────────────────────────────────────────────────────────────────────
# This repo holds a dozen tools, so releases/latest could easily be something else. Pick the
# newest tag that is actually a vaulty one.
if [ -n "${VAULTY_VERSION:-}" ]; then
  tag="$VAULTY_VERSION"
else
  tag=$(curl -fsSL "https://api.github.com/repos/$REPO/releases?per_page=100" \
    | grep -o '"tag_name"[ ]*:[ ]*"vaulty-v[^"]*"' \
    | head -1 \
    | sed -E 's/.*"(vaulty-v[^"]*)".*/\1/')
fi
[ -n "$tag" ] || die "found no vaulty release in $REPO — has one been published yet?"
say "version: $tag"

# ── download and verify ──────────────────────────────────────────────────────────────────
tarball="vaulty-$target.tar.gz"
base="https://github.com/$REPO/releases/download/$tag"

tmp=$(mktemp -d)
# Leaves nothing behind on success OR on any failure below.
trap 'rm -rf "$tmp"' EXIT INT TERM

say "downloading…"
curl -fsSL "$base/$tarball" -o "$tmp/$tarball" || die "could not download $base/$tarball"

# A truncated download is the failure that would otherwise show up much later as a confusing
# crash, so check the hash rather than trusting the transfer.
if curl -fsSL "$base/$tarball.sha256" -o "$tmp/$tarball.sha256" 2>/dev/null; then
  ( cd "$tmp" && if command -v shasum >/dev/null 2>&1; then
      shasum -a 256 -c "$tarball.sha256" >/dev/null
    else
      sha256sum -c "$tarball.sha256" >/dev/null
    fi ) || die "checksum did not match — download it again"
  say "checksum ok"
else
  say "no checksum published for this release — skipping verification"
fi

tar -xzf "$tmp/$tarball" -C "$tmp"
[ -f "$tmp/vaulty" ] || die "the archive did not contain a vaulty binary"

# ── install ──────────────────────────────────────────────────────────────────────────────
mkdir -p "$BIN_DIR"
# mv, not cp: replacing a running binary in place can hand the kernel a half-written file.
mv "$tmp/vaulty" "$BIN_DIR/vaulty"
chmod +x "$BIN_DIR/vaulty"
say "installed: $BIN_DIR/vaulty"

echo
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *)
    say "$BIN_DIR is not on your PATH. Add it:"
    say "  echo 'export PATH=\"$BIN_DIR:\$PATH\"' >> ~/.zshrc && exec zsh"
    echo
    ;;
esac

cat <<'DONE'
Next, on a machine where vaulty already works, run:

    /spawn

then bring its code back here:

    vaulty login <code>

That claims the machine token and pulls every secret onto this machine. The code is good for
ten minutes and one use.
DONE
