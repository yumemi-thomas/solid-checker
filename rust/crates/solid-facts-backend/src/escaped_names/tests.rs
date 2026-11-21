use solid_facts::{
    FileFacts, ProjectFacts, TypeScriptTable,
    compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap},
    core::Generation,
};
use solid_reactive_ir::{Finding, RuleMetadata};
use typefacts::Location;

use super::*;

/// The project of `files` (`(path, source)`, an `@` in a source a backslash), with `roots` its dependency source roots.
fn project(files: &[(&str, &str)], roots: &[&str]) -> ProjectFacts {
    let files = files
        .iter()
        .map(|(path, source)| {
            let source = source.replace('@', "\\");
            let ast = solid_facts::ast::extract(*path, &source).expect("the source parses");
            let compiler = ExecutionMap {
                compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
                source_hash: ast.source.hash.clone(),
                semantic_model: Default::default(),
                tracked_regions: vec![],
                untracked_regions: vec![],
                discarded_regions: vec![],
                ownership_regions: vec![],
                callback_roles: vec![],
                jsx_operations: vec![],
            };
            FileFacts::new(Generation::new(1).unwrap(), source.as_str(), ast, compiler).unwrap()
        })
        .collect();
    ProjectFacts {
        generation: Generation::new(1).unwrap(),
        project_id: "/app/tsconfig.json".to_owned(),
        files,
        typescript: TypeScriptTable::from_parts(
            3,
            1,
            "/app/tsconfig.json".to_owned(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        typescript_changes: None,
        resolved_imports: None,
        runtime_resolutions: None,
        runtime_symbol_redirects: Default::default(),
        dependency_source_roots: roots.iter().map(|root| (*root).to_owned()).collect(),
        dependency_source_call_errors: Vec::new(),
    }
}

const OBLIGATION: RuleMetadata = RuleMetadata {
    code: "SC9012",
    name: OBLIGATION_RULE,
    severity: "warning",
    uncertifiable: true,
    default_enabled: true,
    configurable: false,
    presets: &[],
};

const VIOLATION: RuleMetadata = RuleMetadata {
    code: "SC1001",
    name: "strict-read-untracked",
    severity: "warning",
    uncertifiable: false,
    default_enabled: true,
    configurable: true,
    presets: &[],
};

fn finding(metadata: RuleMetadata, path: &str, start: u64, end: u64) -> Finding {
    Finding::new(
        metadata,
        "a finding".to_owned(),
        Location {
            path: path.into(),
            start_byte: start,
            end_byte: end,
        },
    )
}

fn violation_at(path: &str) -> Finding {
    finding(VIOLATION, path, 4, 9)
}

#[test]
fn a_program_that_writes_no_escape_is_untouched() {
    let facts = project(
        &[
            (
                "/app/App.tsx",
                "const x = a.b; f('click', { k: 1 }, 'a@nb');",
            ),
            ("/app/other.ts", "export const y = 1;"),
        ],
        &[],
    );
    let mut findings = vec![violation_at("/app/App.tsx")];
    let before = findings.clone();
    assert_eq!(
        veto(&facts, Some(OBLIGATION), &mut findings),
        Veto::default()
    );
    assert_eq!(findings, before);
}

#[test]
fn an_identifier_written_with_an_escape_demotes_every_violation_and_notices_every_file() {
    let facts = project(
        &[
            ("/app/App.tsx", "const v = ev@u0061l('1');"),
            ("/app/other.ts", "export const y = 1;"),
        ],
        &[],
    );
    let mut findings = vec![violation_at("/app/other.ts"), violation_at("/app/App.tsx")];
    let report = veto(&facts, Some(OBLIGATION), &mut findings);
    assert_eq!(
        report,
        Veto {
            demoted: 2,
            noticed: 2
        }
    );
    // Both violations stand where they were, uncertifiable, under the named blocker.
    for found in &findings[..2] {
        assert_eq!(found.kind, "uncertifiable");
        assert_eq!(found.id, "SC1001");
        assert_eq!(found.analysis_context, ESCAPED_NAME_UNRESOLVED);
        assert!(found.message.contains("writes a name with an escape"));
    }
    // One notice per project file, spanning it: no site of the file stands outside it.
    let notices = &findings[2..];
    assert_eq!(notices.len(), 2);
    for notice in notices {
        assert_eq!(notice.id, "SC9012");
        assert_eq!(notice.rule, OBLIGATION_RULE);
        assert_eq!(notice.kind, "uncertifiable");
        assert_eq!(notice.subject_kind, "file");
        assert_eq!(notice.analysis_context, ESCAPED_NAME_UNRESOLVED);
        assert_eq!(notice.primary_location.start_byte, 0);
        let file = facts
            .files
            .iter()
            .find(|file| file.path.as_str() == notice.primary_location.path.as_ref())
            .expect("a notice stands in a file of the program");
        assert_eq!(notice.primary_location.end_byte, file.source.len() as u64);
        assert!(notice.message.contains("`eval`"), "{}", notice.message);
        assert!(notice.message.contains("ev\\u0061l"), "{}", notice.message);
        assert_eq!(notice.evidence.len(), 1);
    }
}

#[test]
fn a_string_that_is_one_word_is_a_name_and_a_plain_string_is_not() {
    for source in [
        "el.addEventListener('c@u006cick', f);",
        "const o = { 'ke@u0079': 1 };",
        "o['__pr@x6fto__'] = 1;",
    ] {
        let facts = project(&[("/app/App.tsx", source)], &[]);
        let mut findings = vec![violation_at("/app/App.tsx")];
        assert_eq!(
            veto(&facts, Some(OBLIGATION), &mut findings),
            Veto {
                demoted: 1,
                noticed: 1
            },
            "{source}"
        );
    }
    for source in [
        "f('a@nb', '@u00e9', '@t', { '@n': 1 }, `@r`);",
        "const t = tag`c@u006cick`;",
    ] {
        let facts = project(&[("/app/App.tsx", source)], &[]);
        let mut findings = vec![violation_at("/app/App.tsx")];
        assert_eq!(
            veto(&facts, Some(OBLIGATION), &mut findings),
            Veto::default(),
            "{source}"
        );
        assert_eq!(findings[0].kind, "violation", "{source}");
    }
}

#[test]
fn a_name_in_a_dependency_taints_the_project_and_is_not_noticed_itself() {
    let facts = project(
        &[
            ("/app/App.tsx", "import { f } from 'lib'; f();"),
            (
                "/app/node_modules/lib/index.js",
                "export const f = () => ev@u0061l('1');",
            ),
        ],
        &["/app/node_modules/lib"],
    );
    let mut findings = vec![violation_at("/app/App.tsx")];
    let report = veto(&facts, Some(OBLIGATION), &mut findings);
    // The project's violation is demoted; the project's file has the notice and the package's has none (a package's findings are not the project's).
    assert_eq!(
        report,
        Veto {
            demoted: 1,
            noticed: 1
        }
    );
    assert_eq!(findings[0].kind, "uncertifiable");
    assert_eq!(findings[1].primary_location.path.as_ref(), "/app/App.tsx");
    assert_eq!(
        findings[1].evidence[0]
            .location
            .as_ref()
            .unwrap()
            .path
            .as_ref(),
        "/app/node_modules/lib/index.js"
    );
}

#[test]
fn a_style_rule_and_a_proven_compiler_refusal_stand() {
    let facts = project(&[("/app/App.tsx", "const v = ev@u0061l('1');")], &[]);
    let style = RuleMetadata {
        code: "SC8003",
        name: "no-duplicate-props",
        severity: "error",
        uncertifiable: false,
        default_enabled: true,
        configurable: true,
        presets: &[],
    };
    let refusal = RuleMetadata {
        code: crate::dialect::COMPILER_REFUSED_CODE,
        name: crate::dialect::COMPILER_REFUSED_RULE,
        severity: "error",
        uncertifiable: false,
        default_enabled: true,
        configurable: true,
        presets: &[],
    };
    let mut findings = vec![
        finding(style, "/app/App.tsx", 0, 5),
        finding(refusal, "/app/App.tsx", 0, 0),
        violation_at("/app/App.tsx"),
    ];
    let report = veto(&facts, Some(OBLIGATION), &mut findings);
    assert_eq!(report.demoted, 1);
    assert_eq!(findings[0].kind, "violation");
    assert_eq!(findings[1].kind, "violation");
    assert_eq!(findings[2].kind, "uncertifiable");
}

#[test]
fn an_obligation_already_standing_keeps_its_context_as_the_detail() {
    let facts = project(&[("/app/App.tsx", "const v = ev@u0061l('1');")], &[]);
    let mut standing = violation_at("/app/App.tsx");
    standing.analysis_context = "owner-unproven".into();
    let mut findings = vec![standing];
    veto(&facts, Some(OBLIGATION), &mut findings);
    assert_eq!(
        findings[0].analysis_context,
        format!("{ESCAPED_NAME_UNRESOLVED}:owner-unproven")
    );
}

#[test]
fn a_catalog_without_the_obligation_still_demotes_and_adds_no_notice() {
    let facts = project(&[("/app/App.tsx", "const v = ev@u0061l('1');")], &[]);
    let mut findings = vec![violation_at("/app/App.tsx")];
    let report = veto(&facts, None, &mut findings);
    assert_eq!(
        report,
        Veto {
            demoted: 1,
            noticed: 0
        }
    );
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].kind, "uncertifiable");
}
