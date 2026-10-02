use std::fmt;

use dekopon_provider_sdk::provider::{Code, Failure};

/// Local search failure; no sensitive input is included in its diagnostics.
#[derive(Debug)]
pub struct SearchError {
    code: Code,
    message: &'static str,
}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message)
    }
}

impl Failure for SearchError {
    fn code(&self) -> Code {
        self.code
    }
}

#[cfg(test)]
impl SearchError {
    pub(crate) fn code(&self) -> &'static str {
        self.code.as_str()
    }
}

pub(crate) fn invalid_input() -> SearchError {
    SearchError {
        code: Code::INVALID_INPUT,
        message: "input does not match the closed ripgrep.search schema and decoded limits",
    }
}

pub(crate) fn invalid_options() -> SearchError {
    SearchError {
        code: Code::new("invalid-options"),
        message: "the requested search option combination is not supported",
    }
}

pub(crate) fn invalid_pattern() -> SearchError {
    SearchError {
        code: Code::new("invalid-pattern"),
        message: "pattern is invalid or exceeds the configured regex complexity limits",
    }
}

pub(crate) fn search_failed() -> SearchError {
    SearchError {
        code: Code::new("search-failed"),
        message: "the bounded in-memory search could not be completed",
    }
}
