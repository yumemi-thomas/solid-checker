use std::{
    collections::BTreeMap,
    fmt, fs,
    path::{Path, PathBuf},
    time::Duration,
};

use oxc_diagnostics::{
    DiagnosticService, Error, GraphicalReportHandler, LabeledSpan, OxcDiagnostic,
    reporter::{DiagnosticReporter, DiagnosticResult},
};
use oxc_miette::ReportHandler;
use solid_facts_backend::dialect::Dialect;
use solid_facts_backend::{Snapshot, SnapshotFinding};

use crate::json_output;

pub(crate) struct Emission {
    pub(crate) output: Vec<u8>,
    pub(crate) exit_code: i32,
}

/// Render a diagnostic snapshot and decide its process result.
///
/// Callers only write the returned bytes; output formatting and certification
/// semantics stay local to this module.
pub(crate) fn emit(
    dialect: &'static Dialect,
    format: &str,
    project_id: &str,
    snapshot: &Snapshot,
    certify: bool,
    elapsed: Duration,
) -> Result<Emission, Box<dyn std::error::Error>> {
    let output = match format {
        "json" => {
            let mut output = json_output::go_compatible(snapshot, true)?;
            output.push(b'\n');
            output
        }
        "text" => {
            let mut output = format!("{project_id}: {}\n", snapshot.status).into_bytes();
            for finding in &snapshot.findings {
                output.extend_from_slice(
                    format!("{} [{}] {}\n", finding.id, finding.kind, finding.message).as_bytes(),
                );
            }
            output
        }
        "default" => render_default(dialect, project_id, snapshot, elapsed, true)?,
        "full" => render_default(dialect, project_id, snapshot, elapsed, false)?,
        format => return Err(format!("unsupported format {format:?}").into()),
    };
    Ok(Emission {
        output,
        exit_code: i32::from(certify && snapshot.status != "certified"),
    })
}

/// The rules whose uncertifiable findings describe what the analysis could not
/// see (a package without a contract, a dispatch it could not resolve), not a
/// claim about code the user wrote. ADR 0202 groups them in the default output.
const COVERAGE_RULES: &[&str] = &[
    "package-contract-incomplete",
    "reactive-dispatch-unresolved",
    "reactive-source-uncaptured",
    "unaudited-solid-release",
];

/// How many coverage groups the default output lists before summarizing.
const COVERAGE_GROUPS_SHOWN: usize = 20;

fn render_default(
    dialect: &'static Dialect,
    project_id: &str,
    snapshot: &Snapshot,
    elapsed: Duration,
    tiered: bool,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let project = Path::new(project_id);
    let cwd = project.parent().unwrap_or_else(|| Path::new("."));
    let mut output = Vec::new();
    if tiered {
        // ADR 0202: violations, then uncertifiable findings about the user's
        // code, then the analysis-coverage gaps grouped by root cause.
        let coverage = |finding: &&SnapshotFinding| {
            finding.kind == "uncertifiable" && COVERAGE_RULES.contains(&finding.rule.as_str())
        };
        let violations = snapshot
            .findings
            .iter()
            .filter(|finding| finding.kind != "uncertifiable")
            .collect::<Vec<_>>();
        let review = snapshot
            .findings
            .iter()
            .filter(|finding| finding.kind == "uncertifiable" && !coverage(finding))
            .collect::<Vec<_>>();
        let gaps = snapshot
            .findings
            .iter()
            .filter(coverage)
            .collect::<Vec<_>>();
        if !violations.is_empty() {
            output.extend_from_slice(format!("Violations ({})\n\n", violations.len()).as_bytes());
            output.extend(render_diagnostics(dialect, cwd, violations)?);
        }
        if !review.is_empty() {
            output.extend_from_slice(
                format!(
                    "Needs review ({}): solid-checker could not prove these correct or wrong\n\n",
                    review.len()
                )
                .as_bytes(),
            );
            output.extend(render_diagnostics(dialect, cwd, review)?);
        }
        if !gaps.is_empty() {
            output.extend(render_coverage(cwd, &gaps));
        }
    } else {
        output.extend(render_diagnostics(
            dialect,
            cwd,
            snapshot.findings.iter().collect(),
        )?);
    }
    let threads = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1);
    output.extend_from_slice(
        format!(
            "Finished in {}ms on {} files with {} rules using {threads} threads.\n",
            elapsed.as_millis(),
            snapshot.metrics.files_analyzed,
            dialect.rule_count,
        )
        .as_bytes(),
    );
    Ok(output)
}

/// How many subjects a coverage family names before summarizing the rest.
const COVERAGE_SUBJECTS_SHOWN: usize = 5;

