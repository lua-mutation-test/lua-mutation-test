//! Command-line interface definitions.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Mutation testing tool for Lua.
#[derive(Parser, Debug)]
#[command(
    name = "lua-mutation-test",
    bin_name = "lua-mutation-test",
    version = env!("LMT_GIT_VERSION"),
    about = "A mutation testing tool for Lua",
    long_about = None
)]
#[command(
    after_help = "EXAMPLES:\n  lmut run\n  lmut run src\n  lmut run file.lua --test-command 'busted'\n  lmut run --changed-since origin/main\n  lmut list-operators\n  lmut init"
)]
pub struct Cli {
    /// Path to a configuration file.
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    /// Enable verbose output.
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Suppress non-essential output.
    #[arg(short, long, global = true)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run mutation testing against the given path.
    Run(RunArgs),

    /// Watch source files and re-run mutation tests incrementally.
    Watch(WatchArgs),

    /// List generated mutants for a source file.
    ListMutants(ListMutantsArgs),

    /// List available mutation operators.
    ListOperators,

    /// Create a sample configuration file.
    Init,
}

/// Arguments for the `list-mutants` subcommand.
#[derive(Parser, Debug, Clone)]
pub struct ListMutantsArgs {
    /// Path to a Lua source file.
    pub path: PathBuf,
}

/// Arguments for the `run` subcommand.
#[derive(Parser, Debug, Clone)]
pub struct RunArgs {
    /// Path to a Lua file or directory to mutate.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Custom shell command used to run tests.
    #[arg(long)]
    pub test_command: Option<String>,

    /// Timeout in seconds for each mutant test run.
    #[arg(long)]
    pub timeout: Option<u64>,

    /// Report format: summary, per-mutant, json, ctrf, html, stryker.
    #[arg(long)]
    pub report_format: Option<String>,

    /// Output path for the generated report. Use `-` to write to stdout.
    ///
    /// When `-` is used together with `--report-format`, the report goes to
    /// stdout and the `Mutation score:` summary moves to stderr so stdout
    /// stays pipe-parseable (e.g. `... --report-output - | jq`).
    #[arg(long)]
    pub report_output: Option<PathBuf>,

    /// Number of parallel workers for mutant execution.
    #[arg(long)]
    pub workers: Option<usize>,

    /// Only mutate files changed since the given git ref (e.g. --changed-since origin/main).
    #[arg(long, value_name = "git-ref")]
    pub changed_since: Option<String>,

    /// Shard selection for CI matrix fan-out, as `<k>/<n>` (e.g. `2/5`).
    ///
    /// Partitions the generated mutant inventory deterministically by mutant
    /// count (stable sort by file + location + operator, then stride) so
    /// shards stay balanced regardless of file layout.
    #[arg(long, value_name = "K/N")]
    pub shard: Option<String>,
}

/// Arguments for the `watch` subcommand.
#[derive(Parser, Debug, Clone)]
pub struct WatchArgs {
    /// Path to a Lua file or directory to mutate.
    pub path: PathBuf,

    /// Custom shell command used to run tests.
    #[arg(long)]
    pub test_command: Option<String>,

    /// Timeout in seconds for each mutant test run.
    #[arg(long)]
    pub timeout: Option<u64>,

    /// Debounce duration in milliseconds before re-running after a file change.
    #[arg(long, default_value_t = 500)]
    pub debounce: u64,

    /// Number of parallel workers for mutant execution.
    #[arg(long)]
    pub workers: Option<usize>,

    /// Shard selection for CI matrix fan-out, as `<k>/<n>` (e.g. `2/5`).
    #[arg(long, value_name = "K/N")]
    pub shard: Option<String>,
}

/// Exit codes used by the binary.
///
/// These codes are part of the stable CLI contract documented in
/// `docs/cli-reference.md`:
/// - `0` (`SUCCESS`): all mutants killed.
/// - `1` (`TEST_FAILURES`): mutation testing completed, one or more mutants survived.
/// - `2` (`CLI_ERROR`): startup or configuration error (no test command,
///   no test files, bad config, parse errors, ...).
/// - `3` (`BASELINE_FAILED`): the unmodified test suite is red, so no
///   mutant result would be meaningful.
pub mod exit {
    pub const SUCCESS: i32 = 0;
    pub const TEST_FAILURES: i32 = 1;
    pub const CLI_ERROR: i32 = 2;
    pub const BASELINE_FAILED: i32 = 3;
}

/// Error from the `run`/`watch` pipeline carrying the process exit code.
///
/// Callers use [`ExitError::cli`] for startup/config failures (exit 2) and
/// [`ExitError::baseline`] for a red baseline (exit 3). `main` maps the
/// stored code directly to the process exit status so wrappers (e.g. a
/// GitHub Action) can distinguish the three failure modes without grepping
/// log text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitError {
    message: String,
    code: i32,
}

impl ExitError {
    /// Configuration or CLI usage error (exit [`exit::CLI_ERROR`]).
    pub fn cli(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: exit::CLI_ERROR,
        }
    }

    /// Red baseline: the unmodified suite failed (exit [`exit::BASELINE_FAILED`]).
    pub fn baseline(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: exit::BASELINE_FAILED,
        }
    }

    /// Process exit code for this error.
    pub fn code(&self) -> i32 {
        self.code
    }
}

impl std::fmt::Display for ExitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ExitError {}

impl From<String> for ExitError {
    fn from(message: String) -> Self {
        Self::cli(message)
    }
}

