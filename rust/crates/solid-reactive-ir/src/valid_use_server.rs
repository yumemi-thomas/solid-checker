//! SC7006 `valid-use-server`: where the `"use server"` directive takes effect
//! and what a module-level directive file may export or call (ADR 0427).
//!
//! # Premises
//!
//! All of them are the audited compiler's, `@solidjs/compiler@2.0.0-rc.17` as
//! pinned at the semantic-facts fork revision (`packages/compiler/src/
//! directives/`; the directive pass is the build plugin's, not part of
//! `@solidjs/web`), and `@solidjs/web@2.0.0-rc.17`'s server-functions runtime:
//!
//! - The pass reads a directive only from a module's or function body's
//!   prologue (`transform.rs:148-151` for the module, `:897-901`
//!   `body_has_directive` for a function), so a `"use server"` string
//!   statement after another statement, in a block, or as a template literal is
//!   an ordinary expression and the code stays on the client.
//! - A directive on an object method, getter, setter or class method is never
//!   extracted: rc.17 rejects it with a compile error (`validate.rs:403-437`,
//!   `:403-437`, rc.8 changeset a181e4d; earlier compilers ignore it silently).
//! - Directives inside an already-extracted server function are ignored
//!   (`validate.rs:247-258`), and a module-level directive makes the whole
//!   module server code, so function-level placement is not read there
//!   (`validate.rs` module doc).
//! - A module-level directive replaces the client module with bare references
//!   (`transform.rs:8-11`), so a call to a client-side declaration wrapper
//!   (`GET`, `withMeta`, `live`, router `query`, `action`, `liveQuery`) at module
//!   scope never runs on the client (RFC 10: "client-transport declarations
//!   like `GET` are meaningless inside a module-level file").
//! - A re-export is a compile error under a module-level directive since rc.8
//!   (`transform.rs:1377-1412`, changeset bb905db); a non-function export
//!   throws at server boot, `registerServerReference`
//!   (`@solidjs/web/server-functions/dist/server.js:1567-1571`).
//! - A *wrapped* export (`export const x = withValidation(schema, fn)`) is not a
//!   defect: since rc.4 the server registers each export's evaluated value and
//!   the compiler never inspects its shape (`transform.rs:411-420`; compiler
//!   fixture `module-wrapped-exports`). The RFC sentence the earlier rule
//!   encoded is superseded.

use solid_facts::FileFacts;
use solid_facts::ast::{ClassElementKind, ExportKind, FunctionFact, FunctionKind, ImportKind};
use solid_facts::core::Span;

use crate::name_matcher::NameMatcher;
use crate::pipeline::{AnalysisContext, ProgramDraft};
use crate::{StaticViolation, location};

const USE_SERVER: &str = "use server";

/// The wrappers upstream lists as client-side declarations
/// (`CLIENT_WRAPPERS`), matched by imported name.
const CLIENT_WRAPPERS: [&str; 6] = ["GET", "live", "withMeta", "query", "action", "liveQuery"];

/// The packages upstream trusts those names from (`WRAPPER_SOURCES`:
/// `^(?:solid-js|@solidjs/(?:web|router|start))(?:/|$)`), by specifier.
const WRAPPER_SOURCES: [&str; 4] = [
    "solid-js",
    "@solidjs/web",
    "@solidjs/router",
    "@solidjs/start",
];

const HINT_PLACEMENT: &str = "Put the directive first in the function body (or first in the module), as a plain string, in a function the compiler extracts: a function declaration or expression, or a block-bodied arrow.";

pub(crate) fn check_project(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    let custom = NameMatcher::new(ctx.rule_options.client_wrappers());
    let has_custom = !ctx.rule_options.client_wrappers().is_empty();
    for file in &ctx.facts.files {
        let module_level = file
            .ast
            .module_directives
            .iter()
            .any(|directive| directive.value == USE_SERVER);
        placement(file, module_level, draft);
        if module_level {
            let wrappers = wrapper_calls(file, &custom, has_custom);
            module_exports(file, &wrappers, draft);
            client_wrapper_calls(file, &wrappers, draft);
        }
    }
}

