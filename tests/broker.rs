use std::path::PathBuf;

use dekopon_provider_sdk_testkit::{BrokerHostLimits, FakeBroker};
use serde_json::json;

fn component() -> Option<PathBuf> {
    std::env::var_os("DEKOPON_RIPGREP_COMPONENT").map(PathBuf::from)
}

fn cache_directory() -> Result<PathBuf, std::io::Error> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("broker-testkit-compile-cache");
    std::fs::create_dir_all(&directory)?;
    directory.canonicalize()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fake_broker_invokes_concurrently_without_storage_and_enforces_wire_size()
-> Result<(), Box<dyn std::error::Error>> {
    let Some(component) = component() else {
        // Ordinary native tests compile this harness. The component gate sets the variable after
        // producing the ignored release artifact and therefore executes the real broker host.
        return Ok(());
    };
    let defaults = BrokerHostLimits::default();
    assert_eq!(defaults.max_memory_bytes, 64 * 1024 * 1024);
    assert_eq!(defaults.max_input_bytes, 1_048_576);
    assert_eq!(defaults.max_output_bytes, 1_048_576);
    assert_eq!(defaults.max_timeout.as_secs(), 30);
    let release_limits = BrokerHostLimits {
        fuel: 10_000_000,
        ..defaults
    };

    let broker = FakeBroker::builder()
        .component(component)
        .provider("ripgrep")
        // Deliberately no `.storage(...)`: the component has no authority or import to grant.
        .host_limits(release_limits)
        .compile_cache(cache_directory()?)
        .build()
        .await?;

    let first = broker.invoke(
        "ripgrep.search",
        json!({
            "documents": [{"path": "one", "text": "alpha\nbeta\n"}],
            "pattern": "alpha"
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
    assert_eq!(first?["selected_count"], 1);
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

    let stats = broker.registry().metrics().snapshot();
    assert!(stats.fuel_observations >= 3);
    assert!(stats.fuel_consumed > 0);
    Ok(())
}
