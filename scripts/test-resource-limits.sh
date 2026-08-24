#!/usr/bin/env bash
# shellcheck disable=SC2154 # root/component are assigned by sourced lib-component.sh.
set -euo pipefail

# shellcheck source=lib-component.sh
# shellcheck disable=SC1091
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)/lib-component.sh" "$@"
[[ -f "$component" ]] || { echo "error: missing $component" >&2; exit 1; }

worst="$root/target/provider-limit-input.json"
multiline="$root/target/provider-multiline-limit-input.json"
python3 - "$worst" "$multiline" <<'PY'
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
PY
[[ $(wc -c <"$worst" | tr -d ' ') -lt 1048576 ]]

no_match=$(invoke --input-file "$worst")
jq -e '.output.selected_count == 0 and .output.results == []' <<<"$no_match" >/dev/null

block=$(invoke --input-file "$multiline")
jq -e '
  .output.selected_count == 1 and
  (.output.results[0].text | utf8bytelength) == 32768 and
  .output.results[0].byte_start == 0 and .output.results[0].byte_end == 32768
' <<<"$block" >/dev/null

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
dense_output=$(invoke --input-file "$dense")
jq -e '
  .output.selected_count == 1 and
  (.output.results[0].submatches | length) == 64 and
  .output.results[0].submatches_truncated == true and
  .output.truncation_reasons == ["max_submatches"]
' <<<"$dense_output" >/dev/null

printf 'maximum decoded scan plus selected multiline, fuel, memory, deadline, and dense-match gates passed\n'
