//! Stable inputs and producer interface for backend orchestration.
//!
//! Callers describe source generations through [`SourceFile`] and
//! [`SourceChange`]. The build pipeline asks a [`TypeFactsProvider`] for one
//! semantic table; process lifecycle and transport stay behind its adapters.

use std::{sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};
use solid_facts::{TypeScriptChanges, TypeScriptTable};
use typefacts::v3::EntityDemand;

use crate::BackendError;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceFile {
    pub path: String,
    pub source: Arc<str>,
    #[serde(default)]
    pub compiler_options: solid_facts::compiler::CompilerOptions,
}

#[derive(Clone, Debug)]
pub struct SourceChange {
    pub path: String,
    pub version: u64,
    pub source: Option<String>,
    pub compiler_options: solid_facts::compiler::CompilerOptions,
}

pub struct SemanticDemandGroup<'a> {
    pub path: &'a str,
    pub demands: &'a [EntityDemand],
    pub shared_demands: Option<&'a Arc<[EntityDemand]>>,
}

/// The checker's complete interface to a Type Facts producer.
///
/// The retained session owns process lifecycle, framing, handshake, request
/// correlation, retained demands, and delta application. The analysis module
/// asks only for the fact table for this generation's grouped demands.
pub trait TypeFactsProvider {
    /// Analyses the current generation from demands already grouped by path.
    ///
    /// A group equal to the retained state is neither cloned nor transmitted,
    /// so an unchanged generation costs a lookup rather than a round trip.
    fn semantic_grouped(
        &mut self,
        groups: &[SemanticDemandGroup<'_>],
    ) -> Result<TypeScriptTable, BackendError>;

    /// Analyses from a flat demand list, grouping it first. A convenience for
    /// callers that do not already keep demands grouped.
    fn semantic(&mut self, demands: Vec<EntityDemand>) -> Result<TypeScriptTable, BackendError> {
        let grouped = group_demands(&demands);
        let groups = grouped
            .iter()
            .filter(|run| !run.is_empty())
            .map(|run| SemanticDemandGroup {
                path: run[0].location.path.as_ref(),
                demands: run,
                shared_demands: None,
            })
            .collect::<Vec<_>>();
        self.semantic_grouped(&groups)
    }

    fn take_last_exchange_timings(&mut self) -> Option<TypeFactsExchangeTimings> {
        None
    }

    fn take_last_table_changes(&mut self) -> Option<TypeScriptChanges> {
        None
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TypeFactsExchangeTimings {
    pub roundtrip: Duration,
    pub request_send: Duration,
    pub request_bytes: u64,
    pub response_decode: Duration,
    pub response_bytes: u64,
    pub server_request_decode: Duration,
    pub server_analyze: Duration,
    pub server_async: Duration,
    pub server_demand: Duration,
    pub server_assembly: Duration,
    pub server_sort: Duration,
    pub server_close_symbols: Duration,
    pub server_materialized: bool,
    pub server_retained_files: u64,
    pub server_recomputed_files: u64,
    pub server_non_durable_files: u64,
}

impl TypeFactsExchangeTimings {
    #[must_use]
    pub fn encode_and_transport(self) -> Duration {
        self.roundtrip.saturating_sub(
            self.request_send
                .saturating_add(self.response_decode)
                .saturating_add(self.server_request_decode)
                .saturating_add(self.server_analyze),
        )
    }
}

/// Splits a demand list into per-path runs.
///
/// Demands arrive sorted by location, so equal paths are already adjacent and
/// grouping is a single scan.
fn group_demands(demands: &[EntityDemand]) -> Vec<Vec<EntityDemand>> {
    let mut runs: Vec<Vec<EntityDemand>> = Vec::new();
    for demand in demands {
        match runs.last_mut() {
            Some(run) if run[0].location.path == demand.location.path => run.push(demand.clone()),
            _ => runs.push(vec![demand.clone()]),
        }
    }
    runs
}
