//! The package boundary: the imports of a project whose package behavior the
//! analysis cannot read, each with every reason it was refused.
//!
//! This is a normalized project-analysis input, not an AST fact and not an
//! acceptance document. The backend owns acquisition, filesystem selection
//! and the construction of every reason, at the decision site that made it;
//! this index only stores and answers them. A WASM adapter can build the same
//! input with no filesystem.
//!
//! **Absence is not evidence.** An import with no row here is one this input
//! raises no objection to; source, dialect or contract facts still have to
//! prove what it does. Nothing here certifies a package, and a reason recorded
//! for one installed copy or subpath is never read for another.
//!
//! Phase P3 of the contract-removal plan records reasons beside the existing
//! contract requirements and moves no finding: the [`LegacyRequirements`]
//! projection holds the exact keys that controlled the acceptance gate before
//! the boundary existed, and only those keys reach it. The recorded reasons
//! are explanation until the phase that rewords that gate.
//!
//! A **source-edge refusal** is the one record here that does move a finding:
//! the caller's statement that source-boundary analysis was withheld for one
//! exact occurrence of an import specifier. See [`SourceEdgeSite`]. It is the
//! caller's decision, stored beside the package identities and never inside
//! them; this module neither discovers it nor authenticates it.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest as _, Sha256};

/// Why an import keeps an open requirement, as the requirement discovery
/// recorded it.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum UncertifiableImportReason {
    Unspecified,
    ObsoletePolicy1,
}

/// One import occurrence: the importing file, in the canonical project
/// identity Type Facts uses, and the specifier as written, subpath and alias
/// included. Two installations or two subpaths of one package are two keys.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ImportKey {
    pub importer: String,
    pub specifier: String,
}

impl ImportKey {
    #[must_use]
    pub fn new(importer: impl Into<String>, specifier: impl Into<String>) -> Self {
        Self {
            importer: importer.into(),
            specifier: specifier.into(),
        }
    }
}

/// An exact source range. A producer that knows no exact site records none;
/// absence never licenses choosing a nearby one.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BoundarySite {
    pub path: String,
    pub start_byte: u64,
    pub end_byte: u64,
}

/// How a project file names the specifier of a source-edge refusal. The
/// spellings are the stable context suffixes of the finding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceEdgeForm {
    /// `import … from "m"`.
    Import,
    /// `export … from "m"`, including `export * from` and `export * as ns from`.
    ExportFrom,
    /// `import S = require("m")`, exported or not.
    ImportEquals,
    /// `import("m")` with a literal specifier.
    DynamicImport,
    /// `require("m")` with a literal specifier and an unshadowed `require`.
    Require,
}

impl SourceEdgeForm {
    /// A stable spelling, for contexts and error text; not a wire value.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Import => "import",
            Self::ExportFrom => "export-from",
            Self::ImportEquals => "import-equals",
            Self::DynamicImport => "dynamic-import",
            Self::Require => "require",
        }
    }

    /// The one-byte tag the analysis identity hashes. Explicit, so reordering
    /// the variants can never move an identity.
    const fn tag(self) -> u8 {
        match self {
            Self::Import => 0,
            Self::ExportFrom => 1,
            Self::ImportEquals => 2,
            Self::DynamicImport => 3,
            Self::Require => 4,
        }
    }

    /// Whether the contract-era binder ever bound this form to an accepted
    /// contract. Only these forms deny an accepted contract; the others are
    /// recorded and reported but never had a contract binding to withhold.
    #[must_use]
    pub const fn is_contract_bound(self) -> bool {
        matches!(self, Self::Import | Self::ExportFrom)
    }
}

