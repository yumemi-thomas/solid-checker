//! ADR 0296: the one dependency-source path (ADRs 0284, 0291) that a one-shot
//! check and a retained daemon share. Preparation reads the project's own
//! program, folding rewrites the selected packages' host constants, and
//! binding attaches the selection's roots and redirects to one generation's
//! facts. Neither caller has a second copy of any of the three.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    error::Error,
    path::{Path, PathBuf},
};

use crate::{
    SourceFile, TypeFactsSession,
    dependency_sources::{Host, PreparedDependencyProgram, SelectionNotes, prepare_program},
    dialect::Dialect,
};
use solid_facts::core::SourceHash;
use solid_reactive_ir::{RuntimeEnvironment, RuntimeTarget};

/// What preparation answered: the notes it printed in a one-shot check, and
/// the program, when any package was selected. Two equal values are the same
/// program with the same notes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedDependencies {
    pub notes: SelectionNotes,
    pub program: Option<PreparedDependencyProgram>,
}

/// Preparation's notes, beside its outcome: a refusal still prints the notes
/// that preceded it.
pub type Preparation = (
    SelectionNotes,
    Result<Option<PreparedDependencyProgram>, String>,
);

/// The host a package entry is selected for. A package's entry depends on
/// the host's target and build (review round 1 of ADR 0284), so neither may
/// be left to a default.
pub fn host(runtime: &RuntimeEnvironment) -> Result<Host, String> {
    let (Some(target), Some(_)) = (runtime.target, runtime.build) else {
        return Err(
            "--dependency-source needs --runtime-target and --runtime-build: \
             the file a package loads depends on both"
                .into(),
        );
    };
    Ok(Host {
        conditions: runtime.selected_conditions(),
        browser: target == RuntimeTarget::Browser,
    })
}

/// Prepare from `original`, the Type Facts program of the project's own
/// tsconfig. The project's configured sources, read from its own program, are
/// the project half of the generated program's `files`: composing
/// `include`/`files` through `extends` widened or emptied it (review round 2
/// of ADR 0284).
///
/// The notes come back beside the outcome, so a refusal still prints the
/// notes that preceded it.
pub fn prepare(
    project: &Path,
    selection: &[String],
    host: &Host,
    dialect: &'static Dialect,
    original: &mut TypeFactsSession,
) -> Result<Preparation, Box<dyn Error>> {
    let sources = original.configured_sources()?;
    let modules = program_modules(original)?;
    Ok(prepare_from(
        project, selection, host, dialect, &sources, &modules,
    ))
}

/// Every file the open program holds, by path.
pub fn program_modules(original: &mut TypeFactsSession) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    Ok(original
        .module_graph(&typefacts::ModuleGraphDemand::default())?
        .modules
        .iter()
        .map(|module| PathBuf::from(module.path.as_ref()))
        .collect())
}

/// [`prepare`] from an inventory the caller already holds.
pub fn prepare_from(
    project: &Path,
    selection: &[String],
    host: &Host,
    dialect: &'static Dialect,
    sources: &[SourceFile],
    modules: &[PathBuf],
) -> Preparation {
    let excluded = dialect
        .vocabulary
        .primitive_defining_packages()
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<BTreeSet<_>>();
    let mut notes = SelectionNotes::default();
    let outcome = prepare_program(
        project,
        selection,
        host,
        sources,
        modules,
        &excluded,
        dialect.vocabulary.ecosystem_scopes(),
        &mut notes,
    );
    (notes, outcome)
}

/// The sources a dependency-source program analyzes, folded.
pub struct FoldedSources {
    pub sources: Vec<SourceFile>,
    /// Per folded path, the digest of the program's own text.
    pub program_hashes: HashMap<String, SourceHash>,
    /// Per source inside a selected package, the digest of its folded text,
    /// or `None` when nothing folded: what a retained daemon compares to know
    /// the fold still answers as it did.
    pub record: BTreeMap<String, Option<SourceHash>>,
}

