# Changelog

## [0.5.0] - 2026-09-20

- Move to provider SDK 0.18.0, isolate concurrent test hosts' compilation caches, and update
  the test host's rustls to fix RUSTSEC-2026-0285; no caller-facing behavior changes.

## 0.4.0

- Move to `dekopon-provider-sdk 0.15.0`. `CommandInvocation` gains `secret_use`, named `None` at the
  one call site that builds one by hand; this provider proposes no secret use. `run_command` and
  `export_provider_with_cli!` are otherwise unchanged.
- Delete `.github/workflows/recover-v0.1.0.yml`, a one-shot `workflow_dispatch` recovery job pinned
  to the exact run/job/asset IDs of the original `v0.1.0` release incident and to the toolchain it
  used (Rust 1.97.0, wasm-tools 1.236.1); `ci.yml` and `release.yml` are unaffected. Drop the
  matching "residual finalizer" gates from `scripts/validate-workflows.sh` along with it — they
  existed only to pin that file's byte-for-byte content — and keep the CI/release SHA-pinning,
  interlock, and OCI-verifier self-test gates.
- Update `scripts/assert-lock-and-feature-graph.sh`, `scripts/check-third-party-notices.sh`,
  `scripts/inspect-component.sh`, and `THIRD_PARTY_NOTICES.md` to require and describe
  `dekopon-provider-sdk`/`dekopon-provider-sdk-testkit` 0.15.0 rather than 0.13.0. The vendored
  `dekopon:provider/provider-cli@0.3.0` WIT text is unchanged.

## 0.3.0

- Add the `rg` command word, exported through `run-command` in the `provider-cli` world. It parses
  ripgrep's own spellings for exactly what `ripgrep.search` accepts — `-F`, `-i`, `-S`, `-s`, `-w`,
  `-x`, `-U`, `-v`, `-A`, `-B`, `-C`, `-m` — over the text piped into the word, with an optional
  `PATH` that only labels that text. Help and usage errors render in the guest at status 0 and 2;
  every other ripgrep flag is a usage error naming it. `invoke` and its closed input are unchanged.
- Declare `commandWords: ["rg"]` in the manifest. A Dekopon release after 0.13.0 refuses a provider
  with capabilities and no command word.
- Generate this crate's own bindings with `wit-bindgen 0.62.0` for a local `provider` world that
  includes `dekopon:provider/provider-cli@0.3.0`; the verbatim SDK WIT moves to
  `wit/deps/provider.wit`. The export glue expands in this crate, so `unsafe_code` is `deny` with
  the two generated modules exempt, rather than `forbid`.
- The component and raw-host gates now require the `run-command` export, and the raw gate drives
  `rg --help` and one piped proposal through Wasmtime.

## 0.2.0

- Move to `dekopon-provider-sdk 0.13.0` and `dekopon:provider@0.3.0`, and rebuild the component.
  The 0.13.0 broker host drops the `idempotency` capability classification, and the SDK's decoder
  for it in an old component's manifest is removed in the release after 0.13.0.
- Remove `idempotency` from the `ripgrep.search` capability, the manifest snapshot, and its tests.
- Move the pinned toolchain to Rust 1.98.1, wasm-tools 1.259.0, Wasmtime 48.0.2, and wit-bindgen
  0.62.0, in lockstep. The declared MSRV is the pinned toolchain, so the separate MSRV job is gone.
- Replace the retired `dekopon-run` host with `dekopon-provider-sdk-testkit 0.13.0`. The direct-host
  and resource-limit shell suites are now `tests/broker.rs` cases against `FakeBroker`, which is the
  broker's own Wasmtime host rather than a second one, and they keep every fuel, memory, wire,
  context, thousand-result, multiline and dense-submatch bound. A tight-budget case replaces the
  removed `BrokerProviderRegistry::metrics` fuel counters by proving the ceiling actually binds.
- Generalize the release workflow to any `v<version>` tag: the version comes from `cargo metadata`,
  the GHCR preflight requires only that this version's OCI tag is absent rather than the whole
  package, the publish and finalize gates identify this run's package version by digest and tag
  instead of asserting the package holds exactly one, and rollback deletes only the version this
  run pushed.

## 0.1.0

- Add the import-free `ripgrep.search` capability over bounded caller-fed virtual documents.
- Add exact ripgrep/SDK pins, closed semantic validation, deterministic context/submatch/truncation,
  and static non-reflective errors.
- Add native, adversarial, raw component, direct host, FakeBroker, resource, WIT/import, size,
  supply-chain, reproducibility, CI, and immutable release gates.
- Index and lazily normalize context, defer record/submatch materialization past output truncation,
  and measure high-result/context/near-output fuel at the component host boundary.
- Embed the complete MIT/Apache/WHATWG distribution-license bundle in the Wasm and release
  documentation, with a BSD-inclusive OCI SPDX expression.
