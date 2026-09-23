# Getting Started

## Prerequisites

- A Lua interpreter in your `PATH` (needed to run your test suite against mutants).
- (Optional) [Rust](https://www.rust-lang.org/tools/install) toolchain if you want to build from source.

## Installation

### From GitHub Releases

Download the pre-built binary for your platform from the
[releases page](https://github.com/lua-mutation-test/lua-mutation-test/releases) and place it
on your `PATH`.

The archive contains both `lua-mutation-test` and the shorter `lmut` alias.

### From crates.io

```bash
cargo install lua-mutation-test
```

### From source

See [CONTRIBUTING.md](https://github.com/lua-mutation-test/lua-mutation-test/blob/main/CONTRIBUTING.md)
for the development setup.

## Quick start

Run mutation testing against a file or directory. If no path is given, the
current directory is used:

```bash
lmut run
lmut run <path-to-lua-file-or-directory>
```

Run with a custom test command and timeout:

```bash
lmut run src --test-command 'busted' --timeout 30
```

Create a sample configuration file:

```bash
lmut init
```

Run mutants in parallel and watch for changes:

```bash
lmut run src --workers 4
lmut watch src --test-command 'busted'
```

## Continuous integration

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

### PR vs. nightly runs

Mutate only the diff on pull requests for fast feedback, and run the full
suite on a nightly schedule:

```yaml
# PR workflow: fast, diff-scoped
- name: Fetch base branch
  run: git fetch origin main
- name: Run mutation testing on changed files
  uses: lua-mutation-test/lua-mutation-test-action@v0
  with:
    args: --changed-since origin/main
    fail-under: 80
```

```yaml
# Nightly workflow: full project run
- name: Run mutation testing
  uses: lua-mutation-test/lua-mutation-test-action@v0
  with:
    fail-under: 80
```

The same works locally: `lmut run --changed-since origin/main` on a branch,
plain `lmut run` for the whole project. Scoped scores cover the diff only —
treat the nightly full-run score as the source of truth.

## Configuration

Create a `.lua-mutation-test.toml` (or `.lua-mutation-test.json`) file in your project root:

```toml
version = "1"
test_command = "busted"
timeout = 30
test_globs = ["*_spec.lua", "*_test.lua", "test_*.lua"]
source_globs = ["*.lua"]

[files]
exclude = ["*_test.lua", "*_spec.lua"]

[operators]
exclude = ["control_flow"]
```

CLI flags override configuration file values.

## CLI Reference

See the [CLI Reference](cli-reference.md) for the complete list of subcommands,
options, and exit codes.

## Contributing

See [CONTRIBUTING.md](https://github.com/lua-mutation-test/lua-mutation-test/blob/main/CONTRIBUTING.md)
for development setup, workflow guidelines, and commit conventions.

## Versioning

This project follows [ZeroVer](https://0ver.org/): all releases remain in the `0.x`
range while the API and feature set stabilizes.
