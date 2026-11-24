//! ADR 0458: whether any execution of the program reaches a site.
//!
//! One question, asked of every violation's site ([`unreached_report_sites`])
//! and of every `onCleanup` handle call (ADR 0456): `Unreached` is a proof
//! that the site never runs, so whatever a rule claims happens there does not
//! happen there; `MayRun` proves nothing and leaves the site's role to decide.
//!
//! A site is unreached when the syntax rules it out
//! ([`solid_facts::ast::AstFacts::unreachable_regions`]: after an
//! unconditional exit, in an arm a literal test rules out), or when the
//! function that runs it never runs. A function never runs when:
//!
//! - it is written in such a region, or in a function that never runs: it is
//!   never created;
//! - it is a member of a class (constructor, method, accessor) that nothing
//!   reaches by name, and the class has no static block, static initializer
//!   or decorator that could reach it through `this`;
//! - it is bound to a name (a `function` declaration, a `const`/`let` holding
//!   it) that is not exported, no module calls direct `eval`, and every
//!   reference to that name is itself unreached, in a type query, the
//!   returned value of an IIFE whose result is discarded, or (ADR 1019)
//!   handed to a standard-library carrier that provably never runs it
//!   (`[].forEach(f)`, `new Set().forEach(f)`, `Array.from([], f)`); and
//!   every call or render the call graph resolves to it is unreached;
//! - it is an anonymous function returned by an IIFE whose result is
//!   discarded, or handed to such a carrier.
//!
//! A function at module scope is entered by modules and hosts this does not
//! follow, and its module-surface rules are ADR 0445's: it may run. Anything
//! else unnamed (a callback, an object member, a JSX attribute value) may run
//! once created. The answer is a least fixed point: a function entered only
//! from itself never runs.
//!
//! ADR 0576: a lazy computation (`createMemo(fn, { lazy: true })`) runs at
//! the first read of its accessor, not at its creation. The report query
//! ([`unreached_report_sites`]) asks a second question beside the first:
//! whether the site runs only in such a computation and no read of it is
//! proven (a direct call of its `const` accessor at a site that may run).
//! That answer is no proof the site never runs (a handed accessor may be
//! read), so no other consumer asks it; a violation there is not proven.
//!
//! ADR 0475: an entry the runtime refuses enters nothing. A creation (a
//! primitive the leaf-owner rules forbid, or a component render, which creates
//! a root first) written in the own extent of an effect apply or a leaf
//! callback throws before its callback or the component body runs, so that
//! callback is never entered from there, and a render or reference there
//! enters no function.

use std::cell::{Cell, RefCell};

use super::*;

/// A function by its file's path and its span.
type FunctionKey = (std::sync::Arc<str>, Span);

/// What [`SiteReach::site`] proves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Reach {
    /// No execution of the program runs the site.
    Unreached(UnreachedReason),
    /// No proof either way.
    MayRun,
    /// ADR 0560: the query stopped at its depth limit or its work bound
    /// before it decided: no proof either way, and a violation here is not
    /// proven either ([`crate::UnreachedSite::undecided`]).
    Undecided,
}

/// Why a site is [`Reach::Unreached`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UnreachedReason {
    /// The site follows an unconditional exit, or sits in an arm a literal
    /// test rules out.
    DeadCode,
    /// ADR 0696: it runs only behind a prop every visible caller leaves out.
    AbsentProp,
    /// The function that runs the site is never created or never entered.
    NeverEntered,
}

/// The query, with a memo of each function's answer. One per question set:
/// the memo is valid for one build's facts.
pub(crate) struct SiteReach<'l, 'a> {
    lookup: &'l SemanticLookup<'a>,
    /// Whether each function may run, by `(path, span)`. A `false` derived
    /// while a function in progress was assumed not to run is not kept.
    functions: RefCell<HashMap<FunctionKey, bool>>,
    visiting: RefCell<HashSet<FunctionKey>>,
    /// Set when a derivation met a function in progress.
    assumed: Cell<bool>,
    /// Whether a refused creation enters nothing (ADR 0475).
    refusals: bool,
    /// ADR 0546: the program has fewer functions than [`DEPTH_LIMIT`], so the
    /// depth-first derivation never stops at the limit and every answer is
    /// the least fixed point, which [`Self::least_fixed_point`] iterates.
    iterates: bool,
    /// While [`Self::least_fixed_point`] runs: its current approximation,
    /// and the functions it has met, in order.
    approximation: RefCell<Option<Approximation<'a>>>,
    /// ADR 0560: the least fixed point may answer a query (not with
    /// [`super::follow_every_path`] on).
    fixes: bool,
    /// ADR 0560: the depth-first derivations this query ran, and whether
    /// one stopped at the depth limit or the walks' bound
    /// ([`super::walk_bound`]): from then on no answer of this query is a
    /// proof ([`Reach::Undecided`]). Only a function whose answer reads
    /// [`DEPTH_LIMIT`] functions or more ([`Self::reads_few`]) and is not
    /// proven to run by the least fixed point derives depth first.
    derivations: Cell<u64>,
    limited: Cell<bool>,
    /// ADR 0576: a lazy computation runs only where a read of it is proven.
    lazy_reads: bool,
    /// ADR 1034: a function written as an object literal's member value that a later spread, computed key or repeated key may
    /// replace runs only where the member is proven to hold it, which this query never proves.
    final_members: bool,
}

impl Drop for SiteReach<'_, '_> {
    fn drop(&mut self) {
        self.lookup
            .note_bounded_walk(self.derivations.get(), self.limited.get());
    }
}

/// A least fixed point being iterated: each function met so far, with
/// whether it runs under the current approximation (all start `false`).
struct Approximation<'a> {
    values: HashMap<FunctionKey, bool>,
    order: Vec<(
        FunctionKey,
        &'a solid_facts::FileFacts,
        &'a solid_facts::ast::FunctionFact,
    )>,
    /// A function met is not in one of the program's own files.
    foreign: bool,
}

/// How deep a chain of functions the query follows before it answers
/// `MayRun` (ADR 0560: and every later answer of the query is undecided).
const DEPTH_LIMIT: usize = 64;

/// ADR 1034: whether `function` is written directly as the value of an object literal's member (a data property's value, a method,
/// a getter or a setter) that a later spread, a later key with no static name, or a later property of the same key in the same
/// literal may replace ([`super::flow_helpers::property_is_final_value`]): the member may never hold it, so nothing that reads the
/// member is proven to enter it, and the literal holds no other reference to it.
fn member_value_may_be_replaced(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    let ast = &file.ast;
    ast.object_properties.iter().any(|property| {
        let written_there = ast.peel_ts_sugar_span(property.value) == function.span
            || function
                .method_name
                .as_ref()
                .is_some_and(|name| name.span == property.key);
        written_there && !super::flow_helpers::property_is_final_value(file, property)
    })
}

/// ADR 1015: whether `span` is written in a part of a flow component's children that a literal prop keeps from ever running
/// ([`SemanticLookup::flow_dead_children`]: `<For each={[]}>`'s row function, `<Repeat count={0}>`'s and `<Show when={false}>`'s children). The element must
/// be the Solid-owned component exactly; anything else, and any other value of the prop, may run.
fn in_dead_flow_children(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    file.ast
        .jsx_elements
        .iter()
        .filter(|element| element.span.contains(span))
        .any(|element| {
            lookup
                .flow_dead_children(file, element)
                .iter()
                .any(|dead| dead.contains(span))
        })
}

/// Whether the syntax of `file`, with the literal flow props of ADR 1015, rules `span` out: after an unconditional exit, in an arm a literal test rules out
/// ([`solid_facts::ast::AstFacts::unreachable_contains`]), or in the children a flow component's literal prop keeps from running.
fn statically_dead(file: &solid_facts::FileFacts, span: Span, lookup: &SemanticLookup<'_>) -> bool {
    file.ast.unreachable_contains(span) || in_dead_flow_children(file, span, lookup)
}

/// ADR 0696 (owner's ruling): a function a component's prop holds, whose every
/// call in the component is [`behind_absent_prop`].
fn prop_runs_only_behind_absent_prop(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    component_prop_invocations(file, function, lookup).is_some_and(|(component, calls)| {
        !calls.is_empty()
            && calls
                .iter()
                .all(|call| behind_absent_prop(component, *call, lookup))
    })
}

/// ADR 0696: `site` runs only when a prop of the component holding it is truthy
/// (`if (p.on)`, `p.on && …`, `p.on ? … : …`), and no caller passes that prop.
fn behind_absent_prop(
    file: &solid_facts::FileFacts,
    site: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    ast.if_regions
        .iter()
        .filter(|region| region.consequent.contains(site))
        .map(|region| region.test)
        .chain(
            ast.logical_expressions
                .iter()
                .filter(|logical| {
                    logical.operator == solid_facts::ast::LogicalOperatorKind::And
                        && logical.right.contains(site)
                })
                .map(|logical| logical.left),
        )
        .chain(
            ast.conditional_expressions
                .iter()
                .filter(|conditional| conditional.consequent.contains(site))
                .map(|conditional| conditional.test),
        )
        .any(|test| prop_never_passed(file, test, lookup))
}

