//! ADR 0266: an assumed runtime condition, with positive visible vetoes only.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use solid_dialect::{Dialect, RuntimeConfigurationApi};
use solid_facts::ast::ImportKind;
use solid_facts::core::Span;
use solid_facts::{ProjectFacts, TypeScriptTable};
use typefacts::Location;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum RuntimeConfigurationPremise {
    #[default]
    Assumed,
    Vetoed {
        location: Location,
    },
}

impl RuntimeConfigurationPremise {
    pub fn permits_proof(&self) -> bool {
        matches!(self, Self::Assumed)
    }

    pub(crate) fn apply(&self, finding: &mut crate::Finding) {
        let Self::Vetoed { location } = self else {
            // In particular, do not add evidence to existing Assumed findings.
            return;
        };
        finding.kind = "uncertifiable".into();
        finding.message = format!(
            "Cannot certify this finding under the standard runtime configuration premise: a configuration export is used at {}:{}-{}.",
            location.path, location.start_byte, location.end_byte
        );
        finding.hint = "Review ADR 0266 and the configuration site; restoring the premise requires reanalysis of the configured project.".into();
        finding.fixes.clear();
        finding.evidence.clear();
        finding.evidence.push(crate::EvidenceStep {
            message: "Standard runtime configuration premise vetoed (ADR 0266).".into(),
            location: Some(location.clone()),
        });
        finding.related_locations.push(location.clone());
    }
}

/// Follow exact binder aliases. Missing, cyclic, synthetic or merged evidence
/// supplies no positive identity and therefore cannot veto this premise.
fn configuration_api(
    table: &TypeScriptTable,
    symbol: &str,
    dialect: &dyn Dialect,
    package_roots: &[(String, String)],
) -> Option<RuntimeConfigurationApi> {
    let mut current = symbol;
    let mut seen = HashSet::new();
    loop {
        if !seen.insert(current) {
            return None;
        }
        let resolved = table.symbol(current)?;
        if !resolved.alias_target().is_empty() {
            current = resolved.alias_target();
            continue;
        }
        let [declaration] = resolved.declarations() else {
            return None;
        };
        let path = canonical_declaration_path(declaration.location.path.as_ref(), package_roots)?;
        return dialect.runtime_configuration_api(
            &path,
            declaration.location.start_byte,
            declaration.location.end_byte,
        );
    }
}

/// Normalize npm aliases/linked roots only from existing attested manifests.
/// Conflicting owning identities do not manufacture positive identification.
fn canonical_declaration_path(path: &str, roots: &[(String, String)]) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let mut best: Option<(usize, String)> = None;
    for (root, package) in roots {
        let Some(relative) = normalized
            .strip_prefix(root)
            .and_then(|tail| tail.strip_prefix('/'))
        else {
            continue;
        };
        let canonical = format!("/node_modules/{package}/{relative}");
        match &best {
            Some((length, previous)) if *length == root.len() && previous != &canonical => {
                return None;
            }
            Some((length, _)) if *length > root.len() => {}
            _ => best = Some((root.len(), canonical)),
        }
    }
    Some(best.map_or(normalized, |(_, path)| path))
}

fn runtime_span(ast: &solid_facts::ast::AstFacts, span: Span) -> bool {
    !ast.type_queries.iter().any(|query| query.contains(span))
        && !ast
            .type_imports
            .iter()
            .any(|import| import.span.contains(span))
}

/// `if (DEV)`, `DEV ? a : b` and `DEV && f()` only test the binding's
/// truthiness: none can reach a hook or exclusion. Every other reference,
/// including `DEV.hooks` and passing `DEV` on, still vetoes. Without at least
/// one binder-resolved reference the use is not established as harmless.
fn only_truthiness_tests(ast: &solid_facts::ast::AstFacts, declaration: Span) -> bool {
    use solid_facts::ast::{IdentifierRole, LogicalOperatorKind};
    let mut references = ast
        .identifiers
        .iter()
        .filter(|id| {
            id.role == IdentifierRole::Reference
                && ast.reference_declaration(id.span) == Some(declaration)
        })
        .peekable();
    references.peek().is_some()
        && references.all(|id| {
            !runtime_span(ast, id.span)
                || ast.if_regions.iter().any(|region| region.test == id.span)
                || ast
                    .conditional_expressions
                    .iter()
                    .any(|conditional| conditional.test == id.span)
                || ast.logical_expressions.iter().any(|logical| {
                    logical.operator == LogicalOperatorKind::And && logical.left == id.span
                })
        })
}

