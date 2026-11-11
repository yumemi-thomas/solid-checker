//! ADR 0949, ADR 0284: the one place a package's finding may leave the report.
//!
//! A package analyzed as source is not the project's code, so its own findings are not reported where
//! they stand. What a package does is the project's to answer for, at the project site that runs it
//! (ADR 0284), and dropping a finding there once left silence wherever no summary or attribution
//! carried the execution: a package's write in an owned scope, a read through a JSX render, a handler's
//! requirement past `.call`, `new`, a tagged template or a getter, a package `U` (unproven) turned into
//! "clean". A finding of a package now leaves the report only by being handed to the project, as
//! `SiteSilence::Delegated` hands a site to another rule: each dropped finding, a violation or an
//! uncertifiable result of any rule, stands again as an **uncertifiable** finding of the same rule at
//! every project site the program says may run the function that holds it
//! ([`solid_reactive_ir::PackageEntries`]) and at every project reference no exact call or tag decides
//! ([`solid_reactive_ir::PackageReferences`]); where no site is proven it stands once per package and
//! rule at the project's first import of the package, where there is no import at the project's first
//! file, and where there is no file at the project's configuration. Never a violation
//! (the package's own execution is not proven by a path that is not followed), never silence. A site
//! that already holds a finding of the rule (a violation or an obligation the call walk or the read
//! transport attributed) is that finding's: nothing is added beside it.
//!
//! One kind of finding is not handed off: a **style** rule's (SC80xx: duplicate props, prefer-for,
//! prefer-show). A style finding in a package's code is no claim about what the project's code does at
//! run time, so there is nothing for a project site to answer for. It is dropped under the named premise
//! `package-style-not-project` (`DropPass::PackageStyleNotProject`, `PremiseKind::PackageStyleNotProject`),
//! typed and written to the proof log with the finding's package site; every other rule keeps the hand-off.
//!
//! [`hand_off`] returns the [`DropReport`] of the drop, and `diagnostics.rs` is the only caller of it:
//! the tests in this module and the proof log (`attributed` lines, checked by the proof gate) count
//! drops against attributions and fail on any drop with none.

use std::collections::{BTreeSet, HashMap};

use solid_facts::ProjectFacts;
use solid_reactive_ir::{
    DependencyEntry, DropPass, EvidenceStep, Finding, PACKAGE_FINDING_AT_ENTRY, PackageEntrySite,
    Program, ProofStage,
};
use typefacts::Location;

/// What one drop did, counted per dropped finding.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct DropReport {
    /// Findings of a runtime-code rule located in a package, discarded from the report.
    pub(crate) dropped: usize,
    /// Of those, the ones that stand again at a project site the program proves enters the package code.
    pub(crate) at_entry: usize,
    /// The ones that stand at the project's first import of the package: no exact entry is proven.
    pub(crate) at_import: usize,
    /// Style findings located in a package, dropped under the premise `package-style-not-project`:
    /// no project row stands for them, and the proof log names the premise ([`is_style_rule`]).
    pub(crate) style_dropped: usize,
    /// Each dropped finding's replacements, by the package location it was at, for the log and the tests.
    pub(crate) attributed: Vec<(Location, Vec<Location>)>,
}

impl DropReport {
    /// Whether every drop left a project row: a replacement, or a finding of the rule already at the site.
    pub(crate) fn every_drop_attributed(&self) -> bool {
        self.attributed
            .iter()
            .all(|(_, replacements)| !replacements.is_empty())
            && self.at_entry + self.at_import == self.dropped
    }
}

/// Whether `id` is a style rule's code (SC80xx: duplicate props, prefer-for, prefer-show): the premise
/// `package-style-not-project` applies to a finding of it that sits in a package.
fn is_style_rule(id: &str) -> bool {
    id.starts_with("SC80")
}

