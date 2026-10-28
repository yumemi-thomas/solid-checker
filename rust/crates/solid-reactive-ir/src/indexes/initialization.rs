//! Temporal dead zone: whether a reference to a function-valued or
//! pattern-declared binding runs after the binding's declarator did.
//!
//! `const helper = () => ...` is not hoisted. A call that reaches it before
//! the declarator ran throws `ReferenceError` (a `var` reads `undefined` and
//! throws `TypeError`) and the body never runs, and `tsc` is silent when the
//! early use sits inside another function body. A call edge to such a binding
//! therefore proves execution only where the reference provably follows the
//! declarator. This module decides that per reference; a reference it cannot
//! decide is withheld from call sites and reachability, so a finding that
//! rested on it is uncertifiable, never a violation (ADR 0339).
//!
//! A binding a destructuring declarator binds (`const [count, setCount] =
//! createSignal(0)`, `const { a } = use()`) is decided the same way (ADR
//! 0464): a read, write or call that names it before the declarator ran
//! throws instead, so a violation at a site holding such a mention is
//! uncertifiable ([`early_mention_report_sites`]).

use std::sync::{Arc, Mutex};

use super::*;
use crate::object_members::ModuleCensus;
use solid_facts::ast::{BindingFact, BindingShape, FunctionKind};

/// Per module and declarator: the module-level functions that may run first.
type RunningBeforeDeclarator = HashMap<(String, Span), Arc<HashSet<Span>>>;

/// Per path, the withheld mentions ([`SemanticLookup::may_precede_initialization`])
/// and, among them, those whose order after the declarator is unproven
/// ([`SemanticLookup::mention_may_precede_initialization_within`]).
type Withheld = HashMap<String, (HashSet<Span>, HashSet<Span>)>;

#[derive(Default)]
pub(super) struct State {
    rejected: crate::base::reads::Recorded<Withheld>,
    census: crate::base::reads::Recorded<ModuleCensus>,
    file_refs: Mutex<HashMap<String, Arc<FileFunctionRefs>>>,
    running: Mutex<RunningBeforeDeclarator>,
}

/// The hoisted function declarations of one module that its code mentions,
/// by the outermost function each mention sits in.
#[derive(Default)]
struct FileFunctionRefs {
    /// Functions written at module level (inside no other function), with
    /// whether each is a hoisted declaration.
    top_level: Vec<(Span, bool)>,
    /// `(mention, outermost function containing it, declaration mentioned)`.
    mentions: Vec<(Span, Option<Span>, Span)>,
    /// Hoisted declarations whose mentions the compiler's reference index
    /// does not list: they may run anywhere.
    unknown: Vec<Span>,
}

#[derive(Clone, Copy)]
struct Declarator {
    /// Where the binding becomes initialized: the end of its initializer.
    initialized: u32,
    /// Where the declarator starts (the initializer's own function starts later).
    start: u32,
    /// The function whose body holds the declarator, `None` at module level.
    container: Option<Span>,
}

/// Per-build working tables of the proof.
#[derive(Default)]
struct Scratch {
    /// `(declaring file, binding)` to every mention of it: the mentioning
    /// file's index and the mention's span.
    mentions: HashMap<(usize, usize), Vec<(usize, Span)>>,
    declarators: HashMap<(usize, usize), Option<Declarator>>,
    /// `(file, function span)` to the binding the function is the whole
    /// initializer of.
    function_bindings: HashMap<(usize, Span), Option<usize>>,
    /// The full order (ADR 0464): an object or class method (a getter, setter
    /// or constructor included) is ordered like an arrow, by where it is
    /// created, and a project module's mention of an installed dependency's
    /// binding follows its declarator. The call graph keeps ADR 0339's order
    /// for a function-valued binding, which credits neither.
    full_order: bool,
}

fn function_valued(binding: &BindingFact) -> bool {
    binding.shape == BindingShape::Identifier
        && binding.names.len() == 1
        && binding.initializer.is_some()
        && (binding.initializer_function || binding.call_initializer.is_some())
}

