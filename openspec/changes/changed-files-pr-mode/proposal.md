## Why

PR workflows need fast feedback on the diff, with a full run reserved for nightly — but `lmut run` can only scope by positional `path`, which cannot express "files differing from a base ref". Shelling out to `git diff` externally is unsound because only lmut knows its mutant inventory, test-file mapping, and incremental cache well enough to scope correctly.

## What Changes

- Add `lmut run --changed-since <git-ref>` (e.g. `--changed-since origin/main`) that limits mutant generation to source files differing from the base ref.
- Resolve the changed set inside lmut via git (`merge-base` + `diff --name-only`, plus uncommitted working-tree changes), intersected with discovered source files — never by trusting caller-supplied paths alone.
- Include covering tests for the changed scope so the baseline and mutant runs exercise the touched code.
- Combine with the existing incremental result cache so mutants outside the changed scope are reused from cache, not re-run, while summary output keeps the normal `cached`/`ran` counts, exit codes, and report formats.
- Fail with a clear CLI error (exit code 2) when the ref is unknown/unresolvable or when run outside a git work tree.
- Document the PR-vs-nightly workflow (`lmut run --changed-since origin/main` on PRs, full `lmut run` on nightly) in `docs/`, `README.md`, and CLI reference.

## Capabilities

### New Capabilities

- `changed-files-scoping`: Scoping a `run` to files differing from a base git ref, including ref resolution, changed-file computation, scope intersection with source discovery, covering-test selection, and interaction with the incremental cache.

### Modified Capabilities

- `cli-entry-point`: Add the `--changed-since <git-ref>` flag to the `run` subcommand, its parsing/validation, help text, and error/exit-code behavior.
- `incremental-mutation-testing`: Changed-scope runs reuse cached results for out-of-scope mutants and only generate/run mutants in scope; summary `cached`/`ran` semantics and cache persistence cover the scoped mode.

## Impact

- `src/cli.rs`: new `--changed-since` option on `RunArgs`, help text, examples.
- `src/main.rs` (`run_pipeline`, `discover_source_files`, `RunArgsLike`): scope filtering, baseline test selection, reporting of scoped file counts.
- `src/incremental/change_detection.rs`: new ref-based changed-file resolution (`merge-base`, `diff --name-only`, working-tree status); unit tests for ref handling.
- `src/incremental/` cache/runner integration: reuse-not-rerun semantics for out-of-scope mutants.
- Tests: CLI parsing tests, change-detection unit tests, end-to-end scoped-run integration tests (single-file PR fixture).
- Docs: `README.md`, `docs/getting-started.md`, `docs/cli-reference.md`, `docs/architecture.md` — PR-vs-nightly workflow.
