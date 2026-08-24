#!/usr/bin/env bash
set -euo pipefail

lock=${1:-Cargo.lock}
notices=${2:-THIRD_PARTY_NOTICES.md}
[[ -f "$lock" && -f "$notices" ]] || { echo "error: lockfile and notices are required" >&2; exit 1; }

require_locked() {
  local name=$1 version=$2
  awk -v name="$name" -v version="$version" '
    $0 == "name = \"" name "\"" { found_name = 1; next }
    found_name && $0 == "version = \"" version "\"" { found = 1 }
    found_name && /^$/ { found_name = 0 }
    END { exit !found }
  ' "$lock" || {
    echo "error: $name $version is not locked" >&2
    exit 1
  }
  grep -Fq "$name" "$notices" || {
    echo "error: notices omit $name" >&2
    exit 1
  }
  grep -Fq "$version" "$notices" || {
    echo "error: notices omit version $version" >&2
    exit 1
  }
}

require_locked grep-matcher 0.1.9
require_locked grep-regex 0.1.14
require_locked grep-searcher 0.1.17
require_locked dekopon-provider-sdk 0.11.1
require_locked serde 1.0.229
require_locked serde_json 1.0.151

for phrase in \
  'Cargo.lock is the authority' \
  'Unlicense OR MIT' \
  'MmapChoice::never()' \
  'disables BOM sniffing' \
  'zero imports' \
  'only in native development/test'; do
  grep -Fq "$phrase" "$notices" || {
    echo "error: notices omit required statement: $phrase" >&2
    exit 1
  }
done
printf 'third-party notices cover the exact shipped dependency surface\n'