fn violation(
    file: &FileFacts,
    span: Span,
    message: String,
    hint: &str,
    uncertain: bool,
) -> StaticViolation {
    StaticViolation {
        id: "SC7006".into(),
        rule: "valid-use-server".into(),
        message,
        hint: hint.into(),
        location: location(file.path.shared(), span),
        analysis_context: String::new(),
        fixes: vec![],
        uncertain,
    }
}

/// Whether `function` sits inside another function that carries the
/// directive (the compiler extracts the outermost one only).
fn inside_server_function(file: &FileFacts, function: &FunctionFact) -> bool {
    file.ast
        .functions_owning(function.span)
        .any(|outer| outer.span != function.span && outer.has_directive(USE_SERVER))
}

/// The innermost function whose body contains `span`.
fn enclosing_function(file: &FileFacts, span: Span) -> Option<&FunctionFact> {
    file.ast
        .functions_body_containing(span)
        .min_by_key(|function| function.body.end - function.body.start)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Position {
    ObjectMethod,
    Accessor,
    ClassMethod,
}

impl Position {
    fn label(self) -> &'static str {
        match self {
            Self::ObjectMethod => "an object method",
            Self::Accessor => "a getter or setter",
            Self::ClassMethod => "a class method",
        }
    }
}

/// The position the compiler never extracts, when `function` is written in one
/// (upstream's `getUseServerEligibility`): an object-literal method or
/// accessor, or a class method, constructor or accessor. A function-valued
/// property, a class field's arrow and every other function is eligible.
fn ineligible_position(file: &FileFacts, function: &FunctionFact) -> Option<Position> {
    if function.kind == FunctionKind::Arrow {
        return None;
    }
    if let Some(property) = file
        .ast
        .object_properties
        .iter()
        .find(|property| property.value == function.span)
    {
        return if property.method {
            Some(Position::ObjectMethod)
        } else if !property.data {
            Some(Position::Accessor)
        } else {
            None
        };
    }
    file.ast
        .classes
        .iter()
        .flat_map(|class| &class.elements)
        .find(|element| element.value == Some(function.span))
        .and_then(|element| match element.kind {
            ClassElementKind::Method | ClassElementKind::Constructor => Some(Position::ClassMethod),
            ClassElementKind::Getter | ClassElementKind::Setter => Some(Position::Accessor),
            _ => None,
        })
}

fn placement(file: &FileFacts, module_level: bool, draft: &mut ProgramDraft) {
    // A template literal is never a directive, even in a module-level file.
    // A string statement the parser did not classify as part of a prologue is
    // misplaced unless the module is a directive file or an extracted server
    // function already holds it.
    for stray in &file.ast.stray_directives {
        if stray.value != USE_SERVER {
            continue;
        }
        if stray.template {
            draft.static_violations.push(violation(
                file,
                stray.span,
                "a template literal is never a directive: the compiler reads only a plain string in the prologue, so this statement is an ordinary expression and the code stays on the client".into(),
                "Write the directive as a plain string: \"use server\".",
                false,
            ));
            continue;
        }
        if module_level {
            continue;
        }
        // Inside an extracted server function (the function itself, or one it
        // encloses) the whole body already runs on the server, so a stray
        // directive "changes nothing" (`validate.rs:247-258`). Upstream
        // exempts only the nested functions; a stray statement in the
        // extracted function's own block is reported there, and silent here
        // (ownership case `valid-use-server/stray-in-extracted-function/001`).
        if enclosing_function(file, stray.span).is_some_and(|function| {
            function.has_directive(USE_SERVER) || inside_server_function(file, function)
        }) {
            continue;
        }
        draft.static_violations.push(violation(
            file,
            stray.span,
            if stray.direct {
                "the \"use server\" directive only takes effect as the first statement of a function body or module; here another statement precedes it, so it is an ordinary expression statement and the code stays on the client".into()
            } else {
                "the \"use server\" directive only takes effect as the first statement of a function body or module; a block has no directive prologue, so here it is an ordinary expression statement and the code stays on the client".into()
            },
            HINT_PLACEMENT,
            false,
        ));
    }
    if module_level {
        return;
    }
    // A prologue directive in a function the compiler does not extract.
    for function in &file.ast.functions {
        let Some(index) = function
            .directives
            .iter()
            .position(|directive| directive == USE_SERVER)
        else {
            continue;
        };
        let Some(&directive_span) = function.directive_spans.get(index) else {
            continue;
        };
        if inside_server_function(file, function) {
            continue;
        }
        let Some(position) = ineligible_position(file, function) else {
            continue;
        };
        draft.static_violations.push(violation(
            file,
            directive_span,
            format!(
                "the \"use server\" directive in {} is never extracted to the server: the compiler (rc.8 and later) rejects it with a compile error, earlier compilers ignore it and the body keeps running wherever it is called, the browser included",
                position.label()
            ),
            "Use a standalone function, a function-valued property, or a block-bodied arrow instead.",
            false,
        ));
    }
}

