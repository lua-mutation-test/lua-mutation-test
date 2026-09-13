## Context

`lmut run` currently scopes only by positional `path` plus `source_globs`/filters (`discover_source_files` in `src/main.rs`), then generates mutants for every discovered source file and executes them through `incremental::run_incremental`. The incremental cache (`src/incremental/cache.rs`, `change_detection.rs`, `mod.rs`) already reuses results for unchanged files and reports `cached`/`ran` counts in the summary line (`src/main.rs:229-239`), and `git_changed_files` detects working-tree and cached-HEAD diffs — but there is no way to express "files differing from a base ref" for PR fast-feedback vs. nightly full runs.

## Goals / Non-Goals

**Goals:**

- Add `lmut run --changed-since <git-ref>` that scopes mutant generation/execution to source files differing from the base ref, with normal summary, exit codes, and report formats.
- Compute the changed set inside lmut (merge-base diff + working-tree changes), intersected with discovered/filtered source files.
- Reuse the existing incremental cache so scoped re-runs skip already-cached in-scope mutants; document the PR (`--changed-since origin/main`) vs. nightly (full `run`) workflow.

**Non-Goals:**

- No `--changed-since` on `watch` (watch already reacts to working-tree changes incrementally).
- No test-to-source coverage mapping engine; "covering tests" means changed test files run plus the full discovered test set for the baseline (no selective test filtering beyond including changed tests).
- No automatic base-ref detection (e.g. reading `GITHUB_BASE_REF`); the user always passes the ref explicitly.
- No change to scoring semantics; scoped scores are diff-scoped and documented as such.

## Decisions

1. **New `changed_since_files(project_root, base_ref)` in `src/incremental/change_detection.rs`.** Resolves the ref with `git rev-parse --verify`, computes `merge-base HEAD <ref>`, then `git diff --name-only <merge-base> HEAD` plus `git status --porcelain` for uncommitted changes. Rationale: `merge-base` handles diverged PR branches correctly where a naive `diff <ref> HEAD` over-reports; reuses the existing git-invocation style. Alternative (caller passes `git diff` paths): rejected as unsound per the issue — only lmut knows globs, filters, and cache.

2. **Scope filtering in `run_pipeline` (`src/main.rs`), before mutant generation.** After `discover_source_files`, if `--changed-since` is present, filter `source_files` to those in the changed set (canonicalized/normalized for comparison). Mutant generation then only iterates in-scope files; `run_incremental` is called unchanged with the filtered list. Rationale: minimal cross-cutting change, cache/invalidation logic untouched, and acceptance ("generates/runs mutants only for that file's scope") falls out naturally. Alternative (generate all, skip execution out-of-scope): rejected — wastes generation time and complicates `cached`/`ran` semantics.

3. **Git work-tree root resolution.** When positional `path` is a file, run git commands in its parent directory; when a directory, in `path` itself; fail with exit code 2 if `rev-parse --is-inside-work-tree` is false or the ref is unresolvable, with the ref named in the message. Rationale: keeps single-file `lmut run file.lua --changed-since ...` working while errors stay actionable.

4. **Baseline and covering tests.** Keep the baseline over the full discovered test set (`discover_tests`) so mutant runs stay sound, and additionally guarantee changed `*_spec.lua`/`*_test.lua`/`test_*.lua` files are included even if positional `path` points at `src/` only (union changed test files into the test list). Rationale: avoids the failure mode where a PR touching only tests yields an empty scope while staying cheap; full selective test-mapping is out of scope.

5. **Empty scope behavior.** If intersection yields zero source files, print the normal summary with zero counts and exit 0 (no baseline failure, no report error). Rationale: docs-only PRs must pass fast rather than error.

6. **CLI surface.** `--changed-since <git-ref>` on `RunArgs` only, wired through `RunArgsLike` (returns `Option<String>`; `WatchArgs` returns `None`). Update `after_help` examples and `run --help` text. Rationale: smallest surface that satisfies the PR workflow; watch mode needs no equivalent.

## Risks / Trade-offs

- **[Risk] Renames/copies reported differently across git versions** → Mitigation: use plain `--name-only` (no rename detection flags), normalize with `project_root.join(line)` plus path canonicalization fallback, and cover with integration tests on rename fixtures.
- **[Risk] Scoped score misread as whole-project score** → Mitigation: `eprintln!` a scope banner (`scoped to N file(s) changed since <ref>`) and document that scoped scores are diff-scoped in `docs/architecture.md` and CLI reference.
- **[Risk] Large diffs (e.g. wrong base ref) degrade to near-full runs** → Mitigation: acceptable and correct by construction; the scope banner makes the size visible, and nightly remains the full-run path.
- **[Risk] Non-UTF8 / edge-case paths in git output** → Mitigation: use lossy conversion consistent with existing `git_changed_files`, sort/dedup, and skip entries that do not resolve under the project root.

## Migration Plan

Additive feature; no migration. Existing `lmut run` behavior unchanged when the flag is absent. Docs add the PR-vs-nightly pattern; CI examples use `lmut run --changed-since origin/main` with a prior `git fetch`.

## Open Questions

- Should a future `--changed-since` also accept a commit SHA range or `--merge-base` override? (Recommendation: no — any rev resolvable by `rev-parse` already works; revisit if CI needs it.)
- Should scoped runs optionally include cached out-of-scope results in reports for dashboard continuity? (Recommendation: no for this change; reports cover the scoped run only.)
