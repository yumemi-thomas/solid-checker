//! Ports `retained_contributions_test.go`.

use super::{backend_with, location};
use crate::closure::{DemandClosure, DemandInputGroup};
use crate::context::Context;
use crate::contributions::{
    ContributionRef, PATH_MEMBERSHIP_MAP_THRESHOLD, PathMembership, RetainedContribution,
    RetainedContributionStore, prepare_retained_contribution,
};
use crate::demands::DemandGroup;
use crate::fake::symbol_id;
use crate::model::{SemanticDemandRunResult, SymbolId, blank_entity};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
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
        1,
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
            structural: vec!["".into(), "accessor".into()],
            dependencies: Vec::new(),
            durable: true,
        },
    )
    .unwrap();
    let contribution = contribution.borrow();
    let entities = contribution.entities.as_ref().unwrap();
    assert_eq!(entities.len(), 1, "compacted entities");
    assert_eq!(contribution.structural, vec![SymbolId::from("accessor")]);
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
    assert_eq!(
        contribution.descriptor_symbols,
        vec![SymbolId::from("symbol")],
        "descriptor symbols"
    );
}

fn store_test_contribution(
    dependencies: &[&str],
    descriptor_symbols: &[&str],
    durable: bool,
) -> ContributionRef {
    let entities: Vec<_> = descriptor_symbols
        .iter()
        .map(|symbol| {
            let mut entity = blank_entity(location("", 0, 0));
            entity.symbol = (*symbol).into();
            entity
        })
        .collect();
    Rc::new(RefCell::new(RetainedContribution {
        dependencies: dependencies.iter().map(|path| (*path).to_owned()).collect(),
        entities: Some(entities.into()),
        descriptor_symbols: descriptor_symbols
            .iter()
            .map(|s| SymbolId::from(*s))
            .collect(),
        durable,
        ..RetainedContribution::default()
    }))
}

fn paths(items: &[&str]) -> HashSet<String> {
    items.iter().map(|path| (*path).to_owned()).collect()
}

#[test]
fn path_membership_promotes_without_changing_set_semantics() {
    let mut membership = PathMembership::default();
    for index in 0..PATH_MEMBERSHIP_MAP_THRESHOLD + 4 {
        let path = format!("/project/member-{index:02}.ts");
        membership.add(&path);
        membership.add(&path);
    }
    assert!(
        membership.large.is_some(),
        "high-fanout membership did not promote"
    );
    membership.remove("/project/member-03.ts");
    membership.remove("/project/not-present.ts");

    let mut got = Vec::new();
    membership.range_paths(|path| got.push(path.to_owned()));
    got.sort();
    assert_eq!(got.len(), PATH_MEMBERSHIP_MAP_THRESHOLD + 3);
    assert!(
        !got.contains(&"/project/member-03.ts".to_owned()),
        "removed path remains in promoted membership"
    );
}

#[test]
fn store_discard_does_not_fan_out() {
    const SOURCE: &str = "/project/source.ts";
    const DIRECT: &str = "/project/direct.ts";
    const PEER: &str = "/project/peer.ts";
    let mut store = RetainedContributionStore::default();
    store.add(SOURCE, store_test_contribution(&[], &[], true));
    let direct = store_test_contribution(&[SOURCE], &[], true);
    let peer = store_test_contribution(&[SOURCE], &[], true);
    store.add(DIRECT, Rc::clone(&direct));
    store.add(PEER, Rc::clone(&peer));

    let discarded = paths(&[SOURCE]);
    store.discard(&discarded);

    assert_eq!(discarded.len(), 1, "discard expanded to other demand runs");
    assert!(
        store.get(SOURCE).is_none(),
        "discarded demand run is still retained"
    );
    assert!(
        Rc::ptr_eq(&store.get(DIRECT).unwrap(), &direct)
            && Rc::ptr_eq(&store.get(PEER).unwrap(), &peer),
        "discard removed a source-dependent contribution"
    );
    let mut dependents = Vec::new();
    store.dependents_by_path[SOURCE].range_paths(|path| dependents.push(path.to_owned()));
    dependents.sort();
    assert_eq!(
        dependents,
        vec![DIRECT.to_owned(), PEER.to_owned()],
        "source dependency users, want direct demand runs"
    );
}

