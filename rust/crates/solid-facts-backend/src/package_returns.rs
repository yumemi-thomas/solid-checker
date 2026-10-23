//! ADR 0372: what an installed package's exported function returns, for the
//! spreads that hand its result to an element.
//!
//! For each JSX spread of a call whose callee is a runtime import of a bare
//! specifier (`{...stylex.attrs(x)}`, `{...attrs(x)}`), the package's runtime
//! source file is the one the project loads, and the file's syntax answers
//! which own keys the export's every return can hold
//! (`AstFacts::returned_object_keys`). Which file loads is answered once per
//! check ([`EntrySelection`]): by the project's bundler when the check asked
//! it (ADR 0220), else by the importer's own resolution and ADR 0284's entry
//! selection, only where the project establishes the conditions that
//! selection reads (ADR 0297). Nothing else of the package is read, and it
//! does not enter the program. A package neither selects, a file that does
//! not parse, a module with a direct `eval`, a re-export or an export whose
//! returns are not fixed key sets records no row, and the spread keeps its
//! obligation.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use solid_facts::ProjectFacts;
use solid_facts::ast::{AstFacts, ImportFact, ModuleHazardKind};
use solid_facts::runtime_resolution::{RuntimeOutcome, RuntimeResolutionIndex};
use solid_reactive_ir::package_returns::PackageReturnKeys;
use solid_reactive_ir::{RuntimeBuild, RuntimeEnvironment, RuntimeTarget};

use crate::dependency_sources::{ExportsConditionsPremise, Host};

/// Which runtime file an import of a package loads, decided once per check.
#[derive(Debug)]
enum EntrySelection<'a> {
    /// ADR 0220: the check asked the project's bundler, whose table alone
    /// says where a load goes. It answers Vite's client development
    /// environment, so it serves only a declared browser development host
    /// with no condition of its own; a missing or non-file row withholds.
    Bundler(&'a RuntimeResolutionIndex),
    /// ADR 0284's entry selection, under the premise the implicit source
    /// selection needs (ADR 0297).
    Conditions(Host, ExportsConditionsPremise),
}

impl<'a> EntrySelection<'a> {
    /// `None` where neither answer is the project's: no declared target and
    /// build, a bundler table under a host it does not answer, or no Solid
    /// Vite plugin establishing the conditions `exports` is read with.
    fn for_check(
        project: &Path,
        facts: &'a ProjectFacts,
        runtime: &RuntimeEnvironment,
    ) -> Option<Self> {
        let host = crate::dependency_program::host(runtime).ok()?;
        if let Some(index) = &facts.runtime_resolutions {
            let client_development = runtime.target == Some(RuntimeTarget::Browser)
                && runtime.build == Some(RuntimeBuild::Development)
                && runtime.conditions.is_empty();
            return client_development.then_some(Self::Bundler(index));
        }
        let premise = crate::dependency_sources::exports_conditions_premise(project).ok()?;
        Some(Self::Conditions(host, premise))
    }

    /// The installed runtime file `import` in `importer` loads.
    fn entry(&self, importer: &str, import: &ImportFact) -> Option<PathBuf> {
        match self {
            Self::Bundler(index) => match index.outcome(importer, import.span, &import.module) {
                RuntimeOutcome::File { physical_path, .. } => {
                    let path = PathBuf::from(physical_path.as_ref());
                    path.components()
                        .any(|component| component.as_os_str() == "node_modules")
                        .then_some(path)
                }
                RuntimeOutcome::External | RuntimeOutcome::Builtin | RuntimeOutcome::Unknown => {
                    None
                }
            },
            Self::Conditions(host, premise) => crate::dependency_sources::installed_runtime_entry(
                Path::new(importer),
                &import.module,
                host,
                *premise,
            ),
        }
    }
}

/// The rows for every spread of an imported package call in `facts`, under
/// the declared host, for the project whose tsconfig is `project`. A check
/// whose [`EntrySelection`] selects no file records nothing.
#[must_use]
pub fn package_return_keys(
    project: &Path,
    facts: &ProjectFacts,
    runtime: &RuntimeEnvironment,
) -> PackageReturnKeys {
    let mut rows = PackageReturnKeys::default();
    let Some(selection) = EntrySelection::for_check(project, facts, runtime) else {
        return rows;
    };
    let mut modules = HashMap::<PathBuf, Option<(String, AstFacts)>>::new();
    let mut seen = HashSet::<(&str, String, String)>::new();
    for file in &facts.files {
        for spread in file
            .ast
            .jsx_elements
            .iter()
            .flat_map(|element| &element.spreads)
        {
            let argument = file.ast.peel_ts_sugar_span(spread.argument);
            let Some(call) = file.ast.call_at(argument) else {
                continue;
            };
            let Some((import, export)) = file.ast.imported_callee(&file.source, call.callee) else {
                continue;
            };
            let specifier = import.module.as_str();
            if !seen.insert((file.path.as_str(), specifier.to_owned(), export.to_string())) {
                continue;
            }
            let Some(entry) = selection.entry(file.path.as_str(), import) else {
                continue;
            };
            let module = modules.entry(entry.clone()).or_insert_with(|| {
                let source = fs::read_to_string(&entry).ok()?;
                let ast = solid_facts::ast::extract(entry.to_string_lossy().into_owned(), &source)
                    .ok()?;
                Some((source, ast))
            });
            let Some((source, ast)) = module else {
                continue;
            };
            // A direct `eval` can rebind any of the module's bindings.
            if ast
                .module_hazards
                .iter()
                .any(|hazard| hazard.kind == ModuleHazardKind::Eval)
            {
                continue;
            }
            let Some(keys) = ast
                .exported_function(&export)
                .and_then(|function| ast.returned_object_keys(source, function))
            else {
                continue;
            };
            rows.record(
                file.path.as_str(),
                specifier,
                export.as_str(),
                keys.into_iter().map(String::from),
            );
        }
    }
    rows
}

#[cfg(test)]
mod tests;
