# Dekopon ripgrep provider

A standalone, import-free WebAssembly component exposing one Low-risk, read-only, idempotent
capability: `ripgrep.search`. It uses ripgrep's official `grep-matcher 0.1.9`, `grep-regex 0.1.14`,
and `grep-searcher 0.1.17` crates to search **only UTF-8 virtual documents supplied by the
caller**.

A document `path` is an opaque output label. The provider never opens it. The component has zero
imports and no filesystem, network, storage, HTTP, subprocess, clock, random, WASI, or JavaScript
authority.

## Capability

```json
{
  "documents": [
    {"path": "src/lib.rs", "text": "fn alpha() {}\nfn beta() {}\n"}
  ],
  "pattern": "fn\\s+([a-z]+)",
  "mode": "regex",
  "case": "sensitive",
  "word": false,
  "line": false,
  "multiline": false,
  "invert": false,
  "context": {"before": 0, "after": 0},
  "max_results": 100
}
```

`documents` and `pattern` are required. Every object is closed. Optional fields use the defaults
shown above; if `context` is supplied, both `before` and `after` are required. Unknown fields,
missing fields, `null`, wrong types, fractional/negative counts, and out-of-range values are
rejected.

| Option | Meaning |
|---|---|
| `mode` | `regex` uses Rust regex syntax; `fixed` treats the entire pattern as one literal. |
| `case` | `sensitive`, Unicode `insensitive`, or ripgrep-style `smart` case. |
| `word` | Require ripgrep word boundaries. Incompatible with `line`. |
| `line` | Require the whole LF-delimited line. Incompatible with `word` and `multiline`. |
| `multiline` | Permit explicit matches across LF. `.` still excludes LF unless the pattern enables `s`. Incompatible with `line` and `invert`. |
| `invert` | Select nonmatching lines. Invert-selected records have no submatches. |
| `context` | Return 0–8 complete lines before and after selected records. |
| `max_results` | Return 1–1,000 selected records (default 100); context does not count. |

Look-around and backreferences are not part of Rust regex syntax. PCRE2 is not linked. Inline Rust
regex flags remain available; byte-mode expressions such as `(?-u:\xA9)` can produce submatch
offsets inside a UTF-8 scalar.

### Output

```json
{
  "results": [
    {
      "kind": "match",
      "path": "src/lib.rs",
      "text": "fn alpha() {}\n",
      "byte_start": 0,
      "byte_end": 14,
      "line_start": 1,
      "line_end": 1,
      "submatches": [{"byte_start": 0, "byte_end": 8}],
      "submatches_truncated": false
    }
  ],
  "selected_count": 1,
  "truncated": false,
  "truncation_reasons": []
}
```

Offsets are zero-based, half-open byte offsets into that document's original `text`. Lines are
one-based and `line_end` is inclusive. A result's `text` is the exact selected line or multiline
block, including an existing LF. LF is the only line terminator: CR and BOMs are ordinary preserved
bytes, and a final line need not end in LF.

Results preserve document order and then byte order. A selected line/block appears once even when
it has many occurrences; `submatches` are non-overlapping overall occurrences, not capture groups.
Context is deduplicated. Selected records take priority over context, then `context_before` takes
priority over `context_after`. `selected_count` counts returned selected records.

Each limit is probed once beyond its boundary. Only complete deterministic prefixes are returned;
context is never emitted without a retained selected record. Truncation reasons always use this
order: `max_results`, `max_output_bytes`, `max_submatches`.

## Command word: `rg`

The provider contributes `rg` to Dekopon's sandboxed shell. It is ripgrep's command line over the
text piped into it — the shell has no filesystem, and the component could not open one anyway:

```console
$ cat notes | rg -i -C 1 todo
$ rg --help
```

```text
rg [OPTIONS] <PATTERN> [PATH]
```

`PATH` is never opened; it names the piped text in the results and defaults to `<stdin>` (`-` means
the same). A well-formed argv becomes one `ripgrep.search` proposal, authorized exactly like a
direct call, carrying only the members a flag set:

| Flag | Input member |
|---|---|
| `-F`, `--fixed-strings` | `"mode": "fixed"` |
| `-i`, `--ignore-case` | `"case": "insensitive"` |
| `-S`, `--smart-case` | `"case": "smart"` |
| `-s`, `--case-sensitive` | `"case": "sensitive"` |
| `-w`, `--word-regexp` | `"word": true` |
| `-x`, `--line-regexp` | `"line": true` |
| `-U`, `--multiline` | `"multiline": true` |
| `-v`, `--invert-match` | `"invert": true` |
| `-A`, `--after-context NUM` | `"context": {"before": …, "after": NUM}` |
| `-B`, `--before-context NUM` | `"context": {"before": NUM, "after": …}` |
| `-C`, `--context NUM` | both sides; `-A`/`-B` override their side |
| `-m`, `--max-count NUM` | `"max_results": NUM` |

As in ripgrep, the last of `-i`/`-S`/`-s` wins and a repeated flag takes its last value. Context
counts are checked against 0–8 and `-m` against 1–1,000 before anything is proposed. `--help` and
`--version` render on stdout at status 0; a missing pattern, a bad count, or any other ripgrep flag
(`-g`, `--glob`, `-e`, `-r`, `--json`, `--max-filesize`, …) renders clap's usage error on stderr at
status 2. Nothing piped in is a usage decline. `--` ends the options, so `rg -- -foo` searches for
`-foo`.

## Run it

