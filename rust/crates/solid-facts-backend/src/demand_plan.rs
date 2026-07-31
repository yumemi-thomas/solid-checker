//! Plans the smallest complete TypeFacts query set required by downstream
//! analysis. Keeping this policy separate from transport orchestration makes
//! omissions testable before they become missing diagnostics.

use std::collections::{HashMap, HashSet};

use solid_facts::FileFacts;
use typefacts::v3::EntityDemand;

use crate::dialect::Dialect;
use crate::{
    BackendError, callee_property_location, structural_accessor_spans, typefacts_location,
};

pub(crate) fn plan(
    dialect: &'static Dialect,
    files: &[FileFacts],
) -> Result<Vec<EntityDemand>, BackendError> {
    let mut demands = Vec::new();
    for file in files {
        plan_file(dialect, file, &mut demands)?;
    }
    stable_deduplicate(&mut demands);
    Ok(demands)
}

fn plan_file(
    dialect: &'static Dialect,
    file: &FileFacts,
    demands: &mut Vec<EntityDemand>,
) -> Result<(), BackendError> {
    let path = file.path.to_string();
    let structural_accessors = structural_accessor_spans(file);
    let mut symbol_spans = HashMap::new();
    let mut type_descriptor_spans = HashSet::new();
    let mut async_symbol_spans = HashSet::new();
    let mut async_value_spans = Vec::new();
    let returned_callees = file
        .ast
        .returns
        .iter()
        .filter_map(|returned| returned.callee)
        .collect::<HashSet<_>>();
    let mut add_symbol = |span, references| {
        symbol_spans
            .entry(span)
            .and_modify(|current| *current |= references)
            .or_insert(references);
    };

    for import in &file.ast.imports {
        for binding in import
            .bindings
            .iter()
            .filter(|binding| binding.kind != solid_facts::ast::ImportKind::SideEffect)
        {
            add_symbol(binding.local.span, true);
        }
    }
    for binding in &file.ast.bindings {
        for name in &binding.names {
            add_symbol(name.span, true);
        }
        if let Some(initializer) = &binding.initializer_identifier {
            add_symbol(initializer.span, true);
        }
    }
    for function in &file.ast.functions {
        if let Some(name) = &function.name {
            add_symbol(name.span, true);
        }
        for name in function
            .parameters
            .iter()
            .flat_map(|parameter| &parameter.names)
        {
            add_symbol(name.span, true);
        }
    }
    for export in &file.ast.exports {
        for item in export.specifiers.iter().chain(&export.declarations) {
            add_symbol(item.local.span, true);
            type_descriptor_spans.insert(item.local.span);
        }
    }
    for element in &file.ast.jsx_elements {
        add_symbol(element.name.span, false);
    }
    // Value positions that coerce whatever they receive: an untagged template
    // interpolation stringifies it, a computed member key stringifies it as a
    // property name. Both are positions where handing over an accessor
    // *function* is provably not what the author meant, and the rule that
    // reports it needs the interpolated identifier to resolve to a symbol.
    //
    // Only the dialect whose catalog has that rule asks for them. These are
    // the cheapest demands in the plan individually and the most numerous in
    // template-heavy code, and a v2 project must not pay for a v1 rule: the
    // demand cache keys on the dialect, so each gets its own plan.
    if dialect.vocabulary.version() == solid_dialect::Version::V1 {
        for template in &file.ast.template_literals {
            for interpolated in &template.expressions {
                add_symbol(*interpolated, false);
            }
        }
        for member in &file.ast.members {
            if file.ast.computed_members.contains(&member.span) {
                add_symbol(member.property, false);
            }
        }
    }
    for returned in &file.ast.returns {
        if let Some(callee) = returned.callee
            && let Some(call) = file.ast.calls.iter().find(|call| call.callee == callee)
        {
            let mut planned = demand(typefacts_location(&path, call.span));
            planned.callability = true;
            demands.push(planned);
        }
        if returned.value == solid_facts::ast::ReturnValueKind::Identifier {
            add_symbol(returned.span, false);
        }
    }
    for call in &file.ast.calls {
        for argument in &call.arguments {
            match argument.value {
                solid_facts::ast::ArgumentValueKind::Identifier => {
                    add_symbol(argument.span, false);
                    // A non-callable descriptor discharges unknown callback
                    // escape obligations; callability alone is not enough for
                    // structurally typed object and primitive arguments.
                    type_descriptor_spans.insert(argument.span);
                    async_symbol_spans.insert(argument.span);
                }
                solid_facts::ast::ArgumentValueKind::Function
                | solid_facts::ast::ArgumentValueKind::AsyncFunction => {
                    async_value_spans.push(argument.span);
                }
                _ => {}
            }
        }
    }

    // Member objects are needed to connect reads such as `props.title` and
    // `state.value` to their declarations. Querying only the property token
    // loses that provenance.
    for member in &file.ast.members {
        add_symbol(member.object, false);
    }
    for spread in &file.ast.spreads {
        add_symbol(spread.argument, false);
    }
    for assignment in &file.ast.assignments {
        add_symbol(assignment.target, false);
    }
    for (span, references) in symbol_spans {
        let mut planned = demand(typefacts_location(&path, span)).symbol(references);
        planned.structural_accessor = structural_accessors.contains(&span);
        planned.r#async = async_symbol_spans.contains(&span);
        planned.type_descriptor = type_descriptor_spans.contains(&span);
        planned.callability = type_descriptor_spans.contains(&span);
        planned.reference_space = file.ast.imports.iter().any(|import| {
            import
                .bindings
                .iter()
                .any(|binding| binding.local.span == span)
        });
        planned.runtime_identity = planned.reference_space
            || file.ast.exports.iter().any(|export| {
                export
                    .specifiers
                    .iter()
                    .chain(&export.declarations)
                    .any(|item| item.local.span == span)
            });
        demands.push(planned);
    }

    for location in file.compiler_seed_locations()? {
        demands.push(demand(location).symbol(false));
    }
    for span in &file.ast.awaits {
        demands.push(demand(typefacts_location(&path, *span)).async_context());
    }

    // Async facts are consumed only for function-valued call arguments and
    // functions containing await. Query those exact locations instead of
    // using one call as a whole-file discovery trigger.
    for span in async_value_spans {
        demands.push(demand(typefacts_location(&path, span)).async_context());
    }
    for call in &file.ast.calls {
        let callee = typefacts_location(&path, call.callee);
        let property = callee_property_location(&file.source, &callee);
        let mut planned = demand(callee.clone()).symbol(false);
        // Signature-to-argument mapping is consumed only when a call has an
        // argument to classify, or when cleanup analysis must prove the
        // callability of a returned call. Accessor reads and other ordinary
        // zero-argument calls need only their type descriptor.
        planned.resolved_call =
            !call.arguments.is_empty() || returned_callees.contains(&call.callee);
        planned.query_location = Some(property.clone());
        planned.type_descriptor = call.arguments.is_empty();
        demands.push(planned);
        if property != callee {
            demands.push(demand(property).symbol(false));
        }
    }
    Ok(())
}

