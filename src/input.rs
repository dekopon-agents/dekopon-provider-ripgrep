use std::collections::HashSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[cfg(test)]
use serde_json::Value;

use crate::error::{self, SearchError};

pub(crate) const MIN_DOCUMENTS: usize = 1;
pub(crate) const MAX_DOCUMENTS: usize = 16;
pub(crate) const MAX_PATH_BYTES: usize = 256;
pub(crate) const MAX_DOCUMENT_TEXT_BYTES: usize = 131_072;
pub(crate) const MAX_TOTAL_TEXT_BYTES: usize = 786_432;
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

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Document {
    /// Opaque relative /-separated label, enforced at 1–256 UTF-8 bytes; never dereferenced. Empty, dot, dot-dot, control, backslash, absolute, drive-prefixed, UNC-like, and empty components are rejected.
    #[schemars(length(min = 1, max = 256))]
    pub(crate) path: String,
    /// Decoded UTF-8 virtual content, at most 131072 bytes. LF alone terminates lines; CR and BOM bytes are preserved.
    #[schemars(length(max = 131_072))]
    pub(crate) text: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, JsonSchema, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Context {
    #[schemars(range(min = 0, max = 8))]
    pub(crate) before: usize,
    #[schemars(range(min = 0, max = 8))]
    pub(crate) after: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
#[schemars(
    description = "Closed semantic input. Runtime limits include 64 submatches per selected result and a 1000000-byte JSON stdout bridge. The host separately enforces the raw serialized invocation limit."
)]
pub struct SearchInput {
    /// Caller-fed virtual documents in search order. Paths must be exact-byte unique. Each text is at most 131072 decoded UTF-8 bytes and aggregate text is at most 786432 decoded UTF-8 bytes.
    #[schemars(length(min = 1, max = 16))]
    pub(crate) documents: Vec<Document>,
    /// One Rust-regex pattern or one fixed literal, enforced at 1–4,096 decoded UTF-8 bytes. PCRE2 look-around and backreferences are unsupported.
    #[schemars(length(min = 1, max = 4096))]
    pub(crate) pattern: String,
    /// Regex syntax, or one literal fixed string.
    #[serde(default)]
    pub(crate) mode: SearchMode,
    #[serde(default)]
    pub(crate) case: CaseMode,
    /// Require ripgrep word-boundary matching; cannot be combined with line.
    #[serde(default)]
    pub(crate) word: bool,
    /// Require a whole LF-delimited line; cannot be combined with word or multiline.
    #[serde(default)]
    pub(crate) line: bool,
    /// Permit explicit matches across LF. Dot still excludes LF unless the Rust pattern enables s; cannot be combined with line or invert.
    #[serde(default)]
    pub(crate) multiline: bool,
    /// Select non-matching lines; incompatible with multiline.
    #[serde(default)]
    pub(crate) invert: bool,
    /// Both counts are required when context is supplied; defaults to zero lines on both sides.
    #[serde(default)]
    pub(crate) context: Context,
    /// Maximum returned selected records, not occurrences or context records.
    #[serde(default = "default_max_results")]
    #[schemars(range(min = 1, max = 1000))]
    pub(crate) max_results: usize,
}

const fn default_max_results() -> usize {
    DEFAULT_MAX_RESULTS
}

impl SearchInput {
    #[cfg(test)]
    pub(crate) fn parse(value: Value) -> Result<Self, SearchError> {
        let input: Self = serde_json::from_value(value).map_err(|_| error::invalid_input())?;
        input.validate()?;
        Ok(input)
    }

