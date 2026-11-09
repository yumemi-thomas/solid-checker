//! ADR 0919: the value a handed container root holds, joined over every route
//! a caller can hand it in, and the standing of a write through it.
//!
//! ADR 0509 left a write through a container a caller handed in (a props
//! member, a `useContext` result, a hook's result, a slot of one) an
//! obligation: the store, if it is one, is out of view. This follows the
//! written chain's member names back to every value that can reach it, field
//! by field: each enumerated JSX site of a props object, each provider of a
//! context, the one return of a hook, a closed literal's slot, an exact
//! `const` alias or destructuring. A route ends at an exact source symbol, at
//! a fresh plain value, or open. Only a store proven on every route makes the
//! write a store write; only plain values on every route make it silent;
//! anything else keeps the obligation.

use super::*;

/// What a handed container holds on every route ([`handed_root`]).
pub(super) enum HandedRoot {
    /// One proven `createStore` store on every route; `root` is the prefix of
    /// the written chain that holds it.
    Store { symbol: crate::SymbolId, root: Span },
    /// A plain value on every route: no proxy can drop the write.
    Plain,
    /// Some route is not proven either way.
    Open,
}

/// How many hops one route follows.
const ROUTE_DEPTH: usize = 16;
/// How many values one root's join reads before it answers open.
const ROUTE_STEPS: usize = 256;

/// Where one route ends.
#[derive(Clone, PartialEq, Eq)]
enum Leaf {
    /// An exact source symbol (an accessor, a store, a setter, an action),
    /// with the member names still to project past it.
    Source(crate::SymbolId, usize),
    /// A fresh plain value: a literal, a function, a `new` object.
    Plain,
    Open,
}

struct Routes<'w, 'c> {
    context: &'w UpstreamCompatContext<'c>,
    steps: usize,
    leaves: Vec<Leaf>,
}

/// ADR 0919: what the container `container` (the object a write writes into,
/// a static member chain) holds on every route.
pub(super) fn handed_root(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    container: Span,
) -> HandedRoot {
    let ast = &file.ast;
    // The chain's prefixes from its root, and the names past it.
    let mut prefixes = vec![ast.peel_ts_sugar_span(container)];
    let mut names = Vec::new();
    while let Some(member) = ast.member_at(*prefixes.last().unwrap_or(&container)) {
        if ast.computed_members.binary_search(&member.span).is_ok()
            || ast.optional_members.binary_search(&member.span).is_ok()
        {
            return HandedRoot::Open;
        }
        let Some(name) = file.source_text(member.property) else {
            return HandedRoot::Open;
        };
        names.push(name.to_owned());
        prefixes.push(ast.peel_ts_sugar_span(member.object));
    }
    names.reverse();
    prefixes.reverse();
    let Some(&root) = prefixes.first() else {
        return HandedRoot::Open;
    };
    let mut routes = Routes {
        context,
        steps: 0,
        leaves: Vec::new(),
    };
    routes.value(file, root, &names, ROUTE_DEPTH);
    let mut store: Option<(crate::SymbolId, usize)> = None;
    let mut plain = false;
    for leaf in &routes.leaves {
        match leaf {
            Leaf::Open => return HandedRoot::Open,
            Leaf::Plain => plain = true,
            Leaf::Source(symbol, rest) => {
                if plain_createstore(context, symbol) {
                    // A store reached with more names to read than the
                    // chain holds: the container is a link inside it.
                    let Some(consumed) = names.len().checked_sub(*rest) else {
                        return HandedRoot::Open;
                    };
                    match &store {
                        Some(found) if *found != (symbol.clone(), consumed) => {
                            return HandedRoot::Open;
                        }
                        _ => store = Some((symbol.clone(), consumed)),
                    }
                } else if *rest == 0 && function_source(context, symbol) {
                    // An accessor, a setter or an action is an ordinary
                    // function: a property written on it lands.
                    plain = true;
                } else {
                    return HandedRoot::Open;
                }
            }
        }
    }
    match (store, plain) {
        (Some((symbol, consumed)), false) => match prefixes.get(consumed) {
            Some(&root) => HandedRoot::Store { symbol, root },
            None => HandedRoot::Open,
        },
        (None, true) => HandedRoot::Plain,
        _ => HandedRoot::Open,
    }
}

