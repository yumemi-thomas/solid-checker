//! ADR 0505: which project modules may run code when they are evaluated.
//!
//! A user effect's first apply runs in `render`'s tail flush; a reactive node a
//! module created when it was imported (a root, an effect, a pending render
//! effect) may share that flush and hold it. The census answers the files
//! whose module scope may run code at all, and whether some file loads a
//! module no analyzed file carries (a dependency other than the dialect's own
//! packages, an unresolved or declaration-only module), whose evaluation is
//! unseen.

use super::*;

/// The census: the analyzed project files (outside `node_modules`) whose
/// module scope may run code, sorted; those of them whose scope only creates
/// nodes (ADR 0510, [`crate::execution_role::module_scope_only_creates`]);
/// the runtime imports and re-exports between project files, as sorted
/// `(importer, imported)` pairs (ADR 0510: an entry none imports is a root of
/// evaluation, and no other root may evaluate the source first); and whether a
/// project file loads a module whose evaluation no analyzed file shows.
pub(crate) type ModuleScopeCensus = (Vec<String>, Vec<String>, Vec<(String, String)>, bool);

impl SemanticLookup<'_> {
    /// [`census`] as the whole-program memo's answer.
    pub(crate) fn module_scope_census(&self) -> ModuleScopeCensus {
        self.whole_program_answer::<super::project_memos::ModuleScopeRuns>()
    }
}

/// Whether `span` is outside every function of `ast`, other than a function
/// whose own span it is.
pub(crate) fn at_module_scope(ast: &solid_facts::ast::AstFacts, span: Span) -> bool {
    ast.functions_owning(span)
        .all(|function| function.span == span)
}

pub(super) fn census(lookup: &SemanticLookup<'_>) -> ModuleScopeCensus {
    let mut loud = Vec::new();
    let mut creating = Vec::new();
    let mut edges = Vec::new();
    let mut unseen = false;
    for file in lookup.files() {
        let path = file.path.as_str();
        if path.contains("/node_modules/") || is_declaration_file(path) {
            continue;
        }
        unseen |= loads_unseen_module(lookup, file);
        if module_scope_may_run(file) {
            loud.push(path.to_string());
            if crate::execution_role::module_scope_only_creates(file, lookup) {
                creating.push(path.to_string());
            }
        }
        let ast = &file.ast;
        let modules = ast
            .imports
            .iter()
            .filter(|import| !import.type_only)
            .map(|import| import.module.as_str())
            .chain(
                ast.exports
                    .iter()
                    .filter(|export| !export.type_only)
                    .filter_map(|export| export.module.as_deref()),
            );
        for module in modules {
            if let Some(target) = lookup.relative_module_path(path, module) {
                edges.push((path.to_string(), target.to_string()));
            }
        }
    }
    for list in [&mut loud, &mut creating] {
        list.sort();
        list.dedup();
    }
    edges.sort();
    edges.dedup();
    (loud, creating, edges, unseen)
}

fn is_declaration_file(path: &str) -> bool {
    [".d.ts", ".d.mts", ".d.cts"]
        .iter()
        .any(|suffix| path.ends_with(suffix))
}

/// A runtime import or re-export of a module that is neither one of the
/// dialect's own packages (whose evaluation installs hooks and creates no
/// reactive node: `@solidjs/signals@2.0.0-rc.17` `dist/dev.js:991`, `:1436`,
/// `:2114`) nor an analyzed project file, or a dynamic load at all.
fn loads_unseen_module(lookup: &SemanticLookup<'_>, file: &FileFacts) -> bool {
    let ast = &file.ast;
    let landed = |module: &str| {
        if lookup.dialect.owns_module(module) {
            return true;
        }
        let relative = module.starts_with('.') || module.starts_with('/');
        relative
            && lookup
                .relative_module_path(file.path.as_str(), module)
                .is_some_and(|path| !path.contains("/node_modules/") && !is_declaration_file(path))
    };
    !ast.module_loads.is_empty()
        || !ast.import_equals.is_empty()
        || ast
            .imports
            .iter()
            .any(|import| !import.type_only && !landed(&import.module))
        || ast.exports.iter().any(|export| {
            !export.type_only
                && export
                    .module
                    .as_deref()
                    .is_some_and(|module| !landed(module))
        })
}

/// Whether evaluating the module may run code beyond binding its declarations:
/// at module scope, a call or construction, a member access (a getter), a
/// tagged template, JSX, a class (decorators, static blocks), an `await`, an
/// identifier reference (a binding in its temporal dead zone, a value coerced
/// or iterated), a function other than a declaration or a `const`'s whole
/// initializer (a method a template could coerce through), a `throw`, or a
/// module hazard.
fn module_scope_may_run(file: &FileFacts) -> bool {
    let ast = &file.ast;
    let scope = |span: Span| at_module_scope(ast, span);
    !ast.module_hazards.is_empty()
        || ast.calls.iter().any(|call| scope(call.span))
        || ast.members.iter().any(|member| scope(member.span))
        || ast.computed_members.iter().any(|member| scope(*member))
        || ast.tagged_template_tags.iter().any(|tag| scope(*tag))
        || ast.jsx_elements.iter().any(|element| scope(element.span))
        || ast.jsx_fragments.iter().any(|fragment| scope(*fragment))
        || ast.classes.iter().any(|class| scope(class.span))
        || ast.awaits.iter().any(|awaited| scope(*awaited))
        || ast
            .identifiers
            .iter()
            .any(|identifier| runtime_reference(ast, identifier) && scope(identifier.span))
        || ast
            .functions
            .iter()
            .any(|function| scope(function.span) && !declared_function(ast, function))
        || ast
            .throw_statements
            .iter()
            .any(|statement| scope(*statement))
}

/// A reference to a value: not a name a type position erases (`as const`'s
/// `const`, a type reference, a type query).
pub(crate) fn runtime_reference(
    ast: &solid_facts::ast::AstFacts,
    identifier: &solid_facts::ast::IdentifierFact,
) -> bool {
    identifier.role == solid_facts::ast::IdentifierRole::Reference
        && !ast
            .type_names
            .iter()
            .chain(&ast.type_queries)
            .any(|erased| erased.contains(identifier.span))
}

/// A function declaration, or the whole initializer of a `const` binding one
/// name.
pub(crate) fn declared_function(
    ast: &solid_facts::ast::AstFacts,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    function.method_name.is_none()
        && (function.kind == solid_facts::ast::FunctionKind::Declaration
            || ast.bindings.iter().any(|binding| {
                binding.immutable
                    && binding.shape == solid_facts::ast::BindingShape::Identifier
                    && binding.initializer.is_some_and(|initializer| {
                        ast.peel_ts_sugar_span(initializer) == function.span
                    })
            }))
}
