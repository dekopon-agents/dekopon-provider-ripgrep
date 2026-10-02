//! Real component under the typed SDK/testkit host. RG-a still bridges documents as stdout JSON.
use dekopon_broker_host::BrokerHostError;
use dekopon_provider_sdk::{CommandRunOutcome, provider};
use dekopon_provider_sdk_testkit::{BrokerHostLimits, Harness, HarnessError, conformance};
use dekopon_ripgrep_provider::RipgrepProvider;
use serde_json::{Value, json};
use std::path::PathBuf;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const RELEASE_FUEL: u64 = 350_000_000;
const MAX_DOCUMENT_TEXT_BYTES: usize = 131_072;

fn component() -> PathBuf {
    PathBuf::from(
        std::env::var_os("DEKOPON_PROVIDER_COMPONENT")
            .expect("DEKOPON_PROVIDER_COMPONENT must point at the freshly built component"),
    )
}
fn call_with(fuel: u64, input: Value) -> Result<Value, HarnessError> {
    Harness::<RipgrepProvider>::get(component())
        .host_limits(BrokerHostLimits {
            fuel,
            ..BrokerHostLimits::default()
        })
        .call("ripgrep.search", input)
}
fn call(input: Value) -> Result<Value, HarnessError> {
    call_with(RELEASE_FUEL, input)
}
fn guest_failure(error: &HarnessError) -> Option<(u8, &str)> {
    let HarnessError::Invocation(failure) = error else {
        return None;
    };
    match failure.error.as_ref() {
        BrokerHostError::ProviderFailure { status, stderr, .. } => Some((*status, stderr)),
        _ => None,
    }
}
fn widest_documents() -> Value {
    let text = format!("{}\n", "a".repeat(MAX_DOCUMENT_TEXT_BYTES - 1));
    Value::Array(
        (0..6)
            .map(|index| json!({"path":format!("limits/d{index}"),"text":text}))
            .collect(),
    )
}
#[test]
fn the_rg_word_renders_in_the_guest_and_proposes_a_search_the_host_runs() -> TestResult {
    let CommandRunOutcome::Rendered {
        stdout,
        stderr,
        status,
    } = provider::command::<RipgrepProvider>(&["--help".into()], false)
    else {
        panic!("rg help renders");
    };
    assert_eq!(status, 0);
    assert_eq!(stderr, "");
    assert!(stdout.contains("Usage: rg [OPTIONS] <PATTERN> [PATH]"));
    let CommandRunOutcome::Rendered {
        stdout,
        stderr,
        status,
    } = provider::command::<RipgrepProvider>(
        &["--glob".into(), "*.rs".into(), "alpha".into()],
        false,
    )
    else {
        panic!("unsupported flag renders");
    };
    assert_eq!(status, 2);
    assert_eq!(stdout, "");
    assert!(stderr.contains("--glob"));
    let CommandRunOutcome::Proposed {
        capability,
        input,
        secret_use,
    } = provider::command::<RipgrepProvider>(
        &[
            "-i".into(),
            "-A".into(),
            "1".into(),
            "ALPHA".into(),
            "notes/todo.md".into(),
        ],
        true,
    )
    else {
        panic!("piped search proposes");
    };
    assert!(secret_use.is_none());
    assert_eq!(capability.as_str(), "ripgrep.search");
    assert_eq!(input["documents"][0]["path"], "notes/todo.md");
    // The 1b testkit has no stdin setter: exercise the same typed input with document text here.
    let output = call(
        json!({"documents":[{"path":"notes/todo.md","text":"alpha\nbeta\ngamma\n"}],
        "pattern":"ALPHA","case":"insensitive","context":{"before":0,"after":1}}),
    )?;
    assert_eq!(output["selected_count"], 1);
    assert_eq!(output["results"][0]["path"], "notes/todo.md");
    assert_eq!(output["results"][1]["text"], "beta\n");
    Ok(())
}

#[test]
fn concurrent_invocations_need_no_storage_and_the_host_bounds_the_wire() -> TestResult {
    let defaults = BrokerHostLimits::default();
    assert_eq!(defaults.max_memory_bytes, 64 * 1024 * 1024);
    assert_eq!(defaults.max_input_bytes, 1_048_576);
    assert_eq!(defaults.max_output_bytes, 1_048_576);
    assert_eq!(defaults.max_timeout.as_secs(), 30);
    let inputs = [
        json!({"documents":[{"path":"one","text":"alpha\nbeta\nalpha\n"}],"pattern":"alpha","max_results":2}),
        json!({"documents":[{"path":"two","text":"δ\nΔ\n"}],"pattern":"δ","case":"insensitive"}),
        json!({"documents":[{"path":"three","text":"x\ny\n"}],"pattern":"x","invert":true}),
    ];
    let results = std::thread::scope(|scope| {
        let jobs: Vec<_> = inputs
            .into_iter()
            .map(|input| scope.spawn(move || call(input)))
            .collect();
        jobs.into_iter()
            .map(|job| job.join().unwrap())
            .collect::<Vec<_>>()
    });
    let first = results[0].as_ref().unwrap();
    assert_eq!(first["selected_count"], 2);
    assert_eq!(first["truncated"], false);
    assert_eq!(first["results"][0]["line_start"], 1);
    assert_eq!(first["results"][1]["line_start"], 3);
    assert_eq!(results[1].as_ref().unwrap()["selected_count"], 2);
    assert_eq!(results[2].as_ref().unwrap()["results"][0]["text"], "y\n");
    let escaped = "\u{0000}".repeat(100_000);
    let error = call(json!({"documents":[{"path":"a","text":escaped},{"path":"b","text":escaped}],"pattern":"x"}))
        .expect_err("host rejects serialized input >1 MiB");
    assert!(matches!(error, HarnessError::Invocation(_)), "{error:?}");
    assert!(
        guest_failure(&error).is_none(),
        "host refusal, not guest: {error:?}"
    );
    let error = call(json!({"documents":[{"path":"bad","text":"x"}],"pattern":"x","extra":true}))
        .expect_err("closed schema rejects an unknown member");
    let (status, stderr) = guest_failure(&error).expect("guest closed-schema refusal");
    assert_eq!(status, 2);
    assert_eq!(
        stderr,
        "the input does not match the capability's input schema\n"
    );
    Ok(())
}