/// ADR 0284: under a declared host, a selected package's own host constants
/// (`isServer`) fold as they do for contract emission, matching the browser
/// entry the mode selected. The project's files are analyzed as before.
pub fn fold_sources(
    sources: &[SourceFile],
    program: &PreparedDependencyProgram,
    runtime: &RuntimeEnvironment,
    window_realm: bool,
) -> FoldedSources {
    // The conditions `--runtime-target` and its siblings select, not only
    // explicit `--runtime-condition`s. ADR 0295: a declared Window realm
    // decides these files' `typeof window` guards too, and nothing else's.
    let conditions = runtime.selected_conditions();
    let realm = window_realm.then(|| crate::dependency_sources::Host {
        conditions: conditions.clone(),
        browser: true,
    });
    let mut program_hashes = HashMap::new();
    let mut record = BTreeMap::new();
    let sources = sources
        .iter()
        .cloned()
        .map(|file| {
            let inside = Path::new(&file.path)
                .canonicalize()
                .is_ok_and(|path| program.contains(&path));
            if !inside {
                return file;
            }
            let path = file.path.clone();
            let (mut folded, hashes) = crate::host_constants::fold_host_constants_in_realm(
                vec![file],
                &conditions,
                realm.as_ref(),
            );
            let folded = folded.remove(0);
            record.insert(
                path,
                (!hashes.is_empty()).then(|| SourceHash::of(folded.source.as_ref())),
            );
            program_hashes.extend(hashes);
            folded
        })
        .collect();
    FoldedSources {
        sources,
        program_hashes,
        record,
    }
}

/// ADR 0284: bind one generation's facts to the selection: the package roots
/// that scope premises and findings, and each selected import's redirect to
/// the runtime file it loads. Redirects are read off `typescript`'s current
/// generation, so they are recomputed for every generation, never carried.
pub fn bind(
    facts: &mut solid_facts::ProjectFacts,
    typescript: &mut TypeFactsSession,
    program: &PreparedDependencyProgram,
) -> Result<(), Box<dyn Error>> {
    facts.dependency_source_roots = program
        .selection
        .roots
        .iter()
        .map(|root| root.to_string_lossy().into_owned())
        .collect();
    let redirects =
        crate::source_redirects::dependency_source_redirects(facts, typescript, program)?;
    facts.runtime_symbol_redirects.extend(redirects);
    Ok(())
}

