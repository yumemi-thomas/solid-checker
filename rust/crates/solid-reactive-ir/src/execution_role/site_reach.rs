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
//! ADR 0475: an entry the runtime refuses enters nothing. A creation (a
//! primitive the leaf-owner rules forbid, or a component render, which creates
//! a root first) written in the own extent of an effect apply or a leaf
//! callback throws before its callback or the component body runs, so that
//! callback is never entered from there, and a render or reference there
//! enters no function.

use std::cell::{Cell, RefCell};

use super::*;

/// What [`SiteReach::site`] proves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Reach {
    /// No execution of the program runs the site.
    Unreached(UnreachedReason),
    /// No proof either way.
    MayRun,
}

/// Why a site is [`Reach::Unreached`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UnreachedReason {
    /// The site follows an unconditional exit, or sits in an arm a literal
    /// test rules out.
    DeadCode,
    /// The function that runs the site is never created or never entered.
    NeverEntered,
}

/// The query, with a memo of each function's answer. One per question set:
/// the memo is valid for one build's facts.
pub(crate) struct SiteReach<'l, 'a> {
    lookup: &'l SemanticLookup<'a>,
    /// Whether each function may run, by `(path, span)`. A `false` derived
    /// while a function in progress was assumed not to run is not kept.
    functions: RefCell<HashMap<(String, Span), bool>>,
    visiting: RefCell<HashSet<(String, Span)>>,
    /// Set when a derivation met a function in progress.
    assumed: Cell<bool>,
    /// Whether a refused creation enters nothing (ADR 0475).
    refusals: bool,
}

/// How deep a chain of functions the query follows before it answers
/// `MayRun`.
const DEPTH_LIMIT: usize = 64;

impl<'l, 'a> SiteReach<'l, 'a> {
    pub(crate) fn new(lookup: &'l SemanticLookup<'a>) -> Self {
        Self {
            lookup,
            functions: RefCell::default(),
            visiting: RefCell::default(),
            assumed: Cell::new(false),
            refusals: true,
        }
    }

    /// The query without ADR 0475's refusals: what tells a report why a site
    /// is unreached.
    fn without_refusals(lookup: &'l SemanticLookup<'a>) -> Self {
        Self {
            refusals: false,
            ..Self::new(lookup)
        }
    }

    fn refused(&self, file: &solid_facts::FileFacts, entry: Span) -> bool {
        self.refusals && refused_entry(file, entry, self.lookup)
    }

    /// Whether any execution reaches `site` of `file`.
    pub(crate) fn site(&self, file: &solid_facts::FileFacts, site: Span) -> Reach {
        if file.ast.unreachable_contains(site) {
            return Reach::Unreached(UnreachedReason::DeadCode);
        }
        match crate::owners::owning_function(&file.ast, site) {
            Some(function) if !self.function_runs(file, function) => {
                Reach::Unreached(UnreachedReason::NeverEntered)
            }
            _ => Reach::MayRun,
        }
    }

    fn reached(&self, file: &solid_facts::FileFacts, site: Span) -> bool {
        self.site(file, site) == Reach::MayRun
    }

    /// Whether `function` of `file` may run.
    pub(crate) fn function_runs(
        &self,
        file: &solid_facts::FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> bool {
        let key = (file.path.to_string(), function.span);
        if let Some(runs) = self.functions.borrow().get(&key) {
            return *runs;
        }
        if self.visiting.borrow().len() >= DEPTH_LIMIT {
            return true;
        }
        if !self.visiting.borrow_mut().insert(key.clone()) {
            // In progress: a function entered only from itself never runs.
            self.assumed.set(true);
            return false;
        }
        let outer_assumed = self.assumed.replace(false);
        let runs = self.function_runs_uncached(file, function);
        self.visiting.borrow_mut().remove(&key);
        let assumed = self.assumed.get();
        if runs || !assumed {
            self.functions.borrow_mut().insert(key, runs);
        }
        self.assumed.set(outer_assumed || assumed);
        runs
    }

    fn function_runs_uncached(
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
            return !self.entered_only_through_refusals(file, function);
        };
        if !self.function_runs(file, outer) || self.refused(file, function.span) {
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
            .identifiers
            .iter()
            .filter(|identifier| {
                identifier.role == solid_facts::ast::IdentifierRole::Reference
                    && file.ast.reference_declaration(identifier.span) == Some(declaration)
            })
            .all(|reference| {
                file.ast
                    .type_queries
                    .iter()
                    .any(|query| query.contains(reference.span))
                    || returned_by_discarded_iife(file, reference.span)
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
        let declaration = match &class.name {
            Some(name) => name.span,
            None => {
                let Some(binding) = file.ast.bindings.iter().find(|binding| {
                    binding.shape == solid_facts::ast::BindingShape::Identifier
                        && binding.initializer.is_some_and(|initializer| {
                            file.ast.peel_ts_sugar_span(initializer) == class.span
                        })
                }) else {
                    return false;
                };
                let Some(name) = binding.names.first() else {
                    return false;
                };
                name.span
            }
        };
        !flow_binding_exported(file, declaration) && self.references_unreached(file, declaration)
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
    let Some(function) = crate::owners::owning_function(&file.ast, site) else {
        return false;
    };
    file.ast
        .arguments_containing(function.span)
        .filter(|(call, index)| {
            file.ast.peel_ts_sugar_span(call.arguments[*index].span) == function.span
        })
        .any(|(call, index)| {
            lookup
                .primitive_at_call(file, call.span)
                .is_some_and(|primitive| {
                    effect_apply_argument(lookup.dialect, primitive, call.arguments.len())
                        == Some(index)
                        || callback_owner_at_call(file, call, primitive, index, lookup)
                            == Some(solid_dialect::CallbackOwner::Leaf)
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
        .returns
        .iter()
        .find(|returned| returned_by(returned))
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
        ast.bindings
            .iter()
            .find(|binding| {
                binding.shape == solid_facts::ast::BindingShape::Identifier
                    && binding.initializer.is_some_and(|initializer| {
                        ast.peel_ts_sugar_span(initializer) == function.span
                    })
            })
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
            match query.site(file, site) {
                Reach::Unreached(reason) => Some(crate::UnreachedSite {
                    location: location.clone(),
                    dead_code: reason == UnreachedReason::DeadCode,
                    refused_creation: plain.site(file, site) == Reach::MayRun,
                }),
                Reach::MayRun => None,
            }
        })
        .collect::<Vec<_>>();
    sites.sort_by(|left, right| crate::location_order(&left.location, &right.location));
    sites.dedup();
    sites
}
