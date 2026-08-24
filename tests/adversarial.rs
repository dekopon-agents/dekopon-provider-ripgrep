use dekopon_provider_sdk::{CapabilityId, ComponentResponse, Provider as _};
use dekopon_ripgrep_provider::RipgrepProvider;
use serde_json::{Value, json};

fn capability(name: &str) -> CapabilityId {
    name.parse().expect("valid capability fixture")
}

fn invoke(input: Value) -> Result<Value, dekopon_provider_sdk::ProviderError> {
    RipgrepProvider::invoke(&capability("ripgrep.search"), input)
}

#[test]
fn unsupported_capability_and_pattern_failures_are_static_and_non_reflective() {
    let unsupported = RipgrepProvider::invoke(&capability("ripgrep.other"), json!({"secret": 1}))
        .expect_err("unsupported capability");
    assert_eq!(unsupported.code(), "unsupported-capability");
    assert_eq!(
        unsupported.message(),
        "the ripgrep provider exposes only ripgrep.search"
    );

    for pattern in ["(?=secret-lookaround)", r"(secret)\1", "("] {
        let failure = invoke(json!({
            "documents": [{"path": "secret/path", "text": "secret"}],
            "pattern": pattern
        }))
        .expect_err("unsupported or malformed regex");
        assert_eq!(failure.code(), "invalid-pattern");
        assert_eq!(
            failure.message(),
            "pattern is invalid or exceeds the configured regex complexity limits"
        );
        assert!(!failure.message().contains("secret"));
    }

    let nested = format!("{}a{}", "(".repeat(80), ")".repeat(80));
    let failure = invoke(json!({
        "documents": [{"path": "nested", "text": "a"}],
        "pattern": nested
    }))
    .expect_err("nest limit");
    assert_eq!(failure.code(), "invalid-pattern");

    // A short pattern can still expand into a large Unicode automaton. This fixture is accepted
    // syntactically but exceeds the configured 4 MiB compiled-program limit.
    let failure = invoke(json!({
        "documents": [{"path": "program-limit", "text": "a"}],
        "pattern": r"\p{L}{1000}"
    }))
    .expect_err("compiled regex program limit");
    assert_eq!(failure.code(), "invalid-pattern");
}

#[test]
fn fixed_mode_treats_unsupported_regex_spelling_as_one_literal() {
    let output = invoke(json!({
        "documents": [{"path": "fixed", "text": "(?=x) and (a)\\1\n"}],
        "pattern": "(?=x)",
        "mode": "fixed"
    }))
    .expect("fixed literal");
    assert_eq!(output["selected_count"], 1);
    assert_eq!(output["results"][0]["submatches"][0]["byte_start"], 0);
}

#[test]
fn pattern_and_regex_limits_accept_their_simple_inclusive_boundary() {
    let pattern = "a".repeat(4_096);
    let output = invoke(json!({
        "documents": [{"path": "boundary", "text": ""}],
        "pattern": pattern,
        "mode": "fixed"
    }))
    .expect("4,096-byte literal compiles within configured program limits");
    assert_eq!(output["selected_count"], 0);
}

#[test]
fn max_results_probes_one_more_and_keeps_a_selected_prefix_without_orphans() {
    let output = invoke(json!({
        "documents": [{"path": "many", "text": "before\nhit one\nbetween\nhit two\nafter\n"}],
        "pattern": "hit",
        "context": {"before": 1, "after": 1},
        "max_results": 1
    }))
    .expect("bounded search");
    assert_eq!(output["selected_count"], 1);
    assert_eq!(output["truncated"], true);
    assert_eq!(output["truncation_reasons"], json!(["max_results"]));
    let results = output["results"].as_array().expect("results");
    assert!(results.iter().any(|result| result["kind"] == "match"));
    assert!(results.iter().all(|result| result["text"] != "hit two\n"));
    for context in results.iter().filter(|result| result["kind"] != "match") {
        assert!(context["line_start"].as_u64().unwrap() <= 3);
    }
}

#[test]
fn dense_matches_probe_the_sixty_fifth_submatch_and_return_only_sixty_four() {
    let output = invoke(json!({
        "documents": [{"path": "dense", "text": format!("{}\n", "a".repeat(65))}],
        "pattern": "a"
    }))
    .expect("dense search");
    let result = &output["results"][0];
    assert_eq!(result["submatches"].as_array().unwrap().len(), 64);
    assert_eq!(result["submatches_truncated"], true);
    assert_eq!(output["selected_count"], 1);
    assert_eq!(output["truncation_reasons"], json!(["max_submatches"]));
    assert_eq!(result["submatches"][63]["byte_start"], 63);
    assert_eq!(result["submatches"][63]["byte_end"], 64);
}

#[test]
fn output_limit_returns_complete_records_and_all_reasons_in_fixed_order() {
    let text = "\u{0000}".repeat(100_000);
    let output = invoke(json!({
        "documents": [
            {"path": "one", "text": text},
            {"path": "two", "text": text},
            {"path": "three", "text": text}
        ],
        "pattern": "\u{0000}",
        "mode": "fixed",
        "max_results": 2
    }))
    .expect("bounded output search");

    assert_eq!(output["selected_count"], 1);
    assert_eq!(output["results"].as_array().unwrap().len(), 1);
    assert_eq!(output["results"][0]["path"], "one");
    assert_eq!(
        output["results"][0]["text"].as_str().unwrap().len(),
        100_000
    );
    assert_eq!(
        output["results"][0]["submatches"].as_array().unwrap().len(),
        64
    );
    assert_eq!(
        output["truncation_reasons"],
        json!(["max_results", "max_output_bytes", "max_submatches"])
    );

    let envelope = serde_json::to_vec(&ComponentResponse::Succeeded {
        output: output.clone(),
    })
    .expect("success envelope serializes");
    assert!(envelope.len() <= 1_000_000, "{}", envelope.len());
}

#[test]
fn error_codes_distinguish_semantic_input_from_option_combinations() {
    let invalid = invoke(json!({
        "documents": [{"path": "/not/a/label", "text": "x"}],
        "pattern": "x"
    }))
    .expect_err("invalid path");
    assert_eq!(invalid.code(), "invalid-input");
    assert_eq!(
        invalid.message(),
        "input does not match the closed ripgrep.search schema and decoded limits"
    );

    let options = invoke(json!({
        "documents": [{"path": "a", "text": "x"}],
        "pattern": "x",
        "multiline": true,
        "invert": true
    }))
    .expect_err("unsupported option combination");
    assert_eq!(options.code(), "invalid-options");
}

#[test]
fn repeated_invocation_is_byte_deterministic() {
    let input = json!({
        "documents": [
            {"path": "b", "text": "x x\n"},
            {"path": "a", "text": "x\n"}
        ],
        "pattern": "x",
        "context": {"before": 1, "after": 1}
    });
    let first = serde_json::to_vec(&invoke(input.clone()).expect("first search")).unwrap();
    let second = serde_json::to_vec(&invoke(input).expect("second search")).unwrap();
    assert_eq!(first, second);
}
