//! ADR 1090 (G1 residual): the promise of the function a read-after-await site is written in is never consumed.
//!
//! rc.21's dev build raises UNTRACKED_READ_AFTER_AWAIT only for a read whose V8 async stack holds the frame `__solidAsyncCompute_<id>`
//! (`@solidjs/signals@2.0.0-rc.21` `dist/dev-shared.js` `watchAsyncTail`, `attributedFlight`): the async function that awaits an async
//! computation's returned promise. V8's async frames are the chain of code awaiting the current promise; a promise nothing awaits, returns or
//! hands on ends that chain at the function itself, so no read in its continuation can carry the frame. This module proves that by exact facts
//! only, for a read in a named async function (a declaration or a `const` binding) when all of these hold:
//!
//! - the function is `async` in code, not a generator, a method or a named function expression, and its value is current;
//! - the site defers nothing: a direct read of an exact accessor, or a setter call whose arguments are all inert. A project call may return a
//!   class or a function whose code reads later, wherever it is run (the third review, H1); and the site is in no JSX expression, which
//!   compiles to a getter read wherever the prop or child is read (H3);
//! - it is written in a module with module syntax esbuild keeps (`runtime_module_syntax`). A classic script's top-level function is a global
//!   another file may call as `window.f()`, and a file whose only module syntax esbuild erases is one (the second review, G2; the third, H2);
//! - no class body lies between the function and the read: a field initializer, an accessor, a constructor or a static block is code of the
//!   class, run wherever `new` (or `Reflect.construct`) runs, not the function's own straight-line code (the second review, G1);
//! - it is not exported, unseen code does not enter it, and no other file references or calls it;
//! - the program has no direct `eval` and no code-evaluating call (code a string holds may call the function where its promise is kept);
//! - it has at least one reference, and every one is a direct call whose result is discarded (`f();` as a statement);
//! - the read is past the prefix: an await of the same function on every run (`AstFacts::unconditional_awaits`) ends before it. A read that
//!   may run before the first suspension runs on the caller's stack, inside whatever computation made the call (review F2 of be4990c64).
//!
//! Anything else is not proven: an export, a returned, awaited, stored or handed-on call, a function literal. The `Promise.prototype.then`
//! handler case of be4990c64 was dropped (review F1): a receiver is a native promise only when no `Promise` subclass or patch can stand in,
//! and no exact fact says so.

use solid_facts::FileFacts;
use solid_facts::core::Span;

use crate::local_access::HandOff;

