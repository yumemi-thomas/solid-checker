//! Where code provably runs synchronously inside a user effect's apply
//! callback (the dialect's *effect phase*,
//! [`solid_dialect::Dialect::callback_runs_in_effect_phase`]).
//!
//! The question is the reverse of an execution role: not "what role does this
//! span run in" but "does every step between this span and the effect's apply
//! run on the apply's own stack". The walk climbs from the span outward, one
//! function layer at a time, and answers only where each layer is proven:
//!
//! - the layer is the function written as the effect's apply argument: the
//!   walk is done;
//! - the layer is written as the callback of a primitive the dialect states
//!   runs it during the call ([`solid_dialect::Dialect::synchronous_callback_slot`]:
//!   `untrack`, `createRoot`, `runWithOwner`, `flush(fn)`), or of a
//!   standard-library call the audited rows model as invoking it inline
//!   (`forEach`, `map`, ...): the walk continues from that call;
//! - the layer is an immediately invoked function: the walk continues from the
//!   invoking call;
//! - the layer is a project function bound to a name nothing reassigns: the
//!   walk continues from any one resolved call of it, so a helper reached from
//!   the effect is proven by that call alone (the other calls of the helper
//!   are no evidence either way).
//!
//! Each layer's site, and the `createEffect` registration site, must also be on
//! a proven normal path ([`solid_facts::ast::site_reachable`]): a positive
//! allowlist of constructs from the function entry (blocks, non-constant
//! `if`/loops, non-literal `for`-`of`, `try` blocks but never `catch`, operands
//! of calls and short circuits whose left side is not constant), with no
//! proven abrupt completion before the site (`return`, `throw`, a call that
//! never returns, a throwing same-file getter). The feasible-path premise is
//! what remains: a non-constant condition may take either branch, and a call
//! not proven never to return returns. Array callbacks count only on a receiver
//! that is not an array literal, never for `sort`, and for `reduce` only with an
//! initial value. An effect whose compute is a function written inline, or an
//! exactly resolved project function, must be able to return a value
//! ([`solid_facts::ast::function_may_return`]) and not be `async` or a
//! generator, or the apply is not claimed; any other compute (a signal
//! accessor, a member, a package function) is assumed able to.
//!
//! The queue the nested `flush` finds running must be the apply's: the file
//! holding the `flush` call and the file holding the `createEffect` call must
//! load the same installed `solid-js` at runtime
//! ([`crate::PackageInstanceEvidence`], module resolution with symlinks
//! resolved), and both calls' declarations must be in one package directory.
//!
//! Anything else answers `None`: an unresolved callee, a callback handed to a
//! timer, listener or promise, a generator (calling one runs none of its body),
//! a method, a parameter initializer, an instance field initializer, a call
//! through a binding that may have been replaced. An async layer is on the
//! stack only up to its first suspension.

use crate::{HashMap, HashSet};

use solid_facts::FileFacts;
use solid_facts::ast::{ArgumentValueKind, FunctionCompletion, FunctionFact, PathEvidence};
use solid_facts::core::Span;

use crate::effect_execution::{self, ApplyExecution};
use crate::execution_role::{inside_instance_field, project_callee_at_parameter};
use crate::indexes::SemanticLookup;
use crate::owners::{containing_ast_function, function_binding_name, written_directly_in};

/// Call layers one proof may cross. A chain this long is not one a reader can
/// follow, and the bound keeps a cycle-free but wide helper graph cheap.
const MAX_LAYERS: usize = 12;

/// How many function declarations one question may expand before it gives up:
/// a project with thousands of call sites of one helper stays bounded.
const BUDGET: usize = 400;

/// How far an export chain gets.
enum Chain {
    /// The file that imports the package.
    Found(String),
    /// The module does not export the name.
    Missing,
    /// A hop could not be resolved, or the name is declared in the project.
    Unknown,
}

/// The `createEffect` call whose apply callback a span provably runs inside,
/// and what stood between them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EffectPhaseOrigin {
    /// Path of the file holding the effect call.
    pub(crate) path: String,
    /// Project functions the walk entered through a call, outermost last.
    pub(crate) helper_calls: usize,
}

