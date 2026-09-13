#!/usr/bin/env python3
"""Convert cargo-mutants outcomes.json to a Stryker mutation-testing-report.json (schema v2).

Usage:
    cargo-mutants-to-stryker.py <mutants-out-dir> <project-root> <output-path>

cargo-mutants writes <mutants-out-dir>/outcomes.json with one entry per mutant:
scenario {"Mutant": {name, file, span{start,end}, replacement, genre}} and
summary "CaughtMutant" | "MissedMutant" | "Timeout*" | "Unviable" (plus a
"Baseline" entry with summary "Success", which is skipped).

Mapping to Stryker MutantStatus: CaughtMutant -> Killed,
MissedMutant -> Survived, Timeout* -> Timeout, Unviable -> CompileError.
"""

import json
import sys
from pathlib import Path


def stryker_status(summary: str) -> str:
    lowered = summary.lower()
    if "caught" in lowered or "killed" in lowered:
        return "Killed"
    if "miss" in lowered or "surviv" in lowered:
        return "Survived"
    if "timeout" in lowered:
        return "Timeout"
    if "unviable" in lowered or "compile" in lowered:
        return "CompileError"
    return "NoCoverage"


def main() -> None:
    mutants_out = Path(sys.argv[1])
    project_root = Path(sys.argv[2])
    output_path = Path(sys.argv[3])

    outcomes = json.loads((mutants_out / "outcomes.json").read_text())["outcomes"]
    cargo_mutants_version = None
    try:
        cargo_mutants_version = json.loads(
            (mutants_out / "outcomes.json").read_text()
        ).get("cargo_mutants_version")
    except (OSError, ValueError):
        pass

    sources: dict[str, str | None] = {}
    files: dict[str, dict] = {}

    for outcome in outcomes:
        scenario = outcome.get("scenario")
        if not isinstance(scenario, dict) or "Mutant" not in scenario:
            continue  # Baseline entry
        mutant = scenario["Mutant"]
        rel_path = mutant["file"]
        abs_path = project_root / rel_path
        if rel_path not in sources:
            try:
                sources[rel_path] = abs_path.read_text()
            except OSError:
                sources[rel_path] = None

        span = mutant["span"]
        duration_ms = None
        phases = outcome.get("phase_results") or []
        if phases:
            duration_ms = round(
                sum(p.get("duration", 0) for p in phases) * 1000
            )

        entry = files.setdefault(
            rel_path,
            {
                "language": "rust",
                "source": sources[rel_path] or "",
                "mutants": [],
            },
        )
        entry["mutants"].append(
            {
                "id": mutant["name"],
                "mutatorName": mutant.get("genre", "cargo-mutants"),
                "replacement": mutant.get("replacement"),
                "location": {
                    "start": {
                        "line": span["start"]["line"],
                        "column": span["start"]["column"],
                    },
                    "end": {
                        "line": span["end"]["line"],
                        "column": span["end"]["column"],
                    },
                },
                "status": stryker_status(outcome.get("summary", "")),
                "description": mutant["name"],
                "duration": duration_ms,
            }
        )

    # Drop null durations (Stryker treats missing as unknown).
    for file_entry in files.values():
        for m in file_entry["mutants"]:
            if m["duration"] is None:
                del m["duration"]

    report = {
        "schemaVersion": "2.0",
        "thresholds": {"high": 80, "low": 60},
        "files": files,
        "projectRoot": str(project_root),
        "framework": {
            "name": "cargo-mutants",
            "version": cargo_mutants_version,
        },
    }
    if report["framework"]["version"] is None:
        del report["framework"]["version"]

    output_path.write_text(json.dumps(report, indent=2) + "\n")
    counts: dict[str, int] = {}
    for file_entry in files.values():
        for m in file_entry["mutants"]:
            counts[m["status"]] = counts.get(m["status"], 0) + 1
    print(f"wrote {output_path}: {sum(counts.values())} mutants {counts}")


if __name__ == "__main__":
    main()
