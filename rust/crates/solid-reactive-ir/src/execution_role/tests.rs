use std::sync::Arc;

use solid_facts::{
    compiler::{
        COMPILER_FACTS_PROTOCOL, CallbackRole, CallbackRoleKind, ExecutionMap, ExecutionRegion,
        RegionReason,
    },
    core::{Generation, SourceHash, SourcePath, Span},
};

use super::{execution_role, format_read_context, missing_jsx_census};
use crate::ExecutionRole;

fn execution_map() -> ExecutionMap {
    ExecutionMap {
        compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
        source_hash: SourceHash::of("value"),
        semantic_model: Default::default(),
        tracked_regions: vec![],
        untracked_regions: vec![],
        discarded_regions: vec![],
        ownership_regions: vec![],
        callback_roles: vec![],
        jsx_operations: vec![],
    }
}

#[test]
fn compiler_execution_distinguishes_explicit_untracked_from_unknown() {
    let mut facts = execution_map();
    assert_eq!(
        execution_role(&facts, Span::new(0, 5), &[]),
        ExecutionRole::Unknown
    );
    facts.untracked_regions.push(ExecutionRegion {
        span: Span::new(0, 5),
        reason: RegionReason::JsxChild,
    });
    assert_eq!(
        execution_role(&facts, Span::new(0, 5), &[]),
        ExecutionRole::UntrackedRendering
    );
}

#[test]
fn smallest_compiler_region_wins_across_execution_fact_categories() {
    let mut facts = execution_map();
    facts.untracked_regions.push(ExecutionRegion {
        span: Span::new(0, 100),
        reason: RegionReason::JsxChild,
    });
    facts.callback_roles.push(CallbackRole {
        span: Span::new(40, 60),
        role: CallbackRoleKind::EventHandler,
    });

    assert_eq!(
        execution_role(&facts, Span::new(50, 51), &[]),
        ExecutionRole::EventCallback
    );
}

/// One file with a real AST and a hand-built census.
fn file(source: &str, compiler: ExecutionMap) -> solid_facts::FileFacts {
    const PATH: &str = "app.tsx";
    solid_facts::FileFacts {
        generation: Generation::new(1).unwrap(),
        path: SourcePath::new(PATH).unwrap(),
        source_hash: SourceHash::of(source),
        source: Arc::from(source),
        ast: Arc::new(solid_facts::ast::extract(PATH, source).unwrap()),
        compiler: Arc::new(compiler),
    }
}

/// The byte span of the first occurrence of `needle` in `source`.
fn span_of(source: &str, needle: &str) -> Span {
    let start = source.find(needle).expect("needle occurs in source");
    Span::new(
        u32::try_from(start).unwrap(),
        u32::try_from(start + needle.len()).unwrap(),
    )
}

/// The spread arm of the source-level JSX-region lookup, which no fixture
/// reaches: both census-gap fixtures exercise the child and attribute arms,
/// because the shapes the pinned producers decline to census are a dropped
/// `<head>` and a void element's children rather than a spread.
///
/// The census entry over the sibling child is what makes this the spread
/// arm rather than "nothing was censused at all": the element *is* covered,
/// and the read is still uncertifiable because the narrowest region
/// containing it is the spread container, which no entry touches.
#[test]
fn census_gap_is_detected_through_a_spread_container() {
    let source = "const view = <div {...props}>{label()}</div>;\n";
    let read = span_of(source, "props");
    let mut census = execution_map();
    census.source_hash = SourceHash::of(source);
    census.tracked_regions.push(ExecutionRegion {
        span: span_of(source, "label()"),
        reason: RegionReason::JsxChild,
    });

    let facts = file(source, census.clone());
    assert!(
        missing_jsx_census(&facts, read, ExecutionRole::UntrackedRendering),
        "the spread container carries no census entry"
    );
    // Any other role was decided by a fact, so the gap cannot be what put
    // the read there.
    assert!(!missing_jsx_census(
        &facts,
        read,
        ExecutionRole::UntrackedCallback
    ));

    // A census entry that touches the spread closes the gap.
    census.untracked_regions.push(ExecutionRegion {
        span: span_of(source, "{...props}"),
        reason: RegionReason::JsxChild,
    });
    assert!(!missing_jsx_census(
        &file(source, census),
        read,
        ExecutionRole::UntrackedRendering
    ));
}

/// Deletion dominates. A discarded region is not a narrower or weaker
/// untracked region: it says the code is gone, so a narrower live region
/// inside it describes lowering the deletion took with it, and no
/// width-based competition can be allowed to resurrect it.
#[test]
fn a_discarded_region_dominates_every_live_region_inside_it() {
    let mut facts = execution_map();
    facts.discarded_regions.push(ExecutionRegion {
        span: Span::new(0, 100),
        reason: RegionReason::JsxAttribute,
    });
    facts.tracked_regions.push(ExecutionRegion {
        span: Span::new(40, 60),
        reason: RegionReason::JsxChild,
    });
    facts.callback_roles.push(CallbackRole {
        span: Span::new(45, 55),
        role: CallbackRoleKind::EventHandler,
    });

    assert_eq!(
        execution_role(&facts, Span::new(50, 51), &[]),
        ExecutionRole::DiscardedRendering
    );
    // Outside it, nothing changed.
    assert_eq!(
        execution_role(&facts, Span::new(200, 201), &[]),
        ExecutionRole::Unknown
    );
}

/// A discarded region is a *fact*, so it is not a census hole. The compiler
/// reported on this JSX and said the value is deleted; turning that into an
/// uncertifiable obligation would claim something is missing when nothing
/// is.
#[test]
fn a_discarded_region_closes_the_census_gap_rather_than_being_one() {
    let source = "const view = <span children={ignored()}>{visible()}</span>;\n";
    let read = span_of(source, "ignored()");
    let mut census = execution_map();
    census.source_hash = SourceHash::of(source);
    census
        .jsx_operations
        .push(solid_facts::compiler::JsxOperation {
            span: span_of(source, "ignored()"),
            kind: "dynamic-attribute".into(),
        });
    census.discarded_regions.push(ExecutionRegion {
        span: span_of(source, "ignored()"),
        reason: RegionReason::JsxAttribute,
    });

    assert!(!missing_jsx_census(
        &file(source, census),
        read,
        ExecutionRole::UntrackedRendering
    ));
}

#[test]
fn conditional_context_does_not_repeat_the_function_name_as_a_return_kind() {
    assert_eq!(
        format_read_context("ConditionalReturn", true),
        "ConditionalReturn while evaluating a condition"
    );
    assert_eq!(
        format_read_context("", true),
        "while evaluating a condition"
    );
}