/// The effect call whose apply callback provably runs the code at `span`
/// synchronously, or `None` where that is not proven.
pub(crate) fn effect_phase_origin(
    file: &FileFacts,
    span: Span,
    package_root: &str,
    instances: &crate::PackageInstanceEvidence,
    reachable_calls: &HashMap<typefacts::Location, usize>,
    lookup: &SemanticLookup<'_>,
) -> Option<EffectPhaseOrigin> {
    let mut walk = Walk {
        lookup,
        package_root,
        flush_instance: "",
        instances,
        reachable_calls,
        files: std::cell::OnceCell::new(),
        imported_accessors: HashMap::default(),
        returns: HashMap::default(),
        visiting: HashSet::default(),
        entering: HashSet::default(),
        budget: BUDGET,
    };
    // The instance the flush call's own import loads, followed through
    // re-exports to the file that imports the package; unresolved is no proof.
    let flush_call = file.ast.calls.iter().find(|call| call.span == span)?;
    let flush_file = walk.declaring_file(file, flush_call.callee)?;
    let flush_instance = instances.solid_js_by_file.get(flush_file.as_str())?;
    walk.flush_instance = flush_instance;
    walk.layer(file, span, 0)
}

struct Walk<'l, 'a> {
    lookup: &'l SemanticLookup<'a>,
    /// The package instance the starting call's declaration is in: the effect
    /// call must resolve to the same one, or its queue is not the one running.
    package_root: &'l str,
    /// The `solid-js` directory the flush call's file loads at runtime, and
    /// what every analyzed file loads.
    flush_instance: &'l str,
    instances: &'l crate::PackageInstanceEvidence,
    /// Calls the reachability stage found on a path from a root, by callee.
    reachable_calls: &'l HashMap<typefacts::Location, usize>,
    /// The analyzed files by path, built when a question first needs them.
    files: std::cell::OnceCell<HashMap<&'a str, &'a FileFacts>>,
    /// The throwing accessor names of the files a file imports, by file.
    imported_accessors: HashMap<String, Vec<Option<String>>>,
    /// How a call of a project function ends, by function.
    returns: HashMap<(String, Span), Option<FunctionCompletion>>,
    /// Functions the walk is inside through a call, so a call cycle ends.
    visiting: HashSet<(String, Span)>,
    /// Functions whose entry is being proven, so a call cycle ends.
    entering: HashSet<(String, Span)>,
    budget: usize,
}

