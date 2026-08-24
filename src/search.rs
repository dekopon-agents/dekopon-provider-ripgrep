use std::{collections::HashSet, io};

use dekopon_provider_sdk::{ComponentResponse, ProviderError};
use grep_matcher::{LineTerminator, Matcher};
use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::{
    BinaryDetection, MmapChoice, Searcher, SearcherBuilder, Sink, SinkContext, SinkContextKind,
    SinkMatch,
};

use crate::{
    error,
    input::{CaseMode, MAX_DOCUMENT_TEXT_BYTES, SearchInput, SearchMode},
    output::{
        MAX_SUBMATCHES_PER_RESULT, MAX_SUCCESS_ENVELOPE_BYTES, ResultKind, SearchOutput,
        SearchResult, Submatch, ordered_reasons, success_envelope_len,
    },
};

const REGEX_PROGRAM_BYTES: usize = 4 * 1024 * 1024;
const DFA_CACHE_BYTES: usize = 2 * 1024 * 1024;
const REGEX_NEST_LIMIT: u32 = 64;

#[derive(Debug)]
struct Candidate {
    document_index: usize,
    record: SearchResult,
}

impl Candidate {
    fn selected(&self) -> bool {
        self.record.kind == ResultKind::Match
    }
}

struct CollectSink<'a> {
    matcher: &'a RegexMatcher,
    document_index: usize,
    path: &'a str,
    invert: bool,
    max_results: usize,
    selected_seen: &'a mut usize,
    max_results_probed: &'a mut bool,
    candidates: &'a mut Vec<Candidate>,
}

impl Sink for CollectSink<'_> {
    type Error = io::Error;

    fn matched(&mut self, searcher: &Searcher, mat: &SinkMatch<'_>) -> Result<bool, Self::Error> {
        *self.selected_seen += 1;
        if *self.selected_seen > self.max_results {
            *self.max_results_probed = true;
            return Ok(false);
        }

        let byte_start = usize::try_from(mat.absolute_byte_offset())
            .map_err(|_| io::Error::other("match offset overflow"))?;
        let byte_end = byte_start
            .checked_add(mat.bytes().len())
            .ok_or_else(|| io::Error::other("match range overflow"))?;
        let line_start = usize::try_from(
            mat.line_number()
                .ok_or_else(|| io::Error::other("missing match line number"))?,
        )
        .map_err(|_| io::Error::other("match line overflow"))?;
        let line_end = line_end(line_start, mat.bytes())?;
        let text = String::from_utf8(mat.bytes().to_vec())
            .map_err(|_| io::Error::other("searcher changed UTF-8 input"))?;
        let (submatches, submatches_truncated) = if self.invert {
            (Vec::new(), false)
        } else {
            collect_submatches(searcher, self.matcher, mat)?
        };

        self.candidates.push(Candidate {
            document_index: self.document_index,
            record: SearchResult {
                kind: ResultKind::Match,
                path: self.path.to_owned(),
                text,
                byte_start,
                byte_end,
                line_start,
                line_end,
                submatches,
                submatches_truncated,
            },
        });
        Ok(true)
    }

    fn context(
        &mut self,
        _searcher: &Searcher,
        context: &SinkContext<'_>,
    ) -> Result<bool, Self::Error> {
        let kind = match context.kind() {
            SinkContextKind::Before => ResultKind::ContextBefore,
            SinkContextKind::After => ResultKind::ContextAfter,
            SinkContextKind::Other => {
                return Err(io::Error::other("unexpected passthrough context"));
            }
        };
        let byte_start = usize::try_from(context.absolute_byte_offset())
            .map_err(|_| io::Error::other("context offset overflow"))?;
        let byte_end = byte_start
            .checked_add(context.bytes().len())
            .ok_or_else(|| io::Error::other("context range overflow"))?;
        let line = usize::try_from(
            context
                .line_number()
                .ok_or_else(|| io::Error::other("missing context line number"))?,
        )
        .map_err(|_| io::Error::other("context line overflow"))?;
        let text = String::from_utf8(context.bytes().to_vec())
            .map_err(|_| io::Error::other("searcher changed UTF-8 input"))?;
        self.candidates.push(Candidate {
            document_index: self.document_index,
            record: SearchResult {
                kind,
                path: self.path.to_owned(),
                text,
                byte_start,
                byte_end,
                line_start: line,
                line_end: line,
                submatches: Vec::new(),
                submatches_truncated: false,
            },
        });
        Ok(true)
    }
}

