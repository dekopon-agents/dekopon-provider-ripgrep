use serde::{Deserialize, Serialize};

pub(crate) const MAX_SUBMATCHES_PER_RESULT: usize = 64;
pub(crate) const MAX_SUCCESS_ENVELOPE_BYTES: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ResultKind {
    Match,
    ContextBefore,
    ContextAfter,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TruncationReason {
    #[serde(rename = "max_results")]
    Results,
    #[serde(rename = "max_output_bytes")]
    OutputBytes,
    #[serde(rename = "max_submatches")]
    Submatches,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Submatch {
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct SearchResult {
    pub(crate) kind: ResultKind,
    pub(crate) path: String,
    pub(crate) text: String,
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
    pub(crate) line_start: usize,
    pub(crate) line_end: usize,
    pub(crate) submatches: Vec<Submatch>,
    pub(crate) submatches_truncated: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct SearchOutput {
    pub(crate) results: Vec<SearchResult>,
    pub(crate) selected_count: usize,
    pub(crate) truncated: bool,
    pub(crate) truncation_reasons: Vec<TruncationReason>,
}

pub(crate) fn ordered_reasons(
    max_results: bool,
    max_output_bytes: bool,
    max_submatches: bool,
) -> Vec<TruncationReason> {
    let mut reasons = Vec::with_capacity(3);
    if max_results {
        reasons.push(TruncationReason::Results);
    }
    if max_output_bytes {
        reasons.push(TruncationReason::OutputBytes);
    }
    if max_submatches {
        reasons.push(TruncationReason::Submatches);
    }
    reasons
}

pub(crate) fn success_envelope_len(
    encoded_results_bytes: usize,
    result_count: usize,
    selected_count: usize,
    reasons: &[TruncationReason],
) -> usize {
    const PREFIX: usize = br#"{"outcome":"succeeded","output":{"results":["#.len();
    const AFTER_RESULTS: usize = br#"],"selected_count":"#.len();
    const AFTER_SELECTED: usize = br#", "truncated":"#.len() - 1;
    const AFTER_TRUNCATED: usize = br#", "truncation_reasons":"#.len() - 1;
    const SUFFIX: usize = b"}}".len();

    let result_commas = result_count.saturating_sub(1);
    let reasons_len = serde_json::to_vec(reasons)
        .expect("static truncation reasons always serialize")
        .len();
    PREFIX
        + encoded_results_bytes
        + result_commas
        + AFTER_RESULTS
        + decimal_len(selected_count)
        + AFTER_SELECTED
        + if reasons.is_empty() { 5 } else { 4 }
        + AFTER_TRUNCATED
        + reasons_len
        + SUFFIX
}

const fn decimal_len(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

#[cfg(test)]
mod tests {
    use dekopon_provider_sdk::ComponentResponse;
    use serde_json::json;

    use super::{
        ResultKind, SearchOutput, SearchResult, Submatch, ordered_reasons, success_envelope_len,
    };

    #[test]
    fn envelope_length_accounting_matches_the_sdk_wire_shape() {
        for (results, max_results, max_output, max_submatches) in [
            (Vec::new(), false, false, false),
            (
                vec![SearchResult {
                    kind: ResultKind::Match,
                    path: "a/é".to_owned(),
                    text: "x\u{0000}\n".to_owned(),
                    byte_start: 0,
                    byte_end: 3,
                    line_start: 1,
                    line_end: 1,
                    submatches: vec![Submatch {
                        byte_start: 0,
                        byte_end: 1,
                    }],
                    submatches_truncated: true,
                }],
                true,
                true,
                true,
            ),
        ] {
            let reasons = ordered_reasons(max_results, max_output, max_submatches);
            let output = SearchOutput {
                selected_count: results
                    .iter()
                    .filter(|result| result.kind == ResultKind::Match)
                    .count(),
                truncated: !reasons.is_empty(),
                truncation_reasons: reasons.clone(),
                results,
            };
            let encoded_results_bytes = output
                .results
                .iter()
                .map(|result| serde_json::to_vec(result).expect("result serializes").len())
                .sum();
            let calculated = success_envelope_len(
                encoded_results_bytes,
                output.results.len(),
                output.selected_count,
                &reasons,
            );
            let actual = serde_json::to_vec(&ComponentResponse::Succeeded {
                output: serde_json::to_value(&output).expect("output converts to Value"),
            })
            .expect("SDK envelope serializes")
            .len();
            assert_eq!(calculated, actual, "{}", json!(output));
        }
    }
}