/// Whether `test` is a member of a component's own props parameter, and every
/// reference to the component is a JSX tag in view that passes no such prop.
/// A spread, an unseen caller, a default on the parameter or a write answer no.
fn prop_never_passed(
    file: &solid_facts::FileFacts,
    test: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    let Some(member) = ast
        .member_at(ast.peel_ts_sugar_span(test))
        .filter(|member| !ast.computed_members.contains(&member.span))
    else {
        return false;
    };
    let Some(declared) = ast.reference_declaration(ast.peel_ts_sugar_span(member.object)) else {
        return false;
    };
    let Some(component) = ast.functions.iter().find(|function| {
        function.parameters.first().is_some_and(|parameter| {
            parameter.pattern == declared && parameter.initializer.is_none()
        })
    }) else {
        return false;
    };
    let Some(prop) = file.source_text(member.property) else {
        return false;
    };
    let written = ast.assignments.iter().any(|assignment| {
        assignment.target.contains(member.object) || assignment.target == declared
    });
    if written
        || crate::indexes::binding_written(file, declared)
        || lookup.program_has_eval()
        || !lookup.function_value_is_current(file, component)
        || (!lookup.program_closed && module_exports(file, component, lookup))
    {
        return false;
    }
    let Some(name) = crate::owners::function_binding_name(file, component) else {
        return false;
    };
    let Some(symbol) = lookup.entities().at(file.path.as_str(), name.span) else {
        return false;
    };
    let Some(references) = lookup.symbol_references_if_present(symbol.as_str()) else {
        return false;
    };
    let path = file.path.as_str();
    // Recorded queries (ADR 0713's census): every reference is a site the call
    // graph accounts for, and each site is a tag.
    let accounted = lookup
        .function_call_site_references(path, component.span)
        .into_iter()
        .map(|(caller, callee)| (caller.path.as_str().to_owned(), callee.start, callee.end))
        .collect::<HashSet<_>>();
    let every_reference_a_site = references.iter().all(|reference| {
        let (Ok(start), Ok(end)) = (
            u32::try_from(reference.start_byte),
            u32::try_from(reference.end_byte),
        ) else {
            return false;
        };
        (reference.path.as_ref() == path && start == name.span.start && end == name.span.end)
            || accounted.contains(&(reference.path.as_ref().to_owned(), start, end))
    });
    let sites = lookup.function_call_sites(path, component.span);
    every_reference_a_site
        && !sites.is_empty()
        && sites.iter().all(|(caller, site)| {
            caller
                .ast
                .jsx_elements
                .iter()
                .find(|element| element.name.span == *site)
                .is_some_and(|element| {
                    element.spreads.is_empty()
                        && !(prop == "children" && !element.children.is_empty())
                        && !element.attributes.iter().any(|attribute| {
                            attribute.namespace.is_some()
                                || caller.source_text(attribute.local_name) == Some(prop)
                        })
                })
        })
}

impl<'l, 'a> SiteReach<'l, 'a> {
    pub(crate) fn new(lookup: &'l SemanticLookup<'a>) -> Self {
        Self {
            lookup,
            functions: RefCell::default(),
            visiting: RefCell::default(),
            assumed: Cell::new(false),
            refusals: true,
            iterates: lookup.function_count() < DEPTH_LIMIT && !super::every_path_asked(),
            approximation: RefCell::new(None),
            fixes: !super::every_path_asked(),
            derivations: Cell::new(0),
            limited: Cell::new(false),
            lazy_reads: false,
            final_members: false,
        }
    }

    /// ADR 1034: the query in which a function written as a member value an object literal may replace later
    /// ([`super::flow_helpers::property_is_final_value`] says no) is never entered. Its `Unreached` is "not proven to run", for
    /// the report sites alone, as [`Self::proving_lazy_reads`]' is.
    fn proving_final_members(lookup: &'l SemanticLookup<'a>) -> Self {
        let mut query = Self::new(lookup);
        query.final_members = true;
        query
    }

    /// ADR 0576: the query in which a lazy computation runs only where a
    /// direct read of its accessor may run. Its `Unreached` is "not proven to
    /// run", for the report sites alone.
    fn proving_lazy_reads(lookup: &'l SemanticLookup<'a>) -> Self {
        let mut query = Self::new(lookup);
        query.lazy_reads = true;
        query
    }

    /// The query without ADR 0475's refusals: what tells a report why a site
    /// is unreached.
    fn without_refusals(lookup: &'l SemanticLookup<'a>) -> Self {
        let mut query = Self::new(lookup);
        query.refusals = false;
        query
    }

    fn refused(&self, file: &solid_facts::FileFacts, entry: Span) -> bool {
        self.refusals && refused_entry(file, entry, self.lookup)
    }

    /// Whether any execution reaches `site` of `file`.
    pub(crate) fn site(&self, file: &solid_facts::FileFacts, site: Span) -> Reach {
        let reach = self.site_within(file, site);
        if self.limited.get() {
            return Reach::Undecided;
        }
        reach
    }

    fn site_within(&self, file: &solid_facts::FileFacts, site: Span) -> Reach {
        if statically_dead(file, site, self.lookup) {
            return Reach::Unreached(UnreachedReason::DeadCode);
        }
        // ADR 1021: an instance field's initializer runs only when an instance is made.
        if self.in_field_of_unreached_class(file, site) {
            return Reach::Unreached(UnreachedReason::NeverEntered);
        }
        // ADR 0696: a fact of the whole program.
        if behind_absent_prop(file, site, self.lookup)
            || crate::owners::owning_function(&file.ast, site).is_some_and(|function| {
                prop_runs_only_behind_absent_prop(file, function, self.lookup)
            })
        {
            return Reach::Unreached(UnreachedReason::AbsentProp);
        }
        match crate::owners::owning_function(&file.ast, site) {
            Some(function) if !self.runs(file, function) => {
                Reach::Unreached(UnreachedReason::NeverEntered)
            }
            _ => Reach::MayRun,
        }
    }

    fn reached(&self, file: &solid_facts::FileFacts, site: Span) -> bool {
        self.site_within(file, site) == Reach::MayRun
    }

