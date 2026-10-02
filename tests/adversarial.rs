mod support;
use serde_json::json;
use support::{failure, invoke};

#[test]
fn unsupported_capability_and_pattern_failures_are_static_and_non_reflective() {
    let (status, stderr) = failure("ripgrep.other", json!({"secret":1}));
    assert_eq!(status, 1);
    assert_eq!(stderr, "the provider has no such capability\n");
    for pattern in [
        "(?=secret-lookaround)",
        r"(secret)\1",
        "(",
        &format!("{}a{}", "(".repeat(80), ")".repeat(80)),
        r"\p{L}{1000}",
    ] {
        let (status, stderr) = failure(
            "ripgrep.search",
            json!({"documents":[{"path":"secret/path","text":"secret"}],"pattern":pattern}),
        );
        assert_eq!(status, 1);
        assert_eq!(
            stderr,
            "pattern is invalid or exceeds the configured regex complexity limits\n"
        );
        assert!(!stderr.contains("secret"));
    }
}

#[test]
fn fixed_mode_treats_unsupported_regex_spelling_as_one_literal() {
    let output = invoke(
        json!({"documents":[{"path":"fixed","text":"(?=x) and (a)\\1\n"}],
        "pattern":"(?=x)","mode":"fixed"}),
    );
    assert_eq!(output["selected_count"], 1);
    assert_eq!(output["results"][0]["submatches"][0]["byte_start"], 0);
}

#[test]
fn pattern_and_regex_limits_accept_their_simple_inclusive_boundary() {
    let output = invoke(json!({"documents":[{"path":"boundary","text":""}],
        "pattern":"a".repeat(4096),"mode":"fixed"}));
    assert_eq!(output["selected_count"], 0);
}

#[test]
fn max_results_probes_one_more_and_keeps_a_selected_prefix_without_orphans() {
    let output = invoke(
        json!({"documents":[{"path":"many","text":"before\nhit one\nbetween\nhit two\nafter\n"}],
        "pattern":"hit","context":{"before":1,"after":1},"max_results":1}),
    );
    assert_eq!(output["selected_count"], 1);
    assert_eq!(output["truncated"], true);
    assert_eq!(output["truncation_reasons"], json!(["max_results"]));
    let results = output["results"].as_array().unwrap();
    assert!(results.iter().any(|result| result["kind"] == "match"));
    assert!(results.iter().all(|result| result["text"] != "hit two\n"));
    for context in results.iter().filter(|result| result["kind"] != "match") {
        assert!(context["line_start"].as_u64().unwrap() <= 3);
    }
}

#[test]
fn dense_matches_probe_the_sixty_fifth_submatch_and_return_only_sixty_four() {
    let output = invoke(
        json!({"documents":[{"path":"dense","text":format!("{}\n","a".repeat(65))}],"pattern":"a"}),
    );
    let result = &output["results"][0];
    assert_eq!(result["submatches"].as_array().unwrap().len(), 64);
    assert_eq!(result["submatches_truncated"], true);
    assert_eq!(result["submatches"][63]["byte_start"], 63);
    assert_eq!(result["submatches"][63]["byte_end"], 64);
    assert_eq!(output["selected_count"], 1);
    assert_eq!(output["truncation_reasons"], json!(["max_submatches"]));
}

#[test]
fn output_limit_returns_complete_records_and_all_reasons_in_fixed_order() {
    let text = "\u{0000}".repeat(100_000);
    let output = invoke(json!({"documents":[
        {"path":"one","text":text},{"path":"two","text":text},{"path":"three","text":text}],
        "pattern":"\u{0000}","mode":"fixed","max_results":2}));
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
    let stdout = serde_json::to_vec(&output).unwrap();
    assert!(stdout.len() <= 1_000_000, "{}", stdout.len());
}

#[test]
fn error_codes_distinguish_semantic_input_from_option_combinations() {
    let (status, stderr) = failure(
        "ripgrep.search",
        json!({"documents":[{"path":"/not/a/label","text":"x"}],"pattern":"x"}),
    );
    assert_eq!(status, 2);
    assert_eq!(
        stderr,
        "input does not match the closed ripgrep.search schema and decoded limits\n"
    );
    let (status, stderr) = failure(
        "ripgrep.search",
        json!({"documents":[{"path":"a","text":"x"}],"pattern":"x",
        "multiline":true,"invert":true}),
    );
    assert_eq!(status, 1);
    assert_eq!(
        stderr,
        "the requested search option combination is not supported\n"
    );
}

#[test]
fn repeated_invocation_is_byte_deterministic() {
    let input = json!({"documents":[{"path":"b","text":"x x\n"},{"path":"a","text":"x\n"}],
        "pattern":"x","context":{"before":1,"after":1}});
    let first = serde_json::to_vec(&invoke(input.clone())).unwrap();
    let second = serde_json::to_vec(&invoke(input)).unwrap();
    assert_eq!(first, second);
}
