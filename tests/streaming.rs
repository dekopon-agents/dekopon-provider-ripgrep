use dekopon_provider_sdk_testkit::{Harness, conformance};
use dekopon_ripgrep_provider::RipgrepProvider;
use serde_json::json;

fn component() -> std::path::PathBuf {
    std::env::var_os("DEKOPON_PROVIDER_COMPONENT")
        .expect("fresh component required")
        .into()
}

#[test]
fn real_component_streams_stdin_lines_and_exits_by_guest_status() {
    let path = component();
    conformance::<RipgrepProvider>(&path).unwrap();
    let output = Harness::<RipgrepProvider>::get(&path)
        .stdin(b"miss\nneedle one\nmiss\nneedle two\n".to_vec())
        .call("ripgrep.search", json!({"pattern":"needle"}))
        .unwrap();
    assert_eq!(output.status, 0);
    assert_eq!(output.stdout, b"needle one\nneedle two\n");
    assert_eq!(output.stderr, "");
}

#[test]
fn real_component_no_match_is_a_guest_exit_not_a_host_failure() {
    let output = Harness::<RipgrepProvider>::get(component())
        .stdin(b"other\n".to_vec())
        .call("ripgrep.search", json!({"pattern":"needle"}))
        .expect("no match must not be a host error");
    assert_eq!(output.status, 1);
    assert!(output.stdout.is_empty());
}

#[test]
fn real_component_zero_byte_eof_and_absent_stdin_are_usage_two() {
    for piped in [false, true] {
        let run = Harness::<RipgrepProvider>::get(component());
        let run = if piped { run.stdin(Vec::new()) } else { run };
        let output = run
            .call("ripgrep.search", json!({"pattern":"needle"}))
            .unwrap();
        assert_eq!(output.status, 2);
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn real_component_large_input_is_searched_without_documents() {
    let mut bytes = b"other\n".repeat(25_000);
    bytes.extend_from_slice(b"needle\n");
    let output = Harness::<RipgrepProvider>::get(component())
        .stdin(bytes)
        .call("ripgrep.search", json!({"pattern":"needle"}))
        .unwrap();
    assert_eq!(output.status, 0);
    assert_eq!(output.stdout, b"needle\n");
}
