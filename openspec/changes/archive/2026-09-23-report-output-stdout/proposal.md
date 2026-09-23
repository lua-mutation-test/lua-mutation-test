## Why

Downstream tooling (e.g. a GitHub Action `parse.sh`, per issue #37) currently greps the human-readable `Mutation score:` summary line on stdout because there is no way to get machine-readable report content (`--report-format json|ctrf|stryker|...`) on stdout. `--report-output` only accepts a file path, forcing a write-then-cat roundtrip in CI.

## What Changes

- `--report-output -` (single dash, Unix stdout convention) writes the selected `--report-format` content to stdout instead of a file.
- `lmut run --report-format json --report-output -` emits parseable JSON on stdout (same bytes that would be written to a file, plus trailing newline).
- Applies to all `--report-format` values (`summary`, `per-mutant`, `json`, `ctrf`, `html`, `stryker`).
- No file named `-` is created; `./-` remains the way to address a literal file named `-`.
- Only `-` means stdout; the literal word `stdout` remains a regular file path (avoids collision with a file named `stdout` and follows `cat`/`tar`/`curl` convention).
- In `-` mode the report goes to stdout and the `Mutation score:` summary moves to stderr to keep stdout pipe-parseable; normal file mode is unchanged and progress/diagnostics stay on stderr.

## Capabilities

### New Capabilities

- (none)

### Modified Capabilities

- `report-generation`: Configurable report output path gains a stdout sink — `-` selects stdout instead of a file path.

## Impact

- `src/report.rs`: `generate_report()` branches on `-` (print to stdout vs `fs::write`).
- `src/main.rs`: `run_pipeline` report dispatch (no behavior change besides stdout sink).
- `src/cli.rs`: `--report-output` help text documents `-` as stdout.
- Docs: `docs/cli-reference.md` (and `README.md`/`docs/getting-started.md` examples if touched).
- Tests: unit tests for stdout vs file paths, CLI integration test for `--report-format json --report-output -` emitting parseable JSON.