#[test]
fn release_fuel_covers_the_widest_scan_and_the_widest_output() -> TestResult {
    let documents = widest_documents();
    let scanned = call(json!({"documents":documents,"pattern":"z","mode":"fixed"}))?;
    assert_eq!(scanned["selected_count"], 0);
    assert_eq!(scanned["results"], json!([]));
    let widest = call(json!({"documents":documents,"pattern":"a+","max_results":6}))?;
    assert_eq!(widest["selected_count"], 6);
    for result in widest["results"].as_array().unwrap() {
        assert_eq!(
            result["text"].as_str().unwrap().len(),
            MAX_DOCUMENT_TEXT_BYTES
        );
    }
    assert_eq!(widest["truncated"], false);
    Ok(())
}

#[test]
fn a_far_smaller_budget_still_binds_the_widest_scan() {
    let failure = call_with(
        1_000_000,
        json!({"documents":widest_documents(),"pattern":"z","mode":"fixed"}),
    )
    .expect_err("1M fuel cannot scan the widest input");
    assert!(
        matches!(failure, HarnessError::Invocation(_)),
        "{failure:?}"
    );
    assert!(
        guest_failure(&failure).is_none(),
        "host, not guest: {failure:?}"
    );
}

#[test]
fn disjoint_context_fits_inside_ten_million_fuel() -> TestResult {
    let mut text = String::new();
    for selected in 0..25 {
        text.push_str("m\n");
        if selected != 24 {
            for _ in 0..16 {
                text.push_str("c\n");
            }
        }
    }
    assert_eq!(text.len(), 818);
    let output = call_with(
        10_000_000,
        json!({"documents":[{"path":"limits/context","text":text}],
        "pattern":"m","mode":"fixed","context":{"before":8,"after":8},"max_results":25}),
    )?;
    assert_eq!(output["selected_count"], 25);
    assert_eq!(output["results"].as_array().unwrap().len(), 409);
    assert_eq!(output["truncated"], false);
    Ok(())
}

#[test]
fn a_thousand_results_fit_inside_thirty_million_fuel() -> TestResult {
    let output = call_with(
        30_000_000,
        json!({"documents":[{"path":"limits/thousand","text":"x\n".repeat(1000)}],
        "pattern":"x","mode":"fixed","max_results":1000}),
    )?;
    assert_eq!(output["selected_count"], 1000);
    assert_eq!(output["results"].as_array().unwrap().len(), 1000);
    assert_eq!(output["truncated"], false);
    Ok(())
}

#[test]
fn one_multiline_block_and_dense_matching_stay_bounded() -> TestResult {
    let multiline = format!("a{}z\n", "m".repeat(32_765));
    let block = call(
        json!({"documents":[{"path":"limits/multiline","text":multiline}],
        "pattern":"(?s:a.*z)","multiline":true,"max_results":1}),
    )?;
    assert_eq!(block["results"][0]["text"].as_str().unwrap().len(), 32_768);
    assert_eq!(block["results"][0]["byte_start"], 0);
    assert_eq!(block["results"][0]["byte_end"], 32_768);
    let dense = call(
        json!({"documents":[{"path":"limits/dense","text":"a".repeat(32_768)}],
        "pattern":"a","max_results":1}),
    )?;
    assert_eq!(
        dense["results"][0]["submatches"].as_array().unwrap().len(),
        64
    );
    assert_eq!(dense["results"][0]["submatches_truncated"], true);
    assert_eq!(dense["truncation_reasons"], json!(["max_submatches"]));
    Ok(())
}

#[test]
fn raw_smoke_describe_search_invalid_input_and_help_text() -> TestResult {
    conformance::<RipgrepProvider>(component())?;
    let manifest = provider::manifest::<RipgrepProvider>()?;
    assert_eq!(manifest.id.as_str(), "ripgrep");
    assert_eq!(manifest.command_words, ["rg"]);
    assert_eq!(manifest.capabilities[0].id.as_str(), "ripgrep.search");
    let matched = call(json!({"documents":[{"path":"raw","text":"hit\n"}],"pattern":"hit"}))?;
    assert_eq!(matched["selected_count"], 1);
    let rejected =
        call(json!({"documents":[{"path":"raw","text":"hit"}],"pattern":"hit","unknown":true}))
            .expect_err("unknown member");
    let (status, stderr) = guest_failure(&rejected).expect("guest closed-schema refusal");
    assert_eq!(status, 2);
    assert_eq!(
        stderr,
        "the input does not match the capability's input schema\n"
    );
    let CommandRunOutcome::Rendered {
        stdout,
        stderr,
        status,
    } = provider::command::<RipgrepProvider>(&["--help".into()], false)
    else {
        panic!("help");
    };
    assert_eq!(status, 0);
    assert_eq!(stderr, "");
    assert!(stdout.contains("Usage: rg"));
    Ok(())
}