/// The key or index the element of `pattern` that binds `local` directly
/// projects: a static key or a position, no default.
fn pattern_slot(file: &FileFacts, pattern: Span, local: Span) -> Option<String> {
    let element = file.ast.pattern_elements.iter().find(|element| {
        element.pattern == pattern && element.target == local && !element.assignment
    })?;
    if element.default.is_some() {
        return None;
    }
    match &element.slot {
        solid_facts::ast::PatternSlot::Key(key) => Some(key.to_string()),
        solid_facts::ast::PatternSlot::Index(index) => Some(index.to_string()),
        _ => None,
    }
}

/// A store `createStore` made from a value (not a derive function, not
/// optimistic, not a projection), found by its own creation, not its type.
fn plain_createstore(context: &UpstreamCompatContext<'_>, symbol: &crate::SymbolId) -> bool {
    context.accessors.contains_key(symbol)
        && context.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store)
        && typed_store_name(context, symbol).is_none()
        && context
            .source_primitives
            .get(symbol)
            .is_some_and(|primitive| primitive.as_str() == "createStore")
        && created_from_value(context, symbol)
}

/// The store `symbol` is slot 0 of a `const` array pattern whose
/// `createStore` call's first argument is no function (a derive's store has
/// a compute window), and its value is not marked raw (ADR 0634).
fn created_from_value(context: &UpstreamCompatContext<'_>, symbol: &crate::SymbolId) -> bool {
    let Some((_, declaration)) = context.accessors.get(symbol) else {
        return false;
    };
    let (Ok(start), Ok(end)) = (
        u32::try_from(declaration.start_byte),
        u32::try_from(declaration.end_byte),
    ) else {
        return false;
    };
    let Some((binding_file, binding, bound)) = context
        .lookup
        .binding_at_reference(declaration.path.as_ref(), Span::new(start, end))
    else {
        return false;
    };
    let slot0 = binding
        .array_slots
        .first()
        .and_then(Option::as_ref)
        .and_then(|slot| context.entities.at(binding_file.path.as_str(), slot.span));
    binding.immutable
        && binding.shape == solid_facts::ast::BindingShape::Array
        && slot0 == Some(&bound)
        && bound == *symbol
        && !context.lookup.store_value_may_be_raw(binding_file, binding)
        && binding
            .call_initializer
            .and_then(|initializer| binding_file.ast.call_at(initializer))
            .and_then(|call| call.arguments.first())
            .is_some_and(|first| {
                !first.spread
                    && !matches!(
                        first.value,
                        ArgumentValueKind::Function
                            | ArgumentValueKind::AsyncFunction
                            | ArgumentValueKind::Identifier
                    )
            })
}

/// A source whose value is a function: a signal accessor, a setter, an action.
fn function_source(context: &UpstreamCompatContext<'_>, symbol: &crate::SymbolId) -> bool {
    context.setters.contains_key(symbol)
        || context.actions.contains_key(symbol)
        || (context.accessors.contains_key(symbol)
            && context.source_kinds.get(symbol) != Some(&ReactiveSourceKind::Store))
}

/// A store setter's symbol, or a value no fact separates from one.
fn may_be_store_setter(context: &UpstreamCompatContext<'_>, symbol: &crate::SymbolId) -> bool {
    match context.setters.get(symbol) {
        Some((_, _, _, kind)) => *kind == ReactiveSourceKind::Store,
        None => !(context.actions.contains_key(symbol) || context.accessors.contains_key(symbol)),
    }
}

impl Routes<'_, '_> {
    fn open(&mut self) {
        self.leaves.push(Leaf::Open);
    }

    /// Whether no visible write can replace member `name` of an object the
    /// project made (ADR 0273's census).
    fn stable(&self, file: &FileFacts, name: &str) -> bool {
        self.context
            .lookup
            .member_stable_in(file.path.as_str(), name)
    }

