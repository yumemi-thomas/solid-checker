//! Ports `semantic_demand_test.go`.

use super::{backend_with, location};
use crate::closure::{DemandClosure, DemandInputGroup};
use crate::context::Context;
use crate::demands::{CompactBacking, DemandGroup};
use crate::fake::FakeBackend;
use std::sync::Arc;
use typefacts::producer::{EntityRows, source_digest};
use typefacts::v3::{EntityDemand, compact_demands};

const PATH: &str = "/project/source.ts";

fn closure_over(backend: FakeBackend) -> DemandClosure {
    DemandClosure::new(Box::new(backend), None)
}

#[test]
fn retained_contribution_shares_canonical_entity_backing() {
    let mut closure = closure_over(backend_with(&[(PATH, "const value = 1\n")]));
    let demand = EntityDemand {
        location: location(PATH, 6, 11),
        symbol: true,
        ..EntityDemand::default()
    };
    let table = closure
        .demand_table_for_groups(
            &Context::new(),
            1,
            &[DemandInputGroup {
                path: PATH.to_owned(),
                demands: vec![demand],
            }],
            &[PATH.to_owned()],
        )
        .unwrap();
    let contribution = closure.retained().get(PATH).expect("contribution");
    let contribution = contribution.borrow();
    let entities = contribution.entities.as_ref().expect("entities");
    let EntityRows::Runs(runs) = &table.entities else {
        panic!("entity rows are not per-file runs");
    };
    assert!(
        entities.len() == 1 && runs.len() == 1 && runs[0].len() == 1,
        "unexpected retained state: contribution={contribution:?} runs={}",
        runs.len()
    );
    assert!(
        Arc::ptr_eq(entities, &runs[0]),
        "transport entity run copied the retained contribution"
    );
}

#[test]
fn semantic_table_retains_only_source_digests() {
    let digest = source_digest(PATH, b"const value = 1\n");
    let mut backend = backend_with(&[]);
    backend.digests = Some(vec![digest.clone()]);
    let mut closure = closure_over(backend);

    let table = closure
        .demand_table_for_groups(
            &Context::new(),
            1,
            &[DemandInputGroup {
                path: PATH.to_owned(),
                demands: Vec::new(),
            }],
            &[PATH.to_owned()],
        )
        .unwrap();
    assert_eq!(table.sources, vec![digest], "semantic table source digests");
}

#[test]
/// A compact group is expanded once for its lifetime: the kept passes hold
/// that allocation (and compare it by identity), so a second table neither
/// expands nor copies it again.
fn semantic_materialization_expands_a_compact_group_once() {
    let demand = EntityDemand {
        location: location(PATH, 6, 11),
        symbol: true,
        ..EntityDemand::default()
    };
    let compact = compact_demands(&[demand]);
    let mut groups = vec![DemandGroup {
        path: PATH.to_owned(),
        compact: Some(CompactBacking {
            group: compact.groups[0].clone(),
            strings: compact.strings.clone().into(),
        }),
        ..DemandGroup::default()
    }];
    let mut closure = closure_over(backend_with(&[(PATH, "const value = 1\n")]));

    closure
        .demand_table_for_canonical_groups(&Context::new(), 1, &mut groups, &[PATH.to_owned()])
        .unwrap();
    let expanded = groups[0].demands.clone().expect("the expansion is kept");
    closure.force_full_materialization();
    closure
        .demand_table_for_canonical_groups(&Context::new(), 1, &mut groups, &[])
        .unwrap();
    assert!(
        groups[0]
            .demands
            .as_ref()
            .is_some_and(|demands| std::sync::Arc::ptr_eq(demands, &expanded)),
        "a second table expanded the compact group again"
    );
}
