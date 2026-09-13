//! The `rg` command word: ripgrep's command line over the text piped into it.
//!
//! An agent's shell has no filesystem, and this component could not open one if it had, so the only
//! text `rg` can search is the value piped into the word. `PATH` is therefore never opened: it names
//! that text in the results, and defaults to `<stdin>`, the name ripgrep gives standard input.
//!
//! The flags are ripgrep's own spellings for what `ripgrep.search` accepts, and nothing more.
//! `-g/--glob`, `--max-filesize`, `-e/--regexp`, `-r/--replace`, `--json`, and every other ripgrep
//! flag are clap usage errors at status 2 naming the flag, not settings silently dropped. `--help`,
//! `--version`, and usage errors are rendered here and authorize nothing. A well-formed argv becomes
//! a `ripgrep.search` proposal, authorized exactly as a direct call is.
//!
//! The two counts are range-checked against the constants `invoke` enforces, so a model reads the
//! bound it crossed instead of a static `invalid-input`. Everything else — the pattern length, the
//! piped text's size, conflicting options — is checked once, in `invoke`.

use dekopon_provider_sdk::clap::builder::RangedU64ValueParser;
use dekopon_provider_sdk::clap::{self, CommandFactory, FromArgMatches, Parser};
use dekopon_provider_sdk::{CommandInvocation, CommandRun, ProviderError, cli};
use serde_json::{Map, Value, json};

use crate::SEARCH;
use crate::input::{MAX_CONTEXT_LINES, MAX_RESULTS};

/// The label ripgrep gives standard input, and the one unnamed piped text gets.
const STDIN_LABEL: &str = "<stdin>";

/// The path ripgrep reads as standard input.
const PIPED: &str = "-";

// The `rg` tree, declared once and rendered by clap. Plain comments, not doc comments: clap renders
// a doc comment on the struct as the `about` line above `Usage:`. `args_override_self` is ripgrep's
// last-flag-wins rule: `-m 1 -m 5` is five, not a usage error.
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
    /// The `ripgrep.search` input, carrying only the members a flag set so every unset flag keeps
    /// `invoke`'s own default.
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
        // The three case flags override one another, so at most one is still set.
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
        // ripgrep's rule: `-C` sets both sides, and `-A`/`-B` override their side in any order.
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

/// Runs one `rg` argv.
pub(crate) fn run(argv: &[String], stdin: Option<&str>) -> Result<CommandRun, ProviderError> {
    cli::run_command(Rg::command(), argv, stdin, dispatch)
}

/// Turns clap's matches into the `ripgrep.search` proposal.
///
/// Runs only after clap accepted the argv, so what is left to decide is what clap cannot know:
/// whether anything was piped.
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
    })
}

#[cfg(test)]
mod tests {
    use dekopon_provider_sdk::{CommandInvocation, CommandRun, Provider};
    use serde_json::{Value, json};

    use super::run;
    use crate::{RipgrepProvider, SEARCH};

    const TEXT: &str = "alpha\nbeta\nALPHA\n";

    const HELP: &str = "\
Search the text piped into rg with ripgrep's matchers

Usage: rg [OPTIONS] <PATTERN> [PATH]

Arguments:
  <PATTERN>  A Rust regex, or a literal string with -F
  [PATH]     The name the piped text carries in results; never opened [default: <stdin>]

Options:
  -F, --fixed-strings         Treat the pattern as a literal string instead of a regex
  -i, --ignore-case           Search case insensitively
  -S, --smart-case            Search case insensitively when the pattern is all lowercase
  -s, --case-sensitive        Search case sensitively (the default)
  -w, --word-regexp           Only match whole words
  -x, --line-regexp           Only match whole lines
  -U, --multiline             Let a match span lines
  -v, --invert-match          Select the lines that do not match
  -A, --after-context <NUM>   Show NUM lines after each match, 0-8
  -B, --before-context <NUM>  Show NUM lines before each match, 0-8
  -C, --context <NUM>         Show NUM lines before and after each match, 0-8
  -m, --max-count <NUM>       Stop after NUM matching lines, 1-1000 [default: 100]
  -h, --help                  Print help
  -V, --version               Print version

rg searches only the text piped into it, as in `cat notes | rg -i todo`. PATH names that text in the JSON results and is never opened.
";

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    fn rendered(words: &[&str], stdin: Option<&str>) -> (String, String, u8) {
        let run = run(&argv(words), stdin).expect("clap answers are rendered, not declined");
        let CommandRun::Rendered {
            stdout,
            stderr,
            status,
        } = run
        else {
            panic!("expected rendered text for {words:?}, got {run:?}");
        };
        (stdout, stderr, status)
    }

    fn proposal(words: &[&str], stdin: Option<&str>) -> CommandInvocation {
        match run(&argv(words), stdin).expect("a well-formed argv proposes") {
            CommandRun::Proposal(invocation) => invocation,
            other => panic!("expected a proposal for {words:?}, got {other:?}"),
        }
    }

