#!/usr/bin/env bash
set -euo pipefail

file=${1:?usage: assert-zero-core-imports.sh <core-or-component.wasm>}
temporary=$(mktemp "${TMPDIR:-/tmp}/ripgrep-provider-skeleton.XXXXXX")
trap 'rm -f "$temporary"' EXIT
wasm-tools print --skeleton "$file" >"$temporary"
if grep -Eq '^ *\(import |^ *\(component[[:space:]]+.*\(import ' "$temporary"; then
  echo "error: $file or a nested core module contains an import" >&2
  grep -En '\(import ' "$temporary" >&2 || true
  exit 1
fi