/// Moves the findings located in a package out of `findings` and puts them back, as uncertifiable
/// findings, at the project sites that enter the package code (see the module). `proof_log` writes the
/// drops to the proof log under their typed passes (`findings` of the report, not the withheld ones).
pub(crate) fn hand_off(
    findings: &mut Vec<Finding>,
    facts: &ProjectFacts,
    program: &Program,
    proof_log: bool,
) -> DropReport {
    let mut report = DropReport::default();
    let in_package = |finding: &Finding| {
        facts
            .dependency_source_root(&finding.primary_location.path)
            .is_some()
    };
    if !findings.iter().any(in_package) {
        return report;
    }
    // A style rule's finding in a package: dropped under its named premise, handed to no site.
    let stage = proof_log.then(|| ProofStage::begin(findings)).flatten();
    let held = findings.len();
    findings.retain(|finding| !(in_package(finding) && is_style_rule(&finding.id)));
    report.style_dropped = held - findings.len();
    if let Some(stage) = stage {
        stage.end(DropPass::PackageStyleNotProject, findings);
    }
    // Every other package finding is handed to the project.
    let stage = proof_log.then(|| ProofStage::begin(findings)).flatten();
    let mut kept = Vec::with_capacity(findings.len());
    let mut dropped = Vec::new();
    for finding in std::mem::take(findings) {
        if in_package(&finding) {
            dropped.push(finding);
        } else {
            kept.push(finding);
        }
    }
    // A finding of the rule already at a project site is that site's: a violation or an obligation
    // the call walk or the read transport attributed there.
    let mut standing = kept
        .iter()
        .map(|finding| finding_key(finding, &finding.primary_location))
        .collect::<BTreeSet<_>>();
    let entries = entries_by_path(program);
    let mut first_imports = HashMap::<String, Location>::new();
    let mut replacements = Vec::new();
    for finding in dropped {
        report.dropped += 1;
        let root = facts
            .dependency_source_root(&finding.primary_location.path)
            .unwrap_or_default();
        let sites = sites_of(&finding.primary_location, root, &entries, program);
        let mut at = Vec::<Location>::new();
        if sites.is_empty() {
            // No exact project entry is proven: once per package and rule, at the project's first
            // import of the package (`finding_key` is the rule and the site).
            let location = first_imports
                .entry(root.to_owned())
                .or_insert_with(|| first_import_site(facts, root))
                .clone();
            let key = finding_key(&finding, &location);
            if standing.insert(key) {
                replacements.push(replacement(&finding, &location, None));
            }
            at.push(location);
            report.at_import += 1;
        } else {
            for site in sites {
                let key = finding_key(&finding, &site.location);
                if standing.insert(key) {
                    replacements.push(replacement(&finding, &site.location, Some(site.kind)));
                }
                at.push(site.location.clone());
            }
            report.at_entry += 1;
        }
        report
            .attributed
            .push((finding.primary_location.clone(), at));
    }
    kept.extend(replacements);
    *findings = kept;
    if let Some(stage) = stage {
        stage.end(DropPass::DependencySource, findings);
    }
    report
}

/// Where a package finding stands again: the project sites that may run the innermost package function
/// holding it, and the project references no exact call or tag decides, of its package.
fn sites_of<'p>(
    location: &Location,
    root: &str,
    entries: &HashMap<&str, Vec<&'p solid_reactive_ir::PackageEntries>>,
    program: &'p Program,
) -> Vec<&'p PackageEntrySite> {
    let innermost = entries
        .get(location.path.as_ref())
        .into_iter()
        .flatten()
        .filter(|entry| {
            entry.function.start_byte <= location.start_byte
                && location.end_byte <= entry.function.end_byte
        })
        .min_by_key(|entry| entry.function.end_byte - entry.function.start_byte);
    let mut sites = innermost
        .map(|entry| entry.sites.iter().collect::<Vec<_>>())
        .unwrap_or_default();
    for references in program
        .package_references
        .iter()
        .filter(|references| references.root == root)
    {
        sites.extend(references.sites.iter());
    }
    sites
}

fn entries_by_path(program: &Program) -> HashMap<&str, Vec<&solid_reactive_ir::PackageEntries>> {
    let mut by_path = HashMap::<&str, Vec<_>>::new();
    for entry in &program.package_entries {
        by_path
            .entry(entry.function.path.as_ref())
            .or_default()
            .push(entry);
    }
    by_path
}

/// The project's first import of the package at `root` (the importing file's path, then the offset), or,
/// with none, its first project file, or, with no project file, its configuration: never absent.
fn first_import_site(facts: &ProjectFacts, root: &str) -> Location {
    let first = facts.resolved_imports.as_ref().and_then(|imports| {
        imports
            .iter()
            .filter(|(importer, import)| {
                facts.dependency_source_root(importer).is_none()
                    && std::path::Path::new(import.resolved_path.as_ref()).starts_with(root)
            })
            .map(|(importer, import)| {
                (
                    importer.to_owned(),
                    u64::from(import.span.start),
                    u64::from(import.span.end),
                )
            })
            .min()
    });
    if let Some((path, start_byte, end_byte)) = first {
        return Location {
            path: path.as_str().into(),
            start_byte,
            end_byte,
        };
    }
    // No project file loads the package (a transitive dependency no project file names): the
    // project's first file, else its configuration, so that a drop always leaves a project row.
    let path = facts
        .files
        .iter()
        .find(|file| facts.dependency_source_root(file.path.as_str()).is_none())
        .map_or_else(
            || facts.project_id.as_str().into(),
            |file| file.path.shared(),
        );
    Location {
        path,
        start_byte: 0,
        end_byte: 0,
    }
}