impl<'a> Walk<'_, 'a> {
    fn layer(
        &mut self,
        file: &FileFacts,
        span: Span,
        helper_calls: usize,
    ) -> Option<EffectPhaseOrigin> {
        if helper_calls > MAX_LAYERS || inside_instance_field(file, span) {
            return None;
        }
        // Module scope: nothing around the span runs it.
        let literal = containing_ast_function(&file.ast, span)?;
        // A parameter initializer lies outside every body, so the body-based
        // query above placed it in the outer function; it runs when its own
        // function is called, which no layer here proves.
        if !written_directly_in(&file.ast, literal, span) {
            return None;
        }
        // Calling a generator runs none of its body; a method or accessor is
        // reached through a receiver this walk does not resolve.
        if literal.generator || literal.method_name.is_some() {
            return None;
        }
        if literal.r#async && !runs_before_first_suspension(file, literal, span) {
            return None;
        }
        // A site no normal path reaches never runs. A function the analysis
        // cannot parse or find is no proof either way.
        let evidence = self.evidence(file, Some(literal), 0);
        if solid_facts::ast::site_reachable(
            std::path::Path::new(file.path.as_str()),
            &file.source,
            literal.span,
            span,
            &evidence,
        ) != Some(true)
        {
            return None;
        }
        if let Some((call, index)) = argument_position(file, literal) {
            return self.argument_layer(file, call, index, helper_calls);
        }
        if let Some(call) = iife_call(file, literal) {
            return self.layer(file, call.span, helper_calls);
        }
        self.named_layer(file, literal, helper_calls)
    }

    /// `literal` is written as an argument of `call`.
    fn argument_layer(
        &mut self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        index: usize,
        helper_calls: usize,
    ) -> Option<EffectPhaseOrigin> {
        let dialect = self.lookup.dialect;
        if call.construct
            || call
                .arguments
                .iter()
                .take(index + 1)
                .any(|argument| argument.spread)
        {
            return None;
        }
        if let Some(primitive) = self.lookup.primitive_at_call(file, call.span) {
            if dialect.callback_runs_in_effect_phase(primitive, index) {
                // An apply the runtime may never run proves nothing, nor does
                // one whose compute may always throw, nor one of another
                // package instance's queue.
                return (effect_execution::at_call(file, call, primitive, index, self.lookup)
                    == ApplyExecution::Proven
                    && self.registration_reachable(file, call, 0)
                    && self.compute_may_return(file, call)
                    && self.same_package_instance(file, call))
                .then(|| EffectPhaseOrigin {
                    path: file.path.to_string(),
                    helper_calls,
                });
            }
            return if dialect.synchronous_callback_slot(primitive, index, call.arguments.len()) {
                self.layer(file, call.span, helper_calls)
            } else {
                None
            };
        }
        // A standard-library call the audited rows model as invoking the
        // argument before it returns.
        let resolved = self
            .lookup
            .resolved_callee_call(file, call.callee)
            .filter(|resolved| {
                resolved
                    .declaration
                    .as_ref()
                    .is_some_and(|declaration| declaration.standard_library)
            })?;
        // Only a receiver that is not an array literal (it may be empty,
        // sparse or a single element), and a method that always invokes its
        // callback on an element: `reduce` without an initial value skips the
        // first element, and `sort` may not compare at all.
        let member = file
            .ast
            .member_at(file.ast.peel_ts_sugar_span(call.callee))?;
        if file
            .source_text(file.ast.peel_ts_sugar_span(member.object))
            .is_none_or(|object| object.trim_start().starts_with('['))
        {
            return None;
        }
        match file.source_text(member.property)? {
            "sort" | "toSorted" => return None,
            "reduce" | "reduceRight" if call.arguments.len() < 2 => return None,
            _ => {}
        }
        let callability = self
            .lookup
            .entity_at(file.path.as_str(), call.arguments[index].span)
            .and_then(|entity| entity.callability);
        matches!(
            crate::runtime_semantics::argument_behavior(resolved, callability, index),
            Some(crate::runtime_semantics::RuntimeArgumentBehavior::InlineCallback)
        )
        .then(|| self.layer(file, call.span, helper_calls))
        .flatten()
    }

    /// Whether the effect call's `createEffect` is the one the flush call's
    /// queue runs: its declaration is in the package the walk started from, and
    /// the import it is called through, followed through re-exports to the file
    /// that imports the package, loads the same installed `solid-js` copy at
    /// runtime as the flush call's import does.
    fn same_package_instance(
        &mut self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
    ) -> bool {
        let Some(declaring) = self.declaring_file(file, call.callee) else {
            return false;
        };
        self.instances
            .solid_js_by_file
            .get(declaring.as_str())
            .map(String::as_str)
            == Some(self.flush_instance)
            && self
                .lookup
                .resolved_callee_call(file, call.callee)
                .and_then(|resolved| resolved.declaration.as_ref())
                .and_then(|declaration| package_root(declaration.location.path.as_ref()))
                .is_some_and(|root| root == self.package_root)
    }

    fn files(&self) -> &HashMap<&'a str, &'a FileFacts> {
        self.files.get_or_init(|| {
            self.lookup
                .files()
                .iter()
                .map(|file| (file.path.as_str(), file))
                .collect()
        })
    }

    /// The analyzed file a module specifier written at `span` in `file` lands
    /// on (the resolver's file, or the one the compiler included in its place);
    /// `None` when the resolution is not attested.
    fn landing(&self, file: &FileFacts, span: Span, module: &str) -> Option<String> {
        if let Some(imports) = self.lookup.resolved_imports() {
            let solid_facts::SpecifierAttestation::Attested(row) =
                imports.specifier(file.path.as_str(), span, module)
            else {
                return None;
            };
            if row.resolved_path.is_empty() {
                return None;
            }
            return Some(if row.included_path.is_empty() {
                row.resolved_path.to_string()
            } else {
                row.included_path.to_string()
            });
        }
        solid_facts::resolve_relative_module_path(
            file.path.as_str(),
            module,
            self.lookup.files().iter().map(|file| file.path.as_str()),
        )
        .map(|path| path.to_string())
    }

    /// The analyzed file whose import or re-export of the package declares the
    /// function `callee` calls: the callee's import binding, followed through
    /// every barrel and `export *` to the file that imports from outside the
    /// project. `None` when any hop cannot be resolved exactly.
    fn declaring_file(&self, file: &FileFacts, callee: Span) -> Option<String> {
        let callee = file.ast.peel_ts_sugar_span(callee);
        let (object, name) = match file.ast.member_at(callee) {
            Some(member) => (
                file.ast.peel_ts_sugar_span(member.object),
                Some(file.source_text(member.property)?.to_owned()),
            ),
            None => (callee, None),
        };
        let declaration = file.ast.reference_declaration(object)?;
        let (import, binding) = file.ast.imports.iter().find_map(|import| {
            import
                .bindings
                .iter()
                .find(|binding| binding.local.span == declaration)
                .map(|binding| (import, binding))
        })?;
        if import.type_only || binding.type_only {
            return None;
        }
        let name = match (binding.kind, name) {
            (solid_facts::ast::ImportKind::Namespace, Some(name)) => name,
            (solid_facts::ast::ImportKind::Namespace, None) | (_, Some(_)) => return None,
            (solid_facts::ast::ImportKind::Default, None) => "default".to_owned(),
            (_, None) => binding.imported.as_deref()?.to_owned(),
        };
        match self.follow_export(file, import.span, &import.module, &name, 0) {
            Chain::Found(path) => Some(path),
            Chain::Missing | Chain::Unknown => None,
        }
    }

    /// Follows the export `name` of the module a declaration at `span` in
    /// `file` names, to the file that imports it from outside the project.
    fn follow_export(
        &self,
        file: &FileFacts,
        span: Span,
        module: &str,
        name: &str,
        depth: usize,
    ) -> Chain {
        if depth > 8 {
            return Chain::Unknown;
        }
        let Some(landing) = self.landing(file, span, module) else {
            return Chain::Unknown;
        };
        let Some(target) = self.files().get(landing.as_str()).copied() else {
            // Outside the project: this file's import is the package's.
            return Chain::Found(file.path.to_string());
        };
        let mut stars = Vec::new();
        for export in target
            .ast
            .module_level_exports()
            .filter(|export| !export.type_only)
        {
            if export
                .declarations
                .iter()
                .any(|declaration| declaration.exported == name)
            {
                // Declared here, not re-exported.
                return Chain::Unknown;
            }
            if let Some(specifier) = export
                .specifiers
                .iter()
                .find(|specifier| specifier.exported == name && !specifier.type_only)
            {
                let Some(local) = target.source_text(specifier.local.span) else {
                    return Chain::Unknown;
                };
                if let Some(module) = export.module.as_deref() {
                    return self.follow_export(target, export.span, module, local, depth + 1);
                }
                // `import { a } from "m"; export { a as name }`.
                let Some((import, binding)) = target.ast.imports.iter().find_map(|import| {
                    import
                        .bindings
                        .iter()
                        .find(|binding| {
                            !binding.type_only
                                && target.source_text(binding.local.span) == Some(local)
                        })
                        .map(|binding| (import, binding))
                }) else {
                    return Chain::Unknown;
                };
                if import.type_only {
                    return Chain::Unknown;
                }
                let imported = match binding.kind {
                    solid_facts::ast::ImportKind::Namespace => return Chain::Unknown,
                    solid_facts::ast::ImportKind::Default => "default",
                    _ => match binding.imported.as_deref() {
                        Some(imported) => imported,
                        None => return Chain::Unknown,
                    },
                };
                return self.follow_export(
                    target,
                    import.span,
                    &import.module,
                    imported,
                    depth + 1,
                );
            }
            if export.kind == solid_facts::ast::ExportKind::All
                && export.namespace.is_none()
                && export.module.is_some()
            {
                stars.push(export);
            }
        }
        let mut found: Option<String> = None;
        for export in stars {
            let module = export.module.as_deref().unwrap_or_default();
            match self.follow_export(target, export.span, module, name, depth + 1) {
                Chain::Found(path) => {
                    if found.as_ref().is_some_and(|known| known != &path) {
                        return Chain::Unknown;
                    }
                    found = Some(path);
                }
                Chain::Missing => {}
                Chain::Unknown => return Chain::Unknown,
            }
        }
        found.map_or(Chain::Missing, Chain::Found)
    }

    /// Whether the `createEffect` call is itself on a proven path: reachable
    /// from a reachability root (every enclosing function of it was entered),
    /// and on a normal path from the entry of its enclosing function (or the
    /// module body). A registration that never runs applies nothing; one in
    /// another effect's apply runs only when that effect's compute returns.
    fn registration_reachable(
        &mut self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        depth: usize,
    ) -> bool {
        if depth > MAX_LAYERS {
            return false;
        }
        let path = std::path::Path::new(file.path.as_str());
        match containing_ast_function(&file.ast, call.span) {
            Some(function) => {
                if !written_directly_in(&file.ast, function, call.span)
                    || !self
                        .reachable_calls
                        .contains_key(&crate::location(file.path.shared(), call.callee))
                {
                    return false;
                }
                let evidence = self.evidence(file, Some(function), 0);
                if solid_facts::ast::site_reachable(
                    path,
                    &file.source,
                    function.span,
                    call.span,
                    &evidence,
                ) != Some(true)
                {
                    return false;
                }
                // The function exists only once the call that receives it is
                // evaluated, and an effect's apply runs only after a compute
                // that returns.
                if let Some((outer, index)) = argument_position(file, function) {
                    if let Some(primitive) = self.lookup.primitive_at_call(file, outer.span)
                        && self
                            .lookup
                            .dialect
                            .callback_runs_in_effect_phase(primitive, index)
                        && !(effect_execution::at_call(file, outer, primitive, index, self.lookup)
                            == ApplyExecution::Proven
                            && self.compute_may_return(file, outer))
                    {
                        return false;
                    }
                    return self.registration_reachable(file, outer, depth + 1);
                }
                if let Some(invoking) = iife_call(file, function) {
                    return self.registration_reachable(file, invoking, depth + 1);
                }
                self.function_entered(file, function, depth)
            }
            None => {
                let evidence = self.module_evidence(file);
                solid_facts::ast::module_site_reachable(path, &file.source, call.span, &evidence)
                    == Some(true)
            }
        }
    }

    /// Whether `function` (written neither as an argument nor invoked where it
    /// is written) provably runs: it is a root (an exported or rendered
    /// component, an exported function), or one exact call of it is itself on
    /// a reachable path. A function nothing calls never runs.
    fn function_entered(
        &mut self,
        file: &FileFacts,
        function: &FunctionFact,
        depth: usize,
    ) -> bool {
        let exported = file
            .ast
            .exports
            .iter()
            .any(|export| export.span.contains(function.span))
            || self
                .lookup
                .typescript_file(file.path.as_str())
                .is_some_and(|facts| {
                    facts.functions.iter().any(|candidate| {
                        candidate.exported
                            && candidate.body.start_byte == u64::from(function.body.start)
                            && candidate.body.end_byte == u64::from(function.body.end)
                    })
                });
        if exported || self.lookup.function_is_component(file, function) {
            return true;
        }
        if function.generator
            || function.method_name.is_some()
            || function_binding_name(file, function).is_none()
            || !self.lookup.function_value_is_current(file, function)
        {
            return false;
        }
        let key = (file.path.to_string(), function.span);
        if !self.entering.insert(key.clone()) {
            return false;
        }
        let mut entered = false;
        for (caller, callee) in self
            .lookup
            .function_call_sites(file.path.as_str(), function.span)
        {
            if self.budget == 0 {
                break;
            }
            self.budget -= 1;
            let Some(call) = caller.ast.calls.iter().find(|call| {
                !call.construct && caller.ast.peel_ts_sugar_span(call.callee) == callee
            }) else {
                continue;
            };
            let exact = project_callee_at_parameter(caller, call, 0, self.lookup).is_some_and(
                |(callee_file, candidate, exact)| {
                    exact
                        && callee_file.path.as_str() == file.path.as_str()
                        && candidate.span == function.span
                },
            );
            if exact && self.registration_reachable(caller, call, depth + 1) {
                entered = true;
                break;
            }
        }
        self.entering.remove(&key);
        entered
    }

    /// The path evidence of the module body: its calls that never return.
    fn module_evidence(&mut self, file: &FileFacts) -> PathEvidence {
        let mut evidence = PathEvidence::default();
        for call in &file.ast.calls {
            if call.construct || containing_ast_function(&file.ast, call.span).is_some() {
                continue;
            }
            if let Some(throws) = self.call_divergence(file, call, 0) {
                evidence.diverging.push(call.span);
                if throws {
                    evidence.throwing.push(call.span);
                }
            }
        }
        evidence.accessors = self.imported_accessors(file);
        evidence
    }

    /// Whether the effect's compute can return a value, so that its apply
    /// runs: a function written inline, or an exactly resolved project
    /// function, must show a normal return; anything else is assumed to.
    fn compute_may_return(&mut self, file: &FileFacts, call: &solid_facts::ast::CallFact) -> bool {
        let Some(compute) = call.arguments.first() else {
            return false;
        };
        let span = file.ast.peel_ts_sugar_span(compute.span);
        let function = if let Some(function) = file.ast.function_at(span) {
            Some((file, function))
        } else if compute.value == ArgumentValueKind::Identifier {
            self.lookup
                .function_called_at(file.path.as_str(), span)
                .filter(|(function_file, function)| {
                    self.lookup
                        .function_value_is_current(function_file, function)
                })
        } else {
            None
        };
        match function {
            // Calling an async function or a generator never throws, but the
            // value is a promise or an iterator: the apply is not claimed.
            Some((_, function)) if function.r#async || function.generator => false,
            Some((function_file, function)) => self
                .completion(function_file, function, 0)
                .is_some_and(|completion| completion.returns),
            None => true,
        }
    }

    /// How a call of `function` ends, by the completion analysis; `None` when
    /// it cannot be analyzed or the budget is spent.
    fn completion(
        &mut self,
        file: &FileFacts,
        function: &FunctionFact,
        depth: usize,
    ) -> Option<FunctionCompletion> {
        let key = (file.path.to_string(), function.span);
        if let Some(known) = self.returns.get(&key) {
            return *known;
        }
        if depth > 3 || self.budget == 0 {
            return None;
        }
        self.budget -= 1;
        // A call cycle answers "unknown" to its own inner calls.
        self.returns.insert(key.clone(), None);
        let evidence = self.evidence(file, Some(function), depth + 1);
        let answer = solid_facts::ast::function_completion(
            std::path::Path::new(file.path.as_str()),
            &file.source,
            function.span,
            &evidence,
        );
        self.returns.insert(key, answer);
        answer
    }

    /// What is known about the code `function` runs: the calls written
    /// directly in it that provably never return normally (and which of them
    /// end by throwing), and the accessors of the files it imports.
    fn evidence(
        &mut self,
        file: &FileFacts,
        function: Option<&FunctionFact>,
        depth: usize,
    ) -> PathEvidence {
        let Some(function) = function else {
            return self.module_evidence(file);
        };
        let mut evidence = PathEvidence::default();
        for call in &file.ast.calls {
            if call.construct
                || !function.body.contains(call.span)
                || !written_directly_in(&file.ast, function, call.span)
            {
                continue;
            }
            if let Some(throws) = self.call_divergence(file, call, depth) {
                evidence.diverging.push(call.span);
                if throws {
                    evidence.throwing.push(call.span);
                }
            }
        }
        evidence.accessors = self.imported_accessors(file);
        evidence
    }

    /// The names of the getters, setters and Proxy traps that cannot return,
    /// written in the project files `file` imports or re-exports from
    /// (transitively, bounded): a property read or write with such a name may
    /// reach one.
    fn imported_accessors(&mut self, file: &FileFacts) -> Vec<Option<String>> {
        if let Some(known) = self.imported_accessors.get(file.path.as_str()) {
            return known.clone();
        }
        const MAX_FILES: usize = 48;
        let mut seen = HashSet::<String>::default();
        seen.insert(file.path.to_string());
        let mut queue = vec![file.path.to_string()];
        let mut names = Vec::<Option<String>>::new();
        let mut cursor = 0;
        while cursor < queue.len() && seen.len() <= MAX_FILES {
            let current = queue[cursor].clone();
            cursor += 1;
            let Some(current_file) = self.files().get(current.as_str()).copied() else {
                continue;
            };
            let modules = current_file
                .ast
                .imports
                .iter()
                .filter(|import| !import.type_only)
                .map(|import| (import.span, import.module.as_str()))
                .chain(
                    current_file
                        .ast
                        .module_level_exports()
                        .filter(|export| !export.type_only)
                        .filter_map(|export| Some((export.span, export.module.as_deref()?))),
                )
                .collect::<Vec<_>>();
            for (span, module) in modules {
                let Some(path) = self.landing(current_file, span, module) else {
                    continue;
                };
                if !seen.insert(path.clone()) {
                    continue;
                }
                let Some(imported) = self.files().get(path.as_str()).copied() else {
                    continue;
                };
                if let Some(found) = solid_facts::ast::throwing_accessors(
                    std::path::Path::new(imported.path.as_str()),
                    &imported.source,
                ) {
                    for name in found {
                        if !names.contains(&name) {
                            names.push(name);
                        }
                    }
                }
                queue.push(path);
            }
        }
        self.imported_accessors
            .insert(file.path.to_string(), names.clone());
        names
    }

    /// Whether the call provably never returns normally: `Some(true)` when it
    /// ends by throwing, `Some(false)` when it may run forever instead (or the
    /// rules cannot tell which), `None` when it is not proven to diverge.
    fn call_divergence(
        &mut self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        depth: usize,
    ) -> Option<bool> {
        // A project function reached by an exact call.
        let project = project_callee_at_parameter(file, call, 0, self.lookup)
            .filter(|(_, callee, exact)| *exact && !callee.generator && !callee.r#async);
        let completion = project
            .and_then(|(callee_file, callee, _)| self.completion(callee_file, callee, depth));
        // A project function that only throws or only loops.
        if let Some(completion) = completion
            && !completion.returns
        {
            return Some(completion.throws);
        }
        // A `never` signature (not `asserts`, which may return).
        if self
            .lookup
            .resolved_callee_call(file, call.callee)
            .is_some_and(|resolved| {
                resolved.never_returns
                    && resolved.validity == typefacts::ResolvedCallValidity::Valid
            })
        {
            return Some(false);
        }
        // An inline array method over a literal with a first element, whose
        // callback only throws: the first element reaches it.
        const PER_ELEMENT: [&str; 10] = [
            "forEach",
            "map",
            "filter",
            "find",
            "findIndex",
            "findLast",
            "findLastIndex",
            "some",
            "every",
            "flatMap",
        ];
        let member = file
            .ast
            .member_at(file.ast.peel_ts_sugar_span(call.callee))?;
        if !file
            .source_text(member.property)
            .is_some_and(|name| PER_ELEMENT.contains(&name))
            || solid_facts::ast::literal_array_element(
                std::path::Path::new(file.path.as_str()),
                &file.source,
                file.ast.peel_ts_sugar_span(member.object),
                0,
            )
            .is_none()
        {
            return None;
        }
        let resolved = self
            .lookup
            .resolved_callee_call(file, call.callee)
            .filter(|resolved| {
                resolved
                    .declaration
                    .as_ref()
                    .is_some_and(|declaration| declaration.standard_library)
            })?;
        let argument = call.arguments.first()?;
        let callability = self
            .lookup
            .entity_at(file.path.as_str(), argument.span)
            .and_then(|entity| entity.callability);
        if !matches!(
            crate::runtime_semantics::argument_behavior(resolved, callability, 0),
            Some(crate::runtime_semantics::RuntimeArgumentBehavior::InlineCallback)
        ) || argument.spread
        {
            return None;
        }
        let callback = file
            .ast
            .function_at(file.ast.peel_ts_sugar_span(argument.span))
            .filter(|callback| !callback.r#async && !callback.generator)?;
        self.completion(file, callback, depth)
            .filter(|completion| !completion.returns)
            .map(|completion| completion.throws)
    }

    /// `literal` is a function bound to a name: it runs during the effect
    /// phase when one resolved call of it does.
    fn named_layer(
        &mut self,
        file: &FileFacts,
        literal: &FunctionFact,
        helper_calls: usize,
    ) -> Option<EffectPhaseOrigin> {
        function_binding_name(file, literal)?;
        if !self.lookup.function_value_is_current(file, literal) {
            return None;
        }
        let key = (file.path.to_string(), literal.span);
        if !self.visiting.insert(key.clone()) {
            return None;
        }
        let mut found = None;
        for (caller, callee) in self
            .lookup
            .function_call_sites(file.path.as_str(), literal.span)
        {
            if self.budget == 0 {
                break;
            }
            self.budget -= 1;
            // A call expression of the function, not a JSX tag that renders it
            // or a `new`.
            let Some(call) = caller.ast.calls.iter().find(|call| {
                !call.construct && caller.ast.peel_ts_sugar_span(call.callee) == callee
            }) else {
                continue;
            };
            // The call must *exactly* run this function: a member callee only
            // when its receiver is proven to hold the declared method, a
            // binding only while nothing replaces it, no spread shifting the
            // arguments ([`project_callee_at_parameter`]'s `exact`).
            let exact = project_callee_at_parameter(caller, call, 0, self.lookup).is_some_and(
                |(callee_file, function, exact)| {
                    exact
                        && callee_file.path.as_str() == file.path.as_str()
                        && function.span == literal.span
                },
            );
            if !exact {
                continue;
            }
            if let Some(origin) = self.layer(caller, call.span, helper_calls + 1) {
                found = Some(origin);
                break;
            }
        }
        self.visiting.remove(&key);
        found
    }
}

/// The call and argument position at which `literal` is written as the
/// argument itself (behind TypeScript sugar at most). `wrap(() => …)` places a
/// literal in `wrap`'s argument; `wrap(function () {})()` does not place the
/// inner function anywhere this walk understands.
fn argument_position<'f>(
    file: &'f FileFacts,
    literal: &FunctionFact,
) -> Option<(&'f solid_facts::ast::CallFact, usize)> {
    file.ast
        .arguments_containing(literal.span)
        .filter(|(call, index)| {
            let argument = &call.arguments[*index];
            matches!(
                argument.value,
                ArgumentValueKind::Function | ArgumentValueKind::AsyncFunction
            ) && file.ast.peel_ts_sugar_span(argument.span) == literal.span
        })
        .min_by_key(|(call, _)| call.span.end - call.span.start)
}