impl std::fmt::Display for SourceEdgeForm {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

/// One exact occurrence: its form and the byte range of the whole statement
/// or expression, as the facts of the same generation report it. Multiplicity
/// is the set: two statements are two sites, and the same range under two
/// forms is two sites.
///
/// A site carries no reason, package identity or generation. It is joined to
/// the supplied facts by exact membership and by nothing else, so the caller
/// must reacquire its records for each coherent facts generation; an equal
/// range in a changed generation joins exactly as before.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceEdgeSite {
    pub form: SourceEdgeForm,
    pub start_byte: u64,
    pub end_byte: u64,
}

/// A recorded source-edge refusal that the supplied facts do not support, or
/// that the analysis cannot honor exactly. Each variant is a statement about
/// the join, never about the module, an export or a runtime target: a failed
/// join proves none of them missing.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SourceEdgeMismatch {
    #[error(
        "a source-edge refusal names {importer:?}, which is not a file of the facts supplied with this analysis"
    )]
    UnknownImporter { importer: String },
    #[error(
        "a source-edge refusal names {importer:?}, a file of a dependency source root, whose findings are not reported"
    )]
    ImporterInDependencySource { importer: String },
    #[error(
        "a source-edge refusal for {specifier:?} in {importer:?} names a {form} occurrence at bytes {start_byte}..{end_byte}, and the supplied facts hold no such occurrence"
    )]
    NoSuchOccurrence {
        importer: String,
        specifier: String,
        form: SourceEdgeForm,
        start_byte: u64,
        end_byte: u64,
    },
    #[error(
        "a source-edge refusal for {specifier:?} in {importer:?} leaves the {form} occurrence at bytes {start_byte}..{end_byte} unrecorded, while it denies the accepted contract of the whole key"
    )]
    UnrecordedOccurrence {
        importer: String,
        specifier: String,
        form: SourceEdgeForm,
        start_byte: u64,
        end_byte: u64,
    },
    #[error(
        "the runtime namespace import of {specifier:?} in {importer:?} at bytes {start_byte}..{end_byte} cannot be recorded, and cannot share a key whose accepted contract is denied"
    )]
    UnsupportedOccurrence {
        importer: String,
        specifier: String,
        start_byte: u64,
        end_byte: u64,
    },
    #[error(
        "the import of {specifier:?} in {importer:?} at bytes {start_byte}..{end_byte} has an unreferenced binding and is not a supported occurrence, so it cannot share a key whose accepted contract is denied: the denial would withhold an obligation the analysis could otherwise report there"
    )]
    UnreferencedCollateral {
        importer: String,
        specifier: String,
        start_byte: u64,
        end_byte: u64,
    },
}

/// One export demanded through a refused import: a named or namespace member
/// use, or a re-export, at its exact occurrence.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExportDemandKey {
    pub export: String,
    /// The export's exact declaration, when one is known.
    pub declaration: Option<BoundarySite>,
    pub use_site: BoundarySite,
}

/// One cause. `detail` is the producer's exact explanation; consumers never
/// parse it.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BoundaryReason {
    pub kind: UnanalyzableReason,
    /// The requesting or offending edge, when the producer knows it exactly.
    pub origin: Option<BoundarySite>,
    pub detail: String,
}

impl BoundaryReason {
    #[must_use]
    pub fn new(kind: UnanalyzableReason, detail: impl Into<String>) -> Self {
        Self {
            kind,
            origin: None,
            detail: detail.into(),
        }
    }

    #[must_use]
    pub fn at(mut self, origin: BoundarySite) -> Self {
        self.origin = Some(origin);
        self
    }
}