/// How a module-level directive file's export fares at server boot.
enum ExportShape {
    /// A function declaration or expression: registered.
    Function,
    /// A literal, template, array or object (upstream's set) or any proven
    /// non-callable value: `registerServerReference` throws at module
    /// evaluation.
    NotFunction(&'static str),
    /// Anything else evaluates at run time to something the facts do not name:
    /// a call's result, an alias, a conditional.
    Unresolved,
}

fn export_shape(file: &FileFacts, local: Span) -> ExportShape {
    let peeled = file.ast.peel_ts_sugar_span(local);
    if file.ast.functions.iter().any(|function| {
        function.span == peeled
            || (function.kind == FunctionKind::Declaration
                && function
                    .name
                    .as_ref()
                    .is_some_and(|name| name.span == local))
    }) {
        return ExportShape::Function;
    }
    if let Some(binding) = file
        .ast
        .bindings
        .iter()
        .find(|binding| binding.names.iter().any(|name| name.span == local))
    {
        if binding.initializer_function {
            return ExportShape::Function;
        }
        return match binding.initializer_value_kind {
            solid_facts::ast::RuntimeValueKind::Primitive => {
                ExportShape::NotFunction("a literal value")
            }
            solid_facts::ast::RuntimeValueKind::Object => ExportShape::NotFunction("an object"),
            solid_facts::ast::RuntimeValueKind::Array => ExportShape::NotFunction("an array"),
            solid_facts::ast::RuntimeValueKind::Nullish => {
                ExportShape::NotFunction("a nullish value")
            }
            _ => ExportShape::Unresolved,
        };
    }
    // `export default <expression>`: the recorded span is the expression.
    if file
        .ast
        .object_get_shapes
        .iter()
        .any(|shape| shape.span == peeled)
    {
        return ExportShape::NotFunction("an object");
    }
    if file.ast.array_literals.contains(&peeled) {
        return ExportShape::NotFunction("an array");
    }
    if file
        .ast
        .template_literals
        .iter()
        .any(|template| template.span == peeled)
    {
        return ExportShape::NotFunction("a template literal");
    }
    ExportShape::Unresolved
}

fn module_exports(file: &FileFacts, wrappers: &[WrapperCall], draft: &mut ProgramDraft) {
    for export in &file.ast.exports {
        if export.type_only {
            continue;
        }
        match export.kind {
            ExportKind::All => draft.static_violations.push(re_export(
                file,
                export.span,
                "every re-exported binding",
            )),
            ExportKind::Named if export.module.is_some() => {
                for specifier in export
                    .specifiers
                    .iter()
                    .filter(|specifier| !specifier.type_only)
                {
                    draft.static_violations.push(re_export(
                        file,
                        specifier.local.span,
                        specifier.exported.as_str(),
                    ));
                }
            }
            ExportKind::Named | ExportKind::Default => {
                for declaration in export
                    .declarations
                    .iter()
                    .filter(|declaration| !declaration.type_only)
                {
                    let exported = declaration.exported.as_str();
                    match export_shape(file, declaration.local.span) {
                        ExportShape::Function => {}
                        ExportShape::NotFunction(shape) => {
                            draft.static_violations.push(violation(
                                file,
                                declaration.local.span,
                                format!(
                                    "every export of a module-level \"use server\" file is registered as a server function, but export {exported} is {shape}: registration throws \"is not a function\" when the server module evaluates, so the server fails at boot"
                                ),
                                "Move non-function exports to a module without the directive.",
                                false,
                            ));
                        }
                        // A wrapper call is already its own finding.
                        ExportShape::Unresolved
                            if exported_call(file, declaration.local.span)
                                .is_some_and(|call| wrappers.iter().any(|w| w.span == call)) => {}
                        ExportShape::Unresolved => {
                            draft.static_violations.push(violation(
                                file,
                                declaration.local.span,
                                format!(
                                    "every export of a module-level \"use server\" file is registered by its evaluated value and must be a function, but the value of export {exported} is not shown to be one by the available facts; registration throws at server boot otherwise"
                                ),
                                "Export a function declaration or expression, or a call whose result is a function.",
                                true,
                            ));
                        }
                    }
                }
            }
        }
    }
}

fn re_export(file: &FileFacts, span: Span, exported: &str) -> StaticViolation {
    violation(
        file,
        span,
        format!(
            "a module-level \"use server\" file cannot re-export: {exported} is a re-export, which the compiler rejects with a compile error (rc.8 and later; earlier compilers drop it from the client build)"
        ),
        "Re-export it from a module without the directive.",
        false,
    )
}

/// The built-in wrapper a call's callee is, when its identifier is exactly an
/// import of one of the listed names from one of the listed packages.
///
/// The callee resolves through the binder's scope-aware reference table to the
/// import specifier's local binding, so a shadowing parameter or a project
/// function spelled `query` is not one, and a renamed import (`query as q`) is.
fn built_in_wrapper(file: &FileFacts, callee: Span) -> bool {
    let Some(declaration) = file.ast.reference_declaration(callee) else {
        return false;
    };
    file.ast.imports.iter().any(|import| {
        !import.type_only
            && WRAPPER_SOURCES.iter().any(|source| {
                import.module == *source
                    || import
                        .module
                        .strip_prefix(source)
                        .is_some_and(|rest| rest.starts_with('/'))
            })
            && import.bindings.iter().any(|binding| {
                binding.kind == ImportKind::Named
                    && !binding.type_only
                    && binding.local.span == declaration
                    && binding
                        .imported
                        .as_deref()
                        .is_some_and(|name| CLIENT_WRAPPERS.contains(&name))
            })
    })
}

/// The call a module-level export's initializer is, when it is one.
fn exported_call(file: &FileFacts, local: Span) -> Option<Span> {
    let peeled = file.ast.peel_ts_sugar_span(local);
    file.ast
        .bindings
        .iter()
        .find(|binding| binding.names.iter().any(|name| name.span == local))
        .and_then(|binding| binding.call_initializer)
        .or_else(|| file.ast.call_at(peeled).map(|call| call.span))
}

/// A module-scope call to a client-side declaration wrapper.
struct WrapperCall {
    span: Span,
    name: String,
    built_in: bool,
}

fn wrapper_calls(file: &FileFacts, custom: &NameMatcher, has_custom: bool) -> Vec<WrapperCall> {
    let mut found = Vec::new();
    for call in &file.ast.calls {
        // Only a module-scope call to a plain identifier: a call inside a
        // function is server-side code, and a member call names no import.
        if call.construct
            || !call.direct_callee
            || file
                .ast
                .functions
                .iter()
                .any(|function| function.span.contains(call.span))
        {
            continue;
        }
        let Some(name) = file.source_text(call.callee) else {
            continue;
        };
        let built_in = built_in_wrapper(file, call.callee);
        // A name in the project's own `clientWrappers` list is the project's
        // statement, not a resolution: it is matched by the callee's local
        // spelling, as upstream does.
        if built_in || (has_custom && custom.matches(name)) {
            found.push(WrapperCall {
                span: call.span,
                name: name.to_owned(),
                built_in,
            });
        }
    }
    found
}

fn client_wrapper_calls(file: &FileFacts, wrappers: &[WrapperCall], draft: &mut ProgramDraft) {
    for wrapper in wrappers {
        draft.static_violations.push(violation(
            file,
            wrapper.span,
            format!(
                "in a module-level \"use server\" file the client receives bare server references, so the call to {}, {}, never runs on the client and its behavior is silently lost",
                wrapper.name,
                if wrapper.built_in {
                    "a client-side declaration wrapper imported from Solid"
                } else {
                    "a client wrapper named by the project's `clientWrappers` option"
                }
            ),
            "Wrap the imported server function in a shared module instead, or move the directive inside the wrapped function.",
            false,
        ));
    }
}
