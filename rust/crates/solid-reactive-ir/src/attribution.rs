//! Which project functions can reach an unresolved proof obligation.
//!
//! A consumer that reports obligations per export has to say *which exports*
//! an open obligation belongs to. An obligation that sits inside an exported
//! function answers that question lexically, and the consumer resolves that
//! case itself from the AST. An obligation inside a private helper does not:
//! the only thing that can say whether a consumer of export `A` — and not of
//! export `B` — can trigger it is the call graph.
//!
//! The call graph lives here, beside the interprocedural analysis that owns
//! it, and a consumer takes only the answer ([`Program::obligation_reach`]).
//! The alternative — handing the raw graph across a boundary and walking it in
//! the consumer — would put two independently drifting notions of "calls" in
//! the codebase, and the consumer's copy would be the one nobody tests
//! against real reactive code.
//!
//! The answer is deliberately shaped as *fail closed or exact*: either
//! [`ObligationReach::complete`] is true and `reaching` enumerates every
//! project function that can enter the obligation's own function, or it is
//! false and the consumer must fall back to marking every export. There is no
//! third "probably these" state, because a partial enumeration read as a
//! complete one silently certifies an export that can reach the obligation.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use solid_facts::{FileFacts, ProjectFacts, core::Span};
use typefacts::Location;

use crate::StaticDefect;
use crate::identity::SymbolId;
use crate::indexes::{EntitySymbols, SemanticLookup};

/// ADR 0324: the `<Show>` a children-callback parameter belongs to.
struct ShowChild {
    when: Span,
    keyed: bool,
}

/// Discharges, in a closed program, the obligation an exported helper raises
/// for invoking a member of its own parameter (ADR 0203).
///
/// That obligation stands for callers outside the analyzed files
/// ([`crate::EXPORTED_PARAMETER_MEMBER_DISPATCH`]). A closed program has none,
/// so the helper's callers are the ones the project shows -- provided each way
/// of entering the helper is a call expression the graph resolves to it. Every
/// such call selects the member's implementation from the argument it passes,
/// or files its own obligation, exactly as a call of an unexported helper does.
/// A helper rendered through JSX, handed out as a value, or with no binding to
/// enumerate keeps the obligation.
pub(crate) fn discharge_closed_program_export_dispatch(
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    entities: &EntitySymbols,
    aliases: &HashMap<SymbolId, SymbolId>,
    symbols_by_root: &HashMap<SymbolId, Vec<SymbolId>>,
    defects: &mut Vec<StaticDefect>,
) {
    let exported_dispatch = |defect: &StaticDefect| {
        defect.analysis_context == crate::EXPORTED_PARAMETER_MEMBER_DISPATCH
    };
    let passed_parameter =
        |defect: &StaticDefect| defect.analysis_context == "parameter-member-target-unresolved";
    let unmodeled = |defect: &StaticDefect| defect.analysis_context == "parameter-use-unmodeled";
    let diverging =
        |defect: &StaticDefect| defect.analysis_context == "parameter-member-targets-diverge";
    if !lookup.program_closed
        || !defects.iter().any(|defect| {
            exported_dispatch(defect)
                || passed_parameter(defect)
                || unmodeled(defect)
                || diverging(defect)
        })
    {
        return;
    }
    let graph = CallGraph {
        files_by_path: facts
            .files
            .iter()
            .map(|file| (file.path.as_str(), file))
            .collect(),
        lookup,
        entities,
        aliases,
        symbols_by_root,
        namespace_escapes: RefCell::new(HashMap::new()),
        attested_project_texts: RefCell::new(None),
    };
    defects.retain(|defect| {
        let Some(file) = graph.file(defect.location.path.as_ref()) else {
            return true;
        };
        let span = Span::new(
            u32::try_from(defect.location.start_byte).unwrap_or(u32::MAX),
            u32::try_from(defect.location.end_byte).unwrap_or(u32::MAX),
        );
        if exported_dispatch(defect) {
            return !graph.entered_only_through_call_expressions(file.path.as_str(), span);
        }
        // ADR 0320: an argument that holds a primitive on every run carries
        // no project member (`unmodeled`); a fresh literal whose handed-on
        // member does carries none at that member.
        if unmodeled(defect) {
            return !graph.value_is_primitive(file, span, PASS_THROUGH_DEPTH, None);
        }
        if passed_parameter(defect)
            && let crate::StaticDefectKind::ReactiveDispatchUnresolved {
                member: Some(member),
                ..
            } = &defect.kind
        {
            return !graph.value_holds_builtin(file, span, member, PASS_THROUGH_DEPTH)
                && !graph.literal_member_is_primitive(file, span, member);
        }
        if diverging(defect)
            && let crate::StaticDefectKind::ReactiveDispatchUnresolved {
                member: Some(member),
                ..
            } = &defect.kind
        {
            return !graph.literal_member_is_primitive(file, span, member);
        }
        true
    });
}

/// How many functions a parameter's value is followed back through (ADR 0213).
const PASS_THROUGH_DEPTH: usize = 4;

struct CallGraph<'a, 'b> {
    files_by_path: HashMap<&'a str, &'a FileFacts>,
    lookup: &'a SemanticLookup<'b>,
    entities: &'a EntitySymbols,
    aliases: &'a HashMap<SymbolId, SymbolId>,
    symbols_by_root: &'a HashMap<SymbolId, Vec<SymbolId>>,
    /// One namespace-escape verdict per module; the scan reads every file.
    namespace_escapes: RefCell<HashMap<String, bool>>,
    /// Specifier texts the compiler resolves to a project file somewhere,
    /// built on first use.
    attested_project_texts: RefCell<Option<HashMap<String, Vec<String>>>>,
}

