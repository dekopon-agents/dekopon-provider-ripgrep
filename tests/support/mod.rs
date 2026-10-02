use dekopon_provider_sdk_testkit::Native;
use dekopon_ripgrep_provider::RipgrepProvider;
use serde_json::Value;

pub fn invoke(input: Value) -> Value {
    let output = Native::<RipgrepProvider>::new().call("ripgrep.search", &input.to_string());
    assert_eq!(output.status, 0, "{}", output.stderr);
    serde_json::from_slice(&output.stdout).expect("temporary RG-a JSON stdout")
}

pub fn failure(capability: &str, input: Value) -> (u8, String) {
    let output = Native::<RipgrepProvider>::new().call(capability, &input.to_string());
    assert_ne!(output.status, 0);
    assert!(output.stdout.is_empty());
    (output.status, output.stderr)
}
