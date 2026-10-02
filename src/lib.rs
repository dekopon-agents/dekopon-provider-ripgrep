//! A bounded, import-free ripgrep provider over caller-supplied virtual documents.
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

/// Search over bounded virtual documents (RG-b removes the virtual document input).
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
        // Temporary RG-a CLI bridge: proposal contains a placeholder, not piped bytes.
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
        // RG-a bridge only: retain JSON document output until RG-b deletes the envelope.
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
    fn typed_manifest_is_import_free_and_closed() {
        let manifest = provider::manifest::<RipgrepProvider>().expect("typed manifest");
        assert_eq!(manifest.id.as_str(), "ripgrep");
        assert_eq!(manifest.command_words, ["rg"]);
        assert_eq!(manifest.capabilities.len(), 1);
        let cap = &manifest.capabilities[0];
        assert_eq!(cap.id.as_str(), "ripgrep.search");
        assert_eq!(cap.risk, RiskLevel::Low);
        assert_eq!(cap.input_schema["additionalProperties"], false);
        assert!(cap.input_schema["properties"]["documents"].is_object());
    }
}