pub(crate) fn scan(facts: &ProjectFacts, dialect: &dyn Dialect) -> RuntimeConfigurationPremise {
    let mut roots = facts
        .resolved_imports
        .iter()
        .flat_map(|resolutions| resolutions.iter())
        .filter_map(|(_, import)| {
            let manifest = import.package_manifest.as_ref()?.replace('\\', "/");
            let (root, _) = manifest.rsplit_once('/')?;
            Some((root.to_owned(), import.package_name.as_ref()?.to_string()))
        })
        .collect::<Vec<_>>();
    roots.sort();
    roots.dedup();
    let mut identities = HashMap::new();
    for file in &facts.files {
        // Index only entities already supplied by the analysis. A duplicate
        // exact span is ambiguous; contained entities are never substitutes.
        let mut symbols = HashMap::new();
        for entity in facts.typescript.entities_for_path(file.path.as_str()) {
            let key = (entity.location.start_byte, entity.location.end_byte);
            let symbol = (!entity.symbol_unresolved && !entity.symbol.is_empty())
                .then_some(entity.symbol.as_ref());
            symbols
                .entry(key)
                .and_modify(|value| *value = None)
                .or_insert(symbol);
        }
        let mut api_at = |span: Span| {
            let symbol = symbols
                .get(&(u64::from(span.start), u64::from(span.end)))
                .copied()
                .flatten()?;
            *identities
                .entry(symbol)
                .or_insert_with(|| configuration_api(&facts.typescript, symbol, dialect, &roots))
        };
        let member_properties = file
            .ast
            .members
            .iter()
            .map(|member| (member.span, member.property))
            .collect::<HashMap<_, _>>();
        // The existing Oxc binder proves a value use, excluding type-only
        // imports and value-as-type (`typeof DEV`) queries. No Type Facts
        // reference list is fetched or required, including for namespaces.
        for import in &file.ast.imports {
            if import.type_only {
                continue;
            }
            for binding in &import.bindings {
                if binding.type_only
                    || !binding.runtime_referenced
                    || matches!(binding.kind, ImportKind::Namespace | ImportKind::SideEffect)
                {
                    continue;
                }
                if matches!(
                    api_at(binding.local.span),
                    Some(
                        RuntimeConfigurationApi::DevelopmentHooks
                            | RuntimeConfigurationApi::Observation
                    )
                ) && !only_truthiness_tests(&file.ast, binding.local.span)
                {
                    return RuntimeConfigurationPremise::Vetoed {
                        location: binding.local.span.location(file.path.shared()),
                    };
                }
            }
        }
        // Includes exact namespace members, TS wrappers, and first references
        // before aliasing/destructuring/escape; unrelated members do not veto.
        for member in &file.ast.members {
            if runtime_span(&file.ast, member.span)
                && matches!(
                    api_at(member.property),
                    Some(
                        RuntimeConfigurationApi::DevelopmentHooks
                            | RuntimeConfigurationApi::Observation
                    )
                )
            {
                return RuntimeConfigurationPremise::Vetoed {
                    location: member.span.location(file.path.shared()),
                };
            }
        }
        // Merely importing or reading enableExternalSource is not a call.
        for call in &file.ast.calls {
            if call.construct || !runtime_span(&file.ast, call.span) {
                continue;
            }
            let callee = file.ast.peel_ts_sugar_span(call.callee);
            let property = member_properties.get(&callee).copied();
            if api_at(call.callee)
                .or_else(|| api_at(callee))
                .or_else(|| property.and_then(&mut api_at))
                == Some(RuntimeConfigurationApi::ExternalSource)
            {
                return RuntimeConfigurationPremise::Vetoed {
                    location: call.span.location(file.path.shared()),
                };
            }
        }
    }
    RuntimeConfigurationPremise::Assumed
}

#[cfg(test)]
mod tests {
    use super::*;
    use typefacts::{Declaration, SymbolFact};

