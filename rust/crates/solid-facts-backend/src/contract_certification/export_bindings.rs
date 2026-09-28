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
    /// The names this replay withheld as foreign (ADR 0150) or forwarded
    /// foreign (ADR 0154): unavailable exports whose runtime binding the
    /// replay proved exact. A dependent's replay reads this, and only this, to
    /// recognise a forward of one; the resolver's copy is never trusted.
    withheld: BTreeSet<String>,
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

    /// Authenticated snapshot root of the package that *owns* `name`'s
    /// declaration binding.
    ///
    /// `declaration_binding` returns a path relative to that owner, which for
    /// an export re-exported from a dependency is the dependency's package and
    /// not this snapshot's. Any consumer that turns the path into a module
    /// specifier must join it onto the owner's root; this is how the owner is
    /// identified, using the same root `verify_target` matches a planned
    /// dependency by.
    pub(super) fn declaration_binding_snapshot_root(&self, name: &str) -> Option<&str> {
        self.bindings
            .get(name)
            .map(|binding| binding.declarations_snapshot_root.as_str())
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

    /// The exact declaration-side reference replayed for this export. A
    /// consumer must resolve this span in the authenticated owner snapshot;
    /// the export spelling alone is never a callee identity.
    pub(super) fn declaration_reference(&self, name: &str) -> Option<(&str, &str, Span, &str)> {
        let binding = self.bindings.get(name)?;
        Some((
            &binding.declarations_path,
            &binding.declarations_export,
            binding.declarations_span?,
            &binding.declarations_snapshot_root,
        ))
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

/// ADR 0156: what a dependent's replay may know about a planned dependency
/// node that ADR 0129 pruned. The node was planned from its own archive in
/// the same transaction and proved statementless there
/// (`CertificationPlan::pruned_dependency_evidence`); this is never an
/// accepted dependency and nothing binds through it. It answers one question:
/// does the node's own package export `name`, on both axes, exactly?
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrunedDependencyEvidence {
    pub(super) package_name: String,
    pub(super) specifier: String,
    pub(super) importer: String,
    pub(super) conditions: Vec<String>,
    /// Names whose replayed runtime and declaration bindings both terminate in
    /// the pruned node's own snapshot.
    pub(super) exact_exports: BTreeSet<String>,
}

impl PrunedDependencyEvidence {
    #[must_use]
    pub fn specifier(&self) -> &str {
        &self.specifier
    }

    #[must_use]
    pub fn conditions(&self) -> &[String] {
        &self.conditions
    }
}

impl SnapshotVerifiedExports {
    /// The names whose runtime and declaration bindings both terminate in
    /// this snapshot.
    pub(super) fn own_exact_names(&self) -> BTreeSet<String> {
        self.bindings
            .iter()
            .filter(|(_, binding)| {
                binding.runtime_snapshot_root == self.snapshot_root
                    && binding.declarations_snapshot_root == self.snapshot_root
            })
            .map(|(name, _)| name.clone())
            .collect()
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
    pruned: &[PrunedDependencyEvidence],
) -> Result<SnapshotVerifiedExports, ArtifactSnapshotError> {
    let mut replay = ExportReplay {
        snapshot,
        dependencies,
        pruned,
        package_root: resolved.package_root.as_str(),
        closure_entries: &resolved.closure.entries,
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
    let declaration_surface_names =
        replay.declaration_surface_names(resolution.declarations_path(), &mut BTreeSet::new())?;
    if !resolved.declaration_exports.is_empty()
        && declaration_surface_names != resolved.declaration_exports
    {
        let replayed_only = declaration_surface_names
            .difference(&resolved.declaration_exports)
            .take(8)
            .cloned()
            .collect::<Vec<_>>();
        let supplied_only = resolved
            .declaration_exports
            .difference(&declaration_surface_names)
            .take(8)
            .cloned()
            .collect::<Vec<_>>();
        return export_mismatch(format!(
            "supplied declaration export census does not equal archive replay; replayedOnly(count={}, sample={replayed_only:?}); suppliedOnly(count={}, sample={supplied_only:?})",
            declaration_surface_names
                .difference(&resolved.declaration_exports)
                .count(),
            resolved
                .declaration_exports
                .difference(&declaration_surface_names)
                .count(),
        ));
    }
    let names = runtime_names
        .intersection(&declaration_names)
        .cloned()
        .collect::<BTreeSet<_>>();
    // ADR 0128: a name whose runtime binding is exact but whose declaration
    // re-export chain ends in a module of this package that publishes no
    // export by it is unavailable, not a reason to refuse every other export.
    // The census is the generator's `declarationReexportGap`, replayed here
    // from the archive bytes; the two must agree exactly, so a resolver that
    // omits a bindable name, or names a gap the bytes do not show, refuses.
    // Only the declaration axis qualifies: a runtime re-export of an
    // undeclared name fails the module graph at link time, and that name
    // stays below, where its missing runtime binding refuses the case.
    let mut unbound = BTreeSet::new();
    for name in &names {
        if replay.declaration_reexport_gap(
            resolution.declarations_path(),
            name,
            &mut BTreeSet::new(),
        )? && replay
            .bind_export(
                resolution.runtime_path(),
                name,
                ModuleAxis::Runtime,
                &mut BTreeSet::new(),
            )?
            .is_some()
        {
            unbound.insert(name.clone());
        }
    }
    if unbound != resolved.unbound_declaration_exports {
        return export_mismatch(format!(
            "supplied unbound declaration exports do not equal archive replay; replayed {unbound:?}; supplied {:?}",
            resolved.unbound_declaration_exports,
        ));
    }
    let names = names.difference(&unbound).cloned().collect::<BTreeSet<_>>();
    // ADR 0150: a name whose runtime binding is exact and this package's own
    // definition, while its declaration binding is exact and another
    // package's declaration, binds two different entities. The types describe
    // different code, so neither axis describes the export, and it is
    // unavailable exactly as an ADR 0128 gap is. The census is the
    // generator's `foreignDeclarationOwner`, replayed here over the archive
    // and the planned dependencies' snapshots; the two must agree exactly.
    let mut foreign = BTreeSet::new();
    for name in &names {
        let Some(runtime) = replay.bind_export(
            resolution.runtime_path(),
            name,
            ModuleAxis::Runtime,
            &mut BTreeSet::new(),
        )?
        else {
            continue;
        };
        let Some(declarations) = replay.bind_export(
            resolution.declarations_path(),
            name,
            ModuleAxis::Declarations,
            &mut BTreeSet::new(),
        )?
        else {
            continue;
        };
        let own = snapshot.package_name();
        if replay.target_package(&runtime) == Some(own)
            && replay
                .target_package(&declarations)
                .is_some_and(|owner| owner != own)
        {
            foreign.insert(name.clone());
        }
    }
    if foreign != resolved.foreign_declaration_exports {
        return export_mismatch(format!(
            "supplied foreign declaration exports do not equal archive replay; replayed {foreign:?}; supplied {:?}",
            resolved.foreign_declaration_exports,
        ));
    }
    let names = names.difference(&foreign).cloned().collect::<BTreeSet<_>>();
    // ADR 0154: a name both axes forward, through exact named re-export
    // chains, as the same name of the same planned dependency, which that
    // dependency's own verified replay withheld (ADR 0150, or this rule), is
    // that unavailable export. The generator's `withheldDependencyExport`
    // reads the dependency record's `withheldExports`; this replay reads the
    // dependency plan's recomputed set instead, so a forged or stale record
    // disagrees here and refuses.
    //
    // ADR 0156 widens the runtime axis. The runtime forwards exactly one
    // withheld export -- a planned dependency's recomputed withheld name, or
    // an own exact export of a dependency node ADR 0129 pruned, per that
    // node's replayed evidence -- and the declaration either forwards the
    // same name or binds exactly here, wherever it lives. Withheld, never
    // bound. Every premise is replayed: the runtime graph links (the
    // dependency's replay proved that runtime binding exact), the declaration
    // exists (`bind_export` over the bytes), and a pruned node states nothing
    // (`pruned_dependency_evidence`).
    let mut forwarded = BTreeSet::new();
    let mut runtime_withheld = BTreeSet::new();
    for name in &names {
        let Some(runtime) = replay.forwarded_withheld(
            resolution.runtime_path(),
            name,
            ModuleAxis::Runtime,
            &mut BTreeSet::new(),
        )?
        else {
            continue;
        };
        let pruned = runtime.0;
        let declarations = replay.forwarded_withheld(
            resolution.declarations_path(),
            name,
            ModuleAxis::Declarations,
            &mut BTreeSet::new(),
        )?;
        if declarations.as_ref() == Some(&runtime) {
            if pruned {
                runtime_withheld.insert(name.clone());
            } else {
                forwarded.insert(name.clone());
            }
        } else if declarations.is_none()
            && replay
                .bind_export(
                    resolution.declarations_path(),
                    name,
                    ModuleAxis::Declarations,
                    &mut BTreeSet::new(),
                )?
                .is_some()
        {
            runtime_withheld.insert(name.clone());
        }
    }
    if forwarded != resolved.forwarded_foreign_exports {
        return export_mismatch(format!(
            "supplied forwarded foreign exports do not equal archive replay; replayed {forwarded:?}; supplied {:?}",
            resolved.forwarded_foreign_exports,
        ));
    }
    if runtime_withheld != resolved.runtime_withheld_exports {
        return export_mismatch(format!(
            "supplied runtime-withheld exports do not equal archive replay; replayed {runtime_withheld:?}; supplied {:?}",
            resolved.runtime_withheld_exports,
        ));
    }
    let names = names
        .difference(&forwarded)
        .filter(|name| !runtime_withheld.contains(*name))
        .cloned()
        .collect::<BTreeSet<_>>();
    let withheld = foreign
        .iter()
        .chain(&forwarded)
        .chain(&runtime_withheld)
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
        withheld,
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
    verify_snapshot_exports_with_dependencies(snapshot, resolution, resolved, &[], &[])
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
    declaration_surface_only: BTreeSet<String>,
    stars: Vec<String>,
    external_direct: BTreeMap<String, (String, String)>,
    external_stars: Vec<String>,
    /// Every name a module-level export statement publishes, in either space
    /// and by any spelling. Read only by `declaration_reexport_gap`, to prove
    /// a module publishes *no* export by a name; the generator's
    /// `declaredNames`.
    declared_names: BTreeSet<String>,
    /// An `export *` this replay does not follow as a local value star: a
    /// type-only one, or one whose source is not a module of this package.
    /// The generator's `unfollowedExportSource`, less `export =`, which the
    /// syntax facts do not record (see `has_export_assignment`).
    unfollowed_star: bool,
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
    /// ADR 0156: pruned dependency nodes of the same transaction. Consulted
    /// only by `forwarded_withheld`, never by a binding.
    pruned: &'a [PrunedDependencyEvidence],
    /// This node's own installed package root and replayed closure entries,
    /// which `external_dependency` uses to tell *this* package's dependency
    /// edge from a homonymous edge reached through a descendant package (which
    /// may name a different installed copy). Empty entries break no tie, so a
    /// repeated specifier then stays refused, exactly as before this scope
    /// existed.
    package_root: &'a str,
    closure_entries: &'a [crate::artifact_resolution::ClosureEntry],
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
            description.declared_names.extend(
                export
                    .specifiers
                    .iter()
                    .chain(&export.declarations)
                    .chain(&export.declaration_surface_only)
                    .map(|specifier| specifier.exported.to_string())
                    .chain(export.namespace.as_ref().map(ToString::to_string)),
            );
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
            if export.kind == ExportKind::All
                && export.namespace.is_none()
                && (export.type_only || target.is_none())
            {
                description.unfollowed_star = true;
            }
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
                    let declaration_surface_only = export
                        .declaration_surface_only
                        .iter()
                        .map(|declaration| declaration.local.span)
                        .collect::<BTreeSet<_>>();
                    description.declaration_surface_only.extend(
                        export
                            .declaration_surface_only
                            .iter()
                            .map(|declaration| declaration.exported.to_string()),
                    );
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
                        if axis == ModuleAxis::Declarations
                            && declaration_surface_only.contains(&declaration.local.span)
                        {
                            description
                                .declaration_surface_only
                                .insert(declaration.exported.to_string());
                            continue;
                        }
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
        // ...unless this package *is* the foundation. `solid-js` re-exporting
        // from `solid-js/...` is publishing its own surface, and ADR 0027's
        // reason for dropping a core re-export -- that the package has no
        // standing to describe a name it only forwards -- does not apply to the
        // package the dialect takes that behavior from. Without this guard the
        // replay dropped thirteen of `solid-js`'s own exports (`For`, `Show`,
        // `Switch`, `Suspense`, ...) and refused every graph that composed it.
        let foreign_core = !solid_dialect::primitive_defining_package(self.snapshot.package_name());
        // A name re-exported straight from the built-in runtime foundation is
        // not part of this package's surface (ADR 0027): `solid-js`,
        // `@solidjs/signals` and `@solidjs/web` have no package contract that
        // could ever bind it, and ordinary analysis takes their behavior from
        // the selected dialect instead. The emitter drops it, the resolver
        // returns it unbound, and this replay is the fourth census that has to
        // agree — it computes the surface the supplied export map is compared
        // against, so keeping the name here refused every package with one core
        // re-export beside its own exports (`@solid-primitives/utils`'
        // `isServer`, `@solidjs/start`'s `mount`).
        names.extend(
            description
                .external_direct
                .iter()
                .filter(|(_, (specifier, _))| {
                    !(foreign_core && solid_dialect::core_runtime_specifier(specifier))
                })
                .map(|(name, _)| name.clone()),
        );
        // A local re-export of one is the same name by another route:
        // `@solidjs/start`'s `dist/client/index.jsx` says
        // `export { mount } from "./mount.js"`, and that file says
        // `export { hydrate as mount } from "solid-js/web"`. The resolver
        // follows the chain and drops it; so must this.
        let core_bound = description
            .direct
            .iter()
            .filter(|_| foreign_core)
            .filter(|(_, target)| target.file != path && target.name != "*")
            .map(|(name, target)| {
                Ok::<_, ArtifactSnapshotError>(
                    self.binds_core_runtime(
                        &target.file,
                        &target.name,
                        axis,
                        &mut BTreeSet::new(),
                    )?
                    .then(|| name.clone()),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        for name in core_bound.into_iter().flatten() {
            names.remove(&name);
        }
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

    fn declaration_surface_names(
        &mut self,
        path: &str,
        visiting: &mut BTreeSet<(ModuleAxis, String)>,
    ) -> Result<BTreeSet<String>, ArtifactSnapshotError> {
        let axis = ModuleAxis::Declarations;
        let identity = (axis, path.into());
        if !visiting.insert(identity.clone()) {
            return Ok(BTreeSet::new());
        }
        let description = self.description(path, axis)?;
        let mut names = description.direct.keys().cloned().collect::<BTreeSet<_>>();
        names.extend(description.declaration_surface_only);
        names.extend(description.external_direct.keys().cloned());
        for target in description.stars {
            names.extend(
                self.declaration_surface_names(&target, visiting)?
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

    /// Whether binding `name` from `path` terminates at the built-in runtime
    /// foundation.
    ///
    /// Mirrors exactly the two arms of [`Self::bind_export`] that can reach an
    /// external specifier by an exact name — a local re-export chain and a
    /// direct external re-export — and answers `false` for every other shape.
    /// A star is deliberately not followed: `exported_names` already takes a
    /// star into an external package from that dependency's own verified
    /// exports, which a core specifier has none of, so the name never arrives
    /// by that route in the first place.
    ///
    /// Answering `false` when unsure keeps a name on the surface, which is the
    /// conservative direction here: an extra name refuses loudly at the
    /// intersection check, where a missing one would silently shrink a
    /// published contract.
    fn binds_core_runtime(
        &mut self,
        path: &str,
        name: &str,
        axis: ModuleAxis,
        visiting: &mut BTreeSet<(ModuleAxis, String, String)>,
    ) -> Result<bool, ArtifactSnapshotError> {
        let identity = (axis, path.into(), name.into());
        if !visiting.insert(identity.clone()) {
            return Ok(false);
        }
        let description = self.description(path, axis)?;
        let answer = if let Some((specifier, _)) = description.external_direct.get(name) {
            solid_dialect::core_runtime_specifier(specifier)
        } else if let Some(direct) = description.direct.get(name) {
            let (file, target) = (direct.file.clone(), direct.name.clone());
            if file == path || target == "*" {
                false
            } else {
                self.binds_core_runtime(&file, &target, axis, visiting)?
            }
        } else {
            false
        };
        visiting.remove(&identity);
        Ok(answer)
    }

    /// Whether `name`, looked up in declaration module `path`, reaches a
    /// module of this package that publishes no export by that name at all.
    ///
    /// The byte-for-byte mirror of the generator's `declarationReexportGap`
    /// (`packages/cli/scripts/artifact-resolution.mjs`); change the two
    /// together. `true` implies [`Self::bind_export`] answers `None` on the
    /// declaration axis, and every shape this walk cannot see through answers
    /// `false`, leaving the name to its existing refusal: a module that
    /// declares the name in any space or by any spelling, forwards it from
    /// outside the package, has an `export =` or an unfollowed `export *`,
    /// and a cycle.
    fn declaration_reexport_gap(
        &mut self,
        path: &str,
        name: &str,
        visiting: &mut BTreeSet<(String, String)>,
    ) -> Result<bool, ArtifactSnapshotError> {
        let identity = (path.to_owned(), name.to_owned());
        if !visiting.insert(identity.clone()) {
            return Ok(false);
        }
        let description = self.description(path, ModuleAxis::Declarations)?;
        let gap = if let Some(direct) = description.direct.get(name) {
            direct.file != path
                && direct.name != "*"
                && direct.snapshot_root == self.snapshot.root()
                && self.declaration_reexport_gap(&direct.file, &direct.name, visiting)?
        } else if description.external_direct.contains_key(name)
            || description.declaration_surface_only.contains(name)
            || description.declared_names.contains(name)
            || description.unfollowed_star
            || !description.external_stars.is_empty()
            || self.has_export_assignment(path)?
        {
            false
        } else if name == "default" {
            // ESM `export *` never forwards a default export.
            true
        } else {
            let mut every = true;
            for target in &description.stars {
                if !self.declaration_reexport_gap(target, name, visiting)? {
                    every = false;
                    break;
                }
            }
            every
        };
        visiting.remove(&identity);
        Ok(gap)
    }

    fn has_export_assignment(&self, path: &str) -> Result<bool, ArtifactSnapshotError> {
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
        solid_facts::ast::has_export_assignment(source).map_err(|error| {
            ArtifactSnapshotError::ExportBindings(format!(
                "export module {path:?} cannot be parsed: {error}"
            ))
        })
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

    /// The one planned dependency that *this* package's own import of
    /// `specifier` resolved to.
    ///
    /// `dependencies` is the whole authenticated descendant set, because an
    /// export target can terminate more than one accepted re-export edge
    /// away. The set therefore repeats a specifier whenever two packages in
    /// the graph depend on the same one -- a diamond, which is the ordinary
    /// shape, not an exceptional one (`motion-solidjs` and `framer-motion`
    /// both depend on `motion-utils`). Selecting by specifier alone made every
    /// such repeat ambiguous and bound nothing at all.
    ///
    /// A repeat is disambiguated by the importer, using the same authoritative
    /// edge matcher `plan_published_contract_graph` checks node identity with:
    /// the plan whose importer is a proven runtime or declaration module of
    /// *this* package's replayed closure is this package's own edge, and a
    /// homonymous specifier reached from a descendant package is a different
    /// edge that may name a different installed copy. The narrowing is applied
    /// only to break a tie, so a single unambiguous match keeps binding
    /// exactly as before, and a tie no narrowing resolves stays refused.
    fn external_dependency(&self, specifier: &str) -> Option<&super::CertificationPlan> {
        let matches = self
            .dependencies
            .iter()
            .copied()
            .filter(|dependency| dependency.import_request.specifier == specifier)
            .collect::<Vec<_>>();
        if let [dependency] = matches.as_slice() {
            return Some(dependency);
        }
        let mut owned = matches.into_iter().filter(|dependency| {
            super::dependencies::importer_is_closure_entry_module(
                &dependency.import_request.importer,
                self.package_root,
                self.closure_entries,
            )
        });
        let dependency = owned.next()?;
        owned.next().is_none().then_some(dependency)
    }

    /// The planned dependency's package and export name that `name` forwards,
    /// on `axis`, through exact named re-export chains, when that dependency's
    /// verified replay withheld the name (ADR 0150 or ADR 0154) and binds
    /// nothing by it.
    ///
    /// The generator's `withheldDependencyExport` mirror. A local `export *`
    /// is followed only when every star that reaches the name reaches this
    /// same withheld name and none binds it. A local definition, a default, a
    /// cycle, a dependency with a binding by that name, or one that did not
    /// withhold it answers `None`, which leaves the name to its existing
    /// refusal.
    fn forwarded_withheld(
        &mut self,
        path: &str,
        name: &str,
        axis: ModuleAxis,
        visiting: &mut BTreeSet<(ModuleAxis, String, String)>,
    ) -> Result<Option<(bool, String, String)>, ArtifactSnapshotError> {
        let identity = (axis, path.to_owned(), name.to_owned());
        if !visiting.insert(identity.clone()) {
            return Ok(None);
        }
        let description = self.description(path, axis)?;
        let answer = if let Some(direct) = description.direct.get(name) {
            if direct.file == path || direct.name == "*" {
                None
            } else {
                self.forwarded_withheld(&direct.file, &direct.name, axis, visiting)?
            }
        } else if let Some((specifier, imported)) = description.external_direct.get(name) {
            match self.external_dependency(specifier) {
                Some(dependency) => {
                    let verified = &dependency.verified_exports;
                    (!verified.bindings.contains_key(imported)
                        && verified.withheld.contains(imported))
                    .then(|| {
                        (
                            false,
                            dependency.snapshot.package_name().to_owned(),
                            imported.clone(),
                        )
                    })
                }
                // ADR 0156: no planned dependency answers the specifier, and
                // the one pruned node of this package's own edge exports the
                // name exactly.
                None => self.pruned_dependency(specifier).and_then(|pruned| {
                    pruned
                        .exact_exports
                        .contains(imported)
                        .then(|| (true, pruned.package_name.clone(), imported.clone()))
                }),
            }
        } else if name == "default" {
            None
        } else {
            let mut withheld = BTreeSet::new();
            let mut bound = false;
            for target in &description.stars {
                if let Some(found) = self.forwarded_withheld(target, name, axis, visiting)? {
                    withheld.insert(found);
                } else if self
                    .bind_export(target, name, axis, &mut BTreeSet::new())?
                    .is_some()
                {
                    bound = true;
                }
            }
            bound |= description
                .external_stars
                .iter()
                .any(|specifier| self.external_binding(specifier, name, axis).is_some());
            (!bound && withheld.len() == 1)
                .then(|| withheld.into_iter().next())
                .flatten()
        };
        visiting.remove(&identity);
        Ok(answer)
    }

    /// The one pruned dependency node (ADR 0156) that *this* package's own
    /// import of `specifier` resolved to, by the same importer rule
    /// `external_dependency` breaks ties with; `None` when there is not
    /// exactly one.
    fn pruned_dependency(&self, specifier: &str) -> Option<&PrunedDependencyEvidence> {
        let mut owned = self.pruned.iter().filter(|pruned| {
            pruned.specifier == specifier
                && super::dependencies::importer_is_closure_entry_module(
                    &pruned.importer,
                    self.package_root,
                    self.closure_entries,
                )
        });
        let pruned = owned.next()?;
        owned.next().is_none().then_some(pruned)
    }

    /// The package a replayed target belongs to: this package for a target in
    /// its own snapshot, the planned dependency's package for one in that
    /// dependency's snapshot, and `None` for a snapshot no planned dependency
    /// owns (which then refuses at `verify_target`).
    ///
    /// The generator's `bindingOwner` mirror: package identity, not snapshot
    /// identity, so a self-package edge (ADR 0012) counts as this package's own
    /// on both sides.
    fn target_package(&self, target: &BindingTarget) -> Option<&str> {
        if target.snapshot_root == self.snapshot.root() {
            return Some(self.snapshot.package_name());
        }
        let mut owners = self
            .dependencies
            .iter()
            .filter(|dependency| dependency.snapshot.root() == target.snapshot_root)
            .map(|dependency| dependency.snapshot.package_name());
        let owner = owners.next()?;
        owners.all(|other| other == owner).then_some(owner)
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
            pruned: &[],
            package_root: "/project/node_modules/source-types",
            closure_entries: &[],
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
    fn nested_namespace_blocks_do_not_reclassify_an_exported_value_declaration() {
        let snapshot = snapshot(&[(
            "index.ts",
            "export const value = function () { namespace Hidden {} return 1; };",
        )]);
        let mut replay = ExportReplay {
            snapshot: &snapshot,
            dependencies: &[],
            pruned: &[],
            package_root: "/project/node_modules/source-types",
            closure_entries: &[],
            descriptions: BTreeMap::new(),
        };

        let description = replay
            .description("index.ts", ModuleAxis::Declarations)
            .unwrap();
        assert!(description.direct.contains_key("value"));
        assert!(!description.declaration_surface_only.contains("value"));
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
            pruned: &[],
            package_root: "/project/node_modules/source-types",
            closure_entries: &[],
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
            pruned: &[],
            package_root: "/project/node_modules/source-types",
            closure_entries: &[],
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
            pruned: &[],
            package_root: "/project/node_modules/source-types",
            closure_entries: &[],
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
            pruned: &[],
            package_root: "/project/node_modules/source-types",
            closure_entries: &[],
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
            withheld: BTreeSet::new(),
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
