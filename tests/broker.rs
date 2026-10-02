use dekopon_provider_sdk::{CommandRunOutcome, provider};
use dekopon_provider_sdk_testkit::{BrokerHostLimits, Harness, conformance};
use dekopon_ripgrep_provider::RipgrepProvider;
use serde_json::json;
use std::path::PathBuf;

type TestResult = Result<(), Box<dyn std::error::Error>>;
fn component() -> PathBuf {
    std::env::var_os("DEKOPON_PROVIDER_COMPONENT")
        .expect("DEKOPON_PROVIDER_COMPONENT must point at a freshly built component")
        .into()
}
#[test]
fn checked_component_conforms_and_command_proposal_preserves_authority() -> TestResult {
    let path = component();
    conformance::<RipgrepProvider>(&path)?;
    let manifest = provider::manifest::<RipgrepProvider>()?;
    assert_eq!(manifest.id.as_str(), "ripgrep");
    assert_eq!(manifest.capabilities[0].id.as_str(), "ripgrep.search");
    assert_eq!(
        manifest.capabilities[0].effect,
        dekopon_provider_sdk::EffectKind::ReadOnly
    );
    let CommandRunOutcome::Proposed {
        capability,
        input,
        secret_use,
    } = provider::command::<RipgrepProvider>(&["-i".into(), "needle".into()], true)
    else {
        panic!("proposal")
    };
    assert_eq!(capability.as_str(), "ripgrep.search");
    assert!(secret_use.is_none());
    assert_eq!(input["pattern"], "needle");
    assert!(input.get("documents").is_none());
    Ok(())
}
#[test]
fn host_limits_and_closed_schema_still_bind() -> TestResult {
    let limits = BrokerHostLimits::default();
    assert_eq!(limits.max_memory_bytes, 64 * 1024 * 1024);
    assert_eq!(limits.max_input_bytes, 1_048_576);
    let rejected = Harness::<RipgrepProvider>::get(component())
        .stdin(b"needle\n".to_vec())
        .call("ripgrep.search", json!({"pattern":"needle","documents":[]}))?;
    assert_eq!(rejected.status, 2);
    assert!(rejected.stdout.is_empty());
    let failed = Harness::<RipgrepProvider>::get(component())
        .host_limits(BrokerHostLimits {
            fuel: 1_000,
            ..limits
        })
        .stdin(b"needle\n".repeat(1000))
        .call("ripgrep.search", json!({"pattern":"needle"}));
    assert!(
        failed.is_err(),
        "fuel refusal is a host error, not guest exit"
    );
    Ok(())
}
