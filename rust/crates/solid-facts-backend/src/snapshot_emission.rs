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

/// Whether a finding is shown to users. Uncertifiable findings are an internal
/// measure (sweeps, coverage, the runtime oracle): a user cannot act on "not
/// proven either way". The exception is a project-level notice
/// (`subjectKind: "project"`): the installed Solid runtime is refused or its
/// release is unaudited, so the analysis did not apply as stated, and a run
/// that hid it would look clean over a project it never vouched for.
pub(crate) fn user_facing(kind: &str, subject_kind: &str) -> bool {
    kind != "uncertifiable" || subject_kind == "project"
}

/// Whether `--certify` fails. For users, a violation or a project-level
/// notice fails it; with `--internal`, anything short of certified does.
pub(crate) fn certify_fails(status: &str, notices: bool, internal: bool) -> bool {
    if internal {
        status != "certified"
    } else {
        status == "violation" || notices
    }
}

/// Whether `snapshot` holds a project-level notice.
fn has_notice(snapshot: &Snapshot) -> bool {
    snapshot
        .findings
        .iter()
        .any(|finding| finding.kind == "uncertifiable" && finding.subject_kind == "project")
}

/// The status line users see: a violation, a project-level notice, or clean.
fn user_status(snapshot: &Snapshot) -> &'static str {
    if snapshot.status == "violation" {
        "violation"
    } else if has_notice(snapshot) {
        "notice"
    } else {
        "clean"
    }
}

/// Render a diagnostic snapshot and decide its process result.
///
/// Callers only write the returned bytes; output formatting and certification
/// semantics stay local to this module. `json` is the complete machine report
/// (every finding, internal ones included); the human formats show users only
/// [`user_facing`] findings unless `internal` asks for the complete view.
pub(crate) fn emit(
    dialect: &'static Dialect,
    format: &str,
    project_id: &str,
    snapshot: &Snapshot,
    certify: bool,
    internal: bool,
    elapsed: Duration,
) -> Result<Emission, Box<dyn std::error::Error>> {
    let shown;
    let snapshot = if internal || format == "json" {
        snapshot
    } else {
        shown = Snapshot {
            findings: snapshot
                .findings
                .iter()
                .filter(|finding| user_facing(&finding.kind, &finding.subject_kind))
                .cloned()
                .collect(),
            ..snapshot.clone()
        };
        &shown
    };
    let output = match format {
        "json" => {
            let mut output = json_output::go_compatible_report(snapshot)?.output;
            output.push(b'\n');
            output
        }
        "text" => {
            let status = if internal {
                snapshot.status.as_str()
            } else {
                user_status(snapshot)
            };
            let mut output = format!("{project_id}: {status}\n").into_bytes();
            output.extend_from_slice(&render_premises(snapshot));
            for finding in &snapshot.findings {
                output.extend_from_slice(
                    format!("{} [{}] {}\n", finding.id, finding.kind, finding.message).as_bytes(),
                );
            }
            output
        }
        "default" if !internal => render_user(dialect, project_id, snapshot, elapsed)?,
        "default" => render_default(dialect, project_id, snapshot, elapsed, true)?,
        "full" => render_default(dialect, project_id, snapshot, elapsed, false)?,
        format => return Err(format!("unsupported format {format:?}").into()),
    };
    Ok(Emission {
        output,
        exit_code: i32::from(
            certify && certify_fails(&snapshot.status, has_notice(snapshot), internal),
        ),
    })
}

/// The `json` report narrowed to the findings that can matter to `file`
/// (`--only-file`), and the process result `emit` would give it.
pub(crate) fn emit_only_file(
    snapshot: &Snapshot,
    file: &str,
    certify: bool,
    internal: bool,
) -> Result<Emission, Box<dyn std::error::Error>> {
    let report = json_output::go_compatible_report(snapshot)?;
    let mut output = report
        .index
        .only_file(&report.output, file)
        .unwrap_or(report.output);
    output.push(b'\n');
    Ok(Emission {
        output,
        exit_code: i32::from(
            certify && certify_fails(&snapshot.status, has_notice(snapshot), internal),
        ),
    })
}

/// The rules whose uncertifiable findings describe what the analysis could not
/// see (a package whose behavior is unavailable, a dispatch it could not resolve), not a
/// claim about code the user wrote. ADR 0202 groups them in the default output.
const COVERAGE_RULES: &[&str] = &[
    "package-behavior-unavailable",
    "reactive-dispatch-unresolved",
    "unaudited-solid-release",
];

/// How many coverage groups the default output lists before summarizing.
const COVERAGE_GROUPS_SHOWN: usize = 20;

fn render_premises(snapshot: &Snapshot) -> Vec<u8> {
    if snapshot.premises.is_empty() {
        return Vec::new();
    }
    format!("Premises: {}.\n", snapshot.premises.join("; ")).into_bytes()
}

/// The default output users see: violations, then project-level notices.
fn render_user(
    dialect: &'static Dialect,
    project_id: &str,
    snapshot: &Snapshot,
    elapsed: Duration,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let project = Path::new(project_id);
    let cwd = project.parent().unwrap_or_else(|| Path::new("."));
    let mut output = render_premises(snapshot);
    let (notices, violations): (Vec<_>, Vec<_>) = snapshot
        .findings
        .iter()
        .partition(|finding| finding.kind == "uncertifiable");
    if !violations.is_empty() {
        output.extend_from_slice(format!("Violations ({})\n\n", violations.len()).as_bytes());
        output.extend(render_diagnostics(dialect, cwd, violations)?);
    }
    if !notices.is_empty() {
        output.extend_from_slice(
            format!(
                "Notices ({}): the analysis did not apply to this project as stated\n\n",
                notices.len()
            )
            .as_bytes(),
        );
        output.extend(render_diagnostics(dialect, cwd, notices)?);
    }
    output.extend(render_footer(dialect, snapshot, elapsed));
    Ok(output)
}

fn render_footer(dialect: &'static Dialect, snapshot: &Snapshot, elapsed: Duration) -> Vec<u8> {
    let threads = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1);
    format!(
        "Finished in {}ms on {} files with {} rules using {threads} threads.\n",
        elapsed.as_millis(),
        snapshot.metrics.files_analyzed,
        dialect.rule_count,
    )
    .into_bytes()
}

fn render_default(
    dialect: &'static Dialect,
    project_id: &str,
    snapshot: &Snapshot,
    elapsed: Duration,
    tiered: bool,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let project = Path::new(project_id);
    let cwd = project.parent().unwrap_or_else(|| Path::new("."));
    let mut output = render_premises(snapshot);
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
    output.extend(render_footer(dialect, snapshot, elapsed));
    Ok(output)
}

/// How many subjects a coverage family names before summarizing the rest.
const COVERAGE_SUBJECTS_SHOWN: usize = 5;

/// ADR 0205: the heading of a coverage family, and what its subjects are.
fn coverage_family_heading(family: &str) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match family {
        "package-boundary" => (
            "imports from packages whose behavior is unavailable from source",
            "package",
            "packages",
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
mod tests;