    pub(crate) fn validate(&self) -> Result<(), SearchError> {
        if !(MIN_DOCUMENTS..=MAX_DOCUMENTS).contains(&self.documents.len())
            || self.pattern.is_empty()
            || self.pattern.len() > MAX_PATTERN_BYTES
            || self.context.before > MAX_CONTEXT_LINES
            || self.context.after > MAX_CONTEXT_LINES
            || !(1..=MAX_RESULTS).contains(&self.max_results)
        {
            return Err(error::invalid_input());
        }

        let mut paths = HashSet::with_capacity(self.documents.len());
        let mut total_text_bytes = 0usize;
        for document in &self.documents {
            if !valid_path(&document.path)
                || document.text.len() > MAX_DOCUMENT_TEXT_BYTES
                || !paths.insert(document.path.as_str())
            {
                return Err(error::invalid_input());
            }
            total_text_bytes = total_text_bytes
                .checked_add(document.text.len())
                .ok_or_else(error::invalid_input)?;
            if total_text_bytes > MAX_TOTAL_TEXT_BYTES {
                return Err(error::invalid_input());
            }
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

fn valid_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    if bytes.is_empty()
        || bytes.len() > MAX_PATH_BYTES
        || path.starts_with('/')
        || path.contains('\\')
        || path.chars().any(char::is_control)
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
    {
        return false;
    }
    path.split('/')
        .all(|component| !component.is_empty() && component != "." && component != "..")
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{
        DEFAULT_MAX_RESULTS, MAX_CONTEXT_LINES, MAX_DOCUMENT_TEXT_BYTES, MAX_DOCUMENTS,
        MAX_PATH_BYTES, MAX_PATTERN_BYTES, MAX_RESULTS, SearchInput,
    };

    fn valid() -> Value {
        json!({
            "documents": [{"path": "src/lib.rs", "text": "hello\n"}],
            "pattern": "hello"
        })
    }

    fn error_code(value: Value) -> String {
        SearchInput::parse(value)
            .expect_err("fixture must be rejected")
            .code()
            .to_owned()
    }

    #[test]
    fn defaults_are_exact() {
        let input = SearchInput::parse(valid()).expect("valid defaults");
        assert_eq!(input.max_results, DEFAULT_MAX_RESULTS);
        assert_eq!(input.context.before, 0);
        assert_eq!(input.context.after, 0);
        assert!(!input.word);
        assert!(!input.line);
        assert!(!input.multiline);
        assert!(!input.invert);
    }

    #[test]
    fn root_document_and_context_objects_are_closed_and_required() {
        for value in [
            json!(null),
            json!([]),
            json!({}),
            json!({"documents": [{"path": "a", "text": ""}]}),
            json!({"pattern": "x"}),
            json!({"documents": null, "pattern": "x"}),
            json!({"documents": [], "pattern": null}),
            json!({"documents": [{"path": "a"}], "pattern": "x"}),
            json!({"documents": [{"text": "x"}], "pattern": "x"}),
            json!({"documents": [{"path": "a", "text": "x", "extra": 1}], "pattern": "x"}),
            json!({"documents": [{"path": "a", "text": "x"}], "pattern": "x", "extra": 1}),
            json!({"documents": [{"path": "a", "text": "x"}], "pattern": "x", "context": {"before": 1}}),
            json!({"documents": [{"path": "a", "text": "x"}], "pattern": "x", "context": {"before": 1, "after": 1, "extra": 1}}),
        ] {
            assert_eq!(error_code(value), "invalid-input");
        }
    }

    #[test]
    fn wrong_types_nulls_and_non_integer_counts_are_rejected() {
        let mutations = [
            ("mode", json!(null)),
            ("mode", json!(1)),
            ("case", json!(true)),
            ("word", json!("true")),
            ("line", json!(1)),
            ("multiline", json!([])),
            ("invert", json!({})),
            ("max_results", json!(null)),
            ("max_results", json!(1.5)),
            ("max_results", json!(-1)),
            ("context", json!(null)),
        ];
        for (field, replacement) in mutations {
            let mut value = valid();
            value[field] = replacement;
            assert_eq!(error_code(value), "invalid-input", "field {field}");
        }
        for (field, replacement) in [("before", json!(0.5)), ("after", json!(-1))] {
            let mut value = valid();
            value["context"] = json!({"before": 0, "after": 0});
            value["context"][field] = replacement;
            assert_eq!(error_code(value), "invalid-input", "context.{field}");
        }
    }

    #[test]
    fn enum_and_numeric_boundaries_are_exact() {
        for (field, replacement) in [
            ("mode", json!("literal")),
            ("case", json!("fold")),
            ("max_results", json!(0)),
            ("max_results", json!(MAX_RESULTS + 1)),
        ] {
            let mut value = valid();
            value[field] = replacement;
            assert_eq!(error_code(value), "invalid-input");
        }
        for count in [1, MAX_RESULTS] {
            let mut value = valid();
            value["max_results"] = json!(count);
            SearchInput::parse(value).expect("inclusive result boundary");
        }
        for count in [0, MAX_CONTEXT_LINES] {
            let mut value = valid();
            value["context"] = json!({"before": count, "after": count});
            SearchInput::parse(value).expect("inclusive context boundary");
        }
        let mut value = valid();
        value["context"] = json!({"before": MAX_CONTEXT_LINES + 1, "after": 0});
        assert_eq!(error_code(value), "invalid-input");
    }

    #[test]
    fn decoded_byte_and_document_count_limits_are_exact() {
        let documents = (0..MAX_DOCUMENTS)
            .map(|index| json!({"path": format!("d{index}"), "text": ""}))
            .collect::<Vec<_>>();
        SearchInput::parse(json!({"documents": documents, "pattern": "x"}))
            .expect("maximum document count");

        let too_many = (0..=MAX_DOCUMENTS)
            .map(|index| json!({"path": format!("d{index}"), "text": ""}))
            .collect::<Vec<_>>();
        assert_eq!(
            error_code(json!({"documents": too_many, "pattern": "x"})),
            "invalid-input"
        );

        let valid_text = "é".repeat(MAX_DOCUMENT_TEXT_BYTES / 2);
        SearchInput::parse(json!({
            "documents": [{"path": "a", "text": valid_text}],
            "pattern": "é".repeat(MAX_PATTERN_BYTES / 2)
        }))
        .expect("exact UTF-8 byte boundaries");

        assert_eq!(
            error_code(json!({
                "documents": [{"path": "a", "text": "x".repeat(MAX_DOCUMENT_TEXT_BYTES + 1)}],
                "pattern": "x"
            })),
            "invalid-input"
        );
        assert_eq!(
            error_code(json!({
                "documents": [{"path": "a", "text": "x"}],
                "pattern": "é".repeat(MAX_PATTERN_BYTES / 2 + 1)
            })),
            "invalid-input"
        );

        let total = (0..7)
            .map(|index| {
                json!({
                    "path": format!("d{index}"),
                    "text": "x".repeat(if index < 6 { MAX_DOCUMENT_TEXT_BYTES } else { 1 })
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            error_code(json!({"documents": total, "pattern": "x"})),
            "invalid-input"
        );
    }

    #[test]
    fn paths_are_opaque_safe_unique_relative_labels() {
        let exact = format!("a{}", "b".repeat(MAX_PATH_BYTES - 1));
        SearchInput::parse(json!({
            "documents": [{"path": exact, "text": ""}],
            "pattern": "x"
        }))
        .expect("exact path byte limit");

        for path in [
            "",
            "/absolute",
            "//server/share",
            "C:drive",
            "z:/drive",
            "a\\b",
            "a//b",
            ".",
            "..",
            "a/./b",
            "a/../b",
            "a/",
            "a\u{0000}b",
            "a\u{0085}b",
        ] {
            assert_eq!(
                error_code(json!({
                    "documents": [{"path": path, "text": ""}],
                    "pattern": "x"
                })),
                "invalid-input",
                "path {path:?}"
            );
        }
        assert_eq!(
            error_code(json!({
                "documents": [
                    {"path": "same", "text": "a"},
                    {"path": "same", "text": "b"}
                ],
                "pattern": "x"
            })),
            "invalid-input"
        );
        assert_eq!(
            error_code(json!({
                "documents": [{"path": "x".repeat(MAX_PATH_BYTES + 1), "text": ""}],
                "pattern": "x"
            })),
            "invalid-input"
        );
    }

    #[test]
    fn conflicting_options_have_the_distinct_static_code() {
        for options in [
            json!({"word": true, "line": true}),
            json!({"multiline": true, "line": true}),
            json!({"multiline": true, "invert": true}),
        ] {
            let mut value = valid();
            value
                .as_object_mut()
                .expect("root object")
                .extend(options.as_object().expect("option object").clone());
            assert_eq!(error_code(value), "invalid-options");
        }
    }
}
