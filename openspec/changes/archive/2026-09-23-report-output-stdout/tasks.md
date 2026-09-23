## 1. Report stdout sink

- [x] 1.1 Branch `generate_report()` on output path `-` to print content to stdout (same bytes as file plus trailing newline) instead of `fs::write`, still returning the content string
- [x] 1.2 Add unit tests that `-` writes nothing to disk, `stdout` (literal word) still writes a regular file, and stdout bytes end with a newline

## 2. Pipeline stream handling

- [x] 2.1 Route the `Mutation score:` summary to stderr when `--report-output -` is used, keeping stdout for report content only; normal file mode keeps summary on stdout
- [x] 2.2 Preserve no-op behavior for `--report-output -` without `--report-format` (no report, no extra stdout)

## 3. CLI help and docs

- [x] 3.1 Update `--report-output` help text in `src/cli.rs` to document `-` as stdout, plus a clap parsing test
- [x] 3.2 Update `docs/cli-reference.md` with `-` semantics, stream split (report → stdout, summary/diagnostics → stderr), and a `... --report-format json --report-output - | jq` example

## 4. Validation

- [x] 4.1 Add CLI integration test asserting `--report-format json --report-output -` emits parseable JSON on stdout, summary on stderr, and creates no file named `-`
- [x] 4.2 Run `cargo test` and `cargo clippy`, resolving new warnings
