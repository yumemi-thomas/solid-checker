//! Conservative identity invalidation for mutable consumer bindings.
use crate::{identity::SymbolId, indexes::EntitySymbols};
use solid_facts::{FileFacts, ast::IdentifierRole, core::Span};
use std::collections::HashSet;

fn symbol_at<'a>(
    file: &FileFacts,
    entities: &'a EntitySymbols,
    span: Span,
) -> Option<&'a SymbolId> {
    file.ast
        .reference_declarations
        .iter()
        .find(|(reference, _)| *reference == span)
        .and_then(|(_, declaration)| entities.at(file.path.as_str(), *declaration))
        .or_else(|| entities.at(file.path.as_str(), span))
}

pub(crate) fn binding_has_write(
    file: &FileFacts,
    entities: &EntitySymbols,
    symbol: &SymbolId,
) -> bool {
    file.ast.assignments.iter().any(|assignment| {
        symbol_at(
            file,
            entities,
            file.ast.peel_ts_sugar_span(assignment.target),
        ) == Some(symbol)
    })
}

/// Every member identity is withheld after a write through any exact alias,
/// deletion, or escape. This is whole-file conservative, not a flow analysis.
pub(crate) fn structural_binding_is_stable(
    file: &FileFacts,
    entities: &EntitySymbols,
    root: &SymbolId,
    shape: &crate::ContractReturn,
) -> bool {
    let mut aliases = HashSet::from([root.clone()]);
    loop {
        let before = aliases.len();
        for binding in &file.ast.bindings {
            if binding.shape != solid_facts::ast::BindingShape::Identifier
                || binding.names.len() != 1
            {
                continue;
            }
            let Some(initializer) = &binding.initializer_identifier else {
                continue;
            };
            if symbol_at(file, entities, initializer.span)
                .is_some_and(|symbol| aliases.contains(symbol))
                && let Some(symbol) = entities.at(file.path.as_str(), binding.names[0].span)
            {
                aliases.insert(symbol.clone());
            }
        }
        if aliases.len() == before {
            break;
        }
    }
    for identifier in file
        .ast
        .identifiers
        .iter()
        .filter(|id| id.role == IdentifierRole::Reference)
    {
        if !symbol_at(file, entities, identifier.span)
            .is_some_and(|symbol| aliases.contains(symbol))
        {
            continue;
        }
        let at = identifier.span;
        // A root embedded in another expression can acquire an unenumerated
        // alias (arrays, conditionals, new expressions, closures, etc.). Only
        // direct aliases and direct member receivers are represented here.
        let direct_alias = file.ast.bindings.iter().any(|binding| {
            binding.shape == solid_facts::ast::BindingShape::Identifier
                && binding.names.len() == 1
                && binding
                    .initializer_identifier
                    .as_ref()
                    .is_some_and(|value| value.span == at)
        });
        let direct_receiver = file.ast.members.iter().any(|member| member.object == at);
        if !direct_alias && !direct_receiver {
            return false;
        }
        if file
            .ast
            .assignments
            .iter()
            .any(|a| a.target.contains(at) || a.value_span.contains(at))
            || file
                .ast
                .deleted_targets
                .iter()
                .any(|target| target.contains(at))
            || file
                .ast
                .calls
                .iter()
                .any(|call| call.arguments.iter().any(|arg| arg.span.contains(at)))
            || file
                .ast
                .spreads
                .iter()
                .any(|spread| spread.argument.contains(at))
            || file
                .ast
                .object_properties
                .iter()
                .any(|p| p.value.contains(at))
            || file
                .ast
                .exports
                .iter()
                .flat_map(|export| export.specifiers.iter().chain(&export.declarations))
                .any(|export| {
                    symbol_at(file, entities, export.local.span)
                        .is_some_and(|symbol| aliases.contains(symbol))
                })
            || file
                .ast
                .returns
                .iter()
                .filter_map(|site| site.argument)
                .any(|value| file.ast.peel_ts_sugar_span(value) == at)
        {
            return false;
        }
        for call in file
            .ast
            .calls
            .iter()
            .filter(|call| call.callee.contains(at))
        {
            let Some(member) = file.ast.members.iter().find(|member| {
                member.span == file.ast.peel_ts_sugar_span(call.callee) && member.object == at
            }) else {
                return false;
            };
            let computed = file.ast.computed_members.contains(&member.span);
            let returned = match shape.kind.as_str() {
                "tuple" if computed => file
                    .source_text(member.property)
                    .and_then(|key| key.parse::<usize>().ok())
                    .and_then(|index| shape.elements.get(index))
                    .and_then(Option::as_ref),
                "object" if !computed => file
                    .source_text(member.property)
                    .and_then(|key| shape.properties.get(key)),
                _ => None,
            };
            if !returned.is_some_and(|value| value.kind == "accessor") {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use solid_facts::{
        compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap},
        core::{Generation, SourceHash, SourcePath},
    };
    use std::{collections::HashMap, sync::Arc};

    #[test]
    fn structural_identity_refuses_alias_writes_deletes_and_escapes() {
        for (body, stable) in [
            ("result.value();", true),
            ("const alias = result; alias.value();", true),
            (
                "const alias = result; alias.value = () => 1; result.value();",
                false,
            ),
            ("result[key] = () => 1; result.value();", false),
            ("delete result.value; result.value();", false),
            ("delete (result as any).value; result.value();", false),
            ("replace(result); result.value();", false),
            ("result.reset(); result.value();", false),
            ("const outer = {result}; result.value();", false),
            (
                "const outer = [result]; outer[0].value = other; result.value();",
                false,
            ),
            (
                "const alias = true ? result : other; alias.value = other; result.value();",
                false,
            ),
            ("export {result};", false),
            (
                "const other = {}; other.value = () => 1; result.value();",
                true,
            ),
            (
                "{ const result = other(); result.value = () => 1; } result.value();",
                true,
            ),
        ] {
            let source = format!("const result = make(); {body}");
            let ast = solid_facts::ast::extract("test.ts", &source).unwrap();
            let mut symbols = HashMap::new();
            for binding in &ast.bindings {
                for name in &binding.names {
                    symbols.insert(
                        (u64::from(name.span.start), u64::from(name.span.end)),
                        SymbolId::from(format!("decl-{}", name.span.start)),
                    );
                }
            }
            let root = symbols[&(6, 12)].clone();
            let entities = EntitySymbols {
                by_path: HashMap::from([("test.ts".into(), symbols)]),
            };
            let file = FileFacts {
                generation: Generation::new(1).unwrap(),
                path: SourcePath::new("test.ts").unwrap(),
                source_hash: SourceHash::of(&source),
                source: Arc::from(source),
                ast: Arc::new(ast),
                compiler: Arc::new(ExecutionMap {
                    compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
                    source_hash: SourceHash::of(""),
                    semantic_model: Default::default(),
                    tracked_regions: vec![],
                    untracked_regions: vec![],
                    discarded_regions: vec![],
                    ownership_regions: vec![],
                    callback_roles: vec![],
                    jsx_operations: vec![],
                }),
            };
            let shape = crate::ContractReturn {
                kind: "object".into(),
                properties: std::collections::BTreeMap::from([(
                    "value".into(),
                    crate::ContractReturn {
                        kind: "accessor".into(),
                        ..Default::default()
                    },
                )]),
                ..Default::default()
            };
            assert_eq!(
                structural_binding_is_stable(&file, &entities, &root, &shape),
                stable,
                "{body}"
            );
        }
    }
}
