#!/usr/bin/env bash
# Single source of truth for the project version is the top-level VERSION file.
# This script propagates it into the spots that need a literal (Cargo can't read
# a package version from an external file). It is idempotent: a file is only
# rewritten when its version actually differs, so it won't trigger spurious
# rebuilds. The build targets run this automatically, so editing VERSION is
# enough — you don't have to run it by hand.
#
#   ./scripts/set-version.sh            # sync everything to the current VERSION
#   ./scripts/set-version.sh 0.2.0      # set VERSION to 0.2.0, then sync
set -euo pipefail
cd "$(dirname "$0")/.."

ver="${1:-$(cat VERSION)}"
if [[ ! "$ver" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]]; then
  echo "error: '$ver' is not a valid version" >&2
  exit 1
fi

changed=0

# Replace a `version` literal in $1 only if it currently differs from $ver.
sync() { # file  extract-regex  match-regex  replacement
  local file="$1" cur
  cur=$(sed -nE "s/$2/\\1/p" "$file" | head -1)
  if [[ "$cur" != "$ver" ]]; then
    sed -i.bak -E "s/$3/$4/" "$file" && rm -f "$file.bak"
    changed=1
  fi
}

if [[ "$(cat VERSION 2>/dev/null)" != "$ver" ]]; then
  printf '%s\n' "$ver" > VERSION
  changed=1
fi

sync Cargo.toml \
  '^version = "([^"]+)".*' '^version = "[^"]+"' 'version = "'"$ver"'"'
sync gui/src-tauri/Cargo.toml \
  '^version = "([^"]+)".*' '^version = "[^"]+"' 'version = "'"$ver"'"'
sync gui/src-tauri/tauri.conf.json \
  '.*"version": "([^"]+)".*' '("version": )"[^"]+"' '\1"'"$ver"'"'

if [[ "$changed" == 1 ]]; then
  echo "version synced to $ver (VERSION, both Cargo.toml, tauri.conf.json)"
else
  echo "version already $ver — nothing to change"
fi
