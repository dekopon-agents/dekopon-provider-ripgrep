use dekopon_provider_sdk_testkit::Native;
use dekopon_ripgrep_provider::RipgrepProvider;
use serde_json::json;

#[test]
fn chunk_scale_input_does_not_require_a_document_or_json_envelope() {
    let mut bytes = b"miss\n".repeat(200_000);
    bytes.extend_from_slice(b"needle\n");
    let output = Native::<RipgrepProvider>::new()
        .stdin(bytes)
        .call("ripgrep.search", &json!({"pattern":"needle"}).to_string());
    assert_eq!(output.status, 0);
    assert_eq!(output.stdout, b"needle\n");
}
#[test]
fn regex_complexity_and_unsupported_syntax_fail_without_leaking_input() {
    let output = Native::<RipgrepProvider>::new()
        .stdin(b"private\n".to_vec())
        .call(
            "ripgrep.search",
            &json!({"pattern":"(?=private)"}).to_string(),
        );
    assert_eq!(output.status, 2);
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.contains("private"));
}
