# Agent Guide for lua-mutation-test

This file contains the context and conventions that coding agents need when working
on `lua-mutation-test`.

## Project overview

`lua-mutation-test` is a mutation testing tool for Lua, written in Rust. It parses Lua
source code with tree-sitter, generates mutants, runs a Lua test suite against each
mutant, and reports mutation scores.

> ⚠️ This project is a **work in progress**. APIs, CLI flags, and behavior may change.

## Technology stack

- **Language**: Rust (edition 2021)
- **Parser**: [tree-sitter](https://tree-sitter.github.io/tree-sitter/) with the
  [tree-sitter-lua](https://github.com/tree-sitter-grammars/tree-sitter-lua) grammar
- **Build tool**: Cargo
- **Documentation**: [MkDocs](https://www.mkdocs.org/) with the
  [Material for MkDocs](https://squidfunk.github.io/mkdocs-material/) theme
- **Release automation**: [semantic-release](https://semantic-release.org/) configured
  for [ZeroVer](https://0ver.org/)

## Build and test

Build and test:

```bash
cargo build
cargo test
```

CI runs the same steps via `.github/workflows/rust.yml`.

## Development workflow

This project follows **trunk-based development**:

- Work on `main` or on a very short-lived branch rebased onto `main`.
- Keep commits small and focused.
- Apply **vertical slices**: implement features end-to-end through the minimum
  necessary layers rather than building horizontal layers in isolation.
  See ADR-007 for details.
- Use [Conventional Commits](https://www.conventionalcommits.org/):
  `feat:`, `fix:`, `refactor:`, `perf:`, `chore:`, `ci:`, `docs:`, `test:`.
- Pull/rebase `origin/main` before pushing.
- Ensure CI is green before and after your push.
- Do not push workflow files through HTTPS OAuth without `workflow` scope; use SSH
  (`git@github.com:lua-mutation-test/lua-mutation-test.git`).

## Documentation

Documentation lives in `docs/` and is published with MkDocs.

### When you change code, update the matching docs

| If you change... | Update these docs |
|---|---|
| Build process, dependency setup, or CLI usage | `README.md`, `docs/getting-started.md` |
| CLI commands, flags, or configuration options | `README.md`, `docs/getting-started.md`, `docs/cli-reference.md`, `docs/architecture.md` |
| Parser, AST traversal, or mutation generation | `docs/architecture.md` |
| Test runner integration or mutant execution | `docs/architecture.md` |
| Reporting, scoring, or output formats | `docs/architecture.md` |
| CI/CD workflows or release process | `docs/adrs/0006-semantic-release.md`, relevant workflow README comments |
| A major technology or process choice | Create or update an ADR (see below) |

### MkDocs site structure

The site is configured in `mkdocs.yml`:

- `docs/index.md` — project overview and WIP notice
- `docs/getting-started.md` — setup, build, and usage instructions
- `docs/architecture.md` — high-level architecture and component diagram
- `docs/cli-reference.md` — full CLI command and option reference
- `docs/adrs/` — Architecture Decision Records

When adding a new top-level page, update both `docs/` and the `nav:` section of
`mkdocs.yml`.

## Architecture Decision Records (ADRs)

ADRs are in `docs/adrs/` and follow the MADR format.

### When to write a new ADR

Write an ADR when making a significant technical decision, such as:

- Adopting a new framework, library, or tool.
- Changing the parsing strategy or mutation architecture.
- Changing the branching strategy or release process.
- Introducing a new integration pattern.

### When NOT to write an ADR

Skip an ADR for routine changes such as:

- Bug fixes.
- Minor dependency version upgrades.
- Refactoring that does not change architecture.
- Configuration tweaks.

### How to write an ADR

1. Copy `docs/adrs/template.md` to `docs/adrs/NNNN-short-title.md`.
2. Fill in the MADR sections: status, context, decision drivers, considered options,
   decision, rationale, consequences, and references.
3. Add the new ADR to `docs/adrs/index.md`.
4. Add the new ADR to the `nav:` section of `mkdocs.yml`.

## Release notes

- Releases are automated with semantic-release.
- The project uses ZeroVer: major version stays at `0`.
- Commit messages drive the next version and changelog.
- Breaking changes in 0.x result in a minor-version bump; features and fixes result in
  patch-version bumps.

## Common gotchas

- `tree-sitter-lua` comes from crates.io (see ADR-011); there is no vendored
  grammar directory and no fetch script.
- Releases are automated: semantic-release bumps `Cargo.toml`/`Cargo.lock` via
  `scripts/bump-cargo-version.sh` and publishes to crates.io in
  `.github/workflows/release.yml`. Never bump versions by hand; a
  `CARGO_REGISTRY_TOKEN` repository secret is required.
- The default branch is `main`.
- The binary name is `lua-mutation-test` (configured in `Cargo.toml` via `[[bin]]`).
