# Dekopon ripgrep provider

`ripgrep.search` is a Low-risk, read-only, idempotent typed capability. It searches **only piped stdin**, using the Rust `grep-regex` and `grep-searcher` reader interface. It neither opens paths nor has filesystem, network, storage, or subprocess grants. Matches and requested context are emitted as human-readable text lines to stdout as the reader advances; no JSON results or virtual documents are accepted.

The closed capability input is:

```json
{"pattern":"needle","mode":"regex","case":"sensitive","word":false,"line":false,"multiline":false,"invert":false,"context":{"before":0,"after":0},"max_results":100}
```

Only `pattern` is required. `mode` is `regex` or `fixed`; `case` is `sensitive`, `insensitive`, or `smart`. `word` and `line` cannot both be set; `multiline` cannot combine with `line` or `invert`. The pattern is 1–4096 UTF-8 bytes; context counts are 0–8 each; `max_results` is 1–1000 selected lines. Rust regex syntax does not include PCRE2 look-around or backreferences. The host independently bounds wire size, fuel, memory, time and output. Reader search retains bounded working memory (including a 1 MiB line heap limit), not a whole stdin buffer. For unmatched final lines, stdout adds a newline.

`rg` maps supported ripgrep-style flags (`-F`, `-i`, `-S`, `-s`, `-w`, `-x`, `-U`, `-v`, `-A`, `-B`, `-C`, `-m`) to the same typed input; it accepts no path or filesystem flag. `--help` and `--version` render at status 0. A match exits **0**, valid no-match **1**, and absent or zero-byte piped stdin and other usage failures **2**. A closed downstream reader exits **141**. In a pipeline, use `cat big | rg needle | head -5`; only the pipeline's stdin is searchable.

## Build and validate

The component is built with `/path/to/provider-workflows/build.sh` (inspect that script before running it). Run `cargo fmt --all --check`, `cargo check --locked --all-targets --all-features`, `cargo check --locked --target wasm32-unknown-unknown`, `cargo clippy --locked --all-targets --all-features -- -D warnings`, `cargo test --locked --all-features`, and `cargo deny --all-features check bans licenses sources advisories`. Validate and decode the freshly built Wasm using `wasm-tools`, and point `DEKOPON_PROVIDER_COMPONENT` at it for the real-component testkit conformance and stream tests. The testkit captures status/stdout/stderr without any storage grant. The SDK and testkit remain Git-pinned to the pushed S1b integration head until a separately reviewed main repin.

Project-authored source is MIT OR Apache-2.0 licensed. No release or deployment is performed by these tests.