pub(crate) fn run(input: &SearchInput) -> Result<SearchOutput, ProviderError> {
    let matcher = build_matcher(input)?;
    let mut candidates = Vec::new();
    let mut selected_seen = 0usize;
    let mut max_results_probed = false;

    for (document_index, document) in input.documents.iter().enumerate() {
        let mut searcher = build_searcher(input);
        let mut sink = CollectSink {
            matcher: &matcher,
            document_index,
            path: &document.path,
            invert: input.invert,
            max_results: input.max_results,
            selected_seen: &mut selected_seen,
            max_results_probed: &mut max_results_probed,
            candidates: &mut candidates,
        };
        searcher
            .search_slice(&matcher, document.text.as_bytes(), &mut sink)
            .map_err(|_| error::search_failed())?;
        if max_results_probed {
            break;
        }
    }

    let candidates = normalize_context(input, candidates);
    let output = apply_output_limit(candidates, max_results_probed)?;
    let wire_len = serde_json::to_vec(&ComponentResponse::Succeeded {
        output: serde_json::to_value(&output).map_err(|_| error::search_failed())?,
    })
    .map_err(|_| error::search_failed())?
    .len();
    if wire_len > MAX_SUCCESS_ENVELOPE_BYTES {
        return Err(error::search_failed());
    }
    Ok(output)
}

fn build_matcher(input: &SearchInput) -> Result<RegexMatcher, ProviderError> {
    let mut builder = RegexMatcherBuilder::new();
    builder
        .case_insensitive(input.case == CaseMode::Insensitive)
        .case_smart(input.case == CaseMode::Smart)
        // Like ripgrep, anchors are line-aware independently of whether cross-line searching is
        // enabled. Dot does not cross LF unless the pattern itself uses Rust's `s` flag.
        .multi_line(true)
        .dot_matches_new_line(false)
        .unicode(true)
        .octal(false)
        .crlf(false)
        .word(input.word)
        .fixed_strings(input.mode == SearchMode::Fixed)
        .whole_line(input.line)
        .line_terminator(if input.multiline { None } else { Some(b'\n') })
        .size_limit(REGEX_PROGRAM_BYTES)
        .dfa_size_limit(DFA_CACHE_BYTES)
        .nest_limit(REGEX_NEST_LIMIT);
    builder
        .build(&input.pattern)
        .map_err(|_| error::invalid_pattern())
}

fn build_searcher(input: &SearchInput) -> Searcher {
    SearcherBuilder::new()
        .line_terminator(LineTerminator::byte(b'\n'))
        .line_number(true)
        .multi_line(input.multiline)
        .invert_match(input.invert)
        .before_context(input.context.before)
        .after_context(input.context.after)
        .passthru(false)
        .heap_limit(Some(MAX_DOCUMENT_TEXT_BYTES))
        .memory_map(MmapChoice::never())
        .binary_detection(BinaryDetection::none())
        .encoding(None)
        .bom_sniffing(false)
        .build()
}

fn line_end(line_start: usize, bytes: &[u8]) -> io::Result<usize> {
    if bytes.is_empty() {
        return Err(io::Error::other("searcher emitted an empty record"));
    }
    let terminators = bytes.iter().filter(|&&byte| byte == b'\n').count();
    let lines = if bytes.ends_with(b"\n") {
        terminators
    } else {
        terminators + 1
    };
    line_start
        .checked_add(lines.saturating_sub(1))
        .ok_or_else(|| io::Error::other("line range overflow"))
}

fn collect_submatches(
    searcher: &Searcher,
    matcher: &RegexMatcher,
    mat: &SinkMatch<'_>,
) -> io::Result<(Vec<Submatch>, bool)> {
    let buffer = mat.buffer();
    let range = mat.bytes_range_in_buffer();
    let base = usize::try_from(mat.absolute_byte_offset())
        .map_err(|_| io::Error::other("match offset overflow"))?
        .checked_sub(range.start)
        .ok_or_else(|| io::Error::other("inconsistent match buffer"))?;

    let effective_end =
        if !searcher.multi_line_with_matcher(matcher) && buffer[range.clone()].ends_with(b"\n") {
            range.end - 1
        } else {
            buffer.len()
        };
    let searchable = &buffer[..effective_end];
    let final_unterminated_record = range.end == buffer.len() && !mat.bytes().ends_with(b"\n");
    let mut submatches = Vec::with_capacity(MAX_SUBMATCHES_PER_RESULT);
    let mut truncated = false;

    matcher
        .find_iter_at(searchable, range.start, |found| {
            if found.start() > range.end
                || (found.start() == range.end && !(found.is_empty() && final_unterminated_record))
            {
                return false;
            }
            if found.start() < range.start || found.end() > range.end {
                return true;
            }
            if submatches.len() == MAX_SUBMATCHES_PER_RESULT {
                truncated = true;
                return false;
            }
            submatches.push(Submatch {
                byte_start: base + found.start(),
                byte_end: base + found.end(),
            });
            true
        })
        .map_err(|error| io::Error::other(error.to_string()))?;

    if submatches.is_empty() {
        return Err(io::Error::other(
            "selected record has no rediscovered match",
        ));
    }
    Ok((submatches, truncated))
}