/// ADR 0205: the heading of a coverage family, and what its subjects are.
fn coverage_family_heading(family: &str) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match family {
        "package-contract" => (
            "imports from packages without a complete reactivity contract",
            "package",
            "packages",
        ),
        "own-export-contract" => (
            "callbacks this project's exports hand to code with unknown timing",
            "export",
            "exports",
        ),
        "caller-supplied-member" => (
            "helpers that call a method on a value their caller supplies",
            "helper",
            "helpers",
        ),
        "call-target" => (
            "calls whose runtime target cannot be selected exactly",
            "call",
            "calls",
        ),
        "undescribed-callee" => (
            "reactive values passed to functions whose behaviour is not described",
            "function",
            "functions",
        ),
        "leaf-callback" => (
            "calls in a leaf owner's callback whose body cannot be followed",
            "owner",
            "owners",
        ),
        _ => return None,
    })
}

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// The analysis-coverage gaps, grouped by root cause (ADR 0202, ADR 0205): one
/// group per coverage family, naming its most frequent subjects, and one per
/// rule and message for a gap with no family.
fn render_coverage(cwd: &Path, gaps: &[&SnapshotFinding]) -> Vec<u8> {
    let mut groups = BTreeMap::<(&str, &str, &str), Vec<&SnapshotFinding>>::new();
    for finding in gaps {
        let key = if coverage_family_heading(&finding.coverage_family).is_some() {
            (finding.id.as_str(), finding.coverage_family.as_str(), "")
        } else {
            (
                finding.id.as_str(),
                finding.rule.as_str(),
                finding.message.as_str(),
            )
        };
        groups.entry(key).or_default().push(finding);
    }
    let mut groups = groups.into_iter().collect::<Vec<_>>();
    groups.sort_by(|left, right| right.1.len().cmp(&left.1.len()).then(left.0.cmp(&right.0)));
    let mut output = format!(
        "Analysis coverage: {} in {} that solid-checker could not analyze; they are not findings about your code (all of them: --format full or json)\n\n",
        plural(gaps.len(), "site", "sites"),
        plural(groups.len(), "group", "groups"),
    );
    for ((id, family, message), sites) in groups.iter().take(COVERAGE_GROUPS_SHOWN) {
        let first = sites
            .iter()
            .map(|finding| &finding.primary_location)
            .min_by(|left, right| {
                (left.path.as_str(), left.line, left.column).cmp(&(
                    right.path.as_str(),
                    right.line,
                    right.column,
                ))
            });
        let at = first
            .map(|location| {
                let path = Path::new(&location.path);
                let shown = path.strip_prefix(cwd).unwrap_or(path).display().to_string();
                format!("{shown}:{}:{}", location.line, location.column)
            })
            .unwrap_or_default();
        if let Some((heading, one, many)) = coverage_family_heading(family) {
            let mut subjects = BTreeMap::<&str, usize>::new();
            for finding in sites {
                *subjects
                    .entry(finding.coverage_subject.as_str())
                    .or_default() += 1;
            }
            let mut subjects = subjects.into_iter().collect::<Vec<_>>();
            subjects.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(right.0)));
            let mut named = subjects
                .iter()
                .take(COVERAGE_SUBJECTS_SHOWN)
                .map(|(subject, count)| format!("{subject} ({count})"))
                .collect::<Vec<_>>();
            if subjects.len() > COVERAGE_SUBJECTS_SHOWN {
                named.push(format!(
                    "and {} more",
                    subjects.len() - COVERAGE_SUBJECTS_SHOWN
                ));
            }
            let mut heading = heading.to_owned();
            if let Some(first) = heading.get_mut(..1) {
                first.make_ascii_uppercase();
            }
            output.push_str(&format!(
                "  [{id}] {heading}: {} in {}\n      {}\n      first at {at}\n",
                plural(sites.len(), "site", "sites"),
                plural(subjects.len(), one, many),
                named.join(", "),
            ));
        } else {
            output.push_str(&format!(
                "  [{id}] {message}\n      {}, first at {at}\n",
                plural(sites.len(), "site", "sites"),
            ));
        }
    }
    if groups.len() > COVERAGE_GROUPS_SHOWN {
        output.push_str(&format!(
            "  ... and {} more\n",
            plural(groups.len() - COVERAGE_GROUPS_SHOWN, "group", "groups"),
        ));
    }
    output.push('\n');
    output.into_bytes()
}

