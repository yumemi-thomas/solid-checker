//! Retained per-file contributions to a demand closure (Go:
//! `retained_contributions.go`).
//!
//! A [`RetainedContribution`] is the canonical form of one file's share of a
//! closure: its entity rows, the symbols they reach, and the evidence that
//! decides when it may be reused. Contributions are shared between the demand
//! groups that name them and the [`RetainedContributionStore`] that indexes
//! them, so they live behind [`ContributionRef`]; the one in-place change a
//! published contribution sees is [`RetainedContribution::release_transport_rows`].

use crate::demands::DemandGroup;
use crate::gocompat::{clean, quote};
use crate::model::{SemanticDemandRunResult, SymbolId, location_debug};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use typefacts::v3::EntityDemand;
use typefacts::{EntityFact, SymbolClaim, TypeDescriptor};

pub type ContributionRef = Rc<RefCell<RetainedContribution>>;

/// A type descriptor and the symbol it was minted for, kept after the rows
/// are released so a later closure can seed first-wins descriptor dedup.
#[derive(Clone, Debug)]
pub struct RetainedDescriptor {
    pub symbol: SymbolId,
    pub descriptor: Arc<TypeDescriptor>,
}

#[derive(Debug, Default)]
pub struct RetainedContribution {
    pub demand_hash: u64,
    /// The file's entity rows; `None` once the packed transition owns them.
    /// The table borrows these same allocations as its per-file runs.
    pub entities: Option<Arc<[EntityFact]>>,
    pub full_tier: Vec<u32>,
    pub roots: Vec<SymbolId>,
    pub full_tier_symbols: Vec<SymbolId>,
    pub descriptors: Vec<RetainedDescriptor>,
    pub structural: Vec<SymbolId>,
    pub dependencies: Vec<String>,
    pub descriptor_symbols: Vec<SymbolId>,
    pub durable: bool,
    pub seeds_prepared: bool,
}

impl RetainedContribution {
    /// Captures the compact semantic seeds that prove and patch later closure
    /// generations: source locations and resolved-call bodies remain solely in
    /// the client's retained table (Go: `prepareTransportSeeds`).
    pub fn prepare_transport_seeds(&mut self) {
        let Some(entities) = &self.entities else {
            return;
        };
        if self.seeds_prepared {
            return;
        }
        for entity in entities.iter() {
            if !entity.symbol.is_empty() {
                self.roots.push(entity.symbol.clone());
            }
            if let Some(call) = &entity.resolved_call {
                if !call.target.is_empty() {
                    self.roots.push(call.target.clone());
                }
                if let Some(targets) = &call.targets {
                    for candidate in targets.candidates.iter() {
                        if !candidate.symbol.is_empty() {
                            self.roots.push(candidate.symbol.clone());
                        }
                    }
                }
            }
            if !entity.symbol.is_empty()
                && let Some(descriptor) = &entity.type_descriptor
            {
                self.descriptors.push(RetainedDescriptor {
                    symbol: entity.symbol.clone(),
                    descriptor: Arc::clone(descriptor),
                });
            }
        }
        for &entity_index in &self.full_tier {
            let symbol = &entities[entity_index as usize].symbol;
            if !symbol.is_empty() {
                self.full_tier_symbols.push(symbol.clone());
            }
        }
        self.seeds_prepared = true;
    }

    /// Transfers expanded entity ownership to the packed transition after the
    /// session has retained the compact contribution.
    pub fn release_transport_rows(&mut self) {
        if self.entities.is_none() {
            return;
        }
        self.prepare_transport_seeds();
        self.entities = None;
        self.full_tier = Vec::new();
    }
}

