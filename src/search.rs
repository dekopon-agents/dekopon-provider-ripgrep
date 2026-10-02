use crate::{
    error::{self, SearchError},
    input::{CaseMode, SearchInput, SearchMode},
};
use grep_matcher::LineTerminator;
use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::{
    BinaryDetection, MmapChoice, Searcher, SearcherBuilder, Sink, SinkContext, SinkContextKind,
    SinkMatch,
};
use std::io::{self, Read, Write};

const REGEX_PROGRAM_BYTES: usize = 4 * 1024 * 1024;
const DFA_CACHE_BYTES: usize = 2 * 1024 * 1024;
const REGEX_NEST_LIMIT: u32 = 64;

struct LineSink<'a, W> {
    out: &'a mut W,
    selected: usize,
    max_results: usize,
}
impl<W: Write> LineSink<'_, W> {
    fn line(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.out.write_all(bytes)?;
        if !bytes.ends_with(b"\n") {
            self.out.write_all(b"\n")?;
        }
        Ok(())
    }
}
impl<W: Write> Sink for LineSink<'_, W> {
    type Error = io::Error;
    fn matched(&mut self, _: &Searcher, mat: &SinkMatch<'_>) -> io::Result<bool> {
        if self.selected == self.max_results {
            return Ok(false);
        }
        self.line(mat.bytes())?;
        self.selected += 1;
        Ok(true)
    }
    fn context(&mut self, _: &Searcher, context: &SinkContext<'_>) -> io::Result<bool> {
        if self.selected < self.max_results || *context.kind() == SinkContextKind::After {
            self.line(context.bytes())?;
        }
        Ok(true)
    }
}

pub(crate) fn run<R: Read, W: Write>(
    input: &SearchInput,
    reader: R,
    out: &mut W,
) -> Result<bool, SearchError> {
    let matcher = build_matcher(input)?;
    let mut searcher = build_searcher(input);
    let mut sink = LineSink {
        out,
        selected: 0,
        max_results: input.max_results,
    };
    searcher
        .search_reader(&matcher, reader, &mut sink)
        .map_err(|_| error::search_failed())?;
    Ok(sink.selected != 0)
}

fn build_matcher(input: &SearchInput) -> Result<RegexMatcher, SearchError> {
    let mut builder = RegexMatcherBuilder::new();
    builder
        .case_insensitive(input.case == CaseMode::Insensitive)
        .case_smart(input.case == CaseMode::Smart)
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
        .multi_line(input.multiline)
        .invert_match(input.invert)
        .before_context(input.context.before)
        .after_context(input.context.after)
        .passthru(false)
        .heap_limit(Some(1024 * 1024))
        .memory_map(MmapChoice::never())
        .binary_detection(BinaryDetection::none())
        .encoding(None)
        .bom_sniffing(false)
        .build()
}
