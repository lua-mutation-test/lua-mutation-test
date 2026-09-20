# ADR-003: Vendored tree-sitter-lua Instead of Git Submodule

## Status

Superseded by [ADR-011](0011-crates-io-tree-sitter-lua.md)

## Context

The tree-sitter-lua grammar must be available at build time so the C parser can be
compiled. We initially considered tracking it as a git submodule.

## Decision Drivers

- Simple clone-and-build experience for contributors.
- Avoid submodule friction (forgetting `--recurse-submodules`, stale submodules).
- Keep the grammar version reproducible and explicit.
- Avoid committing generated parser source into the repository.

## Considered Options

### Option 1: Git submodule

- **Pros**: Exact version pinned, no extra download scripts.
- **Cons**: Contributors often forget to initialize submodules; harder to update;
  confusing for newcomers.

### Option 2: Cargo crate for tree-sitter-lua

- **Pros**: Native Rust dependency management.
- **Cons**: Not all tree-sitter grammars publish crates; less control over parser
  generation flags.

### Option 3: Vendored folder fetched by a script

- **Pros**: Simple explicit script, easy to update, no submodule friction, grammar
  source stays out of the main repository.
- **Cons**: Requires running a script before the first build; folder must be gitignored.

## Decision

We will store `tree-sitter-lua` in a gitignored `tree-sitter-lua/` directory and provide
`scripts/fetch-tree-sitter-lua.sh` to clone or update it.

## Rationale

A fetch script gives us explicit control over the grammar version while keeping the
repository free of vendored source code. It also removes the common pitfalls of git
submodules.

## Consequences

### Positive

- Contributors run one script instead of remembering submodule flags.
- Repository stays small; grammar source is not tracked.
- Easy to change grammar URL or pin a specific commit in the script.

### Negative

- Build process depends on the script being run first.
- CI must fetch the grammar before compiling.

## Implementation Notes

- `tree-sitter-lua/` is listed in `.gitignore`.
- `scripts/fetch-tree-sitter-lua.sh` clones or pulls the grammar.
- The project uses the `tree-sitter-lua` crate's Rust bindings (`tree_sitter_lua::LANGUAGE`)
  instead of a custom `build.rs`; the vendored crate compiles its own parser.

## References

- [tree-sitter-lua repository](https://github.com/tree-sitter-grammars/tree-sitter-lua)
