#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
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
require_locked encoding_rs 0.8.35
require_locked foldhash 0.1.5
require_locked unicode-ident 1.0.24
require_locked dekopon-provider-sdk 0.11.1
require_locked serde 1.0.229
require_locked serde_json 1.0.151

for phrase in \
  'Cargo.lock is the authority' \
  '(MIT OR Apache-2.0) AND BSD-3-Clause' \
  'encoding_rs 0.8.35/LICENSE-WHATWG' \
  'Copyright © WHATWG (Apple, Google, Mozilla, Microsoft).' \
  'Redistributions in binary form must reproduce' \
  'Unlicense OR MIT' \
  'MmapChoice::never()' \
  'disables BOM sniffing' \
  'zero imports' \
  'only in native development/test' \
  'dekopon.third-party-notices'; do
  grep -Fq "$phrase" "$notices" || {
    echo "error: notices omit required statement: $phrase" >&2
    exit 1
  }
done

whatwg="$root/licenses/encoding_rs-0.8.35-LICENSE-WHATWG"
[[ -f "$whatwg" ]] || { echo "error: missing canonical WHATWG notice" >&2; exit 1; }
expected_whatwg_sha=838118388fe5c2e7f1dbbaeed13e1c7f3ebf88be91319c7c1d77c18e987d1a50
actual_whatwg_sha=$(shasum -a 256 "$whatwg" | awk '{print $1}')
[[ "$actual_whatwg_sha" == "$expected_whatwg_sha" ]] || {
  echo "error: canonical encoding_rs LICENSE-WHATWG changed" >&2
  exit 1
}

bundle=$(mktemp "${TMPDIR:-/tmp}/ripgrep-license-bundle.XXXXXX")
trap 'rm -f "$bundle"' EXIT
"$root/scripts/embed-license-bundle.py" write "$bundle" "$root"
for source in "$root/THIRD_PARTY_NOTICES.md" "$whatwg" "$root/LICENSE-MIT" "$root/LICENSE-APACHE"; do
  python3 - "$bundle" "$source" <<'PY'
import pathlib
import sys
bundle = pathlib.Path(sys.argv[1]).read_bytes()
source = pathlib.Path(sys.argv[2]).read_bytes()
if bundle.count(source) != 1:
    raise SystemExit(f"error: bundle does not contain {sys.argv[2]} exactly once")
PY
done
printf 'third-party notices and exact embedded distribution-license bundle cover the shipped graph\n'
