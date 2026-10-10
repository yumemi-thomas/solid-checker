//! Closed, read-only project callback-use summaries. Draft against e36502481.
//! Never install a synthetic ExecutionRole: mixed safe roles prove only the
//! absence of a strict-read invocation, not write legality or ownership.

use std::collections::HashSet;

use super::{
    attribute_function_within, callback_runs_outside_tracking, component_prop_literal,
    effect_apply_argument, intrinsic_element,
};
use crate::indexes::{SemanticLookup, binding_written};
use crate::owners::{callback_execution_at_call, containing_ast_function};
use solid_dialect::{Execution, Primitive};
use solid_facts::ast::{BindingShape, FunctionFact, FunctionKind, IdentifierRole};
use solid_facts::core::Span;

const CALLER: u8 = 1;
const TRACKED: u8 = 2;
const UNTRACKED: u8 = 4;
const FRESH: u8 = 8;
const STRICT: u8 = 16;
const SAFE: u8 = TRACKED | UNTRACKED | FRESH;
const MAX_NODES: usize = 128;
const MAX_CONTEXT_DEPTH: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct Slot {
    path: String,
    function: Span,
    parameter: usize,
    // A static path rooted in this exact parameter, not a property symbol
    // shared by every object implementing an interface.
    property: Option<String>,
}

#[derive(Clone)]
enum Uses {
    Caller,
    // Even a resetting boundary must close its carrier's lifetime proof.
    Reset(u8, Box<Uses>),
    Forward(usize, Box<Uses>),
}

impl Uses {
    fn evaluate(&self, closed: &[Option<u8>]) -> Option<u8> {
        match self {
            Self::Caller => Some(CALLER),
            Self::Reset(role, outer) => outer.evaluate(closed).map(|_| *role),
            Self::Forward(target, outer) => {
                let incoming = outer.evaluate(closed)?;
                let modes = closed.get(*target).copied().flatten()?;
                Some((modes & !CALLER) | if modes & CALLER != 0 { incoming } else { 0 })
            }
        }
    }
}

struct Node {
    slot: Slot,
    // None means an incomplete census, storage, return, escape, or unsupported
    // use. No empty/missing fact can accidentally become a closed summary.
    uses: Option<Vec<Uses>>,
    // For a field subject, the other fields of the same object the body
    // invokes. Each runs with the object as `this`, so every site that builds
    // the object must pass an arrow function there (`sibling_is_arrow`).
    invoked_siblings: Vec<String>,
    // The other fields it only reads. An absent one is inherited, and reading
    // it runs a getter if project code installed one on a prototype.
    read_siblings: Vec<String>,
}

#[derive(Default)]
struct Siblings {
    invoked: Vec<String>,
    read: Vec<String>,
}

