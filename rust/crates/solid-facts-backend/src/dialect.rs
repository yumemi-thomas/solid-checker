//! The composition seam between dialect-independent infrastructure and one
//! Solid dialect.
//!
//! A [`Dialect`] bundles everything a Solid version contributes to the
//! checker: its vocabulary, its compiler adapter, its rule catalog, its rule
//! documentation, and its bundled package contracts, plus the stable identity
//! that keys every cache and retained session. The analysis pipeline receives
//! the selected `&Dialect` from its entry point — the CLI's `--dialect` flag,
//! the wasm request, or [`detect`] when a request names none — and never
//! names a dialect crate directly.

use std::path::Path;

use solid_facts::compiler::CompilerFactsProvider;
use solid_reactive_ir::{Finding, PackageContract, Program, RuleMetadata, SolveTimings};

use crate::BackendError;

pub struct Dialect {
    /// Stable identity, folded into every cache key and retained session
    /// identity so artifacts from different dialects can never collide.
    pub id: &'static str,
    /// The Solid-version vocabulary the reactive IR analyzes with: which
    /// names are primitives, where their callbacks sit, which JSX tags open
    /// boundaries. The engine asks this table; it never names a version.
    pub vocabulary: &'static dyn solid_dialect::Dialect,
    /// Size of the rule catalog; reporting only.
    pub rule_count: usize,
    /// Constructs the dialect's in-process compiler-facts provider.
    pub compiler: fn() -> Box<dyn CompilerFactsProvider>,
    /// Runs the dialect's rule catalog over a program.
    pub solve_measured: fn(&Program) -> (Vec<Finding>, SolveTimings),
    /// Documentation page for a rule, addressed by its externally visible
    /// name.
    pub docs_url: fn(&str) -> String,
    /// Identity of the finding reported when an imported package has no
    /// usable reactivity contract.
    pub contract_missing_rule: RuleMetadata,
    /// Package roots the dialect ships a bundled contract for; answers the
    /// cheap membership question without decoding any contract.
    pub bundled_packages: &'static [&'static str],
    /// The contract the dialect bundles for a package root, if any.
    pub bundled_contract: fn(&str) -> Result<Option<PackageContract>, BackendError>,
}

impl Dialect {
    pub fn solve(&self, program: &Program) -> Vec<Finding> {
        (self.solve_measured)(program).0
    }
}

/// Every dialect the checker can run with. A new dialect registers here and
/// becomes selectable by id everywhere a dialect can be named.
pub static ALL: [&Dialect; 2] = [&SOLID_V2, &SOLID_V1];

/// Resolves a dialect by its stable id.
#[must_use]
pub fn by_id(id: &str) -> Option<&'static Dialect> {
    ALL.iter().copied().find(|dialect| dialect.id == id)
}

/// The dialect entry points fall back to when a request names none and
/// nothing resolves.
#[must_use]
pub fn default_dialect() -> &'static Dialect {
    &SOLID_V2
}

/// The dialect for a Solid language version.
#[must_use]
pub fn by_version(version: solid_dialect::Version) -> &'static Dialect {
    match version {
        solid_dialect::Version::V1 => &SOLID_V1,
        solid_dialect::Version::V2 => &SOLID_V2,
    }
}

/// Resolves the dialect a project speaks from the `solid-js` it would
/// actually import: the nearest `node_modules/solid-js/package.json` above
/// the project file, walked the way a bundler resolves.
///
/// Deliberately **not** read from any loaded contract — a bundled contract
/// carries the version the checker ships, not the one the project installed.
/// Falls back to the default dialect when nothing resolves (no node_modules,
/// a non-version like `workspace:*`, or a major nobody has released), which
/// is what every request without an installed solid-js got before detection
/// existed.
#[must_use]
pub fn detect(project: &Path) -> &'static Dialect {
    resolved_solid_version(project).map_or_else(default_dialect, by_version)
}

fn resolved_solid_version(project: &Path) -> Option<solid_dialect::Version> {
    let start = if project.is_dir() {
        project
    } else {
        project.parent()?
    };
    for directory in start.ancestors() {
        let manifest = directory
            .join("node_modules")
            .join("solid-js")
            .join("package.json");
        let Ok(encoded) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        let version = serde_json::from_str::<serde_json::Value>(&encoded)
            .ok()
            .and_then(|manifest| manifest.get("version")?.as_str().map(str::to_owned))?;
        return solid_dialect::Version::for_solid_js(&version);
    }
    None
}

static SOLID_V2: Dialect = Dialect {
    id: "solid-v2",
    vocabulary: &solid_dialect::Solid2,
    rule_count: solid_v2_rules::Rule::ALL.len(),
    compiler: || Box::new(solid_v2_compiler::NativeCompilerFacts),
    solve_measured: solid_v2_rules::solve_measured,
    docs_url: solid_v2_rules::docs_url,
    contract_missing_rule: solid_v2_rules::Rule::PackageContractMissing.metadata(),
    bundled_packages: &["solid-js", "@solidjs/web"],
    bundled_contract: crate::diagnostics::bundled_contract_v2,
};

static SOLID_V1: Dialect = Dialect {
    id: "solid-v1",
    vocabulary: &solid_dialect::Solid1x,
    rule_count: solid_v1_rules::Rule::ALL.len(),
    compiler: || Box::new(solid_v1_compiler::NativeCompilerFacts),
    solve_measured: solid_v1_rules::solve_measured,
    docs_url: solid_v1_rules::docs_url,
    contract_missing_rule: solid_v1_rules::Rule::PackageContractMissing.metadata(),
    bundled_packages: &["solid-js"],
    bundled_contract: crate::diagnostics::bundled_contract_v1,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialect_ids_are_unique_and_resolvable() {
        for dialect in ALL {
            assert_eq!(by_id(dialect.id).map(|found| found.id), Some(dialect.id));
        }
        assert!(by_id("solid-v3").is_none());
    }

    #[test]
    fn detection_reads_the_resolved_solid_js_version() {
        let root = std::env::temp_dir().join(format!(
            "solid-checker-dialect-detect-{}",
            std::process::id()
        ));
        let package = root.join("node_modules/solid-js");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            package.join("package.json"),
            r#"{"name":"solid-js","version":"1.9.14"}"#,
        )
        .unwrap();
        let project = root.join("src/tsconfig.json");
        std::fs::write(&project, "{}").unwrap();
        assert_eq!(detect(&project).id, "solid-v1");

        std::fs::write(
            package.join("package.json"),
            r#"{"name":"solid-js","version":"2.0.0-beta.19"}"#,
        )
        .unwrap();
        assert_eq!(detect(&project).id, "solid-v2");

        // No resolvable version answers the default rather than guessing.
        std::fs::write(
            package.join("package.json"),
            r#"{"name":"solid-js","version":"workspace:*"}"#,
        )
        .unwrap();
        assert_eq!(detect(&project).id, default_dialect().id);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
