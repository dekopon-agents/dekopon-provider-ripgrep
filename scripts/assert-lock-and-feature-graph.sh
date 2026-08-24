#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
lock=${1:-"$root/Cargo.lock"}
[[ -f "$lock" ]] || { echo "error: missing Cargo.lock" >&2; exit 1; }
mkdir -p "$root/target/validation"
metadata="$root/target/validation/metadata.json"
tree="$root/target/validation/component-dependencies.txt"
cargo metadata --locked --manifest-path "$root/Cargo.toml" --format-version 1 >"$metadata"

assert_exact() {
  local name=$1 version=$2
  jq -e --arg name "$name" --arg version "$version" '
    [.packages[] | select(.name == $name)] as $matches |
    ($matches | length) == 1 and $matches[0].version == $version
  ' "$metadata" >/dev/null || {
    echo "error: expected exactly $name $version" >&2
    exit 1
  }
}
assert_exact dekopon-provider-sdk 0.11.1
assert_exact dekopon-provider-sdk-testkit 0.11.1
assert_exact grep-matcher 0.1.9
assert_exact grep-regex 0.1.14
assert_exact grep-searcher 0.1.17
assert_exact serde 1.0.229
assert_exact serde_json 1.0.151
assert_exact tokio 1.49.0

cargo tree --locked --manifest-path "$root/Cargo.toml" \
  --target wasm32-unknown-unknown --edges normal,build --prefix none --format '{p}' |
  LC_ALL=C sort -u >"$tree"
for required in \
  'dekopon-provider-sdk v0.11.1' \
  'grep-matcher v0.1.9' \
  'grep-regex v0.1.14' \
  'grep-searcher v0.1.17' \
  'serde v1.0.229' \
  'serde_json v1.0.151'; do
  grep -Fxq "$required" "$tree" || {
    echo "error: component graph omits $required" >&2
    exit 1
  }
done

if grep -Eqi '^(grep-printer|globset|ignore|pcre2|dekopon-provider-(http|storage)|js-sys|wasm-bindgen|wasi([^-[:alnum:]]|$)|wasip[123]?|reqwest|tokio)([[:space:]]|$)' "$tree"; then
  echo "error: forbidden package reached the component graph" >&2
  grep -Ein '^(grep-printer|globset|ignore|pcre2|dekopon-provider-(http|storage)|js-sys|wasm-bindgen|wasi([^-[:alnum:]]|$)|wasip[123]?|reqwest|tokio)([[:space:]]|$)' "$tree" >&2
  exit 1
fi

# grep-searcher carries encoding/mmap implementations as unconditional transitive code. The only
# permitted provider call surface is the explicitly configured in-memory slice path.
if rg -n 'search_(path|file|reader)|MmapChoice::auto|Encoding::new|std::fs|std::process|std::net|reqwest|Command::new' "$root/src"; then
  echo "error: forbidden ambient I/O or alternate search API appears in provider source" >&2
  exit 1
fi
rg -n 'search_slice' "$root/src/search.rs" >/dev/null
rg -n 'MmapChoice::never' "$root/src/search.rs" >/dev/null
rg -n 'bom_sniffing\(false\)' "$root/src/search.rs" >/dev/null
rg -n 'BinaryDetection::none' "$root/src/search.rs" >/dev/null
rg -n 'encoding\(None\)' "$root/src/search.rs" >/dev/null

printf 'lockfile pins and import-free component dependency graph passed\n'
