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
    let mut runtime_value_domain_spans = HashSet::new();
    let mut constant_value_spans = HashSet::new();
    let mut array_shape_spans = HashSet::new();
    let mut library_type_spans = HashSet::new();
    let mut async_symbol_spans = HashSet::new();
    let mut async_value_spans = Vec::new();
    // An expression-bodied arrow's return lives on its function fact, so both
    // spellings of a returned call have to reach the callee's resolved-call
    // demand and the call-result demand below; cleanup classification treats
    // `() => make()` and `return make()` identically.
    let returned_callees = file
        .ast
        .returns
        .iter()
        .chain(
            file.ast
                .functions
                .iter()
                .filter_map(|function| function.expression_return.as_ref()),
        )
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
            // `reactive-handler-frozen` (both catalogs) proves the
            // value non-callable. The
            // `on:` namespace form is a handler too.
            let handler = native
                && ((attribute.namespace.is_none() && name.starts_with("on"))
                    || attribute
                        .namespace
                        .is_some_and(|namespace| file.source_text(namespace) == Some("on")));
            if handler {
                add_symbol(expression, false);
                type_descriptor_spans.insert(expression);
                runtime_value_domain_spans.insert(expression);
                array_shape_spans.insert(expression);
                if attribute.runtime_type_escape {
                    let runtime_expression = file.ast.peel_ts_sugar_span(expression);
                    if runtime_expression != expression {
                        add_symbol(runtime_expression, false);
                        type_descriptor_spans.insert(runtime_expression);
                        runtime_value_domain_spans.insert(runtime_expression);
                        array_shape_spans.insert(runtime_expression);
                    }
                }
            }
            // A non-literal `keyed` value on a control-flow component picks
            // the children-callback overload at runtime; source discovery
            // claims the custom-key shape only when the value's demanded
            // type proves callable, and claims nothing otherwise. Literal
            // `true`/`false` values are already classified syntactically.
            if name == "keyed"
                && attribute.namespace.is_none()
                && !element
                    .boolean_properties
                    .iter()
                    .any(|property| property.name == attribute.name)
            {
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
    // Both dialects offer a `prefer-for` rewrite only when the receiver's type
    // proves an actual array; demand it at exactly the call shape the rule
    // fires on.
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
                array_shape_spans.insert(member.object);
            }
        }
    }
    // Cleanup classification asks what a returned call *produces*. That is the
    // `callResultDomain` demand, not `runtimeValueDomain`: a call and its callee
    // share a start byte, so a value-domain fact at a call span describes the
    // callee. The producer matches the call-result demand against a call-like
    // node whose start *and end* bytes are exactly the demanded span, and emits
    // no field when none matches, so the subject can never be the callee.
    //
    // Only the result domain is demanded here. `callability` at a call span
    // answers a question no consumer asks any more — cleanup classification
    // reads the result — and demanding it is actively harmful: callability is
    // consumed through `smallest_contained_callability`, which picks the
    // smallest *contained* entity carrying the fact, so a callability entity on
    // an expression-bodied arrow's own returned call (`(post) => post.f(x)`)
    // sits inside the callback-argument span and outranks the arrow, answering
    // "is `post.f(x)` callable" where the caller asked "is this argument a
    // callable callback". The result domain is invisible to that lookup.
    for call in &file.ast.calls {
        if !returned_callees.contains(&call.callee) {
            continue;
        }
        let mut planned = demand(typefacts_location(&path, call.span));
        planned.call_result_domain = true;
        demands.push(planned);
    }
    for returned in &file.ast.returns {
        if matches!(
            returned.value,
            solid_facts::ast::ReturnValueKind::Identifier
                | solid_facts::ast::ReturnValueKind::Member
                | solid_facts::ast::ReturnValueKind::Other
        ) {
            // Cleanup-return classification resolves the returned entity at
            // `returned.span` — the peeled expression, so `return (value)` and
            // `return value as Cleanup` name the identifier, not the wrapper.
            // The symbol and the value-domain demand must name that same span
            // or the fact never materializes at all.
            add_symbol(returned.span, false);
            runtime_value_domain_spans.insert(returned.span);
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
    // An expression-bodied arrow's return is recorded on its function fact,
    // not in `ast.returns`, so `() => value` needs the same pair of demands
    // at the same span the block-bodied form uses -- including `Other`, whose
    // cleanup classification reads the same value domain. Without it the two
    // spellings of one return answer differently.
    for returned in file
        .ast
        .functions
        .iter()
        .filter_map(|function| function.expression_return.as_ref())
    {
        if matches!(
            returned.value,
            solid_facts::ast::ReturnValueKind::Identifier
                | solid_facts::ast::ReturnValueKind::Member
                | solid_facts::ast::ReturnValueKind::Other
        ) {
            add_symbol(returned.span, false);
            runtime_value_domain_spans.insert(returned.span);
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
                    runtime_value_domain_spans.insert(argument.span);
                    async_symbol_spans.insert(argument.span);
                    // `server-function-rich-argument` asks whether the value is
                    // one of a few standard-library runtime types. That is a
                    // declaration identity, not a rendered name: an alias hides
                    // `Date[]` behind its own spelling, and a user-declared type
                    // can share a global's name. The span already carries a
                    // demand, so this adds a field to an existing entity.
                    if dialect.semantic_demands.server_argument_library_types {
                        library_type_spans.insert(argument.span);
                    }
                }
                solid_facts::ast::ArgumentValueKind::Function
                | solid_facts::ast::ArgumentValueKind::AsyncFunction => {
                    async_value_spans.push(argument.span);
                }
                _ => {}
            }
            // A transparent assertion makes the outer ArgumentValueKind
            // `Other`, so this must sit outside the identifier match arm. The
            // wrapper can change the apparent type at the full argument span
            // without changing the runtime value; retain the peeled value's
            // callability as separate evidence for every escaped argument.
            if argument.runtime_type_escape
                && let Some(value_span) = argument.value_span
            {
                add_symbol(value_span, true);
                type_descriptor_spans.insert(value_span);
                runtime_value_domain_spans.insert(value_span);
            }
            // SC7007 must classify inline values such as `new Date()` as
            // well as identifiers. Demand the compiler's library identities
            // at every non-spread argument in the 2.0 server-function
            // dialect; an empty identity set proves the value is outside the
            // rich transport set, while an absent fact remains explicit.
            //
            // Library identities are the same for every inhabitant of a type,
            // so this demand is insensitive to the *value* written. The
            // remaining three are not: a type descriptor and a constant value
            // both spell the literal out, so demanding them at every argument
            // made `createSignal(0)` -> `createSignal(1)` a project-wide
            // TypeScript-table change that invalidated every late-stage cache.
            // A primitive or nullish literal is already classified by the AST
            // and cannot be a rich transport value except as a regexp, which
            // its library identity still reports -- so those shapes are left
            // out of the value-carrying demands.
            if dialect.semantic_demands.server_argument_library_types && !argument.spread {
                add_symbol(argument.span, false);
                library_type_spans.insert(argument.span);
                if !matches!(
                    argument.runtime_value_kind,
                    solid_facts::ast::RuntimeValueKind::Primitive
                        | solid_facts::ast::RuntimeValueKind::Nullish
                ) {
                    type_descriptor_spans.insert(argument.span);
                    constant_value_spans.insert(argument.span);
                }
            }
            // Solid 2 effect bundles accept a callable `effect` property. An
            // asserted or nullable identifier in that slot cannot be judged
            // from callability alone because callability deliberately skips a
            // nullish union constituent. Demand the complete runtime domain
            // at the exact value occurrence when this argument contains such
            // a statically named property.
            let argument_value = argument.value_span.unwrap_or(argument.span);
            for property in file.ast.object_properties.iter().filter(|property| {
                argument_value.contains(property.span)
                    && file.source_text(property.key) == Some("effect")
            }) {
                add_symbol(property.value, true);
                runtime_value_domain_spans.insert(property.value);
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
        planned.runtime_value_domain = runtime_value_domain_spans.contains(&span);
        planned.constant_value = constant_value_spans.contains(&span);
        planned.array_shape = array_shape_spans.contains(&span);
        planned.library_types = library_type_spans.contains(&span);
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
        // argument to classify, when cleanup analysis must prove the
        // callability of a returned call, or when a computed call needs a
        // validity gate before unresolved runtime dispatch is exposed.
        let computed_dispatch = file
            .ast
            .computed_members
            .binary_search(&file.ast.peel_ts_sugar_span(call.callee))
            .is_ok();
        planned.resolved_call = !call.arguments.is_empty()
            || returned_callees.contains(&call.callee)
            || computed_dispatch;
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
        call_result_domain: false,
        constant_value: false,
        array_shape: false,
        // Required by the pinned Type Facts v3 wire shape; no retained rule
        // requests tuple facts after the handler-policy retirement.
        tuple_shape: false,
        library_types: false,
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
            current.call_result_domain |= demand.call_result_domain;
            current.constant_value |= demand.constant_value;
            current.array_shape |= demand.array_shape;
            current.library_types |= demand.library_types;
            current.reference_space |= demand.reference_space;
            current.runtime_identity |= demand.runtime_identity;
        } else {
            merged.push(demand);
        }
    }
    *demands = merged;
}
