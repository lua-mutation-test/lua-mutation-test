# Architecture Decision Records

This directory contains Architecture Decision Records (ADRs) for `lua-mutation-test`.

ADRs capture significant technical decisions, the context in which they were made,
and their consequences. They are not changed after acceptance; if a decision is
reversed or superseded, a new ADR is written.

## Index

| ADR | Title | Status | Date |
|-----|-------|--------|------|
| [ADR-001](0001-use-rust.md) | Use Rust as the implementation language | Accepted | 2026-09-07 |
| [ADR-002](0002-use-tree-sitter.md) | Use tree-sitter for Lua parsing | Accepted | 2026-09-07 |
| [ADR-003](0003-vendored-tree-sitter-lua.md) | Vendored tree-sitter-lua instead of git submodule | Superseded by ADR-011 | 2026-09-07 |
| [ADR-004](0004-zero-versioning.md) | Use ZeroVer (0-based versioning) | Accepted | 2026-09-07 |
| [ADR-005](0005-trunk-based-development.md) | Use trunk-based development | Accepted | 2026-09-07 |
| [ADR-006](0006-semantic-release.md) | Automate releases with semantic-release | Accepted | 2026-09-07 |
| [ADR-007](0007-vertical-slices.md) | Apply vertical slices for feature development | Accepted | 2026-09-07 |
| [ADR-008](0008-agent-driven-development.md) | Agent-driven development | Accepted | 2026-09-07 |
| [ADR-009](0009-elastic-license-2.0.md) | License under Elastic License 2.0 | Superseded | 2026-09-07 |
| [ADR-010](0010-apache-license-2.0.md) | License under Apache License 2.0 | Accepted | 2026-09-08 |
| [ADR-011](0011-crates-io-tree-sitter-lua.md) | Depend on tree-sitter-lua from crates.io | Accepted | 2026-09-20 |

## Creating a new ADR

1. Copy [template.md](template.md) to a new file named `NNNN-short-title.md`.
2. Fill in the sections using the MADR format.
3. Submit the ADR for review.
4. Update this index after acceptance.

## Status definitions

- **Proposed**: Under discussion.
- **Accepted**: Decision made and being implemented.
- **Deprecated**: No longer relevant.
- **Superseded**: Replaced by a newer ADR.
- **Rejected**: Considered but not adopted.
