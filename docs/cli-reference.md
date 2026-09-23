# CLI Reference

`lua-mutation-test` is a command-line mutation testing tool for Lua. The `lmut` binary is a short alias for `lua-mutation-test`; both names share the same CLI and produce identical output.

This page documents the available subcommands, options, and exit codes. Keep it
in sync with `src/cli.rs`; when you add, remove, or change a CLI flag or
command, update this page before committing.

## Global options

These options can be used before any subcommand.

| Option | Description |
|--------|-------------|
| `-c`, `--config <PATH>` | Path to a configuration file. |
| `-v`, `--verbose` | Enable verbose output. |
| `-q`, `--quiet` | Suppress non-essential output. |
| `-h`, `--help` | Print help information. |
| `-V`, `--version` | Print version information. |

## Subcommands

### `run`

Run mutation testing against a Lua file or directory. If `<PATH>` is omitted, the
current directory is used.

```bash
lmut run [PATH] [OPTIONS]
```

#### Arguments

| Argument | Description |
|----------|-------------|
| `[PATH]` | Path to a Lua file or directory to mutate. Defaults to `.`. |

#### Options

| Option | Description |
|--------|-------------|
| `--test-command <COMMAND>` | Custom shell command used to run tests. |
| `--timeout <SECONDS>` | Timeout in seconds for each mutant test run. |
| `--workers <N>` | Number of parallel workers for mutant execution. Overrides the `parallelism` config value. Defaults to the number of available CPUs. |
| `--changed-since <git-ref>` | Only mutate source files changed since the given git ref (e.g. `origin/main`). See [Changed-files mode](#changed-files-mode) below. |
| `--report-format <FORMAT>` | Report format: `summary`, `per-mutant`, `json`, `ctrf`, `html`, `stryker`. |
| `--report-output <PATH>` | Write the generated report to this path. |
| `--workers <N>` | Number of parallel workers for mutant execution. |
| `--shard <K/N>` | Run only shard `K` of `N` (e.g. `--shard 2/5`). Partitions the mutant inventory deterministically by mutant count. |

#### Sharding for CI matrix fan-out

The only way to parallelize across CI matrix jobs without sharding is
splitting the positional `PATH`, which skews badly (one shard gets the big
file, the rest idle). `--shard <k>/<n>` partitions the generated mutant
inventory deterministically by mutant count — not by file — so shards stay
balanced regardless of file layout:

- The full inventory is sorted by stable mutant identity (file + location +
  operator) and then strided: position `i` belongs to shard `k/n` when
  `i % n == k - 1`.
- Sorting makes partitioning stable across runs and machines, so retries hit
  the same mutants and the incremental cache stays coherent.
- Each shard prints the normal summary line and exits with the normal codes;
  aggregation across shards is downstream's job (e.g. merge JSON/CTRF reports).

`--shard 1/3` + `2/3` + `3/3` cover the full inventory exactly once with sizes
differing by at most one.

```yaml
# GitHub Actions matrix example
strategy:
  matrix:
    shard: [1, 2, 3]
steps:
  - run: lmut run src --test-command 'busted' --shard ${{ matrix.shard }}/3
```

Results are cached incrementally in `.lua-mutation-test/cache/`: mutants that
already ran with unchanged sources and configuration are reused from cache
instead of re-executed. The summary line reports both counts
(`... cached <N>, ran <M>`).

#### Examples

```bash
lmut run src
lmut run file.lua --test-command 'busted' --timeout 30 --report-format json
lmut run src --workers 4
lmut run --changed-since origin/main
lmut run src --report-format ctrf --report-output ctrf-report.json
lmut run src --report-format stryker --report-output mutation-testing-report.json
lmut run src --shard 1/3 --report-format json --report-output shard-1.json
```

#### Changed-files mode

`--changed-since <git-ref>` scopes mutant generation to source files
differing from the base ref, for fast PR feedback:

```bash
git fetch origin
lmut run --changed-since origin/main
```

Behavior:

- The changed set is computed inside `lmut` from `merge-base(HEAD, <ref>)`
  plus uncommitted working-tree changes, then intersected with discovered
  sources (positional `path`, `source_globs`, filters). Changed test files
  are added to the baseline run even when `path` points at sources only.
- The run prints a scope banner (`scoped to N file(s) changed since <ref>`)
  and otherwise behaves like a normal run: same summary line (including
  `cached`/`ran`), same reports (covering only scoped results), same config.
  Scores are diff-scoped, not whole-project scores.
- An empty scope (e.g. a docs-only PR) prints the normal summary with zero
  counts and exits `0`.
- Errors exit with code `2`: an unknown or unresolvable ref (the message
  names the ref), or running outside a git work tree (`--changed-since`
  requires git).

### `watch`

Watch source files and re-run mutation tests incrementally when they change.
Performs an initial `run`, then re-runs affected mutants on every change.

```bash
lmut watch <PATH> [OPTIONS]
```

#### Arguments

| Argument | Description |
|----------|-------------|
| `<PATH>` | Path to a Lua file or directory to mutate. |

#### Options

| Option | Description |
|--------|-------------|
| `--test-command <COMMAND>` | Custom shell command used to run tests. |
| `--timeout <SECONDS>` | Timeout in seconds for each mutant test run. |
| `--debounce <MS>` | Debounce duration in milliseconds before re-running after a file change. Defaults to `500`. |
| `--workers <N>` | Number of parallel workers for mutant execution. Overrides the `parallelism` config value. Defaults to the number of available CPUs. |

#### Examples

```bash
lmut watch src
lmut watch src --test-command 'busted' --debounce 250
```

### `list-mutants`

List the mutants that would be generated for a given Lua source file.

```bash
lmut list-mutants <PATH>
```

#### Arguments

| Argument | Description |
|----------|-------------|
| `<PATH>` | Path to a Lua source file. |

### `list-operators`

List the available mutation operators.

```bash
lmut list-operators
```

### `init`

Create a sample configuration file in the current directory.

```bash
lmut init
```

## Exit codes

These exit codes are a stability contract: wrappers such as the
`lua-mutation-test-action` GitHub Action distinguish clean kills,
surviving mutants, red baselines, and misconfiguration without grepping
log text.

| Code | Name | Meaning |
|------|------|---------|
| `0` | Success | All mutants killed (mutation testing completed, nothing survived). |
| `1` | Survivors | Mutation testing completed, but one or more mutants survived. Use this for quality-gate decisions. |
| `2` | CLI/config error | Startup or configuration failure: no test command, no test files discovered, bad config, parse errors, invalid flags, etc. |
| `3` | Baseline failure | The unmodified test suite failed, so no mutant result would be meaningful. Fix the baseline before trusting any score. |

## Configuration file

The `--config` option points to a TOML or JSON file that controls operators,
includes, excludes, and test-runner settings.

### Options

| Option | Type | Description |
|--------|------|-------------|
| `version` | string | Config schema version. Must be `"1"`. |
| `test_command` | string | Shell command used to run the test suite. |
| `framework` | string | Test framework adapter: `"busted"` or `"luaunit"`. |
| `timeout` | integer | Timeout in seconds for each mutant test run. |
| `test_globs` | list of strings | Glob patterns for discovering test files. |
| `source_globs` | list of strings | Glob patterns for discovering source files. |
| `difficulty` | string | `"very_easy"`, `"easy"`, `"normal"`, `"medium"`, `"hard"`, `"very_hard"`. Controls the per-operator-per-item mutant cap. Default is `"very_hard"`. |
| `operators.include` | list of strings | Only run these operator ids. |
| `operators.exclude` | list of strings | Skip these operator ids. |
| `parallelism` | integer | Number of parallel workers for mutant execution. The `--workers` CLI flag takes precedence when both are set. |
| `output` | list of strings | Report output formats. |

#### Difficulty levels

The `difficulty` setting limits how many mutants each operator produces per
mutable source item (e.g., a binary expression or condition). Lower levels run
faster while still visiting every mutable site.

| Level | Mutants per operator per item |
|-------|-------------------------------|
| `very_easy` | 1 |
| `easy` | 2 |
| `normal` | 3 |
| `medium` | 5 |
| `hard` | 10 |
| `very_hard` | unlimited |

Use `lmut list-operators` to see available operator ids.

### Example

```toml
version = "1"
test_command = "make test"
difficulty = "medium"

[operators]
include = ["arithmetic_operator", "relational_operator"]
```