impl From<&str> for ExitError {
    fn from(message: &str) -> Self {
        Self::cli(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_run_subcommand_with_flags() {
        let cli = Cli::parse_from([
            "lua-mutation-test",
            "run",
            "src",
            "--test-command",
            "busted",
            "--timeout",
            "30",
            "--report-format",
            "json",
            "--report-output",
            "report.json",
        ]);
        match cli.command {
            Command::Run(args) => {
                assert_eq!(args.path, PathBuf::from("src"));
                assert_eq!(args.test_command, Some("busted".to_string()));
                assert_eq!(args.timeout, Some(30));
                assert_eq!(args.report_format, Some("json".to_string()));
                assert_eq!(args.report_output, Some(PathBuf::from("report.json")));
            }
            _ => panic!("expected run subcommand"),
        }
    }

    #[test]
    fn parses_report_output_dash_as_stdout() {
        let cli = Cli::parse_from([
            "lua-mutation-test",
            "run",
            "src",
            "--report-format",
            "json",
            "--report-output",
            "-",
        ]);
        match cli.command {
            Command::Run(args) => {
                assert_eq!(args.report_format, Some("json".to_string()));
                assert_eq!(args.report_output, Some(PathBuf::from("-")));
            }
            _ => panic!("expected run subcommand"),
        }
    }

    #[test]
    fn run_subcommand_defaults_path_to_current_directory() {
        let cli = Cli::parse_from([
            "lua-mutation-test",
            "run",
            "--test-command",
            "busted",
            "--timeout",
            "30",
        ]);
        match cli.command {
            Command::Run(args) => {
                assert_eq!(args.path, PathBuf::from("."));
                assert_eq!(args.test_command, Some("busted".to_string()));
                assert_eq!(args.timeout, Some(30));
            }
            _ => panic!("expected run subcommand"),
        }
    }

    #[test]
    fn parses_changed_since_flag() {
        let cli = Cli::parse_from(["lua-mutation-test", "run", "--changed-since", "origin/main"]);
        match cli.command {
            Command::Run(args) => {
                assert_eq!(args.changed_since, Some("origin/main".to_string()));
            }
            _ => panic!("expected run subcommand"),
        }
    }

    #[test]
    fn changed_since_defaults_to_none() {
        let cli = Cli::parse_from(["lua-mutation-test", "run", "src"]);
        match cli.command {
            Command::Run(args) => {
                assert_eq!(args.changed_since, None);
            }
            _ => panic!("expected run subcommand"),
        }
    }

    #[test]
    fn run_help_documents_changed_since_with_pr_example() {
        let err = Cli::try_parse_from(["lua-mutation-test", "run", "--help"])
            .expect_err("expected --help to exit with help text");
        let help = err.to_string();
        assert!(
            help.contains("--changed-since"),
            "run --help should document --changed-since:\n{help}"
        );
        assert!(
            help.contains("origin/main"),
            "run --help should show a PR example:\n{help}"
        );
    }

    #[test]
    fn parses_list_mutants_subcommand() {
        let cli = Cli::parse_from(["lua-mutation-test", "list-mutants", "src/foo.lua"]);
        match cli.command {
            Command::ListMutants(args) => {
                assert_eq!(args.path, PathBuf::from("src/foo.lua"));
            }
            _ => panic!("expected list-mutants subcommand"),
        }
    }

    #[test]
    fn parses_list_operators_subcommand() {
        let cli = Cli::parse_from(["lua-mutation-test", "list-operators"]);
        matches!(cli.command, Command::ListOperators);
    }

    #[test]
    fn parses_init_subcommand() {
        let cli = Cli::parse_from(["lua-mutation-test", "init"]);
        matches!(cli.command, Command::Init);
    }

    #[test]
    fn parses_global_options() {
        let cli = Cli::parse_from([
            "lua-mutation-test",
            "--config",
            "config.toml",
            "--verbose",
            "run",
            "src",
        ]);
        assert_eq!(cli.config, Some(PathBuf::from("config.toml")));
        assert!(cli.verbose);
        assert!(!cli.quiet);
    }

    #[test]
    fn parses_watch_subcommand() {
        let cli = Cli::parse_from(["lua-mutation-test", "watch", "src", "--debounce", "250"]);
        match cli.command {
            Command::Watch(args) => {
                assert_eq!(args.path, PathBuf::from("src"));
                assert_eq!(args.debounce, 250);
            }
            _ => panic!("expected watch subcommand"),
        }
    }

    #[test]
    fn exit_codes_are_distinct_and_stable() {
        assert_eq!(exit::SUCCESS, 0);
        assert_eq!(exit::TEST_FAILURES, 1);
        assert_eq!(exit::CLI_ERROR, 2);
        assert_eq!(exit::BASELINE_FAILED, 3);
    }

    #[test]
    fn exit_error_carries_baseline_code() {
        let err = ExitError::baseline("baseline test run failed; aborting");
        assert_eq!(err.code(), exit::BASELINE_FAILED);
        assert_eq!(err.to_string(), "baseline test run failed; aborting");
    }

    #[test]
    fn string_errors_default_to_cli_error() {
        let from_string: ExitError = "no test files discovered".to_string().into();
        assert_eq!(from_string.code(), exit::CLI_ERROR);

        let from_str: ExitError = "no test command configured".into();
        assert_eq!(from_str.code(), exit::CLI_ERROR);
    }

    #[test]
    fn parses_run_subcommand_with_shard() {
        let cli = Cli::parse_from(["lua-mutation-test", "run", "src", "--shard", "2/5"]);
        match cli.command {
            Command::Run(args) => {
                assert_eq!(args.shard, Some("2/5".to_string()));
            }
            _ => panic!("expected run subcommand"),
        }
    }

    #[test]
    fn run_shard_defaults_to_none() {
        let cli = Cli::parse_from(["lua-mutation-test", "run", "src"]);
        match cli.command {
            Command::Run(args) => {
                assert_eq!(args.shard, None);
            }
            _ => panic!("expected run subcommand"),
        }
    }
}