/// The dedupe identity of a finding at a location: its rule code and where it starts.
fn finding_key(finding: &Finding, location: &Location) -> (String, String, u64) {
    (
        finding.id.clone(),
        location.path.to_string(),
        location.start_byte,
    )
}

/// `dropped` as an uncertifiable finding of its own rule at `at`, the project site that enters the package code.
fn replacement(dropped: &Finding, at: &Location, kind: Option<DependencyEntry>) -> Finding {
    let site = match kind {
        Some(DependencyEntry::Call) => "call",
        Some(DependencyEntry::Render) => "element",
        Some(DependencyEntry::Reference) => "use",
        None => "import",
    };
    let mut evidence = vec![EvidenceStep {
        message: match kind {
            Some(_) => format!("this {site} enters package code that holds the finding below"),
            None => "no project site is proven to enter the package code that holds the finding below, so the project's first import of the package stands for it".into(),
        },
        location: Some(at.clone()),
    }];
    evidence.extend(dropped.evidence.iter().cloned());
    evidence.push(EvidenceStep {
        message: "the finding, in the package's source".into(),
        location: Some(dropped.primary_location.clone()),
    });
    Finding {
        id: dropped.id.clone(),
        rule: dropped.rule.clone(),
        kind: "uncertifiable".into(),
        message: format!(
            "{}; this finding is in a package analyzed as source and is reported at this {site} because the package code that holds it may run from here, which is not proven either way",
            dropped.message
        ),
        hint: dropped.hint.clone(),
        severity: dropped.severity.clone(),
        analysis_context: format!("{PACKAGE_FINDING_AT_ENTRY}:{}", dropped.id),
        subject_kind: dropped.subject_kind.clone(),
        coverage_family: dropped.coverage_family.clone(),
        coverage_subject: dropped.coverage_subject.clone(),
        primary_location: at.clone(),
        related_locations: vec![dropped.primary_location.clone()],
        evidence,
        fixes: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solid_reactive_ir::{PackageEntries, PackageReferences};

    fn at(path: &str, start: u64, end: u64) -> Location {
        Location {
            path: path.into(),
            start_byte: start,
            end_byte: end,
        }
    }

    fn finding(id: &str, kind: &str, location: Location) -> Finding {
        Finding {
            id: id.into(),
            rule: "some-rule".into(),
            kind: kind.into(),
            message: "message".into(),
            hint: String::new(),
            severity: "error".into(),
            analysis_context: String::new(),
            subject_kind: String::new(),
            coverage_family: String::new(),
            coverage_subject: String::new(),
            primary_location: location,
            related_locations: Vec::new(),
            evidence: Vec::new(),
            fixes: Vec::new(),
        }
    }

    fn facts() -> ProjectFacts {
        let project_id = "/p/tsconfig.json".to_string();
        ProjectFacts {
            generation: solid_facts::core::Generation::new(1).unwrap(),
            project_id: project_id.clone(),
            files: Vec::new(),
            typescript: solid_facts::TypeScriptTable::from_parts(
                3,
                1,
                project_id,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
            typescript_changes: None,
            resolved_imports: None,
            runtime_resolutions: None,
            runtime_symbol_redirects: Default::default(),
            dependency_source_roots: vec!["/p/node_modules/pkg".into()],
            dependency_source_call_errors: Vec::new(),
        }
    }

    fn program(entries: Vec<PackageEntries>, references: Vec<PackageReferences>) -> Program {
        Program {
            package_entries: entries,
            package_references: references,
            ..Program::default()
        }
    }

    #[test]
    fn a_dropped_violation_stands_again_as_an_uncertifiable_at_each_entry() {
        let mut findings = vec![
            finding(
                "SC2001",
                "violation",
                at("/p/node_modules/pkg/index.js", 10, 20),
            ),
            finding("SC2001", "violation", at("/p/App.tsx", 1, 2)),
        ];
        let entries = vec![PackageEntries {
            function: at("/p/node_modules/pkg/index.js", 0, 40),
            sites: vec![
                PackageEntrySite {
                    location: at("/p/App.tsx", 100, 110),
                    kind: DependencyEntry::Render,
                },
                PackageEntrySite {
                    location: at("/p/App.tsx", 200, 210),
                    kind: DependencyEntry::Call,
                },
            ],
        }];
        let report = hand_off(
            &mut findings,
            &facts(),
            &program(entries, Vec::new()),
            false,
        );
        assert_eq!(report.dropped, 1);
        assert_eq!(report.at_entry, 1);
        assert!(report.every_drop_attributed());
        let rows = findings
            .iter()
            .map(|finding| (finding.primary_location.start_byte, finding.kind.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            rows,
            vec![
                (1, "violation"),
                (100, "uncertifiable"),
                (200, "uncertifiable")
            ]
        );
        assert!(
            findings[1]
                .analysis_context
                .starts_with(PACKAGE_FINDING_AT_ENTRY)
        );
    }

    #[test]
    fn a_site_that_holds_the_rule_already_keeps_its_own_row() {
        let mut findings = vec![
            finding(
                "SC1001",
                "uncertifiable",
                at("/p/node_modules/pkg/index.js", 10, 20),
            ),
            finding("SC1001", "violation", at("/p/App.tsx", 100, 105)),
        ];
        let entries = vec![PackageEntries {
            function: at("/p/node_modules/pkg/index.js", 0, 40),
            sites: vec![PackageEntrySite {
                location: at("/p/App.tsx", 100, 110),
                kind: DependencyEntry::Call,
            }],
        }];
        let report = hand_off(
            &mut findings,
            &facts(),
            &program(entries, Vec::new()),
            false,
        );
        assert!(report.every_drop_attributed());
        assert_eq!(findings.len(), 1, "the violation stands; nothing beside it");
        assert_eq!(findings[0].kind, "violation");
    }

    #[test]
    fn a_finding_with_no_proven_entry_stands_at_the_project_never_silent() {
        let mut findings = vec![finding(
            "SC4001",
            "uncertifiable",
            at("/p/node_modules/pkg/index.js", 10, 20),
        )];
        let report = hand_off(
            &mut findings,
            &facts(),
            &program(Vec::new(), Vec::new()),
            false,
        );
        assert_eq!(report.dropped, 1);
        assert_eq!(report.at_import, 1);
        assert!(report.every_drop_attributed());
        // The facts of this test hold no project file and no import: the project's configuration stands for it.
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].kind, "uncertifiable");
        assert_eq!(
            findings[0].primary_location.path.as_ref(),
            "/p/tsconfig.json"
        );
        assert_eq!(
            findings[0].analysis_context,
            format!("{PACKAGE_FINDING_AT_ENTRY}:SC4001")
        );
    }

    #[test]
    fn a_style_finding_in_a_package_is_dropped_by_its_premise_and_handed_to_no_site() {
        let mut findings = vec![
            finding(
                "SC8003",
                "violation",
                at("/p/node_modules/pkg/index.js", 10, 20),
            ),
            finding(
                "SC8015",
                "uncertifiable",
                at("/p/node_modules/pkg/index.js", 30, 40),
            ),
            finding("SC8003", "violation", at("/p/App.tsx", 1, 2)),
        ];
        let entries = vec![PackageEntries {
            function: at("/p/node_modules/pkg/index.js", 0, 50),
            sites: vec![PackageEntrySite {
                location: at("/p/App.tsx", 100, 110),
                kind: DependencyEntry::Render,
            }],
        }];
        let report = hand_off(
            &mut findings,
            &facts(),
            &program(entries, Vec::new()),
            false,
        );
        assert_eq!(
            report.style_dropped, 2,
            "both style findings of the package"
        );
        assert_eq!(report.dropped, 0, "no runtime-code finding was dropped");
        assert!(report.every_drop_attributed());
        // The project's own style finding stands, and no row stands for the package's.
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].primary_location.path.as_ref(), "/p/App.tsx");
    }

    #[test]
    fn a_runtime_rule_beside_a_style_rule_keeps_the_hand_off() {
        let mut findings = vec![
            finding(
                "SC8003",
                "violation",
                at("/p/node_modules/pkg/index.js", 10, 20),
            ),
            finding(
                "SC2001",
                "violation",
                at("/p/node_modules/pkg/index.js", 12, 18),
            ),
        ];
        let entries = vec![PackageEntries {
            function: at("/p/node_modules/pkg/index.js", 0, 50),
            sites: vec![PackageEntrySite {
                location: at("/p/App.tsx", 100, 110),
                kind: DependencyEntry::Call,
            }],
        }];
        let report = hand_off(
            &mut findings,
            &facts(),
            &program(entries, Vec::new()),
            false,
        );
        assert_eq!(
            (report.style_dropped, report.dropped, report.at_entry),
            (1, 1, 1)
        );
        assert!(report.every_drop_attributed());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].id, "SC2001");
        assert_eq!(findings[0].kind, "uncertifiable");
    }

    #[test]
    fn with_no_entry_there_is_one_row_per_package_and_rule_at_the_first_import() {
        use solid_facts::resolution::{AttestedImport, AttestedImportIndex, ImportResolution};
        let import = |start: u32| AttestedImport {
            span: solid_facts::core::Span::new(start, start + 5),
            text: "pkg".into(),
            resolution: ImportResolution::NodeModules,
            resolved_path: "/p/node_modules/pkg/index.js".into(),
            included_path: "".into(),
            symlink_path: "".into(),
            extension: ".js".into(),
            package_name: Some("pkg".into()),
            package_version: None,
            package_manifest: None,
            resolver_package_name: None,
            resolver_package_version: None,
        };
        let mut imports = AttestedImportIndex::default();
        // The order of insertion is not the order of the report: the first by path, then offset.
        imports.insert_file("/p/b.tsx", vec![import(0)]);
        imports.insert_file("/p/a.tsx", vec![import(40), import(7)]);
        let mut facts = facts();
        facts.resolved_imports = Some(imports);
        let mut findings = vec![
            finding(
                "SC1001",
                "uncertifiable",
                at("/p/node_modules/pkg/a.js", 1, 2),
            ),
            finding(
                "SC1001",
                "uncertifiable",
                at("/p/node_modules/pkg/b.js", 3, 4),
            ),
            finding(
                "SC4001",
                "uncertifiable",
                at("/p/node_modules/pkg/a.js", 5, 6),
            ),
            finding(
                "SC4001",
                "uncertifiable",
                at("/p/node_modules/pkg/c.js", 7, 8),
            ),
        ];
        let report = hand_off(
            &mut findings,
            &facts,
            &program(Vec::new(), Vec::new()),
            false,
        );
        assert_eq!((report.dropped, report.at_import), (4, 4));
        assert!(report.every_drop_attributed());
        let rows = findings
            .iter()
            .map(|finding| {
                (
                    finding.id.as_str(),
                    finding.primary_location.path.to_string(),
                    finding.primary_location.start_byte,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rows,
            vec![
                ("SC1001", "/p/a.tsx".to_owned(), 7),
                ("SC4001", "/p/a.tsx".to_owned(), 7)
            ],
            "one row per rule, at the package's first import"
        );
    }

    #[test]
    fn a_drop_with_no_replacement_is_not_attributed() {
        let report = DropReport {
            dropped: 1,
            at_import: 1,
            attributed: vec![(at("/p/node_modules/pkg/index.js", 1, 2), Vec::new())],
            ..DropReport::default()
        };
        assert!(!report.every_drop_attributed());
        let counted_without_a_row = DropReport {
            dropped: 2,
            at_import: 1,
            attributed: vec![(
                at("/p/node_modules/pkg/index.js", 1, 2),
                vec![at("/p/a.tsx", 0, 0)],
            )],
            ..DropReport::default()
        };
        assert!(
            !counted_without_a_row.every_drop_attributed(),
            "a dropped finding the report never saw is a drop with no attribution"
        );
    }

    #[test]
    fn a_reference_no_exact_call_decides_is_a_site_of_every_finding_of_its_package() {
        let mut findings = vec![finding(
            "SC2001",
            "violation",
            at("/p/node_modules/pkg/other.js", 10, 20),
        )];
        let references = vec![PackageReferences {
            root: "/p/node_modules/pkg".into(),
            sites: vec![PackageEntrySite {
                location: at("/p/App.tsx", 7, 9),
                kind: DependencyEntry::Reference,
            }],
        }];
        let report = hand_off(
            &mut findings,
            &facts(),
            &program(Vec::new(), references),
            false,
        );
        assert!(report.every_drop_attributed());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].primary_location.start_byte, 7);
        assert_eq!(findings[0].kind, "uncertifiable");
    }
}
