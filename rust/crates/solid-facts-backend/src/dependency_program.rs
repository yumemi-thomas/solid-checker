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

/// Every file the open program holds, in the generated program's root order
/// (ADR 0368): TypeScript orders a program's files by its roots and its
/// answers can depend on that order, so a project root listed after a package
/// file can give a type error `tsc` never reports.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProgramModules {
    /// The configured root files, in TypeScript's own root order
    /// (`ModuleFact.root`), wherever they live: a root under `node_modules`
    /// is the project's root all the same.
    pub roots: Vec<PathBuf>,
    /// Every other file, by path.
    pub others: Vec<PathBuf>,
    /// Configured roots the producer could not place (`ModuleGraph::unmapped_roots`):
    /// the root order is then unknown, and preparation refuses rather than
    /// take path order for it.
    pub unmapped_roots: Vec<String>,
}

impl ProgramModules {
    /// Every file, roots first.
    pub fn iter(&self) -> impl Iterator<Item = &PathBuf> {
        self.roots.iter().chain(&self.others)
    }
}

/// The open program's [`ProgramModules`].
pub fn program_modules(original: &mut TypeFactsSession) -> Result<ProgramModules, Box<dyn Error>> {
    let graph = original.module_graph(&typefacts::ModuleGraphDemand::default())?;
    Ok(in_root_order(&graph))
}

/// `graph.modules` (ordered by path) split into the roots by their position
/// and the rest in the order given.
fn in_root_order(graph: &typefacts::ModuleGraph) -> ProgramModules {
    let mut roots = graph
        .modules
        .iter()
        .filter_map(|module| {
            module
                .root
                .map(|root| (root, PathBuf::from(module.path.as_ref())))
        })
        .collect::<Vec<_>>();
    roots.sort_by_key(|(root, _)| *root);
    ProgramModules {
        roots: roots.into_iter().map(|(_, path)| path).collect(),
        others: graph
            .modules
            .iter()
            .filter(|module| module.root.is_none())
            .map(|module| PathBuf::from(module.path.as_ref()))
            .collect(),
        unmapped_roots: graph
            .unmapped_roots
            .iter()
            .map(|root| root.to_string())
            .collect(),
    }
}