/// The call that invokes `literal` where it is written: the literal is the
/// call's callee, behind TypeScript sugar at most.
fn iife_call<'f>(
    file: &'f FileFacts,
    literal: &FunctionFact,
) -> Option<&'f solid_facts::ast::CallFact> {
    file.ast
        .calls
        .iter()
        .filter(|call| !call.construct && file.ast.peel_ts_sugar_span(call.callee) == literal.span)
        .min_by_key(|call| call.span.end - call.span.start)
}

/// Whether the code at `span` in the async function `function` runs before
/// the function first suspends, so on the stack of whoever called it.
///
/// Every `await` of the body that starts before the span ends must contain it
/// (the awaited operand is evaluated first), no implicit suspension (`for
/// await`, an async `yield*`, `await using`) may start before it, and no loop
/// around it may contain a suspension that a later iteration would pass first.
pub(crate) fn runs_before_first_suspension(
    file: &FileFacts,
    function: &FunctionFact,
    span: Span,
) -> bool {
    let own = |candidate: Span| {
        containing_ast_function(&file.ast, candidate)
            .is_some_and(|owner| owner.span == function.span)
    };
    let awaits = file
        .ast
        .awaits
        .iter()
        .copied()
        .filter(|suspension| function.body.contains(*suspension) && own(*suspension))
        .collect::<Vec<_>>();
    let implicit = file
        .ast
        .implicit_suspensions
        .iter()
        .copied()
        .filter(|suspension| function.body.contains(*suspension) && own(*suspension))
        .collect::<Vec<_>>();
    let before = |suspension: &Span| suspension.start < span.end;
    if awaits
        .iter()
        .any(|suspension| before(suspension) && !suspension.contains(span))
        || implicit.iter().any(before)
    {
        return false;
    }
    !file.ast.loop_statements.iter().any(|looped| {
        looped.contains(span)
            && function.body.contains(*looped)
            && awaits
                .iter()
                .chain(&implicit)
                .any(|suspension| looped.contains(*suspension))
    })
}

