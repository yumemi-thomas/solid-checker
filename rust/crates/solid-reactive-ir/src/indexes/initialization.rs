//! Temporal dead zone: whether a reference to a function-valued binding runs
//! after the binding's declarator did.
//!
//! `const helper = () => ...` is not hoisted. A call that reaches it before
//! the declarator ran throws `ReferenceError` (a `var` reads `undefined` and
//! throws `TypeError`) and the body never runs, and `tsc` is silent when the
//! early use sits inside another function body. A call edge to such a binding
//! therefore proves execution only where the reference provably follows the
//! declarator. This module decides that per reference; a reference it cannot
//! decide is withheld from call sites and reachability, so a finding that
//! rested on it is uncertifiable, never a violation (ADR 0339).

use std::sync::{Arc, Mutex, OnceLock};

use super::*;
use crate::object_members::ModuleCensus;
use solid_facts::ast::{BindingFact, BindingShape, FunctionKind};

/// Per module and declarator: the module-level functions that may run first.
type RunningBeforeDeclarator = HashMap<(String, Span), Arc<HashSet<Span>>>;

#[derive(Default)]
pub(super) struct State {
    rejected: OnceLock<HashMap<String, HashSet<Span>>>,
    census: OnceLock<ModuleCensus>,
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
}

fn function_valued(binding: &BindingFact) -> bool {
    binding.shape == BindingShape::Identifier
        && binding.names.len() == 1
        && binding.initializer.is_some()
        && (binding.initializer_function || binding.call_initializer.is_some())
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
    /// Whether the code at `span` in `path` mentions a function-valued
    /// binding at a point not proven to follow its declarator. Such a mention
    /// supplies no call site and no reachability edge.
    pub(crate) fn may_precede_initialization(&self, path: &str, span: Span) -> bool {
        let Some(spans) = self.uninitialized_references().get(path) else {
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

    /// Every withheld mention, by path. Bound into the cross-file proof
    /// digest: another module's imports decide a mention here.
    pub(super) fn uninitialized_references(&self) -> &HashMap<String, HashSet<Span>> {
        self.initialization.rejected.get_or_init(|| {
            let mut scratch = Scratch::default();
            // Every mention of each function-valued binding, by binding.
            let mut mentions = Vec::<(usize, usize, usize, Span)>::new();
            for (path, references) in self.bindings_by_reference() {
                let Some(caller) = self.files_by_path_index(path) else {
                    continue;
                };
                for ((start, end), resolution) in references {
                    let declaring = &self.facts.files[resolution.file];
                    let binding = &declaring.ast.bindings[resolution.binding];
                    if !function_valued(binding) {
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
                    mentions.push((caller, resolution.file, resolution.binding, span));
                }
            }
            let mut rejected = HashMap::<String, HashSet<Span>>::default();
            for (caller, declaring, binding, span) in mentions {
                if self.initialized_before(
                    &mut scratch,
                    caller,
                    span,
                    declaring,
                    binding,
                    &mut Vec::new(),
                ) {
                    continue;
                }
                rejected
                    .entry(self.facts.files[caller].path.to_string())
                    .or_default()
                    .insert(span);
            }
            rejected
        })
    }

    fn files_by_path_index(&self, path: &str) -> Option<usize> {
        self.facts
            .files
            .iter()
            .position(|file| file.path.as_str() == path)
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
    /// code that creates it. A hoisted declaration or a method may run
    /// anywhere, so it has none.
    fn module_level_position(ast: &solid_facts::ast::AstFacts, span: Span) -> Option<u32> {
        let mut position = span.start;
        let mut executor = executing_function(ast, span);
        while let Some(function) = executor {
            match function.kind {
                FunctionKind::Arrow | FunctionKind::Expression
                    if function.method_name.is_none() =>
                {
                    position = function.span.start;
                    executor = creating_function(ast, function);
                }
                _ => return None,
            }
        }
        Some(position)
    }

    pub(super) fn module_census(&self) -> &ModuleCensus {
        self.initialization
            .census
            .get_or_init(|| ModuleCensus::new(self.facts, self))
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
        if !(binding_fact.immutable || binding_fact.runtime_let) {
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
        // Default parameter initializers run before the body and may throw
        // (a call of a function or a class not yet initialized): a mention
        // that reaches the body is proved only if they cannot. A default that
        // is no more than a literal cannot; any other is an early mention of
        // everything it references, which is not proved, so it is withheld.
        let own_mention = caller == declaring && initializer.contains(span);
        if !own_mention
            && binding_fact.initializer_function
            && declaring_file
                .ast
                .functions_within(initializer)
                .max_by_key(|function| function.span.end - function.span.start)
                .is_none_or(|function| !function.parameters_inert)
        {
            return false;
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
                    Self::module_level_position(&caller_file.ast, span),
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
                // created: it is as late as the code that creates it.
                FunctionKind::Arrow | FunctionKind::Expression
                    if function.method_name.is_none() =>
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
