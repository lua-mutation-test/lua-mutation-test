# ADR-011: Depend on tree-sitter-lua from crates.io

## Status

Accepted

## Context

ADR-003 chose a gitignored, script-fetched `tree-sitter-lua/` directory over a
Cargo registry dependency, assuming grammar crates were not reliably published.
In practice `tree-sitter-lua` **is** published on crates.io (currently `0.5.0`,
matching the grammar revision we build against), and the vendored setup caused
real problems:

- `cargo install lua-mutation-test` and `cargo publish` resolve dependencies
  from the registry, so the fetched directory is invisible to them; CI built
  against grammar `main` while crate consumers would get registry `0.5.0`.
- Every build and CI job required running `scripts/fetch-tree-sitter-lua.sh`
  first, adding friction for contributors and agents.

## Decision Drivers

- Reproducible builds: CI, contributors, and crates.io consumers must compile
  against the same grammar revision.
- Reliable `cargo publish`: no path-only dependencies, no pre-build scripts.
- Simple contributor setup (`cargo build` just works).

## Considered Options

### Option 1: Keep the vendored fetch script (ADR-003 status quo)

- **Pros**: Full control over the exact grammar commit.
- **Cons**: Irreproducible vs registry consumers; extra setup step; CI must
  fetch before compiling; friction for `cargo install`/`cargo publish`.

### Option 2: Depend on the published tree-sitter-lua crate

- **Pros**: Native Cargo version resolution; reproducible builds everywhere;
  no fetch script; `cargo publish` and `cargo install` work out of the box.
- **Cons**: We can only use published revisions (no arbitrary commits); minor
  lag between grammar fixes and crate releases.

## Decision

We will depend on `tree-sitter-lua` from crates.io (`tree-sitter-lua = "0.5.0"`)
and remove the vendored `tree-sitter-lua/` directory and
`scripts/fetch-tree-sitter-lua.sh`. This supersedes ADR-003.

## Rationale

The assumption behind ADR-003 no longer holds: the grammar maintainers publish
the crate, and the published `0.5.0` exposes the same `tree_sitter_lua::LANGUAGE`
API we use. Registry resolution gives every consumer the identical revision,
which the fetch-`main` script could not guarantee.

## Consequences

### Positive

- `cargo build`, `cargo test`, `cargo install`, and `cargo publish` work with
  no extra steps.
- Grammar revision is pinned in `Cargo.toml`/`Cargo.lock` and updated with
  normal Dependabot/renovate flows.

### Negative

- Adopting an unpublished grammar fix requires waiting for (or requesting) a
  crates.io release, or temporarily patching via `[patch]`.

## References

- [ADR-003: Vendored tree-sitter-lua Instead of Git Submodule](0003-vendored-tree-sitter-lua.md)
- [tree-sitter-lua on crates.io](https://crates.io/crates/tree-sitter-lua)
- [tree-sitter-lua repository](https://github.com/tree-sitter-grammars/tree-sitter-lua)