Obtain `ripgrep-provider.wasm` and `ripgrep-provider.wasm.sha256` from the immutable release, whose
bytes are the sole layer at `ghcr.io/dekopon-agents/provider-ripgrep:<version>`. No `latest` tag is
published. The Wasm embeds the complete distribution-license bundle in its
`dekopon.third-party-notices` custom section; the GitHub release documentation reproduces that
byte-exact bundle.

Verify the checksum, install the component in `dekopon-brokerd`, and grant `ripgrep.search`. The
broker — not the component — owns authorization and every host resource ceiling, so an authorized
proposal carries only the semantic input:

```console
$ sha256sum --check ripgrep-provider.wasm.sha256
```

```json
{
  "documents": [{"path": "virtual/log.txt", "text": "begin\ndetail\nend\n"}],
  "pattern": "begin(?s:.*?)end",
  "multiline": true
}
```

To drive the component without a deployment, load it with `FakeBroker` from
[`dekopon-provider-sdk-testkit`](https://docs.rs/dekopon-provider-sdk-testkit): the same Wasmtime
host, the same limits, no policy. `tests/broker.rs` is that harness and
`./scripts/test-broker-testkit.sh` runs it against a freshly built component.

## Limits and JSON boundary

Host wire/runtime limits and provider decoded limits are deliberately separate. JSON escaping
counts against host wire bytes; decoded UTF-8 counts against provider limits.

### Host-enforced boundary

| Resource | Required/default bound |
|---|---:|
| Serialized invocation JSON | 1,048,576 bytes |
| Serialized response envelope | 1,048,576 bytes |
| Linear memory | 64 MiB |
| Release invocation fuel | 350,000,000 |
| Wall time | 30 seconds |

The fuel ceiling is a Wasmtime instruction-accounting bound, not a duration estimate. It is set to
350,000,000 because the component-host gate returns the maximum 786,432 decoded text bytes as six
complete records (about 770 KiB of compact response) under that budget and 64 MiB. Separate
regressions prove an 818-byte/409-record context request under 10,000,000 fuel and all 1,000 simple
selected records under 30,000,000 fuel. The larger release ceiling leaves measured headroom for
SDK JSON materialization near the provider output boundary; the 30-second deadline remains an
independent bound.

The provider neither re-encodes nor estimates raw input size. The broker serializes the semantic
input and rejects an invocation over 1 MiB before entering the component. Thus highly
escaped JSON can exceed the wire limit while its decoded strings remain below provider limits.

The SDK parses the complete WIT `input-json` string into `serde_json::Value` before calling the
provider. Malformed JSON and trailing non-whitespace never reach `Provider::invoke`. Duplicate
object names **cannot be rejected at that boundary**: `serde_json` retains the last value for a
repeated name. The provider validates the resulting semantic object using closed
`deny_unknown_fields` models. Callers should never send duplicate names.

### Provider-enforced decoded/resource boundary

| Resource | Limit |
|---|---:|
| Documents | 1–16 |
| Path label | 1–256 UTF-8 bytes |
| Text per document | 131,072 decoded UTF-8 bytes |
| Aggregate document text | 786,432 decoded UTF-8 bytes |
| Pattern | 1–4,096 decoded UTF-8 bytes |
| Context | 0–8 lines per side |
| `max_results` | 1–1,000 |
| Submatches per selected result | 64 |
| Regex nesting | 64 |
| Compiled regex program | 4 MiB |
| DFA cache | 2 MiB |
| Provider success envelope | 1,000,000 serialized bytes |
| Release component | 2,000,000 bytes |

Path labels must contain nonempty relative `/`-separated components and be exact-byte unique.
Empty, `.`, `..`, control-containing, backslash, absolute, drive-prefixed, UNC-like, and empty
components are rejected.

## This is not the `rg` CLI

The `rg` word borrows ripgrep's spellings, not its program. The provider intentionally does **not**
implement filesystem discovery, path lookup, directory walking, globs, file types, ignore files,
hidden-file rules, encodings/transcoding, BOM stripping, binary detection, mmap, network access,
replacement, archive search, config files, PCRE2, printed `rg` output, or full `rg` command-line
compatibility. It imports neither `grep-printer`, `ignore`, nor `globset`. Supply already selected
UTF-8 text and consume structured JSON results.

## Build and validate

Generated Wasm is ignored and must never be committed. Builds use each checkout's ordinary
`target/` and the machine's global Cargo/sccache configuration—no shared target directory or
project-local compiler cache override.

```console
rustup toolchain install 1.98.1 --profile minimal --component clippy --component rustfmt
rustup target add wasm32-unknown-unknown --toolchain 1.98.1
cargo +1.98.1 install wasm-tools --version 1.259.0 --locked
cargo +1.98.1 install wasmtime-cli --version 48.0.2 --locked
./scripts/validate.sh
./scripts/reproducible-build.sh
```

`validate.sh` runs formatting, warnings-denied clippy, native/adversarial tests, Wasm target
checks, license/source policy, component validation, import/WIT/size inspection, raw SDK boundary
tests under the Wasmtime CLI, and the FakeBroker component-host gate covering concurrent storage-free
invocation, the host wire bound, and every fuel and resource limit. See
[`SECURITY.md`](SECURITY.md) and [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).

## License

Project-authored source is available under MIT OR Apache-2.0. Because the component contains
WHATWG-derived `encoding_rs` data, its binary distribution expression is
`(MIT OR Apache-2.0) AND BSD-3-Clause`. See `LICENSE-MIT`, `LICENSE-APACHE`, and
`THIRD_PARTY_NOTICES.md`; the build and release gates verify the exact embedded notice bundle.