    /// The value `expression` of `file` holds, projected through `path`.
    fn value(&mut self, file: &FileFacts, expression: Span, path: &[String], depth: usize) {
        self.steps += 1;
        if depth == 0 || self.steps > ROUTE_STEPS {
            return self.open();
        }
        let ast = &file.ast;
        let expression = ast.peel_ts_sugar_span(expression);
        if let Some(member) = ast.member_at(expression) {
            if ast.computed_members.binary_search(&member.span).is_ok()
                || ast.optional_members.binary_search(&member.span).is_ok()
            {
                return self.open();
            }
            let Some(name) = file.source_text(member.property) else {
                return self.open();
            };
            let longer: Vec<String> = std::iter::once(name.to_owned())
                .chain(path.iter().cloned())
                .collect();
            return self.value(file, member.object, &longer, depth - 1);
        }
        if let Some(call) = ast.call_at(expression) {
            return self.call(file, call, path, depth);
        }
        if ast
            .object_get_shapes
            .iter()
            .any(|shape| shape.span == expression)
        {
            return self.literal(file, expression, path, depth);
        }
        if let Some(array) = ast.array_literal_at(expression) {
            let Some((name, rest)) = path.split_first() else {
                return self.leaves.push(Leaf::Plain);
            };
            let Ok(index) = name.parse::<usize>() else {
                return self.open();
            };
            if !self.stable(file, name)
                || array
                    .elements
                    .iter()
                    .take(index + 1)
                    .any(|element| element.kind != solid_facts::ast::ArrayElementKind::Value)
            {
                return self.open();
            }
            return match array.elements.get(index) {
                Some(element) => self.value(file, element.span, rest, depth - 1),
                None => self.open(),
            };
        }
        if ast.function_at(expression).is_some() {
            return if path.is_empty() {
                self.leaves.push(Leaf::Plain)
            } else {
                self.open()
            };
        }
        if bare_root(file, expression) {
            return self.identifier(file, expression, path, depth);
        }
        self.open()
    }

    /// A closed object literal projected through `path`: its own data slot.
    fn literal(&mut self, file: &FileFacts, literal: Span, path: &[String], depth: usize) {
        if depth == 0 {
            return self.open();
        }
        let Some((name, rest)) = path.split_first() else {
            // A fresh object written at its own property, unless a member
            // may set its prototype (an open literal).
            let closed = file
                .ast
                .object_get_shapes
                .iter()
                .any(|shape| shape.span == literal && shape.closed);
            return self
                .leaves
                .push(if closed { Leaf::Plain } else { Leaf::Open });
        };
        let Some(properties) =
            crate::interproc::accessor_provenance::closed_properties(file, literal)
        else {
            return self.open();
        };
        if !self.stable(file, name) {
            return self.open();
        }
        let slot = properties.iter().find(|property| {
            file.source_text(property.key)
                .is_some_and(|key| key.trim_matches(['\'', '"']) == name)
        });
        match slot {
            Some(property) if property.data && !property.method => self.value(
                file,
                property.shorthand_binding.unwrap_or(property.value),
                rest,
                depth - 1,
            ),
            _ => self.open(),
        }
    }

    fn identifier(&mut self, file: &FileFacts, reference: Span, path: &[String], depth: usize) {
        let context = self.context;
        if let Some(symbol) = source_symbol_at(context, file, reference)
            && (context.accessors.contains_key(symbol)
                || context.setters.contains_key(symbol)
                || context.actions.contains_key(symbol))
        {
            return self.leaves.push(Leaf::Source(symbol.clone(), path.len()));
        }
        // A props parameter, or a slot of a destructured one.
        if let Some((function, index, _)) = parameter_slot_at(file, reference) {
            let Some(parameter) = function.parameters.get(index).filter(|_| index == 0) else {
                return self.open();
            };
            let declaration = file
                .ast
                .reference_declaration(reference)
                .unwrap_or(reference);
            if crate::indexes::binding_written(file, declaration) {
                return self.open();
            }
            let (key, path) = match parameter.shape {
                solid_facts::ast::BindingShape::Identifier => {
                    (location(file.path.shared(), declaration), path.to_vec())
                }
                solid_facts::ast::BindingShape::Object => {
                    let Some(slot) = pattern_slot(file, parameter.pattern, declaration) else {
                        return self.open();
                    };
                    (
                        location(file.path.shared(), parameter.pattern),
                        std::iter::once(slot).chain(path.iter().cloned()).collect(),
                    )
                }
                solid_facts::ast::BindingShape::Array => return self.open(),
            };
            return self.props(&key, &path, depth - 1);
        }
        let Some((bound_file, binding, symbol)) = context
            .lookup
            .binding_at_reference(file.path.as_str(), reference)
        else {
            return self.open();
        };
        if !binding.immutable
            || binding
                .names
                .iter()
                .any(|name| crate::indexes::binding_written(bound_file, name.span))
        {
            return self.open();
        }
        let Some(initializer) = binding.initializer else {
            return self.open();
        };
        if binding.shape == solid_facts::ast::BindingShape::Identifier {
            return if binding.names.len() == 1 {
                self.value(bound_file, initializer, path, depth - 1)
            } else {
                self.open()
            };
        }
        let Some(local) = binding
            .names
            .iter()
            .find(|name| context.entities.at(bound_file.path.as_str(), name.span) == Some(&symbol))
        else {
            return self.open();
        };
        let Some(slot) = pattern_slot(bound_file, binding.pattern, local.span) else {
            return self.open();
        };
        let longer: Vec<String> = std::iter::once(slot).chain(path.iter().cloned()).collect();
        self.value(bound_file, initializer, &longer, depth - 1);
    }

