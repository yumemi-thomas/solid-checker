//! Where code can only run synchronously inside a user effect's apply
//! callback (the dialect's *effect phase*,
//! [`solid_dialect::Dialect::callback_runs_in_effect_phase`]).
//!
//! The claim is universal: whenever the call runs, it runs on the stack of an
//! apply callback, so a `flush()` there drains nothing wherever it runs
//! (ADR 0341). Reachability is therefore no part of the proof: a call in a
//! dead branch, or in an effect that is never registered, is as pointless as
//! one that runs. What the proof needs is that *no* execution of the call is
//! outside the effect phase. The walk climbs from the span outward, one
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
//! - the layer is a project function bound to a name nothing reassigns, not
//!   exported, with at least one call and *every* reference to it one of its
//!   exact calls, each of which is itself proven by this walk. A caller
//!   outside the effect phase, an alias, a handler, an argument or an export
//!   would let the flush run elsewhere.
//!
//! The file holding the `flush` call and the file holding the `createEffect`
//! call must load the same installed `solid-js` at runtime
//! ([`crate::PackageInstanceEvidence`], the nearest installed copy with
//! symlinks resolved), both through imports of `solid-js` itself (through
//! project barrels at most): a call imported through another package's
//! `exports` conditions is not claimed.
//!
//! Anything else answers `None`: an unresolved callee, a callback handed to a
//! timer, listener or promise, a generator (calling one runs none of its body),
//! a method, a parameter initializer, an instance field initializer, a call
//! through a binding that may have been replaced. An async layer is on the
//! stack only up to its first suspension.

use crate::{HashMap, HashSet};

use solid_facts::FileFacts;
use solid_facts::ast::{ArgumentValueKind, FunctionFact};
use solid_facts::core::Span;

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
    /// The file that imports the package, and the specifier it writes.
    Found(String, String),
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
    /// The apply callback argument the walk ended at.
    pub(crate) apply: Span,
}

/// The effect call whose apply callback provably runs the code at `span`
/// synchronously, or `None` where that is not proven.
pub(crate) fn effect_phase_origin(
    file: &FileFacts,
    span: Span,
    package_root: &str,
    instances: &crate::PackageInstanceEvidence,
    lookup: &SemanticLookup<'_>,
) -> Option<EffectPhaseOrigin> {
    walk_to_effect_phase(file, span, package_root, instances, lookup, false)
}

/// [`effect_phase_origin`] through layers that keep the apply callback's
/// owner (ADR 0483): no primitive callback but `untrack`'s is crossed, since
/// `runWithOwner`, `createRoot` and the rest set an owner (or may) before
/// their callback runs. The apply runs with none (`createEffect`'s is queued
/// to the flush), so code reached this way runs with no owner.
pub(crate) fn ownerless_effect_phase_origin(
    file: &FileFacts,
    span: Span,
    package_root: &str,
    instances: &crate::PackageInstanceEvidence,
    lookup: &SemanticLookup<'_>,
) -> Option<EffectPhaseOrigin> {
    walk_to_effect_phase(file, span, package_root, instances, lookup, true)
}

fn walk_to_effect_phase(
    file: &FileFacts,
    span: Span,
    package_root: &str,
    instances: &crate::PackageInstanceEvidence,
    lookup: &SemanticLookup<'_>,
    owner_preserving: bool,
) -> Option<EffectPhaseOrigin> {
    let mut walk = Walk {
        owner_preserving,
        lookup,
        package_root,
        flush_instance: "",
        instances,
        files: std::cell::OnceCell::new(),
        visiting: HashSet::default(),
        budget: BUDGET,
    };
    // The instance the flush call's own import loads, followed through
    // re-exports to the file that imports the package; unresolved is no proof.
    let flush_call = file.ast.calls.iter().find(|call| call.span == span)?;
    let flush_instance = walk.instance_of(file, flush_call.callee)?;
    walk.flush_instance = flush_instance;
    walk.layer(file, span, 0)
}

