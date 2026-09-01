//! Exact runtime/declaration export binding replay from snapshot bytes.

use std::collections::{BTreeMap, BTreeSet};

use solid_facts::{
    ast::{AstFacts, ExportKind, IdentifierRole, ImportKind, extract},
    core::Span,
};

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

    pub(super) fn declaration_binding(&self, name: &str) -> Option<(&str, &str, &str)> {
        self.bindings.get(name).map(|binding| {
            (
                binding.declarations_path.as_str(),
                binding.declarations_selector.as_str(),
                binding.declarations_export.as_str(),
            )
        })
    }

    pub(super) fn runtime_binding(&self, name: &str) -> Option<(&str, &str, Span, &str)> {
        self.bindings.get(name).and_then(|binding| {
            binding.runtime_span.map(|span| {
                (
                    binding.runtime_path.as_str(),
                    binding.runtime_export.as_str(),
                    span,
                    binding.runtime_snapshot_root.as_str(),
                )
            })
        })
    }

    pub(super) fn runtime_paths(&self) -> impl Iterator<Item = &str> {
        self.bindings
            .values()
            .map(|binding| binding.runtime_path.as_str())
    }

    pub(super) fn has_declaration_target(&self, path: &str, name: &str) -> bool {
        self.bindings.values().any(|binding| {
            binding.declarations_path == path
                && (binding.declarations_resolved_export == name
                    || binding.declarations_export == name)
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VerifiedExportBinding {
    runtime_path: String,
    runtime_export: String,
    runtime_resolved_export: String,
    runtime_selector: String,
    runtime_span: Option<Span>,
    runtime_snapshot_root: String,
    declarations_path: String,
    declarations_export: String,
    declarations_resolved_export: String,
    declarations_selector: String,
    declarations_span: Option<Span>,
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
                runtime_resolved_export: runtime.resolved_name,
                runtime_selector: runtime.selector,
                runtime_span: runtime.span,
                runtime_snapshot_root: runtime.snapshot_root,
                declarations_path: declarations.file,
                declarations_export: declarations.name,
                declarations_resolved_export: declarations.resolved_name,
                declarations_selector: declarations.selector,
                declarations_span: declarations.span,
                declarations_snapshot_root: declarations.snapshot_root,
            },
        );
    }
    let evidence_root = export_bindings_evidence_root(snapshot.root(), &bindings);
    Ok(SnapshotVerifiedExports {
        snapshot_root: snapshot.root().into(),
        evidence_root,
        bindings,
    })
}

fn export_bindings_evidence_root(
    snapshot_root: &str,
    bindings: &BTreeMap<String, VerifiedExportBinding>,
) -> String {
    let mut evidence_fields = vec![snapshot_root.to_owned()];
    for (name, binding) in bindings {
        evidence_fields.extend([
            name.clone(),
            binding.runtime_path.clone(),
            binding.runtime_export.clone(),
            binding
                .runtime_span
                .map_or_else(String::new, |span| format!("{}:{}", span.start, span.end)),
            binding.runtime_snapshot_root.clone(),
            binding.declarations_path.clone(),
            binding.declarations_export.clone(),
            binding
                .declarations_span
                .map_or_else(String::new, |span| format!("{}:{}", span.start, span.end)),
            binding.declarations_snapshot_root.clone(),
        ]);
        if binding.runtime_resolved_export != binding.runtime_export {
            evidence_fields.extend([
                "runtime-resolved-export".into(),
                binding.runtime_resolved_export.clone(),
            ]);
        }
        if binding.runtime_selector != binding.runtime_resolved_export {
            evidence_fields.extend(["runtime-selector".into(), binding.runtime_selector.clone()]);
        }
        if binding.declarations_resolved_export != binding.declarations_export {
            evidence_fields.extend([
                "declarations-resolved-export".into(),
                binding.declarations_resolved_export.clone(),
            ]);
        }
        if binding.declarations_selector != binding.declarations_resolved_export {
            evidence_fields.extend([
                "declarations-selector".into(),
                binding.declarations_selector.clone(),
            ]);
        }
    }
    super::certification_evidence_root(
        "export-bindings",
        evidence_fields.iter().map(String::as_str),
    )
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
    verify_binding_names(
        &supplied.runtime.export_name,
        &runtime.resolved_name,
        &supplied.declarations.export_name,
        &declarations.resolved_name,
    )
}

