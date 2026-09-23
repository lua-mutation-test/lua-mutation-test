use clap::Parser;
use lua_mutation_test::adapter::FrameworkAdapter;
use lua_mutation_test::baseline::run_baseline;
use lua_mutation_test::cli::{exit, Cli, Command, RunArgs, WatchArgs};
use lua_mutation_test::config::{Config, DEFAULT_CONFIG_PATH};
use lua_mutation_test::incremental;
use lua_mutation_test::mutant::{Mutant, MutantGenerator};
use lua_mutation_test::operators::default_operators;
use lua_mutation_test::parser::Parser as LuaParser;
use lua_mutation_test::report::{generate_report, ReportData, ReportFormat};
use lua_mutation_test::runner::RunnerConfig;
use lua_mutation_test::score::score_results;
use lua_mutation_test::test_discovery::discover_tests;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            e.print().expect("failed to print CLI error");
            return ExitCode::from(exit::CLI_ERROR as u8);
        }
    };

    match run(cli) {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(exit::CLI_ERROR as u8)
        }
    }
}

fn run(cli: Cli) -> Result<i32, String> {
    let _ = ctrlc::set_handler(|| {
        eprintln!("\nInterrupt received, flushing cache and killing child processes...");
        lua_mutation_test::runner::kill_all_child_processes();
        lua_mutation_test::incremental::set_interrupt_flag(true);
    });

    let config_path = cli.config.clone().unwrap_or_else(default_config_path);
    let mut config = if config_path.exists() {
        Config::from_file(&config_path).map_err(|e| e.to_string())?
    } else {
        Config::default()
    };

    config.merge(config_from_cli(&cli));

    match cli.command {
        Command::Run(args) => run_pipeline(&args.path, &args, config),
        Command::Watch(args) => run_watch(args, config),
        Command::ListMutants(args) => {
            let source = std::fs::read_to_string(&args.path)
                .map_err(|e| format!("failed to read {}: {e}", args.path.display()))?;
            let mut parser = LuaParser::new().map_err(|e| e.to_string())?;
            let tree = parser.parse_source(&source).map_err(|e| e.to_string())?;
            let generator = MutantGenerator::new(default_operators());
            let mutants = generator.generate(&args.path, &source, &tree);
            for mutant in mutants {
                println!(
                    "{}",
                    serde_json::to_string(&mutant).map_err(|e| e.to_string())?
                );
            }
            Ok(exit::SUCCESS)
        }
        Command::ListOperators => {
            let operators = default_operators();
            if operators.is_empty() {
                println!("Available mutation operators: (none configured yet)");
            } else {
                println!("Available mutation operators:");
                for operator in operators {
                    println!("  {}", operator.id());
                }
            }
            Ok(exit::SUCCESS)
        }
        Command::Init => {
            std::fs::write(
                DEFAULT_CONFIG_PATH,
                r#"version = "1"
test_command = "busted"
timeout = 30
test_globs = ["*_spec.lua", "*_test.lua", "test_*.lua"]
source_globs = ["*.lua"]

# Difficulty controls the trade-off between speed and completeness.
# Options: "very_easy", "easy", "normal", "medium", "hard", "very_hard".
# Default is "very_hard" (all mutants).
# difficulty = "medium"

# Operators to include or exclude. Use operator ids from `list-operators`.
# [operators]
# include = ["arithmetic_operator", "relational_operator"]
# exclude = ["control_flow"]
"#,
            )
            .map_err(|e| format!("failed to write sample config: {e}"))?;
            println!("Created {DEFAULT_CONFIG_PATH}");
            Ok(exit::SUCCESS)
        }
    }
}

fn run_watch(args: WatchArgs, config: Config) -> Result<i32, String> {
    let path = args.path.clone();
    let debounce = Duration::from_millis(args.debounce);
    let args_for_watch = args.clone();
    let path_for_watch = path.clone();

    // Perform an initial incremental run.
    run_pipeline(path.clone(), &args, config.clone())?;
    println!("Watching for changes... (press Ctrl+C to stop)");

    incremental::watch::watch_project(
        &path,
        debounce,
        move || {
            println!("Change detected, re-running mutation tests...");
            run_pipeline(path_for_watch.clone(), &args_for_watch, config.clone()).map(|code| {
                if code == exit::TEST_FAILURES {
                    println!("Surviving mutants detected.");
                }
            })
        },
        || false,
    )?;

    Ok(exit::SUCCESS)
}

