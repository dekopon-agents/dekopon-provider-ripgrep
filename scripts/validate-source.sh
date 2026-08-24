#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$root"
[[ -z "${CARGO_TARGET_DIR-}" ]] || {
  echo "error: CARGO_TARGET_DIR must be unset" >&2
  exit 1
}
[[ "$(cargo deny --version)" == "cargo-deny 0.20.2" ]] || {
  echo "error: cargo-deny 0.20.2 is required" >&2
  exit 1
}

cargo +1.97.0 fmt --all -- --check
cargo +1.97.0 clippy --locked --all-targets -- -D warnings
cargo +1.97.0 test --locked --all-targets
cargo +1.89.0 check --locked --all-targets
cargo +1.97.0 check --locked --target wasm32-unknown-unknown
cargo +1.97.0 clippy --locked --target wasm32-unknown-unknown --lib -- -D warnings
./scripts/assert-lock-and-feature-graph.sh Cargo.lock
cargo deny check licenses advisories bans sources
./scripts/check-third-party-notices.sh Cargo.lock THIRD_PARTY_NOTICES.md

test -z "$(git ls-files '*.wasm')"
git diff --check
bash -n scripts/*.sh
shellcheck scripts/*.sh
python3 -m py_compile scripts/*.py
ruby -e 'require "yaml"; ARGV.each { |path| YAML.safe_load_file(path, aliases: true) }' \
  .github/workflows/*.yml
if command -v actionlint >/dev/null 2>&1; then
  actionlint -color
else
  echo "error: actionlint 1.7.12 is required" >&2
  exit 1
fi
./scripts/validate-workflows.sh
printf 'source, native, MSRV, target, lock, license, shell, YAML, action, and diff gates passed\n'
