//! The `rg` command word: ripgrep's command line over the text piped into it.
//! Paths only label caller text; no filesystem, network, subprocess or storage access is granted.
use clap::Parser;
use clap::builder::RangedU64ValueParser;
use dekopon_provider_sdk::provider::{Proposal, Usage};
use serde_json::{Map, Value, json};

use crate::input::{MAX_CONTEXT_LINES, MAX_RESULTS, SearchInput};
use crate::{RipgrepProvider, Search};

#[derive(Parser)]
#[command(
    name = "rg",
    version,
    about = "Search the text piped into rg with ripgrep's matchers",
    after_help = "rg searches only the text piped into it, as in `cat notes | rg -i todo`. PATH names \
                  that text in the JSON results and is never opened.",
    args_override_self = true
)]
pub struct Rg {
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
            None | Some("-") => "<stdin>",
            Some(path) => path,
        };
        let mut input = Map::new();
        // RG-a bridge: the text is supplied only when the capability runs, never in a proposal.
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

pub(crate) fn propose(rg: Rg, stdin_piped: bool) -> Result<Proposal<RipgrepProvider>, Usage> {
    if !stdin_piped {
        return Err(Usage::new(
            "rg: nothing was piped in; rg searches only the text piped into it",
        ));
    }
    let input: SearchInput =
        serde_json::from_value(rg.input("")).expect("clap-checked input is typed");
    Ok(Proposal::to::<Search>(input))
}

#[cfg(test)]
mod tests {
    use super::RipgrepProvider;
    use dekopon_provider_sdk::CommandRunOutcome;
    use dekopon_provider_sdk::provider::{self, Provider};
    use dekopon_provider_sdk_testkit::Native;
    use serde_json::{Value, json};

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

    fn command(words: &[&str], piped: bool) -> CommandRunOutcome {
        provider::command::<RipgrepProvider>(
            &words
                .iter()
                .map(|word| (*word).to_owned())
                .collect::<Vec<_>>(),
            piped,
        )
    }
    fn proposal(words: &[&str]) -> Value {
        match command(words, true) {
            CommandRunOutcome::Proposed {
                capability,
                input,
                secret_use,
            } => {
                assert_eq!(capability.as_str(), "ripgrep.search");
                assert!(secret_use.is_none());
                input
            }
            other => panic!("{words:?}: expected proposal, got {other:?}"),
        }
    }

