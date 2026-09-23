## Context

`lmut run` always writes the machine-readable report (`--report-format json|ctrf|stryker|html|summary|per-mutant`) to a file via `--report-output <PATH>`. There is no stdout sink, so CI pipelines (issue #37) grep the human-readable `Mutation score:` summary line instead of parsing structured output.

Current state (`src/report.rs:51-70`, `src/main.rs:314-324`, `src/cli.rs:79-81`):
- `generate_report(format, data, output: Option<&Path>)` builds `content: String`, then `fs::write(path, &content)` when `output` is `Some`.
- `run_pipeline` only calls `generate_report` when `--report-format` is `Some`; `--report-output` alone is inert.
- Progress/diagnostics use `eprintln!` (stderr); the `Mutation score: ...` summary uses `println!` (stdout). Integration tests assert this split (`tests/changed_since.rs:225-227`).

## Goals / Non-Goals

**Goals:**
- `--report-output -` writes the selected `--report-format` content to stdout (same bytes as the file path, plus trailing newline).
- `lmut run --report-format json --report-output - | jq .` works — stdout is pure report content.
- Uniform behavior across all six formats; no new flags; exit codes unchanged.

**Non-Goals:**
- No stability promise for the prose summary line in this change (tracked separately in #37 item 1).
- No `--quiet`/`--verbose` rework (both currently parsed but unused).
- No automatic upload, no multi-output (`--report-output` stays single-valued).
- No change to scoring, sharding, caching, or file-report schemas.

## Decisions

1. **`-` means stdout (Unix convention); `stdout` stays a regular filename.**
   - Rationale: `cat`/`tar`/`curl`/`kubectl` convention; CI authors already expect it. A literal `stdout` value would collide with a real file named `stdout`, while `-` only collides with a file named `-` (addressable as `./-`).
   - Alternative considered: accept both `-` and `stdout` — rejected to avoid the collision and keep one canonical spelling.

2. **Branch inside `generate_report()` on `path == "-"`.**
   - Rationale: single choke point; `run_pipeline` dispatch stays unchanged; unit-testable without spawning the binary (`generate_report(Json, data, Some("-"))` returns content and prints it).
   - Alternative considered: branch in `main.rs` — rejected (duplicates format dispatch).

3. **Stdout purity: in `-` mode the summary line moves to stderr.**
   - Rationale: today stdout carries `Mutation score: ...`. Emitting `summary + JSON` on the same stream would break `| jq` and defeat the goal ("stop grepping prose"). Progress already goes to stderr, so moving the summary there in `-` mode keeps stdout pipe-clean with minimal surprise.
   - Behavior: normal mode unchanged (summary → stdout); `-` mode: report → stdout, summary → stderr, diagnostics → stderr. Documented in `docs/cli-reference.md`.
   - Alternative considered: keep summary on stdout alongside report — rejected (unparseable stdout).

4. **Byte identity plus trailing newline.**
   - Stdout gets exactly the file bytes, with a single trailing `\n` appended if missing, so shell tools see a complete final line. Return value of `generate_report` is unchanged (the content string).

5. **`--report-output -` without `--report-format` is a no-op (existing behavior).**
   - Rationale: report generation only triggers on `--report-format`; changing that would alter CLI semantics beyond this change.

## Risks / Trade-offs

- **[Risk] Users expecting summary on stdout in `-` mode see it on stderr** → Mitigation: document the stream switch next to `--report-output -`; integration test asserts summary on stderr and pure JSON on stdout.
- **[Risk] Large HTML piped to stdout clutters logs** → Mitigation: opt-in only; docs recommend `json`/`ctrf`/`stryker` for piping.
- **[Risk] Shell captures mixing stdout/stderr (`2>&1 | jq`) break parsing** → Mitigation: docs show `lmut run ... --report-output - 2>/dev/null | jq` / separate redirection pattern.
- **[Trade-off] No `/dev/stdout` support on non-Unix** → Decided: string comparison on `-` is portable; no OS-specific path handling.
