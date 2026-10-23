//! Ports `entity_projection_test.go`: the canonical wire iteration of a
//! table's entity rows, `FactTable.RangeEntities` there and
//! `EntityRows::for_each` here.
//!
//! Deviation: Go's table can hold both public rows and per-file runs, with the
//! runs taking precedence; `EntityRows` is one or the other, so the
//! "retained precedence" case is the runs variant alone.

use super::location;
use crate::closure::DemandClosure;
use crate::fake::FakeBackend;
use std::sync::Arc;
use typefacts::producer::{EntityRows, ProducerTable};
use typefacts::{
    ArrayShape, Callability, ConstantValue, ConstantValueKind, Constructability, Declaration,
    EntityFact, PrimitiveValueDomain, ReferenceSpace, RuntimeBindingKind, RuntimeValueDomain,
    SymbolClaim, TypeDescriptor,
};

/// An entity with a meaningful value in every field. Each call allocates fresh
/// nested storage, so a second call is an independent expected value, not an
/// alias.
fn projection_row(path: &str, start: u64) -> EntityFact {
    let mut row = crate::model::blank_entity(location(path, start, start + 3));
    row.symbol = format!("s:{path}").into();
    row.symbol_claim = SymbolClaim::Partial;
    row.type_descriptor = Some(Arc::new(TypeDescriptor {
        text: "T".into(),
        origin_module: "m".into(),
        alias_declarations: Arc::from(vec![Declaration {
            name: "A".into(),
            kind: "alias".into(),
            location: location(path, 0, 0),
        }]),
    }));
    row.callability = Some(Callability::Callable);
    row.runtime_value_domain = Some(RuntimeValueDomain::new(true, false, false, false));
    row.primitive_value_domain = PrimitiveValueDomain::new(
        true, false, true, false, false, false, false, false, false, false,
    );
    row.constructability = Some(Constructability::Constructable);
    row.call_result_domain = Some(RuntimeValueDomain::new(false, true, false, false));
    row.constant_value = Some(ConstantValue {
        kind: ConstantValueKind::String,
        string: "c".into(),
        number: 0.0,
    });
    row.array_shape = Some(ArrayShape::Array);
    row.library_types = Some(Arc::new(vec!["Promise".into()]));
    row.reference_space = Some(ReferenceSpace::Value);
    row.runtime_identity = format!("r:{path}").into();
    row.runtime_binding_kind = Some(RuntimeBindingKind::Callable);
    row
}

fn projected(rows: &EntityRows) -> Vec<EntityFact> {
    let mut visited = Vec::new();
    rows.for_each(|entity| visited.push(entity.clone()));
    visited
}

#[test]
fn range_entities_is_the_canonical_wire_iteration() {
    let public = EntityRows::Flat(vec![projection_row("/a", 1), projection_row("/b", 2)]);
    let retained = EntityRows::Runs(vec![
        Arc::from(vec![projection_row("/a", 1)]),
        Arc::from(vec![projection_row("/b", 2), projection_row("/c", 3)]),
    ]);
    for (name, rows, want) in [
        (
            "public fallback",
            public,
            vec![projection_row("/a", 1), projection_row("/b", 2)],
        ),
        (
            "retained runs",
            retained,
            vec![
                projection_row("/a", 1),
                projection_row("/b", 2),
                projection_row("/c", 3),
            ],
        ),
    ] {
        let got = projected(&rows);
        assert_eq!(got, want, "{name}: projected rows");
        assert_eq!(got.len(), rows.len(), "{name}: row count");
    }
}

#[test]
fn range_entities_empty_shapes() {
    // Non-empty runs that are empty are the post-release shape.
    for (name, rows) in [
        ("zero table", EntityRows::default()),
        ("empty runs", EntityRows::Runs(Vec::new())),
    ] {
        assert!(
            projected(&rows).is_empty() && rows.is_empty(),
            "{name}: projected {:?}",
            projected(&rows)
        );
    }
}

/// Runs the closure's own transport release over a populated retained table.
#[test]
fn range_entities_owner_release_leaves_nothing_visitable() {
    let mut closure = DemandClosure::new(Box::new(FakeBackend::default()), None);
    let mut table = ProducerTable::new(1, "project");
    table.entities = EntityRows::Runs(vec![Arc::from(vec![projection_row("/a", 1)])]);
    assert_eq!(
        projected(&table.entities).len(),
        1,
        "the populated table is visitable"
    );
    closure.table = Some(table);

    closure.release_transport_rows();

    let released = closure.table().unwrap();
    assert!(
        projected(&released.entities).is_empty()
            && matches!(released.entities, EntityRows::Runs(_))
            && released.entities.is_empty(),
        "after release: projected {:?}",
        projected(&released.entities)
    );
}

/// The bounded claim the visitor makes: it changes nothing itself, and a
/// callback reassigning its own value changes nothing either. Nested storage
/// is shared, as documented, so a callback writing through a pointer would be a
/// contract violation; this test asserts the sharing rather than exercising it.
#[test]
fn range_entities_does_not_mutate_the_table() {
    let run: Arc<[EntityFact]> = Arc::from(vec![projection_row("/a", 1), projection_row("/b", 2)]);
    let rows = EntityRows::Runs(vec![Arc::clone(&run)]);
    let want = vec![projection_row("/a", 1), projection_row("/b", 2)];
    let mut index = 0;
    rows.for_each(|entity| {
        assert!(
            match (&entity.type_descriptor, &run[index].type_descriptor) {
                (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                _ => false,
            },
            "row {index}: nested storage is not the table's own"
        );
        index += 1;
        let mut copy = entity.clone();
        copy.location.path = "/rewritten".into();
        copy.symbol = "".into();
        copy.type_descriptor = None;
        copy.primitive_value_domain = PrimitiveValueDomain::default();
        copy.library_types = None;
        drop(copy);
    });
    assert_eq!(&*run, want.as_slice(), "the table changed under projection");
    assert_eq!(
        projected(&rows),
        want,
        "a second projection differs from the expected rows"
    );
}
