use crate::error::{self, SearchError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub(crate) const MAX_PATTERN_BYTES: usize = 4_096;
pub(crate) const MAX_CONTEXT_LINES: usize = 8;
pub(crate) const DEFAULT_MAX_RESULTS: usize = 100;
pub(crate) const MAX_RESULTS: usize = 1_000;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, JsonSchema, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SearchMode {
    #[default]
    Regex,
    Fixed,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, JsonSchema, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum CaseMode {
    #[default]
    Sensitive,
    Insensitive,
    Smart,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, JsonSchema, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
/// Both counts are required when context is supplied; defaults to zero lines on both sides.
pub(crate) struct Context {
    #[schemars(range(min = 0, max = 8))]
    pub(crate) before: usize,
    #[schemars(range(min = 0, max = 8))]
    pub(crate) after: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(
    description = "Search only piped stdin. No filesystem path or document input is accepted."
)]
pub struct SearchInput {
    /// One Rust-regex pattern or fixed literal, 1–4096 UTF-8 bytes. PCRE2 look-around and backreferences are unsupported.
    #[schemars(length(min = 1, max = 4096))]
    pub(crate) pattern: String,
    #[serde(default)]
    pub(crate) mode: SearchMode,
    #[serde(default)]
    pub(crate) case: CaseMode,
    #[serde(default)]
    pub(crate) word: bool,
    #[serde(default)]
    pub(crate) line: bool,
    #[serde(default)]
    pub(crate) multiline: bool,
    #[serde(default)]
    pub(crate) invert: bool,
    #[serde(default)]
    pub(crate) context: Context,
    /// Maximum selected lines, not occurrences or context lines.
    #[serde(default = "default_max_results")]
    #[schemars(range(min = 1, max = 1000))]
    pub(crate) max_results: usize,
}
const fn default_max_results() -> usize {
    DEFAULT_MAX_RESULTS
}

impl SearchInput {
    pub(crate) fn validate(&self) -> Result<(), SearchError> {
        if self.pattern.is_empty()
            || self.pattern.len() > MAX_PATTERN_BYTES
            || self.context.before > MAX_CONTEXT_LINES
            || self.context.after > MAX_CONTEXT_LINES
            || !(1..=MAX_RESULTS).contains(&self.max_results)
        {
            return Err(error::invalid_input());
        }
        if (self.word && self.line)
            || (self.multiline && self.line)
            || (self.multiline && self.invert)
        {
            return Err(error::invalid_options());
        }
        Ok(())
    }
}
