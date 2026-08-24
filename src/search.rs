use std::{collections::HashSet, io};

use dekopon_provider_sdk::ProviderError;
use grep_matcher::{LineTerminator, Matcher};
use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::{
    BinaryDetection, MmapChoice, Searcher, SearcherBuilder, Sink, SinkContext, SinkContextKind,
    SinkMatch,
};

use crate::{
    error,
    input::{CaseMode, Context, MAX_DOCUMENT_TEXT_BYTES, SearchInput, SearchMode},
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
    kind: ResultKind,
    byte_start: usize,
    byte_end: usize,
    line_start: usize,
    line_end: usize,
}

impl Candidate {
    fn selected(&self) -> bool {
        self.kind == ResultKind::Match
    }

    fn into_result<'a>(
        self,
        input: &'a SearchInput,
        matcher: &RegexMatcher,
        multiline_with_matcher: bool,
    ) -> Result<SearchResult<'a>, ProviderError> {
        let document = input
            .documents
            .get(self.document_index)
            .ok_or_else(error::search_failed)?;
        let text = document
            .text
            .get(self.byte_start..self.byte_end)
            .ok_or_else(error::search_failed)?;
        let (submatches, submatches_truncated) = if self.selected() && !input.invert {
            collect_submatches(
                document.text.as_bytes(),
                self.byte_start,
                self.byte_end,
                matcher,
                multiline_with_matcher,
            )?
        } else {
            (Vec::new(), false)
        };

        Ok(SearchResult {
            kind: self.kind,
            path: &document.path,
            text,
            byte_start: self.byte_start,
            byte_end: self.byte_end,
            line_start: self.line_start,
            line_end: self.line_end,
            submatches,
            submatches_truncated,
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct Selection {
    byte_start: usize,
    byte_end: usize,
    line_start: usize,
    line_end: usize,
}

impl From<&Candidate> for Selection {
    fn from(candidate: &Candidate) -> Self {
        Self {
            byte_start: candidate.byte_start,
            byte_end: candidate.byte_end,
            line_start: candidate.line_start,
            line_end: candidate.line_end,
        }
    }
}

struct CollectSink<'a> {
    document_index: usize,
    max_results: usize,
    selected_seen: &'a mut usize,
    max_results_probed: &'a mut bool,
    candidates: &'a mut Vec<Candidate>,
}

impl Sink for CollectSink<'_> {
    type Error = io::Error;

    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch<'_>) -> Result<bool, Self::Error> {
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

        self.candidates.push(Candidate {
            document_index: self.document_index,
            kind: ResultKind::Match,
            byte_start,
            byte_end,
            line_start,
            line_end,
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
        self.candidates.push(Candidate {
            document_index: self.document_index,
            kind,
            byte_start,
            byte_end,
            line_start: line,
            line_end: line,
        });
        Ok(true)
    }
}

pub(crate) fn run<'a>(input: &'a SearchInput) -> Result<SearchOutput<'a>, ProviderError> {
    let matcher = build_matcher(input)?;
    let multiline_with_matcher = build_searcher(input).multi_line_with_matcher(&matcher);
    let mut candidates = Vec::new();
    let mut selected_seen = 0usize;
    let mut max_results_probed = false;

    for (document_index, document) in input.documents.iter().enumerate() {
        let mut searcher = build_searcher(input);
        let mut sink = CollectSink {
            document_index,
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

    apply_output_limit(
        input,
        &matcher,
        multiline_with_matcher,
        candidates,
        max_results_probed,
    )
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
    document: &[u8],
    byte_start: usize,
    byte_end: usize,
    matcher: &RegexMatcher,
    multiline_with_matcher: bool,
) -> Result<(Vec<Submatch>, bool), ProviderError> {
    let selected = document
        .get(byte_start..byte_end)
        .ok_or_else(error::search_failed)?;
    let effective_end = if !multiline_with_matcher && selected.ends_with(b"\n") {
        byte_end - 1
    } else {
        document.len()
    };
    let searchable = document
        .get(..effective_end)
        .ok_or_else(error::search_failed)?;
    let final_unterminated_record = byte_end == document.len() && !selected.ends_with(b"\n");
    let mut submatches = Vec::with_capacity(MAX_SUBMATCHES_PER_RESULT);
    let mut truncated = false;

    matcher
        .find_iter_at(searchable, byte_start, |found| {
            if found.start() > byte_end
                || (found.start() == byte_end && !(found.is_empty() && final_unterminated_record))
            {
                return false;
            }
            if found.start() < byte_start || found.end() > byte_end {
                return true;
            }
            if submatches.len() == MAX_SUBMATCHES_PER_RESULT {
                truncated = true;
                return false;
            }
            submatches.push(Submatch {
                byte_start: found.start(),
                byte_end: found.end(),
            });
            true
        })
        .map_err(|_| error::search_failed())?;

    if submatches.is_empty() {
        return Err(error::search_failed());
    }
    Ok((submatches, truncated))
}

fn normalize_context_candidate(
    context: Context,
    selections: &[Vec<Selection>],
    seen: &mut HashSet<(usize, usize, usize)>,
    mut candidate: Candidate,
) -> Option<Candidate> {
    if candidate.selected() {
        return Some(candidate);
    }

    let key = (
        candidate.document_index,
        candidate.byte_start,
        candidate.byte_end,
    );
    if !seen.insert(key) {
        return None;
    }
    let document_selections = selections.get(candidate.document_index)?;

    // Selected ranges are ordered and non-overlapping. Find the first range whose end is after
    // this context range's start; only that range can overlap it.
    let overlap_index =
        document_selections.partition_point(|selection| selection.byte_end <= candidate.byte_start);
    if document_selections
        .get(overlap_index)
        .is_some_and(|selection| selection.byte_start < candidate.byte_end)
    {
        return None;
    }

    // Context classification uses the nearest later/earlier selected range instead of rescanning
    // every selected record three times. `context_before` deliberately wins ties.
    let before_index =
        document_selections.partition_point(|selection| selection.line_start <= candidate.line_end);
    let is_before = document_selections
        .get(before_index)
        .is_some_and(|selection| selection.line_start - candidate.line_end <= context.before);

    let after_index =
        document_selections.partition_point(|selection| selection.line_end < candidate.line_start);
    let is_after = after_index != 0
        && candidate.line_start - document_selections[after_index - 1].line_end <= context.after;

    candidate.kind = if is_before {
        ResultKind::ContextBefore
    } else if is_after {
        ResultKind::ContextAfter
    } else {
        return None;
    };
    Some(candidate)
}

fn apply_output_limit<'a>(
    input: &'a SearchInput,
    matcher: &RegexMatcher,
    multiline_with_matcher: bool,
    mut candidates: Vec<Candidate>,
    max_results_probed: bool,
) -> Result<SearchOutput<'a>, ProviderError> {
    // Searcher callbacks are ordered, but sorting compact metadata makes that invariant explicit
    // before indexed normalization and deterministic output accounting.
    candidates.sort_by_key(|candidate| {
        (
            candidate.document_index,
            candidate.byte_start,
            candidate.byte_end,
            candidate.kind,
        )
    });
    let mut selections = (0..input.documents.len())
        .map(|_| Vec::new())
        .collect::<Vec<Vec<Selection>>>();
    for candidate in &candidates {
        if candidate.selected() {
            selections[candidate.document_index].push(Selection::from(candidate));
        }
    }

    let context = input.context;
    let mut raw_candidates = candidates.into_iter();
    let mut seen_context = HashSet::new();
    // Normalize lazily. Output truncation therefore materializes and re-matches at most one
    // prospective record/chunk beyond the retained prefix, not every excluded candidate.
    let normalized = std::iter::from_fn(move || {
        loop {
            let candidate = raw_candidates.next()?;
            if let Some(candidate) =
                normalize_context_candidate(context, &selections, &mut seen_context, candidate)
            {
                return Some(candidate);
            }
        }
    });
    let mut iterator = normalized.peekable();
    let mut included = Vec::new();
    let mut pending_before = Vec::new();
    let mut encoded_results_bytes = 0usize;
    let mut selected_count = 0usize;
    let mut max_submatches = false;
    let mut max_output_bytes = false;

    while let Some(candidate) = iterator.next() {
        match candidate.kind {
            ResultKind::ContextBefore => {
                pending_before.push(candidate);
            }
            ResultKind::Match => {
                let mut raw_chunk = std::mem::take(&mut pending_before);
                raw_chunk.push(candidate);
                let chunk = raw_chunk
                    .into_iter()
                    .map(|candidate| candidate.into_result(input, matcher, multiline_with_matcher))
                    .collect::<Result<Vec<_>, _>>()?;
                let chunk_bytes: usize = chunk.iter().map(SearchResult::encoded_json_len).sum();
                let proposed_selected = selected_count + 1;
                let proposed_submatches =
                    max_submatches || chunk.iter().any(|item| item.submatches_truncated);
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
                included.extend(chunk);
            }
            ResultKind::ContextAfter => {
                let record = candidate.into_result(input, matcher, multiline_with_matcher)?;
                let record_bytes = record.encoded_json_len();
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
                included.push(record);
            }
        }
    }

    // A before-context record is committed atomically with the selected record that follows it.
    // Discarding a pending tail therefore cannot produce orphan context.
    let reasons = ordered_reasons(max_results_probed, max_output_bytes, max_submatches);
    Ok(SearchOutput {
        results: included,
        selected_count,
        truncated: !reasons.is_empty(),
        truncation_reasons: reasons,
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
