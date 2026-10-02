//! A stdin-only streaming ripgrep provider. No filesystem, network or storage grants.
//! The SDK's stdio import is mandatory; fuel, memory and time remain host limits.

mod commands;
mod error;
mod input;
mod search;

use dekopon_provider_sdk::provider::{self, Capability, Proposal, Provider, Stdout, Usage};
use dekopon_provider_sdk::{EffectKind, RiskLevel};

pub(crate) const COMMAND_WORD: &str = "rg";

/// The single typed provider implementation.
pub struct RipgrepProvider;

/// Search piped stdin.
pub struct Search;

impl Provider for RipgrepProvider {
    const ID: &'static str = "ripgrep";
    const COMMAND_WORDS: &'static [&'static str] = &[COMMAND_WORD];
    const DESCRIPTION: &'static str =
        "Searches only piped stdin with Rust ripgrep matchers; streams matching lines to stdout";
    type Args = commands::Rg;
    type Capabilities = (Search,);

    fn propose(args: Self::Args, stdin_piped: bool) -> Result<Proposal<Self>, Usage> {
        commands::propose(args, stdin_piped)
    }
}

impl Capability for Search {
    type Provider = RipgrepProvider;
    const NAME: &'static str = "search";
    const DESCRIPTION: &'static str = "Search piped stdin with regex/fixed matching and bounded context; emit selected text lines";
    const EFFECT: EffectKind = EffectKind::ReadOnly;
    const RISK: RiskLevel = RiskLevel::Low;
    type Input = input::SearchInput;
    type Needs = ();
    type Error = error::SearchError;

    fn run(input: Self::Input, (): Self::Needs, out: &mut Stdout) -> Result<(), Self::Error> {
        use std::io::Read as _;
        input.validate()?;
        let mut piped = provider::stdin().ok_or_else(error::no_input)?;
        let mut first = [0];
        if piped.read(&mut first).map_err(|_| error::search_failed())? == 0 {
            return Err(error::no_input());
        }
        if search::run(&input, std::io::Cursor::new(first).chain(piped), out)? {
            Ok(())
        } else {
            Err(error::no_match())
        }
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
    fn typed_manifest_is_closed_and_stdin_only() {
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
        assert_eq!(schema["required"], serde_json::json!(["pattern"]));
        assert!(fields.get("documents").is_none());
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
                .contains("selected lines")
        );
    }
}
