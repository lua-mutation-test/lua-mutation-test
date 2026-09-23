//! Mutation testing report generation.

use crate::position::build_line_start_table;
use crate::result::MutantResult;
use crate::score::{score_results, MutationScore, ScoreBreakdown};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Supported report formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportFormat {
    /// Human-readable CLI summary.
    Summary,
    /// Human-readable per-mutant listing.
    PerMutant,
    /// Machine-readable JSON.
    Json,
    /// Common Test Report Format (CTRF) JSON.
    Ctrf,
    /// Self-contained HTML page.
    Html,
    /// Stryker mutation-testing-report.json (schema v2).
    Stryker,
}

impl std::str::FromStr for ReportFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "summary" => Ok(ReportFormat::Summary),
            "per-mutant" => Ok(ReportFormat::PerMutant),
            "json" => Ok(ReportFormat::Json),
            "ctrf" => Ok(ReportFormat::Ctrf),
            "html" => Ok(ReportFormat::Html),
            "stryker" => Ok(ReportFormat::Stryker),
            _ => Err(format!("unknown report format: {s}")),
        }
    }
}

/// Input data for a report.
#[derive(Debug, Clone)]
pub struct ReportData<'a> {
    pub results: &'a [MutantResult],
    pub project_root: &'a Path,
    pub source_paths: &'a [PathBuf],
}

/// Generates a report of the requested format.
pub fn generate_report(
    format: ReportFormat,
    data: ReportData<'_>,
    output: Option<&Path>,
) -> Result<String, String> {
    let content = match format {
        ReportFormat::Summary => summary_report(data),
        ReportFormat::PerMutant => per_mutant_report(data),
        ReportFormat::Json => json_report(data),
        ReportFormat::Ctrf => ctrf_report(data),
        ReportFormat::Html => html_report(data),
        ReportFormat::Stryker => stryker_report(data),
    };

    if let Some(path) = output {
        std::fs::write(path, &content).map_err(|e| e.to_string())?;
    }

    Ok(content)
}

fn summary_report(data: ReportData<'_>) -> String {
    let score = score_results(data.results);
    let mut lines = Vec::new();
    lines.push("Mutation Test Results".to_string());
    lines.push("=====================".to_string());
    lines.push(format!("Total mutants: {}", score.overall.total()));
    lines.push(format!("Killed: {}", score.overall.killed));
    lines.push(format!("Survived: {}", score.overall.survived));
    lines.push(format!("Timed out: {}", score.overall.timed_out));
    lines.push(format!("Errors: {}", score.overall.error));
    lines.push(format!("Equivalent: {}", score.overall.equivalent));
    if let Some(pct) = score.overall.percentage() {
        lines.push(format!("Mutation score: {:.2}%", pct));
    } else {
        lines.push("Mutation score: N/A".to_string());
    }
    lines.join("\n")
}

fn per_mutant_report(data: ReportData<'_>) -> String {
    let mut lines = Vec::new();
    for result in data.results {
        let mutant = result.mutant();
        let reason = match result {
            crate::result::MutantResult::Equivalent { reason, .. } => {
                format!(" ({reason})")
            }
            _ => String::new(),
        };
        lines.push(format!(
            "[{}] {} {}:{} -> {}{}",
            result.category(),
            mutant.id,
            mutant.file.display(),
            mutant.line,
            mutant.replacement,
            reason
        ));
        lines.push(format!("  - {}", mutant.original));
    }
    lines.join("\n")
}

#[derive(Serialize, Deserialize)]
struct JsonReport {
    metadata: ReportMetadata,
    overall: MutationScore,
    breakdown: ScoreBreakdown,
    results: Vec<JsonResult>,
}