fn run_pipeline<P>(path: P, args: &impl RunArgsLike, config: Config) -> Result<i32, String>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    let test_command = config
        .test_command
        .clone()
        .or(args.test_command())
        .ok_or("no test command configured")?;

    // Resolve the --changed-since scope before discovery so changed test
    // files can join the baseline run and sources can be filtered below.
    // Errors name the ref and propagate as CLI errors (exit code 2).
    let changed_scope = match args.changed_since() {
        Some(base_ref) => {
            let git_dir = if path.is_file() {
                path.parent().unwrap_or(Path::new(".")).to_path_buf()
            } else {
                path.to_path_buf()
            };
            let changed = lua_mutation_test::incremental::change_detection::changed_since_files(
                &git_dir, &base_ref,
            )?;
            Some((base_ref, changed))
        }
        None => None,
    };

    // Discover tests and run baseline.
    eprintln!("Discovering test files...");
    let mut tests = discover_tests(path, &config.test_globs);
    if let Some((_, changed)) = &changed_scope {
        // Guarantee changed test files are included even when the positional
        // path points at sources only; the full discovered set still runs.
        for file in changed {
            if file.is_file() && matches_any_glob(&config.test_globs, file) && !tests.contains(file)
            {
                tests.push(file.clone());
            }
        }
        tests.sort();
    }
    eprintln!("  discovered {} test file(s)", tests.len());
    if tests.is_empty() {
        return Err("no test files discovered".to_string());
    }
    let adapter = FrameworkAdapter::from_config(config.framework.as_deref(), Some(&test_command));
    let test_refs: Vec<&Path> = tests.iter().map(|p| p.as_path()).collect();
    eprintln!("Running baseline tests...");
    let baseline = run_baseline(&adapter, &test_refs);
    if !baseline.passed() {
        return Err("baseline test run failed; aborting".to_string());
    }
    eprintln!("  baseline passed");

    // Discover source files and generate mutants.
    eprintln!("Discovering source files...");
    let mut source_files = discover_source_files(path, &config.source_globs, &config.files)?;
    if let Some((base_ref, changed)) = &changed_scope {
        // Intersect the git changed set with discovered sources so external
        // path plumbing cannot widen the scope. An empty scope is valid
        // (e.g. docs-only PRs): the pipeline below prints the normal
        // zero-count summary and exits 0.
        let changed_set: std::collections::HashSet<PathBuf> = changed
            .iter()
            .map(|file| canonical_or_absolute(file))
            .collect();
        source_files.retain(|file| changed_set.contains(&canonical_or_absolute(file)));
        eprintln!(
            "scoped to {} file(s) changed since {}",
            source_files.len(),
            base_ref
        );
    }
    eprintln!("  discovered {} source file(s)", source_files.len());
    eprintln!("Generating mutants...");
    let mut mutants = Vec::new();
    let mut equivalent_results = Vec::new();
    let mut parser = LuaParser::new().map_err(|e| e.to_string())?;
    for (i, file) in source_files.iter().enumerate() {
        let source = std::fs::read_to_string(file)
            .map_err(|e| format!("failed to read {}: {e}", file.display()))?;
        let tree = parser.parse_source(&source).map_err(|e| e.to_string())?;
        let operators: Vec<_> = default_operators()
            .into_iter()
            .filter(|op| config.operators.matches(op.id()))
            .collect();
        if operators.is_empty() {
            return Err("no mutation operators selected; check your configuration".to_string());
        }
        let generator = MutantGenerator::new(operators);
        let (valid, _invalid, equivalent) = generator.generate_validated(file, &source, &tree);
        let valid = apply_mutant_limit(valid, &config);
        mutants.extend(valid.into_iter().map(|m| (m, source.clone())));
        equivalent_results.extend(equivalent.into_iter().map(|m| {
            let reason = m.equivalent_reason.clone().unwrap_or_default();
            lua_mutation_test::result::MutantResult::Equivalent { mutant: m, reason }
        }));
        if (i + 1).is_multiple_of(10) || i + 1 == source_files.len() {
            eprintln!(
                "  processed {}/{} source file(s), {} mutant(s) so far",
                i + 1,
                source_files.len(),
                mutants.len()
            );
        }
    }
    eprintln!("  generated {} mutant(s)", mutants.len());

    // Run each mutant incrementally.
    let timeout = config
        .timeout
        .or(args.timeout())
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(30));
    let runner_config = RunnerConfig {
        command: test_command.split_whitespace().map(String::from).collect(),
        timeout,
        project_root: path.to_path_buf(),
        snippet_limit: 1000,
    };

    let workers = resolve_workers(args.workers(), config.parallelism);
    let incremental_result = incremental::run_incremental(
        path,
        &config,
        &runner_config,
        &source_files,
        mutants,
        workers,
    )?;

    let mut results = incremental_result.results;
    results.extend(equivalent_results);

    // Score and report.
    let score = score_results(&results);
    println!(
        "Mutation score: {:.2}% (killed {}, survived {}, timed out {}, errored {}, equivalent {}, cached {}, ran {})",
        score.overall.percentage().unwrap_or(0.0),
        score.overall.killed,
        score.overall.survived,
        score.overall.timed_out,
        score.overall.error,
        score.overall.equivalent,
        incremental_result.cached,
        incremental_result.ran,
    );

    if let Some(format) = args.report_format() {
        let format: ReportFormat = format.parse().map_err(|e: String| e)?;
        let data = ReportData {
            results: &results,
            project_root: path,
            source_paths: &source_files,
        };
        let report_output = args.report_output();
        let output = report_output.as_deref();
        generate_report(format, data, output)?;
    }

    if score.overall.survived > 0 {
        Ok(exit::TEST_FAILURES)
    } else {
        Ok(exit::SUCCESS)
    }
}