/// ADR 0464: a binding an array or object pattern declares with an
/// initializer (`const [r, setR] = createSignal(0)`). A loop's
/// `for (const [k, v] of ...)` has none: it is bound before each iteration
/// runs, and its mentions are outside this proof.
fn pattern_declared(binding: &BindingFact) -> bool {
    binding.shape != BindingShape::Identifier && binding.initializer.is_some()
}

/// A `var` declarator whose initializer is a call written in its function's
/// straight-line flow: that call, and so the declarator, runs on every
/// execution of the function that reaches it.
fn var_initialized_in_line(file: &FileFacts, binding: &BindingFact) -> bool {
    binding
        .call_initializer
        .is_some_and(|call| file.ast.straight_line_calls.contains(&call))
}

/// The bindings whose mentions the proof decides.
fn decided(binding: &BindingFact) -> bool {
    function_valued(binding) || pattern_declared(binding)
}

/// The function whose invocation runs code at `span`: the innermost function
/// whose span (body or parameter list) holds it.
fn executing_function(
    ast: &solid_facts::ast::AstFacts,
    span: Span,
) -> Option<&solid_facts::ast::FunctionFact> {
    ast.functions
        .iter()
        .filter(|function| function.span.contains(span))
        .min_by_key(|function| function.span.end - function.span.start)
}

/// The function containing `function` (its creation point's executor).
fn creating_function<'f>(
    ast: &'f solid_facts::ast::AstFacts,
    function: &solid_facts::ast::FunctionFact,
) -> Option<&'f solid_facts::ast::FunctionFact> {
    ast.functions
        .iter()
        .filter(|outer| outer.span != function.span && outer.span.contains(function.span))
        .min_by_key(|outer| outer.span.end - outer.span.start)
}

impl<'a> SemanticLookup<'a> {
    /// Whether the code at `span` in `path` mentions a function-valued or
    /// pattern-declared binding at a point not proven to follow its
    /// declarator. Such a mention supplies no call site and no reachability
    /// edge.
    pub(crate) fn may_precede_initialization(&self, path: &str, span: Span) -> bool {
        let Some((spans, _)) = self.uninitialized_references().get(path) else {
            return false;
        };
        if spans.contains(&span) {
            return true;
        }
        // `helper!()`, `(helper as F)()`: the mention is the wrapped identifier.
        self.file_by_path(path).is_some_and(|file| {
            let peeled = file.ast.peel_ts_sugar_span(span);
            if peeled != span && spans.contains(&peeled) {
                return true;
            }
            // `A.helper()`, `ns.helper()`: the mention is the member's
            // property, though the call's callee is the whole member.
            file.ast
                .members
                .iter()
                .find(|member| member.span == peeled)
                .is_some_and(|member| spans.contains(&member.property))
        })
    }

    /// Whether a mention not proven to follow its binding's declarator lies
    /// within `span` of `path` (ADR 0464): code there names a binding that may
    /// not be initialized yet, so what it does may be a `ReferenceError` (or,
    /// for a `var`, a read of `undefined`) in place of the read, write or call
    /// a rule sees. A mention withheld only because the function it names has
    /// a default parameter that may throw is ordered, so it is not one.
    ///
    /// A mention inside a function written within the site (`f({ cb: () =>
    /// late() })`) is not evaluated by evaluating the site: it runs when that
    /// function is called, and a claim resting on it is located there, so it
    /// does not count (ADR 0464 review).
    pub(crate) fn mention_may_precede_initialization_within(&self, path: &str, span: Span) -> bool {
        let Some((_, unordered)) = self.uninitialized_references().get(path) else {
            return false;
        };
        let file = self.file_by_path(path);
        unordered.iter().any(|&mention| {
            span.contains(mention)
                && file.is_none_or(|file| {
                    executing_function(&file.ast, mention).is_none_or(|function| {
                        function.span == span || !span.contains(function.span)
                    })
                })
        })
    }

