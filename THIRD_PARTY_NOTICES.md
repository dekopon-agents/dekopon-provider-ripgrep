# Third-party notices

`dekopon-ripgrep-provider` project source is licensed under **MIT OR Apache-2.0**. The release
component statically links permissively licensed Rust dependencies. `Cargo.lock is the authority`
for every exact transitive version and crates.io checksum; `cargo deny check licenses` is the
machine-enforced license inventory.

## Search implementation

The search implementation uses the official ripgrep library crates by Andrew Gallant and
contributors:

| Crate | Exact version | License |
|---|---:|---|
| `grep-matcher` | 0.1.9 | Unlicense OR MIT |
| `grep-regex` | 0.1.14 | Unlicense OR MIT |
| `grep-searcher` | 0.1.17 | Unlicense OR MIT |

Their upstream repository is <https://github.com/BurntSushi/ripgrep>. The graph also contains
`aho-corasick`, `memchr`, `regex-automata`, and `regex-syntax` from the Rust regex ecosystem under
MIT/Apache-2.0 or Unlicense/MIT terms.

`grep-searcher` unconditionally depends on `encoding_rs`, `encoding_rs_io`, and `memmap2`. Those
packages are therefore present in the linked graph, but v0.1.0 explicitly configures no encoding,
disables BOM sniffing, selects `MmapChoice::never()`, and calls only `search_slice` over already
decoded caller-owned UTF-8. No file, mmap, or transcoding API is invoked. `encoding_rs` includes
BSD-3-Clause material in addition to Apache-2.0 OR MIT.

## Dekopon component interface

The component uses `dekopon-provider-sdk 0.11.1`, with `dekopon-core 0.11.1` and
`dekopon-capability 0.11.1`, under MIT OR Apache-2.0. Its generated component bindings use
`wit-bindgen 0.44.0` and the Wasm/WIT 0.236.1 toolchain crates, under Apache-2.0 WITH
LLVM-exception OR Apache-2.0 OR MIT. The release component has zero imports.

`dekopon-provider-sdk-testkit 0.11.1`, Tokio, Wasmtime, HTTP-host, and storage-host packages occur
only in native development/test resolution. They are not linked into `ripgrep-provider.wasm`; the
component-target tree and decoded WIT gates enforce that distinction.

## Serialization and supporting crates

The component directly pins `serde 1.0.229` and `serde_json 1.0.151` under MIT OR Apache-2.0.
Other exact transitive packages are permissively licensed under the allowlist in `deny.toml`,
including MIT, Apache-2.0, BSD-3-Clause, Unicode-3.0, Unlicense, and Zlib terms.

The project distributes `LICENSE-MIT` and `LICENSE-APACHE`. Package-specific copyright and license
files remain available in the exact crates.io sources identified by `Cargo.lock`. To reproduce the
inventory:

```console
cargo deny list
cargo deny check licenses advisories bans sources
cargo tree --locked --target wasm32-unknown-unknown --edges normal,build
./scripts/check-third-party-notices.sh Cargo.lock THIRD_PARTY_NOTICES.md
```