/// The graphical rendering of `findings`, grouped by file.
fn render_diagnostics(
    dialect: &'static Dialect,
    cwd: &Path,
    findings: Vec<&SnapshotFinding>,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut by_path = BTreeMap::<PathBuf, Vec<&SnapshotFinding>>::new();
    for finding in findings {
        by_path
            .entry(PathBuf::from(&finding.primary_location.path))
            .or_default()
            .push(finding);
    }

    let (mut service, sender) = DiagnosticService::new(Box::new(DefaultReporter::new()));
    for (path, findings) in by_path {
        let source = fs::read_to_string(&path)?;
        let diagnostics = findings
            .into_iter()
            .map(|finding| {
                let location = &finding.primary_location;
                let start = usize::try_from(location.start_byte).unwrap_or(0);
                let end = usize::try_from(location.end_byte).unwrap_or(start);
                let message = format!("[{}] {}", finding.id, finding.message);
                let mut diagnostic = if finding.severity == "warning" {
                    OxcDiagnostic::warn(message)
                } else {
                    OxcDiagnostic::error(message)
                }
                .with_error_code("solid-checker", "certification")
                .with_label(LabeledSpan::new_with_span(
                    evidence_label(finding),
                    (start, end.saturating_sub(start)),
                ));
                let mut help = Vec::new();
                if !finding.hint.is_empty() {
                    help.push(finding.hint.clone());
                }
                if !finding.related_locations.is_empty() {
                    help.push(format!(
                        "{} related location{} available in JSON output",
                        finding.related_locations.len(),
                        if finding.related_locations.len() == 1 {
                            ""
                        } else {
                            "s"
                        }
                    ));
                }
                // The vendored graphical handler does not render
                // Diagnostic::url(), so the docs link rides in the help text.
                help.push(format!("docs: {}", (dialect.docs_url)(&finding.rule)));
                if !help.is_empty() {
                    diagnostic = diagnostic.with_help(help.join("\n"));
                }
                diagnostic
            })
            .collect();
        let wrapped = DiagnosticService::wrap_diagnostics(cwd, &path, &source, diagnostics);
        sender.send(wrapped)?;
    }
    drop(sender);

    let mut output = Vec::new();
    service.run(&mut output);
    Ok(output)
}

struct DefaultReporter {
    handler: GraphicalReportHandler,
}

impl DefaultReporter {
    fn new() -> Self {
        Self {
            handler: GraphicalReportHandler::new(),
        }
    }
}

impl DiagnosticReporter for DefaultReporter {
    fn finish(&mut self, result: &DiagnosticResult) -> Option<String> {
        let warnings = result.warnings_count();
        let errors = result.errors_count();
        Some(format!(
            "Found {warnings} warning{} and {errors} error{}.\n",
            if warnings == 1 { "" } else { "s" },
            if errors == 1 { "" } else { "s" },
        ))
    }

    fn render_error(&mut self, error: Error) -> Option<String> {
        struct Render<'a> {
            handler: &'a GraphicalReportHandler,
            error: &'a Error,
        }

        impl fmt::Debug for Render<'_> {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.handler.debug(self.error.as_ref(), formatter)
            }
        }

        Some(format!(
            "{:?}\n",
            Render {
                handler: &self.handler,
                error: &error,
            }
        ))
    }
}

