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
| `--report-format <FORMAT>` | Report format: `summary`, `per-mutant`, `json`, `ctrf`, `html`, `stryker`. |
| `--report-output <PATH>` | Write the generated report to this path. |

#### Examples

```bash
lmut run src
lmut run file.lua --test-command 'busted' --timeout 30 --report-format json
lmut run src --report-format ctrf --report-output ctrf-report.json
lmut run src --report-format stryker --report-output mutation-testing-report.json
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

| Code | Meaning |
|------|---------|
| `0` | Success. |
| `1` | Test failures detected (mutants survived or baseline failed). |
| `2` | CLI or configuration error. |

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
| `parallelism` | integer | Number of concurrent mutant runs. |
| `output` | list of strings | Report output formats. |

Use `lmut list-operators` to see available operator ids.

### Example

```toml
version = "1"
test_command = "make test"
difficulty = "medium"

[operators]
include = ["arithmetic_operator", "relational_operator"]
```
