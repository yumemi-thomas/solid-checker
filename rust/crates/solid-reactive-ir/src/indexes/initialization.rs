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

/// ADR 1182: by `(mentioning path, mention)`, the function a written `let` holds at that mention: the last of its writes, each a
/// module-level straight-line `name = <function>`, which the mention provably follows ([`SemanticLookup::settled_let_function`]).
type Settled = HashMap<(String, Span), (String, Span)>;

/// ADR 1182: `((file, receiver binding), member name)` with the declarator standing at the name's last write and that write's
/// function ([`SemanticLookup::settling_members`]).
type SettlingMember = (((usize, usize), String), (Declarator, Span));

/// One path's part of [`super::project_memos::UninitializedReferences`]: the withheld mentions, those of them not ordered after the
/// declarator, and (ADR 1182) the settled `let` mentions as `(mention, declaring path, function)`.
pub(crate) type UninitializedRow = (String, Vec<Span>, Vec<Span>, Vec<(Span, String, Span)>);

#[derive(Default)]
pub(super) struct State {
    rejected: crate::base::reads::Recorded<Withheld>,
    settled: crate::base::reads::Recorded<Settled>,
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
    /// ADR 1182: a declarator standing in for a binding's own, at the end of its last write: the proof then asks whether a mention
    /// follows that write. The initializer's own function is no exception there (it may run before the write).
    stand_in: HashMap<(usize, usize), Declarator>,
}

/// The declarations and arrows whose mentions one ordering proof is proving
/// ([`SemanticLookup::declaration_runs_after`], `arrow_runs_after`).
///
/// ADR 0546: the proof is a conjunction over mentions in which a function
/// already being proved counts as proved, so it fails exactly when a
/// mention that fails on its own is reachable from the root through
/// mentions; a function once proved within the root's proof is proved
/// wherever the proof meets it again (as a visited set in a depth-first
/// search), instead of being proved again on every path to it.
struct Proving {
    stack: Vec<(usize, Span)>,
    proved: HashSet<(usize, Span)>,
    /// Keeps what it proved ([`crate::follow_every_path`] off).
    keeps: bool,
}

impl Default for Proving {
    fn default() -> Self {
        Self {
            stack: Vec::new(),
            proved: HashSet::default(),
            keeps: !crate::execution_role::every_path_asked(),
        }
    }
}

impl Proving {
    /// Whether `node` counts as proved here: in progress, or proved before.
    fn assumed(&self, node: &(usize, Span)) -> bool {
        self.stack.contains(node) || self.proved.contains(node)
    }

    /// Ends the innermost proof in progress, keeping it when it held.
    fn finish(&mut self, proved: bool) {
        if let Some(node) = self.stack.pop()
            && proved
            && self.keeps
        {
            self.proved.insert(node);
        }
    }
}

fn function_valued(binding: &BindingFact) -> bool {
    binding.shape == BindingShape::Identifier
        && binding.names.len() == 1
        && binding.initializer.is_some()
        && (binding.initializer_function || binding.call_initializer.is_some())
}