/// Merges an aligned semantic demand-run result exactly once (Go:
/// `prepareRetainedContribution`). Same-location demands are adjacent in
/// canonical order, so no location map or post-build normalization is needed.
pub fn prepare_retained_contribution(
    path: &str,
    hash: u64,
    demands: &[EntityDemand],
    result: SemanticDemandRunResult,
) -> Result<ContributionRef, String> {
    if result.entities.len() != demands.len() || result.structural.len() != demands.len() {
        return Err(format!(
            "semantic demand run {} returned {} entities and {} structural symbols for {} demands",
            quote(path),
            result.entities.len(),
            result.structural.len(),
            demands.len()
        ));
    }
    validate_canonical_dependency_paths(path, &result.dependencies)?;

    let full_tier_count = demands
        .iter()
        .zip(&result.entities)
        .filter(|(demand, entity)| !entity.symbol.is_empty() && demand.references)
        .count();
    let mut entities: Vec<EntityFact> = Vec::with_capacity(result.entities.len());
    let mut full_tier: Vec<u32> = Vec::with_capacity(full_tier_count);
    let mut structural: Vec<SymbolId> = Vec::new();

    for (index, (entity, structural_symbol)) in result
        .entities
        .into_iter()
        .zip(result.structural)
        .enumerate()
    {
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
            if demand.references {
                full_tier.push(u32::try_from(last).map_err(|error| error.to_string())?);
            }
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
        if entity.reference_space.is_some() {
            target.reference_space = entity.reference_space;
        }
        if !entity.runtime_identity.is_empty() {
            target.runtime_identity = entity.runtime_identity;
        }
        if entity.runtime_binding_kind.is_some() {
            target.runtime_binding_kind = entity.runtime_binding_kind;
        }
        if !structural_symbol.is_empty() {
            structural.push(structural_symbol);
        }
    }

    let mut descriptor_symbols: Vec<SymbolId> = Vec::new();
    let mut entity_index: Option<usize> = None;
    for (index, demand) in demands.iter().enumerate() {
        // The immutable demand run is the canonical source for the original
        // row boundaries here.
        if index == 0 || demand.location != demands[index - 1].location {
            entity_index = Some(entity_index.map_or(0, |current| current + 1));
        }
        if demand.type_descriptor {
            let symbol = &entities[entity_index.unwrap_or(0)].symbol;
            if !symbol.is_empty() {
                descriptor_symbols.push(symbol.clone());
            }
        }
    }
    descriptor_symbols.sort();
    descriptor_symbols.dedup();

    Ok(Rc::new(RefCell::new(RetainedContribution {
        demand_hash: hash,
        entities: Some(entities.into()),
        full_tier,
        structural,
        dependencies: result.dependencies,
        descriptor_symbols,
        durable: result.durable,
        ..RetainedContribution::default()
    })))
}

/// Dependencies must be clean, unique, ordered, and exclude their owner
/// (Go: `validateCanonicalDependencyPaths`).
pub fn validate_canonical_dependency_paths(owner: &str, paths: &[String]) -> Result<(), String> {
    if owner.is_empty() || clean(owner) != owner {
        return Err(format!(
            "semantic dependency owner {} is not clean and non-empty",
            quote(owner)
        ));
    }
    for (index, path) in paths.iter().enumerate() {
        if path.is_empty() || clean(path) != *path {
            return Err(format!(
                "semantic dependency path {} is not clean and non-empty",
                quote(path)
            ));
        }
        if path == owner {
            return Err(format!(
                "semantic dependency paths include owner {}",
                quote(owner)
            ));
        }
        if index != 0 && paths[index - 1] >= *path {
            return Err(format!(
                "semantic dependency paths are not strictly ordered at {}",
                quote(path)
            ));
        }
    }
    Ok(())
}

/// Reverse-index buckets stay in a small vector, then promote to constant-time
/// membership (Go: `pathMembershipMapThreshold`).
pub const PATH_MEMBERSHIP_MAP_THRESHOLD: usize = 16;

/// A set of paths whose iteration order no consumer observes: deterministic
/// output is established only after membership has been translated to
/// canonical group indices (Go: `pathMembership`).
#[derive(Clone, Debug, Default)]
pub struct PathMembership {
    pub small: Vec<String>,
    pub large: Option<HashSet<String>>,
}

impl PathMembership {
    pub fn add(&mut self, path: &str) {
        if let Some(large) = &mut self.large {
            large.insert(path.to_owned());
            return;
        }
        if self.small.iter().any(|existing| existing == path) {
            return;
        }
        if self.small.len() < PATH_MEMBERSHIP_MAP_THRESHOLD {
            self.small.push(path.to_owned());
            return;
        }
        let mut large: HashSet<String> = self.small.drain(..).collect();
        large.insert(path.to_owned());
        self.large = Some(large);
    }

