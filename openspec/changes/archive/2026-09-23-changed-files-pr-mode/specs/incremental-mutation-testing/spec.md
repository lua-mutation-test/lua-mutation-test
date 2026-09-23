## ADDED Requirements

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
