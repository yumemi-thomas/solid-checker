//! Cleanup-return and leaf-owner diagnostics.
//!
//! Detects `onCleanup`/leaf-owner misuse and validates the values returned to
//! cleanup-accepting owners. The owner-analysis subsystem asks this module
//! whether a callback returns a cleanup (`function_returns_cleanup`); the
//! pipeline's leaf-and-cleanup stage drives the other two entry points.

use std::collections::HashMap;

use solid_dialect::{CleanupRule, Primitive};
use solid_facts::FileFacts;
use solid_facts::core::Span;
use typefacts::{Callability, Location, ResolvedCallValidity};

use super::{
    Fix, InvalidCleanupReturn, LeafOwnerOperation, PrimitiveName, SemanticLookup, SymbolId,
    TextEdit, UnresolvedCleanupReturn, location, primitive_name,
};
use crate::owners::{callback_owner_at_call, containing_ast_function};
use crate::pipeline::{AnalysisContext, ProgramDraft, parallel_file_results};

/// Runs the project-level leaf-owner and cleanup-return stage.
pub(crate) fn collect_project(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    draft.leaf_operations.extend(
        parallel_file_results(&ctx.facts.files, |file| {
            leaf_owner_operations_for_file(file, ctx.symbol_names, ctx.semantic_lookup)
        })
        .into_iter()
        .flatten(),
    );
    for (invalid, unresolved) in parallel_file_results(&ctx.facts.files, |file| {
        cleanup_returns_for_file(ctx.semantic_lookup, file, ctx.symbol_names)
    }) {
        draft.invalid_cleanup_returns.extend(invalid);
        draft.unresolved_cleanup_returns.extend(unresolved);
    }
}

