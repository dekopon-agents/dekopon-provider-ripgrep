#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
directory=${1:-"$root/dist"}
# shellcheck source=lib-sha256.sh
# shellcheck disable=SC1091
source "$root/scripts/lib-sha256.sh"

[[ -d "$directory" ]] || { echo "error: missing release asset directory" >&2; exit 1; }
mapfile_supported=false
if builtin help mapfile >/dev/null 2>&1; then
  mapfile_supported=true
fi
if [[ "$mapfile_supported" == true ]]; then
  mapfile -t names < <(find "$directory" -mindepth 1 -maxdepth 1 -type f -exec basename {} \; | LC_ALL=C sort)
else
  names=()
  while IFS= read -r name; do names+=("$name"); done < <(
    find "$directory" -mindepth 1 -maxdepth 1 -type f -exec basename {} \; | LC_ALL=C sort
  )
fi
[[ ${#names[@]} -eq 2 ]] || { echo "error: release must contain exactly two files" >&2; exit 1; }
[[ ${names[0]} == ripgrep-provider.wasm && ${names[1]} == ripgrep-provider.wasm.sha256 ]] || {
  echo "error: unexpected release asset names" >&2
  printf '%s\n' "${names[@]}" >&2
  exit 1
}
[[ -z "$(find "$directory" -mindepth 1 -maxdepth 1 ! -type f -print -quit)" ]] || {
  echo "error: release directory contains a non-regular entry" >&2
  exit 1
}
check_sha256 "$directory/ripgrep-provider.wasm.sha256"
wasm-tools validate "$directory/ripgrep-provider.wasm"
"$root/scripts/assert-zero-core-imports.sh" "$directory/ripgrep-provider.wasm"
"$root/scripts/embed-license-bundle.py" verify "$directory/ripgrep-provider.wasm" "$root"
size=$(wc -c <"$directory/ripgrep-provider.wasm" | tr -d ' ')
((size <= 2000000)) || { echo "error: release component exceeds 2,000,000 bytes" >&2; exit 1; }
printf 'verified exact two-file release asset set (%s bytes)\n' "$size"
