//! Snapshot-owned replay of one package's finite module closure.
//!
//! Paths and bytes come only from [`ArtifactSnapshot`]. The caller's closure
//! is comparison material: accepted dependency identities are retained only
//! when an observed external edge names them, and every local file, edge, and
//! syntax hazard is rebuilt before equality is checked.

use std::collections::BTreeSet;

use sha2::{Digest as _, Sha256};
use solid_facts::ast::{ModuleHazardKind, ModuleLoadKind, extract};

use crate::artifact_resolution::{
    AcceptedDependencyEdge, AffectedClaimDomain, ClosureEntry, ClosureFileRole, ClosureHazard,
    ClosureHazardKind, ClosureManifest,
};

use super::{ArtifactSnapshot, ArtifactSnapshotError, SnapshotVerifiedResolution};

const RUNTIME_EXTENSIONS: [&str; 8] =
    [".js", ".mjs", ".cjs", ".jsx", ".ts", ".mts", ".cts", ".tsx"];
const DECLARATION_EXTENSIONS: [&str; 3] = [".d.ts", ".d.mts", ".d.cts"];
const DECLARATION_MODULE_EXTENSIONS: [&str; 11] = [
    ".ts", ".tsx", ".d.ts", ".mts", ".d.mts", ".cts", ".d.cts", ".js", ".jsx", ".mjs", ".cjs",
];

#[derive(Clone, Debug)]
pub struct SnapshotVerifiedClosure {
    snapshot_root: String,
    manifest: ClosureManifest,
}

impl SnapshotVerifiedClosure {
    #[must_use]
    pub fn snapshot_root(&self) -> &str {
        &self.snapshot_root
    }

    #[must_use]
    pub const fn manifest(&self) -> &ClosureManifest {
        &self.manifest
    }
}

pub(super) fn verify_snapshot_closure(
    snapshot: &ArtifactSnapshot,
    resolution: &SnapshotVerifiedResolution,
    supplied: &ClosureManifest,
) -> Result<SnapshotVerifiedClosure, ArtifactSnapshotError> {
    if !supplied.packages.is_empty() {
        return closure_mismatch(
            "a package census cannot stand in for the selected artifact's file-edge closure",
        );
    }
    if supplied
        .entries
        .iter()
        .any(|entry| entry.role == ClosureFileRole::Generated)
    {
        return closure_mismatch(
            "generated closure bytes cannot be reconstructed from the package snapshot",
        );
    }

    let rebuilt = replay_snapshot_closure(snapshot, resolution, &supplied.dependencies)?;
    if &rebuilt != supplied {
        return closure_mismatch(format!(
            "supplied closure {} does not equal snapshot replay {}; diff={}",
            supplied.digest,
            rebuilt.digest,
            closure_difference(supplied, &rebuilt)
        ));
    }
    Ok(SnapshotVerifiedClosure {
        snapshot_root: snapshot.root().into(),
        manifest: rebuilt,
    })
}

fn closure_difference(supplied: &ClosureManifest, replayed: &ClosureManifest) -> String {
    fn set_difference<T>(left: &[T], right: &[T]) -> serde_json::Value
    where
        T: Ord + serde::Serialize,
    {
        let left = left.iter().collect::<BTreeSet<_>>();
        let right = right.iter().collect::<BTreeSet<_>>();
        let count = left.difference(&right).count();
        let sample = left
            .difference(&right)
            .take(8)
            .filter_map(|value| serde_json::to_value(*value).ok())
            .collect::<Vec<_>>();
        serde_json::json!({ "count": count, "sample": sample })
    }

    serde_json::json!({
        "suppliedOnly": {
            "entries": set_difference(&supplied.entries, &replayed.entries),
            "dependencies": set_difference(&supplied.dependencies, &replayed.dependencies),
            "hazards": set_difference(&supplied.hazards, &replayed.hazards),
        },
        "replayedOnly": {
            "entries": set_difference(&replayed.entries, &supplied.entries),
            "dependencies": set_difference(&replayed.dependencies, &supplied.dependencies),
            "hazards": set_difference(&replayed.hazards, &supplied.hazards),
        }
    })
    .to_string()
}

