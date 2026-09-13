## MODIFIED Requirements

### Requirement: Provide run flags
The `run` subcommand SHALL accept a positional path argument and the optional flags `--test-command`, `--timeout`, `--output`, and `--changed-since <git-ref>`.

#### Scenario: Run with default options
- **WHEN** the user runs `lua-mutation-test run path/to/file.lua`
- **THEN** the program parses the path with the optional flags set to their defaults

#### Scenario: Run with all flags
- **WHEN** the user runs `lua-mutation-test run src --test-command 'busted' --timeout 30 --output json`
- **THEN** the program parses all provided flags and the path

#### Scenario: Run scoped to changed files
- **WHEN** the user runs `lua-mutation-test run --changed-since origin/main`
- **THEN** the program parses the git ref and scopes mutant generation to files differing from that ref

## ADDED Requirements

### Requirement: Changed-since flag validation
The system SHALL validate the `--changed-since` value and report actionable errors with exit code 2.

#### Scenario: Unknown ref
- **WHEN** the user runs `lua-mutation-test run --changed-since does-not-exist`
- **THEN** the program prints an error identifying the unresolvable ref and exits with code 2

#### Scenario: Changed-since outside git repository
- **WHEN** the user runs `lua-mutation-test run --changed-since origin/main` outside a git work tree
- **THEN** the program prints an error stating git is required for changed-files mode and exits with code 2

#### Scenario: Help documents changed-since
- **WHEN** the user runs `lua-mutation-test run --help`
- **THEN** the help text documents `--changed-since <git-ref>` with a PR example (`--changed-since origin/main`)
