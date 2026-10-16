//! Which project functions can reach an unresolved proof obligation.
//!
//! Contract emission has to say *which exports* an open claim belongs to.
//! An obligation that sits inside an exported function answers that question
//! lexically, and emission resolves that case itself from the AST. An
//! obligation inside a private helper does not: the only thing that can say
//! whether a consumer of export `A` — and not of export `B` — can trigger it
//! is the call graph.
//!
//! The call graph lives here, beside the interprocedural analysis that owns
//! it, and emission consumes only the answer ([`Program::obligation_reach`]).
//! The alternative — handing the raw graph across the process boundary and
//! walking it in the emitter — would put two independently drifting notions of
//! "calls" in the codebase, and the emitter's copy would be the one nobody
//! tests against real reactive code.
//!
//! The answer is deliberately shaped as *fail closed or exact*: either
//! [`ObligationReach::complete`] is true and `reaching` enumerates every
//! project function that can enter the obligation's own function, or it is
//! false and emission must fall back to marking every export. There is no
//! third "probably these" state, because a partial enumeration read as a
//! complete one silently certifies an export that can reach the obligation.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use solid_facts::{FileFacts, ProjectFacts, core::Span};
use typefacts::Location;

use crate::StaticDefect;
use crate::identity::SymbolId;
use crate::indexes::{EntitySymbols, SemanticLookup};

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
    if !lookup.program_closed
        || !defects
            .iter()
            .any(|defect| exported_dispatch(defect) || passed_parameter(defect))
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
        if passed_parameter(defect)
            && let crate::StaticDefectKind::ReactiveDispatchUnresolved {
                member: Some(member),
                ..
            } = &defect.kind
        {
            return !graph.value_holds_builtin(file, span, member, PASS_THROUGH_DEPTH);
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
            // The member veto holds for every origin, the direct one included.
            && !self.lookup.member_name_may_be_reassigned(member)
            && !self.lookup.member_name_may_be_reassigned("__proto__")
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
            || self.lookup.member_name_may_be_reassigned(member)
            || self.lookup.member_name_may_be_reassigned("__proto__")
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
            || self.lookup.member_name_may_be_reassigned(member)
            || self.lookup.member_name_may_be_reassigned("__proto__")
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