pub(super) fn replay_snapshot_closure(
    snapshot: &ArtifactSnapshot,
    resolution: &SnapshotVerifiedResolution,
    supplied_dependencies: &[AcceptedDependencyEdge],
) -> Result<ClosureManifest, ArtifactSnapshotError> {
    let mut replay = ClosureReplay {
        snapshot,
        supplied_dependencies,
        entries: Vec::new(),
        dependencies: Vec::new(),
        hazards: Vec::new(),
        visited: BTreeSet::new(),
    };
    replay.add_entry(ClosureFileRole::Manifest, "package.json")?;
    replay.add_entry(ClosureFileRole::ResolutionInput, "package.json")?;
    replay.visit(
        resolution.runtime_path(),
        ModuleAxis::Runtime,
        ClosureFileRole::Runtime,
    )?;
    replay.visit(
        resolution.declarations_path(),
        ModuleAxis::Declarations,
        ClosureFileRole::Declaration,
    )?;

    ClosureManifest::new(replay.entries, replay.dependencies, replay.hazards)
        .map_err(|error| ArtifactSnapshotError::ModuleClosure(error.to_string()))
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum ModuleAxis {
    Runtime,
    Declarations,
}

struct ClosureReplay<'a> {
    snapshot: &'a ArtifactSnapshot,
    supplied_dependencies: &'a [AcceptedDependencyEdge],
    entries: Vec<ClosureEntry>,
    dependencies: Vec<AcceptedDependencyEdge>,
    hazards: Vec<ClosureHazard>,
    visited: BTreeSet<(ModuleAxis, ClosureFileRole, String)>,
}

