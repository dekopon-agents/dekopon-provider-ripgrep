use dekopon_provider_sdk::ProviderError;
use std::fmt;

/// Internal static diagnostics; the provider boundary converts them to the SDK error type.
#[derive(Debug)]
pub(crate) struct SearchError {
    code: &'static str,
    message: &'static str,
}
#[cfg(test)]
impl SearchError {
    pub(crate) fn code(&self) -> &'static str {
        self.code
    }
}
impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message)
    }
}
impl From<SearchError> for ProviderError {
    fn from(value: SearchError) -> Self {
        Self::new(value.code, value.message)
    }
}

pub(crate) fn unsupported_capability() -> ProviderError {
    ProviderError::new(
        "unsupported-capability",
        "the ripgrep provider exposes only ripgrep.search",
    )
}
pub(crate) fn invalid_input() -> SearchError {
    SearchError {
        code: "invalid-input",
        message: "input does not match the closed ripgrep.search schema and decoded limits",
    }
}
pub(crate) fn invalid_options() -> SearchError {
    SearchError {
        code: "invalid-options",
        message: "the requested search option combination is not supported",
    }
}
pub(crate) fn invalid_pattern() -> SearchError {
    SearchError {
        code: "invalid-pattern",
        message: "pattern is invalid or exceeds the configured regex complexity limits",
    }
}
pub(crate) fn search_failed() -> SearchError {
    SearchError {
        code: "search-failed",
        message: "the bounded in-memory search could not be completed",
    }
}