    /// Every withheld mention, by path. Bound into the cross-file proof
    /// digest: another module's imports decide a mention here.
    fn uninitialized_references(&self) -> &Withheld {
        self.initialization.rejected.get_or_init(|| {
            self.whole_program_answer::<super::project_memos::UninitializedReferences>()
                .into_iter()
                .map(|(path, spans, unordered)| {
                    (
                        path,
                        (spans.into_iter().collect(), unordered.into_iter().collect()),
                    )
                })
                .collect()
        })
    }

    /// The memo's computation: by path, the withheld mentions and those of
    /// them not proven to follow the declarator, sorted. The
    /// recursive `initialized_before` / `declaration_runs_after` proof runs
    /// inside this one top-level answer (its working tables are scratch of the
    /// call); the memo is keyed per program, a per-mention memo is study S7.
    pub(super) fn compute_uninitialized_references(&self) -> Vec<(String, Vec<Span>, Vec<Span>)> {
        {
            let mut scratch = Scratch::default();
            // Every mention of each decided binding, by binding, with whether
            // the binding is pattern-declared and whether the defaults of the
            // function it names may throw ahead of the mention.
            let mut mentions = Vec::<(&str, usize, usize, usize, Span, bool, bool)>::new();
            for (path, references) in self.bindings_by_reference() {
                let Some(caller) = self.files_by_path_index(path) else {
                    continue;
                };
                for ((start, end), resolution) in references {
                    let declaring = &self.facts.files[resolution.file];
                    let binding = &declaring.ast.bindings[resolution.binding];
                    // ADR 0459's namespace primitive slots are pattern
                    // bindings, so `decided` holds them already.
                    if !decided(binding) && !self.names_namespace_primitive(declaring, binding) {
                        continue;
                    }
                    let (Ok(start), Ok(end)) = (u32::try_from(*start), u32::try_from(*end)) else {
                        continue;
                    };
                    let span = Span::new(start, end);
                    if resolution.file == caller
                        && binding.names.iter().any(|name| name.span == span)
                    {
                        continue;
                    }
                    scratch
                        .mentions
                        .entry((resolution.file, resolution.binding))
                        .or_default()
                        .push((caller, span));
                    mentions.push((
                        path.as_str(),
                        caller,
                        resolution.file,
                        resolution.binding,
                        span,
                        pattern_declared(binding),
                        Self::defaults_may_throw(
                            caller,
                            span,
                            (resolution.file, declaring),
                            binding,
                        ),
                    ));
                }
            }
            let mut rejected = HashMap::<String, (HashSet<Span>, HashSet<Span>)>::default();
            // A decided binding none of whose names carries the compiler's
            // reference list has mentions the table above cannot see (ADR
            // 0464 review): its same-file mentions, from the binder, are
            // withheld and unordered. Another module's mention of it has no
            // such fallback (docs/open-work.md).
            for file in &self.facts.files {
                let path = file.path.as_str();
                for binding in file.ast.bindings.iter().filter(|binding| decided(binding)) {
                    let listed = binding.names.iter().any(|name| {
                        self.entities.at(path, name.span).is_some_and(|symbol| {
                            self.symbol_references_if_present(symbol.as_str()).is_some()
                        })
                    });
                    if listed {
                        continue;
                    }
                    for &(mention, declaration) in &file.ast.reference_declarations {
                        if binding.names.iter().any(|name| name.span == declaration)
                            && binding.names.iter().all(|name| name.span != mention)
                        {
                            let entry = rejected.entry(path.to_owned()).or_default();
                            entry.0.insert(mention);
                            entry.1.insert(mention);
                        }
                    }
                }
            }
            for (path, caller, declaring, binding, span, pattern, defaults_throw) in mentions {
                // The call graph's order: the full order only for a
                // pattern-declared binding (ADR 0464), ADR 0339's for a
                // function-valued one.
                scratch.full_order = pattern;
                let graph_ordered = self.initialized_before(
                    &mut scratch,
                    caller,
                    span,
                    declaring,
                    binding,
                    &mut Vec::new(),
                );
                // The full order, for the site downgrade (ADR 0464).
                let ordered = graph_ordered
                    || (!scratch.full_order && {
                        scratch.full_order = true;
                        self.initialized_before(
                            &mut scratch,
                            caller,
                            span,
                            declaring,
                            binding,
                            &mut Vec::new(),
                        )
                    });
                if graph_ordered && !defaults_throw {
                    continue;
                }
                let entry = rejected.entry(path.to_owned()).or_default();
                entry.0.insert(span);
                if !ordered {
                    entry.1.insert(span);
                }
            }
            let mut answer = rejected
                .into_iter()
                .map(|(path, (spans, unordered))| {
                    let mut spans = spans.into_iter().collect::<Vec<_>>();
                    spans.sort_unstable();
                    let mut unordered = unordered.into_iter().collect::<Vec<_>>();
                    unordered.sort_unstable();
                    (path, spans, unordered)
                })
                .collect::<Vec<_>>();
            answer.sort_unstable();
            answer
        }
    }

