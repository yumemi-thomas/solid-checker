//! The Solid 2.0 server-surface rules: the HTTP response head under
//! streaming SSR (SC7005) and the server-function compiler and transport
//! contracts (SC7006, SC7007).
//!
//! All three rules describe `@solidjs/web@2.0.0-rc.0` behavior and are
//! emitted only where the vocabulary or the dialect's server-function model
//! resolves — a 1.x project never sees them.

use std::collections::HashSet;

use solid_dialect::Primitive;
use solid_facts::FileFacts;
use solid_facts::ast::{ArgumentValueKind, ExportKind, FunctionKind, RuntimeValueKind};
use solid_facts::core::Span;
use typefacts::{ConstantValueKind, EntityFact, ResolvedCallValidity};

use crate::execution_role::semantic_execution_role;
use crate::owners::{containing_ast_function, jsx_element_is_loading};
use crate::pipeline::{AnalysisContext, ProgramDraft};
use crate::{ExecutionRole, StaticViolation, location, primitive_name};

/// The directive whose presence turns a function or module into server-build
/// material (RFC 10: the compiler's only contract).
const USE_SERVER: &str = "use server";

pub(crate) fn check_project(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    http_response_after_flush(ctx, draft);
    if ctx.dialect.models_server_functions() {
        server_function_module_directive(ctx, draft);
        server_function_rich_argument(ctx, draft);
    }
}

/// SC7005: an `httpStatus`/`httpHeader` call whose scope renders below a
/// `Loading` boundary. Both primitives gate the write *and* the cleanup-time
/// retraction on `!response.committed` (`@solidjs/web@2.0.0-rc.0`
/// `dist/server.js:2901-2975`), and `createSSRResponse` commits the stub at
/// the shell flush — so a call made by boundary content that settles after
/// the shell went out is a silent no-op by contract (RFC 12: "there is no
/// queue"). The finding is uncertifiable because a boundary that settles
/// *before* the flush still applies its writes; source analysis cannot prove
/// which side of that request-time race any invocation reaches.
fn http_response_after_flush(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    let mut server_renders = None;
    let mut loading_hosts: Option<HashSet<(&str, Span)>> = None;
    for file in &ctx.facts.files {
        let mut allowed = None;
        for call in &file.ast.calls {
            let Some(kind @ (Primitive::HttpStatus | Primitive::HttpHeader)) = primitive_name(
                file.path.as_str(),
                call.callee,
                call.static_callee(&file.source),
                ctx.entities,
                ctx.symbol_names,
                ctx.dialect,
            )
            .as_ref()
            .and_then(crate::PrimitiveName::primitive) else {
                continue;
            };
            let server_renders = *server_renders
                .get_or_insert_with(|| crate::source_discovery::project_server_renders(ctx.facts));
            // Render-time scopes only. An event handler or deferred callback
            // is a client-time (or post-render) call and is a no-op for a
            // different reason than the post-flush drop; unknown scopes stay
            // silent.
            let allowed = allowed.get_or_insert_with(|| {
                crate::execution_role::allowed_callback_spans(file, ctx.semantic_lookup)
            });
            let role = semantic_execution_role(
                file,
                call.span,
                allowed,
                ctx.entities,
                ctx.symbol_names,
                ctx.semantic_lookup,
            );
            if !matches!(
                role,
                ExecutionRole::UntrackedRendering | ExecutionRole::TrackedJsx
            ) {
                continue;
            }
            // Loading dominance, children position only: content in the
            // boundary's `fallback` renders *with* the shell and its writes
            // apply, so only the children subtree is post-flush material.
            let dominated = call_under_loading_children(ctx, file, call.span)
                || containing_ast_function(&file.ast, call.span).is_some_and(|function| {
                    loading_hosts
                        .get_or_insert_with(|| loading_wrapped_components(ctx))
                        .contains(&(file.path.as_str(), function.span))
                });
            if !dominated {
                continue;
            }
            let name = match kind {
                Primitive::HttpStatus => "httpStatus",
                _ => "httpHeader",
            };
            draft.static_violations.push(StaticViolation {
                id: "SC7005".into(),
                rule: "http-response-after-flush".into(),
                message: if server_renders {
                    format!(
                        "{name}() is called by content below a <Loading> boundary; under streaming SSR the response head commits at the shell flush, and when this boundary settles after the shell has flushed the call is a committed no-op — the {} is silently dropped, with no queue holding it for later",
                        if kind == Primitive::HttpStatus { "status" } else { "header" }
                    )
                } else {
                    format!(
                        "{name}() is called by content below a <Loading> boundary, but the analyzed project cannot prove whether a server-rendering entry exists; if this application streams SSR and the boundary settles after the shell flush, the {} is silently dropped",
                        if kind == Primitive::HttpStatus { "status" } else { "header" }
                    )
                },
                hint: format!(
                    "Decide the response head in shell content — above every <Loading> boundary — or mark the async source this {name}() depends on with deferStream: true so the shell flush waits for it. A boundary may settle before or after the flush, so move the decision to make the response certifiable."
                ),
                location: location(file.path.shared(), call.callee),
                analysis_context: String::new(),
                fixes: vec![],
                uncertain: true,
            });
        }
    }
}