fn num_cpus_like() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// Resolves the mutant-execution worker count: CLI `--workers` wins, then the
/// `parallelism` config file value, then the CPU count.
fn resolve_workers(cli_workers: Option<usize>, config_parallelism: Option<usize>) -> usize {
    cli_workers
        .or(config_parallelism)
        .unwrap_or_else(num_cpus_like)
}

/// Applies the difficulty-based per-mutable-item cap, keeping at most
/// `max_mutants_per_item` mutants per operator per source region.
fn apply_mutant_limit(mutants: Vec<Mutant>, config: &Config) -> Vec<Mutant> {
    let max = match config.difficulty.max_mutants_per_item() {
        Some(max) if max > 0 => max,
        _ => return mutants,
    };

    let mut groups: std::collections::HashMap<(PathBuf, usize, usize, String), Vec<Mutant>> =
        std::collections::HashMap::new();
    for mutant in mutants {
        let key = (
            mutant.file.clone(),
            mutant.start_byte,
            mutant.end_byte,
            mutant.operator.clone(),
        );
        groups.entry(key).or_default().push(mutant);
    }

    let mut limited = Vec::new();
    for (_, group) in groups {
        let mut group = group;
        if group.len() > max {
            // Keep the first `max` mutants by replacement string for determinism.
            // All mutants in the group share file/operator/location, so the
            // replacement is the only meaningful discriminator.
            group.sort_by(|a, b| a.replacement.cmp(&b.replacement));
            group.truncate(max);
        }
        limited.extend(group);
    }

    // Restore a stable order (by source location) before returning.
    limited.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| a.column.cmp(&b.column))
            .then_with(|| a.operator.cmp(&b.operator))
            .then_with(|| a.replacement.cmp(&b.replacement))
    });
    limited
}

/// Trait that abstracts over `RunArgs` and `WatchArgs` so the pipeline can be
/// shared by both commands.
trait RunArgsLike {
    fn test_command(&self) -> Option<String>;
    fn timeout(&self) -> Option<u64>;
    fn workers(&self) -> Option<usize>;
    fn changed_since(&self) -> Option<String>;
    fn report_format(&self) -> Option<String>;
    fn report_output(&self) -> Option<PathBuf>;
}

impl RunArgsLike for RunArgs {
    fn test_command(&self) -> Option<String> {
        self.test_command.clone()
    }
    fn timeout(&self) -> Option<u64> {
        self.timeout
    }
    fn workers(&self) -> Option<usize> {
        self.workers
    }
    fn changed_since(&self) -> Option<String> {
        self.changed_since.clone()
    }
    fn report_format(&self) -> Option<String> {
        self.report_format.clone()
    }
    fn report_output(&self) -> Option<PathBuf> {
        self.report_output.clone()
    }
}

impl RunArgsLike for WatchArgs {
    fn test_command(&self) -> Option<String> {
        self.test_command.clone()
    }
    fn timeout(&self) -> Option<u64> {
        self.timeout
    }
    fn workers(&self) -> Option<usize> {
        self.workers
    }
    fn changed_since(&self) -> Option<String> {
        None
    }
    fn report_format(&self) -> Option<String> {
        None
    }
    fn report_output(&self) -> Option<PathBuf> {
        None
    }
}

