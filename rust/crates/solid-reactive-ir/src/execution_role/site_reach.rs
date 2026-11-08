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
//!   reference to that name is itself unreached, in a type query, or the
//!   returned value of an IIFE whose result is discarded; and every call or
//!   render the call graph resolves to it is unreached;
//! - it is an anonymous function returned by an IIFE whose result is
//!   discarded.
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
        }
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
        if file.ast.unreachable_contains(site) {
            return Reach::Unreached(UnreachedReason::DeadCode);
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
        if file.ast.unreachable_contains(function.span) {
            return false;
        }
        if let Some(class) = class_of_member(file, function)
            && self.class_unreached(file, class)
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
        if self.lazy_reads
            && let Some(call) = lazy_creation(file, function.span, self.lookup)
            && !self.lazy_read_proven(file, call)
        {
            return false;
        }
        match function_declaration_span(file, function) {
            Some(declaration) => !self.binding_unreached(file, function, declaration),
            None => !returned_by_discarded_iife(file, function.span),
        }
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
    /// unreached, erased (a type query) or a value an IIFE returns into a
    /// discarded result.
    fn references_unreached(&self, file: &solid_facts::FileFacts, declaration: Span) -> bool {
        file.ast
            .reference_identifiers_to(declaration)
            .all(|reference| {
                file.ast
                    .type_queries
                    .iter()
                    .any(|query| query.contains(reference.span))
                    || returned_by_discarded_iife(file, reference.span)
                    || (self.lazy_reads
                        && lazy_creation(file, reference.span, self.lookup)
                            .is_some_and(|call| !self.lazy_read_proven(file, call)))
                    || !self.reached(file, reference.span)
                    || self.refused(file, reference.span)
            })
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
        Some((climb, _)) if climb.exhausted => return true,
        Some((climb, _)) => climb.at,
        None => span,
    };
    crate::owners::containing_ast_function(&file.ast, at).is_some_and(|function| {
        class_of_member(file, function).is_some()
            && SiteReach::new(lookup).function_runs(file, function)
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
    if matches!(SiteReach::new(lookup).site(file, span), Reach::Unreached(_)) {
        return false;
    }
    let ast = &file.ast;
    ast.classes
        .iter()
        .flat_map(|class| &class.elements)
        .any(|element| element.value.is_some_and(|value| value.contains(span)))
        || ast.functions_owning(span).any(|function| {
            let value = function.span;
            ast.object_properties
                .iter()
                .any(|property| ast.peel_ts_sugar_span(property.value) == value)
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

/// Whether the value at `value` is what a function called where it is
/// written (an IIFE) returns, and the call's result is discarded: the value
/// goes nowhere. `return value` in the function's own body, or its expression
/// body. A generator is not read: its value goes to its iterator.
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
pub(crate) fn unreached_report_sites(
    program: &crate::Program,
    lookup: &SemanticLookup<'_>,
) -> Vec<crate::UnreachedSite> {
    let query = SiteReach::new(lookup);
    let plain = SiteReach::without_refusals(lookup);
    let lazy = SiteReach::proving_lazy_reads(lookup);
    let mut sites = crate::projection::reported_locations(program)
        .into_iter()
        .filter_map(|location| {
            let file = lookup.file_by_path(location.path.as_ref())?;
            let (Ok(start), Ok(end)) = (
                u32::try_from(location.start_byte),
                u32::try_from(location.end_byte),
            ) else {
                return None;
            };
            let site = Span::new(start, end);
            let undecided = crate::UnreachedSite {
                location: location.clone(),
                dead_code: false,
                absent_prop: false,
                refused_creation: false,
                lazy_unread: false,
                undecided: true,
            };
            match query.site(file, site) {
                Reach::Unreached(reason) => match plain.site(file, site) {
                    // ADR 0560: an undecided query proves nothing; the
                    // violation is not proven either.
                    Reach::Undecided => Some(undecided),
                    plain => Some(crate::UnreachedSite {
                        location: location.clone(),
                        dead_code: reason == UnreachedReason::DeadCode,
                        absent_prop: reason == UnreachedReason::AbsentProp,
                        refused_creation: plain == Reach::MayRun,
                        lazy_unread: false,
                        undecided: false,
                    }),
                },
                Reach::Undecided => Some(undecided),
                // ADR 0576: a site only an unread lazy computation runs.
                Reach::MayRun => match lazy.site(file, site) {
                    Reach::Unreached(_) => Some(crate::UnreachedSite {
                        location: location.clone(),
                        dead_code: false,
                        absent_prop: false,
                        refused_creation: false,
                        lazy_unread: true,
                        undecided: false,
                    }),
                    Reach::Undecided => Some(undecided),
                    Reach::MayRun => None,
                },
            }
        })
        .collect::<Vec<_>>();
    sites.sort_by(|left, right| crate::location_order(&left.location, &right.location));
    sites.dedup();
    sites
}