/// Whether `span` sits in the *children* of a `Loading` element in the same
/// file. The boundary's `fallback` attribute is deliberately excluded: the
/// fallback is shell content.
fn call_under_loading_children(ctx: &AnalysisContext<'_>, file: &FileFacts, span: Span) -> bool {
    file.ast.jsx_containing(span).any(|element| {
        jsx_element_is_loading(file, element, ctx.entities, ctx.symbol_names, ctx.dialect)
            && element.children.iter().any(|child| child.contains(span))
    })
}

/// The project functions rendered as JSX children of a `Loading` element —
/// one component-boundary level deep, matching the dominance the async rules
/// use. Fallback-position renders are excluded (shell content).
fn loading_wrapped_components<'a>(ctx: &AnalysisContext<'a>) -> HashSet<(&'a str, Span)> {
    let mut hosts = HashSet::new();
    for caller in &ctx.facts.files {
        for element in &caller.ast.jsx_elements {
            let Some((target_file, target)) = ctx
                .semantic_lookup
                .function_called_at(caller.path.as_str(), element.name.span)
            else {
                continue;
            };
            let wrapped = caller.ast.jsx_elements.iter().any(|boundary| {
                boundary.span != element.span
                    && jsx_element_is_loading(
                        caller,
                        boundary,
                        ctx.entities,
                        ctx.symbol_names,
                        ctx.dialect,
                    )
                    && boundary
                        .children
                        .iter()
                        .any(|child| child.contains(element.span))
            });
            if wrapped {
                hosts.insert((target_file.path.as_str(), target.span));
            }
        }
    }
    hosts
}