impl ClosureReplay<'_> {
    fn visit(
        &mut self,
        path: &str,
        axis: ModuleAxis,
        role: ClosureFileRole,
    ) -> Result<(), ArtifactSnapshotError> {
        if !self.visited.insert((axis, role, path.into())) {
            return Ok(());
        }
        self.add_entry(role, path)?;
        let bytes = self.snapshot.read(path).ok_or_else(|| {
            ArtifactSnapshotError::ModuleClosure(format!(
                "reachable module {path:?} is absent from the snapshot"
            ))
        })?;
        let source = std::str::from_utf8(bytes).map_err(|_| {
            ArtifactSnapshotError::ModuleClosure(format!(
                "reachable module {path:?} is not valid UTF-8"
            ))
        })?;
        let facts = extract(format!("./{path}"), source).map_err(|error| {
            ArtifactSnapshotError::ModuleClosure(format!(
                "reachable module {path:?} cannot be parsed: {error}"
            ))
        })?;

        for hazard in facts.module_hazards {
            self.hazards.push(ClosureHazard {
                kind: match hazard.kind {
                    ModuleHazardKind::NonliteralDynamicLoading => {
                        ClosureHazardKind::NonliteralDynamicLoading
                    }
                    ModuleHazardKind::Eval => ClosureHazardKind::Eval,
                    ModuleHazardKind::OpaqueWasm => ClosureHazardKind::OpaqueWasm,
                    ModuleHazardKind::MutableUnboundGlobal => {
                        ClosureHazardKind::MutableUnboundGlobal
                    }
                },
                source: format!("./{path}:{}-{}", hazard.span.start, hazard.span.end),
                affected_exports: Vec::new(),
                affected_domains: all_domains(),
            });
        }

        let static_specifiers = facts
            .imports
            .into_iter()
            .map(|fact| fact.module.into_string())
            .chain(
                facts
                    .exports
                    .into_iter()
                    .filter_map(|fact| fact.module.map(|module| module.into_string())),
            )
            .collect::<Vec<_>>();
        for specifier in static_specifiers {
            self.visit_specifier(path, axis, role, &specifier, false)?;
        }
        for load in facts.module_loads {
            let Some(specifier) = load.specifier else {
                continue;
            };
            self.visit_specifier(
                path,
                axis,
                role,
                &specifier,
                load.kind == ModuleLoadKind::DynamicImport,
            )?;
        }
        Ok(())
    }

    fn visit_specifier(
        &mut self,
        importer: &str,
        axis: ModuleAxis,
        current_role: ClosureFileRole,
        specifier: &str,
        dynamic_import: bool,
    ) -> Result<(), ArtifactSnapshotError> {
        if dynamic_import && (specifier.ends_with(".node") || specifier.ends_with(".wasm")) {
            self.hazards.push(ClosureHazard {
                kind: if specifier.ends_with(".node") {
                    ClosureHazardKind::NativeCode
                } else {
                    ClosureHazardKind::OpaqueWasm
                },
                source: format!("./{importer}:{specifier}"),
                affected_exports: Vec::new(),
                affected_domains: all_domains(),
            });
            return Ok(());
        }

        match resolve_local(self.snapshot, importer, specifier, axis)? {
            LocalResolution::Module(target) => {
                let role = if dynamic_import && axis == ModuleAxis::Runtime {
                    ClosureFileRole::LiteralDynamicChunk
                } else {
                    current_role
                };
                self.visit(&target, axis, role)
            }
            LocalResolution::Asset(target) => {
                self.add_entry(ClosureFileRole::ResolutionInput, &target)
            }
            // Bundler-mediated asset import. It never names a package, so it
            // never matches a supplied dependency identity; record the opaque
            // frontier directly. See `bundler_resource_suffix`.
            LocalResolution::OpaqueAsset => {
                self.record_opaque_frontier(importer, specifier);
                Ok(())
            }
            LocalResolution::External => self.record_external(importer, specifier),
            LocalResolution::Missing => closure_mismatch(format!(
                "local closure module {specifier:?} from {importer:?} was not found"
            )),
        }
    }

    fn record_external(
        &mut self,
        importer: &str,
        specifier: &str,
    ) -> Result<(), ArtifactSnapshotError> {
        let matches = self
            .supplied_dependencies
            .iter()
            .filter(|dependency| dependency.specifier == specifier)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [dependency] => self.dependencies.push((*dependency).clone()),
            [] => self.record_opaque_frontier(importer, specifier),
            _ => {
                return closure_mismatch(format!(
                    "external specifier {specifier:?} has more than one supplied dependency identity"
                ));
            }
        }
        Ok(())
    }

    fn record_opaque_frontier(&mut self, importer: &str, specifier: &str) {
        self.hazards.push(ClosureHazard {
            kind: ClosureHazardKind::UnacceptedExternalDependency,
            source: format!("./{importer}:{specifier}"),
            affected_exports: Vec::new(),
            affected_domains: all_domains(),
        });
    }

    fn add_entry(
        &mut self,
        role: ClosureFileRole,
        path: &str,
    ) -> Result<(), ArtifactSnapshotError> {
        let bytes = self.snapshot.read(path).ok_or_else(|| {
            ArtifactSnapshotError::ModuleClosure(format!(
                "closure file {path:?} is absent from the snapshot"
            ))
        })?;
        self.entries.push(ClosureEntry {
            role,
            path: format!("./{path}"),
            digest: format!("sha256:{:x}", Sha256::digest(bytes)),
            transform_digest: None,
        });
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum LocalResolution {
    Module(String),
    Asset(String),
    OpaqueAsset,
    External,
    Missing,
}

/// A Vite-style resource query (`./x.js?raw`, `?url`, `?worker`) or URL
/// fragment makes an import bundler-mediated: the binding's value is whatever
/// the loader produces and never the target module's exports. This is the exact
/// rule `bundlerResourceSuffix` applies in
/// `packages/cli/scripts/artifact-resolution.mjs`; the generator and this replay
/// must classify the same specifiers or every such closure diverges. A `#` at
/// index 0 is a package imports specifier rather than a fragment, and a suffix
/// introducer with nothing after it is not a suffix.
fn bundler_resource_suffix(specifier: &str) -> Option<&str> {
    let query = specifier.find('?');
    let fragment = specifier
        .match_indices('#')
        .map(|(at, _)| at)
        .find(|&at| at > 0);
    let introducer = match (query, fragment) {
        (Some(query), Some(fragment)) => query.min(fragment),
        (Some(at), None) | (None, Some(at)) => at,
        (None, None) => return None,
    };
    if introducer == 0 || introducer + 1 >= specifier.len() {
        return None;
    }
    Some(&specifier[introducer..])
}

pub(super) fn resolve_local(
    snapshot: &ArtifactSnapshot,
    importer: &str,
    specifier: &str,
    axis: ModuleAxis,
) -> Result<LocalResolution, ArtifactSnapshotError> {
    if bundler_resource_suffix(specifier).is_some() {
        return Ok(LocalResolution::OpaqueAsset);
    }
    if !specifier.starts_with('.') && !specifier.starts_with('/') {
        return Ok(LocalResolution::External);
    }
    if specifier.starts_with('/') {
        return Ok(LocalResolution::Missing);
    }
    let base = join_package_path(importer, specifier)?;
    let observed_extension = module_extension(&base);
    let allowed = |extension| match axis {
        ModuleAxis::Runtime => RUNTIME_EXTENSIONS.contains(&extension),
        ModuleAxis::Declarations => {
            RUNTIME_EXTENSIONS.contains(&extension) || DECLARATION_EXTENSIONS.contains(&extension)
        }
    };
    if observed_extension.is_some_and(|extension| !allowed(extension))
        && snapshot.read(&base).is_some()
    {
        return Ok(LocalResolution::Asset(base));
    }
    // A dotted basename with no corresponding file (for example
    // HeadContent.dev) remains extensionless for module suffix resolution.
    let extension = observed_extension.filter(|extension| allowed(extension));

    let substitutions = source_substitutions(&base);
    let declaration_source_substitutions = declaration_source_substitutions(&base);
    let candidates = match (axis, extension) {
        (ModuleAxis::Runtime, Some(_)) => std::iter::once(base.clone())
            .chain(substitutions)
            .collect::<Vec<_>>(),
        (ModuleAxis::Runtime, None) => std::iter::once(base.clone())
            .chain(
                RUNTIME_EXTENSIONS
                    .iter()
                    .map(|extension| format!("{base}{extension}")),
            )
            .chain(
                RUNTIME_EXTENSIONS
                    .iter()
                    .map(|extension| format!("{base}/index{extension}")),
            )
            .collect(),
        (ModuleAxis::Declarations, Some(extension))
            if DECLARATION_EXTENSIONS.contains(&extension)
                || [".ts", ".tsx", ".mts", ".cts"].contains(&extension) =>
        {
            std::iter::once(base.clone())
                .chain(declaration_source_substitutions)
                .collect()
        }
        (ModuleAxis::Declarations, Some(_)) => substitutions
            .into_iter()
            .chain(super::declaration_candidate(snapshot, &base))
            .collect(),
        (ModuleAxis::Declarations, None) => std::iter::once(base.clone())
            .chain(
                DECLARATION_MODULE_EXTENSIONS
                    .iter()
                    .map(|extension| format!("{base}{extension}")),
            )
            .chain(
                DECLARATION_MODULE_EXTENSIONS
                    .iter()
                    .map(|extension| format!("{base}/index{extension}")),
            )
            .collect(),
    };
    if let Some(path) = candidates
        .into_iter()
        .find(|candidate| snapshot.read(candidate).is_some())
    {
        return Ok(LocalResolution::Module(path));
    }
    Ok(if snapshot.read(&base).is_some() {
        LocalResolution::Asset(base)
    } else {
        LocalResolution::Missing
    })
}

fn join_package_path(importer: &str, specifier: &str) -> Result<String, ArtifactSnapshotError> {
    let mut parts = importer
        .rsplit_once('/')
        .map_or(Vec::new(), |(directory, _)| {
            directory.split('/').map(str::to_owned).collect()
        });
    for part in specifier.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return closure_mismatch(format!(
                        "module specifier {specifier:?} escapes the package snapshot"
                    ));
                }
            }
            part if part.contains('\\') => {
                return closure_mismatch(format!(
                    "module specifier {specifier:?} is not canonical"
                ));
            }
            part => parts.push(part.into()),
        }
    }
    if parts.is_empty() {
        return closure_mismatch(format!(
            "module specifier {specifier:?} does not name a package file"
        ));
    }
    Ok(parts.join("/"))
}

