use dekopon_provider_sdk::ProviderError;

pub(crate) fn unsupported_capability() -> ProviderError {
    ProviderError::new(
        "unsupported-capability",
        "the ripgrep provider exposes only ripgrep.search",
    )
}

pub(crate) fn invalid_input() -> ProviderError {
    ProviderError::new(
        "invalid-input",
        "input does not match the closed ripgrep.search schema and decoded limits",
    )
}

pub(crate) fn invalid_options() -> ProviderError {
    ProviderError::new(
        "invalid-options",
        "the requested search option combination is not supported",
    )
}

pub(crate) fn invalid_pattern() -> ProviderError {
    ProviderError::new(
        "invalid-pattern",
        "pattern is invalid or exceeds the configured regex complexity limits",
    )
}

pub(crate) fn search_failed() -> ProviderError {
    ProviderError::new(
        "search-failed",
        "the bounded in-memory search could not be completed",
    )
}