impl<'a> CallGraph<'a, '_> {
    /// The symbols whose references are this function's: its binding's own,
    /// and every alias of the same root.
    ///
    /// `None` when references cannot bound the function's entries at all:
    ///
    /// - a class method is entered by member dispatch, which resolves by the
    ///   instance's class at run time, not by the declaration a reference
    ///   names: a base constructor's `this.init()` names the base's `init` and
    ///   runs a subclass override that no reference names at all (ADR 0134,
    ///   amendment of 2026-09-28). No reference-based reasoning applies to it,
    ///   the render sites of ADR 0138 included;
    /// - a function with no binding name, or a name with no symbol.
    fn function_symbols(&self, path: &str, function: Span) -> Option<Vec<SymbolId>> {
        let file = self.file(path)?;
        let fact = file
            .ast
            .functions
            .iter()
            .find(|candidate| candidate.span == function)?;
        if fact.name.is_none()
            && fact.method_name.is_some()
            && file
                .ast
                .classes
                .iter()
                .any(|class| class.span.start <= fact.span.start && fact.span.end <= class.span.end)
        {
            return None;
        }
        let declaration = crate::owners::function_binding_name(file, fact)?;
        let symbol = self.entities.at(path, declaration.span)?;
        let root = self.aliases.get(symbol).unwrap_or(symbol);
        let mut aliased = self
            .symbols_by_root
            .get(root)
            .cloned()
            .unwrap_or_else(|| vec![symbol.clone()]);
        if !aliased.iter().any(|candidate| candidate == symbol) {
            aliased.push(symbol.clone());
        }
        // A symbol Type Facts does not know has no reference census, so it
        // bounds nothing; a few graph edges cannot stand in for one. A known
        // symbol with no references has no entries.
        if aliased.iter().any(|candidate| {
            self.lookup
                .symbol_references_if_present(candidate.as_str())
                .is_none()
        }) {
            return None;
        }
        Some(aliased)
    }

    /// Whether every way of entering this function is a call expression, each
    /// of which the graph resolves to it: no JSX render, no render through a
    /// dialect renderer or a rendering prop, and no value escape.
    ///
    /// Narrower than [`Self::entered_only_through_calls`], for a consumer that
    /// relies on what a call expression's own site analysis decides (ADR 0203).
    fn entered_only_through_call_expressions(&self, path: &str, function: Span) -> bool {
        let Some(aliased) = self.function_symbols(path, function) else {
            return false;
        };
        let (calls, namespace_calls) = self.call_expression_references(path, function);
        // A type query or a namespace call's property also accounts for a
        // reference (ADR 0218), but only while the module is not reachable as
        // a namespace object that enters exports without naming them.
        let mut widened = false;
        for candidate in &aliased {
            for reference in self.lookup.symbol_references(candidate.as_str()) {
                if self.reference_is_accounted_for(&reference, &calls, false) {
                    continue;
                }
                if self.reference_in_type_query(&reference)
                    || namespace_calls.contains(&location_key(&reference))
                {
                    widened = true;
                    continue;
                }
                return false;
            }
        }
        !widened || !self.module_namespace_escapes(path)
    }

    /// Whether `reference` lies inside a TypeScript type query, which is
    /// erased: it names the function without entering it.
    fn reference_in_type_query(&self, reference: &Location) -> bool {
        let Some(file) = self.file(reference.path.as_ref()) else {
            return false;
        };
        let (start, end) = location_key(reference).1;
        file.ast
            .type_queries
            .iter()
            .any(|query| query.contains(Span::new(start, end)))
    }

    /// Whether the module at `path` is reachable as a namespace object that
    /// code may use other than by naming a static member. `Object.values(ns)`,
    /// `ns["helper"]`, an alias, `export * as` or a dynamic import can each
    /// enter an export with no reference naming it. A namespace whose module
    /// Type Facts does not resolve may be this one. A non-relative dynamic
    /// specifier may be an alias, so it matches when its last segment names
    /// this module.
    fn module_namespace_escapes(&self, path: &str) -> bool {
        if let Some(escapes) = self.namespace_escapes.borrow().get(path) {
            return *escapes;
        }
        let escapes = self.scan_namespace_escapes(path);
        self.namespace_escapes
            .borrow_mut()
            .insert(path.to_string(), escapes);
        escapes
    }

    fn scan_namespace_escapes(&self, path: &str) -> bool {
        // The module the binding's specifier loads, as the compiler resolved
        // it; without a specifier, the module Type Facts resolves the binding
        // to. A barrel may expose this module's functions under its own
        // namespace. A target outside the project's files may be anything.
        let names_this = |file: &solid_facts::FileFacts,
                          span: Span,
                          specifier: Option<(Span, &str)>| match specifier
        {
            Some((declaration, specifier)) => {
                self.specifier_reaches(file, Some(declaration), specifier, path, &mut Vec::new())
            }
            None => self
                .lookup
                .namespace_module_paths(file.path.as_str(), span)
                .is_none_or(|paths| {
                    paths
                        .iter()
                        .any(|target| self.module_exposes(target, path, &mut Vec::new()))
                }),
        };
        self.lookup.files().iter().any(|file| {
            let namespace_escapes = file
                .ast
                .imports
                .iter()
                .filter(|import| !import.type_only)
                .flat_map(|import| import.bindings.iter().map(move |binding| (import, binding)))
                .filter(|(_, binding)| {
                    !binding.type_only && binding.kind == solid_facts::ast::ImportKind::Namespace
                })
                .any(|(import, binding)| {
                    names_this(
                        file,
                        binding.local.span,
                        Some((import.span, import.module.as_str())),
                    ) && !namespace_used_by_static_members(file, binding.local.span)
                });
            let reexported = file.ast.exports.iter().any(|export| {
                !export.type_only
                    && export.namespace_binding.as_ref().is_some_and(|binding| {
                        names_this(
                            file,
                            binding.span,
                            export.module.as_deref().map(|module| (export.span, module)),
                        )
                    })
            });
            let required = file.ast.import_equals.iter().any(|import| {
                !import.type_only
                    && names_this(
                        file,
                        import.local.span,
                        Some((import.span, import.module.as_str())),
                    )
                    && (import.exported
                        || !namespace_used_by_static_members(file, import.local.span))
            });
            let loaded = file.ast.module_loads.iter().any(|load| {
                load.specifier.as_deref().is_none_or(|specifier| {
                    self.specifier_reaches(
                        file,
                        load.specifier_span,
                        specifier,
                        path,
                        &mut Vec::new(),
                    )
                })
            });
            namespace_escapes || reexported || required || loaded
        })
    }

    /// Whether the specifier at `at` in `from` loads the module at `path`, or
    /// a module that exposes it. `at` is the declaration holding the
    /// specifier, or a load's own argument literal.
    ///
    /// The compiler's attested resolution decides when it has a row for this
    /// occurrence:
    /// - an unresolved specifier may name anything;
    /// - a project file is followed;
    /// - a relative specifier with an explicit runtime extension
    ///   (`../dist/index.js`) loads exactly that file, or its TypeScript
    ///   source (`index.ts`): those that are project files are followed, and
    ///   any other is a runtime file outside the program (ADR 0193);
    /// - a file installed under `node_modules` exposes no project module,
    ///   unless the package's directory holds a program file;
    /// - any other target, such as a declaration whose runtime module a
    ///   package `main` or a link selects, may expose anything.
    ///
    /// Without a row, a relative specifier is resolved against the project's
    /// files, failing closed when that does not resolve. A Node built-in name
    /// loads the runtime's module, plus any project file the compiler maps
    /// that name to elsewhere. Any other bare specifier may name anything.
    fn specifier_reaches(
        &self,
        from: &solid_facts::FileFacts,
        at: Option<Span>,
        specifier: &str,
        path: &str,
        visiting: &mut Vec<String>,
    ) -> bool {
        // The runtime's own answer, when the host obtained it (ADR 0220),
        // replaces every inference below. A file outside the program, an
        // external or a built-in module imports no project module (ADR 0193).
        if let Some(runtime) = self.lookup.runtime_resolutions() {
            let Some(at) = at else {
                return true;
            };
            return match runtime.outcome(from.path.as_str(), at, specifier) {
                solid_facts::runtime_resolution::RuntimeOutcome::File {
                    path: runtime_path,
                    physical_path,
                } => [runtime_path.as_ref(), physical_path.as_ref()]
                    .into_iter()
                    .find(|target| self.file(target).is_some())
                    .is_some_and(|target| self.module_exposes(target, path, visiting)),
                solid_facts::runtime_resolution::RuntimeOutcome::External
                | solid_facts::runtime_resolution::RuntimeOutcome::Builtin => false,
                solid_facts::runtime_resolution::RuntimeOutcome::Unknown => true,
            };
        }
        let attested = at.and_then(|at| {
            match self
                .lookup
                .resolved_imports()?
                .specifier(from.path.as_str(), at, specifier)
            {
                solid_facts::SpecifierAttestation::Attested(row) => Some(row),
                solid_facts::SpecifierAttestation::Unattested => None,
            }
        });
        if let Some(row) = attested {
            if row.resolution == solid_facts::ImportResolution::Unresolved {
                return true;
            }
            let resolved = if row.included_path.is_empty() {
                row.resolved_path.as_ref()
            } else {
                row.included_path.as_ref()
            };
            if self.file(resolved).is_some() {
                return self.module_exposes(resolved, path, visiting);
            }
            if let Some(candidates) = explicit_runtime_files(from.path.as_str(), specifier) {
                return candidates
                    .iter()
                    .filter(|candidate| self.file(candidate).is_some())
                    .any(|candidate| self.module_exposes(candidate, path, visiting));
            }
            if row.resolution != solid_facts::ImportResolution::NodeModules
                || !resolved.contains("/node_modules/")
            {
                return true;
            }
            // An installed package exposes no project module, unless its
            // directory holds a program file: an analyzed workspace source
            // installed there. An unknown package directory may hold one.
            return row
                .package_manifest
                .as_deref()
                .and_then(|manifest| manifest.rsplit_once('/'))
                .is_none_or(|(directory, _)| {
                    let directory = format!("{directory}/");
                    self.lookup
                        .files()
                        .iter()
                        .any(|file| file.path.as_str().starts_with(&directory))
                });
        }
        if !specifier.starts_with('.') {
            if !crate::runtime_semantics::is_node_builtin_module(specifier) {
                return true;
            }
            // A mapping the compiler resolves this name through elsewhere adds
            // the project file it names; it never removes a candidate.
            return self
                .attested_project_targets(specifier)
                .iter()
                .any(|target| self.module_exposes(target, path, visiting));
        }
        let files = self.lookup.files();
        solid_facts::resolve_relative_module_path(
            from.path.as_str(),
            specifier,
            files.iter().map(|file| file.path.as_str()),
        )
        .is_none_or(|resolved| self.module_exposes(resolved, path, visiting))
    }

    /// The project files the compiler resolves `specifier` to, in any
    /// importing file: a `paths` mapping or alias that a Node built-in name
    /// must not shadow.
    fn attested_project_targets(&self, specifier: &str) -> Vec<String> {
        let mut cache = self.attested_project_texts.borrow_mut();
        let targets = cache.get_or_insert_with(|| {
            let mut targets: HashMap<String, Vec<String>> = HashMap::new();
            for (_, row) in self
                .lookup
                .resolved_imports()
                .into_iter()
                .flat_map(|index| index.iter())
            {
                let resolved = if row.included_path.is_empty() {
                    row.resolved_path.as_ref()
                } else {
                    row.included_path.as_ref()
                };
                if row.resolution != solid_facts::ImportResolution::Unresolved
                    && self.file(resolved).is_some()
                {
                    let entry = targets.entry(row.text.to_string()).or_default();
                    if !entry.iter().any(|target| target == resolved) {
                        entry.push(resolved.to_string());
                    }
                }
            }
            targets
        });
        targets.get(specifier).cloned().unwrap_or_default()
    }

    /// Whether the module at `target` exposes the module at `path`: it is
    /// that module, or it re-exports from one that does, by `export … from`
    /// or by exporting a binding it imported. A module the project does not
    /// analyze may.
    fn module_exposes(&self, target: &str, path: &str, visiting: &mut Vec<String>) -> bool {
        if target == path {
            return true;
        }
        // A declaration's exports are not its runtime module's.
        if [".d.ts", ".d.mts", ".d.cts"]
            .iter()
            .any(|extension| target.ends_with(extension))
        {
            return true;
        }
        if visiting.iter().any(|seen| seen == target) {
            return false;
        }
        visiting.push(target.to_string());
        let Some(file) = self.file(target) else {
            return true;
        };
        let imported_from = |local: Span| -> Option<(Span, &str)> {
            file.ast
                .imports
                .iter()
                .find(|import| {
                    import
                        .bindings
                        .iter()
                        .any(|binding| binding.local.span == local)
                })
                .map(|import| (import.span, import.module.as_str()))
                .or_else(|| {
                    file.ast
                        .import_equals
                        .iter()
                        .find(|import| import.local.span == local)
                        .map(|import| (import.span, import.module.as_str()))
                })
        };
        for export in &file.ast.exports {
            if let Some(module) = export.module.as_deref() {
                if self.specifier_reaches(file, Some(export.span), module, path, visiting) {
                    return true;
                }
                continue;
            }
            // `export { helper }` lists a specifier; `export default helper`
            // records its expression among the declarations.
            for specifier in export.specifiers.iter().chain(&export.declarations) {
                if let Some((declaration, module)) = file
                    .ast
                    .reference_declaration(specifier.local.span)
                    .and_then(imported_from)
                    && self.specifier_reaches(file, Some(declaration), module, path, visiting)
                {
                    return true;
                }
            }
        }
        false
    }

    /// Exact reference spans of resolved ordinary call edges. A namespace
    /// call's graph edge names `ns.helper`, whereas the helper's own symbol
    /// reference can name only `helper`. Admit that property only when its
    /// exact semantic entity resolves to this same declaration.
    fn call_expression_references(
        &self,
        path: &str,
        function: Span,
    ) -> (HashSet<CallSiteKey>, HashSet<ReferenceKey>) {
        let mut known = HashSet::new();
        let mut properties = HashSet::new();
        for (caller, callee) in self.lookup.function_call_sites(path, function) {
            if self.lookup.call_by_callee(caller, callee).is_none() {
                continue;
            }
            known.insert((caller.path.to_string(), callee.start, callee.end));
            let peeled = caller.ast.peel_ts_sugar_span(callee);
            if caller.ast.computed_members.binary_search(&peeled).is_ok() {
                continue;
            }
            let Some(member) = caller
                .ast
                .members
                .iter()
                .find(|member| member.span == peeled)
            else {
                continue;
            };
            let same_target = self
                .lookup
                .entity_symbol(caller, member.property)
                .and_then(|symbol| self.lookup.function_for_symbol(symbol))
                .is_some_and(|(target_file, target)| {
                    target_file.path.as_str() == path && target.span == function
                });
            if same_target {
                properties.insert((
                    caller.path.to_string(),
                    (member.property.start, member.property.end),
                ));
            }
        }
        (known, properties)
    }

    /// Whether the value at `span` is a built-in value whose `member` is its
    /// prototype's: by its origin (ADR 0211, 0212), or as a parameter
    /// (ADR 0213) or a component prop (ADR 0214) that holds one on every
    /// entry.
    fn value_holds_builtin(
        &self,
        file: &'a FileFacts,
        span: Span,
        member: &str,
        depth: usize,
    ) -> bool {
        depth > 0
            // The member veto holds for every origin, the direct one included;
            // every origin here is a built-in value (ADR 0314).
            && !self.lookup.member_may_be_reassigned_on_builtin(member)
            && !self.lookup.member_may_be_reassigned_on_builtin("__proto__")
            && (matches!(
                self.lookup.value_origin(
                    file,
                    span,
                    solid_facts::ast::RuntimeValueKind::Unknown,
                    6
                ),
                Some(crate::indexes::ValueOrigin::Builtin)
            ) || self.parameter_holds_builtin(file, span, member, depth)
                || self.prop_holds_builtin(file, span, member, depth))
    }

    /// ADR 0320: whether the value at `span` is a primitive other than
    /// `undefined` on every run, proven from what produces it, never from a
    /// declared type:
    ///
    /// - its own expression (`SemanticLookup::proven_primitive`);
    /// - a conditional or logical expression all of whose results are;
    /// - an unwritten `const` of the same file initialized with one;
    /// - a parameter every entry of whose function passes one (ADR 0213's
    ///   census: entered only through call expressions, no spread before it;
    ///   a missing argument is `undefined` unless a proven default covers it),
    ///   or the first parameter of an effect's apply callback whose compute
    ///   returns one on every path;
    /// - `props.name` that every tag rendering the component passes, as a
    ///   string, a boolean, or an expression that holds one (ADR 0214's
    ///   census; a tag without the attribute passes `undefined`);
    /// - a signal accessor's value: a proven initial value, and every setter
    ///   reference a call in its file writing one (an updater's returns are
    ///   proven with its previous value assumed); a bare memo's compute
    ///   returning one on every path;
    /// - an exact project function's result, every return of which is one and
    ///   one of which runs on every path.
    ///
    /// An assertion anywhere on the way refuses; `assumed` is the one
    /// parameter (an updater's previous value) taken as proven.
    fn value_is_primitive(
        &self,
        file: &'a FileFacts,
        span: Span,
        depth: usize,
        assumed: Option<(&str, Span)>,
    ) -> bool {
        if depth == 0 {
            return false;
        }
        let Some(value) = SemanticLookup::peel_parentheses(file, span) else {
            return false;
        };
        if self.lookup.proven_primitive(file, value, 6) {
            return true;
        }
        if let Some(conditional) = file
            .ast
            .conditional_expressions
            .iter()
            .find(|conditional| conditional.span == value)
        {
            return self.value_is_primitive(file, conditional.consequent, depth - 1, assumed)
                && self.value_is_primitive(file, conditional.alternate, depth - 1, assumed);
        }
        if let Some(logical) = file
            .ast
            .logical_expressions
            .iter()
            .find(|logical| logical.span == value)
        {
            return self.value_is_primitive(file, logical.left, depth - 1, assumed)
                && self.value_is_primitive(file, logical.right, depth - 1, assumed);
        }
        if let Some(call) = file.ast.call_at(value) {
            return !call.construct
                && !call.arguments.iter().any(|argument| argument.spread)
                && (self.accessor_holds_primitive(file, call, depth)
                    || self.call_returns_primitive(file, call, depth)
                    || self.show_accessor_holds_primitive(file, call, depth));
        }
        if file.ast.members.iter().any(|member| member.span == value) {
            return self.prop_holds_primitive(file, value, depth);
        }
        let Some(declaration) = file.ast.reference_declaration(value) else {
            return false;
        };
        if let Some((path, parameter)) = assumed
            && path == file.path.as_str()
            && parameter == declaration
        {
            return true;
        }
        // ADR 0324: the value a keyed `<Show>` hands its children callback.
        if let Some(show) = self.show_child_parameter(file, declaration)
            && show.keyed
            && self.truthy_value_is_primitive(file, show.when, depth)
        {
            return true;
        }
        if let Some(binding) = file.ast.bindings.iter().find(|binding| {
            binding.shape == solid_facts::ast::BindingShape::Identifier
                && binding
                    .names
                    .first()
                    .is_some_and(|name| name.span == declaration)
        }) && !file.ast.functions.iter().any(|function| {
            function
                .parameters
                .iter()
                .any(|parameter| parameter.names.iter().any(|name| name.span == declaration))
        }) {
            return binding.immutable
                && !crate::indexes::binding_written(file, declaration)
                && binding.initializer.is_some_and(|initializer| {
                    self.value_is_primitive(file, initializer, depth - 1, assumed)
                });
        }
        self.parameter_holds_primitive(file, value, declaration, depth)
    }

    /// ADR 0324: the `<Show>` whose children callback declares the parameter
    /// at `declaration`, with its `when` expression and whether it is keyed.
    /// Only the exact shape the audited `Show` implementation hands a value:
    /// the dialect's `Show` (never a lookalike), no spread attribute, one
    /// `when`, `keyed` absent or a boolean literal, and the sole child a
    /// function literal of exactly one plain, never-reassigned parameter
    /// (`solid-js@2.0.0-rc.13` `dist/solid.dev.js:1462-1486`: the keyed form
    /// calls the child with the condition's value, the other with an accessor
    /// that returns that value or throws).
    fn show_child_parameter(&self, file: &'a FileFacts, declaration: Span) -> Option<ShowChild> {
        use solid_facts::ast::BindingShape;
        let primitives = self.lookup.primitives(file);
        for (index, element) in file.ast.jsx_elements.iter().enumerate() {
            if crate::known_primitive(primitives.jsx.get(index)?)
                != Some(solid_dialect::Primitive::Show)
                || !element.spreads.is_empty()
            {
                continue;
            }
            let keyed = element
                .boolean_properties
                .iter()
                .find(|property| file.source_text(property.name) == Some("keyed"))
                .map(|property| property.value);
            if keyed.is_none()
                && element
                    .properties
                    .iter()
                    .any(|property| file.source_text(*property) == Some("keyed"))
            {
                continue;
            }
            let mut whens = element.attributes.iter().filter(|attribute| {
                attribute.namespace.is_none()
                    && file.source_text(attribute.local_name) == Some("when")
            });
            let (Some(when), None) = (whens.next(), whens.next()) else {
                continue;
            };
            let Some(when) = when.expression else {
                continue;
            };
            let mut callback = None;
            let mut sole = true;
            for child in &element.children {
                let Some(text) = file.source_text(*child) else {
                    sole = false;
                    continue;
                };
                if text.trim().is_empty() {
                    continue;
                }
                let Some(inner) = text
                    .strip_prefix('{')
                    .and_then(|rest| rest.strip_suffix('}'))
                else {
                    sole = false;
                    continue;
                };
                let (Ok(lead), Ok(trail)) = (
                    u32::try_from(inner.len() - inner.trim_start().len()),
                    u32::try_from(inner.len() - inner.trim_end().len()),
                ) else {
                    sole = false;
                    continue;
                };
                if callback.is_some() {
                    sole = false;
                }
                callback =
                    Some(file.ast.peel_ts_sugar_span(Span::new(
                        child.start + 1 + lead,
                        child.end - 1 - trail,
                    )));
            }
            let Some(callback) = callback.filter(|_| sole) else {
                continue;
            };
            if file.ast.functions.iter().any(|function| {
                function.span == callback
                    && !function.r#async
                    && !function.generator
                    && !function.rest_parameter
                    && matches!(function.parameters.as_slice(), [parameter]
                        if parameter.shape == BindingShape::Identifier
                            && parameter.initializer.is_none()
                            && parameter.names.first().is_some_and(|name| name.span == declaration))
            }) && !crate::indexes::binding_written(file, declaration)
            {
                return Some(ShowChild {
                    when: file.ast.peel_ts_sugar_span(when),
                    keyed: keyed == Some(true),
                });
            }
        }
        None
    }

    /// ADR 0324: `id()` where `id` is the accessor a non-keyed `<Show>` hands
    /// its children callback.
    fn show_accessor_holds_primitive(
        &self,
        file: &'a FileFacts,
        call: &solid_facts::ast::CallFact,
        depth: usize,
    ) -> bool {
        if !call.arguments.is_empty() || call.construct {
            return false;
        }
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        if file
            .ast
            .identifiers
            .binary_search_by_key(&callee, |identifier| identifier.span)
            .is_err()
        {
            return false;
        }
        let Some(declaration) = file.ast.reference_declaration(callee) else {
            return false;
        };
        self.show_child_parameter(file, declaration)
            .is_some_and(|show| {
                !show.keyed && self.truthy_value_is_primitive(file, show.when, depth)
            })
    }

    /// ADR 0324: whether every truthy value of `when` is a primitive: a route
    /// parameter (a string or `undefined`), or an expression that is a
    /// primitive other than `undefined` on every run.
    fn truthy_value_is_primitive(&self, file: &'a FileFacts, when: Span, depth: usize) -> bool {
        depth > 0
            && (self.router_param_member(file, when)
                || self.value_is_primitive(file, when, depth - 1, None))
    }

    /// ADR 0324: `params.key` where `params` is the direct `const` of a
    /// `useParams()` call imported by name from `@solidjs/router`, a release
    /// the dialect audited and a project that replaces nothing the audit read
    /// ([`crate::router_params`]). The value is a URL path-segment string, or
    /// `undefined` for an absent optional parameter. Refused: a computed key,
    /// a name of `Object.prototype` (a route that lacks the parameter falls
    /// through the merged object to it), a name some assignment or reflective
    /// definition of the program writes, and any object that is not that
    /// direct binding (an alias, a wrapper, a re-export, a namespace member).
    fn router_param_member(&self, file: &'a FileFacts, expression: Span) -> bool {
        const OBJECT_PROTOTYPE: [&str; 12] = [
            "constructor",
            "hasOwnProperty",
            "isPrototypeOf",
            "propertyIsEnumerable",
            "toLocaleString",
            "toString",
            "valueOf",
            "__proto__",
            "__defineGetter__",
            "__defineSetter__",
            "__lookupGetter__",
            "__lookupSetter__",
        ];
        if !self.lookup.router_params().applies() {
            return false;
        }
        let Some(expression) = SemanticLookup::peel_parentheses(file, expression) else {
            return false;
        };
        let Some(member) = file
            .ast
            .members
            .iter()
            .find(|member| member.span == expression)
        else {
            return false;
        };
        if file
            .ast
            .computed_members
            .binary_search(&member.span)
            .is_ok()
        {
            return false;
        }
        let Some(key) = file.source_text(member.property) else {
            return false;
        };
        if OBJECT_PROTOTYPE.contains(&key)
            || key.is_empty()
            || self.lookup.member_may_be_reassigned_on_builtin(key)
            || self.lookup.member_may_be_reassigned_on_builtin("__proto__")
            || self.lookup.property_may_be_defined_reflectively_as(key)
        {
            return false;
        }
        let object = file.ast.peel_ts_sugar_span(member.object);
        if SemanticLookup::peel_parentheses(file, member.object).is_none()
            || file
                .ast
                .identifiers
                .binary_search_by_key(&object, |identifier| identifier.span)
                .is_err()
        {
            return false;
        }
        let Some(declaration) = file.ast.reference_declaration(object) else {
            return false;
        };
        let Some(binding) = file.ast.bindings.iter().find(|binding| {
            binding.immutable
                && binding.shape == solid_facts::ast::BindingShape::Identifier
                && binding
                    .names
                    .first()
                    .is_some_and(|name| name.span == declaration)
        }) else {
            return false;
        };
        let Some(call) = binding
            .call_initializer
            .and_then(|span| file.ast.call_at(file.ast.peel_ts_sugar_span(span)))
        else {
            return false;
        };
        if call.construct
            || binding
                .initializer
                .is_none_or(|initializer| file.ast.peel_ts_sugar_span(initializer) != call.span)
        {
            return false;
        }
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        let Some(declared) = file.ast.reference_declaration(callee) else {
            return false;
        };
        // The callee is the local name of `import { useParams } from
        // "@solidjs/router"`: a named import, not a type-only one.
        file.ast.imports.iter().any(|import| {
            import.module.as_str() == crate::router_params::ROUTER_PACKAGE
                && !import.type_only
                && import.bindings.iter().any(|binding| {
                    binding.kind != solid_facts::ast::ImportKind::Namespace
                        && binding.imported.as_deref() == Some("useParams")
                        && binding.local.span == declared
                })
        })
    }

    /// See [`Self::value_is_primitive`]: a parameter, by every entry of its
    /// function, or an effect apply callback's value.
    fn parameter_holds_primitive(
        &self,
        file: &'a FileFacts,
        reference: Span,
        declaration: Span,
        depth: usize,
    ) -> bool {
        let Some((function, index)) = file
            .ast
            .functions_body_containing(reference)
            .filter_map(|function| {
                function
                    .parameters
                    .iter()
                    .position(|parameter| {
                        parameter.shape == solid_facts::ast::BindingShape::Identifier
                            && parameter
                                .names
                                .first()
                                .is_some_and(|name| name.span == declaration)
                    })
                    .map(|index| (function, index))
            })
            .next()
        else {
            return false;
        };
        if (function.rest_parameter && index + 1 == function.parameters.len())
            || crate::indexes::binding_written(file, declaration)
            || function.r#async
            || function.generator
            || file.ast.identifiers.iter().any(|id| {
                function.body.contains(id.span) && file.source_text(id.span) == Some("arguments")
            })
        {
            return false;
        }
        let default = function.parameters[index].initializer;
        if default.is_some_and(|default| !self.value_is_primitive(file, default, depth - 1, None)) {
            return false;
        }
        // The value an effect's apply callback receives first is what its
        // compute returned.
        if index == 0
            && let Some(effect) = file.ast.calls.iter().find(|call| {
                call.arguments.get(1).is_some_and(|argument| {
                    file.ast.peel_ts_sugar_span(argument.span) == function.span
                })
            })
            && matches!(
                self.lookup.primitive_at_call(file, effect.span),
                Some(
                    solid_dialect::Primitive::CreateEffect
                        | solid_dialect::Primitive::CreateRenderEffect
                )
            )
        {
            return default.is_none()
                && effect.arguments.len() == 2
                && crate::cleanup::callback_argument_literal(file, effect.arguments[0].span)
                    .is_some_and(|compute| self.returns_primitive(file, compute, depth, None));
        }
        if !self.entered_only_through_call_expressions(file.path.as_str(), function.span) {
            return false;
        }
        let sites = self
            .lookup
            .function_call_sites(file.path.as_str(), function.span);
        !sites.is_empty()
            && sites.into_iter().all(|(caller, callee)| {
                let Some(call) = self.lookup.call_by_callee(caller, callee) else {
                    return false;
                };
                if call.construct
                    || call
                        .arguments
                        .iter()
                        .take(index + 1)
                        .any(|argument| argument.spread)
                {
                    return false;
                }
                match call.arguments.get(index) {
                    Some(passed) => {
                        // `undefined` written selects the default.
                        caller.source_text(caller.ast.peel_ts_sugar_span(passed.span))
                            != Some("undefined")
                            && self.value_is_primitive(caller, passed.span, depth - 1, None)
                    }
                    None => default.is_some(),
                }
            })
    }

    /// See [`Self::value_is_primitive`]: `props.name` by every tag.
    fn prop_holds_primitive(&self, file: &'a FileFacts, argument: Span, depth: usize) -> bool {
        use solid_facts::ast::JsxAttributeValueKind;
        if file.ast.computed_members.binary_search(&argument).is_ok() {
            return false;
        }
        let Some(access) = file
            .ast
            .members
            .iter()
            .find(|access| access.span == argument)
        else {
            return false;
        };
        let Some(name) = file.source_text(access.property) else {
            return false;
        };
        if name == "children" {
            return false;
        }
        let Some(declaration) = file
            .ast
            .reference_declaration(file.ast.peel_ts_sugar_span(access.object))
        else {
            return false;
        };
        let Some(component) = file.ast.functions.iter().find(|function| {
            function.body.contains(argument)
                && matches!(function.parameters.as_slice(), [parameter]
                    if parameter.shape == solid_facts::ast::BindingShape::Identifier
                        && parameter.initializer.is_none()
                        && parameter.names.first().is_some_and(|name| name.span == declaration))
        }) else {
            return false;
        };
        let targets = file
            .ast
            .assignments
            .iter()
            .map(|assignment| assignment.target)
            .chain(file.ast.iteration_targets.iter().copied())
            .chain(file.ast.deleted_targets.iter().copied())
            .collect::<Vec<_>>();
        let only_read = file
            .ast
            .reference_declarations
            .iter()
            .filter(|(_, declared)| *declared == declaration)
            .all(|(reference, _)| {
                file.ast.members.iter().any(|read| {
                    read.object == *reference
                        && !targets.iter().any(|target| target.contains(read.span))
                        && !file
                            .ast
                            .calls
                            .iter()
                            .any(|call| file.ast.peel_ts_sugar_span(call.callee) == read.span)
                })
            });
        if !only_read {
            return false;
        }
        let Some(renders) = self.rendered_only_through_jsx(file.path.as_str(), component.span)
        else {
            return false;
        };
        !renders.is_empty()
            && renders.into_iter().all(|(caller, element)| {
                let mut passed = element.attributes.iter().filter(|attribute| {
                    attribute.namespace.is_none()
                        && caller.source_text(attribute.local_name) == Some(name)
                });
                let Some(attribute) = passed.next() else {
                    return false;
                };
                element.spreads.is_empty()
                    && passed.next().is_none()
                    && match attribute.value_kind {
                        JsxAttributeValueKind::Boolean | JsxAttributeValueKind::String => true,
                        JsxAttributeValueKind::Expression => {
                            attribute.expression.is_some_and(|expression| {
                                self.value_is_primitive(caller, expression, depth - 1, None)
                            })
                        }
                        JsxAttributeValueKind::Element | JsxAttributeValueKind::Fragment => false,
                    }
            })
    }

    /// See [`Self::value_is_primitive`]: a signal or bare memo accessor's
    /// value, read through a call of the accessor in its own file.
    fn accessor_holds_primitive(
        &self,
        file: &'a FileFacts,
        call: &solid_facts::ast::CallFact,
        depth: usize,
    ) -> bool {
        use solid_dialect::Primitive;
        if !call.arguments.is_empty() {
            return false;
        }
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        let Some((binding_file, binding, _)) =
            self.lookup.binding_at_reference(file.path.as_str(), callee)
        else {
            return false;
        };
        let Some(initializer_span) = binding.call_initializer else {
            return false;
        };
        let initializer_span = binding_file.ast.peel_ts_sugar_span(initializer_span);
        let Some(initializer) = binding_file
            .ast
            .calls
            .iter()
            .find(|candidate| candidate.span == initializer_span)
        else {
            return false;
        };
        if binding_file.path != file.path || !binding.immutable {
            return false;
        }
        match (
            binding.shape,
            self.lookup
                .primitive_at_call(binding_file, initializer.span),
        ) {
            (solid_facts::ast::BindingShape::Array, Some(Primitive::CreateSignal)) => {
                let Some(Some(accessor)) = binding.array_slots.first() else {
                    return false;
                };
                // A tuple slot rest or extra slot could take another value.
                if file.ast.reference_declaration(callee) != Some(accessor.span)
                    || binding.names.len() > 2
                    || initializer.arguments.len() > 2
                {
                    return false;
                }
                let Some(initial) = initializer.arguments.first() else {
                    // No initial value: `undefined`.
                    return false;
                };
                if initial.spread
                    || matches!(
                        initial.value,
                        solid_facts::ast::ArgumentValueKind::Function
                            | solid_facts::ast::ArgumentValueKind::AsyncFunction
                    )
                    || !self.value_is_primitive(file, initial.span, depth - 1, None)
                {
                    return false;
                }
                match binding.array_slots.get(1) {
                    Some(Some(setter)) => self.setter_writes_primitive(file, setter.span, depth),
                    Some(None) => false,
                    None => binding.names.len() == 1,
                }
            }
            (solid_facts::ast::BindingShape::Identifier, Some(Primitive::CreateMemo)) => {
                let [compute] = initializer.arguments.as_slice() else {
                    return false;
                };
                crate::cleanup::callback_argument_literal(file, compute.span)
                    .is_some_and(|function| self.returns_primitive(file, function, depth, None))
            }
            _ => false,
        }
    }

    /// Every reference to the setter declared at `setter` is the callee of a
    /// call in its file writing a proven primitive, or an updater whose
    /// returns are proven with its previous value assumed. A setter handed
    /// anywhere else may write anything.
    fn setter_writes_primitive(&self, file: &'a FileFacts, setter: Span, depth: usize) -> bool {
        let references = file
            .ast
            .reference_declarations
            .iter()
            .filter(|(reference, declaration)| *declaration == setter && *reference != setter)
            .collect::<Vec<_>>();
        if self
            .lookup
            .symbol_references(
                self.entities
                    .at(file.path.as_str(), setter)
                    .map_or("", |symbol| symbol.as_str()),
            )
            .iter()
            .any(|reference| reference.path.as_ref() != file.path.as_str())
        {
            return false;
        }
        references.into_iter().all(|(reference, _)| {
            let Some(call) = self
                .lookup
                .call_by_callee(file, *reference)
                .filter(|call| file.ast.peel_ts_sugar_span(call.callee) == *reference)
            else {
                return false;
            };
            let [written] = call.arguments.as_slice() else {
                return false;
            };
            if written.spread {
                return false;
            }
            if matches!(
                written.value,
                solid_facts::ast::ArgumentValueKind::Function
                    | solid_facts::ast::ArgumentValueKind::AsyncFunction
            ) {
                let Some(updater) = crate::cleanup::callback_argument_literal(file, written.span)
                else {
                    return false;
                };
                let previous = updater
                    .parameters
                    .first()
                    .filter(|parameter| {
                        parameter.shape == solid_facts::ast::BindingShape::Identifier
                            && parameter.initializer.is_none()
                    })
                    .and_then(|parameter| parameter.names.first())
                    .map(|name| name.span)
                    .filter(|name| !crate::indexes::binding_written(file, *name));
                return self.returns_primitive(
                    file,
                    updater,
                    depth,
                    previous.map(|name| (file.path.as_str(), name)),
                );
            }
            self.value_is_primitive(file, written.span, depth - 1, None)
        })
    }

    /// Every value the synchronous `function` returns is proven primitive,
    /// and one return runs on every path (no fall-through to `undefined`).
    fn returns_primitive(
        &self,
        file: &'a FileFacts,
        function: &solid_facts::ast::FunctionFact,
        depth: usize,
        assumed: Option<(&str, Span)>,
    ) -> bool {
        if function.r#async || function.generator || depth == 0 {
            return false;
        }
        let proven = |returned: &solid_facts::ast::ReturnFact| {
            returned
                .argument
                .is_some_and(|argument| self.value_is_primitive(file, argument, depth - 1, assumed))
        };
        if function.expression_body {
            return function.expression_return.as_ref().is_some_and(proven);
        }
        let mut returns = file.ast.own_returns(function).peekable();
        returns.peek().is_some()
            && file
                .ast
                .own_returns(function)
                .any(|returned| returned.straight_line)
            && returns.all(proven)
    }

    /// See [`Self::value_is_primitive`]: an exact project function's result.
    fn call_returns_primitive(
        &self,
        file: &'a FileFacts,
        call: &solid_facts::ast::CallFact,
        depth: usize,
    ) -> bool {
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        if self.lookup.is_member_span(file, callee)
            || file.ast.computed_members.binary_search(&callee).is_ok()
        {
            return false;
        }
        let Some((function_file, function)) = self
            .lookup
            .callee_symbol(file, call.callee)
            .and_then(|symbol| self.lookup.function_for_symbol(symbol))
        else {
            return false;
        };
        self.lookup
            .function_value_is_current(function_file, function)
            && self.returns_primitive(function_file, function, depth, None)
    }

    /// ADR 0320: the argument at `span` is a fresh object literal, passed to
    /// a callee that only reads members of that parameter, whose own `member`
    /// property -- the last one, with no spread, getter or computed key in
    /// the literal -- holds a proven primitive.
    fn literal_member_is_primitive(&self, file: &'a FileFacts, span: Span, member: &str) -> bool {
        if member.contains('.') {
            return false;
        }
        let Some(value) = SemanticLookup::peel_parentheses(file, span) else {
            return false;
        };
        let Some(shape) = file
            .ast
            .object_get_shapes
            .iter()
            .find(|shape| shape.span == value)
        else {
            return false;
        };
        if !shape.closed
            || shape
                .properties
                .iter()
                .any(|property| property.getter.is_some())
        {
            return false;
        }
        let Some((call, index)) = file
            .ast
            .arguments_containing(span)
            .find(|(call, index)| call.arguments[*index].span == span)
        else {
            return false;
        };
        let mut consulted = Vec::new();
        if !self
            .lookup
            .parameter_only_member_accessed_consulting(file, call, index, &mut consulted)
        {
            return false;
        }
        let properties = file
            .ast
            .object_properties
            .iter()
            .filter(|property| value.contains(property.span))
            .filter(|property| {
                !file.ast.object_properties.iter().any(|outer| {
                    outer.span != property.span
                        && value.contains(outer.span)
                        && outer.value.contains(property.span)
                })
            })
            .collect::<Vec<_>>();
        if properties.iter().any(|property| property.computed) {
            return false;
        }
        properties
            .iter()
            .rev()
            .find(|property| {
                crate::execution_role::property_static_key(file, property).as_deref()
                    == Some(member)
            })
            .is_some_and(|property| {
                self.value_is_primitive(file, property.value, PASS_THROUGH_DEPTH, None)
            })
    }

    /// ADR 0214: whether `props.name` at `argument` holds a built-in value on
    /// every render of its component.
    ///
    /// `props` is the only parameter of a function around the argument, a
    /// plain identifier with no default, and nothing in its file writes it or
    /// a member of it. The component is rendered only through JSX tags the
    /// graph resolves to it: a value escape, a call, or a rendering prop
    /// (`component={Panel}`) supplies props no tag shows. Every tag spreads
    /// nothing and passes `name` -- if at all -- as a string, a boolean, or an
    /// expression that holds a built-in value. A missing attribute is
    /// `undefined`. `children` is the element's content, not an attribute,
    /// and is not followed.
    fn prop_holds_builtin(
        &self,
        file: &'a FileFacts,
        argument: Span,
        member: &str,
        depth: usize,
    ) -> bool {
        use solid_facts::ast::JsxAttributeValueKind;
        if depth == 0
            || self.lookup.member_may_be_reassigned_on_builtin(member)
            || self.lookup.member_may_be_reassigned_on_builtin("__proto__")
        {
            return false;
        }
        let argument = file.ast.peel_ts_sugar_span(argument);
        // A computed read (`props["items"]`, `props[key]`) names its key by
        // value, not by spelling: not followed.
        if file.ast.computed_members.binary_search(&argument).is_ok() {
            return false;
        }
        let Some(access) = file
            .ast
            .members
            .iter()
            .find(|access| access.span == argument)
        else {
            return false;
        };
        let Some(name) = file.source_text(access.property) else {
            return false;
        };
        if name == "children" {
            return false;
        }
        let Some(declaration) = file
            .ast
            .reference_declaration(file.ast.peel_ts_sugar_span(access.object))
        else {
            return false;
        };
        let Some(component) = file.ast.functions.iter().find(|function| {
            function.body.contains(argument)
                && matches!(function.parameters.as_slice(), [parameter]
                    if parameter.shape == solid_facts::ast::BindingShape::Identifier
                        && parameter.initializer.is_none()
                        && parameter.names.first().is_some_and(|name| name.span == declaration))
        }) else {
            return false;
        };
        // `props` is only ever the object of a member read: an alias, a
        // spread, an argument or a write could change what `props.name`
        // holds without any tag showing it.
        let targets = file
            .ast
            .assignments
            .iter()
            .map(|assignment| assignment.target)
            .chain(file.ast.iteration_targets.iter().copied())
            .chain(file.ast.deleted_targets.iter().copied())
            .collect::<Vec<_>>();
        let only_read = file
            .ast
            .reference_declarations
            .iter()
            .filter(|(_, declared)| *declared == declaration)
            .all(|(reference, _)| {
                // A call through it (`props.replace()`) hands `props` to user
                // code as `this`, which may write any prop.
                file.ast.members.iter().any(|read| {
                    read.object == *reference
                        && !targets.iter().any(|target| target.contains(read.span))
                        && !file
                            .ast
                            .calls
                            .iter()
                            .any(|call| file.ast.peel_ts_sugar_span(call.callee) == read.span)
                })
            });
        if !only_read {
            return false;
        }
        let Some(renders) = self.rendered_only_through_jsx(file.path.as_str(), component.span)
        else {
            return false;
        };
        !renders.is_empty()
            && renders.into_iter().all(|(caller, element)| {
                element.spreads.is_empty()
                    && element
                        .attributes
                        .iter()
                        .filter(|attribute| {
                            attribute.namespace.is_none()
                                && caller.source_text(attribute.local_name) == Some(name)
                        })
                        .all(|attribute| match attribute.value_kind {
                            JsxAttributeValueKind::Boolean | JsxAttributeValueKind::String => true,
                            JsxAttributeValueKind::Expression => {
                                attribute.expression.is_some_and(|expression| {
                                    self.value_holds_builtin(caller, expression, member, depth - 1)
                                })
                            }
                            JsxAttributeValueKind::Element | JsxAttributeValueKind::Fragment => {
                                false
                            }
                        })
            })
    }

    /// The JSX elements that render the function at `(path, function)`, when
    /// those are every way of entering it: each enumerated site is a tag, and
    /// every reference to the function is one of them, its declaration, or
    /// its module surface. A call, a dialect renderer or a rendering prop
    /// makes this `None`.
    fn rendered_only_through_jsx(
        &self,
        path: &str,
        function: Span,
    ) -> Option<Vec<(&'a FileFacts, &'a solid_facts::ast::JsxElementFact)>> {
        let aliased = self.function_symbols(path, function)?;
        let mut renders = Vec::new();
        for (caller, callee) in self.lookup.function_call_sites(path, function) {
            renders.push((
                caller,
                caller
                    .ast
                    .jsx_elements
                    .iter()
                    .find(|element| element.name.span == callee)?,
            ));
        }
        let known = self
            .lookup
            .function_call_site_references(path, function)
            .into_iter()
            .map(|(caller, span)| (caller.path.to_string(), span.start, span.end))
            .collect::<HashSet<_>>();
        aliased
            .iter()
            .all(|candidate| {
                self.lookup
                    .symbol_references(candidate.as_str())
                    .iter()
                    .all(|reference| self.reference_is_accounted_for(reference, &known, false))
            })
            .then_some(renders)
    }

    /// ADR 0213: whether the argument at `argument`, the obligation's site,
    /// is a parameter that holds a built-in value (ADR 0211) on every entry
    /// of its function, so its `member` is the built-in prototype's.
    ///
    /// The parameter is a plain identifier of the innermost function around
    /// the argument, written nowhere in its file. The function is entered only
    /// through call expressions, so its call sites are every entry. Each one
    /// passes, at the parameter's position, a value `value_origin` proves, or
    /// a parameter of its own function that holds one. A default proves
    /// nothing unless it holds one too.
    fn parameter_holds_builtin(
        &self,
        file: &'a FileFacts,
        argument: Span,
        member: &str,
        depth: usize,
    ) -> bool {
        if depth == 0
            || self.lookup.member_may_be_reassigned_on_builtin(member)
            || self.lookup.member_may_be_reassigned_on_builtin("__proto__")
        {
            return false;
        }
        let argument = file.ast.peel_ts_sugar_span(argument);
        let Some(declaration) = file.ast.reference_declaration(argument) else {
            return false;
        };
        let Some((function, index)) = file
            .ast
            .functions_body_containing(argument)
            .min_by_key(|function| function.body.end - function.body.start)
            .and_then(|function| {
                function
                    .parameters
                    .iter()
                    .position(|parameter| {
                        parameter.shape == solid_facts::ast::BindingShape::Identifier
                            && parameter
                                .names
                                .first()
                                .is_some_and(|name| name.span == declaration)
                    })
                    .map(|index| (function, index))
            })
        else {
            return false;
        };
        if function.rest_parameter && index + 1 == function.parameters.len() {
            return false;
        }
        if crate::indexes::binding_written(file, declaration) {
            return false;
        }
        let builtin = |file: &'a FileFacts, span: Span, kind| {
            matches!(
                self.lookup.value_origin(file, span, kind, 6),
                Some(crate::indexes::ValueOrigin::Builtin)
            ) || self.value_holds_builtin(file, span, member, depth - 1)
        };
        if let Some(default) = function.parameters[index].initializer
            && !builtin(file, default, solid_facts::ast::RuntimeValueKind::Unknown)
        {
            return false;
        }
        if !self.entered_only_through_call_expressions(file.path.as_str(), function.span) {
            return false;
        }
        let sites = self
            .lookup
            .function_call_sites(file.path.as_str(), function.span);
        !sites.is_empty()
            && sites.into_iter().all(|(caller, callee)| {
                let Some(call) = self.lookup.call_by_callee(caller, callee) else {
                    return false;
                };
                if call
                    .arguments
                    .iter()
                    .take(index + 1)
                    .any(|argument| argument.spread)
                {
                    return false;
                }
                call.arguments
                    .get(index)
                    .is_none_or(|passed| builtin(caller, passed.span, passed.runtime_value_kind))
            })
    }

    fn reference_is_accounted_for(
        &self,
        reference: &Location,
        known_call_sites: &HashSet<(String, u32, u32)>,
        through_renders: bool,
    ) -> bool {
        let start = u32::try_from(reference.start_byte).unwrap_or(u32::MAX);
        let end = u32::try_from(reference.end_byte).unwrap_or(u32::MAX);
        if known_call_sites.contains(&(reference.path.to_string(), start, end)) {
            return true;
        }
        let Some(file) = self.file(reference.path.as_ref()) else {
            // A reference in a file the project does not analyze may be a
            // runtime entry nobody has seen. Its path or extension is no
            // evidence that it is erased.
            return false;
        };
        let span = Span::new(start, end);
        // A reference whose value reaches only rendering props is accounted
        // for by the renders it reaches, which `reach_from` walks as its edges
        // (ADR 0138).
        if through_renders
            && self
                .lookup
                .prop_render_sites_at(file.path.as_str(), span)
                .is_some()
        {
            return true;
        }
        // The declaration that introduces the function, and the import/export
        // surface that forwards it, are not runtime entries: emission resolves
        // the export surface itself, by name.
        if file
            .ast
            .functions
            .iter()
            .any(|function| function.name.as_ref().is_some_and(|name| name.span == span))
        {
            return true;
        }
        if file
            .ast
            .bindings
            .iter()
            .any(|binding| binding.names.iter().any(|name| name.span == span))
        {
            return true;
        }
        // Only the export *specifier* — `export { Panel }`, or the name a
        // declaration export introduces. An `ExportNamedDeclaration`'s own span
        // covers the whole declaration, body included (solid-facts
        // `visit_export_named_declaration`), so testing containment accepted
        // every reference written inside an exported function: `apply(Panel)`
        // and `return Panel` read as "export surface", and the escape they
        // prove was lost. That is the unsound direction — the caller reads a
        // true verdict as "the enumerated callers are all of them" and
        // certifies exports that a value-escaped function can reach.
        //
        // `<Panel/>` is not in that list, and is not accepted here either: a
        // rendered tag is a call site, so `known_call_sites` above already
        // holds its span — both of its spans, for `<Panel></Panel>`, since the
        // closing tag rides on the same edge. The acceptance is the call-graph
        // edge, not a syntactic exception for tags, so a tag that resolves to
        // nothing (an unresolved import) reaches none of these branches, which
        // is the honest answer: something the graph cannot name renders it.
        //
        // A dotted tag is the case where the edge exists and the reference is
        // still unaccounted for. `<ns.Panel/>` *does* resolve: TypeScript
        // reports the symbol at the whole `ns.Panel` name span, so the graph
        // emits an edge whose callee is that whole span. But the reference this
        // test walks is the `Panel` property *inside* the name, and this set is
        // a byte-exact span membership test, so the property does not match the
        // whole name and the enumeration reports itself incomplete. That is the
        // conservative direction; closing it means making the edge's callee and
        // the walked reference name the same span, which is a resolution
        // question rather than a widening one.
        if file.ast.exports.iter().any(|export| {
            export
                .specifiers
                .iter()
                .chain(export.declarations.iter())
                .any(|specifier| specifier.local.span == span)
        }) {
            return true;
        }
        file.ast
            .imports
            .iter()
            .any(|import| import.span.contains(span))
    }

    fn file(&self, path: &str) -> Option<&'a FileFacts> {
        self.files_by_path.get(path).copied()
    }
}