    /// Default parameter initializers run before the body and may throw (a
    /// call of a function or a class not yet initialized): a mention that
    /// reaches the body is proved only if they cannot. A default that is no
    /// more than a literal cannot; any other is an early mention of
    /// everything it references, which is not proved, so the call graph
    /// withholds the mention (ADR 0339). It says nothing of the mention's own
    /// order, so the site downgrade does not read it (ADR 0464). A declarator
    /// in a `switch` case is decided by its clause alone.
    fn defaults_may_throw(
        caller: usize,
        span: Span,
        declaring_file: (usize, &FileFacts),
        binding_fact: &BindingFact,
    ) -> bool {
        let (declaring, file) = declaring_file;
        let own_mention = caller == declaring
            && binding_fact
                .initializer
                .is_some_and(|initializer| initializer.contains(span));
        !binding_fact.in_switch_case
            && !own_mention
            && binding_fact.initializer_function
            && binding_fact.initializer.is_some_and(|initializer| {
                file.ast
                    .functions_within(initializer)
                    .max_by_key(|function| function.span.end - function.span.start)
                    .is_none_or(|function| !function.parameters_inert)
            })
    }

    /// Whether a slot of `binding` holds a dialect primitive read from a
    /// namespace import (ADR 0459): a mention of it before the declarator
    /// ran throws rather than calling the primitive.
    fn names_namespace_primitive(&self, file: &FileFacts, binding: &BindingFact) -> bool {
        binding.names.iter().any(|name| {
            file.ast.namespace_member_binding(name.span).is_some()
                && self
                    .entities
                    .at(file.path.as_str(), name.span)
                    .is_some_and(|symbol| self.symbol_names.contains_key(symbol))
        })
    }

    /// Whether the mention `span` of a namespace member slot (ADR 0459) may
    /// run before the slot's declarator: a call or tag there throws rather
    /// than reaching the primitive, so it names none. Every reference to a
    /// slot is in its own file: a directly exported slot is not a fact, and a
    /// value that escapes otherwise is no reference to the slot's symbol.
    pub(crate) fn namespace_slot_mention_withheld(&self, file: &FileFacts, span: Span) -> bool {
        if file.ast.namespace_member_bindings.is_empty() {
            return false;
        }
        let span = file.ast.peel_ts_sugar_span(span);
        file.ast
            .reference_declaration(span)
            .is_some_and(|declaration| file.ast.namespace_member_binding(declaration).is_some())
            && self.may_precede_initialization(file.path.as_str(), span)
    }

    fn files_by_path_index(&self, path: &str) -> Option<usize> {
        self.first_file_index(path)
    }

    fn declarator(file: &FileFacts, binding: &BindingFact) -> Option<Declarator> {
        let initializer = binding.initializer?;
        Some(Declarator {
            initialized: initializer.end,
            start: binding.declaration.start,
            container: executing_function(&file.ast, binding.pattern).map(|function| function.span),
        })
    }