    pub fn remove(&mut self, path: &str) {
        if let Some(large) = &mut self.large {
            large.remove(path);
            return;
        }
        if let Some(index) = self.small.iter().position(|existing| existing == path) {
            self.small.swap_remove(index);
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.large.as_ref().map_or(self.small.len(), HashSet::len)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn range_paths(&self, mut visit: impl FnMut(&str)) {
        if let Some(large) = &self.large {
            large.iter().for_each(|path| visit(path));
            return;
        }
        self.small.iter().for_each(|path| visit(path));
    }
}

/// Retained contributions and the two reverse indexes that make source
/// invalidation and suppression refresh targeted (Go:
/// `retainedContributionStore`).
#[derive(Debug, Default)]
pub struct RetainedContributionStore {
    pub by_path: HashMap<String, ContributionRef>,
    pub dependents_by_path: HashMap<String, PathMembership>,
    pub descriptor_users: HashMap<SymbolId, PathMembership>,
}

impl RetainedContributionStore {
    #[must_use]
    pub fn get(&self, path: &str) -> Option<ContributionRef> {
        self.by_path.get(path).cloned()
    }

    pub fn add(&mut self, path: &str, contribution: ContributionRef) {
        if self.by_path.contains_key(path) {
            self.remove(path);
        }
        {
            let retained = contribution.borrow();
            for dependency in &retained.dependencies {
                self.dependents_by_path
                    .entry(dependency.clone())
                    .or_default()
                    .add(path);
            }
            for symbol in &retained.descriptor_symbols {
                self.descriptor_users
                    .entry(symbol.clone())
                    .or_default()
                    .add(path);
            }
        }
        self.by_path.insert(path.to_owned(), contribution);
    }

    pub fn remove(&mut self, path: &str) {
        let Some(contribution) = self.by_path.remove(path) else {
            return;
        };
        let retained = contribution.borrow();
        for dependency in &retained.dependencies {
            if let Some(users) = self.dependents_by_path.get_mut(dependency) {
                users.remove(path);
                if users.is_empty() {
                    self.dependents_by_path.remove(dependency);
                }
            }
        }
        for symbol in &retained.descriptor_symbols {
            if let Some(users) = self.descriptor_users.get_mut(symbol) {
                users.remove(path);
                if users.is_empty() {
                    self.descriptor_users.remove(symbol);
                }
            }
        }
    }

    /// Removes only the named demand runs. Changing what one file asks for
    /// cannot change a different file's source-derived semantic facts.
    pub fn discard(&mut self, paths: &HashSet<String>) {
        for path in paths {
            self.remove(path);
        }
    }

    pub fn range_descriptor_users(&self, symbol: &str, visit: impl FnMut(&str)) {
        if let Some(users) = self.descriptor_users.get(symbol) {
            users.range_paths(visit);
        }
    }

    /// Publishes a retained-contribution transaction. Called only after
    /// semantic resolution, assembly and the transport manifest have all
    /// succeeded (Go: `commit`).
    pub fn commit(&mut self, groups: &[DemandGroup]) {
        let desired: HashSet<&str> = groups
            .iter()
            .filter(|group| {
                group
                    .contribution
                    .as_ref()
                    .is_some_and(|contribution| contribution.borrow().durable)
            })
            .map(|group| group.path.as_str())
            .collect();
        let departed: Vec<String> = self
            .by_path
            .keys()
            .filter(|path| !desired.contains(path.as_str()))
            .cloned()
            .collect();
        for path in departed {
            self.remove(&path);
        }
        for group in groups {
            let Some(contribution) = &group.contribution else {
                continue;
            };
            if !contribution.borrow().durable
                || self
                    .by_path
                    .get(&group.path)
                    .is_some_and(|existing| Rc::ptr_eq(existing, contribution))
            {
                continue;
            }
            self.add(&group.path, Rc::clone(contribution));
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