    /// Whether `function` of `file` may run: `true` where the query did not
    /// decide ([`Reach::Undecided`]).
    pub(crate) fn function_runs(
        &self,
        file: &solid_facts::FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> bool {
        self.runs(file, function) || self.limited.get()
    }

    fn runs(
        &self,
        file: &solid_facts::FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> bool {
        if self.limited.get() {
            return true;
        }
        let key = (file.path.shared(), function.span);
        if let Some(runs) = self.functions.borrow().get(&key) {
            return *runs;
        }
        // ADR 0560: outside a derivation, the least fixed point answers a
        // function whose answer reads fewer than DEPTH_LIMIT functions (a
        // derivation from it never reaches the limit), and any function it
        // proves runs (a derivation answers at least the fixed point).
        let outside = !self.iterates
            && self.fixes
            && self.approximation.borrow().is_none()
            && self.visiting.borrow().is_empty();
        let fixes = self.iterates || (outside && self.reads_few(file, function));
        if outside && !fixes {
            let own = self
                .lookup
                .own_file(file)
                .and_then(|own| Some((own, own.ast.function_at(function.span)?)));
            if let Some((own, function)) = own
                && self.least_fixed_point(key.clone(), own, function, false) == Some(true)
            {
                return true;
            }
        }
        if fixes || self.approximation.borrow().is_some() {
            let own = self
                .lookup
                .own_file(file)
                .and_then(|own| Some((own, own.ast.function_at(function.span)?)));
            if let Some(approximation) = self.approximation.borrow_mut().as_mut() {
                let Some((own, function)) = own else {
                    approximation.foreign = true;
                    return false;
                };
                if let Some(runs) = approximation.values.get(&key) {
                    return *runs;
                }
                approximation.values.insert(key.clone(), false);
                approximation.order.push((key, own, function));
                return false;
            }
            if let Some((own, function)) = own
                && let Some(runs) = self.least_fixed_point(key.clone(), own, function, true)
            {
                return runs;
            }
        }
        // ADR 0560: a derivation that stops at the limit or the bound
        // decides nothing from here on.
        self.derivations.set(self.derivations.get() + 1);
        if self.visiting.borrow().len() >= DEPTH_LIMIT
            || self.derivations.get() > super::walk_bound()
        {
            self.limited.set(true);
            return true;
        }
        if !self.visiting.borrow_mut().insert(key.clone()) {
            // In progress: a function entered only from itself never runs.
            self.assumed.set(true);
            return false;
        }
        let outer_assumed = self.assumed.replace(false);
        self.lookup.count_reach_evaluation();
        let runs = self.runs_uncached(file, function);
        self.visiting.borrow_mut().remove(&key);
        let assumed = self.assumed.get();
        if runs || !assumed {
            self.functions.borrow_mut().insert(key, runs);
        }
        self.assumed.set(outer_assumed || assumed);
        runs
    }

    /// ADR 0560: whether the functions `function`'s answer may read, and
    /// theirs in turn, are fewer than [`DEPTH_LIMIT`], all of the program's
    /// own files. An answer reads the function written around it, and the
    /// function each call, render or reference of its name (or of its
    /// class's name) is written in ([`Self::site`] asks only those): a
    /// derivation from it holds only functions of that set in progress, so
    /// it never stops at the limit, and its answer is the least fixed point
    /// ([`Self::least_fixed_point`]).
    fn reads_few(
        &self,
        file: &solid_facts::FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> bool {
        let Some(own) = self.lookup.own_file(file) else {
            return false;
        };
        let mut seen = HashSet::from_iter([(own.path.shared(), function.span)]);
        let mut stack = vec![(own, function.span)];
        while let Some((file, span)) = stack.pop() {
            let Some(function) = file.ast.function_at(span) else {
                return false;
            };
            let mut sites = Vec::new();
            if let Some(outer) = crate::owners::owning_function(&file.ast, function.span) {
                sites.push((file, outer.span));
            }
            for (caller, callee) in self
                .lookup
                .function_call_sites(file.path.as_str(), function.span)
            {
                let Some(caller) = self.lookup.own_file(caller) else {
                    return false;
                };
                if let Some(owner) = crate::owners::owning_function(&caller.ast, callee) {
                    sites.push((caller, owner.span));
                }
            }
            let declarations = function_declaration_span(file, function).into_iter().chain(
                class_of_member(file, function).and_then(|class| class_declaration(file, class)),
            );
            for declaration in declarations {
                for reference in file.ast.reference_identifiers_to(declaration) {
                    if let Some(owner) = crate::owners::owning_function(&file.ast, reference.span) {
                        sites.push((file, owner.span));
                    }
                }
            }
            for (file, span) in sites {
                if seen.insert((file.path.shared(), span)) {
                    if seen.len() >= DEPTH_LIMIT {
                        return false;
                    }
                    stack.push((file, span));
                }
            }
        }
        true
    }

    /// ADR 0546: whether the function `key` runs, as the least fixed point of
    /// "runs" over the functions its answer reads, from all `false`: each
    /// round evaluates every function not yet known to run against the
    /// current approximation, and the iteration stops at a round that moves
    /// nothing. Every answer it met is kept. The depth-first derivation
    /// answers the same where the depth limit cannot stop it: a function in
    /// progress is assumed not to run, and a derivation that runs needs no
    /// function twice on any branch. `None` (nothing kept) where a function
    /// met is not one of the program's own. ADR 0560: without `exact` (the
    /// derivation may reach its limit), only the functions found to run are
    /// kept: a derivation answers at least the fixed point, so it answers
    /// `true` there too.
    fn least_fixed_point(
        &self,
        key: FunctionKey,
        file: &'a solid_facts::FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
        exact: bool,
    ) -> Option<bool> {
        struct Reset<'r, 'a>(&'r RefCell<Option<Approximation<'a>>>);
        impl Drop for Reset<'_, '_> {
            fn drop(&mut self) {
                self.0.borrow_mut().take();
            }
        }
        *self.approximation.borrow_mut() = Some(Approximation {
            values: HashMap::from_iter([(key.clone(), false)]),
            order: vec![(key.clone(), file, function)],
            foreign: false,
        });
        let reset = Reset(&self.approximation);
        loop {
            let mut moved = false;
            let mut index = 0;
            loop {
                let next = {
                    let approximation = self.approximation.borrow();
                    let approximation = approximation.as_ref()?;
                    let Some((next, file, function)) = approximation.order.get(index) else {
                        break;
                    };
                    (!approximation.values[next]).then(|| (next.clone(), *file, *function))
                };
                index += 1;
                let Some((next, file, function)) = next else {
                    continue;
                };
                self.lookup.count_reach_evaluation();
                if self.runs_uncached(file, function) {
                    let mut approximation = self.approximation.borrow_mut();
                    *approximation.as_mut()?.values.get_mut(&next)? = true;
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        let approximation = self.approximation.borrow_mut().take()?;
        drop(reset);
        if approximation.foreign {
            return None;
        }
        let runs = approximation.values[&key];
        self.functions.borrow_mut().extend(
            approximation
                .values
                .into_iter()
                .filter(|(_, runs)| exact || *runs),
        );
        Some(runs)
    }

    fn runs_uncached(
        &self,
        file: &solid_facts::FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> bool {
        // Never created: written where nothing runs.
        if statically_dead(file, function.span, self.lookup) {
            return false;
        }
        if let Some(class) = class_of_member(file, function)
            && self.class_unreached(file, class)
        {
            return false;
        }
        // ADR 1021: written in an instance field's initializer of a class nothing
        // reaches, or a member of an object literal whose binding nothing uses.
        if self.in_field_of_unreached_class(file, function.span)
            || self.literal_member_unreached(file, function)
        {
            return false;
        }
        let Some(outer) = crate::owners::owning_function(&file.ast, function.span) else {
            // Module scope: entered by modules and hosts this does not follow,
            // unless it is entered only through refused creations (ADR 0475).
            // ADR 0578: a function bound to a name a module does not export
            // (a script's top level is global) is entered only by what names
            // it, as one written in a function is.
            if function_declaration_span(file, function)
                .is_some_and(|declaration| self.binding_unreached(file, function, declaration))
            {
                return false;
            }
            return !self.entered_only_through_refusals(file, function);
        };
        if !self.runs(file, outer) || self.refused(file, function.span) {
            return false;
        }
        // ADR 1068: the apply of an effect whose compute throws before anything else never receives a value, so it never runs.
        if after_throwing_value_callback(file, function, self.lookup) {
            return false;
        }
        if self.lazy_reads
            && let Some(call) = lazy_creation(file, function.span, self.lookup)
            && !self.lazy_read_proven(file, call)
        {
            return false;
        }
        if self.final_members && member_value_may_be_replaced(file, function) {
            return false;
        }
        // ADR 1010: a function literal handed to an exact project helper whose parameter default may run code at that call (and may
        // throw there, before the body): whether the helper's body, and so this function, ever runs is not decided.
        // ADR 1020: a function literal an effect's or a memo's compute returns is a value the runtime hands on (the apply receives
        // it, the memo stores it); it runs only where program code calls that value, which is not read here: undecided.
        // Third cut: a returned value proven never called stays withheld; one that may be called may run, and a site it reaches is
        // marked by [`reachable_from_returned_value`] ([`OwnerAnswers::returned`]).
        if self.handed_past_live_default(file, function)
            || returned_by_primitive_callback(file, function, self.lookup, true)
        {
            self.limited.set(true);
            return true;
        }
        match function_declaration_span(file, function) {
            Some(declaration) => !self.binding_unreached(file, function, declaration),
            // ADR 1019: a literal handed to a carrier that never runs it goes nowhere.
            None => {
                !returned_by_discarded_iife(file, function.span)
                    && !crate::carrier_runs::handed_to_idle_carrier(
                        file,
                        function.span,
                        self.lookup,
                    )
            }
        }
    }

    /// ADR 1010: `function` is written as an argument of a call of an exact project function, and a default of that function's
    /// parameter list may run code at the call ([`super::defaults_run_no_code_at`]).
    fn handed_past_live_default(
        &self,
        file: &solid_facts::FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> bool {
        let lookup = self.lookup;
        file.ast
            .arguments_containing(function.span)
            .filter(|(call, index)| {
                file.ast.peel_ts_sugar_span(call.arguments[*index].span) == function.span
                    && lookup.primitive_at_call(file, call.span).is_none()
            })
            .any(|(call, index)| {
                super::project_callee_at_parameter(file, call, index, lookup).is_some_and(
                    |(callee_file, callee, exact)| {
                        exact && !super::defaults_run_no_code_at(file, call, callee_file, callee)
                    },
                )
            })
    }

    /// ADR 0475: whether a function bound to a name has a call or render the
    /// runtime refuses and nothing else that runs reaches it. A module-scope
    /// function nothing names keeps ADR 0458's answer (it may run).
    fn entered_only_through_refusals(
        &self,
        file: &solid_facts::FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> bool {
        self.refusals
            && self
                .lookup
                .function_call_sites(file.path.as_str(), function.span)
                .into_iter()
                .any(|(caller, callee)| self.refused(caller, callee))
            && function_declaration_span(file, function)
                .is_some_and(|declaration| self.binding_unreached(file, function, declaration))
    }

    /// Whether nothing that runs reaches the function bound at `declaration`.
    fn binding_unreached(
        &self,
        file: &solid_facts::FileFacts,
        function: &solid_facts::ast::FunctionFact,
        declaration: Span,
    ) -> bool {
        if self.lookup.program_has_eval() || flow_binding_exported(file, declaration) {
            return false;
        }
        // Calls and renders the call graph resolves, in any file.
        if self
            .lookup
            .function_call_sites(file.path.as_str(), function.span)
            .into_iter()
            .any(|(caller, callee)| self.reached(caller, callee) && !self.refused(caller, callee))
        {
            return false;
        }
        self.references_unreached(file, declaration)
    }

    /// Whether every reference to the name declared at `declaration` is
    /// unreached, erased (a type query), a value an IIFE returns into a
    /// discarded result, (ADR 1019) handed to a standard-library carrier
    /// that provably never runs it ([`crate::carrier_runs::handed_to_idle_carrier`]),
    /// or (ADR 1021) a value nothing takes (the operand of `void` or `typeof`,
    /// an expression statement, a side of `===`/`!==`: [`inert_reference`]) or
    /// the `extends` clause of a class nothing reaches.
    fn references_unreached(&self, file: &solid_facts::FileFacts, declaration: Span) -> bool {
        file.ast
            .reference_identifiers_to(declaration)
            .all(|reference| {
                file.ast
                    .type_queries
                    .iter()
                    .any(|query| query.contains(reference.span))
                    || returned_by_discarded_iife(file, reference.span)
                    || crate::carrier_runs::handed_to_idle_carrier(
                        file,
                        reference.span,
                        self.lookup,
                    )
                    || inert_reference(file, reference.span)
                    || self.heritage_of_unreached_class(file, reference.span)
                    || (self.lazy_reads
                        && lazy_creation(file, reference.span, self.lookup)
                            .is_some_and(|call| !self.lazy_read_proven(file, call)))
                    || !self.reached(file, reference.span)
                    || self.refused(file, reference.span)
            })
    }

    /// ADR 1021: whether `span` is written in the initializer of a non-static
    /// field of a class nothing reaches ([`Self::class_unreached`]): the
    /// initializer runs only when an instance (of it or of a subclass) is made.
    fn in_field_of_unreached_class(&self, file: &solid_facts::FileFacts, span: Span) -> bool {
        file.ast.classes.iter().any(|class| {
            class.elements.iter().any(|element| {
                element.kind == solid_facts::ast::ClassElementKind::Field
                    && !element.r#static
                    && element.value.is_some_and(|value| value.contains(span))
            }) && self.class_unreached(file, class)
        })
    }

    /// ADR 1021: whether `reference` is, as it stands, the `extends` clause of a
    /// class nothing reaches. Defining the subclass reads the superclass and its
    /// `prototype` (a class's own, never an accessor), and runs none of its code;
    /// only constructing the subclass or reaching its members can.
    fn heritage_of_unreached_class(&self, file: &solid_facts::FileFacts, reference: Span) -> bool {
        file.ast.classes.iter().any(|class| {
            class
                .heritage
                .is_some_and(|heritage| file.ast.peel_ts_sugar_span(heritage) == reference)
                && self.class_unreached(file, class)
        })
    }

    /// ADR 1021: whether `function` is the value of a property (a getter, a
    /// setter, a method, a function-valued property) written directly in an
    /// object literal that is the whole initializer of a name nothing reaches:
    /// the name is not exported, no module calls direct `eval`, and every
    /// reference to it is unreached ([`Self::references_unreached`]). Making the
    /// literal runs none of its members; only an access of the object can.
    fn literal_member_unreached(
        &self,
        file: &solid_facts::FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> bool {
        let ast = &file.ast;
        let Some(property) = ast.object_property_with_value(function.span) else {
            return false;
        };
        // The literal the property is written in: the smallest one around it.
        let Some(literal) = ast
            .object_get_shapes
            .iter()
            .filter(|shape| shape.span.contains(property.span) && shape.span != property.span)
            .min_by_key(|shape| shape.span.end - shape.span.start)
        else {
            return false;
        };
        let Some(binding) = ast.bindings.iter().find(|binding| {
            binding.shape == solid_facts::ast::BindingShape::Identifier
                && binding
                    .initializer
                    .is_some_and(|initializer| ast.peel_ts_sugar_span(initializer) == literal.span)
        }) else {
            return false;
        };
        let Some(name) = binding.names.first() else {
            return false;
        };
        !self.lookup.program_has_eval()
            && !flow_binding_exported(file, name.span)
            && self.references_unreached(file, name.span)
    }

    /// Whether nothing reaches the class's instances or its constructor: its
    /// name is not exported and every reference to it is unreached, and no
    /// code the class runs at its definition (a static block, a static
    /// initializer, a decorator) can reach them through `this`.
    fn class_unreached(
        &self,
        file: &solid_facts::FileFacts,
        class: &solid_facts::ast::ClassFact,
    ) -> bool {
        let runs_at_definition = class.elements.iter().any(|element| {
            element.r#static
                && (element.kind == solid_facts::ast::ClassElementKind::StaticBlock
                    || (element.kind == solid_facts::ast::ClassElementKind::Field
                        && element.value.is_some()))
        });
        // A decorator receives the class or its members. Where it is written
        // (before the class or inside it) is not read: any `@` refuses.
        let scope = crate::owners::owning_function(&file.ast, class.span)
            .map_or(Span::new(0, u32::MAX), |function| function.span);
        let decorated = file.source.as_bytes()
            [scope.start as usize..(scope.end as usize).min(file.source.len())]
            .contains(&b'@');
        if runs_at_definition || decorated || self.lookup.program_has_eval() {
            return false;
        }
        let Some(declaration) = class_declaration(file, class) else {
            return false;
        };
        !flow_binding_exported(file, declaration) && self.references_unreached(file, declaration)
    }
}

/// ADR 1021: whether the reference at `reference` is a value nothing takes
/// ([`solid_facts::ast::AstFacts::inert_operands`]: the operand of `void` or
/// `typeof`, an expression statement's value, a side of `===` or `!==`),
/// through parentheses and TypeScript assertions. No call, construction,
/// coercion or property access of the value happens there.
fn inert_reference(file: &solid_facts::FileFacts, reference: Span) -> bool {
    file.ast
        .inert_operands
        .iter()
        .any(|operand| file.ast.peel_ts_sugar_span(*operand) == reference)
}

/// The declaration of the name a class is bound to: its own name, or the
/// name of an identifier binding whose initializer is the class.
fn class_declaration(
    file: &solid_facts::FileFacts,
    class: &solid_facts::ast::ClassFact,
) -> Option<Span> {
    match &class.name {
        Some(name) => Some(name.span),
        None => {
            let binding = file.ast.bindings.iter().find(|binding| {
                binding.shape == solid_facts::ast::BindingShape::Identifier
                    && binding.initializer.is_some_and(|initializer| {
                        file.ast.peel_ts_sugar_span(initializer) == class.span
                    })
            })?;
            Some(binding.names.first()?.span)
        }
    }
}

/// ADR 0475: whether `entry` enters nothing because the runtime refuses the
/// creation it is part of: it is the tag of a component element, or a callback
/// argument (a literal or a reference) of a primitive the leaf-owner rules
/// forbid, and that element or call is written in the own extent of an effect
/// apply or a leaf callback ([`creation_refused_at`]).
fn refused_entry(file: &solid_facts::FileFacts, entry: Span, lookup: &SemanticLookup<'_>) -> bool {
    let rendered = file
        .ast
        .jsx_elements
        .iter()
        .find(|element| element.name.span == entry || element.closing_name == Some(entry));
    if let Some(element) = rendered {
        return creation_refused_at(file, element.span, lookup);
    }
    file.ast
        .arguments_containing(entry)
        .filter(|(call, index)| file.ast.peel_ts_sugar_span(call.arguments[*index].span) == entry)
        .any(|(call, index)| {
            let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
                return false;
            };
            // ADR 0681: a cleanup registered where no owner exists never runs.
            if lookup
                .dialect
                .callback_runs_only_in_disposal(primitive, index)
                && in_extent_of(file, call.span, lookup, &|call, primitive, index| {
                    lookup.dialect.apply_never_owned(primitive)
                        && effect_apply_argument(lookup.dialect, primitive, call.arguments.len())
                            == Some(index)
                })
            {
                return true;
            }
            let semantics =
                lookup
                    .dialect
                    .callback_semantics_at(primitive, index, call.arguments.len());
            let callback = lookup
                .dialect
                .callback_runs_in_created_root(primitive, index)
                || (semantics.execution.is_some() && !semantics.requires_return_invocation);
            callback
                && crate::cleanup::creates_forbidden_primitive(
                    lookup.dialect,
                    file,
                    call,
                    primitive,
                )
                && creation_refused_at(file, call.span, lookup)
        })
}

/// Whether a creation at `site` is written in the own extent of a callback
/// whose runs refuse it: an effect apply (rc.17 `PRIMITIVE_IN_EFFECT_CALLBACK`:
/// no owner is current there) or a leaf callback (`PRIMITIVE_IN_FORBIDDEN_SCOPE`,
/// SC3001's owners). Lexical: an `onSettled` called out of band is no leaf at
/// runtime, which costs a violation there, never adds one.
fn creation_refused_at(
    file: &solid_facts::FileFacts,
    site: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    in_extent_of(file, site, lookup, &|call, primitive, index| {
        effect_apply_argument(lookup.dialect, primitive, call.arguments.len()) == Some(index)
            || callback_owner_at_call(file, call, primitive, index, lookup)
                == Some(solid_dialect::CallbackOwner::Leaf)
    })
}

/// Whether `site` is in the own extent of a callback `refuses` names: through
/// the layers that run in place, and short of an await or a possible suspension.
fn in_extent_of(
    file: &solid_facts::FileFacts,
    site: Span,
    lookup: &SemanticLookup<'_>,
    refuses: &dyn Fn(&solid_facts::ast::CallFact, solid_dialect::Primitive, usize) -> bool,
) -> bool {
    // The creation may sit in layers the apply or leaf callback runs in place
    // (an IIFE, a helper calling its callback): rc.17's guard holds for the
    // whole callback run, so those layers are refused too. A layer that also
    // runs elsewhere, or a site past an await or a possible suspension (the
    // guard is off by then), keeps today's answer.
    let mut at = site;
    // Bounded: each step moves to the call that runs the previous function.
    for _ in 0..16 {
        let Some(function) = crate::owners::owning_function(&file.ast, at) else {
            return false;
        };
        if function.generator
            || super::body_roles::follows_dominating_await(file, at)
            || super::body_roles::follows_try_head_await(file, at, lookup)
            || super::body_roles::may_follow_suspension(file, at)
        {
            return false;
        }
        let refused = file
            .ast
            .arguments_containing(function.span)
            .filter(|(call, index)| {
                file.ast.peel_ts_sugar_span(call.arguments[*index].span) == function.span
            })
            .any(|(call, index)| {
                lookup
                    .primitive_at_call(file, call.span)
                    .is_some_and(|primitive| refuses(call, primitive, index))
            });
        if refused {
            return true;
        }
        match super::write_roles::layer_run(file, function, lookup) {
            super::write_roles::LayerRun::InPlace(call) => at = call,
            super::write_roles::LayerRun::InPlaceAmongOthers(_)
            | super::write_roles::LayerRun::FreshStack
            | super::write_roles::LayerRun::Elsewhere
            | super::write_roles::LayerRun::Withheld(_) => return false,
        }
    }
    false
}

/// Whether `span` runs in the body of a class member (a constructor, method
/// or accessor) that some execution may enter: directly, or through the
/// layers that run in place there and the generators stepped at their own
/// call ([`super::in_place_run_site`]). A member's body runs at a
/// construction or a member call, which neither the reachable-call index nor
/// the role proofs follow: a reactive read there is placed nowhere, so it is
/// an obligation, never silence (invariant 2), as for a function handed on
/// unfollowed (ADR 0516). A member of a class nothing reaches
/// ([`SiteReach::function_runs`], ADR 0458) never runs it: no obligation.
pub(crate) fn inside_class_member(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let at = match super::in_place_run_site(file, span, lookup) {
        Some((climb, _)) if climb.bound() => return true,
        Some((climb, _)) => climb.at,
        None => span,
    };
    crate::owners::containing_ast_function(&file.ast, at).is_some_and(|function| {
        class_of_member(file, function).is_some()
            && SiteReach::new(lookup).function_runs(file, function)
            // ADR 1022: placed at its one `new` or receiver call.
            && !construct_only_placed(file, span)
    })
}

/// The flip's shared question (`scripts/flip/PLAN.md`): the site may run, and
/// a layer between it and the code around it runs by a way no call edge
/// follows: a function written as an object member (method, accessor or slot),
/// a class element (a member, a field initializer), a tag function, an array
/// element, or a function handed where the analysis does not follow. Many of
/// these have no call node at all (a getter, `toString`, an iterator, a tag),
/// so the answer comes from what holds the function, not from its calls.
pub(crate) fn unplaced_run(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    unplaced_run_but(file, span, lookup, &|_| false)
}

/// ADR 1065 part 2 (census only): which disjunct of [`unplaced_run`] holds at `span`, asked after it answered true. Never read by an answer.
pub(crate) fn unplaced_layer(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> &'static str {
    let ast = &file.ast;
    if ast
        .classes
        .iter()
        .flat_map(|class| &class.elements)
        .any(|element| element.value.is_some_and(|value| value.contains(span)))
        && !construct_only_placed(file, span)
    {
        return "class-element";
    }
    for function in ast.functions_owning(span) {
        let value = function.span;
        if ast
            .object_properties
            .iter()
            .any(|property| ast.peel_ts_sugar_span(property.value) == value)
        {
            return "object-member";
        }
        if ast.array_literals.iter().any(|literal| {
            ast.array_literal_at(*literal).is_some_and(|array| {
                array
                    .elements
                    .iter()
                    .any(|element| ast.peel_ts_sugar_span(element.span) == value)
            })
        }) {
            return "array-element";
        }
        if crate::owners::function_binding_name(file, function).is_some_and(|name| {
            ast.tagged_template_tags
                .iter()
                .any(|tag| ast.reference_declaration(*tag) == Some(name.span))
        }) {
            return "tag-function";
        }
    }
    if crate::local_access::function_escapes_unfollowed(file, span, lookup) {
        return "escapes-unfollowed";
    }
    "unnamed"
}

/// [`unplaced_run`], a function written as an object member that `placed` answers for left out (ADR 1009: the caller proves
/// where such a member runs). Every other layer is asked as before.
pub(crate) fn unplaced_run_but(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    placed: &dyn Fn(&solid_facts::ast::FunctionFact) -> bool,
) -> bool {
    if matches!(SiteReach::new(lookup).site(file, span), Reach::Unreached(_)) {
        return false;
    }
    // ADR 1190 (slice 2): a site in a component prop literal ADR 1171 proves run by the child's calls runs at those calls: it is
    // placed exactly where each of them is.
    if let Some(calls) = super::prop_literal_run_calls(file, span, lookup) {
        return calls
            .into_iter()
            .any(|call| unplaced_run_but(file, call, lookup, placed));
    }
    let ast = &file.ast;
    // ADR 1022: a construct-only class's member runs at its one `new` or receiver call.
    (ast.classes
        .iter()
        .flat_map(|class| &class.elements)
        .any(|element| element.value.is_some_and(|value| value.contains(span)))
        && !construct_only_placed(file, span))
        || ast.functions_owning(span).any(|function| {
            let value = function.span;
            (ast.object_properties
                .iter()
                .any(|property| ast.peel_ts_sugar_span(property.value) == value)
                && !placed(function))
                || ast.array_literals.iter().any(|literal| {
                    ast.array_literal_at(*literal).is_some_and(|array| {
                        array
                            .elements
                            .iter()
                            .any(|element| ast.peel_ts_sugar_span(element.span) == value)
                    })
                })
                || crate::owners::function_binding_name(file, function).is_some_and(|name| {
                    ast.tagged_template_tags
                        .iter()
                        .any(|tag| ast.reference_declaration(*tag) == Some(name.span))
                })
        })
        || crate::local_access::function_escapes_unfollowed(file, span, lookup)
}

/// ADR 0976, R6: [`runs_implicitly`], and a generator's body: it runs when its object is stepped, by a call on a member or by an iteration no call shows (a stored generator
/// spread, iterated or destructured). The read after an await asks this ([`crate::static_api::unplaced_member_or_generator_blocker`]); the other rules' member nets do not (no
/// Chrome program shows a generator's body missed there).
pub(crate) fn runs_implicitly_or_generator(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    runs_implicitly(file, span, lookup)
        || (!matches!(SiteReach::new(lookup).site(file, span), Reach::Unreached(_))
            && file
                .ast
                .functions_owning(span)
                .any(|function| function.generator))
}

/// ADR 0963: whether the code at `span` is run by a route no call shows and no walk that follows calls can name, so that the
/// rule asking is not answered by the call it would follow: written in a class element (a constructor, a method, an accessor, a
/// field's initializer, a static block), in a function that is an object literal's accessor or method (a getter or setter, a
/// coercion or iterator-protocol method, a Proxy trap), in a tag function, or in a function a `new` runs. [`unplaced_run`]
/// without the function handed where nothing follows it: that function is run by a call, which the walk of the rule asking
/// follows or answers itself, and a rule whose sites are everywhere (a reactive read) cannot ask for every hand-off.
pub(crate) fn runs_implicitly(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if matches!(SiteReach::new(lookup).site(file, span), Reach::Unreached(_)) {
        return false;
    }
    let ast = &file.ast;
    // ADR 1022: as in [`unplaced_run`].
    (ast.classes
        .iter()
        .flat_map(|class| &class.elements)
        .any(|element| element.value.is_some_and(|value| value.contains(span)))
        && !construct_only_placed(file, span))
        || ast.functions_owning(span).any(|function| {
            let value = function.span;
            ast.object_properties
                .iter()
                .any(|property| !property.data && ast.peel_ts_sugar_span(property.value) == value)
                || stored_in_host_called_slot(file, function)
                || crate::owners::function_binding_name(file, function).is_some_and(|name| {
                    ast.tagged_template_tags
                        .iter()
                        .any(|tag| ast.reference_declaration(*tag) == Some(name.span))
                        || ast.calls.iter().any(|call| {
                            call.construct
                                && ast.reference_declaration(ast.peel_ts_sugar_span(call.callee))
                                    == Some(name.span)
                        })
                })
        })
}

/// The names a function is stored under that the host itself reads by, with no call expression naming it: a coercion (`toString`,
/// `valueOf`, `toJSON`, `toLocaleString`: a template, `+o`, `JSON.stringify`, `String(o)`), the promise protocol (`then`: an `await`,
/// a return from an async function), the iterator protocol (`next`, `return`, `throw`: a `for..of`, a spread, a destructuring), a
/// DOM event listener object (`handleEvent`).
const HOST_CALLED_KEYS: &[&str] = &[
    "toString",
    "toLocaleString",
    "valueOf",
    "toJSON",
    "then",
    "next",
    "return",
    "throw",
    "handleEvent",
];

/// The traps of a `Proxy` handler: each is read by the host when its operation runs on the proxy, whatever the key it is written under
/// elsewhere.
const PROXY_TRAPS: &[&str] = &[
    "get",
    "set",
    "has",
    "deleteProperty",
    "ownKeys",
    "getOwnPropertyDescriptor",
    "defineProperty",
    "getPrototypeOf",
    "setPrototypeOf",
    "isExtensible",
    "preventExtensions",
    "apply",
    "construct",
];

/// What a property key says of the slot, as the facts can read it.
enum SlotKey<'f> {
    /// A name written as an identifier or a literal.
    Name(&'f str),
    /// A computed key no literal decides (`[Symbol.iterator]`, `[key]`): the slot may be one the host reads.
    Unknown,
}

/// The key at `key` (an object literal's property key, or the property of a member expression): its name when it is an identifier or a
/// literal (a string, quotes dropped; a number), `Unknown` for a computed key that is neither.
fn slot_key(file: &solid_facts::FileFacts, key: Span, computed: bool) -> SlotKey<'_> {
    let text = file.source_text(key).map_or("", str::trim);
    let quoted = text.len() >= 2
        && (text.starts_with('"') || text.starts_with('\''))
        && text.ends_with(&text[..1]);
    if quoted {
        return SlotKey::Name(&text[1..text.len() - 1]);
    }
    if !computed
        || (!text.is_empty()
            && text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'.'))
    {
        return SlotKey::Name(text);
    }
    SlotKey::Unknown
}

/// Batch 10c (ADR 0963, B2): whether `function` is stored where the host runs it by itself and no call or access shows it, written as a
/// function value rather than as a method or accessor ([`runs_implicitly`] takes those for any key): the value of an object literal's
/// property under a key the host reads by (a coercion, the promise or iterator protocol, a computed key), as a function written there or a
/// name that is its binding (`{ toString: show }`); the value assigned to such a member (`o.toString = fn`, `K.prototype.toString = fn`,
/// `o[Symbol.iterator] = fn`); a trap of a `new Proxy(target, handler)` handler; the `get` or `set` of a descriptor handed to
/// `Object.defineProperty`, `Reflect.defineProperty`, `Object.defineProperties` or `Object.create`. A function stored under any other key
/// runs by a call of that slot, which the call-site net asks ([`crate::static_rules`] `unresolved_invocation`).
fn stored_in_host_called_slot(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    let ast = &file.ast;
    let name = crate::owners::function_binding_name(file, function);
    // The value written at `value` is this function, or names the binding it is.
    let holds = |value: Span| {
        let value = ast.peel_ts_sugar_span(value);
        value == function.span
            || name.is_some_and(|name| ast.reference_declaration(value) == Some(name.span))
    };
    let host_called = |key: &SlotKey<'_>| match key {
        SlotKey::Name(name) => HOST_CALLED_KEYS.contains(name),
        SlotKey::Unknown => true,
    };
    let global = |callee: Span| {
        let mut root = ast.peel_ts_sugar_span(callee);
        while let Some(member) = ast.member_at(root) {
            root = ast.peel_ts_sugar_span(member.object);
        }
        ast.reference_declaration(root).is_none()
    };
    // The handler literal of a `new Proxy(target, handler)`, and the descriptor literals handed to the property-definition functions.
    let in_argument = |property: Span, callee: &str, index: usize, construct: bool| {
        ast.calls.iter().any(|call| {
            call.construct == construct
                && call.static_callee(&file.source) == Some(callee)
                && global(call.callee)
                && call
                    .arguments
                    .get(index)
                    .is_some_and(|argument| !argument.spread && argument.span.contains(property))
        })
    };
    ast.object_properties.iter().any(|property| {
        if !property.data || !holds(property.value) {
            return false;
        }
        let key = slot_key(file, property.key, property.computed);
        let trap = matches!(&key, SlotKey::Name(name) if PROXY_TRAPS.contains(name));
        let descriptor = matches!(&key, SlotKey::Name("get" | "set"));
        host_called(&key)
            || (trap && in_argument(property.span, "Proxy", 1, true))
            || (descriptor
                && (in_argument(property.span, "Object.defineProperty", 2, false)
                    || in_argument(property.span, "Reflect.defineProperty", 2, false)
                    || in_argument(property.span, "Object.defineProperties", 1, false)
                    || in_argument(property.span, "Object.create", 1, false)))
    }) || ast.assignments.iter().any(|assignment| {
        holds(assignment.value_span)
            && ast
                .member_at(ast.peel_ts_sugar_span(assignment.target))
                .is_some_and(|member| {
                    host_called(&slot_key(
                        file,
                        member.property,
                        ast.computed_members.contains(&member.span),
                    ))
                })
    })
}

impl SiteReach<'_, '_> {
    /// ADR 0576: whether a read of the lazy computation `call` creates is
    /// proven: its result is bound to a `const` (the identifier, or the first
    /// slot of an array pattern) that a direct call reads at a site that may
    /// run. A handed, exported or otherwise read accessor proves no read.
    fn lazy_read_proven(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
    ) -> bool {
        let Some(accessor) = computation_accessor(file, call) else {
            return false;
        };
        file.ast
            .reference_identifiers_to(accessor.span)
            .any(|identifier| {
                file.ast.calls.iter().any(|read| {
                    !read.construct && file.ast.peel_ts_sugar_span(read.callee) == identifier.span
                }) && self.reached(file, identifier.span)
            })
    }
}

/// The `const` a computation's accessor is bound to: the identifier, or the
/// first slot of an array pattern.
pub(crate) fn computation_accessor<'f>(
    file: &'f solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
) -> Option<&'f solid_facts::ast::NamedSpan> {
    use solid_facts::ast::BindingShape;
    let binding = file
        .ast
        .bindings
        .iter()
        .find(|binding| binding.call_initializer == Some(call.span) && binding.immutable)?;
    match binding.shape {
        BindingShape::Identifier => binding.names.first(),
        BindingShape::Array => binding.array_slots.first().and_then(Option::as_ref),
        _ => None,
    }
}

/// ADR 1068: `function` is a callback argument of a dialect primitive written after one whose return the runtime hands on
/// (`Dialect::callback_return_is_value`: an effect's compute, whose value the apply receives), and that earlier callback is a
/// function literal whose body starts with a `throw`: every run of it throws before it returns, so no value is ever handed on
/// and `function` is never entered. An `error` arm in a bundle runs on that throw and is not this (only a function written
/// directly as the argument is). `false` for anything else.
fn after_throwing_value_callback(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    ast.arguments_containing(function.span)
        .any(|(call, index)| {
            ast.peel_ts_sugar_span(call.arguments[index].span) == function.span
                && call.arguments[..index]
                    .iter()
                    .all(|argument| !argument.spread)
                && lookup
                    .primitive_at_call(file, call.span)
                    .is_some_and(|primitive| {
                        call.arguments[..index]
                            .iter()
                            .enumerate()
                            .any(|(earlier, argument)| {
                                lookup.dialect.callback_return_is_value(primitive, earlier)
                                    && ast
                                        .function_at(ast.peel_ts_sugar_span(argument.span))
                                        .is_some_and(|value| body_starts_with_throw(file, value))
                            })
                    })
        })
}

/// Whether the first statement of `function`'s block body is a `throw` of its own (no comment or other text before it).
fn body_starts_with_throw(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    if function.expression_body {
        return false;
    }
    let Some(text) = file.source_text(function.body) else {
        return false;
    };
    let Some(open) = text.find('{') else {
        return false;
    };
    let first = text[open + 1..].len() - text[open + 1..].trim_start().len();
    let start = function.body.start + u32::try_from(open + 1 + first).unwrap_or(u32::MAX);
    file.ast.throws.iter().any(|thrown| {
        thrown.span.start == start
            && crate::owners::containing_ast_function(&file.ast, thrown.span)
                .is_some_and(|owner| owner.span == function.span)
    })
}

/// ADR 0576: the call that creates a lazy computation whose callback is the
/// value at `value`: its first argument (no spread) of a primitive whose
/// [`solid_dialect::Dialect::lazy_option_argument`] is not proven absent or
/// `false` (an unknown option may be lazy).
pub(crate) fn lazy_creation<'f>(
    file: &'f solid_facts::FileFacts,
    value: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<&'f solid_facts::ast::CallFact> {
    use solid_dialect::OptionLiteral;
    file.ast
        .arguments_containing(value)
        .filter(|(call, index)| {
            *index == 0
                && !call.arguments[0].spread
                && file.ast.peel_ts_sugar_span(call.arguments[0].span) == value
        })
        .map(|(call, _)| call)
        .find(|call| {
            lookup
                .primitive_at_call(file, call.span)
                .and_then(|primitive| lookup.dialect.lazy_option_argument(primitive))
                .is_some_and(|options| {
                    !matches!(
                        crate::call_option_literal(file, call, options, "lazy"),
                        OptionLiteral::Absent | OptionLiteral::False
                    )
                })
        })
}

/// The class whose constructor, method or accessor `function` is.
fn class_of_member<'f>(
    file: &'f solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> Option<&'f solid_facts::ast::ClassFact> {
    use solid_facts::ast::ClassElementKind as Kind;
    file.ast.classes.iter().find(|class| {
        class.elements.iter().any(|element| {
            matches!(
                element.kind,
                Kind::Constructor | Kind::Method | Kind::Getter | Kind::Setter
            ) && element.value == Some(function.span)
        })
    })
}

/// ADR 1020: `function` is what a callback of a dialect primitive returns (its arrow's expression body, or a `return`'s argument in its
/// own body) at a position whose return the runtime hands on as a value and never calls (`Dialect::callback_return_is_value`: an
/// effect's or a memo's compute), and it is not called where it is written: `createEffect(() => () => { … }, (v) => { … })`. A
/// cleanup an apply returns is called by the runtime, and is not this.
/// `require_never_called` adds the unused-value proof for the reachability gate;
/// the competing-diagnostic query asks only for the returned-value shape.
pub(crate) fn returned_by_primitive_callback(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
    require_never_called: bool,
) -> bool {
    let ast = &file.ast;
    if ast.calls_with_peeled_callee(function.span).next().is_some() {
        return false;
    }
    let returning = ast
        .returns_with_peeled_argument(function.span)
        .filter_map(|returned| crate::owners::containing_body_function(ast, returned.span))
        .chain(ast.functions.iter().filter(|outer| {
            outer.expression_body
                && outer
                    .expression_return
                    .as_ref()
                    .and_then(|value| value.argument)
                    .is_some_and(|argument| ast.peel_ts_sugar_span(argument) == function.span)
        }))
        .collect::<Vec<_>>();
    returning.into_iter().any(|outer| {
        ast.arguments_containing(outer.span).any(|(call, index)| {
            ast.peel_ts_sugar_span(call.arguments[index].span) == outer.span
                && lookup
                    .primitive_at_call(file, call.span)
                    .is_some_and(|primitive| {
                        lookup.dialect.callback_return_is_value(primitive, index)
                    })
                && (!require_never_called || returned_value_never_called(file, call, index))
        })
    })
}

/// ADR 1062: `function` is a value a primitive's compute returns (`Dialect::callback_return_is_value`), not called where written:
/// returned directly ([`returned_by_primitive_callback`]); written inside an expression the compute returns, not as an argument of a
/// call there (`return { run: () => … }`, `return [f]`); or named, with its name in such an expression, not as a callee
/// (`const f = () => …; return f`). A reference from another file is not read (docs/open-work.md).
pub(crate) fn returned_value_holder(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    if ast.calls_with_peeled_callee(function.span).next().is_some() {
        return false;
    }
    if returned_by_primitive_callback(file, function, lookup, false) {
        return true;
    }
    // Whether `span`, written in a function a primitive's compute is, sits in an expression that function returns and is not an
    // argument of a call written there.
    let in_returned_value = |span: Span| {
        let Some(outer) = crate::owners::owning_function(ast, span) else {
            return false;
        };
        let value_returning = ast.arguments_containing(outer.span).any(|(call, index)| {
            ast.peel_ts_sugar_span(call.arguments[index].span) == outer.span
                && lookup
                    .primitive_at_call(file, call.span)
                    .is_some_and(|primitive| {
                        lookup.dialect.callback_return_is_value(primitive, index)
                    })
        });
        if !value_returning {
            return false;
        }
        let returned = ast
            .returns
            .iter()
            .filter(|returned| {
                crate::owners::containing_body_function(ast, returned.span)
                    .is_some_and(|body| body.span == outer.span)
            })
            .filter_map(|returned| returned.argument)
            .chain(
                outer
                    .expression_body
                    .then(|| {
                        outer
                            .expression_return
                            .as_ref()
                            .and_then(|value| value.argument)
                    })
                    .flatten(),
            );
        returned.into_iter().any(|value| {
            value.contains(span)
                && !ast.calls.iter().any(|call| {
                    value.contains(call.span)
                        && call
                            .arguments
                            .iter()
                            .any(|argument| argument.span.contains(span))
                })
        })
    };
    if in_returned_value(function.span) {
        return true;
    }
    function_declaration_span(file, function).is_some_and(|declaration| {
        ast.reference_identifiers_to(declaration).any(|reference| {
            ast.calls_with_peeled_callee(reference.span)
                .next()
                .is_none()
                && in_returned_value(reference.span)
        })
    })
}

/// ADR 1062: whether a value a primitive's compute returns ([`returned_value_holder`]) may run `function`: upward from it through
/// the function written around it, the functions holding its call sites, and those holding a reference to its name in its file.
/// Every path counts, not only the one the reach query took (a second path that runs the site does not hide this one). The walk is
/// bounded; at the bound the answer is yes (fail closed).
fn reachable_from_returned_value(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    const BUDGET: usize = 512;
    let Some(own) = lookup.own_file(file) else {
        return true;
    };
    let Some(start) = own.ast.function_at(function.span) else {
        return true;
    };
    let mut stack = vec![(own, start)];
    let mut seen = crate::HashSet::default();
    while let Some((file, function)) = stack.pop() {
        if !seen.insert((file.path.shared(), function.span)) {
            continue;
        }
        if seen.len() > BUDGET {
            return true;
        }
        if returned_value_holder(file, function, lookup) {
            return true;
        }
        if let Some(outer) = crate::owners::owning_function(&file.ast, function.span) {
            stack.push((file, outer));
        }
        for (caller, callee) in lookup.function_call_sites(file.path.as_str(), function.span) {
            if let Some(holder) = crate::owners::owning_function(&caller.ast, callee) {
                stack.push((caller, holder));
            }
        }
        if let Some(declaration) = function_declaration_span(file, function) {
            for reference in file.ast.reference_identifiers_to(declaration) {
                if let Some(holder) = crate::owners::owning_function(&file.ast, reference.span) {
                    stack.push((file, holder));
                }
            }
        }
    }
    false
}

/// ADR 1020 (fourth cut, Fable's review of 14434c30f): from `function` (the one written around a report site) up through the functions
/// written around it, to the first that a primitive's compute returns as a value: one of them is a callback of a dialect primitive
/// (`createMemo(() => site)`, an effect's apply; `untrack`'s too, which creates nothing: fail closed, no primitive is told apart). Called on an effect apply's stack, the returned value creates
/// that computation first, which throws PRIMITIVE_IN_EFFECT_CALLBACK there, so the callback never runs: no claim at the site is
/// proven. Any primitive counts, whether the release says its creation throws or not.
fn crosses_creation_below_returned_value(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    let mut current = function;
    loop {
        if returned_value_holder(file, current, lookup) {
            return false;
        }
        let handed_to_primitive = ast.calls.iter().any(|call| {
            call.arguments
                .iter()
                .any(|argument| ast.peel_ts_sugar_span(argument.span) == current.span)
                && lookup.primitive_at_call(file, call.span).is_some()
        });
        if handed_to_primitive {
            return true;
        }
        match crate::owners::owning_function(ast, current.span) {
            Some(outer) => current = outer,
            None => return false,
        }
    }
}

/// ADR 1020 (narrowed after #271's smoke census): the value the callback at `index` of `call` returns is provably never called. It
/// goes to the next argument when that is a function literal (an effect's apply): its first parameter is absent, or every reference of
/// it is the operand of `void` and not called (`(v) => { void v; }`). Otherwise it is the call's own result (a memo's value): the call's
/// result is discarded, or it is a `const` whose every reference is called once with no argument and that call's result goes only to
/// `void` or `typeof` (`typeof run()`). Anything else (`v()`, `void v()`, `m()()`, a value handed on) may call it.
fn returned_value_never_called(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    index: usize,
) -> bool {
    let ast = &file.ast;
    // The text of `file` just before `span` (whitespace dropped) ends with a `void` or `typeof` keyword, and just after it is no call,
    // member, index or optional chain.
    let only_read_by_operator = |span: Span| {
        let source = file.source.as_bytes();
        let before = file.source[..span.start as usize].trim_end();
        let keyword = ["void", "typeof"].iter().any(|word| {
            before.ends_with(word)
                && before[..before.len() - word.len()]
                    .chars()
                    .next_back()
                    .map(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
                    != Some(true)
        });
        let after = source[span.end as usize..]
            .iter()
            .find(|byte| !byte.is_ascii_whitespace())
            .copied();
        keyword && !matches!(after, Some(b'(' | b'.' | b'[' | b'?' | b'`'))
    };
    if let Some(apply) = call.arguments.get(index + 1) {
        let Some(apply) = ast.function_at(ast.peel_ts_sugar_span(apply.span)) else {
            return false;
        };
        let Some(parameter) = apply.parameters.first() else {
            return true;
        };
        if parameter.shape != solid_facts::ast::BindingShape::Identifier {
            return false;
        }
        let Some(name) = parameter.names.first() else {
            return false;
        };
        return ast
            .reference_identifiers_to(name.span)
            .all(|reference| only_read_by_operator(reference.span));
    }
    if call.result_discarded {
        return true;
    }
    let Some(binding) = ast.bindings.iter().find(|binding| {
        binding.immutable
            && binding.shape == solid_facts::ast::BindingShape::Identifier
            && binding.names.len() == 1
            && binding
                .initializer
                .is_some_and(|initializer| ast.peel_ts_sugar_span(initializer) == call.span)
    }) else {
        return false;
    };
    ast.reference_identifiers_to(binding.names[0].span)
        .all(|reference| {
            ast.calls.iter().any(|read| {
                !read.construct
                    && read.arguments.is_empty()
                    && ast.peel_ts_sugar_span(read.callee) == reference.span
                    && only_read_by_operator(read.span)
            })
        })
}

fn returned_by_discarded_iife(file: &solid_facts::FileFacts, value: Span) -> bool {
    let ast = &file.ast;
    let returned_by = |returned: &solid_facts::ast::ReturnFact| {
        returned
            .argument
            .is_some_and(|argument| ast.peel_ts_sugar_span(argument) == value)
    };
    let function = ast
        .returns_with_peeled_argument(value)
        .next()
        .and_then(|returned| crate::owners::containing_body_function(ast, returned.span))
        .or_else(|| {
            ast.functions
                .iter()
                .find(|function| function.expression_return.as_ref().is_some_and(returned_by))
        });
    let Some(function) = function else {
        return false;
    };
    !function.generator
        && ast.calls.iter().any(|call| {
            !call.construct
                && call.result_discarded
                && ast.peel_ts_sugar_span(call.callee) == function.span
        })
}

/// The declaration of the name `function` is bound to: a `function`
/// declaration's name, or the name of an identifier binding whose initializer
/// is the function.
pub(super) fn function_declaration_span(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> Option<Span> {
    let ast = &file.ast;
    if function.kind == solid_facts::ast::FunctionKind::Declaration {
        function.name.as_ref().map(|name| name.span)
    } else {
        ast.bindings_with_peeled_initializer(function.span)
            .find(|binding| binding.shape == solid_facts::ast::BindingShape::Identifier)
            .and_then(|binding| binding.names.first().map(|name| name.span))
    }
}

/// The location of every row `program` may report whose site no execution
/// reaches, with why, sorted. The backend makes a violation there
/// uncertifiable (ADR 0458).
///
/// ADR 0912: each site is asked with queries of its own, so its depth limit
/// and work bound ([`super::walk_bound`]) are its own and its answer is a
/// function of the build and the site alone. A site's query derives one
/// answer, whether the function written around it runs ([`SiteReach::site`]:
/// the syntax decides the rest), so that answer, by a query of its own, is
/// one per function ([`Entered::of`]), shared by its sites and computed on
/// the workers.
pub(crate) fn unreached_report_sites(
    program: &crate::Program,
    lookup: &SemanticLookup<'_>,
    cache: Option<&mut ReachChunks>,
    unchanged_paths: &HashSet<String>,
) -> Vec<crate::UnreachedSite> {
    let mut locations = crate::projection::reported_locations(program);
    locations.sort_by(|left, right| crate::location_order(left, right));
    locations.dedup();
    let sites = crate::pipeline::parallel_balanced_results(&locations, |location| {
        let file = lookup.file_by_path(location.path.as_ref())?;
        let (Ok(start), Ok(end)) = (
            u32::try_from(location.start_byte),
            u32::try_from(location.end_byte),
        ) else {
            return None;
        };
        Some(SyntaxReach::of(file, Span::new(start, end), lookup))
    });
    let mut owners = sites
        .iter()
        .flatten()
        .filter_map(|site| match site {
            SyntaxReach::Owned(file, function) => Some((*file, *function)),
            _ => None,
        })
        .collect::<Vec<_>>();
    owners.sort_by(|left, right| {
        (left.0.path.as_str(), left.1.span.start, left.1.span.end).cmp(&(
            right.0.path.as_str(),
            right.1.span.start,
            right.1.span.end,
        ))
    });
    owners.dedup_by(|left, right| std::ptr::eq(left.0, right.0) && left.1.span == right.1.span);
    let answers = owners
        .iter()
        .map(|(file, function)| function_key(file, function))
        .zip(ReachChunks::run(cache, &owners, lookup, unchanged_paths))
        .collect::<HashMap<_, _>>();
    locations
        .iter()
        .zip(sites)
        .filter_map(|(location, site)| {
            let answer = |dead_code, absent_prop, refused_creation, lazy_unread, undecided| {
                Some(crate::UnreachedSite {
                    location: (*location).clone(),
                    dead_code,
                    absent_prop,
                    refused_creation,
                    lazy_unread,
                    member_may_be_replaced: false,
                    returned_value_run: false,
                    returned_value_creation: false,
                    earlier_may_throw: false,
                    undecided,
                })
            };
            // ADR 1204: a site the reach queries leave reached may still follow an evaluation that throws.
            let throw_first = || {
                let file = lookup.file_by_path(location.path.as_ref())?;
                let span = Span::new(
                    u32::try_from(location.start_byte).ok()?,
                    u32::try_from(location.end_byte).ok()?,
                );
                if !earlier_evaluation_may_throw(file, span, lookup) {
                    return None;
                }
                Some(crate::UnreachedSite {
                    earlier_may_throw: true,
                    ..answer(false, false, false, false, false)?
                })
            };
            match site? {
                SyntaxReach::DeadCode => answer(true, false, false, false, false),
                SyntaxReach::AbsentProp => answer(false, true, false, false, false),
                SyntaxReach::Unowned => throw_first(),
                SyntaxReach::Owned(file, function) => {
                    match answers.get(&function_key(file, function))?.site() {
                        SiteAnswer::MayRun => throw_first(),
                        SiteAnswer::NeverEntered { refused_creation } => {
                            answer(false, false, refused_creation, false, false)
                        }
                        SiteAnswer::LazyUnread => answer(false, false, false, true, false),
                        SiteAnswer::MemberMayBeReplaced => Some(crate::UnreachedSite {
                            member_may_be_replaced: true,
                            ..answer(false, false, false, false, false)?
                        }),
                        SiteAnswer::ReturnedValueRun => Some(crate::UnreachedSite {
                            returned_value_run: true,
                            returned_value_creation: crosses_creation_below_returned_value(
                                file, function, lookup,
                            ),
                            ..answer(false, false, false, false, false)?
                        }),
                        // ADR 0560: an undecided query proves nothing; the
                        // violation is not proven either.
                        SiteAnswer::Undecided => answer(false, false, false, false, true),
                    }
                }
            }
        })
        .collect()
}

/// ADR 1204: the site runs only if every evaluation before it, in the function written around it and in each in-place layer
/// whose call runs that function (an IIFE, a closure's one direct call: [`in_place_layer_call`]), completes normally, and one
/// of them visibly may not. By the facts of the file alone, before the site (or the layer's call) in that function's own code
/// (not in a function nested in it):
/// - a `throw` statement;
/// - a Get `o.k` of a `const` bound to an object literal whose `k` is a getter whose own code holds a `throw`;
/// - a call of a `const` function literal or a function declaration of the file whose own code holds a `throw`.
/// A throw a `try` around it catches, where that `try`'s block does not also hold the site, completes the `try` normally.
/// ADR 0279's premise stays: any other call completes normally unless its signature says it may not, so a throw this does not
/// see (a helper's helper, a package) is outside the model.
fn earlier_evaluation_may_throw(
    file: &solid_facts::FileFacts,
    site: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    let own = |function: &solid_facts::ast::FunctionFact, span: Span| {
        crate::owners::containing_ast_function(ast, span)
            .is_some_and(|owner| owner.span == function.span)
    };
    let throws_in = |function: &solid_facts::ast::FunctionFact| {
        ast.throw_statements
            .iter()
            .any(|thrown| own(function, *thrown) && !caught_before(file, *thrown, function.span))
    };
    let local_function = |callee: Span| -> Option<&solid_facts::ast::FunctionFact> {
        let declaration = ast.reference_declaration(ast.peel_ts_sugar_span(callee))?;
        ast.functions
            .iter()
            .find(|function| {
                function.kind == solid_facts::ast::FunctionKind::Declaration
                    && function
                        .name
                        .as_ref()
                        .is_some_and(|name| name.span == declaration)
            })
            .or_else(|| {
                let binding = ast.bindings.iter().find(|binding| {
                    binding.immutable
                        && binding.shape == solid_facts::ast::BindingShape::Identifier
                        && binding.names.iter().any(|name| name.span == declaration)
                })?;
                ast.function_at(ast.peel_ts_sugar_span(binding.initializer?))
            })
    };
    let throwing_getter = |member: &solid_facts::ast::MemberFact| {
        if ast.computed_members.contains(&member.span) {
            return false;
        }
        let Some(key) = file.source_text(member.property) else {
            return false;
        };
        let Some(declaration) = ast.reference_declaration(ast.peel_ts_sugar_span(member.object))
        else {
            return false;
        };
        let Some(literal) = ast
            .bindings
            .iter()
            .find(|binding| {
                binding.immutable
                    && binding.shape == solid_facts::ast::BindingShape::Identifier
                    && binding.names.iter().any(|name| name.span == declaration)
            })
            .and_then(|binding| binding.initializer)
            .map(|value| ast.peel_ts_sugar_span(value))
        else {
            return false;
        };
        ast.object_get_shapes
            .iter()
            .filter(|shape| shape.span == literal)
            .flat_map(|shape| &shape.properties)
            .filter(|property| property.key.as_str() == key)
            .filter_map(|property| property.getter)
            .filter_map(|getter| ast.function_at(getter))
            .any(throws_in)
    };
    let mut at = site;
    for _ in 0..8 {
        let Some(function) = crate::owners::containing_ast_function(ast, at) else {
            return false;
        };
        let before = |span: Span| {
            span.end <= at.start && own(function, span) && !caught_before(file, span, at)
        };
        if ast.throw_statements.iter().any(|thrown| before(*thrown))
            || ast
                .members
                .iter()
                .any(|member| before(member.span) && throwing_getter(member))
            || ast.calls.iter().any(|call| {
                !call.construct
                    && before(call.span)
                    && local_function(call.callee).is_some_and(throws_in)
            })
        {
            return true;
        }
        match in_place_layer_call(file, function, lookup) {
            Some(call) => at = call,
            None => return false,
        }
    }
    false
}

/// ADR 1204: whether a `try` with a `catch` holds `thrown` in its block and does not hold `site` there: the throw lands in the
/// handler, which completes the `try`, and the code after it runs.
fn caught_before(file: &solid_facts::FileFacts, thrown: Span, site: Span) -> bool {
    file.ast
        .try_catches
        .iter()
        .any(|caught| caught.block.contains(thrown) && !caught.block.contains(site))
}

fn function_key(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> FunctionKey {
    (file.path.shared(), function.span)
}

/// ADR 0912: each function's [`OwnerAnswers`], kept across builds while its
/// file is one of the build's `unchanged_paths` held by one slot, with the
/// same source and compiler semantics, the queries would choose the same
/// method and bound ([`Method`]), and every read they recorded answers as it
/// did ([`crate::base::reads`]: memo answers, sections, other files handed
/// out). Shadow and verify-reused modes compute every kept answer again and
/// poison the store on a difference.
#[derive(Default)]
pub(crate) struct ReachChunks {
    kept: HashMap<FunctionKey, KeptAnswers>,
}

struct KeptAnswers {
    source_hash: solid_facts::core::SourceHash,
    compiler: std::sync::Arc<solid_facts::compiler::ExecutionMap>,
    method: Method,
    proof: crate::base::reads::ReadProof,
    answers: OwnerAnswers,
}

/// What a query chooses its method and bound by, outside the reads it
/// records: [`SiteReach::new`]'s whole-program function count, the
/// process's test switches, and whether the program is closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Method {
    iterates: bool,
    fixes: bool,
    bound: u64,
    closed: bool,
}

impl Method {
    fn of(lookup: &SemanticLookup<'_>) -> Self {
        let query = SiteReach::new(lookup);
        Self {
            iterates: query.iterates,
            fixes: query.fixes,
            bound: super::walk_bound(),
            closed: lookup.program_closed,
        }
    }
}

enum ReachOutcome {
    Kept(OwnerAnswers),
    Computed(OwnerAnswers, crate::base::reads::Reads),
}

impl ReachChunks {
    /// Each owner's answers, in order: kept where they hold, computed
    /// (recording what they read) elsewhere. The cache then holds exactly
    /// this build's owners.
    fn run(
        cache: Option<&mut Self>,
        owners: &[(&solid_facts::FileFacts, &solid_facts::ast::FunctionFact)],
        lookup: &SemanticLookup<'_>,
        unchanged_paths: &HashSet<String>,
    ) -> Vec<OwnerAnswers> {
        let Some(cache) = cache else {
            return crate::pipeline::parallel_balanced_results(owners, |(file, function)| {
                OwnerAnswers::of(file, function, lookup)
            });
        };
        let method = Method::of(lookup);
        let store = lookup.memo_store();
        let check = store.check_solvers();
        let kept = &cache.kept;
        let outcomes = crate::pipeline::parallel_balanced_with_state(
            owners,
            || crate::base::reads::PassAnswers::new(lookup),
            |answers, (file, function)| {
                let path = file.path.as_str();
                if unchanged_paths.contains(path)
                    && lookup.path_held_once(path)
                    && let Some(chunk) = kept.get(&function_key(file, function))
                    && chunk.method == method
                    && chunk.source_hash == file.source_hash
                    && (std::sync::Arc::ptr_eq(&chunk.compiler, &file.compiler)
                        || crate::cache::same_compiler_semantics(&chunk.compiler, &file.compiler))
                    && chunk.proof.holds(answers)
                {
                    if check
                        && crate::base::reads::suppress(|| OwnerAnswers::of(file, function, lookup))
                            != chunk.answers
                    {
                        store.poison(&format!(
                            "IR site-reach verify mismatch: the kept answer of the function at {}:{}..{} differs from a fresh one",
                            path, function.span.start, function.span.end
                        ));
                    } else {
                        crate::base::reads::replay(&chunk.proof.reads);
                        return ReachOutcome::Kept(chunk.answers);
                    }
                }
                let (value, reads) =
                    crate::base::reads::record(|| OwnerAnswers::of(file, function, lookup));
                ReachOutcome::Computed(value, reads)
            },
        );
        let mut next = HashMap::default();
        let answers = owners
            .iter()
            .zip(outcomes)
            .map(|((file, function), outcome)| {
                let key = function_key(file, function);
                match outcome {
                    ReachOutcome::Kept(answers) => {
                        if let Some(chunk) = cache.kept.remove(&key) {
                            next.insert(key, chunk);
                        }
                        answers
                    }
                    ReachOutcome::Computed(answers, reads) => {
                        if lookup.path_held_once(file.path.as_str()) {
                            next.insert(
                                key,
                                KeptAnswers {
                                    source_hash: file.source_hash.clone(),
                                    compiler: std::sync::Arc::clone(&file.compiler),
                                    method,
                                    proof: crate::base::reads::ReadProof::new(reads, lookup),
                                    answers,
                                },
                            );
                        }
                        answers
                    }
                }
            })
            .collect();
        cache.kept = next;
        answers
    }
}

/// What a report site's syntax decides before any query runs: the arms of
/// [`SiteReach::site_within`] that read no function's answer.
enum SyntaxReach<'a> {
    DeadCode,
    AbsentProp,
    /// Written in no function: it may run.
    Unowned,
    /// It runs when the function written around it runs.
    Owned(
        &'a solid_facts::FileFacts,
        &'a solid_facts::ast::FunctionFact,
    ),
}

impl<'a> SyntaxReach<'a> {
    fn of(file: &'a solid_facts::FileFacts, site: Span, lookup: &SemanticLookup<'_>) -> Self {
        if statically_dead(file, site, lookup) {
            return Self::DeadCode;
        }
        let owner = crate::owners::owning_function(&file.ast, site);
        if behind_absent_prop(file, site, lookup)
            || owner
                .is_some_and(|function| prop_runs_only_behind_absent_prop(file, function, lookup))
        {
            return Self::AbsentProp;
        }
        owner.map_or(Self::Unowned, |function| Self::Owned(file, function))
    }
}

/// Whether a function runs, by a query of its own (ADR 0912).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Entered {
    Runs,
    Never,
    /// ADR 0560: the query stopped at its depth limit or work bound.
    Undecided,
}

impl Entered {
    fn of<'l, 'a>(
        query: &SiteReach<'l, 'a>,
        file: &solid_facts::FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> Self {
        let runs = query.runs(file, function);
        if query.limited.get() {
            Self::Undecided
        } else if runs {
            Self::Runs
        } else {
            Self::Never
        }
    }
}

/// The answers the report sites of one function read: the query's, then the
/// one without refusals where it never runs, or the lazy-read one where it
/// runs. Each by a fresh query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OwnerAnswers {
    entered: Entered,
    /// [`SiteReach::without_refusals`]' answer, where `entered` is `Never`.
    plain: Option<Entered>,
    /// [`SiteReach::proving_lazy_reads`]' answer, where `entered` is `Runs`.
    lazy: Option<Entered>,
    /// ADR 1034: [`SiteReach::proving_final_members`]' answer, where `entered` is `Runs`.
    members: Option<Entered>,
    /// ADR 1020 (third cut), ADR 1062: `Some(Never)` where `entered` is `Runs` and some function a primitive's compute returns
    /// as a value (directly or by reference) may run the site ([`reachable_from_returned_value`]).
    returned: Option<Entered>,
}

/// What a report site written in a function proves.
enum SiteAnswer {
    MayRun,
    NeverEntered {
        refused_creation: bool,
    },
    LazyUnread,
    /// ADR 1034: the site runs only in a function written as a member value the object literal may replace.
    MemberMayBeReplaced,
    /// ADR 1020 (third cut): the site may run, but it is proven to run only through a call of a value a primitive's compute
    /// returned, which may be on an effect apply's stack.
    ReturnedValueRun,
    Undecided,
}

impl OwnerAnswers {
    fn of(
        file: &solid_facts::FileFacts,
        function: &solid_facts::ast::FunctionFact,
        lookup: &SemanticLookup<'_>,
    ) -> Self {
        let entered = Entered::of(&SiteReach::new(lookup), file, function);
        let plain = (entered == Entered::Never)
            .then(|| Entered::of(&SiteReach::without_refusals(lookup), file, function));
        let lazy = (entered == Entered::Runs)
            .then(|| Entered::of(&SiteReach::proving_lazy_reads(lookup), file, function));
        let members = (entered == Entered::Runs)
            .then(|| Entered::of(&SiteReach::proving_final_members(lookup), file, function));
        // ADR 1062: any path from a value a compute returns (directly, by reference, as a returned literal's member) marks the
        // site, whatever other path also runs it (a second path no longer hides this one).
        let returned = (entered == Entered::Runs
            && reachable_from_returned_value(file, function, lookup))
        .then_some(Entered::Never);
        Self {
            entered,
            plain,
            lazy,
            members,
            returned,
        }
    }

    fn site(self) -> SiteAnswer {
        match (
            self.entered,
            self.plain,
            self.lazy,
            self.members,
            self.returned,
        ) {
            (Entered::Never, Some(Entered::Undecided), _, _, _)
            | (Entered::Undecided, _, _, _, _)
            | (Entered::Runs, _, Some(Entered::Undecided), _, _)
            | (Entered::Runs, _, _, Some(Entered::Undecided), _) => SiteAnswer::Undecided,
            (Entered::Never, plain, _, _, _) => SiteAnswer::NeverEntered {
                refused_creation: plain == Some(Entered::Runs),
            },
            // ADR 0576: a site only an unread lazy computation runs.
            (Entered::Runs, _, Some(Entered::Never), _, _) => SiteAnswer::LazyUnread,
            // ADR 1034: a site only a member value the literal may replace runs.
            (Entered::Runs, _, _, Some(Entered::Never), _) => SiteAnswer::MemberMayBeReplaced,
            // ADR 1020 (third cut), ADR 1062: a returned value may run it.
            (Entered::Runs, _, _, _, Some(Entered::Never | Entered::Undecided)) => {
                SiteAnswer::ReturnedValueRun
            }
            (Entered::Runs, _, _, _, _) => SiteAnswer::MayRun,
        }
    }
}
