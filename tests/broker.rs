//! Component-host gates: the compiled provider driven by the real broker host.
//!
//! Every case here needs the release artifact. `DEKOPON_PROVIDER_COMPONENT` is required and must
//! point at a freshly built component; the shared `provider-workflows` CI builds it before
//! running this suite.

use std::path::PathBuf;

use dekopon_provider_sdk_testkit::{
    BrokerHostLimits, BrokerProviderRegistry, CommandRunOutcome, FakeBroker, FakeBrokerError,
};
use serde_json::{Value, json};

/// The fuel a release deployment supplies, and the ceiling every bounded workload below fits in.
const RELEASE_FUEL: u64 = 350_000_000;

/// The widest decoded aggregate this provider accepts: six maximum-length documents.
const MAX_DOCUMENT_TEXT_BYTES: usize = 131_072;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn component() -> PathBuf {
    PathBuf::from(
        std::env::var_os("DEKOPON_PROVIDER_COMPONENT")
            .expect("DEKOPON_PROVIDER_COMPONENT must point at the built component"),
    )
}

fn cache_directory() -> Result<PathBuf, std::io::Error> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("broker-testkit-compile-cache");
    std::fs::create_dir_all(&directory)?;
    directory.canonicalize()
}

/// Loads the component under a named fuel ceiling and otherwise untouched deployment limits.
async fn broker(component: PathBuf, fuel: u64) -> Result<FakeBroker, FakeBrokerError> {
    let limits = BrokerHostLimits {
        fuel,
        ..BrokerHostLimits::default()
    };
    FakeBroker::builder()
        .component(component)
        .provider("ripgrep")
        // Deliberately no `.storage(...)`: the component has no authority or import to grant.
        .host_limits(limits)
        .compile_cache(cache_directory()?)
        .build()
        .await
}

/// Six maximum-length single-line documents: 786,432 decoded bytes, the aggregate ceiling.
fn widest_documents() -> Value {
    let text = format!("{}\n", "a".repeat(MAX_DOCUMENT_TEXT_BYTES - 1));
    Value::Array(
        (0..6)
            .map(|index| json!({"path": format!("limits/d{index}"), "text": text}))
            .collect(),
    )
}

