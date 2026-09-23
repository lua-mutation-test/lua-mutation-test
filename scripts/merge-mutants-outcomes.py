#!/usr/bin/env python3
"""Merge per-shard cargo-mutants outputs into a single mutants.out directory.

Usage:
    merge-mutants-outcomes.py <output-mutants-out-dir> <input-dir>...

Each input dir is either a cargo-mutants output dir or a downloaded shard
artifact dir (both layouts are accepted): it must contain outcomes.json plus
caught.txt / missed.txt / timeout.txt / unviable.txt, optionally nested under
a mutants.out/ subdirectory. Shards that produced no output (e.g. `--in-diff`
matched nothing) are skipped with a warning.

Only outcomes.json is consumed by cargo-mutants-to-stryker.py, but the .txt
files are merged too so the final artifact stays complete.
"""

import json
import sys
from pathlib import Path
from typing import List, Optional

TXT_FILES = ("caught.txt", "missed.txt", "timeout.txt", "unviable.txt")


def bucket(summary: str) -> str:
    lowered = summary.lower()
    if "caught" in lowered or "killed" in lowered:
        return "caught"
    if "miss" in lowered or "surviv" in lowered:
        return "missed"
    if "timeout" in lowered:
        return "timeout"
    if "unviable" in lowered or "compile" in lowered:
        return "unviable"
    if "success" in lowered:
        return "success"
    return "unviable"


def resolve_out_dir(d: Path) -> Optional[Path]:
    # Artifacts may extract flat (outcomes.json at root) or nested
    # (mutants.out/outcomes.json) depending on the upload path form.
    if (d / "outcomes.json").exists():
        return d
    if (d / "mutants.out" / "outcomes.json").exists():
        return d / "mutants.out"
    return None


def main() -> None:
    if len(sys.argv) < 3:
        print("Usage: merge-mutants-outcomes.py <output-mutants-out-dir> <input-dir>...")
        raise SystemExit(2)
    out_dir = Path(sys.argv[1])
    in_dirs = [Path(a) for a in sys.argv[2:]]

    outcomes: List = []
    version = None
    start_time = None
    end_time = None
    shards = 0
    resolved: List[Path] = []
    for d in in_dirs:
        r = resolve_out_dir(d)
        if r is None:
            print(f"warning: no outcomes.json under {d}, skipping")
            continue
        resolved.append(r)
        f = r / "outcomes.json"
        data = json.loads(f.read_text())
        outcomes.extend(data.get("outcomes", []))
        version = version or data.get("cargo_mutants_version")
        for key in ("start_time", "start"):
            if data.get(key) and (start_time is None or data[key] < start_time):
                start_time = data[key]
        for key in ("end_time", "end"):
            if data.get(key) and (end_time is None or data[key] > end_time):
                end_time = data[key]
        shards += 1

    counts = {"caught": 0, "missed": 0, "timeout": 0, "unviable": 0, "success": 0}
    for outcome in outcomes:
        scenario = outcome.get("scenario")
        if not isinstance(scenario, dict) or "Mutant" not in scenario:
            counts["success"] += 1  # Baseline entry
            continue
        counts[bucket(outcome.get("summary", ""))] += 1

    out_dir.mkdir(parents=True, exist_ok=True)
    merged = {
        "outcomes": outcomes,
        "total_mutants": sum(
            1
            for o in outcomes
            if isinstance(o.get("scenario"), dict) and "Mutant" in o["scenario"]
        ),
        "missed": counts["missed"],
        "caught": counts["caught"],
        "timeout": counts["timeout"],
        "unviable": counts["unviable"],
        "success": counts["success"],
    }
    if version is not None:
        merged["cargo_mutants_version"] = version
    if start_time is not None:
        merged["start_time"] = start_time
    if end_time is not None:
        merged["end_time"] = end_time
    (out_dir / "outcomes.json").write_text(json.dumps(merged, indent=2) + "\n")

    for name in TXT_FILES:
        with (out_dir / name).open("w") as out:
            for r in resolved:
                f = r / name
                if f.exists():
                    out.write(f.read_text())

    print(f"merged {len(outcomes)} outcomes from {shards} shards -> {out_dir}: {counts}")


if __name__ == "__main__":
    main()
