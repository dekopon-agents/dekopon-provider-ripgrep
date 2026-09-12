#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
destination=${2:-"$root/dist"}
# The crate is the single source of the version; an argument may only agree with it.
crate_version=$(cargo metadata --locked --no-deps --format-version 1 \
  --manifest-path "$root/Cargo.toml" |
  jq -er '.packages[] | select(.name == "dekopon-ripgrep-provider") | .version')
version=${1:-$crate_version}
[[ "$version" == "$crate_version" ]] || {
  echo "error: requested v$version but this checkout is v$crate_version" >&2
  exit 1
}
case "$destination" in
  "$root/dist"|"$root"/dist/*) ;;
  *) echo "error: release destination must be inside $root/dist" >&2; exit 1 ;;
esac
# shellcheck source=lib-sha256.sh
# shellcheck disable=SC1091
source "$root/scripts/lib-sha256.sh"

[[ -f "$root/ripgrep-provider.wasm" && -f "$root/ripgrep-provider.wasm.sha256" ]] || {
  echo "error: build the component first" >&2
  exit 1
}
check_sha256 "$root/ripgrep-provider.wasm.sha256"
rm -rf "$destination"
mkdir -p "$destination"
cp "$root/ripgrep-provider.wasm" "$root/ripgrep-provider.wasm.sha256" "$destination/"
"$root/scripts/verify-release-assets.sh" "$destination"
printf 'prepared exactly two v%s release assets in %s\n' "$version" "$destination"