fn normalize_context(input: &SearchInput, candidates: Vec<Candidate>) -> Vec<Candidate> {
    let selected = candidates
        .iter()
        .filter(|candidate| candidate.selected())
        .map(|candidate| (candidate.document_index, candidate.record.clone()))
        .collect::<Vec<_>>();
    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(candidates.len());

    for mut candidate in candidates {
        if candidate.selected() {
            normalized.push(candidate);
            continue;
        }
        let key = (
            candidate.document_index,
            candidate.record.byte_start,
            candidate.record.byte_end,
        );
        if !seen.insert(key) {
            continue;
        }
        if selected.iter().any(|(document_index, selection)| {
            *document_index == candidate.document_index
                && ranges_overlap(selection, &candidate.record)
        }) {
            continue;
        }

        let is_before = selected.iter().any(|(document_index, selection)| {
            *document_index == candidate.document_index
                && candidate.record.line_end < selection.line_start
                && selection.line_start - candidate.record.line_end <= input.context.before
        });
        let is_after = selected.iter().any(|(document_index, selection)| {
            *document_index == candidate.document_index
                && candidate.record.line_start > selection.line_end
                && candidate.record.line_start - selection.line_end <= input.context.after
        });
        candidate.record.kind = if is_before {
            ResultKind::ContextBefore
        } else if is_after {
            ResultKind::ContextAfter
        } else {
            continue;
        };
        normalized.push(candidate);
    }

    normalized.sort_by_key(|candidate| {
        (
            candidate.document_index,
            candidate.record.byte_start,
            candidate.record.kind,
        )
    });
    normalized
}

fn ranges_overlap(left: &SearchResult, right: &SearchResult) -> bool {
    left.byte_start < right.byte_end && right.byte_start < left.byte_end
}

fn apply_output_limit(
    candidates: Vec<Candidate>,
    max_results_probed: bool,
) -> Result<SearchOutput, ProviderError> {
    let mut iterator = candidates.into_iter().peekable();
    let mut included = Vec::new();
    let mut pending_before = Vec::new();
    let mut encoded_results_bytes = 0usize;
    let mut selected_count = 0usize;
    let mut max_submatches = false;
    let mut max_output_bytes = false;

    while let Some(candidate) = iterator.next() {
        match candidate.record.kind {
            ResultKind::ContextBefore => {
                pending_before.push(candidate);
            }
            ResultKind::Match => {
                let mut chunk = std::mem::take(&mut pending_before);
                chunk.push(candidate);
                let chunk_bytes = encoded_len(&chunk)?;
                let proposed_selected = selected_count + 1;
                let proposed_submatches =
                    max_submatches || chunk.iter().any(|item| item.record.submatches_truncated);
                let reserve_output_reason = iterator.peek().is_some();
                if !fits(
                    encoded_results_bytes + chunk_bytes,
                    included.len() + chunk.len(),
                    proposed_selected,
                    max_results_probed,
                    reserve_output_reason,
                    proposed_submatches,
                ) {
                    max_output_bytes = true;
                    break;
                }
                encoded_results_bytes += chunk_bytes;
                selected_count = proposed_selected;
                max_submatches = proposed_submatches;
                included.extend(chunk.into_iter().map(|item| item.record));
            }
            ResultKind::ContextAfter => {
                let record_bytes = serde_json::to_vec(&candidate.record)
                    .map_err(|_| error::search_failed())?
                    .len();
                let reserve_output_reason = iterator.peek().is_some();
                if !fits(
                    encoded_results_bytes + record_bytes,
                    included.len() + 1,
                    selected_count,
                    max_results_probed,
                    reserve_output_reason,
                    max_submatches,
                ) {
                    max_output_bytes = true;
                    break;
                }
                encoded_results_bytes += record_bytes;
                included.push(candidate.record);
            }
        }
    }

    // A before-context record is committed atomically with the selected record that follows it.
    // Discarding a pending tail therefore cannot produce orphan context.
    let reasons = ordered_reasons(max_results_probed, max_output_bytes, max_submatches);
    let output = SearchOutput {
        results: included,
        selected_count,
        truncated: !reasons.is_empty(),
        truncation_reasons: reasons,
    };
    Ok(output)
}

fn encoded_len(candidates: &[Candidate]) -> Result<usize, ProviderError> {
    candidates.iter().try_fold(0usize, |total, candidate| {
        let bytes = serde_json::to_vec(&candidate.record)
            .map_err(|_| error::search_failed())?
            .len();
        total.checked_add(bytes).ok_or_else(error::search_failed)
    })
}

fn fits(
    encoded_results_bytes: usize,
    result_count: usize,
    selected_count: usize,
    max_results: bool,
    max_output_bytes: bool,
    max_submatches: bool,
) -> bool {
    let reasons = ordered_reasons(max_results, max_output_bytes, max_submatches);
    success_envelope_len(
        encoded_results_bytes,
        result_count,
        selected_count,
        &reasons,
    ) <= MAX_SUCCESS_ENVELOPE_BYTES
}