fn module_extension(path: &str) -> Option<&str> {
    let filename = path.rsplit('/').next()?;
    if filename.ends_with(".d.ts") {
        Some(".d.ts")
    } else if filename.ends_with(".d.mts") {
        Some(".d.mts")
    } else if filename.ends_with(".d.cts") {
        Some(".d.cts")
    } else {
        filename.rfind('.').map(|dot| &filename[dot..])
    }
}

fn source_substitutions(base: &str) -> Vec<String> {
    for (extension, replacements) in [
        (".js", &[".ts", ".tsx", ".d.ts"][..]),
        (".jsx", &[".tsx", ".d.ts"][..]),
        (".mjs", &[".mts", ".d.mts"][..]),
        (".cjs", &[".cts", ".d.cts"][..]),
    ] {
        if let Some(stem) = base.strip_suffix(extension) {
            return replacements
                .iter()
                .map(|replacement| format!("{stem}{replacement}"))
                .collect();
        }
    }
    Vec::new()
}

fn declaration_source_substitutions(base: &str) -> Vec<String> {
    for (extension, replacement) in [
        (".tsx", ".d.ts"),
        (".ts", ".d.ts"),
        (".mts", ".d.mts"),
        (".cts", ".d.cts"),
    ] {
        if let Some(stem) = base.strip_suffix(extension) {
            return vec![format!("{stem}{replacement}")];
        }
    }
    Vec::new()
}

fn all_domains() -> Vec<AffectedClaimDomain> {
    vec![
        AffectedClaimDomain::Callbacks,
        AffectedClaimDomain::Reads,
        AffectedClaimDomain::Writes,
        AffectedClaimDomain::Creates,
        AffectedClaimDomain::Invalidates,
        AffectedClaimDomain::Throws,
        AffectedClaimDomain::Returns,
        AffectedClaimDomain::Cleanups,
        AffectedClaimDomain::Disposals,
    ]
}

fn closure_mismatch<T>(reason: impl Into<String>) -> Result<T, ArtifactSnapshotError> {
    Err(ArtifactSnapshotError::ModuleClosure(reason.into()))
}