pub(super) fn leaf_owner_operations_for_file(
    file: &FileFacts,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> Vec<LeafOwnerOperation> {
    let entities = lookup.entities();
    let dialect = lookup.dialect;
    let mut operations = Vec::new();
    let function_spans = file
        .ast
        .functions
        .iter()
        .map(|function| function.span)
        .collect::<Vec<_>>();
    for owner_call in &file.ast.calls {
        let owner = primitive_name(
            file.path.as_str(),
            owner_call.callee,
            owner_call.static_callee(&file.source),
            entities,
            symbol_names,
            dialect,
        );
        let Some(owner) = owner.as_ref() else {
            continue;
        };
        // The leaf owners: an owner whose callback is the end of the ownership
        // chain, so anything created inside it never gets disposed.
        //
        // Asked of the dialect rather than matched here. The pair this used to
        // hardcode -- `onSettled` and `createTrackedEffect` -- is 2.0-only, so
        // under 1.x these rules could not fire at all. Taking the argument
        // index from the same answer also drops the assumption that the
        // callback is always first.
        let Some(region) = owner
            .primitive()
            .into_iter()
            .flat_map(|primitive| {
                (0..owner_call.arguments.len()).filter(move |index| {
                    callback_owner_at_call(file, owner_call, primitive, *index, lookup)
                        == Some(solid_dialect::CallbackOwner::Leaf)
                })
            })
            .next()
            .and_then(|index| owner_call.arguments.get(index))
            .map(|argument| argument.span)
        else {
            continue;
        };
        for call in &file.ast.calls {
            if call.span == owner_call.span || !region.contains(call.span) {
                continue;
            }
            let primitive = primitive_name(
                file.path.as_str(),
                call.callee,
                call.static_callee(&file.source),
                entities,
                symbol_names,
                dialect,
            );
            let Some(primitive) = primitive else {
                continue;
            };
            let rule = primitive
                .primitive()
                .map(|primitive| dialect.cleanup_rule(primitive));
            let forbidden = match rule {
                Some(CleanupRule::Always) => true,
                // `createSignal(fn)` registers work; `createSignal(0)` does
                // not. Flattening this arm into the unconditional one would
                // turn every plainly seeded signal under a leaf owner into a
                // false positive.
                Some(CleanupRule::WhenFirstArgumentIsFunction) => {
                    call.arguments.first().is_some_and(|argument| {
                        function_spans
                            .iter()
                            .any(|function| argument.span.contains(*function))
                    })
                }
                Some(CleanupRule::Never) | None => false,
            };
            if forbidden {
                // Only `onCleanup` has a rewrite -- return the cleanup
                // instead of registering it -- and only where the owner reads
                // a returned function as cleanup. That is a 2.0 idea: 1.x's
                // leaf owner threads return values elsewhere, so offering the
                // rewrite there would introduce a bug, not fix one.
                let fix = (primitive.primitive() == Some(Primitive::OnCleanup)
                    && owner
                        .primitive()
                        .is_some_and(|owner| dialect.accepts_cleanup_return(owner)))
                .then(|| terminal_cleanup_fix(file, region, call))
                .flatten();
                operations.push(LeafOwnerOperation {
                    primitive: primitive.to_string(),
                    owner: owner.to_string(),
                    location: location(file.path.shared(), call.callee),
                    fix,
                });
            }
        }
    }
    operations
}

pub(super) fn cleanup_returns_for_file<'a, 'f>(
    lookup: &SemanticLookup<'a>,
    file: &'f FileFacts,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> (Vec<InvalidCleanupReturn>, Vec<UnresolvedCleanupReturn>)
where
    'a: 'f,
{
    let entities = lookup.entities();
    let mut invalid = Vec::new();
    let mut unresolved = Vec::new();
    for call in &file.ast.calls {
        let dialect = lookup.dialect;
        let primitive = primitive_name(
            file.path.as_str(),
            call.callee,
            call.static_callee(&file.source),
            entities,
            symbol_names,
            dialect,
        );
        // Returning a cleanup is a per-dialect idea: 1.x threads an effect's
        // return value to the next run as `prev`, so nothing there reads a
        // returned function as cleanup and this loop never starts.
        let Some(kind) = primitive.as_ref().and_then(PrimitiveName::primitive) else {
            continue;
        };
        if !dialect.accepts_cleanup_return(kind) {
            continue;
        }
        let [callback_index] = dialect.callback_positions(kind) else {
            continue;
        };
        let Some(argument) = call.arguments.get(*callback_index) else {
            continue;
        };
        let Some((callback_file, callback)) = callback_function(lookup, file, argument.span) else {
            unresolved.push(UnresolvedCleanupReturn {
                primitive: primitive
                    .expect("matched cleanup-return primitive")
                    .to_string(),
                location: location(file.path.shared(), argument.span),
            });
            continue;
        };
        let primitive = primitive.expect("matched cleanup-return primitive");
        if callback.r#async {
            invalid.push(InvalidCleanupReturn {
                primitive: primitive.to_string(),
                location: location(callback_file.path.shared(), callback.span),
            });
            continue;
        }
        let returns =
            callback
                .expression_return
                .iter()
                .chain(callback_file.ast.returns.iter().filter(|returned| {
                    containing_ast_function(&callback_file.ast, returned.span)
                        .is_some_and(|function| function.span == callback.span)
                }));
        for returned in returns {
            match cleanup_return_status(lookup, callback_file, returned) {
                CleanupReturnStatus::Valid => {}
                CleanupReturnStatus::Invalid => {
                    invalid.push(InvalidCleanupReturn {
                        primitive: primitive.to_string(),
                        location: expand_parenthesized_location(
                            callback_file,
                            returned.argument.unwrap_or(returned.span),
                        ),
                    });
                }
                CleanupReturnStatus::Unresolved => {
                    unresolved.push(UnresolvedCleanupReturn {
                        primitive: primitive.to_string(),
                        location: location(
                            callback_file.path.as_str(),
                            returned.argument.unwrap_or(returned.span),
                        ),
                    });
                }
            }
        }
    }
    (invalid, unresolved)
}

fn callback_function<'a, 'f>(
    lookup: &SemanticLookup<'a>,
    call_file: &'f solid_facts::FileFacts,
    argument: Span,
) -> Option<(
    &'f solid_facts::FileFacts,
    &'f solid_facts::ast::FunctionFact,
)>
where
    'a: 'f,
{
    if let Some(function) = call_file
        .ast
        .functions
        .iter()
        .filter(|function| argument.contains(function.span))
        .max_by_key(|function| function.span.end - function.span.start)
    {
        return Some((call_file, function));
    }
    let symbol = lookup.entities().at(call_file.path.as_str(), argument)?;
    lookup.function_for_symbol(symbol)
}

enum CleanupReturnStatus {
    Valid,
    Invalid,
    Unresolved,
}

fn expand_parenthesized_location(file: &solid_facts::FileFacts, span: Span) -> Location {
    let mut start = usize::try_from(span.start).unwrap_or(0);
    let mut end = usize::try_from(span.end).unwrap_or(file.source.len());
    while start > 0
        && end < file.source.len()
        && file.source.as_bytes()[start - 1] == b'('
        && file.source.as_bytes()[end] == b')'
    {
        start -= 1;
        end += 1;
    }
    location(
        file.path.as_str(),
        Span::new(
            u32::try_from(start).unwrap_or(span.start),
            u32::try_from(end).unwrap_or(span.end),
        ),
    )
}

fn terminal_cleanup_fix(
    file: &solid_facts::FileFacts,
    owner_region: Span,
    call: &solid_facts::ast::CallFact,
) -> Option<Fix> {
    let callback = file
        .ast
        .functions
        .iter()
        .filter(|function| owner_region.contains(function.span))
        .max_by_key(|function| function.span.end - function.span.start)?;
    let body_end = usize::try_from(callback.body.end).ok()?.checked_sub(1)?;
    let call_end = usize::try_from(call.span.end).ok()?;
    if call_end > body_end || body_end > file.source.len() {
        return None;
    }
    if !file.source.as_bytes()[call_end..body_end]
        .iter()
        .all(|byte| byte.is_ascii_whitespace() || *byte == b';')
    {
        return None;
    }
    let [argument] = call.arguments.as_slice() else {
        return None;
    };
    let start = usize::try_from(argument.span.start).ok()?;
    let end = usize::try_from(argument.span.end).ok()?;
    let argument = file.source.get(start..end)?.trim();
    if argument.is_empty() {
        return None;
    }
    Some(Fix {
        message: "Return the cleanup function instead of calling onCleanup".into(),
        applicability: "safe".into(),
        edits: vec![TextEdit {
            location: location(file.path.shared(), call.span),
            new_text: format!("return {argument}"),
        }],
    })
}

fn cleanup_return_status(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    returned: &solid_facts::ast::ReturnFact,
) -> CleanupReturnStatus {
    let entities = lookup.entities();
    match returned.value {
        solid_facts::ast::ReturnValueKind::Undefined
        | solid_facts::ast::ReturnValueKind::Function => CleanupReturnStatus::Valid,
        solid_facts::ast::ReturnValueKind::Member => CleanupReturnStatus::Unresolved,
        solid_facts::ast::ReturnValueKind::Other => CleanupReturnStatus::Invalid,
        solid_facts::ast::ReturnValueKind::Call => {
            let Some(callee) = returned.callee else {
                return CleanupReturnStatus::Unresolved;
            };
            let resolved = lookup
                .entity_at(file.path.as_str(), callee)
                .and_then(|entity| entity.resolved_call.as_ref())
                .is_some_and(|call| call.validity == ResolvedCallValidity::Valid);
            if !resolved {
                return CleanupReturnStatus::Unresolved;
            }
            match returned_call_callability(lookup, file, callee) {
                Some(Callability::Callable) => CleanupReturnStatus::Valid,
                // TypeFacts does not yet distinguish voidish from other
                // non-callable return types. Refuse to infer that distinction
                // from rendered type text.
                Some(Callability::NonCallable | Callability::Mixed | Callability::Unknown)
                | None => CleanupReturnStatus::Unresolved,
            }
        }
        solid_facts::ast::ReturnValueKind::Identifier => {
            let Some(symbol) = entities.get(&location(file.path.shared(), returned.span)) else {
                return CleanupReturnStatus::Unresolved;
            };
            let function = file.ast.functions.iter().any(|function| {
                function.name.as_ref().is_some_and(|name| {
                    entities.get(&location(file.path.shared(), name.span)) == Some(symbol)
                })
            }) || file.ast.bindings.iter().any(|binding| {
                binding.initializer_function
                    && binding.names.iter().any(|name| {
                        entities.get(&location(file.path.shared(), name.span)) == Some(symbol)
                    })
            });
            if function {
                CleanupReturnStatus::Valid
            } else {
                CleanupReturnStatus::Unresolved
            }
        }
    }
}

pub(super) fn function_returns_cleanup(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    function
        .expression_return
        .iter()
        .chain(file.ast.returns.iter().filter(|returned| {
            containing_ast_function(&file.ast, returned.span)
                .is_some_and(|owner| owner.span == function.span)
        }))
        .any(|returned| cleanup_return_is_function(lookup, file, returned))
}

fn cleanup_return_is_function(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    returned: &solid_facts::ast::ReturnFact,
) -> bool {
    match returned.value {
        solid_facts::ast::ReturnValueKind::Function => true,
        solid_facts::ast::ReturnValueKind::Identifier => {
            matches!(
                cleanup_return_status(lookup, file, returned),
                CleanupReturnStatus::Valid
            )
        }
        solid_facts::ast::ReturnValueKind::Call => {
            let Some(callee) = returned.callee else {
                return false;
            };
            let valid_call = lookup
                .entity_at(file.path.as_str(), callee)
                .and_then(|entity| entity.resolved_call.as_ref())
                .is_some_and(|call| call.validity == ResolvedCallValidity::Valid);
            valid_call
                && returned_call_callability(lookup, file, callee) == Some(Callability::Callable)
        }
        solid_facts::ast::ReturnValueKind::Undefined
        | solid_facts::ast::ReturnValueKind::Member
        | solid_facts::ast::ReturnValueKind::Other => false,
    }
}

fn returned_call_callability(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    callee: Span,
) -> Option<Callability> {
    let call = lookup.call_by_callee(file, callee)?;
    lookup
        .entity_at(file.path.as_str(), call.span)
        .and_then(|entity| entity.callability)
}