    /// A props object, declared at `declaration`, projected through `path`:
    /// what every enumerated JSX site passes its first name, through the
    /// sites that forward an enclosing component's own props whole.
    fn props(&mut self, declaration: &typefacts::Location, path: &[String], depth: usize) {
        let Some((name, rest)) = path.split_first() else {
            // The props object itself is the container: ADR 0509's.
            return self.open();
        };
        self.steps += 1;
        if depth == 0
            || self.steps > ROUTE_STEPS
            || !self
                .context
                .lookup
                .member_stable_in(declaration.path.as_ref(), name)
        {
            return self.open();
        }
        let Some((sources, plain, forwards)) = self
            .context
            .props_reactivity
            .passed_on_every_site(declaration, name)
        else {
            return self.open();
        };
        if plain {
            // A closed literal of functions and primitives, or a function.
            self.leaves.push(if rest.is_empty() {
                Leaf::Plain
            } else {
                Leaf::Open
            });
        }
        for source in sources {
            self.leaves.push(Leaf::Source(source, rest.len()));
        }
        for parent in forwards {
            self.props(&parent, path, depth - 1);
        }
    }

    fn call(
        &mut self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        path: &[String],
        depth: usize,
    ) {
        let context = self.context;
        let lookup = context.lookup;
        if call.construct {
            return if path.is_empty() {
                self.leaves.push(Leaf::Plain)
            } else {
                self.open()
            };
        }
        if let Some(primitive) = lookup.primitive_at_call(file, call.span) {
            let role = context
                .dialect
                .name_of(primitive)
                .and_then(|name| context.dialect.context_role(name));
            return match (role, primitive) {
                (Some(solid_dialect::ContextRole::Read), _) => {
                    self.context_read(file, call, path, depth)
                }
                (_, solid_dialect::Primitive::Omit) => self.omit(file, call, path, depth),
                (_, solid_dialect::Primitive::Merge) => self.merge(file, call, path, depth),
                _ => self.open(),
            };
        }
        // A project function's one return (a hook).
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        let Some((callee_file, function, _)) = called_function(file, call, callee, lookup) else {
            return self.open();
        };
        if function.r#async
            || function.generator
            || !lookup.function_value_is_current(callee_file, function)
        {
            return self.open();
        }
        let (values, single) = returned_expressions(callee_file, function);
        match values.as_slice() {
            [value] if single || function.expression_return.is_some() => {
                self.value(callee_file, *value, path, depth - 1);
            }
            _ => self.open(),
        }
    }

    /// `omit(props, "a", …)`: the source's slot, unless the name is omitted.
    fn omit(
        &mut self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        path: &[String],
        depth: usize,
    ) {
        let Some((name, _)) = path.split_first() else {
            return self.open();
        };
        let Some((source, keys)) = call.arguments.split_first() else {
            return self.open();
        };
        let mut omitted = false;
        for argument in keys {
            match &argument.literal_value {
                solid_facts::ast::ArgumentLiteralFact::String(key) if !argument.spread => {
                    omitted |= key.as_str() == name;
                }
                _ => return self.open(),
            }
        }
        if source.spread || omitted || !self.props_root(file, source.span) {
            return self.open();
        }
        self.value(file, source.span, path, depth - 1);
    }

