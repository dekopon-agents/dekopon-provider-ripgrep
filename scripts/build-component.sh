#!/usr/bin/env bash
# Build the deterministic release component with this checkout's ordinary Cargo target.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
component=${1:-"$root/ripgrep-provider.wasm"}
core="$root/target/wasm32-unknown-unknown/release/dekopon_ripgrep_provider.wasm"
rust_toolchain=1.97.0
required_rustc='rustc 1.97.0 (2d8144b78 2026-07-07)'
required_wasm_tools='wasm-tools 1.236.1'

# shellcheck source=lib-sha256.sh
# The source path is rooted above, not relative to the caller.
# shellcheck disable=SC1091
source "$root/scripts/lib-sha256.sh"

[[ -z "${CARGO_TARGET_DIR-}" ]] || {
  echo "error: CARGO_TARGET_DIR must be unset; use this checkout's ordinary target/" >&2
  exit 1
}
[[ "$(rustup run "$rust_toolchain" rustc --version)" == "$required_rustc" ]] || {
  echo "error: expected $required_rustc" >&2
  exit 1
}
[[ "$(wasm-tools --version)" == "$required_wasm_tools" ]] || {
  echo "error: expected $required_wasm_tools" >&2
  exit 1
}
if git -C "$root" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  [[ -z "$(git -C "$root" ls-files '*.wasm')" ]] || {
    echo "error: generated Wasm must never be tracked" >&2
    exit 1
  }
fi

cargo_home=${CARGO_HOME:-"$HOME/.cargo"}
cargo_home=$(cd "$cargo_home" && pwd -P)
sysroot=$(rustup run "$rust_toolchain" rustc --print sysroot)
sysroot=$(cd "$sysroot" && pwd -P)
rustflags=(
  "--remap-path-prefix=$root=/dekopon/source"
  "--remap-path-prefix=$cargo_home=/dekopon/cargo"
  "--remap-path-prefix=$sysroot=/dekopon/rust/$rust_toolchain"
  '--cfg=dekopon_provider_repro_v1'
  '--check-cfg=cfg(dekopon_provider_repro_v1)'
)
encoded_rustflags=$(printf '%s\x1f' "${rustflags[@]}")
encoded_rustflags=${encoded_rustflags%$'\x1f'}

mkdir -p "$(dirname "$component")"
SOURCE_DATE_EPOCH=0 \
LANG=C.UTF-8 \
LC_ALL=C \
CARGO_TERM_COLOR=never \
CARGO_ENCODED_RUSTFLAGS="$encoded_rustflags" \
  cargo +"$rust_toolchain" rustc \
    --locked \
    --manifest-path "$root/Cargo.toml" \
    --package dekopon-ripgrep-provider \
    --target wasm32-unknown-unknown \
    --release \
    -- \
    -C metadata=dekopon-ripgrep-provider-0.1.0-repro-v1 \
    -C extra-filename=

test -s "$core"
wasm-tools validate "$core"
"$root/scripts/assert-zero-core-imports.sh" "$core"
wasm-tools component new "$core" -o "$component"
"$root/scripts/embed-license-bundle.py" embed "$component" "$root"
wasm-tools validate "$component"
"$root/scripts/assert-zero-core-imports.sh" "$component"
"$root/scripts/embed-license-bundle.py" verify "$component" "$root"

size=$(wc -c <"$component" | tr -d ' ')
((size <= 2000000)) || {
  echo "error: release component is $size bytes; limit is 2000000" >&2
  exit 1
}
for local_path in "$root" "$cargo_home" "$sysroot"; do
  if LC_ALL=C grep -aF -- "$local_path" "$component" >/dev/null; then
    echo "error: component embeds local build path: $local_path" >&2
    exit 1
  fi
done

checksum="${component}.sha256"
write_sha256 "$component" "$checksum"
check_sha256 "$checksum"
printf 'generated %s (%s bytes, sha256 %s)\n' \
  "$component" "$size" "$(sha256_file "$component")"
