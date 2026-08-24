//! A bounded, import-free ripgrep provider over caller-supplied virtual documents.
//!
//! Paths are opaque labels. Provider code performs no filesystem, network, subprocess, storage,
//! or host-interface operation; raw wire size, fuel, linear memory, and wall time remain host
//! limits.

mod error;
mod input;
mod manifest;
mod output;
mod search;

use dekopon_provider_sdk::{CapabilityId, Provider, ProviderError, ProviderManifest};
use serde_json::Value;

/// The single v0.1.0 provider implementation.
///
/// This type is public only so native integration tests can exercise the exact [`Provider`]
/// boundary. The component exports remain solely `describe` and `invoke`.
pub struct RipgrepProvider;

impl Provider for RipgrepProvider {
    fn manifest() -> ProviderManifest {
        manifest::manifest()
    }

    fn invoke(capability: &CapabilityId, input: Value) -> Result<Value, ProviderError> {
        if capability.as_str() != "ripgrep.search" {
            return Err(error::unsupported_capability());
        }
        let input = input::SearchInput::parse(input)?;
        let output = search::run(&input)?;
        serde_json::to_value(output).map_err(|_| error::search_failed())
    }
}

dekopon_provider_sdk::export_provider!(RipgrepProvider);

#[cfg(test)]
mod tests {
    use dekopon_provider_sdk::Provider;
    use serde_json::Value;

    use super::RipgrepProvider;

    #[test]
    fn mirrored_wit_is_byte_exact() {
        assert_eq!(
            include_str!("../wit/provider.wit"),
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
        assert_eq!(decoded["commandWords"], serde_json::json!([]));
    }
}
