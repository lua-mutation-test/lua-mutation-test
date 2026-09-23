# lua-mutation-test

> Find the bugs your tests miss — mutation testing for Lua and Neovim plugins.

[![Rust](https://github.com/lua-mutation-test/lua-mutation-test/actions/workflows/rust.yml/badge.svg)](https://github.com/lua-mutation-test/lua-mutation-test/blob/main/.github/workflows/rust.yml)
[![GitHub Release](https://img.shields.io/github/v/release/lua-mutation-test/lua-mutation-test)](https://github.com/lua-mutation-test/lua-mutation-test/releases)
[![License](https://img.shields.io/github/license/lua-mutation-test/lua-mutation-test)](LICENSE)
[![Mutation testing badge](https://img.shields.io/endpoint?style=flat&url=https%3A%2F%2Fbadge-api.stryker-mutator.io%2Fgithub.com%2Flua-mutation-test%2Flua-mutation-test%2Fmain)](https://dashboard.stryker-mutator.io/reports/github.com/lua-mutation-test/lua-mutation-test/main)
[![Docs](https://img.shields.io/badge/docs-gh--pages-blue)](https://lua-mutation-test.github.io/lua-mutation-test/)

A mutation testing tool for Lua, written in Rust.

`lua-mutation-test` parses Lua source code with [tree-sitter](https://tree-sitter.github.io/tree-sitter/),
generates mutants, runs your test suite against them, and reports mutation scores.

## Why mutation testing?

100% line coverage can still miss bugs — especially in Neovim plugins where a
flipped condition (`==` vs `~=`, `and` vs `or`) or an off-by-one keeps every
line green while silently breaking behavior.

`lua-mutation-test` seeds those small faults into your code and checks whether
your tests catch them. Survivors point at weak tests; a high mutation score
means your suite actually guards behavior.

## Quick start for a Neovim plugin codebase

```bash
cargo install lua-mutation-test
cd ~/plugins/my-nvim-plugin
lmut run lua --test-command 'busted'
```

Example output:

```text
Mutation score: 87.5% (42 killed, 6 survived, 0 timed out)
Survived: lua/init.lua:42:5 — changed `==` to `~=`
Survived: lua/config.lua:17:3 — removed call to `vim.notify`
```

Works with [lux](https://github.com/nvim-neorocks/lux) and
[rocks.nvim](https://github.com/nvim-neorocks/rocks.nvim) workflows — just point
`--test-command` at `busted`, `plenary`/`mini.test`, or your usual runner.

> 🤖 AI-assisted contributions welcome! Agent-generated PRs are first-class here —
> see [CONTRIBUTING.md](CONTRIBUTING.md) for the agentic workflow.

## Features

### Current

- Parse Lua source code using the [tree-sitter-lua](https://github.com/tree-sitter-grammars/tree-sitter-lua) grammar (pulled from crates.io).
- Generate mutants from Lua source code (arithmetic, relational, logical, literal, unary, and control-flow operators).
- Run a given Lua test suite against each mutant with process isolation and timeouts.
- Run mutants in parallel (`--workers`) with an incremental result cache (`.lua-mutation-test/cache/`) that reuses results for unchanged files and reports `cached`/`ran` counts.
- Watch mode (`lmut watch`) that re-runs affected mutants when source files change.
- Compute and report mutation scores (summary, per-mutant, JSON, CTRF, and HTML).
- Static heuristics that detect likely-equivalent mutants (e.g., `x + 0`) before execution.
- Configurable mutation operators and ignore patterns.

### Planned

- Additional mutation operators.
- More equivalent-mutant heuristics.

## Installation

### From GitHub Releases

Download the pre-built binary for your platform from the
[releases page](https://github.com/lua-mutation-test/lua-mutation-test/releases) and place it
on your `PATH`.

The release archive contains both the `lua-mutation-test` binary and the shorter
`lmut` alias.

### From crates.io

```bash
cargo install lua-mutation-test
```

This installs the `lua-mutation-test` binary. The `lmut` alias is included.

### From source

If you prefer to build from source, see [CONTRIBUTING.md](CONTRIBUTING.md) for the
development setup.

## GitHub Action

Run mutation testing in CI with the
[lua-mutation-test-action](https://github.com/lua-mutation-test/lua-mutation-test-action).
It downloads a pinned `lmut` binary from GitHub Releases — no Rust toolchain
needed — runs `lmut run`, and can fail the build when the mutation score drops
below a threshold:

```yaml
- name: Run mutation testing
  uses: lua-mutation-test/lua-mutation-test-action@v0
  with:
    path: lua
    test-command: busted
    fail-under: 80
```

See the
[action repository](https://github.com/lua-mutation-test/lua-mutation-test-action)
for all inputs (`version`, `config`, `timeout`, `args`, PR comments, job
summaries, annotations) and outputs (`mutation-score`, `killed`, `survived`).

**PR vs. nightly:** mutate only the diff on pull requests, run the full suite
nightly:

```yaml
# PR workflow (fast, diff-scoped)
- run: git fetch origin main
- uses: lua-mutation-test/lua-mutation-test-action@v0
  with:
    args: --changed-since origin/main
    fail-under: 80
# Nightly workflow: same step without `args` for a full run.
```

Locally: `lmut run --changed-since origin/main` on a branch, plain `lmut run`
for the whole project. Scoped scores cover the diff only.

## Usage

The examples below use the `lmut` binary. The full `lua-mutation-test` name works
identically.

Run mutation testing against a file or directory. If no path is given, it
mutates the current directory:

```bash
lmut run
lmut run <path-to-lua-file-or-directory>
```

Run with a custom test command and timeout:

```bash
lmut run src --test-command 'busted' --timeout 30
```

Run mutants in parallel and watch for changes:

```bash
lmut run src --workers 4
lmut watch src --test-command 'busted'
```

Shard a run across CI matrix jobs (balanced by mutant count, not by file):

```bash
lmut run src --shard 1/3
lmut run src --shard 2/3
lmut run src --shard 3/3
```

List available mutation operators:

```bash
lmut list-operators
```

Create a sample configuration file:

```bash
lmut init
```

See the [CLI Reference](https://lua-mutation-test.github.io/lua-mutation-test/cli-reference/) for the
full list of commands and options.

> Note: The project is a work in progress. APIs, CLI flags, and behavior may change.

## Configuration

Create a `.lua-mutation-test.toml` (or `.lua-mutation-test.json`) file in your project root:

```toml
version = "1"
test_command = "busted"
timeout = 30
test_globs = ["*_spec.lua", "*_test.lua", "test_*.lua"]
source_globs = ["*.lua"]
difficulty = "very_hard"

[files]
exclude = ["*_test.lua", "*_spec.lua"]

[operators]
exclude = ["control_flow"]

[functions]
exclude = ["helpers_*"]
```

Pass an explicit config path with `--config`:

```bash
lmut --config path/to/config.toml run src
```

CLI flags override configuration file values.

Generate reports after a run:

```bash
lmut run src --report-format json --report-output report.json
lmut run src --report-format ctrf --report-output ctrf-report.json
lmut run src --report-format html --report-output report.html
lmut run src --report-format stryker --report-output mutation-testing-report.json
```

Publish mutation results to the Stryker dashboard (`dashboard.stryker-mutator.io`):

```bash
export STRYKER_DASHBOARD_API_KEY=xxx
curl -X PUT "https://dashboard.stryker-mutator.io/api/reports/github.com/lua-mutation-test/lua-mutation-test/main" \
  -H "X-Api-Key: $STRYKER_DASHBOARD_API_KEY" \
  -H "Content-Type: application/json" \
  -d @mutation-testing-report.json
```

The badge at the top of this README shows the mutation score of the Rust
codebase itself: CI runs [cargo-mutants](https://mutants.rs/) on every push to
`main`, converts `mutants.out/outcomes.json` with
`scripts/cargo-mutants-to-stryker.py`, and publishes the same Stryker-schema
report (see `.github/workflows/mutation.yml`).

> Note: that workflow self-tests this repo's Rust code with cargo-mutants — it
> is unrelated to the
> [lua-mutation-test-action](https://github.com/lua-mutation-test/lua-mutation-test-action),
> which mutation-tests downstream Lua projects with `lmut`.

## Architecture

- **Rust CLI**: Entry point and orchestration.
- **tree-sitter Lua parser**: Grammar crate from crates.io used to parse Lua source into an AST.
- **Mutant generator**: Applies configurable mutation operators and detects likely-equivalent mutants with static heuristics.
- **Test runner**: Runs the test suite against each mutant in an isolated temporary copy of the project.
- **Reporter**: Computes mutation scores and emits summary, per-mutant, JSON, CTRF, HTML, or Stryker (`mutation-testing-report.json`, schema v2) reports.

See [docs/architecture.md](docs/architecture.md) for more details, including the
list of known equivalent-mutant patterns.

## Versioning

This project follows [ZeroVer](https://0ver.org/): all releases will remain in the
`0.x` range for the foreseeable future while the API and feature set stabilizes.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for
development setup, workflow guidelines, and commit conventions.

## License

This project is licensed under the [Apache License 2.0](LICENSE). You may use,
modify, and distribute it freely, including for commercial purposes, subject to
the terms and conditions of the license.
