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
use solid_facts::ast::{ArgumentValueKind, ExportKind, FunctionKind};
use solid_facts::core::Span;

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
/// queue"). The finding is conditional — a boundary that settles *before*
/// the flush still applies its writes — hence warning severity.
///
/// Fires only in projects that provably server-render: on the client both
/// exports are unconditional no-ops wherever they are called, so a CSR-only
/// project has no post-flush drop to report.
fn http_response_after_flush(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    let mut server_renders = None;
    let mut loading_hosts: Option<HashSet<(&str, Span)>> = None;
    for file in &ctx.facts.files {
        let mut allowed = None;
        for call in &file.ast.calls {
            let Some(kind @ (Primitive::HttpStatus | Primitive::HttpHeader)) =
                primitive_name(
                    file.path.as_str(),
                    call.callee,
                    call.static_callee(&file.source),
                    ctx.entities,
                    ctx.symbol_names,
                    ctx.dialect,
                )
                .as_ref()
                .and_then(crate::PrimitiveName::primitive)
            else {
                continue;
            };
            if !*server_renders
                .get_or_insert_with(|| crate::source_discovery::project_server_renders(ctx.facts))
            {
                return;
            }
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
                message: format!(
                    "{name}() is called by content below a <Loading> boundary; under streaming SSR the response head commits at the shell flush, and when this boundary settles after the shell has flushed the call is a committed no-op — the {} is silently dropped, with no queue holding it for later",
                    if kind == Primitive::HttpStatus { "status" } else { "header" }
                ),
                hint: format!(
                    "Decide the response head in shell content — above every <Loading> boundary — or mark the async source this {name}() depends on with deferStream: true so the shell flush waits for it. If the boundary settles before the flush (fast data, renderToString) the write still applies, which is why this is a warning."
                ),
                location: location(file.path.shared(), call.callee),
                analysis_context: String::new(),
                fixes: vec![],
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
/// from`, `export * from`). Everything else — identifier aliases, plain
/// value exports, unresolvable shapes — routes to silence.
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
                    push_module_directive_violation(
                        draft,
                        file,
                        export.span,
                        "every re-exported binding",
                        "a re-export",
                    );
                }
                ExportKind::Named if export.module.is_some() => {
                    for specifier in export
                        .specifiers
                        .iter()
                        .filter(|specifier| !specifier.type_only)
                    {
                        push_module_directive_violation(
                            draft,
                            file,
                            specifier.local.span,
                            specifier.exported.as_str(),
                            "a re-export",
                        );
                    }
                }
                ExportKind::Named | ExportKind::Default => {
                    for specifier in export
                        .declarations
                        .iter()
                        .filter(|specifier| !specifier.type_only)
                    {
                        let Some(shape) = non_function_export_shape(file, specifier.local.span)
                        else {
                            continue;
                        };
                        push_module_directive_violation(
                            draft,
                            file,
                            specifier.local.span,
                            specifier.exported.as_str(),
                            shape,
                        );
                    }
                }
            }
        }
    }
}

/// Proves an exported declaration is *not* a direct function export, naming
/// the shape for the message; `None` keeps it silent (a direct function, or
/// a shape the analysis cannot prove either way).
fn non_function_export_shape(file: &FileFacts, local: Span) -> Option<&'static str> {
    // A function declaration exported by name, or a default-exported
    // function/class expression whose recorded span is the function itself.
    if file.ast.functions.iter().any(|function| {
        function.span == file.ast.peel_ts_sugar_span(local)
            || (function.kind == FunctionKind::Declaration
                && function.name.as_ref().is_some_and(|name| name.span == local))
    }) {
        return None;
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
            return None;
        }
        return binding
            .call_initializer
            .is_some()
            .then_some("a wrapped function");
    }
    // `export default <expression>`: the recorded local span is the
    // expression. A call is the proven wrapped form.
    let value = file.ast.peel_ts_sugar_span(local);
    file.ast
        .calls
        .iter()
        .any(|call| call.span == value)
        .then_some("a wrapped function")
}