fn discover_source_files(
    path: &Path,
    globs: &[String],
    filter: &lua_mutation_test::config::Filter,
) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    if path.is_file() {
        let file = path.to_path_buf();
        if filter.matches(&file.to_string_lossy()) {
            files.push(file);
        }
        return Ok(files);
    }

    for glob in globs {
        let pattern = path.join(glob).to_string_lossy().to_string();
        for entry in glob::glob(&pattern).map_err(|e| e.to_string())? {
            let p = entry.map_err(|e| e.to_string())?;
            if p.is_file() && filter.matches(&p.to_string_lossy()) {
                files.push(p);
            }
        }
    }

    files.sort();
    files.dedup();
    Ok(files)
}

fn default_config_path() -> PathBuf {
    PathBuf::from(DEFAULT_CONFIG_PATH)
}

/// Best-effort canonicalization for scope comparison. Falls back to an
/// absolute path when the file does not exist (e.g. deleted in the diff).
fn canonical_or_absolute(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(path)
        }
    })
}

/// Returns true when `path` matches any of the given glob patterns, either
/// by file name or by full path.
fn matches_any_glob(patterns: &[String], path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    let full = path.to_string_lossy();
    patterns.iter().any(|pattern| {
        glob::Pattern::new(pattern)
            .map(|matcher| matcher.matches(&name) || matcher.matches(&full))
            .unwrap_or(false)
    })
}

fn config_from_cli(cli: &Cli) -> Config {
    let mut config = Config {
        test_globs: Vec::new(),
        source_globs: Vec::new(),
        ..Config::default()
    };
    match &cli.command {
        Command::Run(args) => {
            config.test_command = args.test_command.clone();
            config.timeout = args.timeout;
            if let Some(workers) = args.workers {
                config.parallelism = Some(workers);
            }
        }
        Command::Watch(args) => {
            config.test_command = args.test_command.clone();
            config.timeout = args.timeout;
            if let Some(workers) = args.workers {
                config.parallelism = Some(workers);
            }
        }
        _ => {}
    }
    config
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_mutant(operator: &str, line: usize, column: usize) -> Mutant {
        dummy_mutant_with_replacement(operator, line, column, "y")
    }

    fn dummy_mutant_with_replacement(
        operator: &str,
        line: usize,
        column: usize,
        replacement: &str,
    ) -> Mutant {
        Mutant {
            id: format!("{operator}-{line}-{column}-{replacement}"),
            operator: operator.to_string(),
            file: PathBuf::from("src/foo.lua"),
            start_byte: 0,
            end_byte: 1,
            line,
            column,
            original: "x".to_string(),
            replacement: replacement.to_string(),
            equivalent_reason: None,
        }
    }

    #[test]
    fn apply_mutant_limit_caps_per_operator_per_item() {
        let mut config = Config::default();
        config.difficulty = lua_mutation_test::config::Difficulty::VeryEasy;

        // Same source item, two operators, two replacements each.
        let mutants = vec![
            dummy_mutant_with_replacement("arithmetic_operator", 1, 1, "a"),
            dummy_mutant_with_replacement("arithmetic_operator", 1, 1, "b"),
            dummy_mutant_with_replacement("relational_operator", 1, 1, "c"),
            dummy_mutant_with_replacement("relational_operator", 1, 1, "d"),
        ];

        let limited = apply_mutant_limit(mutants, &config);
        assert_eq!(limited.len(), 2);
        assert!(limited.iter().any(|m| m.operator == "arithmetic_operator"));
        assert!(limited.iter().any(|m| m.operator == "relational_operator"));
    }

    #[test]
    fn apply_mutant_limit_keeps_all_on_very_hard() {
        let config = Config::default();
        let mutants = vec![
            dummy_mutant("control_flow", 1, 1),
            dummy_mutant("arithmetic_operator", 2, 1),
            dummy_mutant("relational_operator", 3, 1),
        ];

        let limited = apply_mutant_limit(mutants, &config);
        assert_eq!(limited.len(), 3);
    }

    #[test]
    fn resolve_workers_prefers_cli_over_config() {
        assert_eq!(resolve_workers(Some(8), Some(2)), 8);
    }

    #[test]
    fn resolve_workers_falls_back_to_config_parallelism() {
        assert_eq!(resolve_workers(None, Some(2)), 2);
    }

    #[test]
    fn resolve_workers_defaults_to_cpu_count() {
        assert_eq!(resolve_workers(None, None), num_cpus_like());
        assert!(resolve_workers(None, None) >= 1);
    }
}
