//! A bounded ripgrep provider over caller-supplied virtual documents.
//! The SDK's stdio import is mandatory; the search declares no optional imports.
//!
//! Paths are opaque labels. Provider code performs no filesystem, network, subprocess, storage,
//! or host-interface operation; raw wire size, fuel, linear memory, and wall time remain host
//! limits. The `rg` command word is the same search spelled as ripgrep's command line over the
//! text piped into it.

mod commands;
mod error;
mod input;
mod output;
mod search;

use dekopon_provider_sdk::provider::{self, Capability, Proposal, Provider, Stdout, Usage};
use dekopon_provider_sdk::{EffectKind, RiskLevel};

pub(crate) const COMMAND_WORD: &str = "rg";

/// The single typed provider implementation.
pub struct RipgrepProvider;

/// Search over bounded virtual documents.
pub struct Search;

impl Provider for RipgrepProvider {
    const ID: &'static str = "ripgrep";
    const COMMAND_WORDS: &'static [&'static str] = &[COMMAND_WORD];
    const DESCRIPTION: &'static str = "Searches bounded caller-supplied UTF-8 virtual documents with Rust ripgrep matchers; never reads paths or performs I/O";
    type Args = commands::Rg;
    type Capabilities = (Search,);

    fn propose(args: Self::Args, stdin_piped: bool) -> Result<Proposal<Self>, Usage> {
        commands::propose(args, stdin_piped)
    }
}

impl Capability for Search {
    type Provider = RipgrepProvider;
    const NAME: &'static str = "search";
    const DESCRIPTION: &'static str = "Search 1–16 virtual documents in caller order with bounded regex/fixed matching, context, byte offsets, and deterministic truncation";
    const EFFECT: EffectKind = EffectKind::ReadOnly;
    const RISK: RiskLevel = RiskLevel::Low;
    type Input = input::SearchInput;
    type Needs = ();
    type Error = error::SearchError;

    fn run(mut input: Self::Input, (): Self::Needs, out: &mut Stdout) -> Result<(), Self::Error> {
        if let Some(piped) = provider::stdin() {
            use std::io::Read as _;
            let mut bytes = Vec::new();
            piped
                .take((input::MAX_DOCUMENT_TEXT_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|_| error::invalid_input())?;
            if bytes.len() > input::MAX_DOCUMENT_TEXT_BYTES {
                return Err(error::invalid_input());
            }
            let text = String::from_utf8(bytes).map_err(|_| error::invalid_input())?;
            if input.documents.len() == 1 && input.documents[0].text.is_empty() {
                input.documents[0].text = text;
            }
        }
        input.validate()?;
        let result = search::run(&input)?;
        serde_json::to_writer(out, &result).map_err(|_| error::search_failed())?;
        Ok(())
    }
}

#[allow(unsafe_code)]
mod export {
    dekopon_provider_sdk::export!(super::RipgrepProvider);
}

#[cfg(test)]
mod tests {
    use dekopon_provider_sdk::RiskLevel;
    use dekopon_provider_sdk::provider;

    use super::RipgrepProvider;

    #[test]
    fn typed_manifest_is_closed_and_bounds_the_document_bridge() {
        let manifest = provider::manifest::<RipgrepProvider>().expect("typed manifest");
        let snapshot = format!(
            "{}\n",
            serde_json::to_string_pretty(&manifest).expect("manifest JSON")
        );
        assert_eq!(
            snapshot,
            include_str!("../tests/fixtures/typed-manifest.json")
        );
        assert_eq!(manifest.id.as_str(), "ripgrep");
        assert_eq!(manifest.command_words, ["rg"]);
        assert_eq!(manifest.capabilities.len(), 1);
        let cap = &manifest.capabilities[0];
        assert_eq!(cap.id.as_str(), "ripgrep.search");
        assert_eq!(cap.risk, RiskLevel::Low);
        assert_eq!(cap.effect, dekopon_provider_sdk::EffectKind::ReadOnly);
        let schema = &cap.input_schema;
        let fields = &schema["properties"];
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            schema["required"],
            serde_json::json!(["documents", "pattern"])
        );
        assert_eq!(fields["documents"]["minItems"], 1);
        assert_eq!(fields["documents"]["maxItems"], 16);
        assert!(
            fields["documents"]["description"]
                .as_str()
                .unwrap()
                .contains("aggregate text")
        );
        let document = &fields["documents"]["items"];
        assert_eq!(document["additionalProperties"], false);
        assert_eq!(document["properties"]["path"]["minLength"], 1);
        assert_eq!(document["properties"]["path"]["maxLength"], 256);
        assert!(
            document["properties"]["path"]["description"]
                .as_str()
                .unwrap()
                .contains("never dereferenced")
        );
        assert_eq!(document["properties"]["text"]["maxLength"], 131_072);
        assert!(
            document["properties"]["text"]["description"]
                .as_str()
                .unwrap()
                .contains("UTF-8")
        );
        assert_eq!(fields["pattern"]["minLength"], 1);
        assert_eq!(fields["pattern"]["maxLength"], 4096);
        assert!(
            fields["pattern"]["description"]
                .as_str()
                .unwrap()
                .contains("PCRE2")
        );
        assert_eq!(fields["context"]["additionalProperties"], false);
        assert_eq!(
            fields["context"]["required"],
            serde_json::json!(["before", "after"])
        );
        for side in ["before", "after"] {
            assert_eq!(fields["context"]["properties"][side]["minimum"], 0);
            assert_eq!(fields["context"]["properties"][side]["maximum"], 8);
        }
        assert!(
            fields["context"]["description"]
                .as_str()
                .unwrap()
                .contains("zero lines")
        );
        assert_eq!(fields["max_results"]["minimum"], 1);
        assert_eq!(fields["max_results"]["maximum"], 1000);
        assert_eq!(fields["max_results"]["default"], 100);
        assert!(
            fields["max_results"]["description"]
                .as_str()
                .unwrap()
                .contains("selected records")
        );
    }
}