    /// `merge(a, b, …)`: the slot of the last source that has the name, each
    /// source a closed literal or a props object every site passes it.
    fn merge(
        &mut self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        path: &[String],
        depth: usize,
    ) {
        let Some((name, _)) = path.split_first() else {
            return self.open();
        };
        for source in call.arguments.iter().rev() {
            if source.spread {
                return self.open();
            }
            let span = file.ast.peel_ts_sugar_span(source.span);
            if let Some(properties) =
                crate::interproc::accessor_provenance::closed_properties(file, span)
            {
                let has = properties.iter().any(|property| {
                    file.source_text(property.key)
                        .is_some_and(|key| key.trim_matches(['\'', '"']) == name)
                });
                if has {
                    return self.literal(file, span, path, depth - 1);
                }
                continue;
            }
            if self.props_root(file, span) {
                // `props` has the name on every route only when every site
                // passes it, which [`Self::props`] asks.
                return self.value(file, span, path, depth - 1);
            }
            return self.open();
        }
        self.open()
    }

    /// Whether `span` is exactly a props parameter.
    fn props_root(&self, file: &FileFacts, span: Span) -> bool {
        let span = file.ast.peel_ts_sugar_span(span);
        bare_root(file, span)
            && parameter_slot_at(file, span).is_some_and(|(function, index, slot)| {
                index == 0
                    && slot == ParameterSlot::Plain
                    && function.parameters[0].shape == solid_facts::ast::BindingShape::Identifier
            })
    }

    /// `useContext(Ctx)`: every provider's `value`, and the default, of a
    /// `const` context every reference of which is in its own file and is
    /// its declaration, a provider tag or a read.
    fn context_read(
        &mut self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        path: &[String],
        depth: usize,
    ) {
        let context = self.context;
        let lookup = context.lookup;
        if context.dialect.context_provider_member().is_some()
            || call.arguments.len() != 1
            || call.arguments[0].spread
        {
            return self.open();
        }
        let argument = file.ast.peel_ts_sugar_span(call.arguments[0].span);
        if !bare_root(file, argument) {
            return self.open();
        }
        let Some((context_file, binding, symbol)) =
            lookup.binding_at_reference(file.path.as_str(), argument)
        else {
            return self.open();
        };
        let [name] = binding.names.as_slice() else {
            return self.open();
        };
        if !binding.immutable
            || binding.shape != solid_facts::ast::BindingShape::Identifier
            || crate::indexes::binding_written(context_file, name.span)
        {
            return self.open();
        }
        let Some(created) = binding
            .call_initializer
            .and_then(|initializer| context_file.ast.call_at(initializer))
        else {
            return self.open();
        };
        let creates = lookup
            .primitive_at_call(context_file, created.span)
            .and_then(|primitive| context.dialect.name_of(primitive))
            .and_then(|name| context.dialect.context_role(name))
            == Some(solid_dialect::ContextRole::Create);
        let exported = context_file.ast.exports.iter().any(|export| {
            export
                .specifiers
                .iter()
                .chain(export.declarations.iter())
                .any(|specifier| specifier.local.span == name.span)
        });
        if !creates || exported || created.arguments.iter().any(|argument| argument.spread) {
            return self.open();
        }
        let Some(references) = lookup.symbol_references_if_present(symbol.as_str()) else {
            return self.open();
        };
        let ast = &context_file.ast;
        let mut providers = Vec::new();
        for reference in &references {
            let (Ok(start), Ok(end)) = (
                u32::try_from(reference.start_byte),
                u32::try_from(reference.end_byte),
            ) else {
                return self.open();
            };
            let at = Span::new(start, end);
            if reference.path.as_ref() != context_file.path.as_str() {
                return self.open();
            }
            if at == name.span || ast.type_queries.iter().any(|query| query.contains(at)) {
                continue;
            }
            if let Some(element) = ast
                .jsx_elements
                .iter()
                .find(|element| element.name.span == at)
            {
                providers.push(element);
                continue;
            }
            if ast
                .jsx_elements
                .iter()
                .any(|element| element.closing_name == Some(at))
            {
                continue;
            }
            let read = ast.calls.iter().any(|read| {
                read.arguments.len() == 1
                    && ast.peel_ts_sugar_span(read.arguments[0].span) == at
                    && lookup
                        .primitive_at_call(context_file, read.span)
                        .and_then(|primitive| context.dialect.name_of(primitive))
                        .and_then(|name| context.dialect.context_role(name))
                        == Some(solid_dialect::ContextRole::Read)
            });
            if !read {
                return self.open();
            }
        }
        if providers.is_empty() {
            return self.open();
        }
        for element in providers {
            let mut values = element.attributes.iter().filter(|attribute| {
                context_file.source_text(attribute.local_name) == Some("value")
            });
            match (values.next(), values.next()) {
                (Some(value), None)
                    if element.spreads.is_empty()
                        && value.namespace.is_none()
                        && value.value_kind
                            == solid_facts::ast::JsxAttributeValueKind::Expression =>
                {
                    match value.expression {
                        Some(expression) => self.value(context_file, expression, path, depth - 1),
                        None => self.open(),
                    }
                }
                _ => self.open(),
            }
        }
        // A read with no provider above gets the default, or throws.
        if let Some(default) = created.arguments.first() {
            self.value(context_file, default.span, path, depth - 1);
        }
    }
}