/// Why a package's behavior at one import cannot be read.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum UnanalyzableReason {
    /// No complete execution host to select package files for.
    HostUnavailable(HostFailure),
    /// No resolver the project's build would use could be identified.
    ResolverPremiseUnavailable,
    /// The package could be read but was not selected for analysis.
    NotSelected(SelectionFailure),
    /// The import resolves to no exact installed runtime root.
    ResolutionUnavailable,
    /// The installed manifest cannot be read.
    ManifestUnavailable,
    /// No runtime entry for the subpath under the host's conditions.
    RuntimeEntryUnavailable,
    /// The selected entry is not a source file inside the package.
    RuntimeSourceUnavailable,
    /// A file of the package's closure cannot be read, parsed or resolved.
    ClosureUnavailable,
    /// The package's closure exceeds the selection limit.
    ClosureLimit,
    /// A binding form whose source join is not exact.
    UnsupportedBinding(BindingForm),
    /// Selected source whose export cannot be joined exactly.
    ExportJoinUnavailable(ExportJoinFailure),
    /// The analyzed program could not be prepared.
    ProgramPreparationUnavailable,
    /// The exact requirement the contract era raised, carrying no source
    /// verdict. Retired with the contract tiers.
    LegacyContractFallback,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostFailure {
    TargetMissing,
    BuildMissing,
    InferenceRefused,
    MixedExecution,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SelectionFailure {
    NoDemand,
    WorkspaceOutsideProgram,
    RetainedCopy,
    RetainedExportedValue,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BindingForm {
    NamespaceValue,
    NamespaceReexport,
    ImportEquals,
    DynamicImport,
    Require,
    OpaqueInventory,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExportJoinFailure {
    MissingFacts,
    AmbiguousProvider,
    WrongInstallation,
    ConflictingTargets,
}

impl UnanalyzableReason {
    /// A stable spelling, for identities and tests; not a public wire value.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::HostUnavailable(HostFailure::TargetMissing) => "host-unavailable:target-missing",
            Self::HostUnavailable(HostFailure::BuildMissing) => "host-unavailable:build-missing",
            Self::HostUnavailable(HostFailure::InferenceRefused) => {
                "host-unavailable:inference-refused"
            }
            Self::HostUnavailable(HostFailure::MixedExecution) => {
                "host-unavailable:mixed-execution"
            }
            Self::ResolverPremiseUnavailable => "resolver-premise-unavailable",
            Self::NotSelected(SelectionFailure::NoDemand) => "not-selected:no-demand",
            Self::NotSelected(SelectionFailure::WorkspaceOutsideProgram) => {
                "not-selected:workspace-outside-program"
            }
            Self::NotSelected(SelectionFailure::RetainedCopy) => "not-selected:retained-copy",
            Self::NotSelected(SelectionFailure::RetainedExportedValue) => {
                "not-selected:retained-exported-value"
            }
            Self::ResolutionUnavailable => "resolution-unavailable",
            Self::ManifestUnavailable => "manifest-unavailable",
            Self::RuntimeEntryUnavailable => "runtime-entry-unavailable",
            Self::RuntimeSourceUnavailable => "runtime-source-unavailable",
            Self::ClosureUnavailable => "closure-unavailable",
            Self::ClosureLimit => "closure-limit",
            Self::UnsupportedBinding(BindingForm::NamespaceValue) => {
                "unsupported-binding:namespace-value"
            }
            Self::UnsupportedBinding(BindingForm::NamespaceReexport) => {
                "unsupported-binding:namespace-reexport"
            }
            Self::UnsupportedBinding(BindingForm::ImportEquals) => {
                "unsupported-binding:import-equals"
            }
            Self::UnsupportedBinding(BindingForm::DynamicImport) => {
                "unsupported-binding:dynamic-import"
            }
            Self::UnsupportedBinding(BindingForm::Require) => "unsupported-binding:require",
            Self::UnsupportedBinding(BindingForm::OpaqueInventory) => {
                "unsupported-binding:opaque-inventory"
            }
            Self::ExportJoinUnavailable(ExportJoinFailure::MissingFacts) => {
                "export-join-unavailable:missing-facts"
            }
            Self::ExportJoinUnavailable(ExportJoinFailure::AmbiguousProvider) => {
                "export-join-unavailable:ambiguous-provider"
            }
            Self::ExportJoinUnavailable(ExportJoinFailure::WrongInstallation) => {
                "export-join-unavailable:wrong-installation"
            }
            Self::ExportJoinUnavailable(ExportJoinFailure::ConflictingTargets) => {
                "export-join-unavailable:conflicting-targets"
            }
            Self::ProgramPreparationUnavailable => "program-preparation-unavailable",
            Self::LegacyContractFallback => "legacy-contract-fallback",
        }
    }
}

/// One refused import: the resolved identity, when known, and every cause.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UnanalyzableImport {
    /// The resolved package name. Identity, never authority: it selects no
    /// contract and blames no other copy.
    pub package_name: Option<String>,
    pub installed_root: Option<String>,
    package_name_stated: bool,
    installed_root_stated: bool,
    /// Import-wide causes, sorted and without exact duplicates.
    reasons: Vec<BoundaryReason>,
    /// Causes that refuse one demanded export only.
    exports: BTreeMap<ExportDemandKey, Vec<BoundaryReason>>,
}

