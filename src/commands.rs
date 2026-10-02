//! The `rg` command word: ripgrep's command line over the text piped into it.
//! Paths only label caller text; no filesystem, network, subprocess or storage access is granted.
use crate::SEARCH;
use crate::input::{MAX_CONTEXT_LINES, MAX_RESULTS};
use dekopon_provider_sdk::clap::builder::RangedU64ValueParser;
use dekopon_provider_sdk::clap::{self, CommandFactory, FromArgMatches, Parser};
use dekopon_provider_sdk::{CommandInvocation, CommandRun, ProviderError, cli};
use serde_json::{Map, Value, json};
const STDIN_LABEL: &str = "<stdin>";
const PIPED: &str = "-";

#[derive(Parser)]
#[command(
    name = "rg",
    version,
    about = "Search the text piped into rg with ripgrep's matchers",
    after_help = "rg searches only the text piped into it, as in `cat notes | rg -i todo`. PATH names \
                  that text in the JSON results and is never opened.",
    args_override_self = true
)]
struct Rg {
    /// A Rust regex, or a literal string with -F
    #[arg(value_name = "PATTERN")]
    pattern: String,
    /// The name the piped text carries in results; never opened [default: <stdin>]
    #[arg(value_name = "PATH")]
    path: Option<String>,
    /// Treat the pattern as a literal string instead of a regex
    #[arg(short = 'F', long)]
    fixed_strings: bool,
    /// Search case insensitively
    #[arg(short = 'i', long, overrides_with_all = ["smart_case", "case_sensitive"])]
    ignore_case: bool,
    /// Search case insensitively when the pattern is all lowercase
    #[arg(short = 'S', long, overrides_with_all = ["ignore_case", "case_sensitive"])]
    smart_case: bool,
    /// Search case sensitively (the default)
    #[arg(short = 's', long, overrides_with_all = ["ignore_case", "smart_case"])]
    case_sensitive: bool,
    /// Only match whole words
    #[arg(short = 'w', long)]
    word_regexp: bool,
    /// Only match whole lines
    #[arg(short = 'x', long)]
    line_regexp: bool,
    /// Let a match span lines
    #[arg(short = 'U', long)]
    multiline: bool,
    /// Select the lines that do not match
    #[arg(short = 'v', long)]
    invert_match: bool,
    /// Show NUM lines after each match, 0-8
    #[arg(short = 'A', long, value_name = "NUM", value_parser = context_lines())]
    after_context: Option<usize>,
    /// Show NUM lines before each match, 0-8
    #[arg(short = 'B', long, value_name = "NUM", value_parser = context_lines())]
    before_context: Option<usize>,
    /// Show NUM lines before and after each match, 0-8
    #[arg(short = 'C', long, value_name = "NUM", value_parser = context_lines())]
    context: Option<usize>,
    /// Stop after NUM matching lines, 1-1000 [default: 100]
    #[arg(short = 'm', long, value_name = "NUM", value_parser = max_count())]
    max_count: Option<usize>,
}
fn context_lines() -> RangedU64ValueParser<usize> {
    RangedU64ValueParser::new().range(0..=MAX_CONTEXT_LINES as u64)
}
fn max_count() -> RangedU64ValueParser<usize> {
    RangedU64ValueParser::new().range(1..=MAX_RESULTS as u64)
}
impl Rg {
    fn input(self, text: &str) -> Value {
        let path = match self.path.as_deref() {
            None | Some(PIPED) => STDIN_LABEL,
            Some(path) => path,
        };
        let mut input = Map::new();
        input.insert(
            "documents".to_owned(),
            json!([{"path": path, "text": text}]),
        );
        input.insert("pattern".to_owned(), Value::String(self.pattern));
        if self.fixed_strings {
            input.insert("mode".to_owned(), json!("fixed"));
        }
        let case = if self.ignore_case {
            Some("insensitive")
        } else if self.smart_case {
            Some("smart")
        } else if self.case_sensitive {
            Some("sensitive")
        } else {
            None
        };
        if let Some(case) = case {
            input.insert("case".to_owned(), json!(case));
        }
        for (member, set) in [
            ("word", self.word_regexp),
            ("line", self.line_regexp),
            ("multiline", self.multiline),
            ("invert", self.invert_match),
        ] {
            if set {
                input.insert(member.to_owned(), json!(true));
            }
        }
        if self.before_context.is_some() || self.after_context.is_some() || self.context.is_some() {
            input.insert(
                "context".to_owned(),
                json!({
                    "before": self.before_context.or(self.context).unwrap_or(0),
                    "after": self.after_context.or(self.context).unwrap_or(0),
                }),
            );
        }
        if let Some(count) = self.max_count {
            input.insert("max_results".to_owned(), json!(count));
        }
        Value::Object(input)
    }
}
pub(crate) fn run(argv: &[String], stdin: Option<&str>) -> Result<CommandRun, ProviderError> {
    cli::run_command(Rg::command(), argv, stdin, dispatch)
}
fn dispatch(
    matches: clap::ArgMatches,
    stdin: Option<&str>,
) -> Result<CommandInvocation, ProviderError> {
    let rg = Rg::from_arg_matches(&matches)
        .map_err(|error| ProviderError::new("usage", error.to_string()))?;
    let Some(text) = stdin else {
        return Err(ProviderError::new(
            "usage",
            "rg: nothing was piped in; rg searches only the text piped into it",
        ));
    };
    Ok(CommandInvocation {
        capability: SEARCH.parse().expect("static capability ID"),
        input: rg.input(text),
        secret_use: None,
    })
}