/// Whether the promise of the function that holds `at` is never consumed (see the module documentation).
pub(super) fn holder_promise_unconsumed(
    ctx: &crate::pipeline::AnalysisContext<'_>,
    file: &FileFacts,
    at: Span,
    via: &str,
    call: Option<&solid_facts::ast::CallFact>,
    object_prototype_touched: &dyn Fn() -> bool,
) -> bool {
    let lookup = ctx.semantic_lookup;
    // The third review of 1090 G1 (H1): a site that may hand code on (a project call whose result is a class or a function, a construct, an
    // unresolved call) defers its reads past the function's continuation. Only a site that can defer nothing is discharged: a direct read of an
    // exact accessor (`read`), or a setter call every argument of which is inert (no function, no value that may hold one).
    let defers_nothing = match via {
        "read" => true,
        "setter" => call.is_some_and(|call| {
            call.arguments.iter().all(|argument| {
                super::library_call::argument_is_inert(
                    ctx,
                    file,
                    argument,
                    None,
                    object_prototype_touched,
                )
            })
        }),
        _ => false,
    };
    // H3: a site in a JSX expression compiles to a getter read wherever the prop or child is read.
    if !defers_nothing
        || file.ast.any_jsx_containing(at)
        || file
            .ast
            .jsx_fragments
            .iter()
            .any(|fragment| fragment.contains(at))
    {
        return false;
    }
    let Some(function) = crate::owners::owning_function(&file.ast, at) else {
        return false;
    };
    if function.generator || function.method_name.is_some() {
        return false;
    }
    let ast = &file.ast;
    let named =
        function.name.is_some() || crate::owners::function_binding_name(file, function).is_some();
    if !named || !function.r#async {
        return false;
    }
    if function.kind == solid_facts::ast::FunctionKind::Expression && function.name.is_some() {
        return false;
    }
    if !runtime_module_syntax(file)
        || file
            .ast
            .classes
            .iter()
            .any(|class| function.span.contains(class.span) && class.span.contains(at))
        || lookup.hazard_present(crate::base::Hazard::Eval)
        || lookup.hazard_present(crate::base::Hazard::CodeEvaluatingCall)
        || !past_prefix(file, function.span, at)
        || !lookup.function_value_is_current(file, function)
        || crate::execution_role::entered_by_unseen_code(file, function, lookup)
        || !crate::local_access::foreign_reference_hand_offs(file, function, lookup).is_empty()
        || lookup
            .function_call_sites(file.path.as_str(), function.span)
            .iter()
            .any(|(caller, _)| caller.path != file.path)
    {
        return false;
    }
    // An export a host may call (a framework's loader, run inside a computation) is never proven, referenced or not; and a function with no reference at all is no
    // discarded call (`all` of nothing proves nothing).
    let exported = crate::owners::function_binding_name(file, function)
        .is_some_and(|name| crate::indexes::binding_exported(file, name.span))
        || ast.exports_containing(function.span).next().is_some();
    let Some(references) =
        crate::local_access::function_reference_hand_offs(file, function, lookup)
    else {
        return false;
    };
    if exported || references.is_empty() {
        return false;
    }
    references.into_iter().all(|(reference, hand_off)| {
        matches!(hand_off, HandOff::Call)
            && ast
                .calls
                .iter()
                .find(|call| ast.peel_ts_sugar_span(call.callee) == reference)
                .is_some_and(|call| !call.construct && call.result_discarded)
    })
}

/// The file has module syntax esbuild keeps (the third review of 1090 G1, H2), any one of:
/// - a value import binding a runtime call uses: the callee of a call, or the object of a callee member (`createSignal(..)`, `S.createMemo(..)`);
///   an ambient context holds no call, so a reference only in a `declare` does not count;
/// - a side-effect import written `import "x"` (not `import {} from "x"`, which is erased);
/// - `export * from "x"` (or `export * as n from "x"`), not type-only;
/// - an export statement of a runtime declaration: `export const|let|var|function|async function|class|abstract class`;
/// - an `export default` that is not an interface.
///
/// `export {}`, `export {} from "x"`, `export { a }`, `export type`, `export interface`, `export namespace`, `export declare` and
/// type-only imports are not counted: each is erased, or may be, and the file may then load as a classic script.
fn runtime_module_syntax(file: &FileFacts) -> bool {
    let ast = &file.ast;
    let source = |span: Span| file.source_text(span).unwrap_or_default();
    let runtime_call = |binding: Span| {
        ast.identifiers.iter().any(|identifier| {
            identifier.role == solid_facts::ast::IdentifierRole::Reference
                && ast.reference_declaration(identifier.span) == Some(binding)
                && ast.calls.iter().any(|call| {
                    let callee = ast.peel_ts_sugar_span(call.callee);
                    callee == identifier.span
                        || ast.member_at(callee).is_some_and(|member| {
                            ast.peel_ts_sugar_span(member.object) == identifier.span
                        })
                })
        })
    };
    let imported = ast.imports.iter().any(|import| {
        if import.type_only {
            return false;
        }
        import.bindings.iter().any(|binding| match binding.kind {
            solid_facts::ast::ImportKind::SideEffect => {
                import.specifier_span.is_some_and(|specifier| {
                    file.source_text(Span::new(import.span.start, specifier.start))
                        .is_some_and(|head| !head.contains('{'))
                })
            }
            _ => {
                !binding.type_only && binding.runtime_referenced && runtime_call(binding.local.span)
            }
        })
    });
    imported
        || ast.module_level_exports().any(|export| {
            if export.type_only {
                return false;
            }
            let text = source(export.span).trim_start();
            let rest = text.strip_prefix("export").unwrap_or_default().trim_start();
            match export.kind {
                solid_facts::ast::ExportKind::All => true,
                solid_facts::ast::ExportKind::Default => !rest
                    .strip_prefix("default")
                    .unwrap_or_default()
                    .trim_start()
                    .starts_with("interface"),
                solid_facts::ast::ExportKind::Named => {
                    !export.declarations.is_empty()
                        && [
                            "const ",
                            "let ",
                            "var ",
                            "function",
                            "async function",
                            "class ",
                            "abstract class ",
                        ]
                        .iter()
                        .any(|head| rest.starts_with(head))
                }
            }
        })
}