impl UnanalyzableImport {
    #[must_use]
    pub fn reasons(&self) -> &[BoundaryReason] {
        &self.reasons
    }

    #[must_use]
    pub const fn exports(&self) -> &BTreeMap<ExportDemandKey, Vec<BoundaryReason>> {
        &self.exports
    }

    /// Keeps the first identity a producer stated; a second producer naming
    /// another one makes it unknown rather than either.
    fn identify(&mut self, package_name: Option<&str>, installed_root: Option<&str>) {
        fn merge(slot: &mut Option<String>, seen: &mut bool, value: Option<&str>) {
            let Some(value) = value else { return };
            match slot {
                None if !*seen => *slot = Some(value.to_owned()),
                Some(existing) if existing != value => *slot = None,
                _ => {}
            }
            *seen = true;
        }
        merge(
            &mut self.package_name,
            &mut self.package_name_stated,
            package_name,
        );
        merge(
            &mut self.installed_root,
            &mut self.installed_root_stated,
            installed_root,
        );
    }
}

fn insert_reason(reasons: &mut Vec<BoundaryReason>, reason: BoundaryReason) {
    if let Err(position) = reasons.binary_search(&reason) {
        reasons.insert(position, reason);
    }
}

/// The exact requirement keys the contract-era acceptance gate read, and the
/// per-import explanation overrides contract admission replayed. Transitional:
/// it exists so the boundary can be the producer of those keys without moving
/// a finding, and goes with the contract tiers.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct LegacyRequirements {
    markers: BTreeMap<ImportKey, UncertifiableImportReason>,
    /// `None` is an answer: this importer's install refused nothing, so an
    /// explanation replayed from another copy is not its explanation.
    refusal_overrides: BTreeMap<ImportKey, Option<String>>,
}

/// See the module documentation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackageBoundary {
    imports: BTreeMap<ImportKey, UnanalyzableImport>,
    legacy: LegacyRequirements,
    /// Exact occurrences whose source-boundary analysis the caller withheld.
    /// Apart from `imports` on purpose: it creates no [`UnanalyzableImport`],
    /// so `identify`'s conflict history and every package identity stay what
    /// they were, and `import` answers `None` for a key that only has sites.
    source_edges: BTreeMap<ImportKey, BTreeSet<SourceEdgeSite>>,
}