    /// Where, at module level, the code at `span` runs: its own position, or
    /// that of the function expression or arrow it sits in, as late as the
    /// code that creates it. A hoisted declaration may run anywhere, so it
    /// has none, and so may a method unless `full_order` orders it by its
    /// creation too.
    fn module_level_position(
        ast: &solid_facts::ast::AstFacts,
        span: Span,
        full_order: bool,
    ) -> Option<u32> {
        let mut position = span.start;
        let mut executor = executing_function(ast, span);
        while let Some(function) = executor {
            match function.kind {
                FunctionKind::Arrow | FunctionKind::Expression
                    if function.method_name.is_none() || full_order =>
                {
                    position = function.span.start;
                    executor = creating_function(ast, function);
                }
                _ => return None,
            }
        }
        Some(position)
    }

    /// [`ModuleCensus::new`] of the lookup's files, built once per build.
    pub(crate) fn module_census(&self) -> &ModuleCensus {
        self.initialization
            .census
            .get_or_init(|| ModuleCensus::new(self.facts, self))
    }

    /// [`MemberStability::index`] of the lookup's files, built once per build.
    ///
    /// [`MemberStability::index`]: crate::interproc::accessor_provenance::MemberStability::index
    pub(crate) fn member_stability(
        &self,
    ) -> &crate::interproc::accessor_provenance::MemberStability {
        use crate::interproc::accessor_provenance::MemberStability;
        self.member_stability
            .get_or_init(|| MemberStability::index(self.facts, self))
    }