#[derive(Serialize, Deserialize)]
struct ReportMetadata {
    generated_at: String,
    tool_version: String,
    project_root: String,
    source_paths: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct JsonResult {
    id: String,
    file: String,
    operator: String,
    line: usize,
    column: usize,
    original: String,
    replacement: String,
    category: String,
    reason: Option<String>,
}

fn json_report(data: ReportData<'_>) -> String {
    let breakdown = score_results(data.results);
    let results: Vec<JsonResult> = data
        .results
        .iter()
        .map(|r| {
            let m = r.mutant();
            let reason = match r {
                crate::result::MutantResult::Equivalent { reason, .. } => Some(reason.clone()),
                _ => m.equivalent_reason.clone(),
            };
            JsonResult {
                id: m.id.clone(),
                file: m.file.to_string_lossy().to_string(),
                operator: m.operator.clone(),
                line: m.line,
                column: m.column,
                original: m.original.clone(),
                replacement: m.replacement.clone(),
                category: r.category().to_string(),
                reason,
            }
        })
        .collect();

    let report = JsonReport {
        metadata: ReportMetadata {
            generated_at: chrono_now(),
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            project_root: data.project_root.to_string_lossy().to_string(),
            source_paths: data
                .source_paths
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
        },
        overall: breakdown.overall.clone(),
        breakdown,
        results,
    };

    serde_json::to_string_pretty(&report).unwrap_or_default()
}

fn ctrf_report(data: ReportData<'_>) -> String {
    let score = score_results(data.results);
    let start = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut other = 0;

    let tests: Vec<CtrfTest> = data
        .results
        .iter()
        .map(|r| {
            let (status, ctrf_status) = match r {
                MutantResult::Killed { .. } => {
                    passed += 1;
                    ("killed", "passed")
                }
                MutantResult::Survived { .. } => {
                    failed += 1;
                    ("survived", "failed")
                }
                MutantResult::TimedOut { .. } => {
                    other += 1;
                    ("timed_out", "other")
                }
                MutantResult::Error { .. } => {
                    other += 1;
                    ("error", "other")
                }
                MutantResult::Equivalent { .. } => {
                    skipped += 1;
                    ("equivalent", "skipped")
                }
            };
            let m = r.mutant();
            let duration = match r {
                MutantResult::Killed { duration_ms, .. } => *duration_ms,
                MutantResult::Survived { duration_ms, .. } => *duration_ms,
                MutantResult::TimedOut { duration_ms, .. } => *duration_ms,
                MutantResult::Error { duration_ms, .. } => *duration_ms,
                MutantResult::Equivalent { .. } => 0,
            };
            CtrfTest {
                name: format!(
                    "{} {}:{} -> {}",
                    m.operator,
                    m.file.display(),
                    m.line,
                    m.replacement
                ),
                status: ctrf_status.to_string(),
                duration,
                file_path: Some(m.file.to_string_lossy().to_string()),
                line: Some(m.line),
                suite: Some(vec![m.operator.clone()]),
                extra: Some(serde_json::json!({
                    "lua-mutation-test.mutant_id": m.id,
                    "lua-mutation-test.category": status,
                    "lua-mutation-test.operator": m.operator,
                    "lua-mutation-test.original": m.original,
                    "lua-mutation-test.replacement": m.replacement,
                    "lua-mutation-test.line": m.line,
                    "lua-mutation-test.column": m.column,
                })),
            }
        })
        .collect();

    let report = CtrfReport {
        report_format: "CTRF".to_string(),
        spec_version: "0.0.0".to_string(),
        timestamp: Some(chrono_now()),
        generated_by: Some("lua-mutation-test".to_string()),
        results: CtrfResults {
            tool: CtrfTool {
                name: "lua-mutation-test".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
                extra: None,
            },
            summary: CtrfSummary {
                tests: score.overall.total(),
                passed,
                failed,
                pending: 0,
                skipped,
                other,
                start,
                stop: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
                extra: None,
            },
            tests,
            environment: None,
            extra: None,
        },
        extra: None,
    };

    serde_json::to_string_pretty(&report).unwrap_or_default()
}

#[derive(Serialize, Deserialize)]
struct CtrfReport {
    #[serde(rename = "reportFormat")]
    report_format: String,
    #[serde(rename = "specVersion")]
    spec_version: String,
    timestamp: Option<String>,
    #[serde(rename = "generatedBy")]
    generated_by: Option<String>,
    results: CtrfResults,
    extra: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
struct CtrfResults {
    tool: CtrfTool,
    summary: CtrfSummary,
    tests: Vec<CtrfTest>,
    environment: Option<serde_json::Value>,
    extra: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
struct CtrfTool {
    name: String,
    version: Option<String>,
    extra: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
struct CtrfSummary {
    tests: usize,
    passed: usize,
    failed: usize,
    pending: usize,
    skipped: usize,
    other: usize,
    start: u64,
    stop: u64,
    extra: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
struct CtrfTest {
    name: String,
    status: String,
    duration: u64,
    #[serde(rename = "filePath")]
    file_path: Option<String>,
    line: Option<usize>,
    suite: Option<Vec<String>>,
    extra: Option<serde_json::Value>,
}

fn chrono_now() -> String {
    // Fallback that avoids adding chrono dependency for a single timestamp.
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("{}.{:03}Z", d.as_secs(), d.subsec_millis()))
        .unwrap_or_else(|_| "0".to_string())
}

/// Stryker mutation-testing-report.json (schema v2).
/// See https://github.com/stryker-mutator/mutation-testing-elements/tree/master/packages/report-schema
#[derive(Serialize, Deserialize)]
struct StrykerReport {
    #[serde(rename = "schemaVersion")]
    schema_version: String,
    thresholds: StrykerThresholds,
    files: std::collections::BTreeMap<String, StrykerFile>,
    #[serde(rename = "projectRoot", skip_serializing_if = "Option::is_none")]
    project_root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    framework: Option<StrykerFramework>,
}

#[derive(Serialize, Deserialize)]
struct StrykerThresholds {
    high: u8,
    low: u8,
}

#[derive(Serialize, Deserialize)]
struct StrykerFramework {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct StrykerFile {
    language: String,
    source: String,
    mutants: Vec<StrykerMutant>,
}

#[derive(Serialize, Deserialize)]
struct StrykerMutant {
    id: String,
    #[serde(rename = "mutatorName")]
    mutator_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    replacement: Option<String>,
    location: StrykerLocation,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(rename = "statusReason", skip_serializing_if = "Option::is_none")]
    status_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration: Option<u64>,
}

#[derive(Serialize, Deserialize)]
struct StrykerLocation {
    start: StrykerPosition,
    end: StrykerPosition,
}

#[derive(Serialize, Deserialize)]
struct StrykerPosition {
    line: usize,
    column: usize,
}

fn stryker_status(result: &MutantResult) -> (&'static str, Option<String>) {
    match result {
        MutantResult::Killed { .. } => ("Killed", None),
        MutantResult::Survived { .. } => ("Survived", None),
        MutantResult::TimedOut { .. } => ("Timeout", None),
        MutantResult::Error { reason, .. } => ("RuntimeError", Some(reason.clone())),
        MutantResult::Equivalent { reason, .. } => ("Ignored", Some(reason.clone())),
    }
}

fn stryker_location(source: Option<&str>, mutant: &crate::mutant::Mutant) -> StrykerLocation {
    if let Some(text) = source {
        let start = crate::position::byte_offset_to_position(text, mutant.start_byte);
        let end = crate::position::byte_offset_to_position(text, mutant.end_byte);
        if let (Some(s), Some(e)) = (start, end) {
            return StrykerLocation {
                start: StrykerPosition {
                    line: s.line,
                    column: s.column,
                },
                end: StrykerPosition {
                    line: e.line,
                    column: e.column,
                },
            };
        }
    }
    // Fallback to stored 1-indexed line/column when source is unreadable.
    StrykerLocation {
        start: StrykerPosition {
            line: mutant.line,
            column: mutant.column,
        },
        end: StrykerPosition {
            line: mutant.line,
            column: mutant.column + 1,
        },
    }
}

fn relative_key(project_root: &Path, file: &Path) -> String {
    file.strip_prefix(project_root)
        .unwrap_or(file)
        .to_string_lossy()
        .to_string()
}

fn stryker_report(data: ReportData<'_>) -> String {
    use std::collections::{BTreeMap, HashMap};

    // Cache source text per absolute file path for location conversion + `source` field.
    let mut sources: HashMap<PathBuf, Option<String>> = HashMap::new();
    let mut mutants_by_file: BTreeMap<String, Vec<StrykerMutant>> = BTreeMap::new();

    // Group results by relative file key, preserving all mutated files in the report.
    for result in data.results {
        let m = result.mutant();
        // Resolve absolute path: mutant.file may already be absolute or relative to project_root.
        let abs = if m.file.is_absolute() {
            m.file.clone()
        } else {
            data.project_root.join(&m.file)
        };
        let entry = sources
            .entry(abs.clone())
            .or_insert_with(|| std::fs::read_to_string(&abs).ok());
        let source_opt = entry.as_deref();

        let (status, reason) = stryker_status(result);
        let duration = match result {
            MutantResult::Killed { duration_ms, .. }
            | MutantResult::Survived { duration_ms, .. }
            | MutantResult::TimedOut { duration_ms, .. }
            | MutantResult::Error { duration_ms, .. } => Some(*duration_ms),
            MutantResult::Equivalent { .. } => None,
        };
        let description = if m.original.is_empty() && m.replacement.is_empty() {
            None
        } else {
            Some(format!("{} -> {}", m.original, m.replacement))
        };

        let key = relative_key(data.project_root, &abs);
        mutants_by_file.entry(key).or_default().push(StrykerMutant {
            id: m.id.clone(),
            mutator_name: m.operator.clone(),
            replacement: Some(m.replacement.clone()),
            location: stryker_location(source_opt, m),
            status: status.to_string(),
            description,
            status_reason: reason,
            duration,
        });
    }

    // Ensure every source path appears even with zero mutants.
    for path in data.source_paths {
        let abs = if path.is_absolute() {
            path.clone()
        } else {
            data.project_root.join(path)
        };
        let key = relative_key(data.project_root, &abs);
        mutants_by_file.entry(key).or_default();
        sources
            .entry(abs.clone())
            .or_insert_with(|| std::fs::read_to_string(&abs).ok());
    }

    let mut files: BTreeMap<String, StrykerFile> = BTreeMap::new();
    for (key, mutants) in mutants_by_file {
        let abs = data.project_root.join(&key);
        let source = sources
            .get(&abs)
            .and_then(|s| s.clone())
            .or_else(|| {
                // Fall back to lookup by already-cached absolute path variants.
                sources
                    .iter()
                    .find(|(p, _)| relative_key(data.project_root, p) == key)
                    .and_then(|(_, s)| s.clone())
            })
            .unwrap_or_default();
        files.insert(
            key,
            StrykerFile {
                language: "lua".to_string(),
                source,
                mutants,
            },
        );
    }

    let report = StrykerReport {
        schema_version: "2.0".to_string(),
        thresholds: StrykerThresholds { high: 80, low: 60 },
        files,
        project_root: Some(data.project_root.to_string_lossy().to_string()),
        framework: Some(StrykerFramework {
            name: "lua-mutation-test".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
    };

    serde_json::to_string_pretty(&report).unwrap_or_default()
}

fn html_report(data: ReportData<'_>) -> String {
    let breakdown = score_results(data.results);

    let mut results_by_file: std::collections::HashMap<PathBuf, Vec<&MutantResult>> =
        std::collections::HashMap::new();
    for result in data.results {
        results_by_file
            .entry(result.mutant().file.clone())
            .or_default()
            .push(result);
    }

    // Sort source paths by file score ascending (worst first), N/A last.
    let mut sorted_paths: Vec<&PathBuf> = data.source_paths.iter().collect();
    sorted_paths.sort_by(|a, b| {
        let score_a = breakdown.by_file.get(*a).and_then(|s| s.score());
        let score_b = breakdown.by_file.get(*b).and_then(|s| s.score());
        match (score_a, score_b) {
            (Some(sa), Some(sb)) => sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });

    let mut index_items = String::new();
    let mut file_sections = String::new();
    let file_count = sorted_paths.len();
    for source_path in &sorted_paths {
        let source_path = *source_path;
        let results = results_by_file
            .get(source_path)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        let file_score = breakdown
            .by_file
            .get(source_path)
            .cloned()
            .unwrap_or_default();
        let anchor = file_anchor_id(source_path);
        let display_path = source_path
            .strip_prefix(data.project_root)
            .unwrap_or(source_path);

        index_items.push_str(&render_index_item(display_path, &file_score, &anchor));
        file_sections.push_str(&render_file_html(
            source_path,
            data.project_root,
            results,
            &file_score,
            &anchor,
        ));
    }

    let score_text = breakdown
        .overall
        .percentage()
        .map(|p| format!("{:.2}%", p))
        .unwrap_or_else(|| "N/A".to_string());

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Mutation Test Report</title>
<style>
  :root {{
    --killed: #c8e6c9;
    --killed-dark: #2e7d32;
    --survived: #ffcdd2;
    --survived-dark: #c62828;
    --timed-out: #ffe0b2;
    --timed-out-dark: #ef6c00;
    --error: #e0e0e0;
    --error-dark: #424242;
    --equivalent: #e1bee7;
    --equivalent-dark: #6a1b9a;
  }}
  body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; margin: 0; padding: 2rem; background: #fafafa; color: #333; }}
  .container {{ max-width: 1400px; margin: 0 auto; }}
  h1 {{ margin-bottom: 0.25rem; }}
  .summary {{ font-size: 1.25rem; margin-bottom: 1.5rem; color: #555; }}
  .legend {{ display: flex; gap: 1rem; flex-wrap: wrap; margin-bottom: 2rem; }}
  .legend-item {{ display: flex; align-items: center; gap: 0.4rem; font-size: 0.9rem; }}
  .legend-swatch {{ width: 1rem; height: 1rem; border-radius: 3px; border: 1px solid rgba(0,0,0,0.1); }}
  .layout {{ display: flex; gap: 2rem; align-items: flex-start; }}
  .sidebar {{ width: 320px; flex-shrink: 0; position: sticky; top: 1rem; align-self: flex-start; max-height: calc(100vh - 2rem); overflow-y: auto; background: #fff; border: 1px solid #ddd; border-radius: 6px; padding: 1rem; box-shadow: 0 1px 3px rgba(0,0,0,0.05); }}
  .sidebar h2 {{ margin-top: 0; font-size: 1rem; color: #555; }}
  .index-list {{ display: flex; flex-direction: column; gap: 0.5rem; }}
  .index-item {{ text-decoration: none; color: inherit; padding: 0.5rem; border-radius: 4px; border: 1px solid transparent; }}
  .index-item:hover {{ background: #f5f5f5; border-color: #ddd; }}
  .index-name {{ font-family: monospace; font-size: 0.85rem; word-break: break-all; color: #1565c0; }}
  .index-meta {{ display: flex; justify-content: space-between; align-items: baseline; margin-top: 0.25rem; font-size: 0.8rem; color: #666; }}
  .index-score {{ font-weight: 600; color: #333; }}
  .index-counts {{ color: #888; }}
  .index-bar {{ display: flex; height: 6px; border-radius: 3px; overflow: hidden; margin-top: 0.35rem; background: #eee; }}
  .index-bar-killed {{ background: var(--killed-dark); }}
  .index-bar-survived {{ background: var(--survived-dark); }}
  .index-bar-skipped {{ background: #9e9e9e; }}
  .main {{ flex: 1; min-width: 0; }}
  .file {{ background: #fff; border: 1px solid #ddd; border-radius: 6px; margin-bottom: 2rem; overflow: hidden; box-shadow: 0 1px 3px rgba(0,0,0,0.05); }}
  .file-header {{ background: #f5f5f5; padding: 0.75rem 1rem; font-weight: 600; border-bottom: 1px solid #ddd; display: flex; justify-content: space-between; align-items: center; }}
  .file-path {{ font-family: monospace; font-size: 0.95rem; }}
  .file-score {{ background: var(--killed-dark); color: #fff; padding: 0.2rem 0.6rem; border-radius: 12px; font-size: 0.85rem; font-weight: 600; }}
  .file-score.no-mutants {{ background: #9e9e9e; }}
  .file-score.low {{ background: var(--survived-dark); }}
  .file-score.medium {{ background: #f9a825; color: #000; }}
  .file-stats {{ display: flex; gap: 1rem; flex-wrap: wrap; padding: 0.5rem 1rem; background: #fafafa; border-bottom: 1px solid #eee; font-size: 0.85rem; color: #555; }}
  .stat strong {{ color: #333; }}
  .code {{ display: table; width: 100%; border-collapse: collapse; font-family: "SF Mono", Monaco, "Courier New", monospace; font-size: 0.85rem; line-height: 1.5; }}
  .line {{ display: table-row; }}
  .line-number {{ display: table-cell; padding: 0.15rem 0.75rem; color: #888; background: #fafafa; text-align: right; user-select: none; white-space: nowrap; border-right: 1px solid #eee; width: 1%; }}
  .line-content {{ display: table-cell; padding: 0.15rem 1rem; white-space: pre; word-wrap: normal; overflow-x: auto; }}
  .line-content:empty::before {{ content: " "; }}
  .mutant {{ cursor: help; border-radius: 3px; padding: 0 1px; }}
  .mutant.killed {{ background: var(--killed); border-bottom: 2px solid var(--killed-dark); }}
  .mutant.survived {{ background: var(--survived); border-bottom: 2px solid var(--survived-dark); }}
  .mutant.timed_out {{ background: var(--timed-out); border-bottom: 2px solid var(--timed-out-dark); }}
  .mutant.error {{ background: var(--error); border-bottom: 2px solid var(--error-dark); }}
  .mutant.equivalent {{ background: var(--equivalent); border-bottom: 2px solid var(--equivalent-dark); }}
  .mutant-table {{ width: 100%; border-collapse: collapse; font-size: 0.85rem; }}
  .mutant-table th {{ background: #f5f5f5; padding: 0.5rem 0.75rem; text-align: left; border-top: 1px solid #ddd; border-bottom: 1px solid #ddd; }}
  .mutant-table td {{ padding: 0.5rem 0.75rem; border-bottom: 1px solid #eee; }}
  .mutant-table tr.killed {{ background: var(--killed); }}
  .mutant-table tr.survived {{ background: var(--survived); }}
  .mutant-table tr.timed_out {{ background: var(--timed-out); }}
  .mutant-table tr.error {{ background: var(--error); }}
  .mutant-table tr.equivalent {{ background: var(--equivalent); }}
  .status-badge {{ display: inline-block; padding: 0.1rem 0.4rem; border-radius: 3px; font-size: 0.75rem; font-weight: 600; text-transform: uppercase; }}
  .status-badge.killed {{ background: var(--killed-dark); color: #fff; }}
  .status-badge.survived {{ background: var(--survived-dark); color: #fff; }}
  .status-badge.timed_out {{ background: var(--timed-out-dark); color: #fff; }}
  .status-badge.error {{ background: var(--error-dark); color: #fff; }}
  .status-badge.equivalent {{ background: var(--equivalent-dark); color: #fff; }}
</style>
</head>
<body>
<div class="container">
  <h1>Mutation Test Report</h1>
  <div class="summary">Overall score: {score} &middot; Generated: {generated}</div>
  <div class="legend">
    <div class="legend-item"><div class="legend-swatch" style="background: var(--killed);"></div> Killed</div>
    <div class="legend-item"><div class="legend-swatch" style="background: var(--survived);"></div> Survived</div>
    <div class="legend-item"><div class="legend-swatch" style="background: var(--timed-out);"></div> Timed out</div>
    <div class="legend-item"><div class="legend-swatch" style="background: var(--error);"></div> Error</div>
    <div class="legend-item"><div class="legend-swatch" style="background: var(--equivalent);"></div> Equivalent</div>
  </div>
  <div class="layout">
    <div class="sidebar">
      <h2>Files ({file_count})</h2>
      <div class="index-list">
        {index}
      </div>
    </div>
    <div class="main">
      {files}
    </div>
  </div>
</div>
</body>
</html>"#,
        score = score_text,
        generated = chrono_now(),
        file_count = file_count,
        index = index_items,
        files = file_sections
    )
}

fn render_index_item(file_path: &Path, score: &MutationScore, anchor: &str) -> String {
    let total = score.total();
    let score_text = score
        .percentage()
        .map(|p| format!("{:.0}%", p))
        .unwrap_or_else(|| "N/A".to_string());

    let denom = score.denominator();
    let (killed_width, survived_width, skipped_width) = if denom == 0 {
        (0.0, 0.0, 0.0)
    } else {
        (
            score.killed as f64 / denom as f64 * 100.0,
            score.survived as f64 / denom as f64 * 100.0,
            score.skipped as f64 / denom as f64 * 100.0,
        )
    };

    format!(
        r##"<a class="index-item" href="#{0}">
  <div class="index-name">{1}</div>
  <div class="index-meta">
    <span class="index-score">{2}</span>
    <span class="index-counts">{3} killed / {4} survived / {5} total</span>
  </div>
  <div class="index-bar">
    <div class="index-bar-killed" style="width: {6:.0}%"></div>
    <div class="index-bar-survived" style="width: {7:.0}%"></div>
    <div class="index-bar-skipped" style="width: {8:.0}%"></div>
  </div>
</a>"##,
        html_escape(anchor),
        html_escape(&file_path.to_string_lossy()),
        score_text,
        score.killed,
        score.survived,
        total,
        killed_width,
        survived_width,
        skipped_width,
    )
}

fn file_anchor_id(path: &Path) -> String {
    let sanitized = path
        .to_string_lossy()
        .replace(|c: char| !c.is_alphanumeric(), "-");
    format!("file-{sanitized}")
}

fn file_score_badge_class(score: &MutationScore) -> &'static str {
    match score.percentage() {
        None => "no-mutants",
        Some(p) if p < 50.0 => "low",
        Some(p) if p < 80.0 => "medium",
        _ => "",
    }
}

fn render_file_html(
    file_path: &Path,
    project_root: &Path,
    results: &[&MutantResult],
    file_score: &MutationScore,
    anchor: &str,
) -> String {
    let full_path = project_root.join(file_path);
    let display_path = file_path.strip_prefix(project_root).unwrap_or(file_path);

    let code_html = match std::fs::read_to_string(&full_path) {
        Ok(source) => render_source_html(&source, results),
        Err(_) => r#"<div style="padding:1rem;color:#c62828;">Could not read source file.</div>"#
            .to_string(),
    };

    let score_badge = match file_score.percentage() {
        Some(pct) => format!(
            r#"<span class="file-score {class}">{pct:.2}%</span>"#,
            class = file_score_badge_class(file_score),
            pct = pct
        ),
        None => r#"<span class="file-score no-mutants">N/A</span>"#.to_string(),
    };

    let stats = format!(
        r#"<div class="file-stats">
  <span class="stat"><strong>{}</strong> killed</span>
  <span class="stat"><strong>{}</strong> survived</span>
  <span class="stat"><strong>{}</strong> timed out</span>
  <span class="stat"><strong>{}</strong> error</span>
  <span class="stat"><strong>{}</strong> equivalent</span>
  <span class="stat"><strong>{}</strong> total</span>
</div>"#,
        file_score.killed,
        file_score.survived,
        file_score.timed_out,
        file_score.error,
        file_score.equivalent,
        file_score.total()
    );

    let mut table_rows = String::new();
    for result in results {
        let m = result.mutant();
        let reason = match result {
            MutantResult::Equivalent { reason, .. } => {
                format!("<br><small>{}</small>", html_escape(reason))
            }
            _ => String::new(),
        };
        let status = result.category();
        table_rows.push_str(&format!(
            r#"<tr class="{class}"><td><span class="status-badge {class}">{status}</span></td><td>{line}</td><td>{col}</td><td>{operator}</td><td><code>{original}</code> &rarr; <code>{replacement}</code>{reason}</td></tr>"#,
            class = category_class(status),
            status = status,
            line = m.line,
            col = m.column,
            operator = html_escape(&m.operator),
            original = html_escape(&m.original),
            replacement = html_escape(&m.replacement),
            reason = reason
        ));
    }

    let mutant_table = if results.is_empty() {
        String::new()
    } else {
        format!(
            r#"<table class="mutant-table">
  <thead><tr><th>Status</th><th>Line</th><th>Col</th><th>Operator</th><th>Mutation</th></tr></thead>
  <tbody>{}</tbody>
</table>"#,
            table_rows
        )
    };

    format!(
        r#"<div class="file" id="{anchor}">
  <div class="file-header">
    <span class="file-path">{}</span>
    {score_badge}
  </div>
  {stats}
  <div class="code">{}</div>
  {}
</div>"#,
        html_escape(&display_path.to_string_lossy()),
        code_html,
        mutant_table
    )
}

fn render_source_html(source: &str, results: &[&MutantResult]) -> String {
    let line_starts = build_line_start_table(source);
    let mut lines_html = String::new();

    for (i, &line_start) in line_starts.iter().enumerate() {
        let raw_end = if i + 1 < line_starts.len() {
            line_starts[i + 1]
        } else {
            source.len()
        };
        let line_number = i + 1;
        let line_source = &source[line_start..raw_end];
        let line_end = if line_source.ends_with('\n') {
            raw_end - 1
        } else {
            raw_end
        };
        let line_content_source = &source[line_start..line_end];

        let overlapping: Vec<&MutantResult> = results
            .iter()
            .filter(|r| {
                let m = r.mutant();
                m.start_byte < raw_end && m.end_byte > line_start
            })
            .copied()
            .collect();

        let line_html = if overlapping.is_empty() {
            html_escape(line_content_source)
        } else {
            render_line_html(source, line_start, line_end, &overlapping)
        };

        lines_html.push_str(&format!(
            r#"<div class="line"><div class="line-number">{}</div><div class="line-content">{}</div></div>"#,
            line_number, line_html
        ));
    }

    lines_html
}

fn render_line_html(
    source: &str,
    line_start: usize,
    line_end: usize,
    results: &[&MutantResult],
) -> String {
    let mut sorted: Vec<&MutantResult> = results.to_vec();
    sorted.sort_by_key(|r| r.mutant().start_byte);

    let mut segments: Vec<(usize, usize, Option<&MutantResult>)> = Vec::new();
    let mut current = line_start;

    for result in sorted {
        let m = result.mutant();
        let seg_start = m.start_byte.max(line_start);
        let seg_end = m.end_byte.min(line_end);
        if seg_start >= seg_end {
            continue;
        }
        if current < seg_start {
            segments.push((current, seg_start, None));
        }
        segments.push((seg_start, seg_end, Some(result)));
        current = current.max(seg_end);
    }
    if current < line_end {
        segments.push((current, line_end, None));
    }

    let mut html = String::new();
    for (start, end, result) in segments {
        let text = html_escape(&source[start..end]);
        match result {
            Some(r) => {
                let m = r.mutant();
                let class = category_class(r.category());
                let title = format!(
                    "{}: {} -> {} ({} at {}:{}){}",
                    m.operator,
                    m.original,
                    m.replacement,
                    r.category(),
                    m.line,
                    m.column,
                    match r {
                        MutantResult::Equivalent { reason, .. } => {
                            format!(" - {}", reason)
                        }
                        _ => String::new(),
                    }
                );
                html.push_str(&format!(
                    r#"<span class="mutant {}" title="{}">{}</span>"#,
                    class,
                    html_escape(&title),
                    text
                ));
            }
            None => html.push_str(&text),
        }
    }
    html
}

fn category_class(category: &str) -> &'static str {
    match category {
        "killed" => "killed",
        "survived" => "survived",
        "timed_out" => "timed_out",
        "error" => "error",
        "equivalent" => "equivalent",
        _ => "",
    }
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutant::{CandidateMutant, Mutant};
    use crate::score::Category;
    use std::path::PathBuf;

    fn dummy_result(category: Category) -> MutantResult {
        let mutant = Mutant::from_candidate(
            CandidateMutant {
                start_byte: 0,
                end_byte: 0,
                replacement: String::new(),
            },
            "dummy",
            PathBuf::from("src/foo.lua"),
            "local x = 1",
        );
        match category {
            Category::Killed => MutantResult::Killed {
                mutant,
                duration_ms: 10,
                stdout_snippet: String::new(),
                stderr_snippet: String::new(),
            },
            Category::Survived => MutantResult::Survived {
                mutant,
                duration_ms: 20,
                stdout_snippet: String::new(),
                stderr_snippet: String::new(),
            },
            Category::TimedOut => MutantResult::TimedOut {
                mutant,
                duration_ms: 30,
                stdout_snippet: String::new(),
                stderr_snippet: String::new(),
            },
            Category::Error => MutantResult::Error {
                mutant,
                duration_ms: 40,
                reason: String::new(),
                stdout_snippet: String::new(),
                stderr_snippet: String::new(),
            },
            Category::Skipped => MutantResult::Survived {
                mutant,
                duration_ms: 50,
                stdout_snippet: String::new(),
                stderr_snippet: String::new(),
            },
            Category::Equivalent => MutantResult::Equivalent {
                mutant,
                reason: String::new(),
            },
        }
    }

    fn dummy_equivalent(reason: &str) -> MutantResult {
        let mutant = Mutant::from_candidate(
            CandidateMutant {
                start_byte: 0,
                end_byte: 0,
                replacement: String::new(),
            },
            "dummy",
            PathBuf::from("src/foo.lua"),
            "local x = 1",
        );
        MutantResult::Equivalent {
            mutant,
            reason: reason.to_string(),
        }
    }

    fn dummy_data() -> ReportData<'static> {
        static RESULTS: std::sync::OnceLock<Vec<MutantResult>> = std::sync::OnceLock::new();
        let results = RESULTS.get_or_init(|| {
            vec![
                dummy_result(Category::Killed),
                dummy_result(Category::Survived),
            ]
        });
        static PATHS: std::sync::OnceLock<Vec<PathBuf>> = std::sync::OnceLock::new();
        let paths = PATHS.get_or_init(|| vec![PathBuf::from("src/foo.lua")]);

        ReportData {
            results,
            project_root: Path::new("."),
            source_paths: paths,
        }
    }

    #[test]
    fn summary_report_includes_score() {
        let report = generate_report(ReportFormat::Summary, dummy_data(), None).unwrap();
        assert!(report.contains("Mutation score:"));
        assert!(report.contains("Killed:"));
    }

    #[test]
    fn json_report_contains_results() {
        let report = generate_report(ReportFormat::Json, dummy_data(), None).unwrap();
        assert!(report.contains("\"killed\":"));
        assert!(report.contains("\"results\":"));
    }

    #[test]
    fn html_report_contains_table() {
        let report = generate_report(ReportFormat::Html, dummy_data(), None).unwrap();
        assert!(report.contains("<table"));
        assert!(report.contains("</table>"));
    }
    #[test]
    fn html_report_shows_source_code_with_colored_mutants() {
        let tmp = std::env::temp_dir().join(format!("lmt-html-report-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let src_dir = tmp.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let file_path = src_dir.join("foo.lua");
        let source = "local x = 1 + 2\nreturn x\n";
        std::fs::write(&file_path, source).unwrap();

        let one_offset = source.find('1').unwrap();
        let x_offset = source.rfind('x').unwrap();
        let killed = Mutant::from_candidate(
            CandidateMutant {
                start_byte: one_offset,
                end_byte: one_offset + 1,
                replacement: "0".to_string(),
            },
            "literal",
            &file_path,
            source,
        );
        let survived = Mutant::from_candidate(
            CandidateMutant {
                start_byte: x_offset,
                end_byte: x_offset + 1,
                replacement: "nil".to_string(),
            },
            "literal",
            &file_path,
            source,
        );
        let results = vec![
            MutantResult::Killed {
                mutant: killed,
                duration_ms: 10,
                stdout_snippet: String::new(),
                stderr_snippet: String::new(),
            },
            MutantResult::Survived {
                mutant: survived,
                duration_ms: 10,
                stdout_snippet: String::new(),
                stderr_snippet: String::new(),
            },
        ];

        let data = ReportData {
            results: &results,
            project_root: &tmp,
            source_paths: std::slice::from_ref(&file_path),
        };

        let report = generate_report(ReportFormat::Html, data, None).unwrap();
        assert!(report.contains("local x = "));
        assert!(report.contains("return "));
        assert!(report.contains(r#"class="mutant killed""#));
        assert!(report.contains(r#"class="mutant survived""#));
        assert!(report.contains("literal: 1 -&gt; 0"));
        assert!(report.contains("literal: x -&gt; nil"));
        assert!(report.contains("<table"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn html_report_includes_index_sorted_by_score() {
        let tmp =
            std::env::temp_dir().join(format!("lmt-html-report-index-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let src_dir = tmp.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let good_path = src_dir.join("good.lua");
        let bad_path = src_dir.join("bad.lua");
        std::fs::write(&good_path, "local x = 1\n").unwrap();
        std::fs::write(&bad_path, "local y = 2\n").unwrap();

        let good_mutant = Mutant::from_candidate(
            CandidateMutant {
                start_byte: 0,
                end_byte: 0,
                replacement: String::new(),
            },
            "dummy",
            &good_path,
            "local x = 1\n",
        );
        let bad_mutant = Mutant::from_candidate(
            CandidateMutant {
                start_byte: 0,
                end_byte: 0,
                replacement: String::new(),
            },
            "dummy",
            &bad_path,
            "local y = 2\n",
        );

        // good.lua: 1 killed -> 100%
        // bad.lua: 1 survived -> 0%
        let results = vec![
            MutantResult::Killed {
                mutant: good_mutant,
                duration_ms: 10,
                stdout_snippet: String::new(),
                stderr_snippet: String::new(),
            },
            MutantResult::Survived {
                mutant: bad_mutant,
                duration_ms: 10,
                stdout_snippet: String::new(),
                stderr_snippet: String::new(),
            },
        ];

        let data = ReportData {
            results: &results,
            project_root: &tmp,
            source_paths: &[good_path.clone(), bad_path.clone()],
        };

        let report = generate_report(ReportFormat::Html, data, None).unwrap();

        // Index contains both files.
        assert!(report.contains(r#"class="index-name""#));
        assert!(report.contains("good.lua"));
        assert!(report.contains("bad.lua"));

        // Per-file stats are present.
        assert!(report.contains(r#"class="file-stats""#));
        assert!(report.contains("1 killed"));
        assert!(report.contains("1 survived"));

        // bad.lua (0%) should appear before good.lua (100%) in the index.
        let bad_pos = report.find("bad.lua").unwrap();
        let good_pos = report.find("good.lua").unwrap();
        assert!(
            bad_pos < good_pos,
            "expected bad.lua (lower score) to be listed before good.lua"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn ctrf_report_contains_required_fields() {
        let report = generate_report(ReportFormat::Ctrf, dummy_data(), None).unwrap();
        assert!(report.contains("\"reportFormat\": \"CTRF\""));
        assert!(report.contains("\"specVersion\": \"0.0.0\""));
        assert!(report.contains("\"summary\""));
        assert!(report.contains("\"tests\""));
        assert!(report.contains("\"passed\""));
        assert!(report.contains("\"failed\""));
        assert!(report.contains("\"skipped\""));
        assert!(report.contains("\"other\""));
    }

    #[test]
    fn writes_report_to_file() {
        let tmp = std::env::temp_dir().join(format!("lmt-report-test-{}", std::process::id()));
        let report = generate_report(ReportFormat::Summary, dummy_data(), Some(&tmp)).unwrap();
        let contents = std::fs::read_to_string(&tmp).unwrap();
        assert_eq!(contents, report);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn stryker_format_parses_from_str() {
        assert_eq!(
            "stryker".parse::<ReportFormat>().unwrap(),
            ReportFormat::Stryker
        );
        assert_eq!(
            "STRYKER".parse::<ReportFormat>().unwrap(),
            ReportFormat::Stryker
        );
    }

    #[test]
    fn stryker_report_conforms_to_schema_v2() {
        let tmp = std::env::temp_dir().join(format!("lmt-stryker-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("src")).unwrap();
        let file_path = tmp.join("src").join("foo.lua");
        let source = "local x = 1 + 2\nreturn x\n";
        std::fs::write(&file_path, source).unwrap();

        let one_offset = source.find('1').unwrap();
        let killed = Mutant::from_candidate(
            CandidateMutant {
                start_byte: one_offset,
                end_byte: one_offset + 1,
                replacement: "0".to_string(),
            },
            "literal",
            &file_path,
            source,
        );
        let results = vec![MutantResult::Killed {
            mutant: killed,
            duration_ms: 10,
            stdout_snippet: String::new(),
            stderr_snippet: String::new(),
        }];
        let data = ReportData {
            results: &results,
            project_root: &tmp,
            source_paths: std::slice::from_ref(&file_path),
        };

        let report = generate_report(ReportFormat::Stryker, data, None).unwrap();
        let v: serde_json::Value = serde_json::from_str(&report).unwrap();
        assert_eq!(v["schemaVersion"], "2.0");
        assert_eq!(v["thresholds"]["high"], 80);
        assert_eq!(v["thresholds"]["low"], 60);
        let files = v["files"].as_object().unwrap();
        assert_eq!(files.len(), 1);
        let (_, file) = files.iter().next().unwrap();
        assert_eq!(file["language"], "lua");
        assert_eq!(file["source"], source);
        let mutant = &file["mutants"][0];
        assert_eq!(mutant["mutatorName"], "literal");
        assert_eq!(mutant["status"], "Killed");
        assert_eq!(mutant["replacement"], "0");
        assert!(mutant["location"]["start"]["line"].as_u64().unwrap() >= 1);
        assert!(mutant["location"]["start"]["column"].as_u64().unwrap() >= 1);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn stryker_status_mapping_covers_all_categories() {
        assert_eq!(stryker_status(&dummy_result(Category::Killed)).0, "Killed");
        assert_eq!(
            stryker_status(&dummy_result(Category::Survived)).0,
            "Survived"
        );
        assert_eq!(
            stryker_status(&dummy_result(Category::TimedOut)).0,
            "Timeout"
        );
        assert_eq!(
            stryker_status(&dummy_result(Category::Error)).0,
            "RuntimeError"
        );
        assert_eq!(
            stryker_status(&dummy_result(Category::Equivalent)).0,
            "Ignored"
        );
    }

    #[test]
    fn ctrf_counts_each_category() {
        let results = vec![
            dummy_result(Category::Killed),
            dummy_result(Category::Survived),
            dummy_result(Category::TimedOut),
            dummy_result(Category::Error),
            dummy_equivalent("likely equivalent"),
        ];
        let paths = vec![PathBuf::from("src/foo.lua")];
        let data = ReportData {
            results: &results,
            project_root: Path::new("."),
            source_paths: &paths,
        };

        let report = generate_report(ReportFormat::Ctrf, data, None).unwrap();
        let v: serde_json::Value = serde_json::from_str(&report).unwrap();
        let summary = &v["results"]["summary"];
        assert_eq!(summary["tests"], 5);
        assert_eq!(summary["passed"], 1);
        assert_eq!(summary["failed"], 1);
        assert_eq!(summary["skipped"], 1);
        assert_eq!(summary["other"], 2);
        assert_eq!(summary["pending"], 0);

        let statuses: Vec<String> = v["results"]["tests"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["status"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(statuses.len(), 5);
        for expected in ["passed", "failed", "skipped", "other"] {
            assert!(
                statuses.iter().any(|s| s == expected),
                "missing ctrf status {expected}: {statuses:?}"
            );
        }
        assert_eq!(
            statuses.iter().filter(|s| s.as_str() == "other").count(),
            2,
            "timed_out and error must both map to other: {statuses:?}"
        );
    }

    #[test]
    fn report_format_from_str_roundtrip() {
        let cases = [
            ("summary", ReportFormat::Summary),
            ("per-mutant", ReportFormat::PerMutant),
            ("json", ReportFormat::Json),
            ("ctrf", ReportFormat::Ctrf),
            ("html", ReportFormat::Html),
            ("stryker", ReportFormat::Stryker),
        ];
        for (input, expected) in cases {
            assert_eq!(
                input.parse::<ReportFormat>().unwrap(),
                expected,
                "input: {input}"
            );
            assert_eq!(
                input.to_uppercase().parse::<ReportFormat>().unwrap(),
                expected,
                "uppercase input: {input}"
            );
        }
        assert_eq!(
            "Summary".parse::<ReportFormat>().unwrap(),
            ReportFormat::Summary
        );
        assert_eq!(
            "Per-Mutant".parse::<ReportFormat>().unwrap(),
            ReportFormat::PerMutant
        );
        let err = "bogus".parse::<ReportFormat>().unwrap_err();
        assert!(
            err.contains("unknown report format"),
            "unexpected error: {err}"
        );
        assert!("".parse::<ReportFormat>().is_err());
    }

    #[test]
    fn badge_thresholds() {
        fn score_with(killed: usize, survived: usize) -> MutationScore {
            MutationScore {
                killed,
                survived,
                ..Default::default()
            }
        }

        // N/A tier: no countable mutants.
        assert_eq!(
            file_score_badge_class(&MutationScore::default()),
            "no-mutants"
        );
        // Red tier: below 50%.
        assert_eq!(file_score_badge_class(&score_with(0, 1)), "low");
        assert_eq!(file_score_badge_class(&score_with(499, 501)), "low");
        // Yellow tier: 50% up to (not including) 80%.
        assert_eq!(file_score_badge_class(&score_with(1, 1)), "medium");
        assert_eq!(file_score_badge_class(&score_with(799, 201)), "medium");
        // Green tier: 80% and above.
        assert_eq!(file_score_badge_class(&score_with(4, 1)), "");
        assert_eq!(file_score_badge_class(&score_with(1, 0)), "");

        let low = file_score_badge_class(&score_with(0, 1));
        let medium = file_score_badge_class(&score_with(1, 1));
        let high = file_score_badge_class(&score_with(1, 0));
        let na = file_score_badge_class(&MutationScore::default());
        let mut tiers = vec![low, medium, high, na];
        tiers.sort_unstable();
        tiers.dedup();
        assert_eq!(
            tiers.len(),
            4,
            "expected four distinct badge classes, got low={low:?} medium={medium:?} high={high:?} na={na:?}"
        );
    }

    #[test]
    fn equivalent_reason_surfaced() {
        let equivalent = vec![dummy_equivalent("likely equivalent")];
        let killed = vec![dummy_result(Category::Killed)];
        let paths = vec![PathBuf::from("src/foo.lua")];
        fn data_for<'a>(results: &'a [MutantResult], paths: &'a [PathBuf]) -> ReportData<'a> {
            ReportData {
                results,
                project_root: Path::new("."),
                source_paths: paths,
            }
        }

        let per_mutant =
            generate_report(ReportFormat::PerMutant, data_for(&equivalent, &paths), None).unwrap();
        assert!(
            per_mutant.contains("(likely equivalent)"),
            "per-mutant report must surface the reason: {per_mutant}"
        );
        let per_mutant_killed =
            generate_report(ReportFormat::PerMutant, data_for(&killed, &paths), None).unwrap();
        assert!(
            !per_mutant_killed.contains('('),
            "killed mutant must not gain a stray reason: {per_mutant_killed}"
        );

        let json =
            generate_report(ReportFormat::Json, data_for(&equivalent, &paths), None).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["results"][0]["reason"], "likely equivalent");

        let json_killed =
            generate_report(ReportFormat::Json, data_for(&killed, &paths), None).unwrap();
        let vk: serde_json::Value = serde_json::from_str(&json_killed).unwrap();
        assert!(
            vk["results"][0]["reason"].is_null(),
            "killed mutant must have no reason: {json_killed}"
        );
    }

    #[test]
    fn category_classes() {
        let classes = [
            category_class("killed"),
            category_class("survived"),
            category_class("timed_out"),
            category_class("error"),
            category_class("equivalent"),
        ];
        for class in classes {
            assert!(!class.is_empty(), "category class must be non-empty");
        }
        let mut unique = classes.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(
            unique.len(),
            5,
            "expected five distinct category classes, got: {classes:?}"
        );
        assert_eq!(category_class("bogus"), "");
    }

    #[test]
    fn stryker_description_present_unless_both_empty() {
        let source = "local x = 1";
        let file = PathBuf::from("src/foo.lua");
        let both_empty = Mutant::from_candidate(
            CandidateMutant {
                start_byte: 0,
                end_byte: 0,
                replacement: String::new(),
            },
            "dummy",
            &file,
            source,
        );
        let original_only = Mutant::from_candidate(
            CandidateMutant {
                start_byte: 10,
                end_byte: 11,
                replacement: String::new(),
            },
            "dummy",
            &file,
            source,
        );
        let replacement_only = Mutant::from_candidate(
            CandidateMutant {
                start_byte: 0,
                end_byte: 0,
                replacement: "0".to_string(),
            },
            "dummy",
            &file,
            source,
        );
        let mk_killed = |mutant| MutantResult::Killed {
            mutant,
            duration_ms: 1,
            stdout_snippet: String::new(),
            stderr_snippet: String::new(),
        };
        let results = vec![
            mk_killed(both_empty),
            mk_killed(original_only),
            mk_killed(replacement_only),
        ];
        let paths = vec![file];
        let data = ReportData {
            results: &results,
            project_root: Path::new("."),
            source_paths: &paths,
        };

        let report = generate_report(ReportFormat::Stryker, data, None).unwrap();
        let v: serde_json::Value = serde_json::from_str(&report).unwrap();
        let files = v["files"].as_object().unwrap();
        assert_eq!(files.len(), 1);
        let descriptions: Vec<Option<String>> = files.values().next().unwrap()["mutants"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| {
                m["description"]
                    .as_str()
                    .map(std::string::ToString::to_string)
            })
            .collect();
        assert_eq!(descriptions.len(), 3);
        assert!(
            descriptions.contains(&None),
            "both-empty mutant must have no description: {descriptions:?}"
        );
        assert!(
            descriptions.contains(&Some("1 -> ".to_string())),
            "original-only mutant must keep its description: {descriptions:?}"
        );
        assert!(
            descriptions.contains(&Some(" -> 0".to_string())),
            "replacement-only mutant must keep its description: {descriptions:?}"
        );
    }
}
