//! A bounded, import-free ripgrep provider over caller-supplied virtual documents.
//!
//! Paths are opaque labels. Provider code performs no filesystem, network, subprocess, storage,
//! or host-interface operation; raw wire size, fuel, linear memory, and wall time remain host
//! limits. The `rg` command word is the same search spelled as ripgrep's command line over the
//! text piped into it.

mod commands;
mod error;
mod input;
mod manifest;
mod output;
mod search;

use dekopon_provider_sdk::{CapabilityId, CommandRun, Provider, ProviderError, ProviderManifest};
use serde_json::Value;

/// The one capability, named once for the manifest, `invoke`, and the `rg` dispatch.
pub(crate) const SEARCH: &str = "ripgrep.search";

/// The command word this provider contributes to the sandboxed shell.
pub(crate) const COMMAND_WORD: &str = "rg";

// Generated component glue for the `provider-cli` world. Its C-ABI exports are `unsafe` by
// construction, so these two modules are the crate's only exemption from `unsafe_code`.
#[allow(unsafe_code)]
mod bindings {
    wit_bindgen::generate!({
        path: "wit",
        world: "provider",
    });
}

#[allow(unsafe_code)]
mod export {
    use super::bindings;

    dekopon_provider_sdk::export_provider_with_cli!(super::RipgrepProvider, bindings);
}

/// The single provider implementation.
///
/// This type is public only so native integration tests can exercise the exact [`Provider`]
/// boundary. The component exports exactly `describe`, `invoke`, and `run-command`.
pub struct RipgrepProvider;

impl Provider for RipgrepProvider {
    fn manifest() -> ProviderManifest {
        manifest::manifest()
    }

    fn invoke(capability: &CapabilityId, input: Value) -> Result<Value, ProviderError> {
        if capability.as_str() != SEARCH {
            return Err(error::unsupported_capability());
        }
        let input = input::SearchInput::parse(input)?;
        let output = search::run(&input)?;
        serde_json::to_value(output).map_err(|_| error::search_failed())
    }

    fn run_command(argv: &[String], stdin: Option<&str>) -> Result<CommandRun, ProviderError> {
        commands::run(argv, stdin)
    }
}

#[cfg(test)]
mod tests {
    use dekopon_provider_sdk::Provider;
    use serde_json::Value;

    use super::RipgrepProvider;

    #[test]
    fn mirrored_wit_is_byte_exact() {
        assert_eq!(
            include_str!("../wit/deps/provider.wit"),
            dekopon_provider_sdk::PROVIDER_WIT
        );
    }

    #[test]
    fn manifest_snapshot_is_exact() {
        let actual = format!(
            "{}\n",
            serde_json::to_string_pretty(&RipgrepProvider::manifest())
                .expect("manifest serializes")
        );
        assert_eq!(actual, include_str!("../tests/fixtures/manifest.json"));
        let decoded: Value = serde_json::from_str(&actual).expect("manifest is JSON");
        assert_eq!(decoded["commandWords"], serde_json::json!(["rg"]));
    }
}
