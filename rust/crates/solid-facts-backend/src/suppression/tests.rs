use std::collections::{BTreeSet, HashMap};

use solid_facts::ast::extract;
use solid_reactive_ir::{Finding, RuleMetadata};

use super::*;

const PATH: &str = "/project/App.tsx";

fn finding(rule: &'static str, code: &'static str, kind: &str, at: usize) -> Finding {
    let mut finding = Finding::new(
        RuleMetadata {
            code,
            name: rule,
            severity: "error",
            uncertifiable: kind == "uncertifiable",
            default_enabled: true,
            configurable: true,
            presets: &[],
        },
        "synthetic".into(),
        typefacts::Location {
            path: PATH.into(),
            start_byte: at as u64,
            end_byte: at as u64 + 1,
        },
    );
    finding.kind = kind.into();
    finding
}

/// The ids of the findings that survive, for a finding of `rule` at the first
/// `X` that follows each marker in `source`.
fn surviving(source: &str, host: &[&str], findings: Vec<Finding>) -> (Vec<String>, Vec<String>) {
    let ast = extract(PATH, source).unwrap();
    let host = host
        .iter()
        .map(|rule| (*rule).to_owned())
        .collect::<BTreeSet<_>>();
    let mut unknown = Vec::new();
    let directives = compile(
        dialect::default_dialect(),
        &host,
        PATH,
        source,
        &ast,
        &mut unknown,
    );
    let mut by_path = HashMap::new();
    by_path.extend(directives.map(|directives| (PATH, directives)));
    let mut findings = findings;
    retain_unsuppressed(&by_path, &mut findings);
    (
        findings
            .iter()
            .map(|finding| format!("{}@{}", finding.id, finding.primary_location.start_byte))
            .collect(),
        unknown_name_findings(&unknown)
            .iter()
            .map(|finding| finding.message.clone())
            .collect(),
    )
}

fn strict(at: usize) -> Finding {
    finding("strict-read-untracked", "SC1001", "violation", at)
}

fn at(source: &str, needle: &str) -> usize {
    source.find(needle).unwrap_or_else(|| panic!("{needle}"))
}

#[test]
fn next_line_and_line_comments_cover_their_own_line_in_both_families() {
    let source = "\
// eslint-disable-next-line solid-checker/strict-read-untracked
const a = X1;
const b = X2; // eslint-disable-line solid-checker/strict-read-untracked
// solid-checker-disable-next-line strict-read-untracked
const c = X3;
const d = X4; // solid-checker-disable-line solid-checker/SC1001
const e = X5;
";
    let findings = ["X1", "X2", "X3", "X4", "X5"]
        .map(|marker| strict(at(source, marker)))
        .to_vec();
    let (left, notices) = surviving(source, &[], findings);
    assert_eq!(left, [format!("SC1001@{}", at(source, "X5"))]);
    assert!(notices.is_empty());
}

#[test]
fn a_block_disable_runs_until_its_enable_and_enable_names_its_rule() {
    let source = "\
const before = X0;
/* eslint-disable solid-checker/strict-read-untracked, solid-checker/uncalled-accessor */
const a = X1;
/* eslint-enable solid-checker/uncalled-accessor */
const b = X2;
/* eslint-enable */
const c = X3;
";
    let accessor = |marker: &str| {
        finding(
            "uncalled-accessor",
            "SC1005",
            "violation",
            at(source, marker),
        )
    };
    let findings = vec![
        strict(at(source, "X0")),
        strict(at(source, "X1")),
        accessor("X1"),
        strict(at(source, "X2")),
        accessor("X2"),
        strict(at(source, "X3")),
    ];
    let (left, _) = surviving(source, &[], findings);
    assert_eq!(
        left,
        [
            format!("SC1001@{}", at(source, "X0")),
            format!("SC1005@{}", at(source, "X2")),
            format!("SC1001@{}", at(source, "X3")),
        ]
    );
}

#[test]
fn a_comment_naming_no_rule_covers_every_rule_and_foreign_names_none() {
    let source = "\
// eslint-disable-next-line
const a = X1;
// eslint-disable-next-line prefer-rest-params, @typescript-eslint/no-unused-vars
const b = X2;
// eslint-disable-next-line solid/no-single-arg-create-effect, solid/some-other-plugin-rule
const c = X3;
/* solid-checker-disable */
const d = X4;
";
    let findings = ["X1", "X2", "X3", "X4"]
        .map(|marker| strict(at(source, marker)))
        .to_vec();
    let (left, notices) = surviving(source, &[], findings);
    // X2 and X3 name only other plugins' rules or upstream rules without a
    // checker rule: they suppress nothing and are not unknown names.
    assert_eq!(
        left,
        [
            format!("SC1001@{}", at(source, "X2")),
            format!("SC1001@{}", at(source, "X3"))
        ]
    );
    assert!(notices.is_empty(), "{notices:?}");
}