/// Where a field subject's object is built.
#[derive(Clone, Copy)]
enum Site<'f> {
    Element(&'f solid_facts::ast::JsxElementFact),
    Literal(&'f solid_facts::ast::ArgumentFact),
}

pub(crate) struct ReadConsumerSummaries<'a, 'facts> {
    lookup: &'a SemanticLookup<'facts>,
    nodes: Vec<Node>,
}

impl<'a, 'facts> ReadConsumerSummaries<'a, 'facts> {
    pub(crate) fn new(lookup: &'a SemanticLookup<'facts>) -> Self {
        Self {
            lookup,
            nodes: Vec::new(),
        }
    }

    /// Called after cached read tables have been merged, once per generation.
    /// This proof is consumed only by strict-read projection.
    pub(crate) fn proves_non_strict(&mut self, file: &solid_facts::FileFacts, read: Span) -> bool {
        let Some(literal) = containing_ast_function(&file.ast, read) else {
            return false;
        };
        // An arrow has no `this` of its own. Any other function is called as a
        // member of the object it is stored in, and can hand that receiver to
        // code that later calls it again (`replay = () => this.read()`).
        if literal.kind != FunctionKind::Arrow
            || literal.r#async
            || literal.generator
            || literal.name.is_some()
            || literal.method_name.is_some()
            || !crate::owners::written_directly_in(&file.ast, literal, read)
            || file.ast.identifiers.iter().any(|id| {
                literal.span.contains(id.span)
                    && id.role == IdentifierRole::Reference
                    && matches!(file.source_text(id.span), Some("arguments" | "eval"))
            })
        {
            return false;
        }
        let Some((slot, site)) = self.literal_slot(file, literal.span) else {
            return false;
        };
        let Some(root) = self.intern(slot) else {
            return false;
        };
        if !self.siblings_are_arrows(file, root, site) {
            return false;
        }
        let closed = self.solve();
        closed[root].is_some_and(|modes| modes != 0 && modes & !SAFE == 0)
    }

    fn slot(
        &self,
        file: &solid_facts::FileFacts,
        function: &FunctionFact,
        parameter: usize,
        property: Option<String>,
    ) -> Slot {
        Slot {
            path: file.path.to_string(),
            function: function.span,
            parameter,
            property,
        }
    }

    /// Identifier values only. No method dispatch, union target, signature
    /// declaration, smallest-contained entity, or namespace member fallback.
    fn exact_function(
        &self,
        file: &solid_facts::FileFacts,
        callee: Span,
    ) -> Option<(&'facts solid_facts::FileFacts, &'facts FunctionFact)> {
        let callee = file.ast.peel_ts_sugar_span(callee);
        if !file
            .ast
            .identifiers
            .iter()
            .any(|id| id.span == callee && id.role == IdentifierRole::Reference)
        {
            return None;
        }
        let symbols = self.lookup.callee_symbols(file, callee);
        let [symbol] = symbols.as_slice() else {
            return None;
        };
        let (target_file, target) = self.lookup.function_for_symbol(symbol.as_str())?;
        (target.method_name.is_none() && self.lookup.function_value_is_current(target_file, target))
            .then_some((target_file, target))
    }

    fn component_slot(
        &self,
        file: &solid_facts::FileFacts,
        element: &solid_facts::ast::JsxElementFact,
        prop: &str,
    ) -> Option<Slot> {
        // A `__proto__` attribute is lowered to a literal `__proto__` entry,
        // which installs a prototype on the props object.
        if !element.spreads.is_empty()
            || element.member_object.is_some()
            || intrinsic_element(file, element)
            || element
                .attributes
                .iter()
                .any(|a| a.namespace.is_none() && file.source_text(a.name) == Some("__proto__"))
            || element
                .attributes
                .iter()
                .filter(|a| a.namespace.is_none() && file.source_text(a.name) == Some(prop))
                .count()
                != 1
        {
            return None;
        }
        // A plain JSX tag has its own exact Type Facts value identity. It is
        // not necessarily recorded as an IdentifierReference by the parser.
        let symbol = self
            .lookup
            .entities()
            .at(file.path.as_str(), element.name.span)?;
        let (target_file, target) = self.lookup.function_for_symbol(symbol.as_str())?;
        if target.method_name.is_some()
            || !self.lookup.function_value_is_current(target_file, target)
        {
            return None;
        }
        Some(self.slot(target_file, target, 0, Some(prop.to_string())))
    }

    fn literal_slot<'f>(
        &self,
        file: &'f solid_facts::FileFacts,
        literal: Span,
    ) -> Option<(Slot, Option<Site<'f>>)> {
        if let Some((element, prop)) = component_prop_literal(file, literal) {
            return self
                .component_slot(file, element, prop)
                .map(|slot| (slot, Some(Site::Element(element))));
        }
        let (call, index, property, argument) = file
            .ast
            .arguments_containing(literal)
            .filter_map(|(call, index)| {
                let argument = &call.arguments[index];
                if call.construct || call.arguments.iter().any(|a| a.spread) {
                    return None;
                }
                if file.ast.peel_ts_sugar_span(argument.span) == literal {
                    return Some((call, index, None, argument));
                }
                if !argument.exact_object_literal {
                    return None;
                }
                // A `__proto__` entry installs a prototype whose getters can
                // run when a sibling field is read.
                if argument
                    .literal_members
                    .iter()
                    .any(|m| m.key == "__proto__")
                {
                    return None;
                }
                let member = argument
                    .literal_members
                    .iter()
                    .find(|m| file.ast.peel_ts_sugar_span(m.value) == literal)?;
                Some((call, index, Some(member.key.to_string()), argument))
            })
            .min_by_key(|(call, _, _, _)| call.span.end - call.span.start)?;
        if self.lookup.primitive_at_call(file, call.span).is_some() {
            return None;
        }
        let (target_file, target) = self.exact_function(file, call.callee)?;
        let site = property.is_some().then_some(Site::Literal(argument));
        Some((self.slot(target_file, target, index, property), site))
    }

    /// No sibling field the subject's node uses can reach the subject at
    /// this site. One it invokes runs with the object as `this`, so it must be
    /// an arrow function, written there or named by an exact `const`; an
    /// absent one is refused, since the object inherits it (`valueOf` returns
    /// the object) and project code may have extended `Object.prototype`. One
    /// it only reads must be present.
    fn siblings_are_arrows(
        &self,
        file: &solid_facts::FileFacts,
        node: usize,
        site: Option<Site<'_>>,
    ) -> bool {
        let node = &self.nodes[node];
        if node.invoked_siblings.is_empty() && node.read_siblings.is_empty() {
            return true;
        }
        let Some(site) = site else {
            return false;
        };
        let present = |name: &str| match site {
            Site::Element(element) => element
                .attributes
                .iter()
                .any(|a| a.namespace.is_none() && file.source_text(a.name) == Some(name)),
            Site::Literal(argument) => argument.literal_members.iter().any(|m| m.key == name),
        };
        // A present field is an own property the site wrote: a data property
        // in an exact literal, or a getter of the caller's own expression on
        // the props object. The subject is an arrow, so neither can reach it.
        // An absent one is inherited, and reading it runs any getter project
        // code installed on a prototype, by whatever spelling.
        let reads_inert = node.read_siblings.iter().all(|name| present(name));
        reads_inert
            && node.invoked_siblings.iter().all(|name| {
                let value = match site {
                    Site::Element(element) => {
                        // JSX children are supplied outside the attributes.
                        if name == "children" {
                            return false;
                        }
                        let mut matching = element.attributes.iter().filter(|a| {
                            a.namespace.is_none() && file.source_text(a.name) == Some(name.as_str())
                        });
                        match (matching.next(), matching.next()) {
                            (Some(attribute), None) => attribute.expression,
                            _ => None,
                        }
                    }
                    Site::Literal(argument) => {
                        let mut matching = argument
                            .literal_members
                            .iter()
                            .filter(|m| m.key == name.as_str());
                        match (matching.next(), matching.next()) {
                            (Some(member), None) => Some(member.value),
                            _ => None,
                        }
                    }
                };
                value.is_some_and(|value| {
                    let value = file.ast.peel_ts_sugar_span(value);
                    file.ast
                        .functions
                        .iter()
                        .any(|f| f.span == value && f.kind == FunctionKind::Arrow)
                        || self
                            .exact_function(file, value)
                            .is_some_and(|(_, target)| target.kind == FunctionKind::Arrow)
                })
            })
    }

    fn intern(&mut self, slot: Slot) -> Option<usize> {
        if let Some(index) = self.nodes.iter().position(|n| n.slot == slot) {
            return Some(index);
        }
        if self.nodes.len() >= MAX_NODES {
            return None;
        }
        let index = self.nodes.len();
        // Reserve before following edges. A cycle refers to an unknown node.
        self.nodes.push(Node {
            slot: slot.clone(),
            uses: None,
            invoked_siblings: Vec::new(),
            read_siblings: Vec::new(),
        });
        let file = self.lookup.file_by_path(&slot.path)?;
        let function = file
            .ast
            .functions
            .iter()
            .find(|f| f.span == slot.function)?;
        let mut siblings = Siblings::default();
        let uses = self.census(file, function, &slot, &mut siblings);
        self.nodes[index].uses = uses;
        self.nodes[index].invoked_siblings = siblings.invoked;
        self.nodes[index].read_siblings = siblings.read;
        Some(index)
    }

    fn census(
        &mut self,
        file: &solid_facts::FileFacts,
        function: &FunctionFact,
        slot: &Slot,
        siblings: &mut Siblings,
    ) -> Option<Vec<Uses>> {
        if function.r#async || function.generator || function.rest_parameter {
            return None;
        }
        let parameter = function.parameters.get(slot.parameter)?;
        if parameter.shape != BindingShape::Identifier
            || parameter.initializer.is_some()
            || parameter.names.len() != 1
        {
            return None;
        }
        let name = &parameter.names[0];
        if binding_written(file, name.span) {
            return None;
        }
        // arguments aliases and dynamic evaluation can bypass lexical uses.
        // These spellings only refuse proof; they never confer trust.
        if file.ast.identifiers.iter().any(|id| {
            function.span.contains(id.span)
                && id.role == IdentifierRole::Reference
                && matches!(file.source_text(id.span), Some("arguments" | "eval"))
        }) {
            return None;
        }
        let symbol = self.lookup.entities().at(file.path.as_str(), name.span)?;
        // JSX names are a distinct syntax family. Do not let a missing
        // IdentifierReference row hide invocation as a tag. Dotted tags are
        // conservatively refused in this first cut, even when unrelated.
        if file.ast.jsx_elements.iter().any(|element| {
            function.span.contains(element.span)
                && (element.member_object.is_some()
                    || self
                        .lookup
                        .entities()
                        .at(file.path.as_str(), element.name.span)
                        == Some(symbol)
                    || file.source_text(element.name.span) == file.source_text(name.span))
        }) {
            return None;
        }
        // The local syntax census is authoritative for all runtime references,
        // including references for which no Type Facts demand was made. Union
        // the reference index; mismatching or nonlocal rows refuse closure.
        let mut references: HashSet<Span> = file
            .ast
            .identifiers
            .iter()
            .filter(|id| {
                id.role == IdentifierRole::Reference
                    && file.ast.reference_declaration(id.span) == Some(name.span)
            })
            .map(|id| id.span)
            .collect();
        for reference in self.lookup.symbol_references(symbol.as_str()) {
            if reference.path.as_ref() != file.path.as_str() {
                return None;
            }
            let start = u32::try_from(reference.start_byte).ok()?;
            let end = u32::try_from(reference.end_byte).ok()?;
            let span = Span::new(start, end);
            if span == name.span {
                continue;
            }
            if file.ast.reference_declaration(span) != Some(name.span) {
                return None;
            }
            references.insert(span);
        }
        let mut references: Vec<_> = references.into_iter().collect();
        references.sort_by_key(|span| (span.start, span.end));
        let mut uses = Vec::new();
        for reference in references {
            if !function.body.contains(reference) {
                return None;
            }
            let value = match &slot.property {
                None => reference,
                Some(prop) => {
                    let member = file
                        .ast
                        .members
                        .iter()
                        .find(|m| file.ast.peel_ts_sugar_span(m.object) == reference)?;
                    // No whole-object alias, escape, spread, destructuring,
                    // computed read, or implicit enumeration is admitted.
                    if file.ast.computed_members.contains(&member.span) {
                        return None;
                    }
                    // A field written or deleted after construction is not the
                    // value the site passed: `o.invoke = function () { … }`
                    // or `o.__proto__ = …` can reach the subject.
                    if member_is_written(file, member.span) {
                        return None;
                    }
                    if file.source_text(member.property) != Some(prop.as_str()) {
                        // Another field is independent while it is read.
                        // Invoking it passes the whole object as `this`
                        // (`o.invoke()` can run `this.key()`; an inherited
                        // `o.valueOf()` returns the object), so each site must
                        // pass an arrow there. A dereference is refused.
                        if member_is_dereferenced(file, member.span) {
                            return None;
                        }
                        let name = file.source_text(member.property)?.to_string();
                        let list = if member_is_invoked(file, member.span) {
                            &mut siblings.invoked
                        } else {
                            &mut siblings.read
                        };
                        if !list.contains(&name) {
                            list.push(name);
                        }
                        continue;
                    }
                    member.span
                }
            };
            uses.push(self.value_use(file, function, value)?);
        }
        // Unused values remain open in this first cut; no vacuous clean proof.
        (!uses.is_empty()).then_some(uses)
    }

    fn value_use(
        &mut self,
        file: &solid_facts::FileFacts,
        function: &FunctionFact,
        value: Span,
    ) -> Option<Uses> {
        if let Some(call) = file
            .ast
            .calls
            .iter()
            .find(|c| !c.construct && file.ast.peel_ts_sugar_span(c.callee) == value)
        {
            return self.context(file, function, call.span, 0);
        }
        if let Some((call, index)) = file.ast.arguments_containing(value).find(|(c, index)| {
            !c.construct
                && !c.arguments.iter().any(|a| a.spread)
                && file.ast.peel_ts_sugar_span(c.arguments[*index].span) == value
        }) {
            let outer = self.context(file, function, call.span, 0)?;
            return self.argument_use(file, call, index, outer);
        }
        // Exact project-component forwarding. Intrinsic listeners stay open:
        // their dispatch can be synchronous, even if registration is not.
        if let Some((element, attribute)) = file.ast.jsx_containing(value).find_map(|e| {
            e.attributes
                .iter()
                .find(|a| {
                    a.namespace.is_none()
                        && a.expression
                            .is_some_and(|v| file.ast.peel_ts_sugar_span(v) == value)
                })
                .map(|a| (e, a))
        }) {
            let prop = file.source_text(attribute.name)?;
            let slot = self.component_slot(file, element, prop)?;
            let target = self.intern(slot)?;
            if !self.siblings_are_arrows(file, target, Some(Site::Element(element))) {
                return None;
            }
            // Directly written JSX constructs an element in this function;
            // it does not invoke the callback value while evaluating props.
            if !crate::owners::written_directly_in(&file.ast, function, element.span) {
                return None;
            }
            // The child's body runs in the strict-read window the component
            // entry opens, whatever the carrier the element is built in.
            return Some(Uses::Forward(
                target,
                Box::new(Uses::Reset(STRICT, Box::new(Uses::Caller))),
            ));
        }
        // Stored callback, returned callback, alias, unknown container, .call,
        // .apply, .bind, predicate testing, and unaudited package all fail closed.
        None
    }

    fn argument_use(
        &mut self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        index: usize,
        outer: Uses,
    ) -> Option<Uses> {
        if let Some(primitive) = self.lookup.primitive_at_call(file, call.span) {
            let execution = callback_execution_at_call(file, call, primitive, index, self.lookup)?;
            let semantics =
                self.lookup
                    .dialect
                    .callback_semantics_at(primitive, index, call.arguments.len());
            if self.lookup.dialect.callback_may_open_strict_window(
                primitive,
                index,
                call.arguments.len(),
            ) {
                return None;
            }
            if semantics.tracks_reads {
                return Some(Uses::Reset(TRACKED, Box::new(outer)));
            }
            // Explicit untrack only: no blanket deferred-primitive trust.
            // A strict-read label (`untrack(fn, "name")`) opens a warning
            // window of its own; only the unlabelled form is silent.
            if primitive == Primitive::Untrack
                && call.arguments.len() == 1
                && execution == Execution::Inline
                && callback_runs_outside_tracking(
                    self.lookup.dialect,
                    primitive,
                    index,
                    call.arguments.len(),
                )
                && !self.lookup.dialect.reports_untracked_reads_at(
                    primitive,
                    index,
                    call.arguments.len(),
                )
            {
                return Some(Uses::Reset(UNTRACKED, Box::new(outer)));
            }
            if effect_apply_argument(self.lookup.dialect, primitive, call.arguments.len())
                == Some(index)
            {
                return Some(Uses::Reset(STRICT, Box::new(outer)));
            }
            // A labelled `untrack` opens a warning window of its own, as does
            // every slot the dialect reports untracked reads for: neither
            // inherits the carrier's safety.
            if primitive == Primitive::Untrack && call.arguments.len() != 1 {
                return None;
            }
            if self.lookup.dialect.reports_untracked_reads_at(
                primitive,
                index,
                call.arguments.len(),
            ) {
                return Some(Uses::Reset(STRICT, Box::new(outer)));
            }
            if execution == Execution::Inline {
                return Some(outer);
            }
            return None;
        }
        if let Some(resolved) = self.lookup.resolved_callee_call(file, call.callee) {
            let callable = self
                .lookup
                .entity_at(file.path.as_str(), call.arguments[index].span)
                .and_then(|e| e.callability);
            match crate::runtime_semantics::argument_behavior(resolved, callable, index) {
                Some(crate::runtime_semantics::RuntimeArgumentBehavior::FreshStackCallback)
                    if crate::runtime_semantics::dispatches_through_receiver_then(resolved) =>
                {
                    return None;
                }
                Some(crate::runtime_semantics::RuntimeArgumentBehavior::FreshStackCallback) => {
                    return Some(Uses::Reset(FRESH, Box::new(outer)));
                }
                Some(crate::runtime_semantics::RuntimeArgumentBehavior::InlineCallback) => {
                    return Some(outer);
                }
                // DeferredCallback never proves a fresh stack (ADR 0210).
                Some(_) => return None,
                None => {}
            }
        }
        let (target_file, target) = self.exact_function(file, call.callee)?;
        let slot = self.slot(target_file, target, index, None);
        let target = self.intern(slot)?;
        Some(Uses::Forward(target, Box::new(outer)))
    }

    /// Close every intervening function's carrier, not just the nearest
    /// untrack/memo. An untrack inside a returned reload is still an escape.
    fn context(
        &mut self,
        file: &solid_facts::FileFacts,
        function: &FunctionFact,
        site: Span,
        depth: usize,
    ) -> Option<Uses> {
        if depth >= MAX_CONTEXT_DEPTH {
            return None;
        }
        let owner = file
            .ast
            .functions
            .iter()
            .filter(|f| f.span.contains(site))
            .min_by_key(|f| f.span.end - f.span.start)?;
        if !crate::owners::written_directly_in(&file.ast, owner, site) {
            return None;
        }
        let outer = if owner.span == function.span {
            Uses::Caller
        } else {
            if owner.r#async
                || owner.generator
                || owner.name.is_some()
                || owner.method_name.is_some()
            {
                return None;
            }
            let (call, index) = file
                .ast
                .arguments_containing(owner.span)
                .find(|(c, index)| {
                    !c.construct
                        && !c.arguments.iter().any(|a| a.spread)
                        && file.ast.peel_ts_sugar_span(c.arguments[*index].span) == owner.span
                })?;
            let parent = self.context(file, function, call.span, depth + 1)?;
            self.argument_use(file, call, index, parent)?
        };
        // A callback-valued attribute merely constructs a function. The
        // written-directly check above prevents transferring this region
        // through another function or any nested default parameter.
        if file.ast.any_jsx_containing(site) {
            if file.compiler.tracked_regions.iter().any(|r| {
                r.span.contains(site) && !attribute_function_within(file, r.span, site, self.lookup)
            }) {
                return Some(Uses::Reset(TRACKED, Box::new(outer)));
            }
            return None;
        }
        Some(outer)
    }

    /// Least closure fixpoint. A node closes only after all of its uses and
    /// forwarding dependencies close. Recursive SCCs stay unknown; neither
    /// an empty bottom nor "already visiting" is a clean summary.
    fn solve(&self) -> Vec<Option<u8>> {
        let mut closed = vec![None; self.nodes.len()];
        for _ in 0..=self.nodes.len() {
            let mut changed = false;
            for (index, node) in self.nodes.iter().enumerate() {
                if closed[index].is_some() {
                    continue;
                }
                let Some(uses) = &node.uses else { continue };
                let modes = uses.iter().try_fold(0, |modes, uses| {
                    uses.evaluate(&closed).map(|use_modes| modes | use_modes)
                });
                if let Some(modes) = modes {
                    closed[index] = Some(modes);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        closed
    }
}

fn member_is_invoked(file: &solid_facts::FileFacts, member: Span) -> bool {
    file.ast
        .calls
        .iter()
        .any(|call| file.ast.peel_ts_sugar_span(call.callee) == member)
        || file
            .ast
            .tagged_template_tags
            .iter()
            .any(|tag| file.ast.peel_ts_sugar_span(*tag) == member)
}

fn member_is_dereferenced(file: &solid_facts::FileFacts, member: Span) -> bool {
    file.ast
        .members
        .iter()
        .any(|outer| file.ast.peel_ts_sugar_span(outer.object) == member)
}

fn member_is_written(file: &solid_facts::FileFacts, member: Span) -> bool {
    file.ast
        .assignments
        .iter()
        .map(|assignment| assignment.target)
        .chain(file.ast.iteration_targets.iter().copied())
        .chain(file.ast.deleted_targets.iter().copied())
        .any(|target| target.contains(member))
}
