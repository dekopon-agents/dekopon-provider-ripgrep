# Security policy

## Reporting

Report suspected vulnerabilities privately through GitHub's security-advisory flow for
`dekopon-agents/dekopon-provider-ripgrep`. Do not include private document content in a public
issue. This repository supports the immutable v0.1.0 line; a fix is released as a new version and
never by replacing published v0.1.0 bytes.

## Authority boundary

`ripgrep.search` processes only caller-supplied `serde_json::Value` data. A document path is an
opaque label and is never dereferenced. Handwritten provider code performs no filesystem, path
lookup, directory walk, mmap, network, HTTP, storage, subprocess, environment, clock, random, WASI,
or JavaScript operation. The decoded component WIT has no imports and exports exactly `describe`
and `invoke`; `commandWords` is empty.

The security boundary is the validated component plus a correctly configured Dekopon host:

- the host enforces 1,048,576-byte serialized input and response limits, 64 MiB linear memory,
  350,000,000 fuel for release invocations, and a 30-second deadline;
- the provider enforces decoded document/pattern/path/count limits, closed semantic objects,
  regex nesting/program/cache limits, result/submatch limits, and a 1,000,000-byte success envelope;
- the broker separately authenticates callers, authorizes `ripgrep.search`, and constrains an
  invocation. The component itself grants nothing.

Compilation is outside invocation fuel/deadline accounting. A deployment should admit only a
reviewed component digest and keep its Wasmtime compilation cache owner-writable only.

## JSON boundary

`export_provider!` parses the entire WIT `input-json` string into `serde_json::Value` before
`Provider::invoke`. Malformed JSON and trailing non-whitespace are rejected by that SDK adapter.
The provider then deserializes closed `deny_unknown_fields` models and validates decoded bytes and
numeric ranges.

Duplicate object names cannot be detected after parsing to `Value`. `serde_json` retains the last
value. This is documented behavior, tested at the raw component boundary, and is not represented as
a provider guarantee. Producers must emit unique object names. The provider intentionally performs
no second raw input-size calculation; JSON escaping is a host wire concern.

## Resource and regex controls

Inputs are bounded to 16 documents, 131,072 decoded UTF-8 bytes each, 786,432 aggregate text bytes,
and a 4,096-byte pattern. Rust regex compilation uses nesting 64, a 4 MiB program limit, and a 2 MiB
hybrid DFA cache. PCRE2, look-around, and backreferences are unavailable. Searches call only
`grep_searcher::Searcher::search_slice`; BOM sniffing, transcoding, binary detection, and mmap are
explicitly disabled.

The sink observes one selected record and one submatch beyond caller/provider limits, then returns
only complete deterministic prefixes. At most 64 overall occurrences are retained per selected
record. Context is deduplicated through per-document selected-range indexes and cannot survive
without a retained selected record. Normalization and record/submatch materialization stop after
the single output probe instead of processing the excluded tail. Exact nonallocating record-length
accounting keeps the SDK success envelope at or below 1,000,000 serialized bytes.

The 350,000,000 release fuel ceiling is measured by component-host regressions rather than inferred
from native timing. The gates cover maximum decoded no-match scanning, a maximum-aggregate
roughly-770-KiB response under release fuel and 64 MiB, the reported 409-record context case under
10,000,000 fuel, and 1,000 simple selected records under 30,000,000 fuel. Fuel and the 30-second
deadline are independent host-failure bounds, not provider error codes.

Host traps (fuel, deadline, or memory) are host failures, not provider-declared errors. Provider
failures use static messages and only these stable codes: `unsupported-capability`, `invalid-input`,
`invalid-options`, `invalid-pattern`, and `search-failed`. Upstream parser/search diagnostics,
document paths, patterns, and text are never reflected into provider error messages.

## Supply chain and release

`Cargo.lock` pins every transitive version/checksum. Direct SDK, ripgrep, Serde, and testkit pins are
exact. `cargo-deny` limits registries, licenses, advisories, and forbidden packages. CI rejects any
tracked `*.wasm`, validates both core and component modules, proves zero imports, checks decoded WIT,
and enforces the 2,000,000-byte component ceiling. All third-party Actions are full commit SHAs.

The v0.1.0 release workflow:

1. accepts only an annotated `v0.1.0` tag whose peeled commit has version 0.1.0 and is in `main`;
2. runs all source/component/host/resource/license gates and compares two clean Rust 1.97.0 builds;
3. verifies the byte-exact MIT/Apache/WHATWG notice bundle embedded in the Wasm, then attests
   exactly `ripgrep-provider.wasm` and its SHA-256 file;
4. creates a run-marked draft, then pushes directly to the sole final
   `ghcr.io/dekopon-agents/provider-ripgrep:0.1.0` tag;
5. verifies the one `application/wasm` layer, artifact type, digest, BSD-inclusive SPDX annotation,
   embedded-notice annotation, and anonymously pulled bytes;
6. publishes release notes containing the OCI manifest digest, limitations, and the complete
   byte-exact distribution-license bundle, then finalizes the GitHub release as the transaction's
   last mutation.

No `latest`, temporary, or staging OCI tag is created. Failure cleanup resolves only the exact
run-marked draft and final manifest, verifies run ownership plus sole-tag package metadata, and
preserves anything it cannot prove belongs exclusively to that failed run. Published release bytes
are immutable; recovery requires a new version.
