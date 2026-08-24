#!/usr/bin/env bash
# shellcheck shell=bash
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
component=${1:-"$root/ripgrep-provider.wasm"}
cache="$root/target/dekopon-run-compile-cache"
release_fuel=350000000
mkdir -p "$cache"

invoke_with_fuel() {
  local fuel=$1
  shift
  dekopon-run invoke \
    --provider "$component" \
    --compile-cache "$cache" \
    --max-memory-bytes 67108864 \
    --max-input-bytes 1048576 \
    --max-output-bytes 1048576 \
    --fuel "$fuel" \
    --timeout-ms 30000 \
    ripgrep.search "$@"
}

invoke() {
  invoke_with_fuel "$release_fuel" "$@"
}
