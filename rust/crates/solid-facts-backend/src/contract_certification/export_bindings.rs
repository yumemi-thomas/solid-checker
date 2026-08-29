//! Exact runtime/declaration export binding replay from snapshot bytes.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest as _, Sha256};
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VerifiedExportBinding {
    runtime_path: String,
    runtime_export: String,
    declarations_path: String,
    declarations_export: String,
}

pub(super) fn verify_snapshot_exports(
    snapshot: &ArtifactSnapshot,
    resolution: &SnapshotVerifiedResolution,
    resolved: &ResolvedImport,
) -> Result<SnapshotVerifiedExports, ArtifactSnapshotError> {
    let mut replay = ExportReplay {
        snapshot,
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
    if names != resolved.exports.keys().cloned().collect() {
        return export_mismatch(
            "supplied export names do not equal the runtime/declaration intersection",
        );
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
        verify_binding(snapshot, resolved, supplied, &runtime, &declarations)?;
        bindings.insert(
            name,
            VerifiedExportBinding {
                runtime_path: runtime.file,
                runtime_export: runtime.name,
                declarations_path: declarations.file,
                declarations_export: declarations.name,
            },
        );
    }
    let mut evidence_fields = vec![snapshot.root().to_owned()];
    for (name, binding) in &bindings {
        evidence_fields.extend([
            name.clone(),
            binding.runtime_path.clone(),
            binding.runtime_export.clone(),
            binding.declarations_path.clone(),
            binding.declarations_export.clone(),
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

fn verify_binding(
    snapshot: &ArtifactSnapshot,
    resolved: &ResolvedImport,
    supplied: &ResolvedExportBinding,
    runtime: &BindingTarget,
    declarations: &BindingTarget,
) -> Result<(), ArtifactSnapshotError> {
    verify_resolved_file(snapshot, resolved, &supplied.runtime.module, &runtime.file)?;
    verify_resolved_file(
        snapshot,
        resolved,
        &supplied.declarations.module,
        &declarations.file,
    )?;
    if supplied.runtime.export_name != runtime.name
        || supplied.declarations.export_name != declarations.name
    {
        return export_mismatch("supplied export target name disagrees with snapshot replay");
    }
    Ok(())
}

#[derive(Clone, Debug, Default)]
struct ModuleDescription {
    direct: BTreeMap<String, BindingTarget>,
    stars: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BindingTarget {
    file: String,
    name: String,
}

struct ExportReplay<'a> {
    snapshot: &'a ArtifactSnapshot,
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
        for import in &facts.imports {
            let LocalResolution::Module(target) =
                resolve_local(self.snapshot, path, &import.module, axis)?
            else {
                continue;
            };
            for binding in &import.bindings {
                if binding.kind == ImportKind::SideEffect || binding.kind == ImportKind::Namespace {
                    continue;
                }
                let local = span_text(source, binding.local.span.start, binding.local.span.end)?;
                let Some(imported) = &binding.imported else {
                    continue;
                };
                imports.insert(
                    local.into(),
                    BindingTarget {
                        file: target.clone(),
                        name: imported.to_string(),
                    },
                );
            }
        }

        for export in &facts.exports {
            let target = export
                .module
                .as_deref()
                .map(|specifier| resolve_local(self.snapshot, path, specifier, axis))
                .transpose()?
                .and_then(|resolution| match resolution {
                    LocalResolution::Module(target) => Some(target),
                    _ => None,
                });
            match export.kind {
                ExportKind::All => {
                    if let Some(target) = target {
                        if let Some(namespace) = &export.namespace {
                            description.direct.insert(
                                namespace.to_string(),
                                BindingTarget {
                                    file: target,
                                    name: "*".into(),
                                },
                            );
                        } else {
                            description.stars.push(target);
                        }
                    }
                }
                ExportKind::Named => {
                    for specifier in &export.specifiers {
                        let local = span_text(
                            source,
                            specifier.local.span.start,
                            specifier.local.span.end,
                        )?;
                        let binding = target.as_ref().map_or_else(
                            || {
                                imports
                                    .get(local)
                                    .cloned()
                                    .unwrap_or_else(|| BindingTarget {
                                        file: path.into(),
                                        name: local.into(),
                                    })
                            },
                            |target| BindingTarget {
                                file: target.clone(),
                                name: local.into(),
                            },
                        );
                        description
                            .direct
                            .insert(specifier.exported.to_string(), binding);
                    }
                    for declaration in &export.declarations {
                        description.direct.insert(
                            declaration.exported.to_string(),
                            BindingTarget {
                                file: path.into(),
                                name: declaration.exported.to_string(),
                            },
                        );
                    }
                }
                ExportKind::Default => {
                    description.direct.insert(
                        "default".into(),
                        BindingTarget {
                            file: path.into(),
                            name: "default".into(),
                        },
                    );
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
        for target in description.stars {
            names.extend(
                self.exported_names(&target, axis, visiting)?
                    .into_iter()
                    .filter(|name| name != "default"),
            );
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
        if name == "default" {
            visiting.remove(&identity);
            return Ok(None);
        }

        let mut candidates = BTreeMap::<(String, String), BindingTarget>::new();
        for target in description.stars {
            if let Some(candidate) = self.bind_export(&target, name, axis, visiting)? {
                let bytes = self.snapshot.read(&candidate.file).ok_or_else(|| {
                    ArtifactSnapshotError::ExportBindings(format!(
                        "export target {:?} is absent from the snapshot",
                        candidate.file
                    ))
                })?;
                candidates.insert(
                    (
                        format!("sha256:{:x}", Sha256::digest(bytes)),
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
}

fn span_text(source: &str, start: u32, end: u32) -> Result<&str, ArtifactSnapshotError> {
    source
        .get(start as usize..end as usize)
        .ok_or_else(|| ArtifactSnapshotError::ExportBindings("export span is invalid".into()))
}

fn export_mismatch<T>(reason: impl Into<String>) -> Result<T, ArtifactSnapshotError> {
    Err(ArtifactSnapshotError::ExportBindings(reason.into()))
}
