//! Policy-2 dependency composition planning.
//!
//! This module owns canonical bottom-up ordering and cycle refusal. It does not
//! authenticate receipts: Slice 8 supplies that authority. Until then the
//! schedule can identify the exact first missing dependency demand but cannot
//! construct an accepted-composition witness from a policy-1 receipt or a
//! caller-provided digest.

use solid_reactive_ir::contract_semantics::certification::{
    DependencyDemandInput, ProofDemandSubject, ProofFamily,
};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

use super::CertificationPlan;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DependencyNodeIdentity {
    pub package: String,
    pub artifact_case: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyQueueNode {
    identity: DependencyNodeIdentity,
    dependencies: Vec<DependencyNodeIdentity>,
}

impl DependencyQueueNode {
    #[must_use]
    pub fn new(
        package: impl Into<String>,
        artifact_case: impl Into<String>,
        dependencies: Vec<DependencyNodeIdentity>,
    ) -> Self {
        Self {
            identity: DependencyNodeIdentity {
                package: package.into(),
                artifact_case: artifact_case.into(),
            },
            dependencies,
        }
    }
}

/// Canonical dependency-first order for a finite certification batch.
#[derive(Debug)]
pub struct DependencyCertificationQueue {
    order: Vec<DependencyNodeIdentity>,
}

impl DependencyCertificationQueue {
    pub fn build(
        nodes: impl IntoIterator<Item = DependencyQueueNode>,
    ) -> Result<Self, DependencyCompositionError> {
        let mut graph = BTreeMap::<DependencyNodeIdentity, Vec<DependencyNodeIdentity>>::new();
        for mut node in nodes {
            validate_node(&node.identity)?;
            node.dependencies.sort();
            node.dependencies.dedup();
            for dependency in &node.dependencies {
                validate_node(dependency)?;
            }
            if graph
                .insert(node.identity.clone(), node.dependencies)
                .is_some()
            {
                return Err(DependencyCompositionError::DuplicateNode(node.identity));
            }
        }
        let mut states = BTreeMap::<DependencyNodeIdentity, VisitState>::new();
        let mut stack = Vec::new();
        let mut order = Vec::with_capacity(graph.len());
        for node in graph.keys() {
            visit(node, &graph, &mut states, &mut stack, &mut order)?;
        }
        Ok(Self { order })
    }

    #[must_use]
    pub fn order(&self) -> &[DependencyNodeIdentity] {
        &self.order
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VisitState {
    Visiting,
    Complete,
}

fn visit(
    node: &DependencyNodeIdentity,
    graph: &BTreeMap<DependencyNodeIdentity, Vec<DependencyNodeIdentity>>,
    states: &mut BTreeMap<DependencyNodeIdentity, VisitState>,
    stack: &mut Vec<DependencyNodeIdentity>,
    order: &mut Vec<DependencyNodeIdentity>,
) -> Result<(), DependencyCompositionError> {
    match states.get(node) {
        Some(VisitState::Complete) => return Ok(()),
        Some(VisitState::Visiting) => {
            let start = stack.iter().position(|entry| entry == node).unwrap_or(0);
            let mut cycle = stack[start..].to_vec();
            cycle.push(node.clone());
            return Err(DependencyCompositionError::Cycle(canonical_cycle(cycle)));
        }
        None => {}
    }
    states.insert(node.clone(), VisitState::Visiting);
    stack.push(node.clone());
    if let Some(dependencies) = graph.get(node) {
        for dependency in dependencies {
            // An edge outside this batch is a leaf awaiting an authenticated
            // receipt, not a graph node whose unseen dependencies may be
            // guessed. It is handled by the composition schedule below.
            if graph.contains_key(dependency) {
                visit(dependency, graph, states, stack, order)?;
            }
        }
    }
    stack.pop();
    states.insert(node.clone(), VisitState::Complete);
    order.push(node.clone());
    Ok(())
}

fn canonical_cycle(mut cycle: Vec<DependencyNodeIdentity>) -> Vec<DependencyNodeIdentity> {
    cycle.pop();
    if cycle.is_empty() {
        return cycle;
    }
    let start = cycle
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| left.cmp(right))
        .map_or(0, |(index, _)| index);
    cycle.rotate_left(start);
    cycle.push(cycle[0].clone());
    cycle
}

fn validate_node(node: &DependencyNodeIdentity) -> Result<(), DependencyCompositionError> {
    if node.package.trim().is_empty() || node.artifact_case.trim().is_empty() {
        return Err(DependencyCompositionError::InvalidNode(node.clone()));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DependencyCompositionRequirement {
    demand_id: String,
    dependency: DependencyDemandInput,
    parent_export: String,
    semantic_claim_id: String,
}

impl DependencyCompositionRequirement {
    #[must_use]
    pub fn demand_id(&self) -> &str {
        &self.demand_id
    }

    #[must_use]
    pub const fn dependency(&self) -> &DependencyDemandInput {
        &self.dependency
    }

    #[must_use]
    pub fn parent_export(&self) -> &str {
        &self.parent_export
    }

    #[must_use]
    pub fn semantic_claim_id(&self) -> &str {
        &self.semantic_claim_id
    }
}

pub struct DependencyCompositionSchedule {
    requirements: Vec<DependencyCompositionRequirement>,
}

impl DependencyCompositionSchedule {
    pub(crate) fn from_plan(plan: &CertificationPlan) -> Result<Self, DependencyCompositionError> {
        let mut requirements = plan
            .demand_graph
            .demands()
            .iter()
            .filter(|demand| demand.family() == ProofFamily::AcceptedDependencyComposition)
            .map(|demand| {
                let ProofDemandSubject::DependencyClosure {
                    dependency,
                    parent,
                    semantic_claim_id,
                } = demand.subject()
                else {
                    return Err(DependencyCompositionError::InvalidDemand);
                };
                Ok(DependencyCompositionRequirement {
                    demand_id: demand.id().as_str().into(),
                    dependency: dependency.clone(),
                    parent_export: parent.export.clone(),
                    semantic_claim_id: semantic_claim_id.clone(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        requirements.sort_by(|left, right| {
            (
                left.dependency.package.as_str(),
                left.dependency.artifact_case.as_str(),
                left.parent_export.as_str(),
                left.semantic_claim_id.as_str(),
            )
                .cmp(&(
                    right.dependency.package.as_str(),
                    right.dependency.artifact_case.as_str(),
                    right.parent_export.as_str(),
                    right.semantic_claim_id.as_str(),
                ))
        });
        Ok(Self { requirements })
    }

    #[must_use]
    pub fn requirements(&self) -> &[DependencyCompositionRequirement] {
        &self.requirements
    }

    /// Returns the exact canonical first dependency demand absent from a
    /// caller's structural set. This is an ordering helper only: an empty
    /// result is not receipt authority and cannot construct a witness. Slice 8
    /// will feed it IDs obtained from opaque authenticated receipt tokens.
    #[must_use]
    pub fn first_unaccepted<'a>(
        &'a self,
        authenticated_demand_ids: &BTreeSet<String>,
    ) -> Option<&'a DependencyCompositionRequirement> {
        self.requirements
            .iter()
            .find(|requirement| !authenticated_demand_ids.contains(&requirement.demand_id))
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DependencyCompositionError {
    #[error("duplicate dependency certification node {0:?}")]
    DuplicateNode(DependencyNodeIdentity),
    #[error("invalid dependency certification node {0:?}")]
    InvalidNode(DependencyNodeIdentity),
    #[error("dependency certification cycle {0:?}")]
    Cycle(Vec<DependencyNodeIdentity>),
    #[error("dependency-composition demand has the wrong subject")]
    InvalidDemand,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(package: &str) -> DependencyNodeIdentity {
        DependencyNodeIdentity {
            package: package.into(),
            artifact_case: format!("artifact-case:{package}"),
        }
    }

    #[test]
    fn queue_is_dependency_first_and_independent_of_input_order() {
        let nodes = vec![
            DependencyQueueNode::new("root", "artifact-case:root", vec![id("b"), id("a")]),
            DependencyQueueNode::new("b", "artifact-case:b", vec![id("a")]),
            DependencyQueueNode::new("a", "artifact-case:a", vec![]),
        ];
        let forward = DependencyCertificationQueue::build(nodes.clone()).unwrap();
        let reverse = DependencyCertificationQueue::build(nodes.into_iter().rev()).unwrap();
        assert_eq!(forward.order(), reverse.order());
        assert_eq!(forward.order(), &[id("a"), id("b"), id("root")]);
    }

    #[test]
    fn cycle_reporting_rotates_to_the_canonical_first_node() {
        let error = DependencyCertificationQueue::build([
            DependencyQueueNode::new("z", "artifact-case:z", vec![id("b")]),
            DependencyQueueNode::new("b", "artifact-case:b", vec![id("a")]),
            DependencyQueueNode::new("a", "artifact-case:a", vec![id("z")]),
        ])
        .unwrap_err();
        assert_eq!(
            error,
            DependencyCompositionError::Cycle(vec![id("a"), id("z"), id("b"), id("a")])
        );
    }
}
