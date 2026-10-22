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