/// An await of the function at `function` that runs on every run of it ends before `at`: the code at `at` runs only after the function suspended.
fn past_prefix(file: &FileFacts, function: Span, at: Span) -> bool {
    let ast = &file.ast;
    ast.unconditional_awaits.iter().any(|await_span| {
        await_span.end <= at.start
            && function.contains(*await_span)
            && crate::owners::owning_function(ast, *await_span)
                .is_some_and(|owner| owner.span == function)
    })
}

#[cfg(test)]
mod tests {
    use super::runtime_module_syntax;
    use solid_facts::FileFacts;
    use solid_facts::compiler::ExecutionMap;
    use solid_facts::core::{Generation, SourceHash, SourcePath};
    use std::sync::Arc;

    fn file(source: &str) -> FileFacts {
        let ast = solid_facts::ast::extract("unconsumed.ts", source).unwrap();
        FileFacts {
            generation: Generation::new(1).unwrap(),
            path: SourcePath::new("unconsumed.ts").unwrap(),
            source_hash: SourceHash::of(source),
            source: Arc::from(source),
            ast: Arc::new(ast),
            compiler: Arc::new(ExecutionMap {
                compiler_facts_protocol: solid_facts::compiler::COMPILER_FACTS_PROTOCOL,
                source_hash: SourceHash::of(source),
                semantic_model: Default::default(),
                tracked_regions: vec![],
                untracked_regions: vec![],
                discarded_regions: vec![],
                ownership_regions: vec![],
                callback_roles: vec![],
                jsx_operations: vec![],
            }),
        }
    }

    /// The second review of 1090 G1, G2: a file whose only module syntax a stripping transpiler erases may load as a classic script.
    #[test]
    fn only_module_syntax_a_stripping_transpiler_keeps_makes_a_module() {
        for source in [
            "import { f } from \"m\"; f();",
            "import \"m\";",
            "import * as n from \"m\"; n.f();",
            "export const x = 1;",
            "export function g() {}",
            "export class K {}",
            "export default 1;",
            "export * from \"m\";",
        ] {
            assert!(runtime_module_syntax(&file(source)), "{source}");
        }
        for source in [
            "const y = 1;",
            "import type { T } from \"m\"; let v: T;",
            "import { type T } from \"m\"; let v: T;",
            "import { T } from \"m\"; let v: T;",
            "import { X } from \"m\"; declare const y: typeof X;",
            "import {} from \"m\";",
            "export {};",
            "export {} from \"m\";",
            "const y = 1; export { y };",
            "export type T = number;",
            "export interface I { a: number }",
            "export default interface I { a: number }",
            "export namespace N { export type T = 1; }",
            "export declare const z: number;",
            "type U = 1; export type { U };",
            "export type * from \"m\";",
        ] {
            assert!(!runtime_module_syntax(&file(source)), "{source}");
        }
    }
}