struct Walk<'l, 'a> {
    /// Cross only layers that keep the owner ([`ownerless_effect_phase_origin`]).
    owner_preserving: bool,
    lookup: &'l SemanticLookup<'a>,
    /// The package instance the starting call's declaration is in: the effect
    /// call must resolve to the same one, or its queue is not the one running.
    package_root: &'l str,
    /// The `solid-js` directory the flush call's file loads at runtime, and
    /// what every analyzed file loads.
    flush_instance: &'l str,
    instances: &'l crate::PackageInstanceEvidence,
    /// The analyzed files by path, built when a question first needs them.
    files: std::cell::OnceCell<HashMap<&'a str, &'a FileFacts>>,
    /// Functions the walk is inside through a call, so a call cycle ends.
    visiting: HashSet<(String, Span)>,
    budget: usize,
}

impl<'l, 'a> Walk<'l, 'a> {
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
        if let Some((call, index)) = argument_position(file, literal) {
            if let Some(origin) = self.argument_layer(file, call, index, helper_calls) {
                return Some(origin);
            }
            // ADR 0484: a project helper that calls this argument in its own
            // body on every completion, and never hands it on or keeps it
            // (`layer_run`'s `InPlace`), runs it on the helper call's stack, so
            // every run of the literal is a run of that call. A primitive's or
            // an unknown callee's argument is not such a layer.
            return match crate::execution_role::layer_run(file, literal, self.lookup) {
                crate::execution_role::LayerRun::InPlace(at) if at == call.span => {
                    self.layer(file, at, helper_calls + 1)
                }
                _ => None,
            };
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
                // The queue the flush finds running must be this apply's.
                return self
                    .same_package_instance(file, call)
                    .then(|| EffectPhaseOrigin {
                        path: file.path.to_string(),
                        helper_calls,
                        apply: call.arguments[index].span,
                    });
            }
            if self.owner_preserving && primitive != solid_dialect::Primitive::Untrack {
                return None;
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
        self.instance_of(file, call.callee) == Some(self.flush_instance)
            // ADR 0483: in the owner-preserving walk the creating call and the
            // effect may be declared in different files of one installation
            // (rc.17's `createMemo` is `solid-js`'s own overload object, its
            // `createEffect` is `typeof` signals'); both imports loading the
            // same installed `solid-js` copy is what makes them share the
            // signals copy, and with it the marker and the owner.
            && (self.owner_preserving
                || self
                    .lookup
                    .resolved_callee_call(file, call.callee)
                    .and_then(|resolved| resolved.declaration.as_ref())
                    .and_then(|declaration| package_root(declaration.location.path.as_ref()))
                    .is_some_and(|root| root == self.package_root))
    }

    /// The installed `solid-js` the file that imports the function `callee`
    /// calls loads, when it imports `solid-js` itself (through project barrels
    /// at most). A function imported through another package is loaded by
    /// that package's `exports` conditions, which this does not select.
    fn instance_of(&self, file: &FileFacts, callee: Span) -> Option<&'l str> {
        let (path, module) = self.declaring_file(file, callee)?;
        if module != "solid-js" && !module.starts_with("solid-js/") {
            return None;
        }
        self.instances
            .solid_js_by_file
            .get(path.as_str())
            .map(String::as_str)
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
    fn declaring_file(&self, file: &FileFacts, callee: Span) -> Option<(String, String)> {
        let callee = file.ast.peel_ts_sugar_span(callee);
        let (object, name) = match file.ast.member_at(callee) {
            Some(member) => (
                file.ast.peel_ts_sugar_span(member.object),
                Some(file.source_text(member.property)?.to_owned()),
            ),
            None => (callee, None),
        };
        let mut declaration = file.ast.reference_declaration(object)?;
        // `const { flush: drain } = Solid` (ADR 0459): `drain()` calls the
        // namespace's member, as `Solid.flush()` does.
        let mut name = name;
        if let Some(member) = file.ast.namespace_member_binding(declaration) {
            if name.is_some() {
                return None;
            }
            declaration = member.namespace;
            name = Some(member.key.to_string());
        }
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
            Chain::Found(path, module) => Some((path, module)),
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
            return Chain::Found(file.path.to_string(), module.to_owned());
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
        let mut found: Option<(String, String)> = None;
        for export in stars {
            let module = export.module.as_deref().unwrap_or_default();
            match self.follow_export(target, export.span, module, name, depth + 1) {
                Chain::Found(path, module) => {
                    let next = (path, module);
                    if found.as_ref().is_some_and(|known| known != &next) {
                        return Chain::Unknown;
                    }
                    found = Some(next);
                }
                Chain::Missing => {}
                Chain::Unknown => return Chain::Unknown,
            }
        }
        found.map_or(Chain::Missing, |(path, module)| Chain::Found(path, module))
    }

