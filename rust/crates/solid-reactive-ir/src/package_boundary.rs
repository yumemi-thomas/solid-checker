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
//! contract requirements and moves no finding: the legacy markers hold the
//! exact keys that controlled the acceptance gate before the boundary existed,
//! and only those keys reach it. The recorded reasons
//! are explanation until the phase that rewords that gate.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest as _, Sha256};

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
}

impl UnanalyzableImport {
    #[must_use]
    pub fn reasons(&self) -> &[BoundaryReason] {
        &self.reasons
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

/// See the module documentation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackageBoundary {
    imports: BTreeMap<ImportKey, UnanalyzableImport>,
    /// The exact requirement keys the contract-era acceptance gate read.
    /// Transitional: it exists so the boundary can be the producer of those
    /// keys without moving a finding, and goes with the contract tiers.
    legacy_markers: BTreeSet<ImportKey>,
}

impl PackageBoundary {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.imports.is_empty() && self.legacy_markers.is_empty()
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

    #[must_use]
    pub fn import(&self, importer: &str, specifier: &str) -> Option<&UnanalyzableImport> {
        self.imports.get(&ImportKey::new(importer, specifier))
    }

    pub fn imports(&self) -> impl Iterator<Item = (&ImportKey, &UnanalyzableImport)> {
        self.imports.iter()
    }

    /// Adds `key` to the contract-era requirement set.
    pub fn require_legacy(&mut self, key: ImportKey) {
        self.legacy_markers.insert(key);
    }

    #[must_use]
    pub fn requires_legacy(&self, importer: &str, specifier: &str) -> bool {
        self.legacy_markers
            .contains(&ImportKey::new(importer, specifier))
    }

    /// Folds `other` in: every cause of both, and every requirement.
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
        }
        self.legacy_markers.extend(other.legacy_markers);
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
        hash.update(b"package-boundary-v2");
        hash.update((self.imports.len() as u64).to_be_bytes());
        for (key, import) in &self.imports {
            text(hash, &key.importer);
            text(hash, &key.specifier);
            optional(hash, import.package_name.as_deref());
            optional(hash, import.installed_root.as_deref());
            reasons(hash, &import.reasons);
        }
        hash.update((self.legacy_markers.len() as u64).to_be_bytes());
        for key in &self.legacy_markers {
            text(hash, &key.importer);
            text(hash, &key.specifier);
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