/// [`prepare`] from an inventory the caller already holds.
pub fn prepare_from(
    project: &Path,
    selection: &[String],
    host: &Host,
    dialect: &'static Dialect,
    sources: &[SourceFile],
    modules: &ProgramModules,
) -> Preparation {
    let excluded = dialect
        .vocabulary
        .primitive_defining_packages()
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<BTreeSet<_>>();
    let mut notes = SelectionNotes::default();
    // ADR 0360: the closure file the build would compile, compiled as the
    // build compiles it; only a refusal answers here, every other compiler
    // failure is the fact build's to report.
    let refusal = |path: &Path, source: &str| {
        let request = solid_facts::compiler::AnalysisRequest::new(
            path.to_string_lossy().into_owned(),
            source,
            solid_facts::compiler::CompilerOptions::default(),
        );
        match (dialect.compiler)().analyze(&request) {
            Err(solid_facts::compiler::CompilerProviderError::Refused(message)) => Some(message),
            _ => None,
        }
    };
    let outcome = prepare_program(
        project,
        selection,
        host,
        sources,
        modules,
        &excluded,
        dialect.vocabulary.ecosystem_scopes(),
        &refusal,
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
/// (`isServer`) fold as they do for the host's own constants, matching the browser
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

/// Review of ADR 0368: whether TypeScript owns an error at a project call is
/// a fact about the project's own program, not the dependency-source one.
/// The generated program adds package sources to the project's files, and
/// TypeScript's answers can depend on what a program holds and in what order
/// (a package augmenting a global interface reorders the merged overloads):
/// it can reject a call `tsc` accepts on the project, and accept one `tsc`
/// rejects. The generated program supplies package behaviour; a project
/// call's validity comes from the project's own program.
#[derive(Debug, Default)]
pub struct ProjectCallValidity {
    /// Project calls the generated program answers as errors and the
    /// project's own program does not, or answers nothing for: not
    /// TypeScript's, so uncertifiable (`ProjectFacts::dependency_source_call_errors`).
    pub generated_errors: Vec<typefacts::Location>,
    /// Per path, the entities with every project call the project's own
    /// program answers `Recovery` marked `Recovery`: TypeScript reports
    /// those, and the checker stays silent on them (invariant 1).
    pub tsc_owned: HashMap<String, Vec<typefacts::EntityFact>>,
    /// How many project calls were asked.
    pub asked: usize,
}

impl ProjectCallValidity {
    /// Whether applying this changes nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.generated_errors.is_empty() && self.tsc_owned.is_empty()
    }

    /// Takes the project's own validity into `facts`.
    pub fn apply(self, facts: &mut solid_facts::ProjectFacts) {
        facts.dependency_source_call_errors = self.generated_errors;
        if !self.tsc_owned.is_empty() {
            facts.typescript = facts
                .typescript
                .clone()
                .with_entity_overrides(self.tsc_owned);
        }
    }
}

/// The resolved calls in the project's own files of `facts` (the
/// dependency-source program's), sorted: what [`project_call_validity`]
/// asks the project's own program about.
#[must_use]
pub fn project_calls(
    facts: &solid_facts::ProjectFacts,
    program: &PreparedDependencyProgram,
) -> Vec<typefacts::Location> {
    let mut calls = facts
        .typescript
        .entities()
        .filter(|entity| {
            entity.resolved_call.is_some()
                && program
                    .project_files
                    .contains(Path::new(entity.location.path.as_ref()))
        })
        .map(|entity| entity.location.clone())
        .collect::<Vec<_>>();
    calls.sort_by(|left, right| {
        (&left.path, left.start_byte, left.end_byte).cmp(&(
            &right.path,
            right.start_byte,
            right.end_byte,
        ))
    });
    calls.dedup();
    calls
}

/// [`ProjectCallValidity`] for `facts` (the dependency-source program's):
/// `calls` ([`project_calls`]) and `own`, the validity the project's own
/// program answers for each (`TypeFactsSession::call_validities`).
#[must_use]
pub fn project_call_validity(
    facts: &solid_facts::ProjectFacts,
    calls: Vec<typefacts::Location>,
    own: Vec<Option<typefacts::ResolvedCallValidity>>,
) -> ProjectCallValidity {
    use typefacts::ResolvedCallValidity::Recovery;
    let mut answer = ProjectCallValidity {
        asked: calls.len(),
        ..ProjectCallValidity::default()
    };
    let own = calls.into_iter().zip(own).collect::<HashMap<_, _>>();
    let mut paths = own
        .keys()
        .map(|location| location.path.clone())
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    for path in paths {
        let entities = facts.typescript.entities_for_path(&path);
        let mut replaced = false;
        let mut patched = entities.to_vec();
        for entity in &mut patched {
            let Some(call) = entity.resolved_call.as_mut() else {
                continue;
            };
            let Some(own) = own.get(&entity.location) else {
                continue;
            };
            match (call.validity, *own) {
                (Recovery, Some(Recovery)) => {}
                (Recovery, _) | (_, None) => answer.generated_errors.push(entity.location.clone()),
                (_, Some(Recovery)) => {
                    std::sync::Arc::make_mut(call).validity = Recovery;
                    replaced = true;
                }
                _ => {}
            }
        }
        if replaced {
            answer.tsc_owned.insert(path.to_string(), patched);
        }
    }
    answer.generated_errors.sort_by(|left, right| {
        (&left.path, left.start_byte, left.end_byte).cmp(&(
            &right.path,
            right.start_byte,
            right.end_byte,
        ))
    });
    answer.generated_errors.dedup();
    answer
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    fn module(path: &str, root: Option<u32>) -> typefacts::ModuleFact {
        typefacts::ModuleFact {
            path: path.into(),
            root,
            format: typefacts::ModuleFormat::Unknown,
            declaration_file: false,
            redirect_targets: Vec::new().into(),
            project_reference: None,
        }
    }

    /// ADR 0368: TypeScript's root order first, not path order.
    #[test]
    fn roots_come_first_in_typescript_order() {
        let graph = typefacts::ModuleGraph {
            modules: vec![
                module("/p/node_modules/pkg/globals.d.ts", Some(2)),
                module("/p/node_modules/pkg/index.d.ts", None),
                module("/p/src/app.tsx", Some(1)),
                module("/p/src/main.tsx", Some(0)),
                module("/p/src/shared.ts", None),
            ],
            unmapped_roots: vec!["/p/src/lost".into()],
            ..typefacts::ModuleGraph::default()
        };
        let modules = super::in_root_order(&graph);
        assert_eq!(
            modules.roots,
            [
                "/p/src/main.tsx",
                "/p/src/app.tsx",
                "/p/node_modules/pkg/globals.d.ts",
            ]
            .map(PathBuf::from)
        );
        assert_eq!(
            modules.others,
            ["/p/node_modules/pkg/index.d.ts", "/p/src/shared.ts"].map(PathBuf::from)
        );
        assert_eq!(modules.unmapped_roots, ["/p/src/lost"]);
    }
}
