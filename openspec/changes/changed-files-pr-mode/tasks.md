## 1. Change detection (`--changed-since` core)

- [ ] 1.1 Add `changed_since_files(project_root, base_ref)` in `src/incremental/change_detection.rs` (verify ref via `rev-parse`, `merge-base HEAD <ref>`, `diff --name-only`, plus `status --porcelain` for uncommitted changes; sorted/deduped, error on unknown ref or non-git tree).
- [ ] 1.2 Add unit tests for ref resolution: valid ref returns changed files, unknown ref errors naming the ref, non-git directory errors, uncommitted changes included, unchanged files excluded.

## 2. CLI plumbing

- [ ] 2.1 Add `--changed-since <git-ref>` to `RunArgs` in `src/cli.rs`, extend `RunArgsLike` with a `changed_since()` accessor (`WatchArgs` returns `None`), and update `after_help` examples.
- [ ] 2.2 Add CLI parsing tests: flag parses the ref, defaults to `None`, `run --help` documents `--changed-since` with a PR example.

## 3. Pipeline scoping (`src/main.rs`)

- [ ] 3.1 Filter `source_files` to the changed set (work-tree root from `path` file/dir, intersect with discovery globs/filters) when the flag is present; emit a scope banner (`scoped to N file(s) changed since <ref>`).
- [ ] 3.2 Union changed test files into the baseline/mutant test list so covering tests run; keep full discovered tests for the baseline.
- [ ] 3.3 Handle empty scope: print the normal summary with zero counts and exit 0; propagate ref/git errors as CLI errors (exit code 2) with the ref named.
- [ ] 3.4 Add integration tests: single-file PR fixture generates/runs mutants only for that file's scope with normal codes/summary; empty-scope (docs-only) fixture exits 0; unknown ref exits 2.

## 4. Cache interaction and reporting parity

- [ ] 4.1 Verify scoped runs flow through `run_incremental` unchanged: in-scope mutants execute/cache normally, `cached`/`ran` keep existing meanings in the summary line.
- [ ] 4.2 Verify `--report-format`/`--report-output` work in scoped mode and cover only scoped results; add an integration test for a scoped JSON report.
- [ ] 4.3 Run `cargo test`, `cargo clippy`, and `cargo fmt --check`; resolve new warnings.

## 5. Documentation

- [ ] 5.1 Document `lmut run --changed-since origin/main` in `docs/cli-reference.md` (flag, errors, exit codes, empty-scope behavior).
- [ ] 5.2 Add the PR-vs-nightly workflow (`--changed-since` on PRs, full `run` on nightly, with `git fetch`) to `docs/getting-started.md` and `README.md`.
- [ ] 5.3 Document diff-scoped scoring and the cache-reuse interaction in `docs/architecture.md`.
