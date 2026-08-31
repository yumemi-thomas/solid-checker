//! Exact runtime/declaration export binding replay from snapshot bytes.

use std::collections::{BTreeMap, BTreeSet};

use solid_facts::ast::{AstFacts, ExportKind, ImportKind, extract};

use crate::artifact_resolution::{ResolvedExportBinding, ResolvedImport};

use super::module_closure::{LocalResolution, ModuleAxis, resolve_local};
use super::{
    ArtifactSnapshot, ArtifactSnapshotError, SnapshotVerifiedResolution, verify_resolved_file,
};

#[derive(Clone, Debug)]
pub struct SnapshotVerifiedExports {
    snapshot_root: String,
    evidence_root: String,
    bindings: BTreeMap<String, VerifiedExportBinding>,
}

impl SnapshotVerifiedExports {
    #[must_use]
    pub fn snapshot_root(&self) -> &str {
        &self.snapshot_root
    }

    #[must_use]
    pub fn binding_count(&self) -> usize {
        self.bindings.len()
    }

    #[must_use]
    pub fn evidence_root(&self) -> &str {
        &self.evidence_root
    }

    #[must_use]
    pub fn site_ids(&self) -> Vec<String> {
        self.bindings.keys().cloned().collect()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    pub(super) fn declaration_binding(&self, name: &str) -> Option<(&str, &str)> {
        self.bindings.get(name).map(|binding| {
            (
                binding.declarations_path.as_str(),
                binding.declarations_export.as_str(),
            )
        })
    }

    pub(super) fn has_declaration_target(&self, path: &str, name: &str) -> bool {
        self.bindings.iter().any(|(public_name, binding)| {
            binding.declarations_path == path
                && (public_name == name || binding.declarations_export == name)
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VerifiedExportBinding {
    runtime_path: String,
    runtime_export: String,
    runtime_snapshot_root: String,
    declarations_path: String,
    declarations_export: String,
    declarations_snapshot_root: String,
}

pub(super) fn verify_snapshot_exports_with_dependencies(
    snapshot: &ArtifactSnapshot,
    resolution: &SnapshotVerifiedResolution,
    resolved: &ResolvedImport,
    dependencies: &[&super::CertificationPlan],
) -> Result<SnapshotVerifiedExports, ArtifactSnapshotError> {
    let mut replay = ExportReplay {
        snapshot,
        dependencies,
        descriptions: BTreeMap::new(),
    };
    let runtime_names = replay.exported_names(
        resolution.runtime_path(),
        ModuleAxis::Runtime,
        &mut BTreeSet::new(),
    )?;
    let declaration_names = replay.exported_names(
        resolution.declarations_path(),
        ModuleAxis::Declarations,
        &mut BTreeSet::new(),
    )?;
    let names = runtime_names
        .intersection(&declaration_names)
        .cloned()
        .collect::<BTreeSet<_>>();
    let supplied_names = resolved.exports.keys().cloned().collect::<BTreeSet<_>>();
    if names != supplied_names {
        let replayed_only = names
            .difference(&supplied_names)
            .take(8)
            .cloned()
            .collect::<Vec<_>>();
        let supplied_only = supplied_names
            .difference(&names)
            .take(8)
            .cloned()
            .collect::<Vec<_>>();
        return export_mismatch(format!(
            "supplied export names do not equal the runtime/declaration intersection; replayedOnly(count={}, sample={replayed_only:?}); suppliedOnly(count={}, sample={supplied_only:?})",
            names.difference(&supplied_names).count(),
            supplied_names.difference(&names).count(),
        ));
    }

    let mut bindings = BTreeMap::new();
    for name in names {
        let runtime = replay
            .bind_export(
                resolution.runtime_path(),
                &name,
                ModuleAxis::Runtime,
                &mut BTreeSet::new(),
            )?
            .ok_or_else(|| {
                ArtifactSnapshotError::ExportBindings(format!(
                    "runtime export {name:?} has no exact binding"
                ))
            })?;
        let declarations = replay
            .bind_export(
                resolution.declarations_path(),
                &name,
                ModuleAxis::Declarations,
                &mut BTreeSet::new(),
            )?
            .ok_or_else(|| {
                ArtifactSnapshotError::ExportBindings(format!(
                    "declaration export {name:?} has no exact binding"
                ))
            })?;
        let supplied = resolved
            .exports
            .get(&name)
            .expect("supplied export key set was compared above");
        verify_binding(
            snapshot,
            resolved,
            supplied,
            &runtime,
            &declarations,
            dependencies,
        )?;
        bindings.insert(
            name,
            VerifiedExportBinding {
                runtime_path: runtime.file,
                runtime_export: runtime.name,
                runtime_snapshot_root: runtime.snapshot_root,
                declarations_path: declarations.file,
                declarations_export: declarations.name,
                declarations_snapshot_root: declarations.snapshot_root,
            },
        );
    }
    let mut evidence_fields = vec![snapshot.root().to_owned()];
    for (name, binding) in &bindings {
        evidence_fields.extend([
            name.clone(),
            binding.runtime_path.clone(),
            binding.runtime_export.clone(),
            binding.runtime_snapshot_root.clone(),
            binding.declarations_path.clone(),
            binding.declarations_export.clone(),
            binding.declarations_snapshot_root.clone(),
        ]);
    }
    let evidence_root = super::certification_evidence_root(
        "export-bindings",
        evidence_fields.iter().map(String::as_str),
    );
    Ok(SnapshotVerifiedExports {
        snapshot_root: snapshot.root().into(),
        evidence_root,
        bindings,
    })
}

#[cfg(test)]
pub(super) fn verify_snapshot_exports(
    snapshot: &ArtifactSnapshot,
    resolution: &SnapshotVerifiedResolution,
    resolved: &ResolvedImport,
) -> Result<SnapshotVerifiedExports, ArtifactSnapshotError> {
    verify_snapshot_exports_with_dependencies(snapshot, resolution, resolved, &[])
}

fn verify_binding(
    snapshot: &ArtifactSnapshot,
    resolved: &ResolvedImport,
    supplied: &ResolvedExportBinding,
    runtime: &BindingTarget,
    declarations: &BindingTarget,
    dependencies: &[&super::CertificationPlan],
) -> Result<(), ArtifactSnapshotError> {
    verify_target(
        snapshot,
        resolved,
        &supplied.runtime.module,
        runtime,
        dependencies,
    )?;
    verify_target(
        snapshot,
        resolved,
        &supplied.declarations.module,
        declarations,
        dependencies,
    )?;
    if supplied.runtime.export_name != runtime.name
        || supplied.declarations.export_name != declarations.name
    {
        return export_mismatch("supplied export target name disagrees with snapshot replay");
    }
    Ok(())
}

fn verify_target(
    parent_snapshot: &ArtifactSnapshot,
    parent_resolved: &ResolvedImport,
    supplied: &crate::artifact_resolution::ResolvedFile,
    replayed: &BindingTarget,
    dependencies: &[&super::CertificationPlan],
) -> Result<(), ArtifactSnapshotError> {
    if replayed.snapshot_root == parent_snapshot.root() {
        return verify_resolved_file(parent_snapshot, parent_resolved, supplied, &replayed.file);
    }
    let dependency = dependencies
        .iter()
        .copied()
        .find(|dependency| dependency.snapshot.root() == replayed.snapshot_root)
        .ok_or_else(|| {
            ArtifactSnapshotError::ExportBindings(
                "external export target has no exact planned dependency snapshot".into(),
            )
        })?;
    verify_resolved_file(
        &dependency.snapshot,
        &dependency.resolved_import,
        supplied,
        &replayed.file,
    )
}

#[derive(Clone, Debug, Default)]
struct ModuleDescription {
    direct: BTreeMap<String, BindingTarget>,
    stars: Vec<String>,
    external_direct: BTreeMap<String, (String, String)>,
    external_stars: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BindingTarget {
    file: String,
    name: String,
    snapshot_root: String,
}

struct ExportReplay<'a> {
    snapshot: &'a ArtifactSnapshot,
    dependencies: &'a [&'a super::CertificationPlan],
    descriptions: BTreeMap<(ModuleAxis, String), ModuleDescription>,
}

impl ExportReplay<'_> {
    fn description(
        &mut self,
        path: &str,
        axis: ModuleAxis,
    ) -> Result<ModuleDescription, ArtifactSnapshotError> {
        let key = (axis, path.to_owned());
        if let Some(description) = self.descriptions.get(&key) {
            return Ok(description.clone());
        }
        let bytes = self.snapshot.read(path).ok_or_else(|| {
            ArtifactSnapshotError::ExportBindings(format!(
                "export module {path:?} is absent from the snapshot"
            ))
        })?;
        let source = std::str::from_utf8(bytes).map_err(|_| {
            ArtifactSnapshotError::ExportBindings(format!(
                "export module {path:?} is not valid UTF-8"
            ))
        })?;
        let facts = extract(format!("./{path}"), source).map_err(|error| {
            ArtifactSnapshotError::ExportBindings(format!(
                "export module {path:?} cannot be parsed: {error}"
            ))
        })?;
        let description = self.describe(path, axis, source, &facts)?;
        self.descriptions.insert(key, description.clone());
        Ok(description)
    }

    fn describe(
        &self,
        path: &str,
        axis: ModuleAxis,
        source: &str,
        facts: &AstFacts,
    ) -> Result<ModuleDescription, ArtifactSnapshotError> {
        let mut description = ModuleDescription::default();
        let mut imports = BTreeMap::<String, BindingTarget>::new();
        let mut external_imports = BTreeMap::<String, (String, String)>::new();
        for import in facts.imports.iter().filter(|import| !import.type_only) {
            let resolution = resolve_local(self.snapshot, path, &import.module, axis)?;
            for binding in &import.bindings {
                if binding.type_only
                    || binding.kind == ImportKind::SideEffect
                    || binding.kind == ImportKind::Namespace
                {
                    continue;
                }
                let local = span_text(source, binding.local.span.start, binding.local.span.end)?;
                let Some(imported) = &binding.imported else {
                    continue;
                };
                match &resolution {
                    LocalResolution::Module(target) => {
                        imports.insert(
                            local.into(),
                            BindingTarget {
                                file: target.clone(),
                                name: imported.to_string(),
                                snapshot_root: self.snapshot.root().into(),
                            },
                        );
                    }
                    LocalResolution::External => {
                        external_imports.insert(
                            local.into(),
                            (import.module.to_string(), imported.to_string()),
                        );
                    }
                    _ => {}
                }
            }
        }

        for export in facts.module_level_exports() {
            let module_resolution = export
                .module
                .as_deref()
                .map(|specifier| resolve_local(self.snapshot, path, specifier, axis))
                .transpose()?;
            let target = module_resolution
                .as_ref()
                .and_then(|resolution| match resolution {
                    LocalResolution::Module(target) => Some(target.clone()),
                    _ => None,
                });
            let external = matches!(module_resolution, Some(LocalResolution::External))
                .then(|| export.module.as_deref())
                .flatten();
            match export.kind {
                ExportKind::All => {
                    if !export.type_only
                        && let Some(target) = target
                    {
                        if let Some(namespace) = &export.namespace {
                            description.direct.insert(
                                namespace.to_string(),
                                BindingTarget {
                                    file: target,
                                    name: "*".into(),
                                    snapshot_root: self.snapshot.root().into(),
                                },
                            );
                        } else {
                            description.stars.push(target);
                        }
                    } else if !export.type_only
                        && let Some(external) = external
                    {
                        if let Some(namespace) = &export.namespace {
                            description
                                .external_direct
                                .insert(namespace.to_string(), (external.into(), "*".into()));
                        } else {
                            description.external_stars.push(external.into());
                        }
                    }
                }
                ExportKind::Named => {
                    for specifier in export
                        .specifiers
                        .iter()
                        .filter(|specifier| !export.type_only && !specifier.type_only)
                    {
                        let local = span_text(
                            source,
                            specifier.local.span.start,
                            specifier.local.span.end,
                        )?;
                        if let Some(external) = external {
                            description.external_direct.insert(
                                specifier.exported.to_string(),
                                (external.into(), local.into()),
                            );
                            continue;
                        }
                        if let Some(external_import) = external_imports.get(local) {
                            description
                                .external_direct
                                .insert(specifier.exported.to_string(), external_import.clone());
                            continue;
                        }
                        let binding = target.as_ref().map_or_else(
                            || {
                                imports
                                    .get(local)
                                    .cloned()
                                    .unwrap_or_else(|| BindingTarget {
                                        file: path.into(),
                                        name: local.into(),
                                        snapshot_root: self.snapshot.root().into(),
                                    })
                            },
                            |target| BindingTarget {
                                file: target.clone(),
                                name: local.into(),
                                snapshot_root: self.snapshot.root().into(),
                            },
                        );
                        description
                            .direct
                            .insert(specifier.exported.to_string(), binding);
                    }
                    for declaration in export
                        .declarations
                        .iter()
                        .filter(|declaration| !declaration.type_only)
                    {
                        description.direct.insert(
                            declaration.exported.to_string(),
                            BindingTarget {
                                file: path.into(),
                                name: declaration.exported.to_string(),
                                snapshot_root: self.snapshot.root().into(),
                            },
                        );
                    }
                }
                ExportKind::Default => {
                    if !export.type_only {
                        description.direct.insert(
                            "default".into(),
                            BindingTarget {
                                file: path.into(),
                                name: "default".into(),
                                snapshot_root: self.snapshot.root().into(),
                            },
                        );
                    }
                }
            }
        }
        Ok(description)
    }

    fn exported_names(
        &mut self,
        path: &str,
        axis: ModuleAxis,
        visiting: &mut BTreeSet<(ModuleAxis, String)>,
    ) -> Result<BTreeSet<String>, ArtifactSnapshotError> {
        let identity = (axis, path.into());
        if !visiting.insert(identity.clone()) {
            return Ok(BTreeSet::new());
        }
        let description = self.description(path, axis)?;
        let mut names = description.direct.keys().cloned().collect::<BTreeSet<_>>();
        names.extend(description.external_direct.keys().cloned());
        for target in description.stars {
            names.extend(
                self.exported_names(&target, axis, visiting)?
                    .into_iter()
                    .filter(|name| name != "default"),
            );
        }
        for specifier in description.external_stars {
            if let Some(dependency) = self.external_dependency(&specifier) {
                names.extend(
                    dependency
                        .verified_exports
                        .bindings
                        .keys()
                        .filter(|name| name.as_str() != "default")
                        .cloned(),
                );
            }
        }
        visiting.remove(&identity);
        Ok(names)
    }

    fn bind_export(
        &mut self,
        path: &str,
        name: &str,
        axis: ModuleAxis,
        visiting: &mut BTreeSet<(ModuleAxis, String, String)>,
    ) -> Result<Option<BindingTarget>, ArtifactSnapshotError> {
        let identity = (axis, path.into(), name.into());
        if !visiting.insert(identity.clone()) {
            return export_mismatch(format!("export {name:?} participates in a re-export cycle"));
        }
        let description = self.description(path, axis)?;
        if let Some(direct) = description.direct.get(name) {
            let result = if direct.file == path || direct.name == "*" {
                Some(direct.clone())
            } else {
                self.bind_export(&direct.file, &direct.name, axis, visiting)?
            };
            visiting.remove(&identity);
            return Ok(result);
        }
        if let Some((specifier, imported)) = description.external_direct.get(name) {
            let result = self.external_binding(specifier, imported, axis);
            visiting.remove(&identity);
            return Ok(result);
        }
        if name == "default" {
            visiting.remove(&identity);
            return Ok(None);
        }

        let mut candidates = BTreeMap::<(String, String, String), BindingTarget>::new();
        for target in description.stars {
            if let Some(candidate) = self.bind_export(&target, name, axis, visiting)? {
                candidates.insert(
                    (
                        candidate.snapshot_root.clone(),
                        candidate.file.clone(),
                        candidate.name.clone(),
                    ),
                    candidate,
                );
            }
        }
        for specifier in description.external_stars {
            if let Some(candidate) = self.external_binding(&specifier, name, axis) {
                candidates.insert(
                    (
                        candidate.snapshot_root.clone(),
                        candidate.file.clone(),
                        candidate.name.clone(),
                    ),
                    candidate,
                );
            }
        }
        visiting.remove(&identity);
        match candidates.len() {
            0 => Ok(None),
            1 => Ok(candidates.into_values().next()),
            _ => export_mismatch(format!(
                "export {name:?} resolves through multiple star exports"
            )),
        }
    }

    fn external_dependency(&self, specifier: &str) -> Option<&super::CertificationPlan> {
        let mut matches = self
            .dependencies
            .iter()
            .copied()
            .filter(|dependency| dependency.import_request.specifier == specifier);
        let dependency = matches.next()?;
        matches.next().is_none().then_some(dependency)
    }

    fn external_binding(
        &self,
        specifier: &str,
        name: &str,
        axis: ModuleAxis,
    ) -> Option<BindingTarget> {
        if name == "*" {
            return None;
        }
        let dependency = self.external_dependency(specifier)?;
        let binding = dependency.verified_exports.bindings.get(name)?;
        let (file, name, snapshot_root) = match axis {
            ModuleAxis::Runtime => (
                &binding.runtime_path,
                &binding.runtime_export,
                &binding.runtime_snapshot_root,
            ),
            ModuleAxis::Declarations => (
                &binding.declarations_path,
                &binding.declarations_export,
                &binding.declarations_snapshot_root,
            ),
        };
        Some(BindingTarget {
            file: file.clone(),
            name: name.clone(),
            snapshot_root: snapshot_root.clone(),
        })
    }
}

fn span_text(source: &str, start: u32, end: u32) -> Result<&str, ArtifactSnapshotError> {
    source
        .get(start as usize..end as usize)
        .ok_or_else(|| ArtifactSnapshotError::ExportBindings("export span is invalid".into()))
}

fn export_mismatch<T>(reason: impl Into<String>) -> Result<T, ArtifactSnapshotError> {
    Err(ArtifactSnapshotError::ExportBindings(reason.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn snapshot(files: &[(&str, &str)]) -> ArtifactSnapshot {
        ArtifactSnapshot {
            package_name: "source-types".into(),
            package_version: "1.0.0".into(),
            package_integrity: "sha512:test".into(),
            files: Arc::new(
                files
                    .iter()
                    .map(|(path, source)| ((*path).into(), Arc::<[u8]>::from(source.as_bytes())))
                    .collect(),
            ),
            directories: Arc::new(BTreeSet::new()),
            root: "sha256:snapshot".into(),
            provenance_root: "sha256:provenance".into(),
        }
    }

    #[test]
    fn source_artifact_replay_excludes_explicit_type_only_exports() {
        let snapshot = snapshot(&[
            ("index.ts", "export { value, type Props } from './impl';\n"),
            (
                "impl.ts",
                "export const value = 1; export interface Props { label: string }\n",
            ),
        ]);
        let mut replay = ExportReplay {
            snapshot: &snapshot,
            dependencies: &[],
            descriptions: BTreeMap::new(),
        };

        let names = replay
            .exported_names("index.ts", ModuleAxis::Runtime, &mut BTreeSet::new())
            .unwrap();
        assert_eq!(names, BTreeSet::from(["value".into()]));
        assert_eq!(
            replay
                .bind_export(
                    "index.ts",
                    "value",
                    ModuleAxis::Runtime,
                    &mut BTreeSet::new(),
                )
                .unwrap(),
            Some(BindingTarget {
                file: "impl.ts".into(),
                name: "value".into(),
                snapshot_root: snapshot.root().into(),
            })
        );
    }

    #[test]
    fn declaration_replay_resolves_js_specifier_to_declaration_file() {
        let snapshot = snapshot(&[
            (
                "build/index.d.ts",
                "export { SolidQueryDevtools } from './_tsup-dts-rollup.js';\n",
            ),
            (
                "build/_tsup-dts-rollup.d.ts",
                "export declare const SolidQueryDevtools: () => unknown;\n",
            ),
        ]);
        let mut replay = ExportReplay {
            snapshot: &snapshot,
            dependencies: &[],
            descriptions: BTreeMap::new(),
        };

        assert_eq!(
            replay
                .description("build/index.d.ts", ModuleAxis::Declarations)
                .unwrap()
                .direct,
            BTreeMap::from([(
                "SolidQueryDevtools".into(),
                BindingTarget {
                    file: "build/_tsup-dts-rollup.d.ts".into(),
                    name: "SolidQueryDevtools".into(),
                    snapshot_root: snapshot.root().into(),
                },
            )])
        );
        assert_eq!(
            replay
                .description("build/_tsup-dts-rollup.d.ts", ModuleAxis::Declarations,)
                .unwrap()
                .direct,
            BTreeMap::from([(
                "SolidQueryDevtools".into(),
                BindingTarget {
                    file: "build/_tsup-dts-rollup.d.ts".into(),
                    name: "SolidQueryDevtools".into(),
                    snapshot_root: snapshot.root().into(),
                },
            )])
        );

        assert_eq!(
            replay
                .bind_export(
                    "build/index.d.ts",
                    "SolidQueryDevtools",
                    ModuleAxis::Declarations,
                    &mut BTreeSet::new(),
                )
                .unwrap(),
            Some(BindingTarget {
                file: "build/_tsup-dts-rollup.d.ts".into(),
                name: "SolidQueryDevtools".into(),
                snapshot_root: snapshot.root().into(),
            })
        );
    }
}