#[test]
fn names_resolve_through_groups_aliases_codes_and_both_prefixes() {
    let source = "\
// eslint-disable-next-line solid-checker/reactivity
const a = X1;
// eslint-disable-next-line solid/reactivity
const b = X2;
// eslint-disable-next-line solid-checker/no-direct-mutation
const c = X3;
// eslint-disable-next-line solid/no-store-mutation-outside-setter
const d = X4;
// solid-checker-disable-next-line SC1007
const e = X5;
// eslint-disable-next-line solid-checker/SC1007
const f = X6;
// eslint-disable-next-line solid-checker/no-destructure
const g = X7;
";
    let handler = |marker: &str| {
        finding(
            "reactive-handler-frozen",
            "SC1007",
            "violation",
            at(source, marker),
        )
    };
    let store = |marker: &str| {
        finding(
            "no-store-mutation-outside-setter",
            "SC2003",
            "violation",
            at(source, marker),
        )
    };
    let findings = vec![
        strict(at(source, "X1")),
        handler("X2"),
        store("X3"),
        store("X4"),
        handler("X5"),
        handler("X6"),
        strict(at(source, "X7")),
    ];
    let (left, notices) = surviving(source, &[], findings);
    // The group reaches its members, not SC1003; an SC code is a
    // `solid-checker-*` spelling only, and `eslint-disable` refuses it as ESLint
    // would.
    assert_eq!(
        left,
        [
            format!("SC1007@{}", at(source, "X6")),
            format!("SC1001@{}", at(source, "X7")),
        ]
    );
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert!(notices[0].contains("solid-checker/SC1007"), "{notices:?}");
}

#[test]
fn a_former_name_of_a_merged_rule_covers_its_own_code_only() {
    let source = "\
// eslint-disable-next-line solid-checker/resolve-in-tracked-scope
const a = X1;
// eslint-disable-next-line solid-checker/no-await-primitive-in-tracked-scope
const b = X2;
";
    let merged = |code: &'static str, marker: &str| {
        finding(
            "no-await-primitive-in-tracked-scope",
            code,
            "violation",
            at(source, marker),
        )
    };
    let findings = vec![
        merged("SC2004", "X1"),
        merged("SC2005", "X1"),
        merged("SC2004", "X2"),
        merged("SC2005", "X2"),
    ];
    let (left, _) = surviving(source, &[], findings);
    assert_eq!(left, [format!("SC2005@{}", at(source, "X1"))]);
}

#[test]
fn only_site_violations_are_ever_suppressed() {
    let source = "\
/* eslint-disable */
const a = X1;
";
    let at_x = at(source, "X1");
    let mut project = strict(at_x);
    project.subject_kind = "project".into();
    let findings = vec![
        strict(at_x),
        finding("strict-read-untracked", "SC1001", "uncertifiable", at_x),
        finding(
            "reactive-dispatch-unresolved",
            "SC9012",
            "uncertifiable",
            at_x,
        ),
        project,
    ];
    let (left, _) = surviving(source, &[], findings);
    assert_eq!(left.len(), 3, "{left:?}");
}

#[test]
fn the_host_applies_the_names_it_reports_under_and_the_checker_skips_them() {
    let source = "\
// eslint-disable-next-line solid-checker/strict-read-untracked
const a = X1;
// eslint-disable-next-line solid-checker/uncalled-accessor
const b = X2;
// solid-checker-disable-next-line strict-read-untracked
const c = X3;
// eslint-disable-next-line solid-checker/reactivity
const d = X4;
// eslint-disable-next-line solid-checker/strict-read-untracked
// the next line of a comment is the line after it, comment or not
const e = X5;
";
    let accessor = |marker: &str| {
        finding(
            "uncalled-accessor",
            "SC1005",
            "violation",
            at(source, marker),
        )
    };
    let findings = vec![
        strict(at(source, "X1")),
        accessor("X2"),
        strict(at(source, "X3")),
        strict(at(source, "X4")),
        strict(at(source, "X5")),
    ];
    let (left, _) = surviving(source, &["strict-read-untracked"], findings);
    // X1: ESLint's to suppress, so still here. X2: not host-applied, so the
    // checker suppresses. X3: a `solid-checker-*` comment is always the
    // checker's. X4: the group is the checker's, ESLint cannot apply it. X5:
    // the comment's line is not the next one.
    assert_eq!(
        left,
        [
            format!("SC1001@{}", at(source, "X1")),
            format!("SC1001@{}", at(source, "X5")),
        ]
    );
}

#[test]
fn a_line_comment_must_stay_on_one_line_and_next_line_counts_from_its_end() {
    let source = "\
/* eslint-disable-line solid-checker/strict-read-untracked --
   spans two lines */ const a = X1;
/* eslint-disable-next-line solid-checker/strict-read-untracked --
   spans two lines */
const b = X2;
const c = X3;
";
    let findings = ["X1", "X2", "X3"]
        .map(|marker| strict(at(source, marker)))
        .to_vec();
    let (left, _) = surviving(source, &[], findings);
    assert_eq!(
        left,
        [
            format!("SC1001@{}", at(source, "X1")),
            format!("SC1001@{}", at(source, "X3"))
        ]
    );
}