    /// `literal` is a function bound to a name: its flush runs only in the
    /// effect phase when it has calls, is neither exported nor referenced
    /// other than by those calls, and every one of them is a proven layer.
    fn named_layer(
        &mut self,
        file: &FileFacts,
        literal: &FunctionFact,
        helper_calls: usize,
    ) -> Option<EffectPhaseOrigin> {
        let binding = function_binding_name(file, literal)?;
        if !self.lookup.function_value_is_current(file, literal)
            || exported(self.lookup, file, literal)
        {
            return None;
        }
        let path = file.path.as_str();
        let symbol = self.lookup.entities().at(path, binding.span)?;
        let sites = self.lookup.function_call_sites(path, literal.span);
        if sites.is_empty() {
            return None;
        }
        let accounted: HashSet<(String, u32, u32)> = self
            .lookup
            .function_call_site_references(path, literal.span)
            .into_iter()
            .map(|(caller, callee)| (caller.path.to_string(), callee.start, callee.end))
            .collect();
        let escapes = self
            .lookup
            .symbol_references(symbol.as_str())
            .iter()
            .any(|reference| {
                let start = u32::try_from(reference.start_byte).unwrap_or(u32::MAX);
                let end = u32::try_from(reference.end_byte).unwrap_or(u32::MAX);
                let declaration = reference.path.as_ref() == path
                    && start == binding.span.start
                    && end == binding.span.end;
                !declaration && !accounted.contains(&(reference.path.to_string(), start, end))
            });
        if escapes {
            return None;
        }
        let key = (file.path.to_string(), literal.span);
        if !self.visiting.insert(key.clone()) {
            return None;
        }
        let mut found: Option<EffectPhaseOrigin> = None;
        let mut every = true;
        for (caller, callee) in sites {
            if self.budget == 0 {
                every = false;
                break;
            }
            self.budget -= 1;
            // A call expression of the function, not a JSX tag that renders it
            // or a `new`.
            let Some(call) = caller.ast.calls.iter().find(|call| {
                !call.construct && caller.ast.peel_ts_sugar_span(call.callee) == callee
            }) else {
                every = false;
                break;
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
            let origin = exact
                .then(|| self.layer(caller, call.span, helper_calls + 1))
                .flatten();
            match origin {
                Some(origin) => {
                    if found
                        .as_ref()
                        .is_none_or(|known| origin.helper_calls < known.helper_calls)
                    {
                        found = Some(origin);
                    }
                }
                None => {
                    every = false;
                    break;
                }
            }
        }
        self.visiting.remove(&key);
        if every { found } else { None }
    }
}

/// Whether the function is part of its module's surface, which callers
/// outside the analyzed project may use.
/// ADR 0484: the module exports the function itself. An export statement's
/// span holds every function nested in the exported one, which is not
/// exported by it (`crate::execution_role::module_exports`).
fn exported(lookup: &SemanticLookup<'_>, file: &FileFacts, function: &FunctionFact) -> bool {
    crate::execution_role::module_exports(file, function, lookup)
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
