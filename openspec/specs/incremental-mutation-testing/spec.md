# incremental-mutation-testing Specification

## Purpose
TBD - created by archiving change incremental-mutation-testing-watch-mode. Update Purpose after archive.
## Requirements
### Requirement: Result cache
The system SHALL cache mutation results keyed by source file path and mutant hash.

#### Scenario: Cache hit
- **WHEN** a mutant with the same file path and hash was previously run successfully
- **THEN** the system reuses the cached result without executing the mutant again

#### Scenario: Cache miss
- **WHEN** no cached entry exists for a mutant
- **THEN** the system executes the mutant and stores the result

### Requirement: Change detection
The system SHALL detect changed source files since the last run using git or file content hashes.

#### Scenario: File modified
- **WHEN** a source file has changed since the previous run
- **THEN** the system re-runs mutants for that file

#### Scenario: File unchanged
- **WHEN** a source file has not changed since the previous run
- **THEN** the system skips mutants for that file and uses cached results

### Requirement: Cache invalidation on configuration change
The system SHALL invalidate cached results when the mutation testing configuration changes.

#### Scenario: Config updated
- **WHEN** the configuration file or CLI options change between runs
- **THEN** the system invalidates all cached results affected by the configuration change

### Requirement: Watch mode
The system SHALL provide a `watch` subcommand that re-runs mutation tests when source files change.

#### Scenario: File change triggers re-run
- **WHEN** the user runs the `watch` subcommand and a tracked source file is modified
- **THEN** the system re-runs only the affected mutants and reports updated results

#### Scenario: Watch mode debouncing
- **WHEN** multiple file changes occur in rapid succession
- **THEN** the system debounces the events and performs a single incremental re-run

### Requirement: Persistent incremental state
The system SHALL store incremental state in `.lua-mutation-test/cache/`.

#### Scenario: State persists across invocations
- **WHEN** a mutation test run completes
- **THEN** the system writes cache data to `.lua-mutation-test/cache/`

#### Scenario: State is read on next run
- **WHEN** the mutation test command starts
- **THEN** the system reads existing incremental state from `.lua-mutation-test/cache/`

### Requirement: Changed-scope cache reuse
The system SHALL combine `--changed-since` scoping with the existing incremental result cache so mutants outside the changed scope are reused from cache, not re-run.

#### Scenario: Out-of-scope mutant reused from cache
- **WHEN** a scoped run encounters a mutant outside the changed scope with a valid cached result
- **THEN** the system reuses the cached result without executing the mutant and counts it in the `cached` total of the summary line

#### Scenario: In-scope mutant executes normally
- **WHEN** a scoped run encounters a mutant inside the changed scope without a valid cache entry
- **THEN** the system executes the mutant, stores the result, and counts it in the `ran` total of the summary line

#### Scenario: Scoped run persists state
- **WHEN** a scoped run completes
- **THEN** the system writes incremental state to `.lua-mutation-test/cache/` so subsequent full or scoped runs can reuse its results

### Requirement: Scoped invalidation
The system SHALL invalidate cached results for changed files in a scoped run and keep reporting `cached`/`ran` counts with their existing meanings.

#### Scenario: Changed file reruns despite cache
- **WHEN** a source file differs from the base ref
- **THEN** the system treats its mutants as uncached for this run and re-executes them even if older cache entries exist

#### Scenario: Unchanged file uses cache
- **WHEN** a source file does not differ from the base ref and its configuration is unchanged
- **THEN** the system serves its mutants from cache without re-execution