/// ADR 0919: the standing of a write at `site` through a store a caller
/// handed in, judged at the write. The store's own setter may reach the write
/// through any handed value, so every call here that may be a store's setter
/// and is handed a function opens a window ([`write_window_standing`]): a
/// write outside every one, whose stack starts where no setter call can be
/// ([`origin_closed`]), is dropped; anything else is unproven, never exempt
/// (which setter instance a hook's call hands back is not followed).
pub(super) fn handed_store_standing(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    site: Span,
) -> WriteStanding {
    let mut windows = Windows::default();
    for call in &file.ast.calls {
        let handed_function = call.arguments.iter().any(|argument| {
            argument.spread
                || matches!(
                    argument.value,
                    ArgumentValueKind::Identifier
                        | ArgumentValueKind::Function
                        | ArgumentValueKind::AsyncFunction
                        | ArgumentValueKind::Other
                )
        });
        if !handed_function || !callee_may_set_store(file, context, call) {
            continue;
        }
        for argument in &call.arguments {
            let direct = file.ast.peel_ts_sugar_span(argument.span);
            windows.arguments.push(direct);
            for function in file.ast.functions_within(argument.span) {
                windows.regions.push(WriteWindow {
                    function: function.span,
                    direct: function.span == direct,
                });
            }
        }
    }
    let standing = if windows.is_empty() {
        WriteStanding::Dropped
    } else {
        write_window_standing(file, context, &windows, site, WINDOW_DEPTH)
    };
    match standing {
        WriteStanding::Dropped if origin_closed(file, context, site, 4) => WriteStanding::Dropped,
        WriteStanding::Unproven(blocker) => WriteStanding::Unproven(blocker),
        _ => WriteStanding::Unproven(None),
    }
}

/// ADR 0919: whether the callee `callee` is, on every route, a store's
/// setter: a function literal handed to it as its first argument is then the
/// setter's callback, whose parameter is the draft.
pub(super) fn handed_store_setter(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    callee: Span,
) -> bool {
    let mut routes = Routes {
        context,
        steps: 0,
        leaves: Vec::new(),
    };
    routes.value(file, callee, &[], ROUTE_DEPTH);
    !routes.leaves.is_empty()
        && routes.leaves.iter().all(|leaf| {
            matches!(leaf, Leaf::Source(symbol, 0)
                if context.setters.get(symbol).is_some_and(|(_, _, _, kind)| *kind == ReactiveSourceKind::Store))
        })
}

/// Whether `call` may call a store's setter: anything but a dialect
/// primitive, a standard-library call the census does not withhold, a
/// project function, a function literal, or a value every route of which is
/// a source that is no store setter, or a plain value.
fn callee_may_set_store(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    call: &solid_facts::ast::CallFact,
) -> bool {
    let lookup = context.lookup;
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    if call.construct
        || file.ast.function_at(callee).is_some()
        || lookup.primitive_at_call(file, call.span).is_some()
    {
        return false;
    }
    if lookup
        .standard_library_declaration(file, call, typefacts::CallKind::Call)
        .is_some()
    {
        return lookup.standard_call_withheld(file, call.callee);
    }
    if lookup
        .function_called_at(file.path.as_str(), callee)
        .is_some()
    {
        return false;
    }
    let mut routes = Routes {
        context,
        steps: 0,
        leaves: Vec::new(),
    };
    routes.value(file, callee, &[], ROUTE_DEPTH);
    routes.leaves.is_empty()
        || routes.leaves.iter().any(|leaf| match leaf {
            Leaf::Open => true,
            Leaf::Plain => false,
            Leaf::Source(symbol, rest) => *rest != 0 || may_be_store_setter(context, symbol),
        })
}

