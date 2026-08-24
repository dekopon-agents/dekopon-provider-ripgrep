#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
component=${1:-"$root/ripgrep-provider.wasm"}
core="$root/target/wasm32-unknown-unknown/release/dekopon_ripgrep_provider.wasm"
# shellcheck source=lib-sha256.sh
# shellcheck disable=SC1091
source "$root/scripts/lib-sha256.sh"

[[ -f "$component" && -f "${component}.sha256" && -f "$core" ]] || {
  echo "error: build the component first" >&2
  exit 1
}
wasm-tools validate "$core"
wasm-tools validate "$component"
"$root/scripts/assert-zero-core-imports.sh" "$core"
"$root/scripts/assert-zero-core-imports.sh" "$component"
check_sha256 "${component}.sha256"

size=$(wc -c <"$component" | tr -d ' ')
((size <= 2000000)) || { echo "error: component exceeds 2,000,000 bytes" >&2; exit 1; }
mkdir -p "$root/target/validation"
wasm-tools component wit "$component" >"$root/target/validation/component.wit"
wasm-tools component wit -j "$component" >"$root/target/validation/component-wit.json"
"$root/scripts/assert-provider-wit.sh" "$root/target/validation/component-wit.json"
if grep -Eqi 'wasi:|resolve-command|dekopon:(http|storage)' \
  "$root/target/validation/component.wit"; then
  echo "error: forbidden component import or export" >&2
  exit 1
fi

metadata=$(cargo metadata --locked --manifest-path "$root/Cargo.toml" --format-version 1)
sdk_manifest=$(jq -er '
  .packages[] |
  select(.name == "dekopon-provider-sdk" and .version == "0.11.1") |
  .manifest_path
' <<<"$metadata")
cmp "$(dirname "$sdk_manifest")/wit/provider.wit" "$root/wit/provider.wit"

printf 'component valid: zero imports, describe/invoke only, %s bytes, sha256 %s\n' \
  "$size" "$(sha256_file "$component")"
