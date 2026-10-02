# Security policy

## Reporting

Report suspected vulnerabilities privately through GitHub's security-advisory flow for
`dekopon-agents/dekopon-provider-ripgrep`. Do not include private piped input in a public
issue. Every published version of this repository is immutable; a fix is released as a new version
and never by replacing bytes already published under an existing one.

## Authority boundary

`ripgrep.search` searches only the SDK's piped stdin reader. Its closed JSON proposal supplies
search options, never text or a path. Handwritten provider code performs no filesystem, path
lookup, directory walk, mmap, network, HTTP, storage, subprocess, environment, clock, random,
WASI, or JavaScript operation. The decoded component imports mandatory
`dekopon:stdio/streams@0.1.0` and exports `describe`, `invoke`, and `run-command`;
`commandWords` is `["rg"]`. Stdin and stdout are host-controlled streams, not grants to files.

`run-command` is the `rg` word. It reads argv and the boolean `stdin-piped` marker only, and either
renders clap help/version/usage — which authorizes nothing — or proposes `ripgrep.search`,
authorized by the broker exactly as a direct call. No path operand or filesystem flag is accepted.

The security boundary is the validated component plus a correctly configured Dekopon host:

- the host enforces 1,048,576-byte serialized proposal input, 64 MiB linear memory,
  350,000,000 fuel for release invocations, and a 30-second deadline; stdout is a stream;
- the provider enforces closed search options, pattern/context/count limits, regex
  nesting/program/cache limits, and a 1 MiB grep-searcher heap limit;
- the broker separately authenticates callers, authorizes `ripgrep.search`, and constrains an
  invocation. The component itself grants nothing.

Compilation is outside invocation fuel/deadline accounting. A deployment should admit only a
reviewed component digest and keep its Wasmtime compilation cache owner-writable only.

## JSON proposal and stream boundary

The SDK parses `input-json` into the closed typed schema; malformed JSON, unknown properties,
invalid ranges and incompatible options are rejected. The only required member is `pattern`.
The proposal is bounded separately from stdin. Duplicate JSON object keys retain the last value
after parsing; producers must emit unique keys. No JSON result envelope is returned.

`grep_searcher::Searcher::search_reader` consumes the stdin reader and writes selected matching
and context text lines to stdout, without buffering the complete default-mode input. BOM sniffing,
transcoding, binary detection and mmap are disabled. The matcher bounds regex nesting to 64,
program size to 4 MiB, DFA cache to 2 MiB, and pattern length to 4,096 UTF-8 bytes; PCRE2,
look-around and backreferences are unavailable. Context is bounded to eight lines per side,
`max_results` to 1–1,000 selected lines. The searcher heap limit is 1 MiB: long records fail
cleanly. **Multiline `-U` reads at most 1 MiB of stdin and fails above it** because cross-line
matching needs the whole input, as with ripgrep's multiline mode. Over-limit search exits nonzero
with a bounded static stderr message; it never emits input or a JSON envelope as an error.

A match exits 0; valid no-match exits 1 as a guest exit, not a host failure; absent or zero-byte
required stdin and malformed usage exit 2. A closed stdout reader exits 141 and supersedes other
provider errors. SDK `Failure` currently writes an empty line to stderr for valid no-match; it
never includes the pattern or stdin bytes. Fuel exhaustion, deadline, and memory traps remain host
failures, not guest statuses. The broker still audits and authorizes the proposal; a streamed
stdout line is not an independent authority decision.

## Supply chain and release

`Cargo.lock` pins every transitive version/checksum. Direct SDK, ripgrep, Serde, wit-bindgen, and
testkit pins are exact. `cargo-deny` limits registries, licenses, advisories, and forbidden
packages. CI rejects any tracked `*.wasm`, validates both core and component modules, checks the mandatory stdio import and no optional imports, checks decoded WIT, and enforces the 2,000,000-byte component ceiling. All third-party
Actions are full commit SHAs.

The release workflow:

1. accepts only an annotated `v<version>` tag whose peeled commit declares that exact crate version
   and is in `main`;
2. runs all source/component/host/resource/license gates and compares two clean Rust 1.98.1 builds;
3. verifies the byte-exact MIT/Apache/WHATWG notice bundle embedded in the Wasm, then attests
   exactly `ripgrep-provider.wasm` and its SHA-256 file;
4. creates a run-marked draft, then pushes directly to that version's sole immutable
   `ghcr.io/dekopon-agents/provider-ripgrep:<version>` tag;
5. verifies the one `application/wasm` layer, artifact type, digest, BSD-inclusive SPDX annotation,
   embedded-notice annotation, and anonymously pulled bytes;
6. publishes release notes containing the OCI manifest digest, limitations, and the complete
   byte-exact distribution-license bundle, then finalizes the GitHub release as the transaction's
   last mutation.

No `latest`, temporary, or staging OCI tag is created. Failure cleanup resolves only the exact
run-marked draft and the manifest this run pushed, verifies run ownership plus that version's sole
tag, and preserves anything it cannot prove belongs exclusively to that failed run — every
predecessor's release and package version included. Published release bytes
are immutable; recovery requires a new version.
