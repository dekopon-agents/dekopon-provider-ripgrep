//! Release component driven through the old FakeBroker, before the typed SDK migration.
//! RG-a follow-up replaces this host with typed testkit conformance without dropping gates.
use dekopon_provider_sdk_testkit::{
    BrokerHostLimits, BrokerProviderRegistry, CommandRunOutcome, FakeBroker, FakeBrokerError,
};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const RELEASE_FUEL: u64 = 350_000_000;
const MAX_DOCUMENT_TEXT_BYTES: usize = 131_072;
fn component() -> PathBuf {
    PathBuf::from(
        std::env::var_os("DEKOPON_PROVIDER_COMPONENT")
            .expect("DEKOPON_PROVIDER_COMPONENT must point at the freshly built component"),
    )
}
fn cache_directory() -> Result<PathBuf, std::io::Error> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/broker-testkit-compile-cache");
    std::fs::create_dir_all(&root)?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(std::io::Error::other)?
        .as_nanos();
    let directory = root.join(format!(
        "{}-{nanos}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory)?;
    directory.canonicalize()
}
async fn broker(fuel: u64) -> Result<FakeBroker, FakeBrokerError> {
    FakeBroker::builder()
        .component(component())
        .provider("ripgrep")
        // No `.storage(...)`: this provider has no authority to request or use it.
        .host_limits(BrokerHostLimits {
            fuel,
            ..BrokerHostLimits::default()
        })
        .compile_cache(cache_directory()?)
        .build()
        .await
}
fn widest_documents() -> Value {
    let text = format!("{}\n", "a".repeat(MAX_DOCUMENT_TEXT_BYTES - 1));
    Value::Array(
        (0..6)
            .map(|index| json!({"path":format!("limits/d{index}"), "text":text}))
            .collect(),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_rg_word_renders_in_the_guest_and_proposes_a_search_the_host_runs() -> TestResult {
    let broker = broker(RELEASE_FUEL).await?;
    let CommandRunOutcome::Rendered {
        stdout,
        stderr,
        status,
    } = broker.run_command("rg", &["--help".into()], None).await?
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
    } = broker
        .run_command(
            "rg",
            &["--glob".into(), "*.rs".into(), "alpha".into()],
            Some("alpha\n"),
        )
        .await?
    else {
        panic!("unsupported flag renders");
    };
    assert_eq!(status, 2);
    assert_eq!(stdout, "");
    assert!(stderr.contains("--glob"));
    let CommandRunOutcome::Proposed {
        capability,
        input,
        secret_use: _,
    } = broker
        .run_command(
            "rg",
            &[
                "-i".into(),
                "-A".into(),
                "1".into(),
                "ALPHA".into(),
                "notes/todo.md".into(),
            ],
            Some("alpha\nbeta\ngamma\n"),
        )
        .await?
    else {
        panic!("piped search proposes");
    };
    assert_eq!(capability.as_str(), "ripgrep.search");
    let output = broker.invoke(capability.as_str(), input).await?;
    assert_eq!(output["selected_count"], 1);
    assert_eq!(output["results"][0]["path"], "notes/todo.md");
    assert_eq!(output["results"][1]["text"], "beta\n");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_invocations_need_no_storage_and_the_host_bounds_the_wire() -> TestResult {
    let defaults = BrokerHostLimits::default();
    assert_eq!(defaults.max_memory_bytes, 64 * 1024 * 1024);
    assert_eq!(defaults.max_input_bytes, 1_048_576);
    assert_eq!(defaults.max_output_bytes, 1_048_576);
    assert_eq!(defaults.max_timeout.as_secs(), 30);
    let broker = broker(RELEASE_FUEL).await?;
    let inputs = [
        json!({"documents":[{"path":"one","text":"alpha\nbeta\nalpha\n"}],"pattern":"alpha","max_results":2}),
        json!({"documents":[{"path":"two","text":"δ\nΔ\n"}],"pattern":"δ","case":"insensitive"}),
        json!({"documents":[{"path":"three","text":"x\ny\n"}],"pattern":"x","invert":true}),
    ];
    let (first, second, third) = tokio::join!(
        broker.invoke("ripgrep.search", inputs[0].clone()),
        broker.invoke("ripgrep.search", inputs[1].clone()),
        broker.invoke("ripgrep.search", inputs[2].clone()),
    );
    let first = first?;
    assert_eq!(first["selected_count"], 2);
    assert_eq!(first["truncated"], false);
    assert_eq!(first["results"][0]["line_start"], 1);
    assert_eq!(first["results"][1]["line_start"], 3);
    assert_eq!(second?["selected_count"], 2);
    assert_eq!(third?["results"][0]["text"], "y\n");
    let escaped = "\u{0000}".repeat(100_000);
    let failure = broker.invoke("ripgrep.search", json!({"documents":[
        {"path":"escaped-one","text":escaped},{"path":"escaped-two","text":escaped}],"pattern":"x"}))
        .await.expect_err("serialized input >1 MiB rejected by host");
    assert!(failure.provider_failure().is_none());
    let detail = failure.to_string().to_lowercase();
    assert!(
        detail.contains("input") && (detail.contains("large") || detail.contains("maximum")),
        "{detail}"
    );
    let refused = broker
        .invoke(
            "ripgrep.search",
            json!({"documents":[{"path":"bad","text":"x"}],
        "pattern":"x","extra":true}),
        )
        .await
        .expect_err("closed schema");
    assert_eq!(
        refused.provider_failure().expect("guest refusal").0,
        "invalid-input"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn release_fuel_covers_the_widest_scan_and_the_widest_output() -> TestResult {
    let broker = broker(RELEASE_FUEL).await?;
    let documents = widest_documents();
    let scanned = broker
        .invoke(
            "ripgrep.search",
            json!({"documents":documents,"pattern":"z","mode":"fixed"}),
        )
        .await?;
    assert_eq!(scanned["selected_count"], 0);
    assert_eq!(scanned["results"], json!([]));
    let widest = broker
        .invoke(
            "ripgrep.search",
            json!({"documents":documents,"pattern":"a+","max_results":6}),
        )
        .await?;
    assert_eq!(widest["selected_count"], 6);
    for result in widest["results"].as_array().expect("results") {
        assert_eq!(
            result["text"].as_str().expect("text").len(),
            MAX_DOCUMENT_TEXT_BYTES
        );
    }
    assert_eq!(widest["truncated"], false);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_far_smaller_budget_still_binds_the_widest_scan() -> TestResult {
    let failure = broker(1_000_000)
        .await?
        .invoke(
            "ripgrep.search",
            json!({"documents":widest_documents(),"pattern":"z","mode":"fixed"}),
        )
        .await
        .expect_err("1M fuel cannot scan the widest input");
    assert!(failure.provider_failure().is_none());
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disjoint_context_fits_inside_ten_million_fuel() -> TestResult {
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
    let output = broker(10_000_000)
        .await?
        .invoke(
            "ripgrep.search",
            json!({
        "documents":[{"path":"limits/context","text":text}],"pattern":"m","mode":"fixed",
        "context":{"before":8,"after":8},"max_results":25}),
        )
        .await?;
    assert_eq!(output["selected_count"], 25);
    assert_eq!(output["results"].as_array().unwrap().len(), 409);
    assert_eq!(output["truncated"], false);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_thousand_results_fit_inside_thirty_million_fuel() -> TestResult {
    let output = broker(30_000_000)
        .await?
        .invoke(
            "ripgrep.search",
            json!({
        "documents":[{"path":"limits/thousand","text":"x\n".repeat(1_000)}],
        "pattern":"x","mode":"fixed","max_results":1_000}),
        )
        .await?;
    assert_eq!(output["selected_count"], 1_000);
    assert_eq!(output["results"].as_array().unwrap().len(), 1_000);
    assert_eq!(output["truncated"], false);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_multiline_block_and_dense_matching_stay_bounded() -> TestResult {
    let broker = broker(RELEASE_FUEL).await?;
    let multiline = format!("a{}z\n", "m".repeat(32_765));
    let block = broker
        .invoke(
            "ripgrep.search",
            json!({"documents":[{"path":"limits/multiline","text":multiline}],
        "pattern":"(?s:a.*z)","multiline":true,"max_results":1}),
        )
        .await?;
    assert_eq!(block["selected_count"], 1);
    assert_eq!(block["results"][0]["text"].as_str().unwrap().len(), 32_768);
    assert_eq!(block["results"][0]["byte_start"], 0);
    assert_eq!(block["results"][0]["byte_end"], 32_768);
    let dense = broker
        .invoke(
            "ripgrep.search",
            json!({"documents":[{"path":"limits/dense","text":"a".repeat(32_768)}],
        "pattern":"a","max_results":1}),
        )
        .await?;
    assert_eq!(dense["selected_count"], 1);
    assert_eq!(
        dense["results"][0]["submatches"].as_array().unwrap().len(),
        64
    );
    assert_eq!(dense["results"][0]["submatches_truncated"], true);
    assert_eq!(dense["truncation_reasons"], json!(["max_submatches"]));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn raw_smoke_describe_search_invalid_input_and_help_text() -> TestResult {
    let registry = BrokerProviderRegistry::load([component()], BrokerHostLimits::default()).await?;
    let manifests: Vec<_> = registry.manifests().collect();
    assert_eq!(manifests.len(), 1);
    assert_eq!(manifests[0].id.as_str(), "ripgrep");
    assert_eq!(manifests[0].command_words, ["rg"]);
    assert_eq!(
        manifests[0]
            .capabilities
            .iter()
            .map(|cap| cap.id.as_str())
            .collect::<Vec<_>>(),
        ["ripgrep.search"]
    );
    let broker = broker(RELEASE_FUEL).await?;
    let matched = broker
        .invoke(
            "ripgrep.search",
            json!({"documents":[{"path":"raw","text":"hit\n"}],"pattern":"hit"}),
        )
        .await?;
    assert_eq!(matched["selected_count"], 1);
    let rejected = broker
        .invoke(
            "ripgrep.search",
            json!({"documents":[{"path":"raw","text":"hit"}],
        "pattern":"hit","unknown":true}),
        )
        .await
        .expect_err("closed schema");
    assert_eq!(
        rejected.provider_failure().expect("guest refusal").0,
        "invalid-input"
    );
    let CommandRunOutcome::Rendered {
        stdout,
        stderr,
        status,
    } = broker.run_command("rg", &["--help".into()], None).await?
    else {
        panic!("help");
    };
    assert_eq!(status, 0);
    assert_eq!(stderr, "");
    assert!(stdout.contains("Usage: rg"));
    Ok(())
}
