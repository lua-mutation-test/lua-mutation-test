# lua-mutation-test

A mutation testing tool for Lua, written in Rust.

!!! warning "Work in Progress"
    This project is in early development and is **not yet ready for production use**.
    APIs, CLI flags, and behavior may change at any time until a stable release is published.

## What is mutation testing?

Mutation testing is a technique for evaluating the quality of a test suite. The tool
introduces small, syntactically valid changes (mutants) into the source code and then
runs the test suite. If a mutant causes tests to fail, it is "killed"; if tests still
pass, the mutant "survived" and highlights a potential gap in test coverage.

## What does this project do?

`lua-mutation-test` parses Lua source code with [tree-sitter](https://tree-sitter.github.io/tree-sitter/)
to:

- Generate mutants from Lua source code (operator swaps, literal flips, control-flow changes).
- Run a given Lua test suite against each mutant, in parallel with incremental caching.
- Compute and report mutation scores.
- Provide configurable mutation operators and ignore patterns.

## Where to go next

- [Getting Started](getting-started.md) — install, build, run the project, and use the GitHub Action in CI.
- [CLI Reference](cli-reference.md) — full list of subcommands, options, and exit codes.
- [Architecture](architecture.md) — high-level design and component overview.
- [Architecture Decision Records](adrs/index.md) — why we chose Rust, tree-sitter, ZeroVer, and more.