#[cfg(test)]
mod tests {
    use super::run;
    use crate::{RipgrepProvider, SEARCH};
    use dekopon_provider_sdk::{CommandInvocation, CommandRun, Provider};
    use serde_json::{Value, json};
    const TEXT: &str = "alpha\nbeta\nALPHA\n";
    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }
    fn rendered(words: &[&str], stdin: Option<&str>) -> (String, String, u8) {
        let CommandRun::Rendered {
            stdout,
            stderr,
            status,
        } = run(&argv(words), stdin).expect("rendered")
        else {
            panic!("expected rendered text for {words:?}");
        };
        (stdout, stderr, status)
    }
    fn proposal(words: &[&str], stdin: Option<&str>) -> CommandInvocation {
        let CommandRun::Proposal(proposal) = run(&argv(words), stdin).expect("proposal") else {
            panic!("expected proposal for {words:?}");
        };
        assert_eq!(proposal.capability.as_str(), SEARCH);
        proposal
    }
    #[test]
    fn help_is_byte_pinned_on_stdout_at_status_zero() {
        for flag in ["--help", "-h"] {
            let (stdout, stderr, status) = rendered(&[flag], None);
            assert_eq!(status, 0);
            assert_eq!(stderr, "");
            assert!(stdout.contains("Usage: rg [OPTIONS] <PATTERN> [PATH]"));
            assert!(
                stdout.contains("PATH names that text in the JSON results and is never opened.")
            );
        }
    }
    #[test]
    fn version_is_the_crate_version_on_stdout_at_status_zero() {
        for flag in ["--version", "-V"] {
            let (stdout, stderr, status) = rendered(&[flag], None);
            assert_eq!(status, 0);
            assert_eq!(stderr, "");
            assert_eq!(stdout, concat!("rg ", env!("CARGO_PKG_VERSION"), "\n"));
        }
    }
    #[test]
    fn a_missing_pattern_is_a_usage_error_at_status_two() {
        let (stdout, stderr, status) = rendered(&[], Some(TEXT));
        assert_eq!(status, 2);
        assert_eq!(stdout, "");
        assert!(stderr.contains("<PATTERN>"));
    }
    #[test]
    fn unsupported_flags_and_out_of_range_counts_are_usage_errors_at_status_two() {
        for (words, named) in [
            (&["-g", "*.rs", "alpha"][..], "-g"),
            (&["--glob", "*.rs", "alpha"], "--glob"),
            (&["--max-filesize", "1M", "alpha"], "--max-filesize"),
            (&["-e", "alpha"], "-e"),
            (&["--json", "alpha"], "--json"),
            (&["-r", "omega", "alpha"], "-r"),
            (&["-C", "9", "alpha"], "9"),
            (&["-m", "0", "alpha"], "0"),
            (&["-m", "1001", "alpha"], "1001"),
            (&["alpha", "one", "two"], "two"),
        ] {
            let (stdout, stderr, status) = rendered(words, Some(TEXT));
            assert_eq!(status, 2);
            assert_eq!(stdout, "");
            assert!(stderr.contains(named), "{stderr}");
            assert!(!stderr.contains('\u{1b}'));
        }
    }
    #[test]
    fn each_flag_sets_exactly_its_input_member() {
        for (words, member, expected) in [
            (&["-F", "alpha"][..], "mode", json!("fixed")),
            (&["-i", "alpha"], "case", json!("insensitive")),
            (&["-S", "alpha"], "case", json!("smart")),
            (&["-s", "alpha"], "case", json!("sensitive")),
            (&["-w", "alpha"], "word", json!(true)),
            (&["-x", "alpha"], "line", json!(true)),
            (&["-U", "alpha"], "multiline", json!(true)),
            (&["-v", "alpha"], "invert", json!(true)),
            (&["-m", "7", "alpha"], "max_results", json!(7)),
        ] {
            assert_eq!(
                proposal(words, Some(TEXT)).input[member],
                expected,
                "{words:?}"
            );
        }
        assert_eq!(
            proposal(&["-C", "4", "-A", "1", "alpha"], Some(TEXT)).input["context"],
            json!({"before":4,"after":1})
        );
        assert_eq!(
            proposal(&["-m", "1", "-m", "5", "alpha"], Some(TEXT)).input["max_results"],
            5
        );
    }
    #[test]
    fn path_names_the_piped_text_and_dash_is_stdin() {
        assert_eq!(
            proposal(&["alpha", "notes/todo.md"], Some(TEXT)).input["documents"],
            json!([{"path":"notes/todo.md","text":TEXT}])
        );
        assert_eq!(
            proposal(&["alpha", "-"], Some(TEXT)).input["documents"],
            json!([{"path":"<stdin>","text":TEXT}])
        );
    }
    #[test]
    fn double_dash_ends_the_options() {
        assert_eq!(
            proposal(&["--", "-foo"], Some(TEXT)).input["pattern"],
            "-foo"
        );
        let input = proposal(&["-i", "--", "-v", "--label"], Some(TEXT)).input;
        assert_eq!(input["pattern"], "-v");
        assert_eq!(input["documents"][0]["path"], "--label");
    }
    #[test]
    fn nothing_piped_is_a_decline() {
        let error = run(&argv(&["alpha"]), None).expect_err("no pipe");
        assert_eq!(error.code(), "usage");
        assert!(error.message().contains("nothing was piped in"));
        assert_eq!(
            proposal(&["alpha"], Some("")).input["documents"][0]["text"],
            ""
        );
    }
    #[test]
    fn every_dispatch_target_is_declared_in_the_manifest() {
        let declared: Vec<_> = RipgrepProvider::manifest()
            .capabilities
            .iter()
            .map(|cap| cap.id.to_string())
            .collect();
        assert!(declared.contains(&proposal(&["alpha"], Some(TEXT)).capability.to_string()));
    }
    #[test]
    fn every_proposal_is_a_search_invoke_accepts() {
        for (words, selected) in [
            (&["alpha"][..], 1),
            (&["-F", "-i", "-w", "-C", "8", "-m", "1000", "ALPHA"], 2),
            (&["-S", "-x", "-A", "0", "-B", "8", "alpha"], 2),
            (&["-s", "-U", "-m", "1", "alpha\nbeta"], 1),
            (&["-v", "alpha", "notes/todo.md"], 2),
        ] {
            let invocation = proposal(words, Some(TEXT));
            let output: Value = RipgrepProvider::invoke(&invocation.capability, invocation.input)
                .expect("search succeeds");
            assert_eq!(output["selected_count"], selected, "{words:?}: {output}");
        }
    }
}
