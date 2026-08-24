use dekopon_provider_sdk::{
    EffectKind, Idempotency, ProviderApiVersion, ProviderCapability, ProviderManifest, RiskLevel,
};
use serde_json::{Value, json};

use crate::{
    input::{
        DEFAULT_MAX_RESULTS, MAX_CONTEXT_LINES, MAX_DOCUMENT_TEXT_BYTES, MAX_DOCUMENTS,
        MAX_PATH_BYTES, MAX_PATTERN_BYTES, MAX_RESULTS, MAX_TOTAL_TEXT_BYTES, MIN_DOCUMENTS,
    },
    output::{MAX_SUBMATCHES_PER_RESULT, MAX_SUCCESS_ENVELOPE_BYTES},
};

pub(crate) fn manifest() -> ProviderManifest {
    ProviderManifest {
        api_version: ProviderApiVersion::V1Alpha1,
        id: "ripgrep".parse().expect("static provider identifier"),
        description: "Searches bounded caller-supplied UTF-8 virtual documents with Rust ripgrep matchers; never reads paths or performs I/O"
            .to_owned(),
        command_words: Vec::new(),
        capabilities: vec![ProviderCapability {
            id: "ripgrep.search"
                .parse()
                .expect("static capability identifier"),
            description: "Search 1–16 virtual documents in caller order with bounded regex/fixed matching, context, byte offsets, and deterministic truncation"
                .to_owned(),
            effect: EffectKind::ReadOnly,
            risk: RiskLevel::Low,
            idempotency: Idempotency::Idempotent,
            input_schema: input_schema(),
        }],
    }
}

fn input_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "documents": {
                "type": "array",
                "minItems": MIN_DOCUMENTS,
                "maxItems": MAX_DOCUMENTS,
                "description": format!(
                    "Caller-fed virtual documents in search order. Paths must be exact-byte unique. Each text is at most {MAX_DOCUMENT_TEXT_BYTES} decoded UTF-8 bytes and aggregate text is at most {MAX_TOTAL_TEXT_BYTES} decoded UTF-8 bytes."
                ),
                "items": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "minLength": 1,
                            "maxLength": MAX_PATH_BYTES,
                            "description": "Opaque relative /-separated label, enforced at 1–256 UTF-8 bytes; never dereferenced. Empty, dot, dot-dot, control, backslash, absolute, drive-prefixed, UNC-like, and empty components are rejected."
                        },
                        "text": {
                            "type": "string",
                            "maxLength": MAX_DOCUMENT_TEXT_BYTES,
                            "description": format!(
                                "Decoded UTF-8 virtual content, at most {MAX_DOCUMENT_TEXT_BYTES} bytes. LF alone terminates lines; CR and BOM bytes are preserved."
                            )
                        }
                    },
                    "required": ["path", "text"],
                    "additionalProperties": false
                }
            },
            "pattern": {
                "type": "string",
                "minLength": 1,
                "maxLength": MAX_PATTERN_BYTES,
                "description": "One Rust-regex pattern or one fixed literal, enforced at 1–4,096 decoded UTF-8 bytes. PCRE2 look-around and backreferences are unsupported."
            },
            "mode": {
                "type": "string",
                "enum": ["regex", "fixed"],
                "default": "regex",
                "description": "Regex syntax, or one literal fixed string."
            },
            "case": {
                "type": "string",
                "enum": ["sensitive", "insensitive", "smart"],
                "default": "sensitive"
            },
            "word": {
                "type": "boolean",
                "default": false,
                "description": "Require ripgrep word-boundary matching; cannot be combined with line."
            },
            "line": {
                "type": "boolean",
                "default": false,
                "description": "Require a whole LF-delimited line; cannot be combined with word or multiline."
            },
            "multiline": {
                "type": "boolean",
                "default": false,
                "description": "Permit explicit matches across LF. Dot still excludes LF unless the Rust pattern enables s; cannot be combined with line or invert."
            },
            "invert": {
                "type": "boolean",
                "default": false,
                "description": "Select non-matching lines; incompatible with multiline."
            },
            "context": {
                "type": "object",
                "properties": {
                    "before": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": MAX_CONTEXT_LINES
                    },
                    "after": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": MAX_CONTEXT_LINES
                    }
                },
                "required": ["before", "after"],
                "additionalProperties": false,
                "description": "Both counts are required when context is supplied; defaults to zero lines on both sides."
            },
            "max_results": {
                "type": "integer",
                "minimum": 1,
                "maximum": MAX_RESULTS,
                "default": DEFAULT_MAX_RESULTS,
                "description": "Maximum returned selected records, not occurrences or context records."
            }
        },
        "required": ["documents", "pattern"],
        "additionalProperties": false,
        "description": format!(
            "Closed semantic input. Runtime limits include {MAX_SUBMATCHES_PER_RESULT} submatches per selected result and a {MAX_SUCCESS_ENVELOPE_BYTES}-byte provider success envelope. The host separately enforces the raw serialized invocation limit."
        )
    })
}

#[cfg(test)]
mod tests {
    use dekopon_provider_sdk::{EffectKind, Idempotency, RiskLevel};

    use super::manifest;

    #[test]
    fn manifest_surface_and_schema_are_exactly_narrow() {
        let manifest = manifest();
        assert_eq!(manifest.id.as_str(), "ripgrep");
        assert!(manifest.command_words.is_empty());
        assert_eq!(manifest.capabilities.len(), 1);
        let capability = &manifest.capabilities[0];
        assert_eq!(capability.id.as_str(), "ripgrep.search");
        assert_eq!(capability.effect, EffectKind::ReadOnly);
        assert_eq!(capability.risk, RiskLevel::Low);
        assert_eq!(capability.idempotency, Idempotency::Idempotent);
        assert_eq!(capability.input_schema["type"], "object");
        assert_eq!(capability.input_schema["additionalProperties"], false);
        assert_eq!(
            capability.input_schema["required"],
            serde_json::json!(["documents", "pattern"])
        );
        assert_eq!(
            capability.input_schema["properties"]["documents"]["items"]["additionalProperties"],
            false
        );
        assert_eq!(
            capability.input_schema["properties"]["context"]["required"],
            serde_json::json!(["before", "after"])
        );
    }
}
