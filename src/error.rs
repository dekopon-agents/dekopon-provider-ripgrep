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

pub(crate) fn invalid_input() -> SearchError {
    SearchError {
        code: Code::INVALID_INPUT,
        message: "input does not match the closed ripgrep.search schema or option limits",
    }
}

pub(crate) fn invalid_options() -> SearchError {
    SearchError {
        code: Code::USAGE,
        message: "the requested search option combination is not supported",
    }
}

pub(crate) fn invalid_pattern() -> SearchError {
    SearchError {
        code: Code::USAGE,
        message: "pattern is invalid or exceeds the configured regex complexity limits",
    }
}

pub(crate) fn no_input() -> SearchError {
    SearchError {
        code: Code::USAGE,
        message: "rg requires nonempty piped stdin",
    }
}

pub(crate) fn no_match() -> SearchError {
    SearchError {
        code: Code::new("no-match"),
        message: "",
    }
}

pub(crate) fn search_failed() -> SearchError {
    SearchError {
        code: Code::new("search-failed"),
        message: "the streaming search could not be completed",
    }
}