fn argv(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

/// The `rg` word through the broker's own host: help and a usage error render in the guest, and a
/// piped search proposes exactly the input `invoke` then runs.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_rg_word_renders_in_the_guest_and_proposes_a_search_the_host_runs() -> TestResult {
    let component = component();
    let broker = broker(component, RELEASE_FUEL).await?;

    let CommandRunOutcome::Rendered {
        stdout,
        stderr,
        status,
    } = broker.run_command("rg", &argv(&["--help"]), None).await?
    else {
        panic!("rg --help renders");
    };
    assert_eq!(status, 0);
    assert!(
        stdout.contains("Usage: rg [OPTIONS] <PATTERN> [PATH]"),
        "{stdout}"
    );
    assert_eq!(stderr, "");

    let CommandRunOutcome::Rendered {
        stdout,
        stderr,
        status,
    } = broker
        .run_command("rg", &argv(&["--glob", "*.rs", "alpha"]), Some("alpha\n"))
        .await?
    else {
        panic!("an unsupported ripgrep flag renders a usage error");
    };
    assert_eq!(status, 2);
    assert_eq!(stdout, "");
    assert!(stderr.contains("'--glob'"), "{stderr}");

    let CommandRunOutcome::Proposed {
        capability,
        input,
        secret_use: _,
    } = broker
        .run_command(
            "rg",
            &argv(&["-i", "-A", "1", "ALPHA", "notes/todo.md"]),
            Some("alpha\nbeta\ngamma\n"),
        )
        .await?
    else {
        panic!("a piped search proposes");
    };
    assert_eq!(capability.as_str(), "ripgrep.search");
    let output = broker.invoke(capability.as_str(), input).await?;
    assert_eq!(output["selected_count"], 1);
    assert_eq!(output["results"][0]["path"], "notes/todo.md");
    assert_eq!(output["results"][1]["kind"], "context_after");
    assert_eq!(output["results"][1]["text"], "beta\n");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_invocations_need_no_storage_and_the_host_bounds_the_wire() -> TestResult {
    let component = component();
    let defaults = BrokerHostLimits::default();
    assert_eq!(defaults.max_memory_bytes, 64 * 1024 * 1024);
    assert_eq!(defaults.max_input_bytes, 1_048_576);
    assert_eq!(defaults.max_output_bytes, 1_048_576);
    assert_eq!(defaults.max_timeout.as_secs(), 30);

    let broker = broker(component, RELEASE_FUEL).await?;

    let first = broker.invoke(
        "ripgrep.search",
        json!({
            "documents": [{"path": "one", "text": "alpha\nbeta\nalpha\n"}],
            "pattern": "alpha",
            "max_results": 2
        }),
    );
    let second = broker.invoke(
        "ripgrep.search",
        json!({
            "documents": [{"path": "two", "text": "δ\nΔ\n"}],
            "pattern": "δ",
            "case": "insensitive"
        }),
    );
    let third = broker.invoke(
        "ripgrep.search",
        json!({
            "documents": [{"path": "three", "text": "x\ny\n"}],
            "pattern": "x",
            "invert": true
        }),
    );
    let (first, second, third) = tokio::join!(first, second, third);
    let first = first?;
    assert_eq!(first["selected_count"], 2);
    assert_eq!(first["truncated"], false);
    assert_eq!(first["results"][0]["line_start"], 1);
    assert_eq!(first["results"][1]["line_start"], 3);
    assert_eq!(second?["selected_count"], 2);
    assert_eq!(third?["results"][0]["text"], "y\n");

    // Decoded provider limits permit this value, but JSON escaping makes the host serialization
    // larger than 1 MiB. The host must reject it before the provider sees a semantic Value.
    let escaped = "\u{0000}".repeat(100_000);
    let failure = broker
        .invoke(
            "ripgrep.search",
            json!({
                "documents": [
                    {"path": "escaped-one", "text": escaped},
                    {"path": "escaped-two", "text": escaped}
                ],
                "pattern": "x"
            }),
        )
        .await
        .expect_err("default host rejects serialized input over 1 MiB");
    assert!(failure.provider_failure().is_none());
    let detail = failure.to_string().to_ascii_lowercase();
    assert!(
        detail.contains("input") && (detail.contains("large") || detail.contains("maximum")),
        "{detail}"
    );

    // A closed-schema violation is the provider's own refusal, not the host's.
    let refused = broker
        .invoke(
            "ripgrep.search",
            json!({
                "documents": [{"path": "bad", "text": "x"}],
                "pattern": "x",
                "extra": true
            }),
        )
        .await
        .expect_err("the closed ripgrep.search schema rejects an unknown member");
    let (code, _) = refused
        .provider_failure()
        .expect("the guest declared this failure");
    assert_eq!(code, "invalid-input");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn release_fuel_covers_the_widest_scan_and_the_widest_output() -> TestResult {
    let component = component();
    let broker = broker(component, RELEASE_FUEL).await?;
    let documents = widest_documents();

    // Maximum decoded aggregate scanned end to end with no match at all.
    let scanned = broker
        .invoke(
            "ripgrep.search",
            json!({"documents": documents, "pattern": "z", "mode": "fixed"}),
        )
        .await?;
    assert_eq!(scanned["selected_count"], 0);
    assert_eq!(scanned["results"], json!([]));

    // The same aggregate returned as six complete records: a roughly 770 KiB compact response,
    // and the rationale for the bounded release fuel ceiling. The host's own 1 MiB output bound
    // is what makes this succeeding an assertion rather than an observation.
    let widest = broker
        .invoke(
            "ripgrep.search",
            json!({"documents": documents, "pattern": "a+", "max_results": 6}),
        )
        .await?;
    assert_eq!(widest["selected_count"], 6);
    let results = widest["results"].as_array().expect("results array");
    assert_eq!(results.len(), 6);
    for result in results {
        assert_eq!(
            result["text"].as_str().expect("record text").len(),
            MAX_DOCUMENT_TEXT_BYTES
        );
    }
    assert_eq!(widest["truncated"], false);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_far_smaller_budget_still_binds_the_widest_scan() -> TestResult {
    let component = component();
    // 1M fuel against 786,432 bytes of ripgrep scanning: the ceiling is real, and the host — not
    // the guest — stops the invocation.
    let broker = broker(component, 1_000_000).await?;
    let failure = broker
        .invoke(
            "ripgrep.search",
            json!({"documents": widest_documents(), "pattern": "z", "mode": "fixed"}),
        )
        .await
        .expect_err("the widest scan cannot complete on 1,000,000 fuel");
    assert!(failure.provider_failure().is_none());
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disjoint_context_fits_inside_ten_million_fuel() -> TestResult {
    let component = component();
    // 25 selected lines with disjoint eight-line context on each side: 409 output records from an
    // 818-byte document. This is the review regression that previously exhausted 10,000,000 fuel.
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

    let broker = broker(component, 10_000_000).await?;
    let output = broker
        .invoke(
            "ripgrep.search",
            json!({
                "documents": [{"path": "limits/context", "text": text}],
                "pattern": "m",
                "mode": "fixed",
                "context": {"before": 8, "after": 8},
                "max_results": 25
            }),
        )
        .await?;
    assert_eq!(output["selected_count"], 25);
    assert_eq!(output["results"].as_array().expect("results").len(), 409);
    assert_eq!(output["truncated"], false);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_thousand_results_fit_inside_thirty_million_fuel() -> TestResult {
    let component = component();
    let broker = broker(component, 30_000_000).await?;
    let output = broker
        .invoke(
            "ripgrep.search",
            json!({
                "documents": [{"path": "limits/thousand", "text": "x\n".repeat(1_000)}],
                "pattern": "x",
                "mode": "fixed",
                "max_results": 1_000
            }),
        )
        .await?;
    assert_eq!(output["selected_count"], 1_000);
    assert_eq!(output["results"].as_array().expect("results").len(), 1_000);
    assert_eq!(output["truncated"], false);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_multiline_block_and_dense_matching_stay_bounded() -> TestResult {
    let component = component();
    let broker = broker(component, RELEASE_FUEL).await?;

    // One explicit match across LF spanning a full 32 KiB block.
    let multiline = format!("a{}z\n", "m".repeat(32_765));
    let block = broker
        .invoke(
            "ripgrep.search",
            json!({
                "documents": [{"path": "limits/multiline", "text": multiline}],
                "pattern": "(?s:a.*z)",
                "multiline": true,
                "max_results": 1
            }),
        )
        .await?;
    assert_eq!(block["selected_count"], 1);
    assert_eq!(
        block["results"][0]["text"].as_str().expect("text").len(),
        32_768
    );
    assert_eq!(block["results"][0]["byte_start"], 0);
    assert_eq!(block["results"][0]["byte_end"], 32_768);

    // Dense matching must stop submatch enumeration after observing the 65th occurrence.
    let dense = broker
        .invoke(
            "ripgrep.search",
            json!({
                "documents": [{"path": "limits/dense", "text": "a".repeat(32_768)}],
                "pattern": "a",
                "max_results": 1
            }),
        )
        .await?;
    assert_eq!(dense["selected_count"], 1);
    assert_eq!(
        dense["results"][0]["submatches"]
            .as_array()
            .expect("submatches")
            .len(),
        64
    );
    assert_eq!(dense["results"][0]["submatches_truncated"], true);
    assert_eq!(dense["truncation_reasons"], json!(["max_submatches"]));
    Ok(())
}

/// Carries the raw SDK-boundary assertions formerly made by `scripts/test-raw-component.sh`
/// against a real Wasmtime CLI invocation: `describe()` names the provider, its capability, and
/// its command word; a small in-memory search invoke matches; a closed-schema violation is
/// rejected as `invalid-input`; and the `rg` word's `--help` renders usage text. The script's own
/// truncated-JSON and duplicate-key cases exercise the raw WIT `input-json` string boundary, which
/// has no equivalent in this typed `serde_json::Value` API — the unknown-member case below is the
/// script's other invalid-input fixture, and it does exercise the same provider-side rejection.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn raw_smoke_describe_search_invalid_input_and_help_text() -> TestResult {
    let component = component();

    let registry =
        BrokerProviderRegistry::load([component.clone()], BrokerHostLimits::default()).await?;
    let manifests: Vec<_> = registry.manifests().collect();
    assert_eq!(manifests.len(), 1);
    assert_eq!(manifests[0].id.as_str(), "ripgrep");
    assert_eq!(manifests[0].command_words, vec!["rg".to_owned()]);
    let capability_ids: Vec<&str> = manifests[0]
        .capabilities
        .iter()
        .map(|capability| capability.id.as_str())
        .collect();
    assert_eq!(capability_ids, vec!["ripgrep.search"]);

    let broker = broker(component, RELEASE_FUEL).await?;

    let matched = broker
        .invoke(
            "ripgrep.search",
            json!({"documents": [{"path": "raw", "text": "hit\n"}], "pattern": "hit"}),
        )
        .await?;
    assert_eq!(matched["selected_count"], 1);

    let rejected = broker
        .invoke(
            "ripgrep.search",
            json!({
                "documents": [{"path": "raw", "text": "hit"}],
                "pattern": "hit",
                "unknown": true
            }),
        )
        .await
        .expect_err("the closed ripgrep.search schema rejects an unknown member");
    let (code, _) = rejected
        .provider_failure()
        .expect("the guest declared this failure");
    assert_eq!(code, "invalid-input");

    let CommandRunOutcome::Rendered {
        stdout,
        stderr,
        status,
    } = broker.run_command("rg", &argv(&["--help"]), None).await?
    else {
        panic!("rg --help renders");
    };
    assert_eq!(status, 0);
    assert_eq!(stderr, "");
    assert!(
        stdout.contains("Usage: rg [OPTIONS] <PATTERN> [PATH]"),
        "{stdout}"
    );
    Ok(())
}