    fn table(path: &str, start: u64, end: u64) -> TypeScriptTable {
        TypeScriptTable::from_parts(
            2,
            1,
            "premise",
            vec![],
            vec![],
            vec![
                SymbolFact {
                    id: "alias".into(),
                    alias_target: "export".into(),
                    declarations: Vec::new().into(),
                    references: Vec::new().into(),
                },
                SymbolFact {
                    id: "export".into(),
                    alias_target: "".into(),
                    declarations: vec![Declaration {
                        name: "untrusted-name".into(),
                        kind: "variable".into(),
                        location: Location {
                            path: path.into(),
                            start_byte: start,
                            end_byte: end,
                        },
                    }]
                    .into(),
                    references: Vec::new().into(),
                },
            ],
            vec![],
        )
    }

    #[test]
    fn only_positive_exact_alias_resolution_identifies_configuration() {
        let dialect = solid_dialect::Solid2;
        let original = table("/p/node_modules/solid-js/types/index.d.ts", 2531, 2534);
        assert_eq!(
            configuration_api(&original, "alias", &dialect, &[]),
            Some(RuntimeConfigurationApi::DevelopmentHooks)
        );
        assert_eq!(configuration_api(&original, "missing", &dialect, &[]), None);
        for path in [
            "/p/src/index.d.ts",
            "/p/node_modules/ordinary/types/index.d.ts",
        ] {
            assert_eq!(
                configuration_api(&table(path, 2531, 2534), "alias", &dialect, &[]),
                None
            );
        }
        for target in ["alias", "export", "missing"] {
            let mut encoded = serde_json::to_value(&original).unwrap();
            encoded["symbols"][1]["aliasTarget"] = target.into();
            let cyclic_or_missing: TypeScriptTable = serde_json::from_value(encoded).unwrap();
            assert_eq!(
                configuration_api(&cyclic_or_missing, "alias", &dialect, &[]),
                None
            );
        }
        let mut encoded = serde_json::to_value(&original).unwrap();
        let extra = encoded["symbols"][1]["declarations"][0].clone();
        encoded["symbols"][1]["declarations"]
            .as_array_mut()
            .unwrap()
            .push(extra);
        let merged: TypeScriptTable = serde_json::from_value(encoded).unwrap();
        assert_eq!(configuration_api(&merged, "alias", &dialect, &[]), None);
        let roots = [("/p/node_modules/alias".into(), "solid-js".into())];
        assert_eq!(
            configuration_api(
                &table("/p/node_modules/alias/types/index.d.ts", 2531, 2534),
                "alias",
                &dialect,
                &roots
            ),
            Some(RuntimeConfigurationApi::DevelopmentHooks)
        );
        let roots = [("/p/node_modules/solid-js".into(), "ordinary".into())];
        assert_eq!(
            configuration_api(&original, "alias", &dialect, &roots),
            None
        );
    }

    #[test]
    fn assumed_is_default_and_projection_is_byte_identical() {
        let mut finding = crate::Finding::new(
            crate::RuleMetadata {
                code: "SC1001",
                name: "strict-read-untracked",
                severity: "error",
                uncertifiable: false,
                default_enabled: true,
                presets: &[],
            },
            "original wording".into(),
            Span::new(1, 2).location("App.tsx"),
        );
        let before = serde_json::to_vec(&finding).unwrap();
        RuntimeConfigurationPremise::default().apply(&mut finding);
        assert_eq!(serde_json::to_vec(&finding).unwrap(), before);
        let site = Span::new(10, 13).location("setup.ts");
        RuntimeConfigurationPremise::Vetoed {
            location: site.clone(),
        }
        .apply(&mut finding);
        assert_eq!(finding.kind, "uncertifiable");
        assert_eq!(finding.related_locations.last(), Some(&site));
        assert_eq!(finding.evidence[0].location.as_ref(), Some(&site));
        assert!(finding.fixes.is_empty());
        assert!(RuntimeConfigurationPremise::default().permits_proof());
    }

    #[test]
    fn a_process_free_project_without_resolution_evidence_keeps_the_premise() {
        let facts = ProjectFacts {
            generation: solid_facts::core::Generation::new(1).unwrap(),
            project_id: "premise".into(),
            files: vec![],
            typescript: table("/p/src/local.d.ts", 1, 4),
            typescript_changes: None,
            resolved_imports: None,
            runtime_resolutions: None,
            runtime_symbol_redirects: HashMap::new(),
        };
        assert_eq!(
            scan(&facts, &solid_dialect::Solid2),
            RuntimeConfigurationPremise::Assumed
        );
    }
}