fn push_module_directive_violation(
    draft: &mut ProgramDraft,
    file: &FileFacts,
    span: Span,
    exported: &str,
    shape: &str,
) {
    draft.static_violations.push(StaticViolation {
        id: "SC7006".into(),
        rule: "server-function-module-directive".into(),
        message: format!(
            "this module's top-level \"use server\" directive extracts every export to the server build, but export {exported} is {shape}, not a direct function declaration; the compiler turns only direct function exports into client references, so this export is silently dropped from the client build"
        ),
        hint: "Move the \"use server\" directive into each function body and keep the wrapper at the export site — export const getData = GET(async (id) => { \"use server\"; ... }) round-trips the wrapper call — or export plain functions from this module and wrap them where they are imported.".into(),
        location: location(file.path.shared(), span),
        analysis_context: String::new(),
        fixes: vec![],
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
    let mut rich_arguments_enabled = None;
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
                // Only identifier arguments carry demanded type facts; an
                // inline expression is unresolvable here and an unproven rich
                // type is not a proven throw — silence, not uncertifiable.
                if argument.spread || argument.value != ArgumentValueKind::Identifier {
                    continue;
                }
                let Some(descriptor) = ctx
                    .semantic_lookup
                    .entity_at(file.path.as_str(), argument.span)
                    .and_then(|entity| entity.type_descriptor.as_ref())
                else {
                    continue;
                };
                let Some(matched) = rich_transport_member(&descriptor.text) else {
                    continue;
                };
                // Natural HTTP encodings: a lone Uint8Array/ArrayBuffer
                // argument — or one in trailing position after JSON-safe
                // leading arguments — is sent as a body, not as JSON
                // (probed), so those positions stay silent.
                if matched.natural_encoding
                    && (call.arguments.len() == 1 || index + 1 == call.arguments.len())
                {
                    continue;
                }
                if *rich_arguments_enabled
                    .get_or_insert_with(|| project_enables_rich_arguments(ctx))
                {
                    return;
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
                        descriptor.text, matched.member
                    ),
                    hint: "Call enableRichArguments() from \"@solidjs/web/server-functions/rich-args\" once at client startup to send Dates, Maps, Sets, and typed arrays through the codec (~5 KB gz), or convert the argument to a JSON-safe shape at the call site (date.toISOString(), Array.from(set)).".into(),
                    location: location(file.path.shared(), argument.span),
                    analysis_context: String::new(),
                    fixes: vec![],
                });
            }
        }
    }
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
/// `ReadonlySet` are TypeScript views of the same runtime values. Matching
/// is against the argument's *top-level* type text: union/intersection
/// members and `[]` array suffixes are walked, nested object properties are
/// not (an unproven rich type is not a proven throw).
fn rich_transport_member(text: &str) -> Option<RichMember> {
    top_level_members(text).find_map(|member| {
        let member = member.trim();
        let member = member.strip_suffix("[]").unwrap_or(member).trim();
        let head = member
            .split_once('<')
            .map_or(member, |(head, _)| head)
            .trim();
        let matched = match head {
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

/// Splits a type text on top-level `|` and `&`, ignoring separators nested
/// inside `<>`, `()`, `[]`, or `{}`.
fn top_level_members(text: &str) -> impl Iterator<Item = &str> {
    let mut members = Vec::new();
    let mut depth = 0_i32;
    let mut start = 0;
    for (offset, byte) in text.bytes().enumerate() {
        match byte {
            b'<' | b'(' | b'[' | b'{' => depth += 1,
            b'>' | b')' | b']' | b'}' => depth -= 1,
            b'|' | b'&' if depth == 0 => {
                members.push(&text[start..offset]);
                start = offset + 1;
            }
            _ => {}
        }
    }
    members.push(&text[start..]);
    members.into_iter()
}

/// Whether anything in the project installs an argument serializer: a value
/// import of the `rich-args` entry (whose one export is
/// `enableRichArguments`), or `configureServerFunctionsClient({ …,
/// serializeArgs })`. Either removes the transport throw everywhere
/// (probed), so the whole rule goes silent — over-approximation here only
/// ever silences.
fn project_enables_rich_arguments(ctx: &AnalysisContext<'_>) -> bool {
    ctx.facts.files.iter().any(|file| {
        let rich_args_import = file.ast.imports.iter().any(|import| {
            !import.type_only
                && import.module.as_str() == "@solidjs/web/server-functions/rich-args"
                && import
                    .bindings
                    .iter()
                    .any(|binding| !binding.type_only)
        });
        if rich_args_import {
            return true;
        }
        let imports_configure = file.ast.imports.iter().any(|import| {
            import.module.as_str().starts_with("@solidjs/web")
                && import.bindings.iter().any(|binding| {
                    !binding.type_only
                        && binding
                            .imported
                            .as_deref()
                            .or_else(|| file.source_text(binding.local.span))
                            == Some("configureServerFunctionsClient")
                })
        });
        imports_configure
            && file.ast.calls.iter().any(|call| {
                call.arguments.iter().any(|argument| {
                    argument.property_names.iter().any(|name| {
                        file.source_text(*name) == Some("serializeArgs")
                    })
                })
            })
    })
}

#[cfg(test)]
mod tests {
    use super::{rich_transport_member, top_level_members};

    #[test]
    fn rich_member_matching_walks_top_level_members_only() {
        assert_eq!(rich_transport_member("Date").unwrap().member, "Date");
        assert_eq!(
            rich_transport_member("Map<string, number>").unwrap().member,
            "Map"
        );
        assert_eq!(
            rich_transport_member("ReadonlySet<string>").unwrap().member,
            "Set"
        );
        assert_eq!(rich_transport_member("RegExp").unwrap().member, "RegExp");
        assert_eq!(
            rich_transport_member("Date | undefined").unwrap().member,
            "Date"
        );
        assert_eq!(rich_transport_member("Date[]").unwrap().member, "Date");
        assert_eq!(
            rich_transport_member("Float64Array").unwrap().member,
            "a typed array"
        );
        assert!(
            !rich_transport_member("Float64Array")
                .unwrap()
                .natural_encoding
        );
        assert!(
            rich_transport_member("Uint8Array<ArrayBufferLike>")
                .unwrap()
                .natural_encoding
        );
        // Nested rich types are not top-level and stay silent.
        assert!(rich_transport_member("{ when: Date }").is_none());
        assert!(rich_transport_member("Array<Date>").is_none());
        assert!(rich_transport_member("string").is_none());
        assert!(rich_transport_member("MapLike").is_none());
        assert!(rich_transport_member("(a: Date) => void").is_none());
    }

    #[test]
    fn top_level_split_respects_nesting() {
        assert_eq!(
            top_level_members("Map<string | number, Set<Date>> | null").collect::<Vec<_>>(),
            ["Map<string | number, Set<Date>> ", " null"]
        );
    }
}