fn group_of(path: &str, contribution: &ContributionRef) -> DemandGroup {
    DemandGroup {
        path: path.to_owned(),
        contribution: Some(Rc::clone(contribution)),
        ..DemandGroup::default()
    }
}

#[test]
fn store_reconciles_descriptor_users() {
    const PATH_A: &str = "/project/a.ts";
    const PATH_B: &str = "/project/b.ts";
    const PATH_C: &str = "/project/c.ts";
    const OLD: &str = "symbol:h:old";
    const SHARED: &str = "symbol:h:shared";
    const NEW: &str = "symbol:h:new";
    let mut store = RetainedContributionStore::default();
    store.add(PATH_A, store_test_contribution(&[], &[OLD, SHARED], true));
    store.add(PATH_B, store_test_contribution(&[], &[OLD], true));

    let replacement_a = store_test_contribution(&[], &[NEW, SHARED], true);
    let new_c = store_test_contribution(&[], &[NEW], true);
    let non_durable = store_test_contribution(&[], &[OLD], false);
    store.commit(&[
        group_of(PATH_A, &replacement_a),
        group_of(PATH_C, &new_c),
        group_of("/project/volatile.ts", &non_durable),
    ]);

    assert!(
        Rc::ptr_eq(&store.get(PATH_A).unwrap(), &replacement_a)
            && Rc::ptr_eq(&store.get(PATH_C).unwrap(), &new_c),
        "reconcile did not install replacement contributions"
    );
    assert!(
        store.get(PATH_B).is_none() && store.get("/project/volatile.ts").is_none(),
        "reconcile retained a removed or non-durable contribution"
    );
    assert!(
        store
            .descriptor_users
            .get(OLD)
            .is_none_or(PathMembership::is_empty),
        "old descriptor user count is not none"
    );
    assert_eq!(
        store.descriptor_users[SHARED].len(),
        1,
        "shared descriptor users"
    );
    let mut new_users = Vec::new();
    store.range_descriptor_users(NEW, |path| new_users.push(path.to_owned()));
    new_users.sort();
    assert_eq!(new_users, vec![PATH_A.to_owned(), PATH_C.to_owned()]);

    store.commit(&[group_of(PATH_C, &new_c)]);
    assert!(
        store.get(PATH_A).is_none(),
        "second reconcile did not remove departed contribution"
    );
    assert!(
        store
            .descriptor_users
            .get(SHARED)
            .is_none_or(PathMembership::is_empty),
        "removed contribution remains in shared descriptor index"
    );
    let mut remaining = Vec::new();
    store.range_descriptor_users(NEW, |path| remaining.push(path.to_owned()));
    assert_eq!(remaining, vec![PATH_C.to_owned()]);
}

#[test]
fn contribution_commit_is_transactional() {
    const PATH: &str = "/project/source.ts";
    const OLD_PATH: &str = "/project/old.ts";
    const OLD_SUPPRESS: &str = "symbol:h:old-suppression";
    let location = location(PATH, 13, 18);
    let mut backend = backend_with(&[(PATH, "export const value = 1\n")]);
    backend.tamper_dependencies =
        Some(vec!["/project/z.ts".to_owned(), "/project/a.ts".to_owned()]);
    let (semantic_calls, reference_calls) = (
        Arc::clone(&backend.semantic_calls),
        Arc::clone(&backend.reference_calls),
    );
    let mut closure = DemandClosure::new(Box::new(backend), None);

    let old_contribution = store_test_contribution(&[], &[OLD_SUPPRESS], true);
    closure.retained.add(OLD_PATH, old_contribution);
    closure.last_suppression = Some(HashSet::from([SymbolId::from(OLD_SUPPRESS)]));
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
    assert_eq!(
        closure.last_suppression, None,
        "last suppression advanced after failure"
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
    assert_eq!(
        closure.last_suppression,
        Some(
            HashMap::from([(symbol_id(&location), ())])
                .into_keys()
                .collect()
        ),
        "retry did not commit the new suppression"
    );
}
