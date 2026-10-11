//! Authored integration premises (ADR 0271). Package identities and affected
//! exports come from the authored index, never the dialect or shared IR.

use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use solid_facts::ast::{AstFacts, BindingShape, ImportKind};
use solid_facts::core::Span;
use solid_facts::{FileFacts, ProjectFacts};
use typefacts::Location;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct IntegrationPremise {
    pub name: String,
    pub exports: Vec<String>,
    pub value_references: Vec<DeclarationIdentity>,
    pub config_calls: Vec<ConfigCall>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct DeclarationIdentity {
    pub path: String,
    pub start: u64,
    pub end: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ConfigCall {
    pub declaration: DeclarationIdentity,
    pub property: String,
    pub factories: Vec<DeclarationIdentity>,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum PremiseState {
    Assumed,
    Vetoed(Location),
}

fn runtime_span(ast: &AstFacts, span: Span) -> bool {
    !ast.type_queries.iter().any(|query| query.contains(span))
        && !ast
            .type_imports
            .iter()
            .any(|import| import.span.contains(span))
}

struct Identities<'a> {
    facts: &'a ProjectFacts,
    file: &'a FileFacts,
    symbols: HashMap<(u64, u64), Option<&'a str>>,
    roots: &'a [String],
}

impl Identities<'_> {
    fn at(&self, expression: Span) -> Option<DeclarationIdentity> {
        self.at_inner(expression, &mut HashSet::new())
    }

    fn at_inner(
        &self,
        expression: Span,
        visited: &mut HashSet<Span>,
    ) -> Option<DeclarationIdentity> {
        let ast = &self.file.ast;
        let expression = ast.peel_ts_sugar_span(expression);
        if !visited.insert(expression) {
            return None;
        }
        // A computed target is not exact static member dispatch, even when a
        // type assertion happens to make its result look like an export.
        if ast.computed_members.contains(&expression) {
            return None;
        }
        let span = ast
            .members
            .iter()
            .find(|member| member.span == expression)
            .map_or(expression, |member| member.property);
        if let Some(symbol) = self
            .symbols
            .get(&(u64::from(span.start), u64::from(span.end)))
            .copied()
            .flatten()
        {
            let mut current = symbol;
            let mut seen = HashSet::new();
            while seen.insert(current) {
                let symbol = self.facts.typescript.symbol(current)?;
                if !symbol.alias_target().is_empty() {
                    current = symbol.alias_target();
                    continue;
                }
                let [declaration] = symbol.declarations() else {
                    break;
                };
                let path = declaration.location.path.replace('\\', "/");
                let mut relatives = self.roots.iter().filter_map(|root| {
                    path.strip_prefix(root)
                        .and_then(|tail| tail.strip_prefix('/'))
                });
                if let Some(relative) = relatives.next() {
                    if relatives.any(|other| other != relative) {
                        return None;
                    }
                    return Some(DeclarationIdentity {
                        path: relative.to_owned(),
                        start: declaration.location.start_byte,
                        end: declaration.location.end_byte,
                    });
                }
                break;
            }
        }
        // Exact immutable local aliases, using the binder's declaration;
        // mutable/reassigned and destructured dispatch remains unresolved.
        let declaration = ast.reference_declaration(expression)?;
        let binding = ast.bindings.iter().find(|binding| {
            binding.immutable
                && binding.shape == BindingShape::Identifier
                && binding.names.iter().any(|name| name.span == declaration)
        })?;
        self.at_inner(binding.initializer?, visited)
    }
}

pub(super) fn scan(
    facts: &ProjectFacts,
    package: &str,
    version: &str,
    premise: &IntegrationPremise,
) -> PremiseState {
    // Only existing owning-manifest evidence may normalize aliases/store
    // links. A path suffix or an import's spelling supplies no identity.
    let mut owners = HashMap::new();
    for (_, import) in facts
        .resolved_imports
        .iter()
        .flat_map(|imports| imports.iter())
    {
        let Some(manifest) = import.package_manifest.as_ref() else {
            continue;
        };
        let manifest = manifest.replace('\\', "/");
        let Some((root, _)) = manifest.rsplit_once('/') else {
            continue;
        };
        let matching = import.package_name.as_deref() == Some(package)
            && import.package_version.as_deref() == Some(version);
        owners
            .entry(root.to_owned())
            .and_modify(|agrees| *agrees &= matching)
            .or_insert(matching);
    }
    let mut roots = owners
        .into_iter()
        .filter_map(|(root, agrees)| agrees.then_some(root))
        .collect::<Vec<_>>();
    roots.sort();
    for file in &facts.files {
        let mut symbols = HashMap::new();
        for entity in facts.typescript.entities_for_path(file.path.as_str()) {
            let symbol = (!entity.symbol_unresolved && !entity.symbol.is_empty())
                .then_some(entity.symbol.as_ref());
            symbols
                .entry((entity.location.start_byte, entity.location.end_byte))
                .and_modify(|value| *value = None)
                .or_insert(symbol);
        }
        let ids = Identities {
            facts,
            file,
            symbols,
            roots: &roots,
        };
        let is_value = |span| {
            ids.at(span)
                .is_some_and(|id| premise.value_references.contains(&id))
        };
        for import in &file.ast.imports {
            if import.type_only {
                continue;
            }
            for binding in &import.bindings {
                if !binding.type_only
                    && binding.runtime_referenced
                    && !matches!(binding.kind, ImportKind::Namespace | ImportKind::SideEffect)
                    && is_value(binding.local.span)
                {
                    return PremiseState::Vetoed(binding.local.span.location(file.path.shared()));
                }
            }
        }
        for export in &file.ast.exports {
            if export.type_only {
                continue;
            }
            for specifier in &export.specifiers {
                if !specifier.type_only && is_value(specifier.local.span) {
                    return PremiseState::Vetoed(specifier.local.span.location(file.path.shared()));
                }
            }
        }
        for member in &file.ast.members {
            if runtime_span(&file.ast, member.span) && is_value(member.span) {
                return PremiseState::Vetoed(member.span.location(file.path.shared()));
            }
        }
        for call in &file.ast.calls {
            if call.construct || !runtime_span(&file.ast, call.span) {
                continue;
            }
            let Some(identity) = ids.at(call.callee) else {
                continue;
            };
            let Some(config) = premise
                .config_calls
                .iter()
                .find(|config| config.declaration == identity)
            else {
                continue;
            };
            if !standard_config(
                &ids,
                call.arguments
                    .first()
                    .filter(|arg| !arg.spread)
                    .map(|arg| arg.span),
                config,
            ) {
                return PremiseState::Vetoed(call.span.location(file.path.shared()));
            }
        }
    }
    PremiseState::Assumed
}

fn standard_config(ids: &Identities<'_>, argument: Option<Span>, config: &ConfigCall) -> bool {
    let ast = &ids.file.ast;
    let Some(argument) = argument.map(|span| ast.peel_ts_sugar_span(span)) else {
        return false;
    };
    let Some(object) = ast
        .object_get_shapes
        .iter()
        .find(|object| object.span == argument && object.closed)
    else {
        return false;
    };
    let Some(index) = object
        .properties
        .iter()
        .position(|property| property.key == config.property)
    else {
        return true;
    };
    // The closed literal records cooked keys in source order; join the
    // corresponding direct property by that order, including quoted keys.
    let property = ast
        .object_properties
        .iter()
        .filter(|property| {
            object.span.contains(property.span)
                && ast
                    .object_get_shapes
                    .iter()
                    .filter(|shape| shape.span.contains(property.span))
                    .min_by_key(|shape| shape.span.end - shape.span.start)
                    .is_some_and(|shape| shape.span == object.span)
        })
        .nth(index);
    let Some(property) = property.filter(|property| property.data && !property.computed) else {
        return false;
    };
    let value = ast.peel_ts_sugar_span(property.value);
    ast.calls
        .iter()
        .find(|call| call.span == value && !call.construct)
        .and_then(|call| ids.at(call.callee))
        .is_some_and(|id| config.factories.contains(&id))
}

#[cfg(test)]
mod tests;
