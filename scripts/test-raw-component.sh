#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
component=${1:-"$root/ripgrep-provider.wasm"}
[[ -f "$component" ]] || { echo "error: missing $component" >&2; exit 1; }
[[ "$(wasmtime --version)" == "wasmtime 48.0.0" ]] || {
  echo "error: wasmtime 48.0.0 is required" >&2
  exit 1
}

raw_invoke() {
  local capability=$1 raw=$2 quoted_capability quoted_raw
  quoted_capability=$(jq -rn --arg value "$capability" '$value | tojson')
  quoted_raw=$(jq -rn --arg value "$raw" '$value | tojson')
  wasmtime run --invoke "invoke($quoted_capability,$quoted_raw)" "$component" | jq -r .
}

description=$(wasmtime run --invoke 'describe()' "$component" | jq -r .)
jq -e '
  .id == "ripgrep" and .commandWords == [] and
  [.capabilities[].id] == ["ripgrep.search"]
' <<<"$description" >/dev/null

valid='{"documents":[{"path":"raw","text":"hit\n"}],"pattern":"hit"}'
raw_invoke ripgrep.search "$valid" | jq -e '
  .outcome == "succeeded" and .output.selected_count == 1
' >/dev/null

# Invalid capability plus malformed/trailing JSON proves the SDK parser rejects before Provider::invoke:
# if invoke were reached, the provider's first branch would return unsupported-capability instead.
for raw in \
  '{"documents":[' \
  '{"documents":[{"path":"raw","text":"hit"}],"pattern":"hit"} trailing'; do
  response=$(raw_invoke ripgrep.other "$raw")
  jq -e '.outcome == "failed" and .error.code == "invalid-input"' <<<"$response" >/dev/null
done

# The SDK parses input-json directly into serde_json::Value. serde_json deliberately retains the
# last value for a repeated object name, so the provider sees pattern="hit" here.
duplicate='{"documents":[{"path":"duplicate","text":"hit\n"}],"pattern":"miss","pattern":"hit"}'
response=$(raw_invoke ripgrep.search "$duplicate")
jq -e '
  .outcome == "succeeded" and .output.selected_count == 1 and
  .output.results[0].text == "hit\n"
' <<<"$response" >/dev/null

unknown='{"documents":[{"path":"raw","text":"hit"}],"pattern":"hit","unknown":true}'
raw_invoke ripgrep.search "$unknown" | jq -e '
  .outcome == "failed" and .error.code == "invalid-input"
' >/dev/null

printf 'raw component SDK-boundary tests passed\n'
