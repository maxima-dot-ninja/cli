#!/bin/sh
# vaulty used to install from here. It is private now: its builds live in a private bucket, and the
# installer is served by the api behind a code that only the owner can mint. This file stays so an
# old copy of the one-liner says where to go instead of failing on a release that no longer exists.

cat >&2 <<'MOVED'
vaulty no longer installs from this repository.

Sign in at https://www.maxima.ninja/downloads and copy the install line from there.
It picks the right build for this machine by itself, a Raspberry Pi included.
MOVED
exit 1