fn evidence_label(finding: &SnapshotFinding) -> Option<String> {
    let messages = finding
        .evidence
        .iter()
        .filter_map(|step| (!step.message.is_empty()).then_some(step.message.as_str()))
        .collect::<Vec<_>>();
    (!messages.is_empty()).then(|| messages.join("; "))
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{create_dir_all, remove_dir_all, write},
        time::Duration,
        time::{SystemTime, UNIX_EPOCH},
    };

    use solid_facts_backend::{Metrics, Snapshot, SnapshotFinding, SourceLocation};

    use super::emit;

    fn snapshot(status: &str) -> Snapshot {
        Snapshot {
            feedback_facts: Vec::new(),
            status: status.into(),
            findings: Vec::new(),
            package_summaries: Vec::new(),
            metrics: Metrics {
                files_analyzed: 1,
                functions_analyzed: 2,
                proof_obligations: 3,
                cached_summaries: 4,
                unresolved_obligations: 0,
            },
        }
    }

    #[test]
    fn text_and_json_share_certification_semantics() {
        for format in ["text", "json"] {
            assert_eq!(
                emit(
                    solid_facts_backend::dialect::default_dialect(),
                    format,
                    "project",
                    &snapshot("violation"),
                    true,
                    Duration::ZERO,
                )
                .unwrap()
                .exit_code,
                1
            );
            assert_eq!(
                emit(
                    solid_facts_backend::dialect::default_dialect(),
                    format,
                    "project",
                    &snapshot("certified"),
                    true,
                    Duration::ZERO,
                )
                .unwrap()
                .exit_code,
                0
            );
            assert_eq!(
                emit(
                    solid_facts_backend::dialect::default_dialect(),
                    format,
                    "project",
                    &snapshot("violation"),
                    false,
                    Duration::ZERO,
                )
                .unwrap()
                .exit_code,
                0
            );
        }
    }

    #[test]
    fn wire_snapshot_with_omitted_optional_fields_round_trips() {
        // The daemon client re-parses emitted snapshots; findings serialize
        // without their empty optional fields, so deserialization must
        // default them instead of failing (which silently downgraded every
        // daemon check with findings to a cold one-shot run).
        let wire = r#"{
            "status": "violation",
            "findings": [{
                "id": "SC1003",
                "rule": "strict-read-untracked",
                "kind": "violation",
                "severity": "error",
                "message": "reactive read outside tracked scope",
                "primaryLocation": {
                    "path": "App.tsx", "startByte": 1, "endByte": 2,
                    "line": 1, "column": 1
                }
            }],
            "packageSummaries": [{
                "name": "solid-js", "evidence": "reviewed", "exportsAnalyzed": 3
            }],
            "metrics": {
                "filesAnalyzed": 1, "functionsAnalyzed": 1,
                "proofObligations": 1, "cachedSummaries": 0,
                "unresolvedObligations": 0
            }
        }"#;
        let decoded: Snapshot = serde_json::from_str(wire).unwrap();
        assert_eq!(decoded.findings[0].subject_kind, "");
        assert!(decoded.findings[0].fixes.is_empty());
        let reemitted = emit(
            solid_facts_backend::dialect::default_dialect(),
            "json",
            "project",
            &decoded,
            true,
            Duration::ZERO,
        )
        .unwrap();
        assert_eq!(reemitted.exit_code, 1);
    }

    #[test]
    fn formats_snapshot_without_adapter_specific_logic() {
        let text = emit(
            solid_facts_backend::dialect::default_dialect(),
            "text",
            "project",
            &snapshot("certified"),
            false,
            Duration::ZERO,
        )
        .unwrap();
        assert_eq!(text.output, b"project: certified\n");

        let json = emit(
            solid_facts_backend::dialect::default_dialect(),
            "json",
            "project",
            &snapshot("certified"),
            false,
            Duration::ZERO,
        )
        .unwrap();
        let decoded: serde_json::Value = serde_json::from_slice(&json.output).unwrap();
        assert_eq!(decoded["status"], "certified");
        assert!(json.output.ends_with(b"\n"));
    }

    #[test]
    fn default_format_renders_oxc_source_frames() {
        let directory = std::env::temp_dir().join(format!(
            "solid-checker-default-format-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        create_dir_all(&directory).unwrap();
        let source_path = directory.join("App.tsx");
        write(
            &source_path,
            "export function Bad({ title }) {\n  return <h2>{title}</h2>;\n}\n",
        )
        .unwrap();
        let mut snapshot = snapshot("violation");
        snapshot.findings.push(SnapshotFinding {
            id: "SC1003".into(),
            rule: "no-destructure".into(),
            kind: "violation".into(),
            severity: "error".into(),
            message: "destructuring component props reads them outside tracking".into(),
            hint: "Keep the props object intact and read props.<name> inside JSX.".into(),
            analysis_context: String::new(),
            subject_kind: "component-props".into(),
            coverage_family: String::new(),
            coverage_subject: String::new(),
            primary_location: SourceLocation {
                path: source_path.to_string_lossy().into_owned(),
                start_byte: 20,
                end_byte: 29,
                line: 1,
                column: 21,
            },
            related_locations: Vec::new(),
            evidence: Vec::new(),
            fixes: Vec::new(),
        });

        let rendered = emit(
            solid_facts_backend::dialect::default_dialect(),
            "default",
            &directory.join("tsconfig.json").to_string_lossy(),
            &snapshot,
            true,
            Duration::from_millis(295),
        )
        .unwrap();
        let text = String::from_utf8(rendered.output).unwrap();
        assert!(text.contains("solid-checker(certification)"));
        assert!(text.contains("[SC1003]"));
        assert!(text.contains("App.tsx"));
        assert!(text.contains("destructuring component props"));
        assert!(text.contains("Keep the props object intact"));
        assert!(text.contains("docs/rules/no-destructure.md"));
        assert!(text.contains(&format!(
            "Finished in 295ms on 1 files with {} rules using",
            solid_facts_backend::dialect::default_dialect().rule_count
        )));
        assert_eq!(rendered.exit_code, 1);
        remove_dir_all(directory).unwrap();
    }
}
