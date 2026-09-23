## MODIFIED Requirements

### Requirement: CLI summary report
The system SHALL emit a CLI summary containing the overall mutation score and counts for each result category.

#### Scenario: Run completes with mixed results
- **WHEN** the mutation run finishes
- **THEN** the CLI summary SHALL display the overall mutation score and the counts of killed, survived, error, and timeout mutants

#### Scenario: Summary goes to stderr when report goes to stdout
- **WHEN** the user passes `--report-output -` alongside `--report-format`
- **THEN** the CLI summary SHALL be written to stderr instead of stdout so stdout carries only report content

### Requirement: Configurable report output path
The system SHALL allow the user to configure the directory or file path for generated reports, or `-` for stdout.

#### Scenario: Specify report output directory
- **WHEN** the user provides a report output path
- **THEN** the system SHALL write the requested reports to that path

#### Scenario: Write selected report format to stdout with dash
- **WHEN** the user passes `--report-format <FORMAT> --report-output -` to the `run` command
- **THEN** the system SHALL write the selected format (`summary`, `per-mutant`, `json`, `ctrf`, `html`, `stryker`) to stdout with the same bytes as the file output plus a trailing newline, and SHALL NOT create a file named `-`

#### Scenario: Stdout report is pipe-parseable
- **WHEN** the user runs `lmut run --report-format json --report-output -`
- **THEN** stdout SHALL contain only the JSON report content parseable as JSON (e.g. via `jq`), with diagnostics and the CLI summary on stderr

#### Scenario: Literal word stdout remains a file path
- **WHEN** the user passes `--report-output stdout`
- **THEN** the system SHALL write to a regular file named `stdout` and SHALL NOT treat it as the stdout stream

#### Scenario: Dash without format is inert
- **WHEN** the user passes `--report-output -` without `--report-format`
- **THEN** the system SHALL behave as today (no report generated, no stdout report content)