    /// Whether code at `span` in file `caller` runs after the declarator of
    /// binding `binding` of file `declaring`. `visiting` holds the arrows
    /// whose own mentions are being proved, so a cycle of them ends.
    fn initialized_before(
        &self,
        scratch: &mut Scratch,
        caller: usize,
        span: Span,
        declaring: usize,
        binding: usize,
        visiting: &mut Vec<(usize, Span)>,
    ) -> bool {
        let declaring_file = &self.facts.files[declaring];
        let caller_file = &self.facts.files[caller];
        let binding_fact = &declaring_file.ast.bindings[binding];
        let Some(initializer) = binding_fact.initializer else {
            return false;
        };
        // An allowlist (ADR 0339): only a `const` or `let` is initialized by
        // its declarator running, and so by source position alone. A `var` is
        // initialized only if its initializer executed (a loop, branch or
        // caught throw may skip it) and reads `undefined` until then.
        //
        // ADR 0464 review: under the full order a `var` whose initializer is
        // a call in its function's straight-line flow (`straight_line_calls`)
        // is initialized whenever that function reaches the declarator, so it
        // is ordered by position like a `const`.
        if !(binding_fact.immutable
            || binding_fact.runtime_let
            || (scratch.full_order && var_initialized_in_line(declaring_file, binding_fact)))
        {
            return false;
        }
        // A direct `eval` calls any function, or rebinds any binding, visible
        // where it is written, at any point: nothing is proved of this file.
        if crate::interproc::property_gets::direct_eval_in(declaring_file, Span::new(0, u32::MAX))
            || crate::interproc::property_gets::direct_eval_in(caller_file, Span::new(0, u32::MAX))
        {
            return false;
        }
        if binding_fact.in_switch_case {
            // A jump to a later clause enters the shared scope without
            // running the declarator, whoever mentions it.
            return caller == declaring
                && initializer.contains(span)
                && binding_fact.initializer_function;
        }
        if caller != declaring
            && scratch.full_order
            && self
                .facts
                .dependency_source_root(declaring_file.path.as_str())
                .is_some()
            && self
                .facts
                .dependency_source_root(caller_file.path.as_str())
                .is_none()
        {
            // ADR 0464, on ADR 0339's premise that installed dependency code
            // imports no project module: a project module names the binding
            // only through a load of the package (static, `require` or
            // `import()`), which completes the package's evaluation, the
            // declarator included, before the name is usable.
            return true;
        }
        if caller != declaring {
            // ES modules evaluate a module's static imports before its body
            // (ADR 0339), except a module already being evaluated: an
            // importer that no import cycle reaches has none of its imports
            // (and so not the declaring module) being evaluated, so every
            // module it imports, transitively, finished first. An importer on
            // or below a cycle can run before the declaring module did
            // (`b` imports `c` then re-exports `d`; `c` imports `b`).
            let census = self.module_census();
            return !census.is_cyclic(caller_file.path.as_str())
                && census.imports_before(
                    caller_file.path.as_str(),
                    declaring_file.path.as_str(),
                    Self::module_level_position(&caller_file.ast, span, scratch.full_order),
                );
        }
        if initializer.contains(span) {
            // Inside the function expression itself: it runs only once it
            // exists, which is after the declarator. Inside a call's
            // arguments (`const x = wrap(() => x())`) the callee may run it
            // before `x` is initialized.
            return binding_fact.initializer_function;
        }
        let Some(declarator) = scratch
            .declarators
            .entry((declaring, binding))
            .or_insert_with(|| Self::declarator(declaring_file, binding_fact))
            .as_ref()
            .copied()
        else {
            return false;
        };
        let ast = &declaring_file.ast;
        let mut position = span.start;
        let mut executor = executing_function(ast, span);
        // The arrow, written directly in the declarator's body, that the
        // mention sits in, when it is created before the declarator.
        let mut early_arrow = None;
        loop {
            if executor.map(|function| function.span) == declarator.container {
                if position >= declarator.initialized {
                    return true;
                }
                // Written before the declarator, so it may run before it,
                // unless every mention of the arrow follows the declarator:
                // it then cannot run until they do.
                return early_arrow.is_some_and(|arrow| {
                    self.arrow_runs_after(
                        scratch,
                        declaring,
                        arrow,
                        (declaring, binding),
                        &declarator,
                        visiting,
                    )
                });
            }
            let Some(function) = executor else {
                return false;
            };
            match function.kind {
                // A function expression or arrow cannot run before it is
                // created: it is as late as the code that creates it. Nor can
                // a method of an object literal or a class, which is not
                // hoisted either; the call graph credits that only for a
                // pattern-declared binding (ADR 0464).
                FunctionKind::Arrow | FunctionKind::Expression
                    if function.method_name.is_none() || scratch.full_order =>
                {
                    position = function.span.start;
                    executor = creating_function(ast, function);
                    early_arrow = Some(function.span);
                }
                // A hoisted declaration may be called before any statement
                // of its scope. At module level that is decided by what the
                // module runs ahead of the declarator.
                FunctionKind::Declaration
                    if declarator.container.is_none()
                        && creating_function(ast, function).is_none() =>
                {
                    return !self.module_census().is_cyclic(declaring_file.path.as_str())
                        && !self
                            .runs_before_declarator(declaring_file, &declarator)
                            .contains(&function.span);
                }
                // A hoisted declaration nested in a function runs wherever
                // its mentions do (it is not visible outside its scope).
                FunctionKind::Declaration => {
                    return self.declaration_runs_after(
                        scratch,
                        declaring,
                        function,
                        (declaring, binding),
                        visiting,
                    );
                }
                _ => return false,
            }
        }
    }

    /// Whether the nested function declaration `function` runs only after the
    /// declarator of `declarator_binding`: every mention of it, from the
    /// compiler's reference index, follows the declarator.
    fn declaration_runs_after(
        &self,
        scratch: &mut Scratch,
        file: usize,
        function: &solid_facts::ast::FunctionFact,
        declarator_binding: (usize, usize),
        visiting: &mut Vec<(usize, Span)>,
    ) -> bool {
        if visiting.contains(&(file, function.span)) {
            return true;
        }
        let facts = &self.facts.files[file];
        let Some(name) = function.name.as_ref() else {
            return false;
        };
        let Some(symbol) = self.entities.at(facts.path.as_str(), name.span) else {
            return false;
        };
        let Some(references) = self.symbol_references_if_present(symbol.as_str()) else {
            return false;
        };
        visiting.push((file, function.span));
        let proved = references.iter().all(|reference| {
            let (Ok(start), Ok(end)) = (
                u32::try_from(reference.start_byte),
                u32::try_from(reference.end_byte),
            ) else {
                return false;
            };
            let mention = Span::new(start, end);
            if mention == name.span || function.span.contains(mention) {
                return true;
            }
            self.files_by_path_index(reference.path.as_ref())
                .is_some_and(|caller| {
                    self.initialized_before(
                        scratch,
                        caller,
                        mention,
                        declarator_binding.0,
                        declarator_binding.1,
                        visiting,
                    )
                })
        });
        visiting.pop();
        proved
    }

