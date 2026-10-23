//! The Go package's compiler-independent tests, ported case for case. Each
//! module names the Go test file it ports. Tests that need the real compiler
//! (the `*_integration_test.go` files and the corpus tests) belong to S4.

mod binder_ids;
mod binder_retention;
mod canonical_assembly;
mod contributions;
mod entity_projection;
mod generation;
mod history_order;
mod materialize;
mod module_graph;
mod project_invalidation;
mod project_open;
mod project_references;
mod project_support;
mod protocol;
mod randomized_incremental;
mod reactor;
mod regression;
mod resolved_calls;
mod runtime_facts;
mod semantic_demand;
mod session;
mod session_demands;
mod shape_facts;
mod source_facts;
mod symbol_facts;
mod symbol_integration;
mod type_facts;

use crate::context::Context;
use crate::fake::FakeBackend;
use crate::session::Session;
use crate::wire::Incoming;
use typefacts::Location;
use typefacts::v3::{EntityDemand, Operation, Request, Response, TYPE_FACTS_SCHEMA_V1};

pub(crate) const PROJECT: &str = "/project/tsconfig.json";

pub(crate) fn lifecycle_request(id: u64, operation: Operation, generation: u64) -> Request {
    Request {
        schema: TYPE_FACTS_SCHEMA_V1,
        changes: Vec::new(),
        demands: Vec::new(),
        operation,
        project_id: PROJECT.to_owned(),
        request_id: id,
        generation,
        reset_state: false,
        state_token: String::new(),
        compact_demands: None,
        cancel_request_id: 0,
        removed_demand_paths: Vec::new(),
        symbol_queries: Vec::new(),
        release_analysis: false,
        reference_changes: false,
        reference_paths: Vec::new(),
        module_graph: None,
    }
}

pub(crate) fn lifecycle(session: &mut Session, request: Request) -> Response {
    lifecycle_with(session, &Context::new(), request)
}

pub(crate) fn lifecycle_with(session: &mut Session, ctx: &Context, request: Request) -> Response {
    session.lifecycle(
        ctx,
        &Incoming {
            request,
            bad_operation: None,
        },
    )
}

/// A fake backend holding exactly one source (Go: `transportOnlyBackend{source}`).
pub(crate) fn backend_with(files: &[(&str, &str)]) -> FakeBackend {
    FakeBackend::with_sources(files.iter().copied())
}

pub(crate) fn session_over(backend: FakeBackend) -> Session {
    Session::new(Box::new(backend), PROJECT, true).unwrap()
}

pub(crate) fn location(path: &str, start: u64, end: u64) -> Location {
    Location {
        path: path.into(),
        start_byte: start,
        end_byte: end,
    }
}

/// Go's `demandAt`: a symbol demand over `[start, start+1)`.
pub(crate) fn demand_at(path: &str, start: u64) -> EntityDemand {
    EntityDemand {
        location: location(path, start, start + 1),
        symbol: true,
        ..EntityDemand::default()
    }
}

/// A symbol demand that also asks for references.
pub(crate) fn regression_demand(location: Location) -> EntityDemand {
    EntityDemand {
        location,
        symbol: true,
        references: true,
        ..EntityDemand::default()
    }
}