    fn search(members: Value) -> Value {
        let mut expected = json!({
            "documents": [{"path": "<stdin>", "text": ""}],
            "pattern": "alpha",
            "mode": "regex",
            "case": "sensitive",
            "word": false,
            "line": false,
            "multiline": false,
            "invert": false,
            "context": {"before": 0, "after": 0},
            "max_results": 100,
        });
        expected.as_object_mut().unwrap().extend(
            members
                .as_object()
                .expect("members fixture is an object")
                .clone(),
        );
        expected
    }
    #[test]
    fn help_is_byte_pinned_on_stdout_at_status_zero() {
        for flag in ["--help", "-h"] {
            let CommandRunOutcome::Rendered {
                stdout,
                stderr,
                status,
            } = command(&[flag], false)
            else {
                panic!("help");
            };
            assert_eq!(status, 0);
            assert_eq!(stderr, "");
            assert_eq!(stdout, HELP, "{flag}");
        }
    }
    #[test]
    fn version_is_the_crate_version_on_stdout_at_status_zero() {
        for flag in ["--version", "-V"] {
            let CommandRunOutcome::Rendered {
                stdout,
                stderr,
                status,
            } = command(&[flag], false)
            else {
                panic!("version");
            };
            assert_eq!(status, 0);
            assert_eq!(stderr, "");
            assert_eq!(stdout, concat!("rg ", env!("CARGO_PKG_VERSION"), "\n"));
        }
    }
    #[test]
    fn a_missing_pattern_is_a_usage_error_at_status_two() {
        let CommandRunOutcome::Rendered {
            stdout,
            stderr,
            status,
        } = command(&[], true)
        else {
            panic!("usage");
        };
        assert_eq!(status, 2);
        assert_eq!(stdout, "");
        assert_eq!(
            stderr,
            "error: the following required arguments were not provided:\n  <PATTERN>\n\n\
             Usage: rg <PATTERN> [PATH]\n\nFor more information, try '--help'.\n"
        );
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
            (&["-foo"], "-f"),
            (&["-C", "many", "alpha"], "many"),
            (&["-C", "9", "alpha"], "9"),
            (&["-A", "-1", "alpha"], "-1"),
            (&["-m", "0", "alpha"], "0"),
            (&["-m", "1001", "alpha"], "1001"),
            (&["alpha", "one", "two"], "two"),
        ] {
            let CommandRunOutcome::Rendered {
                stdout,
                stderr,
                status,
            } = command(words, true)
            else {
                panic!("{words:?}");
            };
            assert_eq!(status, 2);
            assert_eq!(stdout, "");
            assert!(stderr.starts_with("error: "), "{words:?}: {stderr}");
            assert!(stderr.contains(named), "{words:?}: {stderr}");
            assert!(
                stderr.ends_with("\nFor more information, try '--help'.\n"),
                "{words:?}: {stderr}"
            );
            assert!(!stderr.contains('\u{1b}'), "{words:?}: {stderr}");
        }
    }
    #[test]
    fn each_flag_sets_exactly_its_input_member() {
        for (words, members) in [
            (&["alpha"][..], json!({})),
            (&["-F", "alpha"], json!({"mode": "fixed"})),
            (&["--fixed-strings", "alpha"], json!({"mode": "fixed"})),
            (&["-i", "alpha"], json!({"case": "insensitive"})),
            (&["--ignore-case", "alpha"], json!({"case": "insensitive"})),
            (&["-S", "alpha"], json!({"case": "smart"})),
            (&["--smart-case", "alpha"], json!({"case": "smart"})),
            (&["-s", "alpha"], json!({"case": "sensitive"})),
            (&["--case-sensitive", "alpha"], json!({"case": "sensitive"})),
            (&["-i", "-s", "alpha"], json!({"case": "sensitive"})),
            (&["-s", "-S", "alpha"], json!({"case": "smart"})),
            (&["-w", "alpha"], json!({"word": true})),
            (&["--word-regexp", "alpha"], json!({"word": true})),
            (&["-x", "alpha"], json!({"line": true})),
            (&["--line-regexp", "alpha"], json!({"line": true})),
            (&["-U", "alpha"], json!({"multiline": true})),
            (&["--multiline", "alpha"], json!({"multiline": true})),
            (&["-v", "alpha"], json!({"invert": true})),
            (&["--invert-match", "alpha"], json!({"invert": true})),
            (
                &["-A", "2", "alpha"],
                json!({"context": {"before": 0, "after": 2}}),
            ),
            (
                &["--after-context=2", "alpha"],
                json!({"context": {"before": 0, "after": 2}}),
            ),
            (
                &["-B", "3", "alpha"],
                json!({"context": {"before": 3, "after": 0}}),
            ),
            (
                &["--before-context", "3", "alpha"],
                json!({"context": {"before": 3, "after": 0}}),
            ),
            (
                &["-C", "8", "alpha"],
                json!({"context": {"before": 8, "after": 8}}),
            ),
            (
                &["--context", "0", "alpha"],
                json!({"context": {"before": 0, "after": 0}}),
            ),
            (
                &["-C", "4", "-A", "1", "alpha"],
                json!({"context": {"before": 4, "after": 1}}),
            ),
            (
                &["-B", "1", "-C", "4", "alpha"],
                json!({"context": {"before": 1, "after": 4}}),
            ),
            (&["-m", "7", "alpha"], json!({"max_results": 7})),
            (
                &["--max-count", "1000", "alpha"],
                json!({"max_results": 1000}),
            ),
            (&["-m", "1", "-m", "5", "alpha"], json!({"max_results": 5})),
            (
                &["alpha", "-iw"],
                json!({"case": "insensitive", "word": true}),
            ),
        ] {
            assert_eq!(proposal(words), search(members), "{words:?}");
        }
    }
    #[test]
    fn path_names_the_piped_text_and_dash_is_stdin() {
        assert_eq!(
            proposal(&["alpha", "notes/todo.md"]),
            search(json!({"documents": [{"path": "notes/todo.md", "text": ""}]}))
        );
        assert_eq!(proposal(&["alpha", "-"]), search(json!({})));
    }
    #[test]
    fn double_dash_ends_the_options() {
        assert_eq!(
            proposal(&["--", "-foo"]),
            search(json!({"pattern": "-foo"}))
        );
        assert_eq!(
            proposal(&["-i", "--", "-v", "--label"]),
            search(json!({
                "documents": [{"path": "--label", "text": ""}],
                "pattern": "-v",
                "case": "insensitive",
            }))
        );
    }
    #[test]
    fn nothing_piped_is_a_decline() {
        let CommandRunOutcome::Failed { error } = command(&["alpha"], false) else {
            panic!("missing pipe");
        };
        assert_eq!(error.code, "usage");
        assert!(error.message.contains("nothing was piped in"));
        assert_eq!(proposal(&["alpha"])["documents"][0]["text"], "");
    }
    #[test]
    fn every_dispatch_target_is_declared_in_the_manifest() {
        let manifest = provider::manifest::<RipgrepProvider>().unwrap();
        let declared: Vec<String> = manifest
            .capabilities
            .iter()
            .map(|cap| cap.id.to_string())
            .collect();
        assert!(declared.contains(&"ripgrep.search".to_owned()));
        assert_eq!(RipgrepProvider::COMMAND_WORDS, &["rg"]);
    }
    #[test]
    fn every_proposal_is_a_search_invoke_accepts() {
        const TEXT: &str = "alpha\nbeta\nALPHA\n";
        for (words, selected) in [
            (&["alpha"][..], 1),
            (&["-F", "-i", "-w", "-C", "8", "-m", "1000", "ALPHA"], 2),
            (&["-S", "-x", "-A", "0", "-B", "8", "alpha"], 2),
            (&["-s", "-U", "-m", "1", "alpha\nbeta"], 1),
            (&["-v", "alpha", "notes/todo.md"], 2),
        ] {
            let mut input = proposal(words);
            input["documents"][0]["text"] = json!(TEXT);
            let output =
                Native::<RipgrepProvider>::new().call("ripgrep.search", &input.to_string());
            assert_eq!(output.status, 0, "{words:?}: {}", output.stderr);
            let value: Value =
                serde_json::from_slice(&output.stdout).expect("RG-a JSON stdout bridge");
            assert_eq!(value["selected_count"], selected, "{words:?}: {value}");
        }
    }
}
