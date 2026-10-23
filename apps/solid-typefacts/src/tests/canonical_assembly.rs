//! Ports `canonical_assembly_test.go`.

use super::{backend_with, location};
use crate::closure::{DemandClosure, DemandInputGroup};
use crate::context::Context;
use crate::contributions::prepare_retained_contribution;
use crate::model::{SemanticDemandRunResult, blank_entity};
use typefacts::producer::EntityRows;
use typefacts::v3::{EntityDemand, SourceFile};

fn source(path: &str, text: &str) -> SourceFile {
    SourceFile {
        path: path.to_owned(),
        local: false,
        source: text.as_bytes().to_vec(),
    }
}

#[test]
fn closure_rejects_noncanonical_adapter_sources() {
    let a = source("/project/a.ts", "a");
    let b = source("/project/b.ts", "b");
    let mut backend = backend_with(&[("/project/a.ts", "a")]);
    backend.raw_sources = Some(vec![b, a]);
    let mut closure = DemandClosure::new(Box::new(backend), None);

    let error = closure
        .demand_table_for_groups(
            &Context::new(),
            1,
            &[DemandInputGroup {
                path: "/project/a.ts".to_owned(),
                demands: vec![EntityDemand {
                    location: location("/project/a.ts", 4, 5),
                    symbol: true,
                    ..EntityDemand::default()
                }],
            }],
            &[],
        )
        .unwrap_err();
    assert!(
        error.0.contains("strictly path-ordered"),
        "noncanonical adapter sources error = {error}"
    );
}

#[test]
fn closure_canonicalizes_demand_at_input_boundary() {
    let mut closure = DemandClosure::new(Box::new(backend_with(&[("/project/a.ts", "a")])), None);
    let table = closure
        .demand_table_for_groups(
            &Context::new(),
            1,
            &[DemandInputGroup {
                path: "/project/a.ts".to_owned(),
                demands: vec![
                    EntityDemand {
                        location: location("/project/a.ts", 10, 11),
                        ..EntityDemand::default()
                    },
                    EntityDemand {
                        location: location("/project/a.ts", 1, 2),
                        ..EntityDemand::default()
                    },
                ],
            }],
            &[],
        )
        .unwrap();
    let EntityRows::Runs(runs) = &table.entities else {
        panic!("entity rows are not per-file runs");
    };
    assert!(
        runs.len() == 1
            && runs[0].len() == 2
            && runs[0][0].location.start_byte == 1
            && runs[0][1].location.start_byte == 10,
        "canonicalized entity runs = {runs:?}"
    );
}

#[test]
fn closure_rejects_duplicate_demand_group_paths() {
    let mut closure = DemandClosure::new(Box::new(backend_with(&[("/project/a.ts", "a")])), None);
    let group = DemandInputGroup {
        path: "/project/a.ts".to_owned(),
        demands: Vec::new(),
    };
    let error = closure
        .demand_table_for_groups(&Context::new(), 1, &[group.clone(), group], &[])
        .unwrap_err();
    assert!(
        error.0.contains("each path may appear once"),
        "duplicate groups error = {error}"
    );
}

#[test]
fn prepare_retained_contribution_rejects_misaligned_entity() {
    let path = "/project/a.ts";
    let demand = EntityDemand {
        location: location(path, 1, 2),
        ..EntityDemand::default()
    };
    let error = prepare_retained_contribution(
        path,
        &[demand],
        SemanticDemandRunResult {
            entities: vec![blank_entity(location(path, 2, 3))],
            durable: true,
        },
    )
    .unwrap_err();
    assert!(
        error.contains("entity 0 location"),
        "misaligned entity error = {error}"
    );
}
