//! The `rg` command word: ripgrep's command line over piped stdin only.
use crate::input::{MAX_CONTEXT_LINES, MAX_RESULTS, SearchInput};
use crate::{RipgrepProvider, Search};
use clap::Parser;
use clap::builder::RangedU64ValueParser;
use dekopon_provider_sdk::provider::{Proposal, Usage};
use serde_json::{Map, Value, json};

#[derive(Parser)]
#[command(
    name = "rg",
    version,
    about = "Search the text piped into rg with ripgrep's matchers",
    after_help = "rg searches only the text piped into it, as in `cat notes | rg -i todo`.",
    args_override_self = true
)]
pub struct Rg {
    /// A Rust regex, or a literal string with -F
    #[arg(value_name = "PATTERN")]
    pattern: String,
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
    fn input(self) -> Value {
        let mut input = Map::new();
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
        serde_json::from_value(rg.input()).expect("clap-checked input is typed");
    Ok(Proposal::to::<Search>(input))
}

#[cfg(test)]
mod tests {
    use super::RipgrepProvider;
    use dekopon_provider_sdk::{CommandRunOutcome, provider};
    use serde_json::json;
    fn command(words: &[&str], piped: bool) -> CommandRunOutcome {
        provider::command::<RipgrepProvider>(
            &words.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(),
            piped,
        )
    }
    #[test]
    fn command_proposes_only_piped_stdin_and_keeps_flags() {
        let CommandRunOutcome::Proposed {
            capability,
            input,
            secret_use,
        } = command(&["-i", "-A", "2", "needle"], true)
        else {
            panic!("proposal")
        };
        assert_eq!(capability.as_str(), "ripgrep.search");
        assert!(secret_use.is_none());
        assert_eq!(
            input,
            json!({"pattern":"needle","mode":"regex","case":"insensitive","word":false,"line":false,"multiline":false,"invert":false,"context":{"before":0,"after":2},"max_results":100})
        );
        assert!(matches!(
            command(&["needle"], false),
            CommandRunOutcome::Failed { .. }
        ));
    }
    #[test]
    fn help_version_and_bad_flags_have_guest_status() {
        for flag in ["--help", "--version"] {
            let CommandRunOutcome::Rendered {
                status,
                stdout,
                stderr,
            } = command(&[flag], false)
            else {
                panic!("render")
            };
            assert_eq!(status, 0);
            assert!(stdout.starts_with("rg ") || stdout.contains("Usage: rg"));
            assert!(stderr.is_empty());
        }
        for words in [
            &["--json", "needle"][..],
            &["needle", "path"],
            &["-m", "0", "needle"],
        ] {
            let CommandRunOutcome::Rendered {
                status,
                stdout,
                stderr,
            } = command(words, true)
            else {
                panic!("usage")
            };
            assert_eq!(status, 2);
            assert!(stdout.is_empty());
            assert!(stderr.starts_with("error: "));
        }
    }
}
