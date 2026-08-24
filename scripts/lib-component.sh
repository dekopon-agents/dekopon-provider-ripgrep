#!/usr/bin/env bash
# shellcheck shell=bash
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
component=${1:-"$root/ripgrep-provider.wasm"}
cache="$root/target/dekopon-run-compile-cache"
mkdir -p "$cache"

invoke() {
  dekopon-run invoke \
    --provider "$component" \
    --compile-cache "$cache" \
    --max-memory-bytes 67108864 \
    --max-input-bytes 1048576 \
    --max-output-bytes 1048576 \
    --fuel 10000000 \
    --timeout-ms 30000 \
    ripgrep.search "$@"
}
