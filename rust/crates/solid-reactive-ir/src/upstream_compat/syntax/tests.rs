use super::{check_file, duplicate_slot, is_lowercase_led, is_numeric_literal};

/// The compiler lowers delegated events to `el.$$event = handler`, a
/// property write where a later occurrence overwrites an earlier one —
/// the one event spelling with a single-winner slot, folded
/// case-insensitively because `to_event_name` lowercases the suffix.
#[test]
fn delegated_event_spellings_share_one_later_wins_slot() {
    assert_eq!(
        duplicate_slot("onClick", false, true),
        duplicate_slot("onclick", false, true)
    );
    assert!(duplicate_slot("onClick", false, true).is_some());
}

/// `on:evt` (bubble) and `oncapture:evt` (capture) lower to separate
/// `addEventListener` calls — both attach, nothing is overwritten, so
/// neither competes in any slot. Upstream folds all four spellings onto
/// one name; the pinned compiler proves that wrong.
#[test]
fn listener_namespace_spellings_never_collide() {
    assert_eq!(duplicate_slot("on:click", false, true), None);
    assert_eq!(duplicate_slot("oncapture:click", false, true), None);
}

/// A non-delegated event attaches one listener per occurrence; both
/// fire, so a repeated `onMouseEnter` is runtime-legal, not a dead prop.
/// Names that merely start with `on` (`once`, `only`) are events to the
/// compiler too, and non-delegated ones land here.
#[test]
fn non_delegated_events_never_collide() {
    assert_eq!(duplicate_slot("onMouseEnter", false, true), None);
    assert_eq!(duplicate_slot("once", false, true), None);
}

/// A statically string/number-valued (or bare) `on*` prop is frozen into
/// the template, where the HTML parser keeps the first occurrence — a
/// first-wins slot shared with the `attr:` spelling of the same name.
#[test]
fn static_valued_event_names_share_the_template_attribute_slot() {
    assert_eq!(
        duplicate_slot("onClick", true, true),
        Some("onclick".to_owned())
    );
    assert_eq!(
        duplicate_slot("onClick", true, true),
        duplicate_slot("attr:onclick", false, true)
    );
    // Delegated handler and template attribute are different slots:
    // the inline HTML handler and the `$$click` handler both fire.
    assert_ne!(
        duplicate_slot("onClick", true, true),
        duplicate_slot("onClick", false, true)
    );
}

#[test]
fn strips_attr_and_prop_prefixes_onto_the_shared_slot() {
    assert_eq!(
        duplicate_slot("attr:title", false, true),
        Some("title".to_owned())
    );
    assert_eq!(
        duplicate_slot("prop:value", false, true),
        Some("value".to_owned())
    );
    assert_eq!(
        duplicate_slot("attr:title", false, true),
        duplicate_slot("title", true, true)
    );
}

#[test]
fn leaves_names_that_are_not_on_prefixed_or_attr_prop_prefixed_alone() {
    assert_eq!(
        duplicate_slot("class", true, true),
        Some("class".to_owned())
    );
    assert_eq!(duplicate_slot("id", false, true), Some("id".to_owned()));
}

/// A component's props are a plain object the compiler never lowers, so
/// the slot is the key as written: repeats collide whatever the DOM
/// model would have said about them, and differently-spelled names do
/// not.
#[test]
fn component_props_compare_by_key_identity() {
    for name in ["onSave", "on:click", "oncapture:click", "onMouseEnter"] {
        assert_eq!(
            duplicate_slot(name, false, false),
            Some(name.to_owned()),
            "{name} must keep its own slot on a component"
        );
    }
    // No DOM aliasing: these are three distinct keys on a props object.
    assert_ne!(
        duplicate_slot("onClick", false, false),
        duplicate_slot("onclick", false, false)
    );
    assert_ne!(
        duplicate_slot("attr:title", false, false),
        duplicate_slot("title", false, false)
    );
    // And the static-value distinction is DOM lowering too.
    assert_eq!(
        duplicate_slot("onClick", true, false),
        duplicate_slot("onClick", false, false)
    );
}

/// The compiler's inline branch matches `Expression::NumericLiteral`
/// nodes, so the predicate has to follow the literal *grammar*: a signed
/// or named value is a different node and lowers dynamically, while the
/// radix and separator forms are literals the compiler freezes.
#[test]
fn numeric_literal_lexemes_match_the_compiler_node_kind() {
    for literal in [
        "0", "1", "42", "1_000", "0x10", "0X1f", "0b1010", "0o17", "0755", "1.5", "1.", ".5",
        "1e3", "1E-3", "1.5e+10",
    ] {
        assert!(is_numeric_literal(literal), "{literal} is a NumericLiteral");
    }
    for other in [
        "-1",
        "+1",
        "NaN",
        "inf",
        "Infinity",
        "-Infinity",
        "1n",
        "0x",
        "_1",
        "1__0",
        "1_",
        ".",
        "",
        "1e",
        "0xg",
        "true",
    ] {
        assert!(
            !is_numeric_literal(other),
            "{other} is not a NumericLiteral"
        );
    }
}
#[test]
fn classifies_element_names_by_leading_case() {
    assert!(is_lowercase_led("div"));
    assert!(!is_lowercase_led("Component"));
    assert!(!is_lowercase_led("Foo.Bar"));
}

/// Solid 2.0 has no lowercase `innerhtml` content slot. rc.13 types only the
/// camel-case name (TS2322 on the lowercase spelling), and the runtime writes
/// an unknown lowercase prop with `setAttribute`, so it competes with nothing.
/// The camel-case control beside it must still report.
#[test]
fn lowercase_innerhtml_is_not_a_content_source() {
    let lowercase = violations_for(r#"export const Case = () => <div innerhtml="x">child</div>;"#);
    assert_eq!(lowercase.len(), 0, "lowercase innerhtml is an attribute");

    let camel = violations_for(r#"export const Case = () => <div innerHTML="x">child</div>;"#);
    assert_eq!(
        camel.len(),
        1,
        "camel-case innerHTML competes with children"
    );
    assert_eq!(camel[0].rule, "jsx-no-duplicate-props");
}

fn violations_for(source: &str) -> Vec<crate::StaticViolation> {
    use solid_facts::{
        FileFacts, ast,
        compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap},
        core::Generation,
    };
    let ast = ast::extract("app.tsx", source).unwrap();
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
    let file = FileFacts::new(Generation::new(1).unwrap(), source, ast, compiler).unwrap();
    let mut violations = Vec::new();
    check_file(&file, &mut violations);
    violations
}
