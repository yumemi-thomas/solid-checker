//! Ports `retained_contributions_test.go`.

use super::{backend_with, location};
use crate::closure::{DemandClosure, DemandInputGroup};
use crate::context::Context;
use crate::contributions::{
    ContributionRef, RetainedContribution, RetainedContributionStore, prepare_retained_contribution,
};
use crate::demands::DemandGroup;
use crate::model::{SemanticDemandRunResult, blank_entity};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use typefacts::v3::EntityDemand;
use typefacts::{
    ArrayShape, Callability, ConstantValue, ConstantValueKind, Constructability,
    PrimitiveValueDomain, RuntimeValueDomain, SymbolClaim, TupleShape, TypeDescriptor,
};

#[test]
fn prepared_contribution_takes_and_compacts_entity_backing() {
    let location = location("/project/source.ts", 1, 2);
    let domain = RuntimeValueDomain::new(true, true, false, false);
    let primitive = PrimitiveValueDomain::new(
        true, false, true, false, false, false, false, false, false, false,
    );
    let call_result = RuntimeValueDomain::new(false, false, true, false);
    let constant = ConstantValue {
        kind: ConstantValueKind::String,
        string: "constant".into(),
        number: 0.0,
    };
    // `TupleShape` packs its scalars and has no public constructor; its wire
    // form is the public way to make one.
    let tuple: TupleShape = ciborium::Value::Map(vec![
        ("fixedLength".into(), 2.into()),
        ("elementZero".into(), "callable".into()),
        ("elementZeroMinParameters".into(), 2.into()),
    ])
    .deserialized()
    .unwrap();
    let descriptor = Arc::new(TypeDescriptor {
        text: "Value".into(),
        origin_module: "".into(),
        alias_declarations: Arc::from(Vec::new()),
    });
    let mut first = blank_entity(location.clone());
    first.symbol = "symbol".into();
    first.symbol_claim = SymbolClaim::Covering;
    let mut second = blank_entity(location.clone());
    second.type_descriptor = Some(Arc::clone(&descriptor));
    second.callability = Some(Callability::Callable);
    second.constructability = Some(Constructability::Constructable);
    second.runtime_value_domain = Some(domain);
    second.primitive_value_domain = primitive;
    second.call_result_domain = Some(call_result);
    second.constant_value = Some(constant.clone());
    second.array_shape = Some(ArrayShape::Array);
    second.tuple_shape = Some(tuple);
    second.library_types = Some(Arc::new(vec!["Date".into()]));

    let contribution = prepare_retained_contribution(
        &location.path,
        &[
            EntityDemand {
                location: location.clone(),
                symbol: true,
                ..EntityDemand::default()
            },
            EntityDemand {
                location: location.clone(),
                type_descriptor: true,
                callability: true,
                constructability: true,
                runtime_value_domain: true,
                primitive_value_domain: true,
                call_result_domain: true,
                constant_value: true,
                array_shape: true,
                tuple_shape: true,
                library_types: true,
                ..EntityDemand::default()
            },
        ],
        SemanticDemandRunResult {
            entities: vec![first, second],
            durable: true,
        },
    )
    .unwrap();
    let contribution = contribution.borrow();
    let entities = contribution.entities.as_ref().unwrap();
    assert_eq!(entities.len(), 1, "compacted entities");
    let entity = &entities[0];
    assert!(
        &*entity.symbol == "symbol"
            && entity.symbol_claim == SymbolClaim::Covering
            && entity.callability == Some(Callability::Callable)
            && entity.constructability == Some(Constructability::Constructable)
            && entity.runtime_value_domain == Some(domain)
            && entity.primitive_value_domain == primitive
            && entity.call_result_domain == Some(call_result)
            && entity.constant_value == Some(constant)
            && entity.array_shape == Some(ArrayShape::Array)
            && entity.tuple_shape == Some(tuple)
            && entity.library_types.as_deref() == Some(&vec!["Date".into()])
            && entity
                .type_descriptor
                .as_ref()
                .is_some_and(|taken| Arc::ptr_eq(taken, &descriptor)),
        "compacted entity lost demand fields: {entity:?}"
    );
}

fn store_test_contribution(durable: bool) -> ContributionRef {
    Rc::new(RefCell::new(RetainedContribution {
        entities: Some(vec![blank_entity(location("", 0, 0))].into()),
        durable,
    }))
}

fn group_of(path: &str, contribution: &ContributionRef) -> DemandGroup {
    DemandGroup {
        path: path.to_owned(),
        contribution: Some(Rc::clone(contribution)),
        ..DemandGroup::default()
    }
}

/// A table publishes the contributions that hold only binder-level ids.
#[test]
fn store_commit_publishes_only_durable_contributions() {
    const PATH_A: &str = "/project/a.ts";
    const PATH_B: &str = "/project/b.ts";
    let mut store = RetainedContributionStore::default();
    let durable = store_test_contribution(true);
    let volatile = store_test_contribution(false);
    store.commit(&[group_of(PATH_A, &durable), group_of(PATH_B, &volatile)]);
    assert!(Rc::ptr_eq(&store.get(PATH_A).unwrap(), &durable));
    assert!(
        store.get(PATH_B).is_none(),
        "a counter-id contribution was published"
    );

    store.reset();
    assert!(store.get(PATH_A).is_none());
}

#[test]
fn contribution_commit_is_transactional() {
    const PATH: &str = "/project/source.ts";
    const OLD_PATH: &str = "/project/old.ts";
    let location = location(PATH, 13, 18);
    let mut backend = backend_with(&[(PATH, "export const value = 1\n")]);
    backend.fail_semantic_once = Some("checker unavailable".to_owned());
    let (semantic_calls, reference_calls) = (
        Arc::clone(&backend.semantic_calls),
        Arc::clone(&backend.reference_calls),
    );
    let mut closure = DemandClosure::new(Box::new(backend), None);

    closure
        .retained
        .add(OLD_PATH, store_test_contribution(true));
    let groups = [DemandInputGroup {
        path: PATH.to_owned(),
        demands: vec![EntityDemand {
            location: location.clone(),
            symbol: true,
            references: true,
            structural_accessor: true,
            ..EntityDemand::default()
        }],
    }];

    assert!(
        closure
            .demand_table_for_groups(&Context::new(), 1, &groups, &[])
            .is_err(),
        "failed materialization unexpectedly succeeded"
    );
    assert_eq!(
        (
            *semantic_calls.lock().unwrap(),
            *reference_calls.lock().unwrap()
        ),
        (1, 0),
        "backend calls after failure (semantic, references)"
    );
    // Building a table starts by discarding what an earlier table left, so a
    // failure publishes nothing and leaves nothing a retry could reuse.
    assert!(
        closure.retained.get(OLD_PATH).is_none() && closure.retained.get(PATH).is_none(),
        "failed materialization published or kept contributions"
    );

    let table = closure
        .demand_table_for_groups(&Context::new(), 1, &groups, &[])
        .expect("retry materialization");
    let typefacts::producer::EntityRows::Runs(runs) = &table.entities else {
        panic!("entity rows are not per-file runs");
    };
    assert!(
        runs.len() == 1 && runs[0].len() == 1,
        "retry table, want one entity"
    );
    assert_eq!(
        (
            *semantic_calls.lock().unwrap(),
            *reference_calls.lock().unwrap()
        ),
        (2, 0),
        "backend calls after retry (semantic, references)"
    );
    assert!(
        closure.retained.get(OLD_PATH).is_none() && closure.retained.get(PATH).is_some(),
        "successful retry did not commit the new contribution set"
    );
}