/// Whether the stack a write at `site` runs on starts where no setter call
/// can open a window around it: climbing the layers that run in place, a
/// fresh stack (a scheduler), a Solid primitive's or a standard call's
/// callback, a JSX attribute's function, module scope, or a named function
/// every reference of which is a render or a call the call graph resolved to
/// it, each call itself such a stack. A function handed to a project
/// function that does not run it in place, or to anything unknown, may be
/// run by a setter.
fn origin_closed(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    site: Span,
    depth: usize,
) -> bool {
    let lookup = context.lookup;
    let ast = &file.ast;
    let mut at = site;
    for _ in 0..32 {
        let Some(function) = crate::owners::owning_function(ast, at) else {
            return true;
        };
        match crate::execution_role::layer_run(file, function, lookup) {
            crate::execution_role::LayerRun::InPlace(call) => {
                at = call;
                continue;
            }
            crate::execution_role::LayerRun::FreshStack => return true,
            crate::execution_role::LayerRun::InPlaceAmongOthers(_)
            | crate::execution_role::LayerRun::Withheld(_) => return false,
            crate::execution_role::LayerRun::Elsewhere => {}
        }
        if let Some((call, _)) = ast
            .arguments_containing(function.span)
            .find(|(call, position)| {
                ast.peel_ts_sugar_span(call.arguments[*position].span) == function.span
            })
        {
            return lookup.primitive_at_call(file, call.span).is_some()
                || (lookup
                    .standard_library_declaration(file, call, typefacts::CallKind::Call)
                    .is_some()
                    && !lookup.standard_call_withheld(file, call.callee));
        }
        if ast.jsx_elements.iter().any(|element| {
            element.attributes.iter().any(|attribute| {
                attribute
                    .expression
                    .is_some_and(|expression| ast.peel_ts_sugar_span(expression) == function.span)
            })
        }) {
            return true;
        }
        let Some(name) = crate::owners::function_binding_name(file, function) else {
            return false;
        };
        if depth == 0 || function.generator {
            return false;
        }
        let path = file.path.as_str();
        let Some(symbol) = lookup.entities().at(path, name.span) else {
            return false;
        };
        let Some(references) = lookup.symbol_references_if_present(symbol.as_str()) else {
            return false;
        };
        // Every reference is a call or a render the call graph resolved to
        // this function, each started where no setter can run it.
        let sites = lookup.function_call_site_references(path, function.span);
        let accounted = references.iter().all(|reference| {
            let (Ok(start), Ok(end)) = (
                u32::try_from(reference.start_byte),
                u32::try_from(reference.end_byte),
            ) else {
                return false;
            };
            let at = Span::new(start, end);
            (reference.path.as_ref() == path
                && (at == name.span || ast.type_queries.iter().any(|query| query.contains(at))))
                || sites.iter().any(|(site_file, site)| {
                    site_file.path.as_str() == reference.path.as_ref() && *site == at
                })
        });
        return accounted
            && sites.iter().all(|(site_file, at)| {
                let site_ast = &site_file.ast;
                // A tag is created where it is written: one inside a function
                // handed to a call that may be a setter renders in its window.
                let rendered = site_ast
                    .jsx_elements
                    .iter()
                    .any(|element| element.name.span == *at || element.closing_name == Some(*at));
                (rendered
                    && !site_ast.calls.iter().any(|call| {
                        call.arguments.iter().any(|argument| {
                            site_ast
                                .functions_within(argument.span)
                                .any(|function| function.span.contains(*at))
                        }) && callee_may_set_store(site_file, context, call)
                    }))
                    || site_ast
                        .calls
                        .iter()
                        .find(|call| site_ast.peel_ts_sugar_span(call.callee) == *at)
                        .is_some_and(|call| origin_closed(site_file, context, call.span, depth - 1))
            });
    }
    false
}
