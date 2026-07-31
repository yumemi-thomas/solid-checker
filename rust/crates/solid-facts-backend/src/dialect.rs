//! The composition seam between dialect-independent infrastructure and one
//! Solid dialect.
//!
//! A [`Dialect`] bundles everything a Solid version contributes to the
//! checker: its compiler adapter, its rule catalog, its rule documentation,
//! and its bundled package contracts, plus the stable identity that keys
//! every cache and retained session. The analysis pipeline receives the
//! selected `&Dialect` from its entry point — the CLI's `--dialect` flag or
//! the wasm request — and never names a dialect crate directly, so adding a
//! Solid 1.x dialect means constructing a second `Dialect` value and listing
//! it in [`ALL`]; no other backend code changes.

use solid_facts::compiler::CompilerFactsProvider;
use solid_reactive_ir::{Finding, PackageContract, Program, RuleMetadata, SolveTimings};

use crate::BackendError;

pub struct Dialect {
    /// Stable identity, folded into every cache key and retained session
    /// identity so artifacts from different dialects can never collide.
    pub id: &'static str,
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
pub static ALL: [&Dialect; 1] = [&SOLID_V2];

/// Resolves a dialect by its stable id.
#[must_use]
pub fn by_id(id: &str) -> Option<&'static Dialect> {
    ALL.iter().copied().find(|dialect| dialect.id == id)
}

/// The dialect entry points fall back to when a request names none.
#[must_use]
pub fn default_dialect() -> &'static Dialect {
    &SOLID_V2
}

static SOLID_V2: Dialect = Dialect {
    id: "solid-v2",
    rule_count: solid_v2_rules::Rule::ALL.len(),
    compiler: || Box::new(solid_v2_compiler::NativeCompilerFacts),
    solve_measured: solid_v2_rules::solve_measured,
    docs_url: solid_v2_rules::docs_url,
    contract_missing_rule: solid_v2_rules::Rule::PackageContractMissing.metadata(),
    bundled_packages: &["solid-js", "@solidjs/web"],
    bundled_contract: crate::diagnostics::bundled_contract_v2,
};