    /// Whether the arrow or function expression `arrow`, written before
    /// `declarator`, runs only after it: it is the whole initializer of a
    /// binding and every mention of that binding follows the declarator. At
    /// module level another module's mention follows it too, unless the
    /// module is on an import cycle.
    fn arrow_runs_after(
        &self,
        scratch: &mut Scratch,
        file: usize,
        arrow: Span,
        declarator_binding: (usize, usize),
        declarator: &Declarator,
        visiting: &mut Vec<(usize, Span)>,
    ) -> bool {
        if visiting.contains(&(file, arrow)) {
            // A mention inside the arrow's own proof: it runs only when the
            // arrow does.
            return true;
        }
        let facts = &self.facts.files[file];
        let own = *scratch
            .function_bindings
            .entry((file, arrow))
            .or_insert_with(|| {
                facts.ast.bindings.iter().position(|candidate| {
                    function_valued(candidate)
                        && candidate.initializer_function
                        && candidate.initializer.is_some_and(|initializer| {
                            facts
                                .ast
                                .functions_within(initializer)
                                .max_by_key(|function| function.span.end - function.span.start)
                                .is_some_and(|function| function.span == arrow)
                        })
                })
            });
        let Some(own) = own else {
            return false;
        };
        // The mention table is the compiler's reference list plus demanded
        // entities; without the list it may be incomplete.
        let listed = facts.ast.bindings[own]
            .names
            .first()
            .and_then(|name| self.entities.at(facts.path.as_str(), name.span))
            .is_some_and(|symbol| self.symbol_references_if_present(symbol.as_str()).is_some());
        if !listed {
            return false;
        }
        if declarator.container.is_none() && self.module_census().is_cyclic(facts.path.as_str()) {
            return false;
        }
        let mentions = scratch
            .mentions
            .get(&(file, own))
            .cloned()
            .unwrap_or_default();
        if mentions.is_empty() {
            return false;
        }
        visiting.push((file, arrow));
        let proved = mentions.into_iter().all(|(caller, mention)| {
            (caller == file && Span::new(arrow.start, arrow.end).contains(mention))
                || self.initialized_before(
                    scratch,
                    caller,
                    mention,
                    declarator_binding.0,
                    declarator_binding.1,
                    visiting,
                )
        });
        visiting.pop();
        proved
    }

    /// The module-level functions that may run before the declarator: every
    /// function written ahead of it at module level (an arrow or method there
    /// may be invoked by the code around it), the hoisted declarations that
    /// code mentions, and what those mention in turn. Module-level code that
    /// calls into other modules reaches nothing here without a mention, since
    /// a module that cannot import this one cannot hold a reference to it.
    fn runs_before_declarator(
        &self,
        file: &FileFacts,
        declarator: &Declarator,
    ) -> Arc<HashSet<Span>> {
        let key = (
            file.path.to_string(),
            Span::new(declarator.start, declarator.initialized),
        );
        if let Some(running) = self
            .initialization
            .running
            .lock()
            .expect("initialization memo")
            .get(&key)
        {
            return Arc::clone(running);
        }
        let refs = self.function_refs(file);
        let mut running = refs.unknown.iter().copied().collect::<HashSet<Span>>();
        for (span, declaration) in &refs.top_level {
            if !declaration && span.end <= declarator.start {
                running.insert(*span);
            }
        }
        loop {
            let mut changed = false;
            for (mention, outer, target) in &refs.mentions {
                let active = match outer {
                    None => mention.start < declarator.start,
                    Some(outer) => running.contains(outer),
                };
                if active {
                    changed |= running.insert(*target);
                }
            }
            if !changed {
                break;
            }
        }
        let running = Arc::new(running);
        self.initialization
            .running
            .lock()
            .expect("initialization memo")
            .insert(key, Arc::clone(&running));
        running
    }

