#!/usr/bin/env bash
# shellcheck disable=SC2154 # root/component/cache/release_fuel are assigned by sourced library.
set -euo pipefail

# shellcheck source=lib-component.sh
# shellcheck disable=SC1091
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)/lib-component.sh" "$@"
[[ -f "$component" ]] || { echo "error: missing $component" >&2; exit 1; }
[[ "$release_fuel" -eq 350000000 ]]

worst="$root/target/provider-limit-input.json"
multiline="$root/target/provider-multiline-limit-input.json"
context="$root/target/provider-context-fuel-input.json"
thousand="$root/target/provider-thousand-results-input.json"
wide="$root/target/provider-wide-output-input.json"
python3 - "$worst" "$multiline" "$context" "$thousand" "$wide" <<'PY'
import json
import pathlib
import sys

text = "a" * 131_071 + "\n"
documents = [{"path": f"limits/d{index}", "text": text} for index in range(6)]
pathlib.Path(sys.argv[1]).write_text(
    json.dumps({"documents": documents, "pattern": "z", "mode": "fixed"}),
    encoding="utf-8",
)
multiline = "a" + ("m" * 32_765) + "z\n"
pathlib.Path(sys.argv[2]).write_text(
    json.dumps({
        "documents": [{"path": "limits/multiline", "text": multiline}],
        "pattern": "(?s:a.*z)",
        "multiline": True,
        "max_results": 1,
    }),
    encoding="utf-8",
)

# 25 selected lines with disjoint eight-line context on each side: 409 output records from an
# 818-byte document. This is the review regression that previously exhausted 10,000,000 fuel.
lines = []
for selected in range(25):
    lines.append("m\n")
    if selected != 24:
        lines.extend("c\n" for _ in range(16))
context_text = "".join(lines)
assert len(context_text.encode()) == 818
pathlib.Path(sys.argv[3]).write_text(json.dumps({
    "documents": [{"path": "limits/context", "text": context_text}],
    "pattern": "m",
    "mode": "fixed",
    "context": {"before": 8, "after": 8},
    "max_results": 25,
}), encoding="utf-8")

pathlib.Path(sys.argv[4]).write_text(json.dumps({
    "documents": [{"path": "limits/thousand", "text": "x\n" * 1_000}],
    "pattern": "x",
    "mode": "fixed",
    "max_results": 1_000,
}), encoding="utf-8")

# Maximum decoded aggregate input returned as six complete selected records. This exercises a
# roughly 770 KiB compact response and is the rationale for the bounded release fuel ceiling.
pathlib.Path(sys.argv[5]).write_text(json.dumps({
    "documents": documents,
    "pattern": "a+",
    "max_results": 6,
}), encoding="utf-8")
PY
[[ $(wc -c <"$worst" | tr -d ' ') -lt 1048576 ]]
[[ $(wc -c <"$wide" | tr -d ' ') -lt 1048576 ]]

no_match="$root/target/provider-limit-output.json"
invoke --input-file "$worst" >"$no_match"
jq -e '.output.selected_count == 0 and .output.results == []' "$no_match" >/dev/null

block="$root/target/provider-multiline-limit-output.json"
invoke --input-file "$multiline" >"$block"
jq -e '
  .output.selected_count == 1 and
  (.output.results[0].text | utf8bytelength) == 32768 and
  .output.results[0].byte_start == 0 and .output.results[0].byte_end == 32768
' "$block" >/dev/null

context_output="$root/target/provider-context-fuel-output.json"
invoke_with_fuel 10000000 --input-file "$context" >"$context_output"
jq -e '
  .output.selected_count == 25 and
  (.output.results | length) == 409 and
  .output.truncated == false
' "$context_output" >/dev/null

thousand_output="$root/target/provider-thousand-results-output.json"
invoke_with_fuel 30000000 --input-file "$thousand" >"$thousand_output"
jq -e '
  .output.selected_count == 1000 and
  (.output.results | length) == 1000 and
  .output.truncated == false
' "$thousand_output" >/dev/null

wide_output="$root/target/provider-wide-output.json"
invoke --input-file "$wide" >"$wide_output"
jq -e '
  .output.selected_count == 6 and
  (.output.results | length) == 6 and
  ([.output.results[].text | utf8bytelength] | all(. == 131072)) and
  .output.truncated == false
' "$wide_output" >/dev/null
[[ $(jq -c . <"$wide_output" | wc -c | tr -d ' ') -lt 1048576 ]]

# Dense matching must stop submatch enumeration after observing the 65th occurrence.
dense="$root/target/provider-dense-limit-input.json"
python3 - "$dense" <<'PY'
import json
import pathlib
import sys
pathlib.Path(sys.argv[1]).write_text(json.dumps({
    "documents": [{"path": "limits/dense", "text": "a" * 32_768}],
    "pattern": "a",
    "max_results": 1,
}), encoding="utf-8")
PY
dense_output="$root/target/provider-dense-limit-output.json"
invoke --input-file "$dense" >"$dense_output"
jq -e '
  .output.selected_count == 1 and
  (.output.results[0].submatches | length) == 64 and
  .output.results[0].submatches_truncated == true and
  .output.truncation_reasons == ["max_submatches"]
' "$dense_output" >/dev/null

printf 'maximum scan/output, 10M context, 30M thousand-result, 350M release, memory, deadline, and dense-match gates passed\n'