/// SC7006: a module-level `"use server"` directive with exports that are not
/// direct function declarations. RFC 10 §Compiler implications records the
/// compiler defect this surfaces: "with a module-level `\"use server\"`
/// directive, a wrapped export (`export const x = wrapper(async () => ...)`)
/// is silently dropped from the client build — only direct function exports
/// become references. ... Minimum: a diagnostic." The directive pass is a
/// build-plugin contract, not part of the pinned `@solidjs/web` runtime, so
/// the RFC text is the specification this rule encodes.
///
/// Positive forms: a wrapped export (call-expression initializer), a
/// non-function `export default` expression, and re-exports (`export { x }
/// from`, `export * from`). Identifier aliases, plain values, and other shapes
/// that are neither proven direct functions nor proven wrapped exports are
/// explicit uncertifiable results.
fn server_function_module_directive(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    for file in &ctx.facts.files {
        if !file
            .ast
            .module_directives
            .iter()
            .any(|directive| directive.value == USE_SERVER)
        {
            continue;
        }
        for export in &file.ast.exports {
            if export.type_only {
                continue;
            }
            match export.kind {
                ExportKind::All => {
                    push_module_directive_finding(
                        draft,
                        file,
                        export.span,
                        "every re-exported binding",
                        "a re-export",
                        false,
                    );
                }
                ExportKind::Named if export.module.is_some() => {
                    for specifier in export
                        .specifiers
                        .iter()
                        .filter(|specifier| !specifier.type_only)
                    {
                        push_module_directive_finding(
                            draft,
                            file,
                            specifier.local.span,
                            specifier.exported.as_str(),
                            "a re-export",
                            false,
                        );
                    }
                }
                ExportKind::Named | ExportKind::Default => {
                    for specifier in export
                        .declarations
                        .iter()
                        .filter(|specifier| !specifier.type_only)
                    {
                        match module_export_shape(file, specifier.local.span) {
                            ModuleExportShape::DirectFunction => {}
                            ModuleExportShape::NotDirect(shape) => {
                                push_module_directive_finding(
                                    draft,
                                    file,
                                    specifier.local.span,
                                    specifier.exported.as_str(),
                                    shape,
                                    false,
                                );
                            }
                            ModuleExportShape::Unresolved => {
                                push_module_directive_finding(
                                    draft,
                                    file,
                                    specifier.local.span,
                                    specifier.exported.as_str(),
                                    "an export whose runtime shape cannot be proven",
                                    true,
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

enum ModuleExportShape {
    DirectFunction,
    NotDirect(&'static str),
    Unresolved,
}

/// Classifies an exported declaration without treating an unrecognized shape
/// as a direct-function proof.
fn module_export_shape(file: &FileFacts, local: Span) -> ModuleExportShape {
    // A function declaration exported by name, or a default-exported
    // function/class expression whose recorded span is the function itself.
    if file.ast.functions.iter().any(|function| {
        function.span == file.ast.peel_ts_sugar_span(local)
            || (function.kind == FunctionKind::Declaration
                && function
                    .name
                    .as_ref()
                    .is_some_and(|name| name.span == local))
    }) {
        return ModuleExportShape::DirectFunction;
    }
    if let Some(binding) = file
        .ast
        .bindings
        .iter()
        .find(|binding| binding.names.iter().any(|name| name.span == local))
    {
        // `export const x = wrap(fn)` — the RFC's named case. A direct
        // function expression is a reference; anything else stays silent
        // rather than guessed.
        if binding.initializer_function {
            return ModuleExportShape::DirectFunction;
        }
        return if binding.call_initializer.is_some() {
            ModuleExportShape::NotDirect("a wrapped function")
        } else {
            ModuleExportShape::Unresolved
        };
    }
    // `export default <expression>`: the recorded local span is the
    // expression. A call is the proven wrapped form.
    let value = file.ast.peel_ts_sugar_span(local);
    if file.ast.calls.iter().any(|call| call.span == value) {
        ModuleExportShape::NotDirect("a wrapped function")
    } else {
        ModuleExportShape::Unresolved
    }
}

fn push_module_directive_finding(
    draft: &mut ProgramDraft,
    file: &FileFacts,
    span: Span,
    exported: &str,
    shape: &str,
    uncertain: bool,
) {
    draft.static_violations.push(StaticViolation {
        id: "SC7006".into(),
        rule: "server-function-module-directive".into(),
        message: if uncertain {
            format!(
                "this module's top-level \"use server\" directive requires direct function exports, but export {exported} is {shape}; available syntax facts cannot prove that the compiler will turn it into a client reference"
            )
        } else {
            format!(
                "this module's top-level \"use server\" directive extracts every export to the server build, but export {exported} is {shape}, not a direct function declaration; the compiler turns only direct function exports into client references, so this export is silently dropped from the client build"
            )
        },
        hint: "Move the \"use server\" directive into each function body and keep the wrapper at the export site — export const getData = GET(async (id) => { \"use server\"; ... }) round-trips the wrapper call — or export plain functions from this module and wrap them where they are imported.".into(),
        location: location(file.path.shared(), span),
        analysis_context: String::new(),
        fixes: vec![],
        uncertain,
    });
}

/// SC7007: a rich-typed argument handed to a server-function reference while
/// nothing installs an argument serializer. Probed on the pinned
/// `@solidjs/web@2.0.0-rc.0` server-functions client: argument lists travel
/// as plain JSON by default, and a value `isJSONSafe` rejects — Date, Map,
/// Set, RegExp, typed arrays, cycles, class instances
/// (`server-functions/dist/client.js:141-170`, throw at `:395-401`) — makes
/// the call reject with "Server function arguments are sent as JSON by
/// default … Call enableRichArguments()". A lone `Uint8Array`/`ArrayBuffer`
/// (or one in trailing position) has a natural HTTP encoding and does not
/// throw (probed); `enableRichArguments()` or a configured `serializeArgs`
/// removes the throw entirely (probed).
fn server_function_rich_argument(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    // The serializer proof is a project-wide scan of every call. Only a client
    // call to a server function can consume it, and most projects have none,
    // so resolve it on first use rather than on entry.
    let mut serializer_proof = None;
    for file in &ctx.facts.files {
        // A call site inside server-side code never crosses the client
        // transport: in-process SSR and server-to-server calls run the
        // original function directly (RFC 10).
        if file
            .ast
            .module_directives
            .iter()
            .any(|directive| directive.value == USE_SERVER)
        {
            continue;
        }
        for call in &file.ast.calls {
            if call.arguments.is_empty() {
                continue;
            }
            let Some((declaration_file, function)) = ctx
                .semantic_lookup
                .function_called_at(file.path.as_str(), call.callee)
            else {
                continue;
            };
            let server_function = function.has_directive(USE_SERVER)
                || (declaration_file
                    .ast
                    .module_directives
                    .iter()
                    .any(|directive| directive.value == USE_SERVER)
                    && declaration_file.path != file.path);
            if !server_function {
                continue;
            }
            match ctx.semantic_lookup.resolved_callee_call(file, call.callee) {
                Some(resolved) if resolved.validity == ResolvedCallValidity::Valid => {}
                // Recovery and unresolved calls are TypeScript-owned. Emitting
                // a transport finding on the same invalid call would violate
                // the project's no-duplicate boundary.
                Some(_) => continue,
                None => {
                    push_rich_argument_uncertainty(
                        draft,
                        file,
                        call.callee,
                        "the compiler did not return the demanded call-validity fact",
                    );
                    continue;
                }
            }
            // A call whose own lexical scope is extracted to the server (an
            // enclosing "use server" function) is a direct server-side call.
            if file
                .ast
                .functions
                .iter()
                .any(|scope| scope.body.contains(call.span) && scope.has_directive(USE_SERVER))
            {
                continue;
            }
            for (index, argument) in call.arguments.iter().enumerate() {
                if argument.spread {
                    if *serializer_proof
                        .get_or_insert_with(|| project_rich_argument_serializer(ctx))
                        != SerializerProof::Enabled
                    {
                        push_rich_argument_uncertainty(
                            draft,
                            file,
                            argument.span,
                            "a spread hides the runtime argument values",
                        );
                    }
                    continue;
                }
                let Some(entity) = ctx
                    .semantic_lookup
                    .entity_at(file.path.as_str(), argument.span)
                else {
                    push_rich_argument_uncertainty(
                        draft,
                        file,
                        argument.span,
                        "the compiler did not return the demanded argument facts",
                    );
                    continue;
                };
                let matched = entity
                    .library_types
                    .as_deref()
                    .and_then(|types| rich_transport_member(types));
                let Some(matched) = matched else {
                    if argument_is_proven_json_safe(argument, entity) {
                        continue;
                    }
                    if *serializer_proof
                        .get_or_insert_with(|| project_rich_argument_serializer(ctx))
                        != SerializerProof::Enabled
                    {
                        push_rich_argument_uncertainty(
                            draft,
                            file,
                            argument.span,
                            if *serializer_proof
                                .get_or_insert_with(|| project_rich_argument_serializer(ctx))
                                == SerializerProof::Unresolved
                            {
                                "neither the argument's JSON safety nor the project's serializer configuration can be resolved exactly"
                            } else {
                                "the argument's complete JSON safety cannot be proven from the available type and literal facts"
                            },
                        );
                    }
                    continue;
                };
                // The descriptor is quoted in the message so the report names the
                // type the author wrote; the decision above never reads it. A
                // literal argument carries no descriptor demand, so the value
                // the author wrote stands in for it.
                let descriptor = entity.type_descriptor.as_ref().map_or_else(
                    || file.source_text(argument.span).unwrap_or("this argument"),
                    |descriptor| descriptor.text.as_ref(),
                );
                // Natural HTTP encodings: a lone Uint8Array/ArrayBuffer
                // argument — or one in trailing position after JSON-safe
                // leading arguments — is sent as a body, not as JSON
                // (probed), so those positions stay silent.
                if matched.natural_encoding
                    && (call.arguments.len() == 1 || index + 1 == call.arguments.len())
                {
                    continue;
                }
                match *serializer_proof.get_or_insert_with(|| project_rich_argument_serializer(ctx))
                {
                    SerializerProof::Enabled => return,
                    SerializerProof::Unresolved => {
                        push_rich_argument_uncertainty(
                            draft,
                            file,
                            argument.span,
                            "the project's server-function serializer configuration cannot be resolved exactly",
                        );
                        continue;
                    }
                    SerializerProof::Disabled => {}
                }
                let name = function
                    .name
                    .as_ref()
                    .and_then(|name| declaration_file.source_text(name.span))
                    .unwrap_or("this server function");
                draft.static_violations.push(StaticViolation {
                    id: "SC7007".into(),
                    rule: "server-function-rich-argument".into(),
                    message: format!(
                        "server function {name} receives an argument typed {} ({}); server-function arguments travel as plain JSON by default, and a value JSON cannot carry faithfully throws at the transport: \"Server function arguments are sent as JSON by default and these arguments are not JSON-serializable\"",
                        descriptor, matched.member
                    ),
                    hint: "Call enableRichArguments() from \"@solidjs/web/server-functions/rich-args\" once at client startup to send Dates, Maps, Sets, and typed arrays through the codec (~5 KB gz), or convert the argument to a JSON-safe shape at the call site (date.toISOString(), Array.from(set)).".into(),
                    location: location(file.path.shared(), argument.span),
                    analysis_context: String::new(),
                    fixes: vec![],
                    uncertain: false,
                });
            }
        }
    }
}

/// A deliberately small safe set, proved from the value the call actually
/// passes: `null`, a constant string, a finite constant number, and a written
/// primitive or nullish literal. Objects, arrays, aliases, unions, `any`, and
/// `unknown` are not guessed -- nested rich values, cycles, non-finite
/// numbers, and runtime type escapes remain possible, so their branch is
/// explicit `uncertifiable` unless a serializer is proven.
///
/// This deliberately does *not* consult `TypeDescriptor.text`. A rendered name
/// is not a fact about the value: `type Name = string` renders as `Name`, so a
/// text test certified `save(plain)` and raised an obligation on the identical
/// `save(aliased)`. Type Facts exposes no structural primitive-kind fact to
/// replace it with, so an unresolved declared type is now uniformly an
/// obligation rather than a spelling-dependent one. See
/// docs/precision-backlog.md for the ledger entry.
fn argument_is_proven_json_safe(
    argument: &solid_facts::ast::ArgumentFact,
    entity: &EntityFact,
) -> bool {
    if argument.value == ArgumentValueKind::Null {
        return true;
    }
    if let Some(value) = entity.constant_value.as_ref() {
        return match value.kind {
            ConstantValueKind::String => true,
            ConstantValueKind::Number => value.number.is_finite(),
        };
    }
    // A written primitive or nullish literal is JSON-safe, and the AST proves
    // the shape without any compiler fact. This is only reached once the
    // library identities have already ruled the value out of the rich
    // transport set, so a regexp literal never lands here.
    matches!(
        argument.runtime_value_kind,
        RuntimeValueKind::Primitive | RuntimeValueKind::Nullish
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SerializerProof {
    Enabled,
    Disabled,
    Unresolved,
}

fn push_rich_argument_uncertainty(
    draft: &mut ProgramDraft,
    file: &solid_facts::FileFacts,
    span: Span,
    reason: &str,
) {
    draft.static_violations.push(StaticViolation {
        id: "SC7007".into(),
        rule: "server-function-rich-argument".into(),
        message: format!(
            "a client call to a server function has an unresolved rich-argument transport proof: {reason}"
        ),
        hint: "Resolve the argument type and configure the serializer through the exact @solidjs/web server-functions API, or pass a JSON-safe value explicitly.".into(),
        location: location(file.path.shared(), span),
        analysis_context: "server-function-rich-argument-unresolved".into(),
        fixes: vec![],
        uncertain: true,
    });
}

/// A matched rich-transport type member.
struct RichMember {
    member: &'static str,
    /// Whether the value has a natural HTTP body encoding in the lone or
    /// trailing argument position (`Uint8Array`; `ArrayBuffer` is never in
    /// the flag set at all).
    natural_encoding: bool,
}

/// The documented constrained set — "Dates, Maps, Sets, typed arrays" (RFC
/// 10 and the rich-args entry's own docs) — plus `RegExp`, which the doc
/// covers as "etc." and the probe confirms throws. `ReadonlyMap`/
/// `ReadonlySet` are TypeScript views of the same runtime values.
///
/// The names come from `libraryTypes`, the compiler's own identities for the
/// standard-library types the argument's type is built from at its top level:
/// itself, its union and intersection members, and one array-element unwrap.
/// Nested object properties are not included. Their absence cannot certify the
/// object graph, so the caller retains an explicit uncertifiable obligation.
///
/// This replaced a walk over `TypeDescriptor.text` that split on top-level
/// `|`/`&` and matched the head of each member. Text could not answer it: an
/// alias renders as its own name, so `type Stamps = Date[]` matched nothing
/// whether declared here or imported; `Array<Date>` and `Date[]` are the same
/// runtime value but only the second matched; and a user-declared `Map` matched
/// the global. The name list stays here because which types are interesting —
/// and that a lone `Uint8Array` has a natural HTTP encoding — is this rule's
/// knowledge, not the compiler's.
fn rich_transport_member(library_types: &[std::sync::Arc<str>]) -> Option<RichMember> {
    library_types.iter().find_map(|name| {
        let matched = match name.as_ref() {
            "Date" => "Date",
            "Map" | "ReadonlyMap" => "Map",
            "Set" | "ReadonlySet" => "Set",
            "RegExp" => "RegExp",
            "Int8Array" | "Uint8ClampedArray" | "Int16Array" | "Uint16Array" | "Int32Array"
            | "Uint32Array" | "Float32Array" | "Float64Array" | "BigInt64Array"
            | "BigUint64Array" => "a typed array",
            "Uint8Array" => {
                return Some(RichMember {
                    member: "a typed array",
                    natural_encoding: true,
                });
            }
            _ => return None,
        };
        Some(RichMember {
            member: matched,
            natural_encoding: false,
        })
    })
}

/// Whether anything in the project installs an argument serializer: a value
/// import of the `rich-args` entry (whose one export is
/// `enableRichArguments`), or `configureServerFunctionsClient({ …,
/// serializeArgs })`. Either removes the transport throw everywhere (probed),
/// so the whole rule goes silent. Because that is a project-wide safety proof,
/// ambiguous configuration is kept separate from both enabled and disabled.
fn project_rich_argument_serializer(ctx: &AnalysisContext<'_>) -> SerializerProof {
    let mut unresolved = false;
    for file in &ctx.facts.files {
        let rich_args_import = file.ast.imports.iter().any(|import| {
            !import.type_only
                && import.module.as_str() == "@solidjs/web/server-functions/rich-args"
                && import.bindings.iter().any(|binding| !binding.type_only)
        });
        if rich_args_import {
            return SerializerProof::Enabled;
        }
        for call in &file.ast.calls {
            let exact_configure = ctx
                .semantic_lookup
                .resolved_callee_call(file, call.callee)
                .filter(|resolved| resolved.validity == typefacts::ResolvedCallValidity::Valid)
                .and_then(|resolved| resolved.declaration.as_ref())
                .is_some_and(|declaration| {
                    declaration.name.as_ref() == "configureServerFunctionsClient"
                        && matches!(
                            declaration.origin_module.as_ref(),
                            "@solidjs/web/server-functions"
                                | "@solidjs/web/server-functions/client"
                        )
                });
            if !exact_configure {
                continue;
            }
            let Some(options) = call.arguments.first() else {
                continue;
            };
            if options
                .property_names
                .iter()
                .any(|name| file.source_text(*name) == Some("serializeArgs"))
            {
                return SerializerProof::Enabled;
            }
            if !options.closed_object_literal {
                unresolved = true;
            }
        }
    }
    if unresolved {
        SerializerProof::Unresolved
    } else {
        SerializerProof::Disabled
    }
}

#[cfg(test)]
mod tests {
    use super::rich_transport_member;
    use std::sync::Arc;

    fn names(values: &[&str]) -> Vec<Arc<str>> {
        values.iter().map(|value| Arc::from(*value)).collect()
    }

    #[test]
    fn rich_member_matching_reads_library_type_names() {
        assert_eq!(
            rich_transport_member(&names(&["Date"])).unwrap().member,
            "Date"
        );
        assert_eq!(
            rich_transport_member(&names(&["Map"])).unwrap().member,
            "Map"
        );
        assert_eq!(
            rich_transport_member(&names(&["ReadonlySet"]))
                .unwrap()
                .member,
            "Set"
        );
        assert_eq!(
            rich_transport_member(&names(&["RegExp"])).unwrap().member,
            "RegExp"
        );
        // A union contributes every member, so an optional Date still matches.
        assert_eq!(
            rich_transport_member(&names(&["Date"])).unwrap().member,
            "Date"
        );
        // An array of Dates arrives as both names; either order matches Date and
        // `Array` is simply not in the set this rule cares about.
        assert_eq!(
            rich_transport_member(&names(&["Array", "Date"]))
                .unwrap()
                .member,
            "Date"
        );
        assert_eq!(
            rich_transport_member(&names(&["Float64Array"]))
                .unwrap()
                .member,
            "a typed array"
        );
        assert!(
            !rich_transport_member(&names(&["Float64Array"]))
                .unwrap()
                .natural_encoding
        );
        assert!(
            rich_transport_member(&names(&["Uint8Array"]))
                .unwrap()
                .natural_encoding
        );
        // Nothing interesting, and nothing at all. A user-declared `MapLike` is
        // not a library type and never reaches this list; the producer decides
        // that from the declaration file, not from the name.
        assert!(rich_transport_member(&names(&["Array"])).is_none());
        assert!(rich_transport_member(&names(&["String"])).is_none());
        assert!(rich_transport_member(&[]).is_none());
    }
}