    fn function_refs(&self, file: &FileFacts) -> Arc<FileFunctionRefs> {
        if let Some(refs) = self
            .initialization
            .file_refs
            .lock()
            .expect("initialization memo")
            .get(file.path.as_str())
        {
            return Arc::clone(refs);
        }
        let ast = &file.ast;
        let mut top_level = ast
            .functions
            .iter()
            .filter(|function| creating_function(ast, function).is_none())
            .map(|function| (function.span, function.kind == FunctionKind::Declaration))
            .collect::<Vec<_>>();
        top_level.sort_unstable();
        top_level.dedup();
        let outermost = |span: Span| {
            top_level
                .iter()
                .find(|(function, _)| function.contains(span))
                .map(|(function, _)| *function)
        };
        let mut mentions = Vec::new();
        let mut unknown = Vec::new();
        for function in ast.functions.iter().filter(|function| {
            function.kind == FunctionKind::Declaration && creating_function(ast, function).is_none()
        }) {
            // The compiler's reference index, not the identifiers that
            // happened to get an entity: `go!()`, `(go as F)()`, `void go` and
            // `new (go as any)()` are mentions too. A missing list is missing
            // evidence, so the declaration may run anywhere.
            let references = function
                .name
                .as_ref()
                .and_then(|name| self.entities.at(file.path.as_str(), name.span))
                .and_then(|symbol| self.symbol_references_if_present(symbol.as_str()));
            let Some(references) = references else {
                unknown.push(function.span);
                continue;
            };
            let name = function.name.as_ref().map(|name| name.span);
            for reference in references
                .iter()
                .filter(|reference| reference.path.as_ref() == file.path.as_str())
            {
                let (Ok(start), Ok(end)) = (
                    u32::try_from(reference.start_byte),
                    u32::try_from(reference.end_byte),
                ) else {
                    unknown.push(function.span);
                    continue;
                };
                let mention = Span::new(start, end);
                if Some(mention) != name {
                    mentions.push((mention, outermost(mention), function.span));
                }
            }
        }
        let refs = Arc::new(FileFunctionRefs {
            top_level,
            mentions,
            unknown,
        });
        self.initialization
            .file_refs
            .lock()
            .expect("initialization memo")
            .insert(file.path.to_string(), Arc::clone(&refs));
        refs
    }
}

/// The location of every row `program` may report whose site holds a mention
/// not proven to follow its binding's declarator ([`SemanticLookup::
/// mention_may_precede_initialization_within`]), sorted. The backend makes a
/// violation there uncertifiable (ADR 0464): when the mention runs first, the
/// site throws in place of what the rule claims.
pub(crate) fn early_mention_report_sites(
    program: &crate::Program,
    lookup: &SemanticLookup<'_>,
) -> Vec<Location> {
    let early = |location: &Location| {
        let (Ok(start), Ok(end)) = (
            u32::try_from(location.start_byte),
            u32::try_from(location.end_byte),
        ) else {
            return false;
        };
        lookup.mention_may_precede_initialization_within(
            location.path.as_ref(),
            Span::new(start, end),
        )
    };
    let mut sites = crate::projection::reported_locations(program)
        .into_iter()
        .filter(|location| early(location))
        .cloned()
        .collect::<Vec<_>>();
    // A read a helper's call propagates (`read()` where `function read() {
    // return count(); }`) rests on the mention at its origin.
    sites.extend(
        program
            .reads
            .iter()
            .filter(|read| read.origin.as_ref().is_some_and(early))
            .map(|read| read.location.clone()),
    );
    sites.sort_by(crate::location_order);
    sites.dedup();
    sites
}
