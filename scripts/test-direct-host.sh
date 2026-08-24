#!/usr/bin/env bash
# shellcheck disable=SC2154 # root/component are assigned by sourced lib-component.sh.
set -euo pipefail

# shellcheck source=lib-component.sh
# shellcheck disable=SC1091
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)/lib-component.sh" "$@"
[[ -f "$component" ]] || { echo "error: missing $component" >&2; exit 1; }
[[ "$(dekopon-run --version)" == "dekopon-run 0.11.1" ]] || {
  echo "error: dekopon-run 0.11.1 is required" >&2
  exit 1
}

inline=$(invoke --input '{"documents":[{"path":"inline.txt","text":"alpha\nbeta\nalpha\n"}],"pattern":"alpha","max_results":2}')
jq -e '
  .provider == "ripgrep" and .capability == "ripgrep.search" and
  .output.selected_count == 2 and .output.truncated == false and
  [.output.results[].line_start] == [1, 3]
' <<<"$inline" >/dev/null

stdin=$(printf '%s' \
  '{"documents":[{"path":"stdin.txt","text":"One\ntwo\n"}],"pattern":"one","case":"insensitive"}' |
  invoke --input-file -)
jq -e '
  .output.selected_count == 1 and .output.results[0].path == "stdin.txt" and
  .output.results[0].text == "One\n"
' <<<"$stdin" >/dev/null

if invoke --input '{"documents":[{"path":"bad","text":"x"}],"pattern":"x","extra":true}' \
  >"$root/target/direct-invalid.out" 2>"$root/target/direct-invalid.err"; then
  echo "error: closed-schema violation unexpectedly succeeded" >&2
  exit 1
fi
grep -Eq 'invalid-input|closed ripgrep.search schema' "$root/target/direct-invalid.err"

escaped="$root/target/escaped-wire-input.json"
python3 - "$escaped" <<'PY'
import json
import pathlib
import sys
value = "\0" * 100_000
pathlib.Path(sys.argv[1]).write_text(json.dumps({
    "documents": [
        {"path": "one", "text": value},
        {"path": "two", "text": value},
    ],
    "pattern": "x",
}), encoding="utf-8")
PY
[[ $(wc -c <"$escaped" | tr -d ' ') -gt 1048576 ]]
if invoke --input-file "$escaped" \
  >"$root/target/direct-oversize.out" 2>"$root/target/direct-oversize.err"; then
  echo "error: over-1-MiB escaped wire input unexpectedly succeeded" >&2
  exit 1
fi
grep -Eqi 'input.*(large|maximum|1048576)|exceeds.*input' "$root/target/direct-oversize.err"

printf 'direct inline, stdin, semantic, and wire-boundary host tests passed\n'