/// The directory of the package a declaration file belongs to: the path up to
/// and including the package name after its last `node_modules/`, scope
/// included. `None` for a file outside any `node_modules`.
pub(crate) fn package_root(path: &str) -> Option<String> {
    let marker = "node_modules/";
    let at = path.rfind(marker)? + marker.len();
    let rest = &path[at..];
    let mut segments = rest.split('/');
    let first = segments.next()?;
    let name_len = if first.starts_with('@') {
        first.len() + 1 + segments.next()?.len()
    } else {
        first.len()
    };
    Some(path[..at + name_len].to_owned())
}

#[cfg(test)]
mod tests {
    use super::package_root;

    #[test]
    fn a_package_root_is_the_nearest_node_modules_package_directory() {
        assert_eq!(
            package_root("/p/node_modules/@solidjs/signals/dist/types/signals.d.ts").as_deref(),
            Some("/p/node_modules/@solidjs/signals")
        );
        // A copy nested under `solid-js` is another instance.
        assert_eq!(
            package_root(
                "/p/node_modules/solid-js/node_modules/@solidjs/signals/dist/types/signals.d.ts"
            )
            .as_deref(),
            Some("/p/node_modules/solid-js/node_modules/@solidjs/signals")
        );
        assert_eq!(
            package_root("/p/node_modules/solid-js/types/index.d.ts").as_deref(),
            Some("/p/node_modules/solid-js")
        );
        assert_eq!(package_root("/p/src/App.ts"), None);
    }
}
