//! Per-file contributions to a demand closure (Go: `retained_contributions.go`).
//!
//! A [`RetainedContribution`] is one file's share of a table: its entity rows,
//! and whether every identity they carry is a binder-level one. A table builds
//! each afresh (ADR 0364); the [`RetainedContributionStore`] only holds the
//! published ones until the packed transition owns their rows
//! ([`RetainedContribution::release_transport_rows`]). Contributions are shared
//! between the demand groups that name them and the store, so they live behind
//! [`ContributionRef`].

use crate::demands::DemandGroup;
use crate::gocompat::quote;
use crate::model::{SemanticDemandRunResult, location_debug};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use typefacts::v3::EntityDemand;
use typefacts::{EntityFact, SymbolClaim};

pub type ContributionRef = Rc<RefCell<RetainedContribution>>;

#[derive(Debug, Default)]
pub struct RetainedContribution {
    /// The file's entity rows; `None` once the packed transition owns them.
    /// The table borrows these same allocations as its per-file runs.
    pub entities: Option<Arc<[EntityFact]>>,
    /// Whether every symbol identity in the rows is binder-level
    /// (`symbol:h:`); the timings count the files where it is not.
    pub durable: bool,
}

impl RetainedContribution {
    /// Transfers expanded entity ownership to the packed transition after the
    /// session has retained the compact contribution.
    pub fn release_transport_rows(&mut self) {
        self.entities = None;
    }
}

/// Merges an aligned semantic demand-run result exactly once (Go:
/// `prepareRetainedContribution`). Same-location demands are adjacent in
/// canonical order, so no location map or post-build normalization is needed.
pub fn prepare_retained_contribution(
    path: &str,
    demands: &[EntityDemand],
    result: SemanticDemandRunResult,
) -> Result<ContributionRef, String> {
    let entities = prepare_rows(path, demands, result.entities)?;
    Ok(Rc::new(RefCell::new(RetainedContribution {
        entities: Some(entities.into()),
        durable: result.durable,
    })))
}

/// The entity rows of an aligned run result: one row per location, each
/// field from the last entity at the location that has it.
pub fn prepare_rows(
    path: &str,
    demands: &[EntityDemand],
    result: Vec<EntityFact>,
) -> Result<Vec<EntityFact>, String> {
    if result.len() != demands.len() {
        return Err(format!(
            "semantic demand run {} returned {} entities for {} demands",
            quote(path),
            result.len(),
            demands.len()
        ));
    }

    let mut entities: Vec<EntityFact> = Vec::with_capacity(result.len());

    for (index, entity) in result.into_iter().enumerate() {
        let demand = &demands[index];
        if entity.location != demand.location {
            return Err(format!(
                "semantic demand run {} entity {} location = {}, want {}",
                quote(path),
                index,
                location_debug(&entity.location),
                location_debug(&demand.location)
            ));
        }
        if entities
            .last()
            .is_none_or(|last| last.location != entity.location)
        {
            entities.push(crate::model::blank_entity(entity.location.clone()));
        }
        let last = entities.len() - 1;
        let target = &mut entities[last];
        if !entity.symbol.is_empty() {
            target.symbol = entity.symbol.clone();
            target.symbol_claim = if entity.symbol_claim == SymbolClaim::Covering {
                SymbolClaim::Covering
            } else {
                SymbolClaim::Partial
            };
        } else if entity.symbol_claim == SymbolClaim::Unresolved && target.symbol.is_empty() {
            target.symbol_claim = SymbolClaim::Unresolved;
        }
        if entity.type_descriptor.is_some() {
            target.type_descriptor = entity.type_descriptor;
        }
        if entity.resolved_call.is_some() {
            target.resolved_call = entity.resolved_call;
        }
        if entity.callability.is_some() {
            target.callability = entity.callability;
        }
        if entity.constructability.is_some() {
            target.constructability = entity.constructability;
        }
        if entity.runtime_value_domain.is_some() {
            target.runtime_value_domain = entity.runtime_value_domain;
        }
        if entity.primitive_value_domain.is_present() {
            target.primitive_value_domain = entity.primitive_value_domain;
        }
        if entity
            .primitive_literal_candidates
            .as_ref()
            .is_some_and(|candidates| !candidates.is_empty())
        {
            target.primitive_literal_candidates = entity.primitive_literal_candidates;
        }
        if entity.call_result_domain.is_some() {
            target.call_result_domain = entity.call_result_domain;
        }
        if entity.constant_value.is_some() {
            target.constant_value = entity.constant_value;
        }
        if entity.array_shape.is_some() {
            target.array_shape = entity.array_shape;
        }
        if entity.tuple_shape.is_some() {
            target.tuple_shape = entity.tuple_shape;
        }
        if entity.library_types.is_some() {
            target.library_types = entity.library_types;
        }
        if entity.declared_keys.is_some() {
            target.declared_keys = entity.declared_keys;
        }
        if entity.writable_optional_property {
            target.writable_optional_property = true;
        }
        if entity.reference_space.is_some() {
            target.reference_space = entity.reference_space;
        }
        if !entity.runtime_identity.is_empty() {
            target.runtime_identity = entity.runtime_identity;
        }
        if entity.runtime_binding_kind.is_some() {
            target.runtime_binding_kind = entity.runtime_binding_kind;
        }
    }

    Ok(entities)
}

/// The contributions a table published, by clean path, until the packed
/// transition releases their rows (Go: `retainedContributionStore`).
#[derive(Debug, Default)]
pub struct RetainedContributionStore {
    pub by_path: HashMap<String, ContributionRef>,
}

impl RetainedContributionStore {
    #[must_use]
    pub fn get(&self, path: &str) -> Option<ContributionRef> {
        self.by_path.get(path).cloned()
    }

    pub fn add(&mut self, path: &str, contribution: ContributionRef) {
        self.by_path.insert(path.to_owned(), contribution);
    }

    /// Publishes a table's contributions into the empty store. Called only
    /// after semantic resolution, assembly and the transport manifest have all
    /// succeeded (Go: `commit`). A contribution whose evidence is not
    /// durable (a cross-file census answer) is not published.
    pub fn commit(&mut self, groups: &[DemandGroup]) {
        for group in groups {
            if let Some(contribution) = &group.contribution
                && contribution.borrow().durable
            {
                self.add(&group.path, Rc::clone(contribution));
            }
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