/// Every runtime reference to the namespace binding declared at
/// `declaration` names a static member (`ns.helper`). A reference inside a
/// type query is erased.
fn namespace_used_by_static_members(file: &solid_facts::FileFacts, declaration: Span) -> bool {
    file.ast
        .reference_declarations
        .iter()
        .filter(|(_, declared)| *declared == declaration)
        .all(|(reference, _)| {
            file.ast
                .type_queries
                .iter()
                .any(|query| query.contains(*reference))
                || file.ast.members.iter().any(|member| {
                    file.ast.peel_ts_sugar_span(member.object) == *reference
                        && file
                            .ast
                            .computed_members
                            .binary_search(&member.span)
                            .is_err()
                })
        })
}

/// A call's callee, by file and byte range.
type CallSiteKey = (String, u32, u32);
/// A reference, by file and byte range.
type ReferenceKey = (String, (u32, u32));

fn location_key(location: &Location) -> ReferenceKey {
    (
        location.path.to_string(),
        (
            u32::try_from(location.start_byte).unwrap_or(u32::MAX),
            u32::try_from(location.end_byte).unwrap_or(u32::MAX),
        ),
    )
}

/// The files a relative specifier with an explicit runtime extension loads at
/// run time: the written path, and the TypeScript source a bundler maps it to
/// (`./x.js` to `x.ts` or `x.tsx`). `None` for any other specifier, whose
/// runtime module a directory index or package `main` may select.
fn explicit_runtime_files(from: &str, specifier: &str) -> Option<Vec<String>> {
    // Only a plain spelling: a backslash, an empty segment, or a query or
    // fragment suffix may name the file differently.
    if (!specifier.starts_with("./") && !specifier.starts_with("../"))
        || specifier.contains('\\')
        || specifier.contains("//")
        || specifier.contains(['?', '#'])
    {
        return None;
    }
    let (stem, sources): (&str, &[&str]) = [
        (".js", &[".ts", ".tsx"][..]),
        (".jsx", &[".tsx"][..]),
        (".mjs", &[".mts"][..]),
        (".cjs", &[".cts"][..]),
        (".ts", &[][..]),
        (".tsx", &[][..]),
        (".mts", &[][..]),
        (".cts", &[][..]),
    ]
    .iter()
    .find_map(|(extension, sources)| {
        specifier
            .strip_suffix(extension)
            .map(|stem| (stem, *sources))
    })?;
    let mut segments: Vec<&str> = from.split('/').collect();
    segments.pop();
    for segment in stem.split('/') {
        match segment {
            "." => {}
            // Never above the importer's root: `/` splits to an empty first
            // segment, which stays.
            ".." if segments.len() > 1 => {
                segments.pop();
            }
            ".." => return None,
            segment => segments.push(segment),
        }
    }
    let base = segments.join("/");
    let written = format!("{base}{}", &specifier[stem.len()..]);
    let mut files = vec![written];
    files.extend(sources.iter().map(|source| format!("{base}{source}")));
    Some(files)
}
