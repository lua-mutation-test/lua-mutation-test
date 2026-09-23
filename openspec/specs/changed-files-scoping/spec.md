# changed-files-scoping Specification

## Purpose
Scope mutation testing runs to source files differing from a base git ref, enabling fast PR-scoped runs.
## Requirements
### Requirement: Changed-since ref scoping
The system SHALL support scoping a `run` to source files differing from a base git ref supplied via `--changed-since <git-ref>`, so that a PR touching one file generates and runs mutants only for that file's scope.

#### Scenario: PR touching one file scopes the run
- **WHEN** the user runs `lmut run --changed-since origin/main` on a PR that touches one source file
- **THEN** the system generates and runs mutants only for that file's scope and exits with the normal codes and summary line

#### Scenario: Unchanged files are not mutated
- **WHEN** a source file does not differ from the base ref and has no uncommitted changes
- **THEN** the system generates no new mutants for that file in the scoped run

#### Scenario: Covering tests are included
- **WHEN** a scoped run selects a changed source file
- **THEN** the system includes the covering tests for that file in the baseline and mutant runs so touched code is exercised

### Requirement: Ref resolution and changed-file computation
The system SHALL resolve the base ref and compute the changed set inside lmut using git, combining committed diff against the merge base with uncommitted working-tree changes.

#### Scenario: Base ref resolves via merge-base
- **WHEN** the user supplies a valid ref such as `origin/main`
- **THEN** the system resolves it and diffs `merge-base(HEAD, ref)..HEAD` plus uncommitted changes to build the changed set

#### Scenario: Unknown ref fails clearly
- **WHEN** the user supplies an unresolvable or unknown git ref
- **THEN** the system reports a CLI error identifying the bad ref and exits with code 2 without running mutants

#### Scenario: Non-git directory fails clearly
- **WHEN** the user passes `--changed-since` outside a git work tree
- **THEN** the system reports a CLI error stating git is required and exits with code 2

### Requirement: Scope intersection with discovery and filters
The system SHALL intersect the git changed set with discovered source files (positional `path`, `source_globs`, and configured file filters) so external path plumbing cannot widen or corrupt the scope.

#### Scenario: Changed file outside path scope is excluded
- **WHEN** a git-changed file falls outside the positional `path` or fails the configured source globs/filters
- **THEN** the system excludes it from the scoped run

#### Scenario: Empty scope after intersection
- **WHEN** no discovered source file intersects the changed set
- **THEN** the system reports zero mutants, prints the normal summary line with zero counts, and exits with code 0

### Requirement: Scoped summary and reporting parity
The system SHALL preserve normal run semantics in scoped mode: the `Mutation score ... (killed ..., survived ..., ... cached ..., ran ...)` summary, exit codes (0 success, 1 surviving mutants, 2 CLI error), and requested report formats.

#### Scenario: Summary shows scoped run counts
- **WHEN** a scoped run completes
- **THEN** the summary line reports killed/survived/timed-out/errored/equivalent counts for the scoped mutants plus `cached`/`ran` counts, and the exit code follows the standard rule

#### Scenario: Report formats work in scoped mode
- **WHEN** the user combines `--changed-since` with `--report-format`/`--report-output`
- **THEN** the system writes the report covering the scoped results
