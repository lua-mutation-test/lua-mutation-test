# Architecture

This page describes the high-level architecture of `lua-mutation-test`.

## Overview

`lua-mutation-test` is a command-line tool written in Rust. It reads Lua source files,
generates mutants, runs a Lua test suite against each mutant, and reports mutation
scores.

## Components

```text
+----------------+      +-------------------+      +------------------+
| CLI / Config   |----->| Mutant Generator  |----->| Mutant Runner    |
+----------------+      +-------------------+      +------------------+
                                |                           |
                                v                           v
                       +----------------+          +------------------+
                       | tree-sitter    |          | Lua test suite   |
                       | Lua parser     |          | (busted/luaunit/ |
                       +----------------+          | custom command)  |
                                                   +------------------+
                                |
                                v
                       +------------------+
                       | Reporter         |
                       | (CLI/JSON/HTML)  |
                       +------------------+
```

### CLI / Config

- Parses command-line arguments and configuration files.
- Selects files, operators, and test runner.
- Entry point for `run`, `list-mutants`, `list-operators`, and `init` subcommands.
- See the [CLI Reference](cli-reference.md) for the complete command documentation.

### Lua Parser

- Uses a vendored [tree-sitter-lua](https://github.com/tree-sitter-grammars/tree-sitter-lua)
  grammar to parse Lua source into an AST.
- Provides AST traversal utilities and source-location mapping.

### Mutant Generator

- Applies configured mutation operators to AST nodes.
- Produces one or more mutants per mutation point.
- Deduplicates mutants and discards any mutation that does not produce syntactically valid Lua.

### Mutant Runner

- Writes each mutant to a temporary file.
- Runs the project's test suite in an isolated process.
- Enforces timeouts and captures exit codes and output.

### Reporter

- Aggregates results into mutation scores.
- Generates CLI summaries, JSON, CTRF, HTML, and Stryker (`mutation-testing-report.json`, schema v2) reports.
- Stryker status mapping: `Killed` → `Killed`, `Survived` → `Survived`, `Timeout` → `Timeout`, `Error` → `RuntimeError`, `Equivalent` → `Ignored`. Locations use 1-based line/column; thresholds default to `{high: 80, low: 60}`.

## Data flow

1. The CLI discovers Lua source files and test files based on configuration.
2. The parser produces an AST for each source file.
3. The mutant generator walks the AST and creates mutants.
4. Static heuristics classify obviously equivalent mutants before they are executed.
5. The runner executes tests against each remaining mutant.
6. The reporter categorizes mutants (killed, survived, timed out, error, equivalent) and computes scores.

## Equivalent-mutant heuristics

Equivalent mutants do not change program behavior, so they survive every test and
inflate mutation scores while wasting execution time. Perfect equivalence
detection is undecidable, but `lua-mutation-test` applies lightweight static
heuristics to catch obvious cases before they are run.

Detected equivalent mutants are skipped during execution and reported separately
with the heuristic reason. They are excluded from the mutation-score denominator
so that scores reflect only mutants that actually exercise the test suite.

### Current heuristics

The initial heuristic covers common arithmetic identities:

| Original expression | Mutated expression | Reason |
|---------------------|--------------------|--------|
| `x + 0` or `0 + x`  | `x - 0`            | Adding or subtracting zero |
| `x - 0`             | `x + 0`            | Subtracting or adding zero |
| `x * 1` or `1 * x`  | `x / 1` or `x // 1`| Multiplying or dividing by one |
| `x / 1` or `x // 1` | `x * 1`            | Dividing or multiplying by one |

Heuristics require the identity operand to appear literally (e.g., `0` or `1`)
to keep false positives low. More patterns may be added as the project matures.

## Integration tests and benchmarks

Sample Lua projects live in `tests/fixtures/` and are exercised by the
integration test suite (`tests/integration_tests.rs`). The Criterion benchmark
suite in `benches/mutation_benchmark.rs` measures end-to-end mutation execution
time on these fixtures.

## Technology choices

See the [Architecture Decision Records](adrs/index.md) for the reasoning behind the
main technology and process choices.