impl PackageBoundary {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.imports.is_empty()
            && self.legacy.markers.is_empty()
            && self.legacy.refusal_overrides.is_empty()
            && self.source_edges.is_empty()
    }

    /// Records one import-wide cause, with the identity the producer resolved.
    pub fn record(
        &mut self,
        key: ImportKey,
        package_name: Option<&str>,
        installed_root: Option<&str>,
        reason: BoundaryReason,
    ) {
        let import = self.imports.entry(key).or_default();
        import.identify(package_name, installed_root);
        insert_reason(&mut import.reasons, reason);
    }

    /// Records one cause that refuses a single demanded export.
    pub fn record_export(
        &mut self,
        key: ImportKey,
        export: ExportDemandKey,
        reason: BoundaryReason,
    ) {
        let import = self.imports.entry(key).or_default();
        insert_reason(import.exports.entry(export).or_default(), reason);
    }

    #[must_use]
    pub fn import(&self, importer: &str, specifier: &str) -> Option<&UnanalyzableImport> {
        self.imports.get(&ImportKey::new(importer, specifier))
    }

    pub fn imports(&self) -> impl Iterator<Item = (&ImportKey, &UnanalyzableImport)> {
        self.imports.iter()
    }

    /// Records that source-boundary analysis was withheld for one exact
    /// occurrence. Valid only for the facts generation the caller read the
    /// site from; the analysis checks it against the facts it is given.
    pub fn record_source_edge_refusal(&mut self, key: ImportKey, site: SourceEdgeSite) {
        self.source_edges.entry(key).or_default().insert(site);
    }

    /// Every recorded key with its sites, in key order and then site order.
    pub fn source_edge_refusals(
        &self,
    ) -> impl Iterator<Item = (&ImportKey, &BTreeSet<SourceEdgeSite>)> {
        self.source_edges.iter()
    }

    #[must_use]
    pub fn source_edge_sites(
        &self,
        importer: &str,
        specifier: &str,
    ) -> Option<&BTreeSet<SourceEdgeSite>> {
        self.source_edges.get(&ImportKey::new(importer, specifier))
    }

    /// Whether any source-edge refusal is recorded. The analysis does no work
    /// for this input when it is false.
    #[must_use]
    pub fn has_source_edge_refusals(&self) -> bool {
        !self.source_edges.is_empty()
    }

    /// The recorded keys of one importer, in specifier order.
    pub(crate) fn source_edges_of<'a>(
        &'a self,
        importer: &'a str,
    ) -> impl Iterator<Item = (&'a ImportKey, &'a BTreeSet<SourceEdgeSite>)> + 'a {
        self.source_edges
            .range(ImportKey::new(importer, "")..)
            .take_while(move |(key, _)| key.importer == importer)
    }

    /// Adds `key` to the contract-era requirement set. A later reason for the
    /// same key replaces an earlier one, as the acceptance index's did.
    pub fn require_legacy(&mut self, key: ImportKey, reason: UncertifiableImportReason) {
        self.legacy.markers.insert(key, reason);
    }

    #[must_use]
    pub fn legacy_reason(
        &self,
        importer: &str,
        specifier: &str,
    ) -> Option<UncertifiableImportReason> {
        self.legacy
            .markers
            .get(&ImportKey::new(importer, specifier))
            .copied()
    }

    pub fn legacy_requirements(
        &self,
    ) -> impl Iterator<Item = (&ImportKey, UncertifiableImportReason)> {
        self.legacy
            .markers
            .iter()
            .map(|(key, reason)| (key, *reason))
    }

    /// Records the explanation contract admission replayed for one importer's
    /// own install. The first answer for a key stands.
    pub fn override_refusal(&mut self, key: ImportKey, refusal: Option<String>) {
        self.legacy.refusal_overrides.entry(key).or_insert(refusal);
    }

    /// `None`: no override for this importer. `Some(None)`: its install
    /// refused nothing, which is itself the answer.
    #[must_use]
    pub fn refusal_override(&self, importer: &str, specifier: &str) -> Option<Option<&str>> {
        self.legacy
            .refusal_overrides
            .get(&ImportKey::new(importer, specifier))
            .map(Option::as_deref)
    }

    /// Folds `other` in: every cause of both, and `self`'s requirement and
    /// override answers before `other`'s. Source-edge sites are unioned.
    pub fn extend(&mut self, other: Self) {
        for (key, import) in other.imports {
            let entry = self.imports.entry(key).or_default();
            entry.identify(
                import.package_name.as_deref(),
                import.installed_root.as_deref(),
            );
            for reason in import.reasons {
                insert_reason(&mut entry.reasons, reason);
            }
            for (export, reasons) in import.exports {
                let slot = entry.exports.entry(export).or_default();
                for reason in reasons {
                    insert_reason(slot, reason);
                }
            }
        }
        for (key, reason) in other.legacy.markers {
            self.legacy.markers.entry(key).or_insert(reason);
        }
        for (key, refusal) in other.legacy.refusal_overrides {
            self.legacy.refusal_overrides.entry(key).or_insert(refusal);
        }
        for (key, sites) in other.source_edges {
            self.source_edges.entry(key).or_default().extend(sites);
        }
    }

    /// Everything a finding or cache entry can depend on, causes included: a
    /// changed reason must invalidate a cached answer although no file moved.
    pub fn fingerprint(&self, hash: &mut Sha256) {
        fn text(hash: &mut Sha256, value: &str) {
            hash.update((value.len() as u64).to_be_bytes());
            hash.update(value.as_bytes());
        }
        fn optional(hash: &mut Sha256, value: Option<&str>) {
            match value {
                Some(value) => {
                    hash.update([1]);
                    text(hash, value);
                }
                None => hash.update([0]),
            }
        }
        fn site(hash: &mut Sha256, site: Option<&BoundarySite>) {
            match site {
                Some(site) => {
                    hash.update([1]);
                    text(hash, &site.path);
                    hash.update(site.start_byte.to_be_bytes());
                    hash.update(site.end_byte.to_be_bytes());
                }
                None => hash.update([0]),
            }
        }
        fn reasons(hash: &mut Sha256, reasons: &[BoundaryReason]) {
            hash.update((reasons.len() as u64).to_be_bytes());
            for reason in reasons {
                text(hash, reason.kind.code());
                site(hash, reason.origin.as_ref());
                text(hash, &reason.detail);
            }
        }
        hash.update(b"package-boundary-v1");
        hash.update((self.imports.len() as u64).to_be_bytes());
        for (key, import) in &self.imports {
            text(hash, &key.importer);
            text(hash, &key.specifier);
            optional(hash, import.package_name.as_deref());
            optional(hash, import.installed_root.as_deref());
            reasons(hash, &import.reasons);
            hash.update((import.exports.len() as u64).to_be_bytes());
            for (export, causes) in &import.exports {
                text(hash, &export.export);
                site(hash, export.declaration.as_ref());
                site(hash, Some(&export.use_site));
                reasons(hash, causes);
            }
        }
        hash.update((self.legacy.markers.len() as u64).to_be_bytes());
        for (key, reason) in &self.legacy.markers {
            text(hash, &key.importer);
            text(hash, &key.specifier);
            hash.update([match reason {
                UncertifiableImportReason::Unspecified => 0,
                UncertifiableImportReason::ObsoletePolicy1 => 1,
            }]);
        }
        hash.update((self.legacy.refusal_overrides.len() as u64).to_be_bytes());
        for (key, refusal) in &self.legacy.refusal_overrides {
            text(hash, &key.importer);
            text(hash, &key.specifier);
            optional(hash, refusal.as_deref());
        }
        // Nothing is hashed for a boundary with no source-edge records, so
        // every boundary keeps the digest it had before they existed.
        if !self.source_edges.is_empty() {
            hash.update(b"source-edge-refusals-v1");
            hash.update((self.source_edges.len() as u64).to_be_bytes());
            for (key, sites) in &self.source_edges {
                text(hash, &key.importer);
                text(hash, &key.specifier);
                hash.update((sites.len() as u64).to_be_bytes());
                for site in sites {
                    hash.update([site.form.tag()]);
                    hash.update(site.start_byte.to_be_bytes());
                    hash.update(site.end_byte.to_be_bytes());
                }
            }
        }
    }

    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        self.fingerprint(&mut hash);
        hash.finalize().into()
    }
}

#[cfg(test)]
mod tests;
