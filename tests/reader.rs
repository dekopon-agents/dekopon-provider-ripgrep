use dekopon_provider_sdk::provider::{NativeStdio, invoke_native};
use dekopon_ripgrep_provider::RipgrepProvider;
use serde_json::json;
use std::{
    io::{self, Read, Write},
    sync::{Arc, Mutex},
};

struct Chunks {
    bytes: Vec<u8>,
    at: usize,
}
impl Read for Chunks {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.at == self.bytes.len() {
            return Ok(0);
        }
        let n = buf.len().min(3).min(self.bytes.len() - self.at);
        buf[..n].copy_from_slice(&self.bytes[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}
#[derive(Clone)]
struct Capture(Arc<Mutex<Vec<u8>>>);
impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn chunked_reader_preserves_line_order_without_a_value_result() {
    let capture = Capture(Arc::new(Mutex::new(Vec::new())));
    let mut bytes = b"noise\n".repeat(25_000);
    bytes.extend_from_slice(b"needle 1\nnoise\nneedle 2\n");
    let exit = invoke_native::<RipgrepProvider>(
        "ripgrep.search",
        &json!({"pattern":"needle"}).to_string(),
        NativeStdio {
            stdin: Some(Box::new(Chunks { bytes, at: 0 })),
            stdout: Box::new(capture.clone()),
        },
    );
    assert_eq!(exit.status, 0);
    assert_eq!(*capture.0.lock().unwrap(), b"needle 1\nneedle 2\n");
}

#[test]
fn zero_byte_reader_and_no_reader_are_usage_exits() {
    for stdin in [Some(Box::new(io::empty()) as Box<dyn Read>), None] {
        let exit = invoke_native::<RipgrepProvider>(
            "ripgrep.search",
            &json!({"pattern":"needle"}).to_string(),
            NativeStdio {
                stdin,
                stdout: Box::new(io::sink()),
            },
        );
        assert_eq!(exit.status, 2);
    }
}

#[test]
fn no_match_is_a_guest_exit_one() {
    let exit = invoke_native::<RipgrepProvider>(
        "ripgrep.search",
        &json!({"pattern":"needle"}).to_string(),
        NativeStdio {
            stdin: Some(Box::new(&b"miss\n"[..])),
            stdout: Box::new(io::sink()),
        },
    );
    assert_eq!(exit.status, 1);
}

#[test]
fn head_closing_after_five_lines_exits_141_upstream() {
    struct Head {
        lines: usize,
        captured: Vec<u8>,
    }
    impl Write for Head {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.lines == 5 {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            self.lines += bytes.iter().filter(|&&b| b == b'\n').count();
            self.captured.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let exit = invoke_native::<RipgrepProvider>(
        "ripgrep.search",
        &json!({"pattern":"needle"}).to_string(),
        NativeStdio {
            stdin: Some(Box::new(Chunks {
                bytes: b"needle\n".repeat(100),
                at: 0,
            })),
            stdout: Box::new(Head {
                lines: 0,
                captured: Vec::new(),
            }),
        },
    );
    assert_eq!(exit.status, 141);
    assert!(exit.stderr.is_empty());
}

#[test]
fn maximum_selected_lines_keeps_trailing_context_but_not_a_second_match() {
    let capture = Capture(Arc::new(Mutex::new(Vec::new())));
    let exit = invoke_native::<RipgrepProvider>(
        "ripgrep.search",
        &json!({"pattern":"needle","max_results":1,"context":{"before":1,"after":1}}).to_string(),
        NativeStdio {
            stdin: Some(Box::new(
                &b"before\nneedle one\nafter\ngap\nneedle two\n"[..],
            )),
            stdout: Box::new(capture.clone()),
        },
    );
    assert_eq!(exit.status, 0);
    assert_eq!(*capture.0.lock().unwrap(), b"before\nneedle one\nafter\n");
}
