#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
component=${1:-"$root/ripgrep-provider.wasm"}
DEKOPON_RIPGREP_COMPONENT="$component" \
  cargo +1.98.1 test --locked --manifest-path "$root/Cargo.toml" --test broker -- --nocapture
