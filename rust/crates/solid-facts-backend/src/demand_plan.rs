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
    let structural_accessors = structural_accessor_spans(dialect, file);
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
            type_descriptor_spans.insert(name.span);
        }
        // Class and object-literal methods do not have a lexical binding, but
        // their property symbol is the exact identity needed to dispatch a
        // structural member call. Demand the property itself so the
        // TypeScript fact table can retain that identity even when the first
        // call is through an alias or a spread object.
        if let Some(method_name) = &function.method_name {
            add_symbol(method_name.span, true);
        }
        for name in function
            .parameters
            .iter()
            .flat_map(|parameter| &parameter.names)
        {
            add_symbol(name.span, true);
        }
    }
    // Object properties have no lexical binding. Demand each static key so
    // a later exact-value walk can distinguish a function implementation from
    // an unrelated property without falling back to the key spelling.
    for property in &file.ast.object_properties {
        if !property.computed {
            add_symbol(property.key, true);
        }
    }
    // Component identity is a semantic type/usage question. Demand the type
    // of every function-valued binding (including a wrapper call that contains
    // the function) so the IR never has to infer component status from case.
    for binding in &file.ast.bindings {
        let contains_function = binding.initializer_function
            || binding
                .initializer
                .is_some_and(|initializer| file.ast.functions_within(initializer).next().is_some());
        if contains_function {
            for name in &binding.names {
                type_descriptor_spans.insert(name.span);
            }
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
        // Some catalogs judge a dotted tag by its root identifier
        // alone (`Foo` in `<Foo.Bar/>`), so the root span needs its own
        // resolution — the combined span above answers a different
        // question. Demanded only when the catalog carries the rule that
        // reads the answer.
        if dialect.semantic_demands.jsx_member_root_symbols
            && let Some(name) = file.source_text(element.name.span)
            && let Some(dot) = name.find('.')
        {
            let root_length = name[..dot].trim_end().len();
            if root_length > 0 {
                add_symbol(
                    solid_facts::core::Span::new(
                        element.name.span.start,
                        element.name.span.start + u32::try_from(root_length).unwrap_or_default(),
                    ),
                    false,
                );
            }
        }
        // This is intrinsic-element syntax, not component identity. The case
        // check only decides whether DOM event-handler type facts are useful;
        // component classification remains semantic in the reactive IR.
        let native = file
            .source_text(element.name.span)
            .is_some_and(|name| name.starts_with(|c: char| c.is_ascii_lowercase()));
        for attribute in &element.attributes {
            let Some(expression) = attribute.expression else {
                continue;
            };
            let name = file.source_text(attribute.name).unwrap_or_default();
            // Context values are cross-file return-flow evidence. Demand an
            // identifier value even when it is not otherwise a handler or a
            // literal-bearing attribute, so contract generation can connect
            // `useContext(Context)` to the value supplied by its provider.
            if name == "value"
                && file.ast.identifiers.iter().any(|identifier| {
                    identifier.span == expression
                        && identifier.role == solid_facts::ast::IdentifierRole::Reference
                })
            {
                add_symbol(expression, false);
            }
            // A native event-handler value is judged by its resolved type:
            // `expected-function-got-expression` (both catalogs) proves the
            // value non-callable, `event-handlers` and `no-array-handlers`
            // (1.x) prove it statically string/number or array-shaped. The
            // `on:` namespace form is a handler too.
            let handler = native
                && ((attribute.namespace.is_none()
                    && name.starts_with("on")
                    && name.as_bytes().get(2).is_some_and(u8::is_ascii_alphabetic))
                    || attribute
                        .namespace
                        .is_some_and(|namespace| file.source_text(namespace) == Some("on")));
            // The 1.x catalog also recovers string values from literal
            // string *types*: `no-innerhtml`'s allowStatic acceptance, and
            // `jsx-no-script-url` for URL-carrying attributes.
            let static_value_type_required = dialect.semantic_demands.jsx_static_value_types
                && matches!(
                    name,
                    "innerHTML"
                        | "innerhtml"
                        | "href"
                        | "src"
                        | "action"
                        | "formaction"
                        | "formAction"
                        | "data"
                        | "to"
                        | "xlink:href"
                );
            if handler || static_value_type_required {
                add_symbol(expression, false);
                type_descriptor_spans.insert(expression);
            }
        }
    }
    // JSX compilation lowers a context provider to a component call whose
    // props object contains `value`. Preserve the value binding's identity so
    // contract generation can connect the lowered form just as precisely as
    // source JSX.
    for property in &file.ast.object_properties {
        let is_call_property = file.ast.calls.iter().any(|call| {
            call.arguments
                .iter()
                .any(|argument| argument.span.contains(property.span))
        });
        if is_call_property
            && file.source_text(property.key) == Some("value")
            && file.ast.identifiers.iter().any(|identifier| {
                identifier.span == property.value
                    && identifier.role == solid_facts::ast::IdentifierRole::Reference
            })
        {
            add_symbol(property.value, false);
        }
    }
    // Value positions that coerce whatever they receive: an untagged template
    // interpolation stringifies it, a computed member key stringifies it as a
    // property name. Both are positions where handing over an accessor
    // *function* is provably not what the author meant, and
    // `uncalled-accessor` — in both dialects' catalogs — needs the
    // interpolated identifier to resolve to a symbol.
    for template in &file.ast.template_literals {
        for interpolated in &template.expressions {
            add_symbol(*interpolated, false);
        }
    }
    for member in &file.ast.members {
        // `computed_members` is sorted by the extractor.
        if file
            .ast
            .computed_members
            .binary_search(&member.span)
            .is_ok()
        {
            add_symbol(member.property, false);
        }
    }
    // `prefer-for` (1.x) rewrites `.map()` to `<For>` only when the
    // receiver's type proves an actual array; demand it at exactly the call
    // shape the rule fires on.
    if dialect.semantic_demands.array_map_receiver_types {
        for call in &file.ast.calls {
            let is_single_function_map = call.arguments.len() == 1
                && matches!(
                    call.arguments[0].value,
                    solid_facts::ast::ArgumentValueKind::Function
                        | solid_facts::ast::ArgumentValueKind::AsyncFunction
                )
                && file.ast.members.iter().any(|member| {
                    member.span == call.callee && file.source_text(member.property) == Some("map")
                });
            if is_single_function_map {
                let member = file
                    .ast
                    .members
                    .iter()
                    .find(|member| member.span == call.callee)
                    .expect("matched above");
                add_symbol(member.object, false);
                type_descriptor_spans.insert(member.object);
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
        for value in returned
            .elements()
            .iter()
            .flatten()
            .copied()
            .chain(returned.properties().iter().map(|property| property.value))
        {
            if file.ast.identifiers.iter().any(|identifier| {
                identifier.span == value
                    && identifier.role == solid_facts::ast::IdentifierRole::Reference
            }) {
                add_symbol(value, false);
                if let Some(spelling) = file.source_text(value) {
                    for name in file.ast.bindings.iter().flat_map(|binding| &binding.names) {
                        if file.source_text(name.span) == Some(spelling) {
                            add_symbol(name.span, true);
                        }
                    }
                }
            }
        }
    }
    for call in &file.ast.calls {
        for argument in &call.arguments {
            match argument.value {
                solid_facts::ast::ArgumentValueKind::Identifier => {
                    // Interprocedural value flow follows imported aliases and
                    // destructured bindings from callback arguments, so keep
                    // the exact symbol's reference space at this use.
                    add_symbol(argument.span, true);
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
        if file.ast.calls.iter().any(|call| call.span == member.object) {
            add_symbol(member.property, false);
        }
    }
    for spread in &file.ast.spreads {
        add_symbol(spread.argument, false);
    }
    for assignment in &file.ast.assignments {
        add_symbol(assignment.target, false);
        for slot in assignment.array_slots.iter().flatten() {
            add_symbol(*slot, true);
        }
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
        runtime_value_domain: false,
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
            current.runtime_value_domain |= demand.runtime_value_domain;
            current.reference_space |= demand.reference_space;
            current.runtime_identity |= demand.runtime_identity;
        } else {
            merged.push(demand);
        }
    }
    *demands = merged;
}