    /// The input `rg alpha` proposes over [`TEXT`], with `members` merged in.
    fn search(members: Value) -> Value {
        let mut input = json!({
            "documents": [{"path": "<stdin>", "text": TEXT}],
            "pattern": "alpha"
        });
        input
            .as_object_mut()
            .expect("input object")
            .extend(members.as_object().expect("member object").clone());
        input
    }

    #[test]
    fn help_is_byte_pinned_on_stdout_at_status_zero() {
        for flag in ["--help", "-h"] {
            let (stdout, stderr, status) = rendered(&[flag], None);
            assert_eq!(stdout, HELP, "{flag}");
            assert_eq!(stderr, "", "{flag}");
            assert_eq!(status, 0, "{flag}");
        }
    }

    #[test]
    fn version_is_the_crate_version_on_stdout_at_status_zero() {
        for flag in ["--version", "-V"] {
            let (stdout, stderr, status) = rendered(&[flag], None);
            assert_eq!(stdout, concat!("rg ", env!("CARGO_PKG_VERSION"), "\n"));
            assert_eq!(stderr, "", "{flag}");
            assert_eq!(status, 0, "{flag}");
        }
    }

    #[test]
    fn a_missing_pattern_is_a_usage_error_at_status_two() {
        let (stdout, stderr, status) = rendered(&[], Some(TEXT));
        assert_eq!(status, 2);
        assert_eq!(stdout, "");
        assert_eq!(
            stderr,
            "error: the following required arguments were not provided:\n  <PATTERN>\n\n\
             Usage: rg <PATTERN> [PATH]\n\nFor more information, try '--help'.\n"
        );
    }

    /// Every ripgrep flag without a backing `ripgrep.search` member is refused by name, and so is a
    /// count outside the bound `invoke` enforces.
    #[test]
    fn unsupported_flags_and_out_of_range_counts_are_usage_errors_at_status_two() {
        for (words, named) in [
            (&["-g", "*.rs", "alpha"][..], "-g"),
            (&["--glob", "*.rs", "alpha"][..], "--glob"),
            (&["--max-filesize", "1M", "alpha"][..], "--max-filesize"),
            (&["-e", "alpha"][..], "-e"),
            (&["--json", "alpha"][..], "--json"),
            (&["-r", "omega", "alpha"][..], "-r"),
            (&["-foo"][..], "-f"),
            (&["-C", "many", "alpha"][..], "many"),
            (&["-C", "9", "alpha"][..], "9"),
            (&["-A", "-1", "alpha"][..], "-1"),
            (&["-m", "0", "alpha"][..], "0"),
            (&["-m", "1001", "alpha"][..], "1001"),
            (&["alpha", "one", "two"][..], "two"),
        ] {
            let (stdout, stderr, status) = rendered(words, Some(TEXT));
            assert_eq!(status, 2, "{words:?}");
            assert_eq!(stdout, "", "{words:?}");
            assert!(stderr.starts_with("error: "), "{words:?}: {stderr}");
            assert!(stderr.contains(named), "{words:?}: {stderr}");
            assert!(
                stderr.ends_with("\nFor more information, try '--help'.\n"),
                "{words:?}: {stderr}"
            );
            assert!(!stderr.contains('\u{1b}'), "{words:?}: {stderr:?}");
        }
    }