/// ADR 0440: SC9005 and SC9012 are obligations, so a comment naming one of them
/// (by rule name, code, or a retired name or code) is accepted without a notice
/// and removes nothing, whatever the comment's range.
#[test]
fn a_comment_naming_an_obligation_or_a_retired_identity_suppresses_nothing_and_is_not_noticed() {
    let source = "\
/* eslint-disable solid-checker/package-behavior-unavailable, solid-checker/reactive-dispatch-unresolved */
/* eslint-disable solid-checker/package-contract-incomplete, solid-checker/package-contract-missing */
/* eslint-disable solid-checker/reactive-source-uncaptured */
/* solid-checker-disable SC9005, SC9011, SC9012, package-behavior-unavailable, reactive-dispatch-unresolved */
const a = X1;
";
    let at_x = at(source, "X1");
    let findings = vec![
        finding(
            "package-behavior-unavailable",
            "SC9005",
            "uncertifiable",
            at_x,
        ),
        finding(
            "reactive-dispatch-unresolved",
            "SC9012",
            "uncertifiable",
            at_x,
        ),
        strict(at_x),
    ];
    let (left, unknown) = surviving(source, &[], findings);
    assert_eq!(left.len(), 3, "{left:?}");
    assert!(unknown.is_empty(), "{unknown:?}");
}

#[test]
fn unknown_names_are_noticed_once_each_and_known_or_foreign_ones_are_not() {
    let source = "\
// eslint-disable-next-line solid-checker/strict-read-untrackd
// eslint-disable-next-line solid-checker/strict-read-untrackd, solid-checker/missing-ownr
// solid-checker-disable-next-line nope
// eslint-disable-next-line solid-checker/certification, solid-checker/contract-note, solid-checker/package-note
// eslint-disable-next-line solid-checker/v1/imports, solid-checker/untracked-derived-function
// eslint-disable-next-line solid/imports, solid/no-single-arg-create-effect, solid/prefer-classlist
// eslint-disable-next-line solid/not-an-upstream-rule, other/strict-read-untrackd, strict-read-untrackd
/* eslint-disable solid-checker/no-owner-cleanup, solid-checker/package-contract-missing */
/* eslint-disable solid-checker/package-contract-incomplete, solid-checker/package-behavior-unavailable */
/* eslint-disable solid-checker/reactive-dispatch-unresolved, solid-checker/reactive-source-uncaptured */
/* solid-checker-disable SC9005, SC9011, SC9012, package-behavior-unavailable, reactive-dispatch-unresolved */
";
    let ast = extract(PATH, source).unwrap();
    let mut unknown = Vec::new();
    compile(
        dialect::default_dialect(),
        &BTreeSet::new(),
        PATH,
        source,
        &ast,
        &mut unknown,
    );
    let notices = unknown_name_findings(&unknown);
    assert_eq!(
        notices
            .iter()
            .map(|finding| finding.message.as_str())
            .collect::<Vec<_>>(),
        [
            "a suppression comment names \"nope\", which is not a rule of this checker; it suppresses nothing for that name",
            "a suppression comment names \"solid-checker/missing-ownr\", which is not a rule of this checker; it suppresses nothing for that name",
            "a suppression comment names \"solid-checker/strict-read-untrackd\", which is not a rule of this checker; it suppresses nothing for that name",
        ]
    );
    let typo = notices
        .iter()
        .find(|finding| {
            finding
                .message
                .contains("solid-checker/strict-read-untrackd")
        })
        .unwrap();
    assert_eq!(typo.related_locations.len(), 1, "the second use is related");
    assert!(notices.iter().all(|finding| finding.kind == "uncertifiable"
        && finding.subject_kind == "project"
        && finding.id == "SC9016"));
}

#[test]
fn the_upstream_name_list_is_pinned() {
    assert_eq!(dialect::UPSTREAM_RULE_NAMES.len(), 35);
    let distinct = dialect::UPSTREAM_RULE_NAMES.iter().collect::<BTreeSet<_>>();
    assert_eq!(distinct.len(), 35);
    // Every upstream name a checker rule carries resolves to it, or to its
    // group; the one with a different claim has no alias (ADR 0432).
    let v2 = dialect::default_dialect();
    for (upstream, resolves) in [
        ("components-return-once", true),
        ("no-destructure", true),
        ("jsx-no-duplicate-props", true),
        ("prefer-for", true),
        ("prefer-show", true),
        ("no-write-in-pure-computation", true),
        ("no-store-mutation-outside-setter", true),
        ("valid-use-server", true),
        ("reactivity", true),
        ("no-single-arg-create-effect", false),
        ("imports", false),
    ] {
        assert!(
            dialect::UPSTREAM_RULE_NAMES.contains(&upstream),
            "{upstream}"
        );
        assert_eq!(
            matches!(checker_name(v2, upstream, false), Resolution::Matchers(_)),
            resolves,
            "{upstream}"
        );
    }
}