/// Whether a name of `binding` is a local alias (ADR 0463): a mention of it
/// before the declarator ran throws rather than reaching the value it holds.
fn names_local_alias(file: &FileFacts, binding: &BindingFact) -> bool {
    binding.names.iter().any(|name| {
        file.ast.local_alias_binding(name.span).is_some()
            // ADR 0511: an import alias likewise.
            || crate::source_discovery::import_alias_declaration(file, name.span)
    })
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
                .member_at(peeled)
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

    /// Whether a mention not proven to follow its binding's declarator is
    /// written before `span` in the function that runs `span` (or both at
    /// module level): when it runs first and throws, the code at `span` does
    /// not run at all (`[1].forEach(() => { late(); count(); })`), so what a
    /// rule claims there is not proven either (ADR 0517). Only a mention
    /// written before its binding's declarator in this file: one written
    /// after it is unordered when the proof cannot place the function's run
    /// (a handler a test dispatches), which is no sign it runs early. A
    /// hoisted function called above the declarator whose body names it
    /// later in the text is not covered (`docs/open-work.md`).
    pub(crate) fn unordered_mention_precedes_in_function(&self, path: &str, span: Span) -> bool {
        let Some((_, unordered)) = self.uninitialized_references().get(path) else {
            return false;
        };
        let Some(file) = self.file_by_path(path) else {
            return !unordered.is_empty();
        };
        let runner = executing_function(&file.ast, span).map(|function| function.span);
        unordered.iter().any(|&mention| {
            mention.end <= span.start
                && executing_function(&file.ast, mention).map(|function| function.span) == runner
                && file
                    .ast
                    .reference_declaration(mention)
                    .is_some_and(|declaration| declaration.start >= mention.end)
        })
    }

    /// Every withheld mention, by path. Bound into the cross-file proof
    /// digest: another module's imports decide a mention here.
    pub(super) fn uninitialized_references(&self) -> &Withheld {
        self.initialization.rejected.get_or_init(|| {
            self.whole_program_answer::<super::project_memos::UninitializedReferences>()
                .into_iter()
                .map(|(path, spans, unordered, _)| {
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
    /// ADR 1182: the function the written `let` named at `mention` of `path` holds there, when every write of it is a module-level
    /// straight-line `name = <function>` and the mention provably runs after the last ([`Scratch::stand_in`]).
    pub(crate) fn settled_let_function(
        &self,
        path: &str,
        mention: Span,
    ) -> Option<(&'a FileFacts, &'a solid_facts::ast::FunctionFact)> {
        let settled = self.initialization.settled.get_or_init(|| {
            self.whole_program_answer::<super::project_memos::UninitializedReferences>()
                .into_iter()
                .flat_map(|(path, _, _, settled)| {
                    settled
                        .into_iter()
                        .map(move |(mention, declaring, function)| {
                            ((path.clone(), mention), (declaring, function))
                        })
                })
                .collect()
        });
        let (declaring, function) = settled.get(&(path.to_owned(), mention))?;
        let file = self.file_by_path(declaring)?;
        Some((file, file.ast.function_at(*function)?))
    }

    pub(super) fn compute_uninitialized_references(&self) -> Vec<UninitializedRow> {
        {
            let mut scratch = Scratch::default();
            // Every mention of each decided binding, by binding, with whether
            // the binding is pattern-declared and whether the defaults of the
            // function it names may throw ahead of the mention.
            let mut mentions = Vec::<(&str, usize, usize, usize, Span, bool, bool)>::new();
            // Whether a binding's mentions are decided here: a function of
            // the declaring file and binding alone, asked once per binding
            // rather than once per mention.
            let mut eligible = HashMap::<(usize, usize), bool>::default();
            for (path, references) in self.bindings_by_reference() {
                let Some(caller) = self.files_by_path_index(path) else {
                    continue;
                };
                for ((start, end), resolution) in references {
                    let declaring = &crate::base::reads::file_slots(self.facts)[resolution.file];
                    let binding = &declaring.ast.bindings[resolution.binding];
                    // ADR 0459's namespace primitive slots are pattern
                    // bindings, so `decided` holds them already.
                    if !*eligible
                        .entry((resolution.file, resolution.binding))
                        .or_insert_with(|| {
                            decided(binding)
                                || self.names_namespace_primitive(declaring, binding)
                                || names_local_alias(declaring, binding)
                        })
                    {
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
            // ADR 1182: the written `let`s whose every write is a module-level straight-line `name = <function>`, with their last write.
            let settling = self.settling_lets();
            // A decided binding none of whose names carries the compiler's
            // reference list has mentions the table above cannot see (ADR
            // 0464 review): its same-file mentions, from the binder, are
            // withheld and unordered. Another module's mention of it has no
            // such fallback (docs/open-work.md).
            for file in crate::base::reads::file_slots(self.facts) {
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
                    // The file's mentions resolved to one of the binding's
                    // names (sets: the order the binder lists them in is not
                    // read).
                    for name in &binding.names {
                        for &mention in file.ast.references_to(name.span) {
                            if binding.names.iter().all(|name| name.span != mention) {
                                let entry = rejected.entry(path.to_owned()).or_default();
                                entry.0.insert(mention);
                                entry.1.insert(mention);
                            }
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
                    &mut Proving::default(),
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
                            &mut Proving::default(),
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
            // ADR 1182: each mention of a settling `let` (no write of it), proved after its last write by the same proof.
            let mut settled = HashMap::<String, Vec<(Span, String, Span)>>::default();
            for (key, (declarator, function)) in &settling {
                scratch.stand_in.insert(*key, *declarator);
                let declaring_path = crate::base::reads::file_slots(self.facts)[key.0]
                    .path
                    .to_string();
                for (caller, mention) in scratch.mentions.get(key).cloned().unwrap_or_default() {
                    let caller_file = &crate::base::reads::file_slots(self.facts)[caller];
                    if caller_file.ast.any_write_target_containing(mention) {
                        continue;
                    }
                    scratch.full_order = false;
                    let ordered = self.initialized_before(
                        &mut scratch,
                        caller,
                        mention,
                        key.0,
                        key.1,
                        &mut Proving::default(),
                    ) || {
                        scratch.full_order = true;
                        self.initialized_before(
                            &mut scratch,
                            caller,
                            mention,
                            key.0,
                            key.1,
                            &mut Proving::default(),
                        )
                    };
                    if ordered {
                        settled
                            .entry(caller_file.path.to_string())
                            .or_default()
                            .push((mention, declaring_path.clone(), *function));
                    }
                }
                scratch.stand_in.remove(key);
            }
            // ADR 1182: each call of a settling member through its receiver, proved after the member's last write the same way.
            for ((key, name), (declarator, function)) in self.settling_members() {
                let file = &crate::base::reads::file_slots(self.facts)[key.0];
                let receiver = &file.ast.bindings[key.1].names[0];
                scratch.stand_in.insert(key, declarator);
                for &mention in file.ast.references_to(receiver.span) {
                    let Some(member) = file.ast.members.iter().find(|member| {
                        file.ast.peel_ts_sugar_span(member.object) == mention
                            && file.source_text(member.property) == Some(name.as_str())
                    }) else {
                        continue;
                    };
                    if file
                        .ast
                        .calls_with_peeled_callee(member.span)
                        .next()
                        .is_none()
                    {
                        continue;
                    }
                    scratch.full_order = false;
                    let ordered = self.initialized_before(
                        &mut scratch,
                        key.0,
                        mention,
                        key.0,
                        key.1,
                        &mut Proving::default(),
                    ) || {
                        scratch.full_order = true;
                        self.initialized_before(
                            &mut scratch,
                            key.0,
                            mention,
                            key.0,
                            key.1,
                            &mut Proving::default(),
                        )
                    };
                    if ordered {
                        settled.entry(file.path.to_string()).or_default().push((
                            member.span,
                            file.path.to_string(),
                            function,
                        ));
                    }
                }
                scratch.stand_in.remove(&key);
            }
            let mut paths = rejected.keys().cloned().collect::<HashSet<_>>();
            paths.extend(settled.keys().cloned());
            let mut answer = paths
                .into_iter()
                .map(|path| {
                    let (spans, unordered) = rejected.remove(&path).unwrap_or_default();
                    let mut spans = spans.into_iter().collect::<Vec<_>>();
                    spans.sort_unstable();
                    let mut unordered = unordered.into_iter().collect::<Vec<_>>();
                    unordered.sort_unstable();
                    let mut rows = settled.remove(&path).unwrap_or_default();
                    rows.sort_unstable();
                    (path, spans, unordered, rows)
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
        if file.ast.namespace_member_bindings.is_empty() && file.ast.imports.is_empty() {
            return false;
        }
        let span = file.ast.peel_ts_sugar_span(span);
        // Review D36: an import alias that names a primitive
        // (`crate::symbols::import_alias_source`) is withheld the same way.
        file.ast
            .reference_declaration(span)
            .is_some_and(|declaration| {
                file.ast.namespace_member_binding(declaration).is_some()
                    || crate::symbols::import_alias_source(file, declaration).is_some()
            })
            && self.may_precede_initialization(file.path.as_str(), span)
    }

    /// ADR 0463: whether the mention `span` of a local alias
    /// ([`solid_facts::ast::LocalAliasBindingFact`]) may run before the
    /// alias's declarator, where it throws instead of reading or calling the
    /// value it would hold. Every reference to an alias is in its own file.
    /// ADR 0511: an import alias (`const d = a1`, `const { d } = { d: a1 }`)
    /// is withheld the same way.
    pub(crate) fn local_alias_mention_withheld(&self, file: &FileFacts, span: Span) -> bool {
        if file.ast.local_alias_bindings.is_empty() && file.ast.imports.is_empty() {
            return false;
        }
        let span = file.ast.peel_ts_sugar_span(span);
        file.ast
            .reference_declaration(span)
            .is_some_and(|declaration| {
                file.ast.local_alias_binding(declaration).is_some()
                    || crate::source_discovery::import_alias_declaration(file, declaration)
            })
            && self.may_precede_initialization(file.path.as_str(), span)
    }

    fn files_by_path_index(&self, path: &str) -> Option<usize> {
        self.first_file_index(path)
    }

    /// ADR 1182: by `(file, binding)`, the module-level `let`s bound to a function and written, every write a module-level straight-line
    /// plain `name = <function literal>` (no compound or logical assignment, no destructuring, no write in a function or on a branch), with
    /// a declarator standing at the end of the last write and that write's function. Module code runs in order, so after the last write
    /// the binding holds its function, and nothing else can write it (an import cannot assign a module's binding).
    fn settling_lets(&self) -> Vec<((usize, usize), (Declarator, Span))> {
        let mut found = Vec::new();
        for (index, file) in crate::base::reads::file_slots(self.facts)
            .iter()
            .enumerate()
        {
            let ast = &file.ast;
            if ast.assignments.is_empty()
                || crate::interproc::property_gets::direct_eval_in(file, Span::new(0, u32::MAX))
            {
                continue;
            }
            for (position, binding) in ast.bindings.iter().enumerate() {
                let [name] = binding.names.as_slice() else {
                    continue;
                };
                if !binding.runtime_let
                    || binding.shape != BindingShape::Identifier
                    || binding.in_switch_case
                    || !binding.initializer_function
                    || executing_function(ast, binding.declaration).is_some()
                {
                    continue;
                }
                let mut last: Option<(&solid_facts::ast::AssignmentFact, Span)> = None;
                let mut settles = true;
                let mut written = false;
                for &reference in ast.references_to(name.span) {
                    if !ast.any_write_target_containing(reference) {
                        continue;
                    }
                    written = true;
                    let write = ast
                        .assignments
                        .iter()
                        .find(|assignment| ast.peel_ts_sugar_span(assignment.target) == reference);
                    let function = write.and_then(|write| {
                        let straight = ast.straight_line_assignments.iter().any(|expression| {
                            expression.start == write.target.start
                                && expression.end >= write.value_span.end
                        });
                        (straight
                            && !write.reads_target
                            && write.logical.is_none()
                            && executing_function(ast, reference).is_none())
                        .then(|| ast.function_at(ast.peel_ts_sugar_span(write.value_span)))
                        .flatten()
                        .filter(|function| !function.generator)
                    });
                    match (write, function) {
                        (Some(write), Some(function)) => {
                            if last.is_none_or(|(previous, _)| {
                                previous.target.start < write.target.start
                            }) {
                                last = Some((write, function.span));
                            }
                        }
                        _ => settles = false,
                    }
                }
                if let (true, true, Some((write, function))) = (settles, written, last) {
                    found.push((
                        (index, position),
                        (
                            Declarator {
                                initialized: write.value_span.end,
                                start: write.target.start,
                                container: None,
                            },
                            function,
                        ),
                    ));
                }
            }
        }
        found
    }

    /// ADR 1182: by `((file, receiver binding), member name)`, the members of a module-level `const` bound to a closed object literal that
    /// module code replaces: every reference of the receiver is in its own file and is the object of a member call or of a module-level
    /// straight-line plain `receiver.name = <function literal>`; no function written in the literal or assigned to it reads `this`,
    /// `arguments` or `super` (none can write a member through the receiver); no direct `eval`. With a declarator standing at the end of
    /// the name's last write, and that write's function: after it the member holds that function, and no other code can reach the object.
    fn settling_members(&self) -> Vec<SettlingMember> {
        let mut found = Vec::new();
        for (index, file) in crate::base::reads::file_slots(self.facts)
            .iter()
            .enumerate()
        {
            let ast = &file.ast;
            if ast.assignments.is_empty()
                || crate::interproc::property_gets::direct_eval_in(file, Span::new(0, u32::MAX))
            {
                continue;
            }
            let exposes_receiver = |function: &solid_facts::ast::FunctionFact| {
                function.kind != FunctionKind::Arrow
                    && file.source_text(function.span).is_none_or(|text| {
                        text.contains("this")
                            || text.contains("arguments")
                            || text.contains("super")
                    })
            };
            for (position, binding) in ast.bindings.iter().enumerate() {
                let [name] = binding.names.as_slice() else {
                    continue;
                };
                if !binding.immutable
                    || binding.shape != BindingShape::Identifier
                    || binding.in_switch_case
                    || executing_function(ast, binding.declaration).is_some()
                {
                    continue;
                }
                let Some(initializer) =
                    binding.initializer.map(|span| ast.peel_ts_sugar_span(span))
                else {
                    continue;
                };
                if !ast.object_get_shapes.iter().any(|shape| {
                    shape.span == initializer
                        && shape.closed
                        && shape.properties.iter().all(|p| p.getter.is_none())
                }) || ast.functions_within(initializer).any(&exposes_receiver)
                {
                    continue;
                }
                // Every reference in this file: the receiver is not exported or handed anywhere.
                let all_here =
                    self.entities
                        .at(file.path.as_str(), name.span)
                        .is_some_and(|symbol| {
                            self.symbol_references_if_present(symbol.as_str())
                                .is_some_and(|references| {
                                    references.iter().all(|reference| {
                                        reference.path.as_ref() == file.path.as_str()
                                    })
                                })
                        });
                if !all_here
                    || ast
                        .exports
                        .iter()
                        .any(|export| export.span.contains(name.span))
                {
                    continue;
                }
                let mut last = HashMap::<String, (Span, u32, Span)>::default();
                let mut closed = true;
                for &reference in ast.references_to(name.span) {
                    let member = ast.members.iter().find(|member| {
                        ast.peel_ts_sugar_span(member.object) == reference
                            && !ast.computed_members.contains(&member.span)
                            && !ast.optional_members.contains(&member.span)
                    });
                    let Some(member) = member else {
                        closed = false;
                        break;
                    };
                    let called = ast
                        .calls_with_peeled_callee(member.span)
                        .any(|call| !call.construct);
                    if called {
                        continue;
                    }
                    let write = ast.assignments.iter().find(|assignment| {
                        ast.peel_ts_sugar_span(assignment.target) == member.span
                    });
                    let function = write.and_then(|write| {
                        let straight = ast.straight_line_assignments.iter().any(|expression| {
                            expression.start == write.target.start
                                && expression.end >= write.value_span.end
                        });
                        (straight
                            && !write.reads_target
                            && write.logical.is_none()
                            && executing_function(ast, reference).is_none())
                        .then(|| ast.function_at(ast.peel_ts_sugar_span(write.value_span)))
                        .flatten()
                        .filter(|function| !function.generator && !exposes_receiver(function))
                    });
                    let (Some(write), Some(function), Some(property)) =
                        (write, function, file.source_text(member.property))
                    else {
                        closed = false;
                        break;
                    };
                    let entry = last.entry(property.to_owned()).or_insert((
                        write.target,
                        write.value_span.end,
                        function.span,
                    ));
                    if entry.0.start < write.target.start {
                        *entry = (write.target, write.value_span.end, function.span);
                    }
                }
                if !closed {
                    continue;
                }
                for (property, (target, end, function)) in last {
                    found.push((
                        ((index, position), property),
                        (
                            Declarator {
                                initialized: end,
                                start: target.start,
                                container: None,
                            },
                            function,
                        ),
                    ));
                }
            }
        }
        found.sort_by(|a, b| (a.0.0, &a.0.1).cmp(&(b.0.0, &b.0.1)));
        found
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

    /// ADR 0919: whether member stability assumes member `name` of an object
    /// created in the file at `owner` (no visible write can replace it),
    /// recorded as the probe [`crate::base::memo::probes::Probe::Stable`]
    /// asks it. A path this build does not analyze answers no.
    pub(crate) fn member_stable_in(&self, owner: &str, name: &str) -> bool {
        let Some(id) = self.file_table.interner().get(owner) else {
            return false;
        };
        self.note_probe_read(crate::base::memo::probes::Probe::Stable, || {
            crate::base::memo::probes::PolledKey::FileText(id, std::sync::Arc::from(name))
        });
        matches!(
            self.member_stability().state_of(self.facts, owner, name),
            crate::interproc::accessor_provenance::Stability::Assumed
        )
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
        visiting: &mut Proving,
    ) -> bool {
        let declaring_file = &crate::base::reads::file_slots(self.facts)[declaring];
        let caller_file = &crate::base::reads::file_slots(self.facts)[caller];
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
        let stand_in = scratch.stand_in.get(&(declaring, binding)).copied();
        if stand_in.is_none() && initializer.contains(span) {
            // Inside the function expression itself: it runs only once it
            // exists, which is after the declarator. Inside a call's
            // arguments (`const x = wrap(() => x())`) the callee may run it
            // before `x` is initialized.
            return binding_fact.initializer_function;
        }
        let Some(declarator) = stand_in.or_else(|| {
            scratch
                .declarators
                .entry((declaring, binding))
                .or_insert_with(|| Self::declarator(declaring_file, binding_fact))
                .as_ref()
                .copied()
        }) else {
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
        visiting: &mut Proving,
    ) -> bool {
        if visiting.assumed(&(file, function.span)) {
            return true;
        }
        let facts = &crate::base::reads::file_slots(self.facts)[file];
        let Some(name) = function.name.as_ref() else {
            return false;
        };
        let Some(symbol) = self.entities.at(facts.path.as_str(), name.span) else {
            return false;
        };
        let Some(references) = self.symbol_references_if_present(symbol.as_str()) else {
            return false;
        };
        visiting.stack.push((file, function.span));
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
        visiting.finish(proved);
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
        visiting: &mut Proving,
    ) -> bool {
        if visiting.assumed(&(file, arrow)) {
            // A mention inside the arrow's own proof: it runs only when the
            // arrow does.
            return true;
        }
        let facts = &crate::base::reads::file_slots(self.facts)[file];
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
        visiting.stack.push((file, arrow));
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
        visiting.finish(proved);
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
        let span = Span::new(start, end);
        lookup.mention_may_precede_initialization_within(location.path.as_ref(), span)
            || lookup.unordered_mention_precedes_in_function(location.path.as_ref(), span)
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
