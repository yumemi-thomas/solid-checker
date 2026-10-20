//! The read-only view over a computed interprocedural result, and its dependency bookkeeping.

use super::*;

#[derive(Clone, Copy)]
pub(crate) struct InterproceduralResultView<'a> {
    pub(crate) nodes: &'a [SummaryNode],
    pub(crate) indexes: &'a HashMap<(String, Span), usize>,
    pub(crate) by_symbol: &'a HashMap<SymbolId, usize>,
    pub(crate) summaries: &'a [SummaryReads],
    pub(crate) invoked_parameters: &'a [Vec<usize>],
    pub(crate) invoked_parameter_members: &'a [Vec<ParameterMemberInvocation>],
    pub(crate) returned_bindings: &'a HashMap<SymbolId, Vec<SummaryRead>>,
}

impl InterproceduralResultView<'_> {
    pub(super) fn dependency_state(
        &self,
        dependency: &InterproceduralResultDependency,
    ) -> InterproceduralResultDependencyState {
        match dependency {
            InterproceduralResultDependency::Symbol(symbol) => {
                if let Some(index) = self.by_symbol.get(symbol) {
                    InterproceduralResultDependencyState::Function {
                        name: self.nodes[*index].name.clone(),
                        summary: self.summaries[*index].to_vec(),
                        invoked_parameters: self.invoked_parameters[*index].clone(),
                        invoked_parameter_members: self.invoked_parameter_members[*index].clone(),
                    }
                } else if let Some(summary) = self.returned_bindings.get(symbol) {
                    InterproceduralResultDependencyState::Returned(summary.clone())
                } else {
                    InterproceduralResultDependencyState::Missing
                }
            }
            InterproceduralResultDependency::FunctionSource(path, span) => self
                .indexes
                .get(&(path.clone(), *span))
                .map_or(InterproceduralResultDependencyState::Missing, |index| {
                    InterproceduralResultDependencyState::Source(
                        self.nodes[*index].source_hash.clone(),
                    )
                }),
            InterproceduralResultDependency::InlineFunction(path, span) => {
                self.indexes.get(&(path.clone(), *span)).map_or(
                    InterproceduralResultDependencyState::Missing,
                    |index| InterproceduralResultDependencyState::Function {
                        name: self.nodes[*index].name.clone(),
                        summary: self.summaries[*index].to_vec(),
                        invoked_parameters: self.invoked_parameters[*index].clone(),
                        invoked_parameter_members: self.invoked_parameter_members[*index].clone(),
                    },
                )
            }
        }
    }

    pub(crate) fn dependency_matches(
        &self,
        retained: &InterproceduralResultDependencyState,
        dependency: &InterproceduralResultDependency,
    ) -> bool {
        match dependency {
            InterproceduralResultDependency::Symbol(symbol) => {
                if let Some(index) = self.by_symbol.get(symbol) {
                    matches!(
                        retained,
                        InterproceduralResultDependencyState::Function {
                            name,
                            summary,
                            invoked_parameters: previous_parameters,
                            invoked_parameter_members: previous_members,
                        } if name == &self.nodes[*index].name
                            && summary.as_slice() == &self.summaries[*index][..]
                            && previous_parameters == &self.invoked_parameters[*index]
                            && previous_members == &self.invoked_parameter_members[*index]
                    )
                } else if let Some(summary) = self.returned_bindings.get(symbol) {
                    matches!(
                        retained,
                        InterproceduralResultDependencyState::Returned(previous)
                            if previous == summary
                    )
                } else {
                    matches!(retained, InterproceduralResultDependencyState::Missing)
                }
            }
            InterproceduralResultDependency::FunctionSource(path, span) => {
                if let Some(index) = self.indexes.get(&(path.clone(), *span)) {
                    matches!(retained, InterproceduralResultDependencyState::Source(previous)
                        if previous == &self.nodes[*index].source_hash)
                } else {
                    matches!(retained, InterproceduralResultDependencyState::Missing)
                }
            }
            InterproceduralResultDependency::InlineFunction(path, span) => {
                if let Some(index) = self.indexes.get(&(path.clone(), *span)) {
                    matches!(
                        retained,
                        InterproceduralResultDependencyState::Function {
                            name,
                            summary,
                            invoked_parameters: previous_parameters,
                            invoked_parameter_members: previous_members,
                        } if name == &self.nodes[*index].name
                            && summary.as_slice() == &self.summaries[*index][..]
                            && previous_parameters == &self.invoked_parameters[*index]
                            && previous_members == &self.invoked_parameter_members[*index]
                    )
                } else {
                    matches!(retained, InterproceduralResultDependencyState::Missing)
                }
            }
        }
    }
}

pub(super) fn add_interprocedural_dependency_user(
    users: &mut HashMap<InterproceduralResultDependency, usize>,
    dependency: &InterproceduralResultDependency,
) {
    *users.entry(dependency.clone()).or_default() += 1;
}

pub(super) fn remove_interprocedural_dependency_user(
    users: &mut HashMap<InterproceduralResultDependency, usize>,
    states: &mut HashMap<InterproceduralResultDependency, InterproceduralResultDependencyState>,
    dependency: &InterproceduralResultDependency,
) {
    let Some(count) = users.get_mut(dependency) else {
        debug_assert!(false, "missing interprocedural dependency reference count");
        return;
    };
    *count -= 1;
    if *count == 0 {
        users.remove(dependency);
        states.remove(dependency);
    }
}
