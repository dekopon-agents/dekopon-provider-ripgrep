use serde::Serialize;

pub(crate) const MAX_SUBMATCHES_PER_RESULT: usize = 64;
pub(crate) const MAX_SUCCESS_ENVELOPE_BYTES: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ResultKind {
    Match,
    ContextBefore,
    ContextAfter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TruncationReason {
    #[serde(rename = "max_results")]
    Results,
    #[serde(rename = "max_output_bytes")]
    OutputBytes,
    #[serde(rename = "max_submatches")]
    Submatches,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct Submatch {
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct SearchResult<'a> {
    pub(crate) kind: ResultKind,
    pub(crate) path: &'a str,
    pub(crate) text: &'a str,
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
    pub(crate) line_start: usize,
    pub(crate) line_end: usize,
    pub(crate) submatches: Vec<Submatch>,
    pub(crate) submatches_truncated: bool,
}

impl SearchResult<'_> {
    /// Exact compact-JSON length under serde_json's formatter.
    ///
    /// Computing this directly avoids allocating and serializing every prospective record solely
    /// to enforce the output ceiling. Unit tests compare it with serde_json over all field shapes.
    pub(crate) fn encoded_json_len(&self) -> usize {
        let kind_len = match self.kind {
            ResultKind::Match => b"\"match\"".len(),
            ResultKind::ContextBefore => b"\"context_before\"".len(),
            ResultKind::ContextAfter => b"\"context_after\"".len(),
        };
        let mut length = b"{\"kind\":".len()
            + kind_len
            + b",\"path\":".len()
            + json_string_len(self.path)
            + b",\"text\":".len()
            + json_string_len(self.text)
            + b",\"byte_start\":".len()
            + decimal_len(self.byte_start)
            + b",\"byte_end\":".len()
            + decimal_len(self.byte_end)
            + b",\"line_start\":".len()
            + decimal_len(self.line_start)
            + b",\"line_end\":".len()
            + decimal_len(self.line_end)
            + b",\"submatches\":[".len();

        for (index, submatch) in self.submatches.iter().enumerate() {
            if index != 0 {
                length += 1;
            }
            length += b"{\"byte_start\":".len()
                + decimal_len(submatch.byte_start)
                + b",\"byte_end\":".len()
                + decimal_len(submatch.byte_end)
                + 1;
        }

        length
            + b"],\"submatches_truncated\":".len()
            + if self.submatches_truncated { 4 } else { 5 }
            + 1
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct SearchOutput<'a> {
    pub(crate) results: Vec<SearchResult<'a>>,
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

fn json_string_len(value: &str) -> usize {
    value.as_bytes().iter().fold(2usize, |length, byte| {
        length
            + match byte {
                b'"' | b'\\' | b'\x08' | b'\t' | b'\n' | b'\x0c' | b'\r' => 2,
                0..=0x1f => 6,
                _ => 1,
            }
    })
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
    fn record_and_envelope_length_accounting_match_the_sdk_wire_shape() {
        let samples = [
            SearchResult {
                kind: ResultKind::Match,
                path: "a/é",
                text: "x\u{0000}\n\t\r\u{0085}\"\\",
                byte_start: 0,
                byte_end: 3,
                line_start: 1,
                line_end: 1,
                submatches: vec![Submatch {
                    byte_start: 0,
                    byte_end: 1,
                }],
                submatches_truncated: true,
            },
            SearchResult {
                kind: ResultKind::ContextBefore,
                path: "context",
                text: "before\n",
                byte_start: 123_456,
                byte_end: 123_463,
                line_start: 999,
                line_end: 999,
                submatches: Vec::new(),
                submatches_truncated: false,
            },
            SearchResult {
                kind: ResultKind::ContextAfter,
                path: "context",
                text: "after",
                byte_start: 123_464,
                byte_end: 123_469,
                line_start: 1_000,
                line_end: 1_000,
                submatches: Vec::new(),
                submatches_truncated: false,
            },
        ];
        for sample in &samples {
            assert_eq!(
                sample.encoded_json_len(),
                serde_json::to_vec(sample).expect("result serializes").len()
            );
        }

        for (results, max_results, max_output, max_submatches) in [
            (Vec::new(), false, false, false),
            (samples.to_vec(), true, true, true),
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
                .map(SearchResult::encoded_json_len)
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