    #[test]
    fn each_flag_sets_exactly_its_input_member() {
        for (words, members) in [
            (&["alpha"][..], json!({})),
            (&["-F", "alpha"][..], json!({"mode": "fixed"})),
            (&["--fixed-strings", "alpha"][..], json!({"mode": "fixed"})),
            (&["-i", "alpha"][..], json!({"case": "insensitive"})),
            (
                &["--ignore-case", "alpha"][..],
                json!({"case": "insensitive"}),
            ),
            (&["-S", "alpha"][..], json!({"case": "smart"})),
            (&["--smart-case", "alpha"][..], json!({"case": "smart"})),
            (&["-s", "alpha"][..], json!({"case": "sensitive"})),
            (
                &["--case-sensitive", "alpha"][..],
                json!({"case": "sensitive"}),
            ),
            (&["-i", "-s", "alpha"][..], json!({"case": "sensitive"})),
            (&["-s", "-S", "alpha"][..], json!({"case": "smart"})),
            (&["-w", "alpha"][..], json!({"word": true})),
            (&["--word-regexp", "alpha"][..], json!({"word": true})),
            (&["-x", "alpha"][..], json!({"line": true})),
            (&["--line-regexp", "alpha"][..], json!({"line": true})),
            (&["-U", "alpha"][..], json!({"multiline": true})),
            (&["--multiline", "alpha"][..], json!({"multiline": true})),
            (&["-v", "alpha"][..], json!({"invert": true})),
            (&["--invert-match", "alpha"][..], json!({"invert": true})),
            (
                &["-A", "2", "alpha"][..],
                json!({"context": {"before": 0, "after": 2}}),
            ),
            (
                &["--after-context=2", "alpha"][..],
                json!({"context": {"before": 0, "after": 2}}),
            ),
            (
                &["-B", "3", "alpha"][..],
                json!({"context": {"before": 3, "after": 0}}),
            ),
            (
                &["--before-context", "3", "alpha"][..],
                json!({"context": {"before": 3, "after": 0}}),
            ),
            (
                &["-C", "8", "alpha"][..],
                json!({"context": {"before": 8, "after": 8}}),
            ),
            (
                &["--context", "0", "alpha"][..],
                json!({"context": {"before": 0, "after": 0}}),
            ),
            (
                &["-C", "4", "-A", "1", "alpha"][..],
                json!({"context": {"before": 4, "after": 1}}),
            ),
            (
                &["-B", "1", "-C", "4", "alpha"][..],
                json!({"context": {"before": 1, "after": 4}}),
            ),
            (&["-m", "7", "alpha"][..], json!({"max_results": 7})),
            (
                &["--max-count", "1000", "alpha"][..],
                json!({"max_results": 1000}),
            ),
            (
                &["-m", "1", "-m", "5", "alpha"][..],
                json!({"max_results": 5}),
            ),
            (
                &["alpha", "-iw"][..],
                json!({"case": "insensitive", "word": true}),
            ),
        ] {
            let invocation = proposal(words, Some(TEXT));
            assert_eq!(invocation.capability.as_str(), SEARCH, "{words:?}");
            assert_eq!(invocation.input, search(members), "{words:?}");
        }
    }

    #[test]
    fn path_names_the_piped_text_and_dash_is_stdin() {
        let invocation = proposal(&["alpha", "notes/todo.md"], Some(TEXT));
        assert_eq!(
            invocation.input["documents"],
            json!([{"path": "notes/todo.md", "text": TEXT}])
        );
        let invocation = proposal(&["alpha", "-"], Some(TEXT));
        assert_eq!(invocation.input, search(json!({})));
    }

    #[test]
    fn double_dash_ends_the_options() {
        let invocation = proposal(&["--", "-foo"], Some(TEXT));
        assert_eq!(invocation.input["pattern"], "-foo");
        assert_eq!(invocation.input["documents"][0]["path"], "<stdin>");

        let invocation = proposal(&["-i", "--", "-v", "--label"], Some(TEXT));
        assert_eq!(
            invocation.input,
            json!({
                "documents": [{"path": "--label", "text": TEXT}],
                "pattern": "-v",
                "case": "insensitive"
            })
        );
    }

    #[test]
    fn nothing_piped_is_a_decline() {
        let error = run(&argv(&["alpha"]), None).expect_err("a decline, reported as a usage error");
        assert_eq!(error.code(), "usage");
        assert_eq!(
            error.message(),
            "rg: nothing was piped in; rg searches only the text piped into it"
        );

        let invocation = proposal(&["alpha"], Some(""));
        assert_eq!(invocation.input["documents"][0]["text"], "");
    }

    /// Every capability the word can propose is one the manifest declares. Without this, a renamed
    /// capability would be discovered by a model at runtime as an authorization denial.
    #[test]
    fn every_dispatch_target_is_declared_in_the_manifest() {
        let declared: Vec<String> = RipgrepProvider::manifest()
            .capabilities
            .iter()
            .map(|capability| capability.id.to_string())
            .collect();
        let invocation = proposal(&["alpha"], Some(TEXT));
        assert!(
            declared.contains(&invocation.capability.to_string()),
            "rg proposes {} which the manifest does not declare",
            invocation.capability
        );
    }

    /// A proposal is only useful if the closed schema accepts it: every flag's member, at its
    /// boundary, is a search `invoke` runs.
    #[test]
    fn every_proposal_is_a_search_invoke_accepts() {
        for (words, selected) in [
            (&["alpha"][..], 1),
            (&["-F", "-i", "-w", "-C", "8", "-m", "1000", "ALPHA"][..], 2),
            // A lowercase pattern under smart case matches `ALPHA` too.
            (&["-S", "-x", "-A", "0", "-B", "8", "alpha"][..], 2),
            (&["-s", "-U", "-m", "1", "alpha\nbeta"][..], 1),
            (&["-v", "alpha", "notes/todo.md"][..], 2),
        ] {
            let invocation = proposal(words, Some(TEXT));
            let output = RipgrepProvider::invoke(&invocation.capability, invocation.input)
                .unwrap_or_else(|error| panic!("{words:?}: {}", error.message()));
            assert_eq!(output["selected_count"], selected, "{words:?}: {output}");
        }
    }
}