fn verify_binding_names(
    supplied_runtime: &str,
    replayed_runtime: &str,
    supplied_declarations: &str,
    replayed_declarations: &str,
) -> Result<(), ArtifactSnapshotError> {
    if supplied_runtime != replayed_runtime || supplied_declarations != replayed_declarations {
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
    /// Export name that addresses this target from its terminal module file.
    selector: String,
    /// Canonical target name returned by module export resolution.
    resolved_name: String,
    /// Exact name the Type Facts query at `span` must report.
    name: String,
    snapshot_root: String,
    span: Option<Span>,
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
        let type_only_imports = facts
            .imports
            .iter()
            .flat_map(|import| {
                import.bindings.iter().filter(move |binding| {
                    binding.kind != ImportKind::SideEffect
                        && (import.type_only || binding.type_only)
                })
            })
            .map(|binding| {
                span_text(source, binding.local.span.start, binding.local.span.end)
                    .map(ToOwned::to_owned)
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        let named_default_declarations = facts
            .module_level_exports()
            .filter(|export| export.kind == ExportKind::Default && !export.type_only)
            .filter_map(|export| export.declarations.first())
            .filter(|declaration| {
                facts.identifiers.iter().any(|identifier| {
                    identifier.role == IdentifierRole::Binding
                        && identifier.span == declaration.local.span
                })
            })
            .map(|declaration| {
                Ok((
                    declaration.local.span,
                    BindingTarget {
                        file: path.into(),
                        selector: "default".into(),
                        resolved_name: "default".into(),
                        name: if axis == ModuleAxis::Runtime {
                            span_text(
                                source,
                                declaration.local.span.start,
                                declaration.local.span.end,
                            )?
                            .into()
                        } else {
                            "default".into()
                        },
                        snapshot_root: self.snapshot.root().into(),
                        span: Some(declaration.local.span),
                    },
                ))
            })
            .collect::<Result<BTreeMap<_, _>, ArtifactSnapshotError>>()?;
        for import in facts.imports.iter().filter(|import| !import.type_only) {
            let resolution = resolve_local(self.snapshot, path, &import.module, axis)?;
            for binding in &import.bindings {
                if binding.type_only || binding.kind == ImportKind::SideEffect {
                    continue;
                }
                let local = span_text(source, binding.local.span.start, binding.local.span.end)?;
                if binding.kind == ImportKind::Namespace {
                    match &resolution {
                        LocalResolution::Module(target) => {
                            imports.insert(
                                local.into(),
                                BindingTarget {
                                    file: target.clone(),
                                    selector: "*".into(),
                                    resolved_name: "*".into(),
                                    name: "*".into(),
                                    snapshot_root: self.snapshot.root().into(),
                                    span: None,
                                },
                            );
                        }
                        LocalResolution::External | LocalResolution::OpaqueAsset => {
                            // Preserve the fact that this local came from an
                            // external namespace. `external_binding` rejects
                            // `*` fail-closed; omitting the entry would let the
                            // later export-specifier fallback misclassify the
                            // imported namespace as a declaration in this file.
                            external_imports
                                .insert(local.into(), (import.module.to_string(), "*".into()));
                        }
                        _ => {}
                    }
                    continue;
                }
                let Some(imported) = &binding.imported else {
                    continue;
                };
                match &resolution {
                    LocalResolution::Module(target) => {
                        imports.insert(
                            local.into(),
                            BindingTarget {
                                file: target.clone(),
                                selector: imported.to_string(),
                                resolved_name: imported.to_string(),
                                name: imported.to_string(),
                                snapshot_root: self.snapshot.root().into(),
                                span: None,
                            },
                        );
                    }
                    // A bundler-mediated asset import binds an opaque value, so
                    // it is unresolved here exactly as an external one is. The
                    // generator's `moduleDescription` records the same binding
                    // in `externalImports`.
                    LocalResolution::External | LocalResolution::OpaqueAsset => {
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
            let external = matches!(
                module_resolution,
                Some(LocalResolution::External | LocalResolution::OpaqueAsset)
            )
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
                                    selector: "*".into(),
                                    resolved_name: "*".into(),
                                    name: "*".into(),
                                    snapshot_root: self.snapshot.root().into(),
                                    span: None,
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
                        if export.module.is_none() && type_only_imports.contains(local) {
                            continue;
                        }
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
                                facts
                                    .reference_declaration(specifier.local.span)
                                    .and_then(|declaration| {
                                        named_default_declarations.get(&declaration)
                                    })
                                    .map(|target| BindingTarget {
                                        selector: specifier.exported.to_string(),
                                        resolved_name: local.into(),
                                        ..target.clone()
                                    })
                                    .or_else(|| imports.get(local).cloned())
                                    .unwrap_or_else(|| BindingTarget {
                                        file: path.into(),
                                        selector: specifier.exported.to_string(),
                                        resolved_name: local.into(),
                                        name: local.into(),
                                        snapshot_root: self.snapshot.root().into(),
                                        span: Some(specifier.local.span),
                                    })
                            },
                            |target| BindingTarget {
                                file: target.clone(),
                                selector: local.into(),
                                resolved_name: local.into(),
                                name: local.into(),
                                snapshot_root: self.snapshot.root().into(),
                                span: None,
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
                                selector: declaration.exported.to_string(),
                                resolved_name: declaration.exported.to_string(),
                                name: declaration.exported.to_string(),
                                snapshot_root: self.snapshot.root().into(),
                                span: Some(declaration.local.span),
                            },
                        );
                    }
                }
                ExportKind::Default => {
                    if !export.type_only {
                        let declaration = export.declarations.first();
                        let local_identifier = declaration
                            .filter(|declaration| {
                                facts
                                    .reference_declaration(declaration.local.span)
                                    .is_some()
                            })
                            .map(|declaration| {
                                span_text(
                                    source,
                                    declaration.local.span.start,
                                    declaration.local.span.end,
                                )
                            })
                            .transpose()?;
                        let query_name = local_identifier.or_else(|| {
                            declaration.and_then(|declaration| {
                                named_default_declarations
                                    .get(&declaration.local.span)
                                    .map(|target| target.name.as_str())
                            })
                        });
                        description.direct.insert(
                            "default".into(),
                            BindingTarget {
                                file: path.into(),
                                selector: "default".into(),
                                resolved_name: "default".into(),
                                name: query_name.unwrap_or("default").into(),
                                snapshot_root: self.snapshot.root().into(),
                                span: declaration.map(|declaration| declaration.local.span),
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

        let mut candidates = BTreeMap::<(String, String, String, String), BindingTarget>::new();
        for target in description.stars {
            if let Some(candidate) = self.bind_export(&target, name, axis, visiting)? {
                candidates.insert(
                    (
                        candidate.snapshot_root.clone(),
                        candidate.file.clone(),
                        candidate.selector.clone(),
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
                        candidate.selector.clone(),
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
        let (file, selector, resolved_name, name, snapshot_root, span) = match axis {
            ModuleAxis::Runtime => (
                &binding.runtime_path,
                &binding.runtime_selector,
                &binding.runtime_resolved_export,
                &binding.runtime_export,
                &binding.runtime_snapshot_root,
                binding.runtime_span,
            ),
            ModuleAxis::Declarations => (
                &binding.declarations_path,
                &binding.declarations_selector,
                &binding.declarations_resolved_export,
                &binding.declarations_export,
                &binding.declarations_snapshot_root,
                binding.declarations_span,
            ),
        };
        Some(BindingTarget {
            file: file.clone(),
            selector: selector.clone(),
            resolved_name: resolved_name.clone(),
            name: name.clone(),
            snapshot_root: snapshot_root.clone(),
            span,
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
                selector: "value".into(),
                resolved_name: "value".into(),
                name: "value".into(),
                snapshot_root: snapshot.root().into(),
                span: Some(Span { start: 13, end: 18 }),
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
                    selector: "SolidQueryDevtools".into(),
                    resolved_name: "SolidQueryDevtools".into(),
                    name: "SolidQueryDevtools".into(),
                    snapshot_root: snapshot.root().into(),
                    span: None,
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
                    selector: "SolidQueryDevtools".into(),
                    resolved_name: "SolidQueryDevtools".into(),
                    name: "SolidQueryDevtools".into(),
                    snapshot_root: snapshot.root().into(),
                    span: Some(Span { start: 21, end: 39 }),
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
                selector: "SolidQueryDevtools".into(),
                resolved_name: "SolidQueryDevtools".into(),
                name: "SolidQueryDevtools".into(),
                snapshot_root: snapshot.root().into(),
                span: Some(Span { start: 21, end: 39 }),
            })
        );
    }

    #[test]
    fn namespace_exports_replay_the_exact_module_object_target() {
        let snapshot = snapshot(&[
            (
                "index.d.ts",
                concat!(
                    "import * as IR from './query/ir.js'; ",
                    "import { value } from './query/ir.js'; ",
                    "import type * as Types from './query/ir.js'; ",
                    "import * as ExternalImported from 'unplanned'; ",
                    "export { IR, value, Types as LeakedTypes, ExternalImported }; ",
                    "export type { Types }; ",
                    "export { Types as RuntimeTypes } from './query/ir.js'; ",
                    "export * as Direct from './query/ir.js'; ",
                    "export type * as DirectTypes from './query/ir.js'; ",
                    "export * as ExternalDirect from 'unplanned';",
                ),
            ),
            (
                "query/ir.d.ts",
                "export declare const value: number; export declare const other: string; export declare const Types: symbol;",
            ),
        ]);
        let mut replay = ExportReplay {
            snapshot: &snapshot,
            dependencies: &[],
            descriptions: BTreeMap::new(),
        };

        for name in ["IR", "Direct"] {
            let target = replay
                .bind_export(
                    "index.d.ts",
                    name,
                    ModuleAxis::Declarations,
                    &mut BTreeSet::new(),
                )
                .unwrap()
                .unwrap();
            assert_eq!(target.file, "query/ir.d.ts");
            assert_eq!(target.selector, "*");
            assert_eq!(target.resolved_name, "*");
            assert_eq!(target.name, "*");
        }

        let ordinary = replay
            .bind_export(
                "index.d.ts",
                "value",
                ModuleAxis::Declarations,
                &mut BTreeSet::new(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(ordinary.file, "query/ir.d.ts");
        assert_eq!(ordinary.selector, "value");
        assert_eq!(ordinary.resolved_name, "value");
        assert_eq!(ordinary.name, "value");

        let same_spelling_reexport = replay
            .bind_export(
                "index.d.ts",
                "RuntimeTypes",
                ModuleAxis::Declarations,
                &mut BTreeSet::new(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(same_spelling_reexport.file, "query/ir.d.ts");
        assert_eq!(same_spelling_reexport.selector, "Types");
        assert_eq!(same_spelling_reexport.resolved_name, "Types");
        assert_eq!(same_spelling_reexport.name, "Types");

        for name in [
            "Types",
            "LeakedTypes",
            "DirectTypes",
            "ExternalImported",
            "ExternalDirect",
        ] {
            assert!(
                replay
                    .bind_export(
                        "index.d.ts",
                        name,
                        ModuleAxis::Declarations,
                        &mut BTreeSet::new(),
                    )
                    .unwrap()
                    .is_none(),
                "{name} must not become an authenticated namespace binding"
            );
        }
    }

    #[test]
    fn default_export_replay_preserves_exact_local_declaration_identity() {
        let sources = [
            (
                "identifier.js",
                "function createX() {} export default createX;",
            ),
            (
                "declaration.d.ts",
                "export default function createX(): void; export { createX };",
            ),
            (
                "named.js",
                "export default function createRuntime() {} export { createRuntime };",
            ),
            ("anonymous.js", "export default (value) => value;"),
        ];
        let snapshot = snapshot(&sources);
        let mut replay = ExportReplay {
            snapshot: &snapshot,
            dependencies: &[],
            descriptions: BTreeMap::new(),
        };

        let identifier = replay
            .description("identifier.js", ModuleAxis::Runtime)
            .unwrap();
        let identifier_target = identifier.direct.get("default").unwrap();
        assert_eq!(identifier_target.name, "createX");
        assert_eq!(identifier_target.resolved_name, "default");
        assert_eq!(
            span_text(
                sources[0].1,
                identifier_target.span.unwrap().start,
                identifier_target.span.unwrap().end,
            )
            .unwrap(),
            "createX"
        );

        let declaration = replay
            .description("declaration.d.ts", ModuleAxis::Declarations)
            .unwrap();
        let default_target = declaration.direct.get("default").unwrap();
        let named_target = declaration.direct.get("createX").unwrap();
        assert_eq!(default_target.name, "default");
        assert_eq!(default_target.resolved_name, "default");
        assert_eq!(named_target.name, "default");
        assert_eq!(named_target.resolved_name, "createX");
        assert_eq!(named_target.file, default_target.file);
        assert_eq!(named_target.span, default_target.span);
        assert_eq!(
            span_text(
                sources[1].1,
                named_target.span.unwrap().start,
                named_target.span.unwrap().end,
            )
            .unwrap(),
            "createX"
        );

        let named_runtime = replay.description("named.js", ModuleAxis::Runtime).unwrap();
        let runtime_default = named_runtime.direct.get("default").unwrap();
        let runtime_named = named_runtime.direct.get("createRuntime").unwrap();
        assert_eq!(runtime_default.name, "createRuntime");
        assert_eq!(runtime_default.resolved_name, "default");
        assert_eq!(runtime_named.name, "createRuntime");
        assert_eq!(runtime_named.resolved_name, "createRuntime");
        assert_eq!(runtime_named.span, runtime_default.span);

        let anonymous = replay
            .description("anonymous.js", ModuleAxis::Runtime)
            .unwrap();
        let anonymous_target = anonymous.direct.get("default").unwrap();
        assert_eq!(anonymous_target.name, "default");
        assert_eq!(anonymous_target.resolved_name, "default");
        assert_eq!(
            span_text(
                sources[3].1,
                anonymous_target.span.unwrap().start,
                anonymous_target.span.unwrap().end,
            )
            .unwrap(),
            "(value) => value"
        );

        assert!(verify_binding_names("default", "default", "createX", "createX").is_ok());
        assert!(matches!(
            verify_binding_names("createX", "default", "createX", "createX"),
            Err(ArtifactSnapshotError::ExportBindings(_))
        ));
        assert!(matches!(
            verify_binding_names("default", "default", "default", "createX"),
            Err(ArtifactSnapshotError::ExportBindings(_))
        ));
    }

    #[test]
    fn default_export_entry_forms_and_propagation_remain_distinct() {
        let snapshot = snapshot(&[
            (
                "forward.js",
                "export { createForward }; export default function createForward() {}",
            ),
            (
                "class.js",
                "export default class NamedClass {} export { NamedClass };",
            ),
            (
                "alias-default.js",
                "const value = () => 1; export { value as default };",
            ),
            (
                "expression-default.js",
                "const value = () => 1; export default value;",
            ),
            ("impl.js", "export default function implementation() {}"),
            (
                "barrel.js",
                "export { default as publicName } from './impl';",
            ),
            (
                "import-barrel.js",
                "import implementation from './impl'; export { implementation as publicName };",
            ),
            (
                "external.js",
                "import externalDefault from 'unplanned'; export { externalDefault };",
            ),
            (
                "fan-in-source.js",
                "const x = 1; export default x; export { x };",
            ),
            (
                "fan-in-default.js",
                "export { default as y } from './fan-in-source';",
            ),
            (
                "fan-in-named.js",
                "export { x as y } from './fan-in-source';",
            ),
            (
                "fan-in-entry.js",
                "export * from './fan-in-default'; export * from './fan-in-named';",
            ),
        ]);
        let mut replay = ExportReplay {
            snapshot: &snapshot,
            dependencies: &[],
            descriptions: BTreeMap::new(),
        };

        for (path, name) in [("forward.js", "createForward"), ("class.js", "NamedClass")] {
            let description = replay.description(path, ModuleAxis::Runtime).unwrap();
            let default = description.direct.get("default").unwrap();
            let named = description.direct.get(name).unwrap();
            assert_eq!(default.name, name);
            assert_eq!(default.selector, "default");
            assert_eq!(default.resolved_name, "default");
            assert_eq!(named.name, name);
            assert_eq!(named.selector, name);
            assert_eq!(named.resolved_name, name);
            assert_eq!(named.span, default.span);
        }

        let alias = replay
            .description("alias-default.js", ModuleAxis::Runtime)
            .unwrap()
            .direct
            .remove("default")
            .unwrap();
        let expression = replay
            .description("expression-default.js", ModuleAxis::Runtime)
            .unwrap()
            .direct
            .remove("default")
            .unwrap();
        assert_eq!(alias.resolved_name, "value");
        assert_eq!(expression.resolved_name, "default");
        assert_eq!(alias.selector, "default");
        assert_eq!(expression.selector, "default");
        assert_eq!(alias.name, "value");
        assert_eq!(expression.name, "value");
        assert_ne!(alias.span, expression.span);
        let verified = SnapshotVerifiedExports {
            snapshot_root: snapshot.root().into(),
            evidence_root: "sha256:test".into(),
            bindings: BTreeMap::from([
                (
                    "default".into(),
                    VerifiedExportBinding {
                        runtime_path: "expression-default.js".into(),
                        runtime_export: expression.name.clone(),
                        runtime_resolved_export: expression.resolved_name.clone(),
                        runtime_selector: expression.selector.clone(),
                        runtime_span: expression.span,
                        runtime_snapshot_root: snapshot.root().into(),
                        declarations_path: "expression-default.js".into(),
                        declarations_export: expression.name.clone(),
                        declarations_resolved_export: expression.resolved_name.clone(),
                        declarations_selector: expression.selector.clone(),
                        declarations_span: expression.span,
                        declarations_snapshot_root: snapshot.root().into(),
                    },
                ),
                (
                    "y".into(),
                    VerifiedExportBinding {
                        runtime_path: "alias.d.ts".into(),
                        runtime_export: "default".into(),
                        runtime_resolved_export: "createX".into(),
                        runtime_selector: "y".into(),
                        runtime_span: None,
                        runtime_snapshot_root: snapshot.root().into(),
                        declarations_path: "alias.d.ts".into(),
                        declarations_export: "default".into(),
                        declarations_resolved_export: "createX".into(),
                        declarations_selector: "y".into(),
                        declarations_span: None,
                        declarations_snapshot_root: snapshot.root().into(),
                    },
                ),
            ]),
        };
        assert_eq!(
            verified.declaration_binding("default"),
            Some(("expression-default.js", "default", "value"))
        );
        assert!(verified.has_declaration_target("expression-default.js", "default"));
        assert!(verified.has_declaration_target("expression-default.js", "value"));
        assert!(!verified.has_declaration_target("expression-default.js", "other"));
        assert!(verified.has_declaration_target("alias.d.ts", "createX"));
        assert!(verified.has_declaration_target("alias.d.ts", "default"));
        assert!(!verified.has_declaration_target("alias.d.ts", "y"));
        let evidence_root = export_bindings_evidence_root(snapshot.root(), &verified.bindings);
        let mut selector_mutation = verified.bindings.clone();
        selector_mutation
            .get_mut("y")
            .unwrap()
            .declarations_selector = "other".into();
        assert_ne!(
            export_bindings_evidence_root(snapshot.root(), &selector_mutation),
            evidence_root
        );
        let mut resolver_mutation = verified.bindings.clone();
        resolver_mutation
            .get_mut("y")
            .unwrap()
            .declarations_resolved_export = "other".into();
        assert_ne!(
            export_bindings_evidence_root(snapshot.root(), &resolver_mutation),
            evidence_root
        );
        let mut runtime_selector_mutation = verified.bindings.clone();
        runtime_selector_mutation
            .get_mut("y")
            .unwrap()
            .runtime_selector = "other".into();
        assert_ne!(
            export_bindings_evidence_root(snapshot.root(), &runtime_selector_mutation),
            evidence_root
        );
        let mut runtime_resolver_mutation = verified.bindings.clone();
        runtime_resolver_mutation
            .get_mut("y")
            .unwrap()
            .runtime_resolved_export = "other".into();
        assert_ne!(
            export_bindings_evidence_root(snapshot.root(), &runtime_resolver_mutation),
            evidence_root
        );

        for path in ["barrel.js", "import-barrel.js"] {
            let target = replay
                .bind_export(
                    path,
                    "publicName",
                    ModuleAxis::Runtime,
                    &mut BTreeSet::new(),
                )
                .unwrap()
                .unwrap();
            assert_eq!(target.file, "impl.js");
            assert_eq!(target.selector, "default");
            assert_eq!(target.resolved_name, "default");
            assert_eq!(target.name, "implementation");
        }

        assert!(
            replay
                .bind_export(
                    "external.js",
                    "externalDefault",
                    ModuleAxis::Runtime,
                    &mut BTreeSet::new(),
                )
                .unwrap()
                .is_none(),
            "an unplanned external default import must not acquire a local target"
        );
        assert!(matches!(
            replay.bind_export(
                "fan-in-entry.js",
                "y",
                ModuleAxis::Runtime,
                &mut BTreeSet::new(),
            ),
            Err(ArtifactSnapshotError::ExportBindings(_))
        ));
    }
}