fn demand(location: typefacts::Location) -> EntityDemand {
    EntityDemand {
        location,
        query_location: None,
        symbol: false,
        type_descriptor: false,
        resolved_call: false,
        references: false,
        r#async: false,
        structural_accessor: false,
        callability: false,
        reference_space: false,
        runtime_identity: false,
    }
}

trait DemandFlags {
    fn symbol(self, references: bool) -> Self;
    fn async_context(self) -> Self;
}

impl DemandFlags for EntityDemand {
    fn symbol(mut self, references: bool) -> Self {
        self.symbol = true;
        self.references = references;
        self
    }

    fn async_context(mut self) -> Self {
        self.r#async = true;
        self
    }
}

fn stable_deduplicate(demands: &mut Vec<EntityDemand>) {
    demands.sort_by(|left, right| {
        (
            &left.location.path,
            left.location.start_byte,
            left.location.end_byte,
            left.query_location.as_ref().map(|value| value.start_byte),
            left.query_location.as_ref().map(|value| value.end_byte),
        )
            .cmp(&(
                &right.location.path,
                right.location.start_byte,
                right.location.end_byte,
                right.query_location.as_ref().map(|value| value.start_byte),
                right.query_location.as_ref().map(|value| value.end_byte),
            ))
    });
    let mut merged = Vec::<EntityDemand>::with_capacity(demands.len());
    for demand in demands.drain(..) {
        if let Some(current) = merged
            .last_mut()
            .filter(|current| current.location == demand.location)
        {
            if current.query_location.is_none() {
                current.query_location = demand.query_location;
            }
            current.symbol |= demand.symbol;
            current.type_descriptor |= demand.type_descriptor;
            current.resolved_call |= demand.resolved_call;
            current.references |= demand.references;
            current.r#async |= demand.r#async;
            current.structural_accessor |= demand.structural_accessor;
            current.callability |= demand.callability;
            current.reference_space |= demand.reference_space;
            current.runtime_identity |= demand.runtime_identity;
        } else {
            merged.push(demand);
        }
    }
    *demands = merged;
}