/// Decide the remaining contract boundary only after the run's actual tiers
/// and artifact admission have been acquired. An internal external call is
/// source analysis's obligation; an exported external value must preserve the
/// exporting package's contract boundary when its target stays on contracts.
pub fn retain_exported_contracts(
    facts: &mut solid_facts::ProjectFacts,
    program: &PreparedDependencyProgram,
    contracts: &mut solid_reactive_ir::contract_semantics::AcceptedContractIndex,
    boundary: &mut solid_reactive_ir::package_boundary::PackageBoundary,
    refusals: &[crate::dependency_sources::SourceRefusal],
    dialect: &'static Dialect,
) -> (BTreeSet<String>, SelectionNotes) {
    let mut selection = program.selection.clone();
    let mut retained = crate::dependency_sources::UnsupportedBindings::default();
    let mut notes = SelectionNotes::default();
    loop {
        let mut changed = false;
        for file in &facts.files {
            let path = Path::new(file.path.as_str());
            let Some(root) = selection
                .roots
                .iter()
                .filter(|root| path.starts_with(root))
                .max_by_key(|root| root.components().count())
            else {
                continue;
            };
            let eligible =
                |module: &str| {
                    crate::dependency_sources::package_name(module).is_some_and(|name| {
                        !selection.package_names.contains(&name)
                            && !dialect.vocabulary.primitive_defining_packages().iter().any(
                                |package| {
                                    crate::dependency_sources::package_name(package).as_deref()
                                        == Some(name.as_str())
                                },
                            )
                            && contracts.contract(file.path.as_str(), module).is_ok()
                    })
                };
            for (module, (span, form)) in exported_external_modules(&file.ast, &eligible) {
                // Use selection names as well as manifest names (npm aliases).
                let names = selection
                    .resolutions
                    .iter()
                    .filter(|edge| edge.target.starts_with(root))
                    .filter_map(|edge| crate::dependency_sources::package_name(&edge.specifier))
                    .filter(|name| selection.package_names.contains(name))
                    .collect::<BTreeSet<_>>();
                for exporting in names {
                    let line = file.source[..span.start as usize]
                        .bytes()
                        .filter(|byte| *byte == b'\n')
                        .count()
                        + 1;
                    retained.retain(
                        exporting,
                        solid_reactive_ir::package_boundary::UnanalyzableReason::NotSelected(
                            solid_reactive_ir::package_boundary::SelectionFailure::RetainedExportedValue,
                        ),
                        format!(
                            "{form} from accepted unselected package {module} at {}:{line}",
                            file.path
                        ),
                    );
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
        match crate::dependency_sources::without_unredirected_packages(
            selection, &retained, &mut notes,
        ) {
            Some(remaining) => selection = remaining,
            None => {
                selection = program.selection.clone();
                selection.roots.clear();
                selection.files.clear();
                selection.resolutions.clear();
                selection.package_names.clear();
                break;
            }
        }
    }
    let removed = program
        .selection
        .roots
        .iter()
        .filter(|root| !selection.roots.contains(root))
        .collect::<Vec<_>>();
    if !removed.is_empty() {
        let inside = |path: &str| removed.iter().any(|root| Path::new(path).starts_with(root));
        let removed_symbols = facts
            .typescript
            .symbols()
            .filter(|symbol| {
                symbol
                    .declarations()
                    .iter()
                    .any(|declaration| inside(&declaration.location.path))
            })
            .map(|symbol| symbol.id().to_owned())
            .collect::<BTreeSet<_>>();
        facts.runtime_symbol_redirects.retain(|source, target| {
            !removed_symbols.contains(source) && !removed_symbols.contains(target)
        });
        facts.files.retain(|file| !inside(file.path.as_str()));
        facts.dependency_source_roots = selection
            .roots
            .iter()
            .map(|root| root.to_string_lossy().into_owned())
            .collect();
        // Initial acquisition saw these roots as source and consequently did
        // not request their ordinary external-package obligations. Restore
        // those requirements together with the contract boundary.
        let refusals = refusals
            .iter()
            .chain(&notes.refusals)
            .cloned()
            .collect::<Vec<_>>();
        let restored = crate::external_package_boundary(facts, &refusals);
        *contracts = std::mem::take(contracts).with_fallback(crate::requirement_index(&restored));
        boundary.extend(restored);
    }
    (selection.package_names, notes)
}

fn exported_external_modules(
    facts: &solid_facts::ast::AstFacts,
    eligible: &impl Fn(&str) -> bool,
) -> BTreeMap<String, (solid_facts::core::Span, &'static str)> {
    use solid_facts::core::Span;
    let mut modules = BTreeMap::new();
    let mut functions_by_start = facts.functions.iter().collect::<Vec<_>>();
    functions_by_start.sort_by_key(|function| function.span);
    let mut returned_values = BTreeMap::<Span, Vec<Span>>::new();
    for returned in &facts.returns {
        if let Some(argument) = returned.argument
            && let Some(owner) = facts
                .functions
                .iter()
                .filter(|function| function.body.contains(returned.span))
                .min_by_key(|function| function.body.end - function.body.start)
        {
            returned_values
                .entry(owner.span)
                .or_default()
                .push(argument);
        }
    }
    for export in facts.module_level_exports().filter(|edge| !edge.type_only) {
        if let Some(module) = export.module.as_deref()
            && eligible(module)
            && (export.kind == solid_facts::ast::ExportKind::All
                || export.specifiers.iter().any(|binding| !binding.type_only))
        {
            modules.insert(
                module.to_owned(),
                (
                    export.span,
                    if export.namespace.is_some() {
                        "namespace re-export"
                    } else if export.kind == solid_facts::ast::ExportKind::All {
                        "star re-export"
                    } else {
                        "named re-export"
                    },
                ),
            );
        }
    }
    let seeds = facts
        .imports
        .iter()
        .filter(|edge| !edge.type_only)
        .map(|edge| {
            (
                edge.module.as_str(),
                edge.span,
                edge.bindings
                    .iter()
                    .filter(|binding| !binding.type_only && binding.runtime_referenced)
                    .map(|binding| binding.local.span)
                    .collect::<BTreeSet<_>>(),
                None,
            )
        })
        .chain(
            facts
                .import_equals
                .iter()
                .filter(|edge| !edge.type_only)
                .map(|edge| {
                    (
                        edge.module.as_str(),
                        edge.span,
                        [edge.local.span].into_iter().collect(),
                        None,
                    )
                }),
        )
        .chain(facts.module_loads.iter().filter_map(|edge| {
            edge.specifier
                .as_deref()
                .map(|module| (module, edge.span, BTreeSet::new(), Some(edge.span)))
        }));
    for (module, import_span, mut tainted, load) in seeds.filter(|(module, ..)| eligible(module)) {
        if modules.contains_key(module) {
            continue;
        }
        let mut derived_functions = BTreeSet::new();
        let reads = |span: Span, tainted: &BTreeSet<Span>, functions: &BTreeSet<Span>| {
            let first_function =
                functions_by_start.partition_point(|function| function.span.start < span.start);
            let direct = |reference| {
                !functions_by_start[first_function..]
                    .iter()
                    .take_while(|function| function.span.start < span.end)
                    .any(|function| {
                        span.contains(function.span) && function.body.contains(reference)
                    })
            };
            let first_reference = facts
                .reference_declarations
                .partition_point(|(reference, _)| reference.start < span.start);
            functions.iter().any(|function| span.contains(*function))
                || load.is_some_and(|load| span.contains(load) && direct(load))
                || facts.reference_declarations[first_reference..]
                    .iter()
                    .take_while(|(reference, _)| reference.start < span.end)
                    .any(|(reference, declaration)| {
                        span.contains(*reference)
                            && tainted.contains(declaration)
                            && direct(*reference)
                    })
        };
        loop {
            let before = (tainted.len(), derived_functions.len());
            for binding in &facts.bindings {
                if binding
                    .initializer
                    .is_some_and(|span| reads(span, &tainted, &derived_functions))
                {
                    tainted.extend(binding.names.iter().map(|name| name.span));
                }
            }
            for assignment in &facts.assignments {
                if reads(assignment.value_span, &tainted, &derived_functions) {
                    tainted.extend(
                        facts
                            .reference_declarations
                            .iter()
                            .filter(|(reference, _)| assignment.target.contains(*reference))
                            .map(|(_, declaration)| *declaration),
                    );
                }
            }
            for function in &facts.functions {
                if (function.expression_body && reads(function.body, &tainted, &derived_functions))
                    || returned_values
                        .get(&function.span)
                        .into_iter()
                        .flatten()
                        .any(|span| reads(*span, &tainted, &derived_functions))
                {
                    derived_functions.insert(function.span);
                    tainted.extend(function.name.iter().map(|name| name.span));
                    tainted.extend(
                        facts
                            .bindings
                            .iter()
                            .filter(|binding| {
                                binding.initializer_function
                                    && binding
                                        .initializer
                                        .is_some_and(|span| span.contains(function.span))
                            })
                            .flat_map(|binding| binding.names.iter().map(|name| name.span)),
                    );
                }
            }
            if before == (tainted.len(), derived_functions.len()) {
                break;
            }
        }
        // Transport above proves some flows; silence is not proof of absence.
        // Drop the exporting boundary only when every imported/derived use
        // belongs to that model. Classes, accessors, captured state and mutation
        // calls can publish by channels it does not summarize. A side-effect
        // helper may also mutate another file's exported state.
        let references = &facts.reference_declarations;
        let unclosed = references
            .iter()
            .filter(|(reference, declaration)| {
                tainted.contains(declaration)
                    && !facts
                        .type_queries
                        .iter()
                        .any(|query| query.contains(*reference))
            })
            .map(|(reference, _)| reference)
            .chain(load.as_ref())
            .any(|reference| {
                if facts
                    .classes
                    .iter()
                    .any(|class| class.span.contains(*reference))
                    || facts
                        .object_properties
                        .iter()
                        .any(|property| !property.data && property.value.contains(*reference))
                    || facts
                        .write_targets
                        .iter()
                        .chain(&facts.deleted_targets)
                        .any(|target| target.contains(*reference))
                    || facts
                        .assignments
                        .iter()
                        .any(|assignment| assignment.target.contains(*reference))
                {
                    return true;
                }
                // No guessed purity: arguments may escape or mutate a
                // receiver/global. A tainted callee receiving local values
                // may mutate them too. Literal-only discarded calls remain
                // the narrow internal-use control.
                if facts.calls.iter().any(|call| {
                    call.arguments
                        .iter()
                        .any(|argument| argument.span.contains(*reference))
                        || (call.callee.contains(*reference)
                            && call.arguments.iter().any(|argument| {
                                references.iter().any(|(argument_reference, _)| {
                                    argument.span.contains(*argument_reference)
                                })
                            }))
                }) {
                    return true;
                }
                let transported = facts.bindings.iter().any(|binding| {
                    binding.initializer.is_some_and(|span| {
                        span.contains(*reference)
                            && functions_by_start.iter().all(|function| {
                                !span.contains(function.span) || !function.body.contains(*reference)
                            })
                    })
                }) || facts
                    .assignments
                    .iter()
                    .any(|assignment| assignment.value_span.contains(*reference))
                    || returned_values
                        .values()
                        .flatten()
                        .any(|span| span.contains(*reference))
                    || facts.functions.iter().any(|function| {
                        function.expression_body && function.body.contains(*reference)
                    })
                    || facts.module_level_exports().any(|export| {
                        !export.type_only
                            && export.span.contains(*reference)
                            && facts
                                .functions
                                .iter()
                                .all(|function| !function.body.contains(*reference))
                    });
                let discarded = facts.calls.iter().any(|call| {
                    call.result_discarded && facts.peel_ts_sugar_span(call.callee) == *reference
                });
                !transported && !discarded
            });
        if unclosed {
            modules.insert(
                module.to_owned(),
                (import_span, "exported-value provenance is not closed"),
            );
            continue;
        }
        if facts
            .module_level_exports()
            .filter(|edge| !edge.type_only && edge.module.is_none())
            .any(|export| {
                export
                    .specifiers
                    .iter()
                    .chain(&export.declarations)
                    .filter(|binding| !binding.type_only)
                    .any(|binding| {
                        tainted.contains(&binding.local.span)
                            || reads(binding.local.span, &tainted, &derived_functions)
                    })
                    || facts.exported_bindings(export).any(|binding| {
                        binding
                            .names
                            .iter()
                            .any(|name| tainted.contains(&name.span))
                    })
                    || (export.kind == solid_facts::ast::ExportKind::Default
                        && reads(export.span, &tainted, &derived_functions))
            })
        {
            modules.insert(
                module.to_owned(),
                (import_span, "exports an imported or derived value"),
            );
        }
    }
    modules
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_exported_external_values_retain_a_boundary() {
        for source in [
            "export { x } from 'pkg';",
            "export * from 'pkg';",
            "export * as ns from 'pkg';",
            "import { x } from 'pkg'; export { x as y };",
            "import { x } from 'pkg'; export const y = x;",
            "import { x } from 'pkg'; const y = x(); export { y };",
            "import { x } from 'pkg'; export function y() { return x(); }",
            "import { x } from 'pkg'; export const y = () => x();",
            "import { x } from 'pkg'; let y; y = x; export { y };",
            "import { x } from 'pkg'; class C { value = x(); } const c = new C(); export function y() { return c.value; }",
            "import { x } from 'pkg'; export class C { value = x(); }",
            "import { x } from 'pkg'; const obj = {}; Object.assign(obj, { value: x() }); export { obj };",
            "import { x } from 'pkg'; const obj = {}; mutate(obj, x()); export { obj };",
            "import { x } from 'pkg'; const obj = { get value() { return x(); } }; export { obj };",
            "import { x } from 'pkg'; let state = x(); export const update = () => { state++; };",
            "import { x } from 'pkg'; const obj = {}; x(obj); export { obj };",
            "import { x } from 'pkg'; const value = x(); export const update = () => { sink(value); };",
        ] {
            let facts = solid_facts::ast::extract("module.ts", source).unwrap();
            assert!(
                super::exported_external_modules(&facts, &|_| true).contains_key("pkg"),
                "{source}"
            );
        }
        for source in [
            "import { x } from 'pkg'; x(); export const y = 1;",
            "import { x } from 'pkg'; export function y() { x(); return 1; }",
            "import { x } from 'pkg'; export const y = () => { x(); return 1; };",
            "import { x } from 'pkg'; export default function y() { x(); return 1; }",
            "import { x } from 'pkg'; export default () => { x(); return 1; };",
            "import { x } from 'pkg'; function internal() { return x(); } export const y = 1;",
            "import { x } from 'pkg'; function y(x) { return x; } export { y };",
            "import { x } from 'pkg'; x('literal'); export const y = 1;",
            "import { x } from 'pkg'; (((x)))(); export const y = 1;",
            "import { x } from 'pkg'; (x as typeof x)(); export const y = 1;",
            "import { x } from 'pkg'; class C { value = 1; } x(); export { C };",
            "export type { T } from 'pkg';",
        ] {
            let facts = solid_facts::ast::extract("module.ts", source).unwrap();
            assert!(
                !super::exported_external_modules(&facts, &|_| true).contains_key("pkg"),
                "{source}"
            );
        }
    }
}
