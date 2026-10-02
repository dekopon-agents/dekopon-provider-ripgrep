use dekopon_provider_sdk_testkit::{Native, NativeOutput};
use dekopon_ripgrep_provider::RipgrepProvider;
use serde_json::{Value, json};

fn run(input: Value, bytes: &[u8]) -> NativeOutput {
    Native::<RipgrepProvider>::new()
        .stdin(bytes.to_vec())
        .call("ripgrep.search", &input.to_string())
}
#[test]
fn matching_lines_preserve_order_and_original_bytes() {
    let result = run(
        json!({"pattern":"needle"}),
        b"other\nneedle one\nneedle two\n",
    );
    assert_eq!(result.status, 0);
    assert_eq!(result.stdout, b"needle one\nneedle two\n");
    assert!(result.stderr.is_empty());
}
#[test]
fn no_match_is_guest_status_one_not_an_empty_success() {
    let result = run(json!({"pattern":"needle"}), b"other\n");
    assert_eq!(result.status, 1);
    assert!(result.stdout.is_empty());
}
#[test]
fn empty_pipe_and_absent_pipe_are_usage_two() {
    for native in [
        Native::<RipgrepProvider>::new().stdin(Vec::new()),
        Native::new(),
    ] {
        let result = native.call("ripgrep.search", &json!({"pattern":"a"}).to_string());
        assert_eq!(result.status, 2);
        assert!(result.stdout.is_empty());
    }
}
#[test]
fn context_invert_and_fixed_match_stream_text() {
    let result = run(
        json!({"pattern":"needle","context":{"before":1,"after":1}}),
        b"before\nneedle\nafter\n",
    );
    assert_eq!(result.status, 0);
    assert_eq!(result.stdout, b"before\nneedle\nafter\n");
    let result = run(
        json!({"pattern":"x","mode":"fixed","invert":true}),
        b"x\ny\n",
    );
    assert_eq!(result.stdout, b"y\n");
}
#[test]
fn closed_schema_and_invalid_options_exit_without_search() {
    for input in [
        json!({"pattern":"x","documents":[]}),
        json!({"pattern":"x","word":true,"line":true}),
    ] {
        let result = run(input, b"x\n");
        assert_ne!(result.status, 0);
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn downstream_closed_stdout_exits_141_without_error_text() {
    use dekopon_provider_sdk::provider::{NativeStdio, invoke_native};
    use std::io::{self, Write};
    struct Closed;
    impl Write for Closed {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let exit = invoke_native::<RipgrepProvider>(
        "ripgrep.search",
        &json!({"pattern":"needle"}).to_string(),
        NativeStdio {
            stdin: Some(Box::new(&b"needle\nmore\n"[..])),
            stdout: Box::new(Closed),
        },
    );
    assert_eq!(exit.status, 141);
    assert!(exit.stderr.is_empty());
}
