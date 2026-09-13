# Changelog

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
