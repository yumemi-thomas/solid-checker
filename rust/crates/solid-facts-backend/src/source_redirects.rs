//! Runtime symbol redirects for analyzed dependency source (ADRs 0284, 0291),
//! shared by the one-shot check and the retained daemon.
//! Moved here from the checker binary unchanged so the library's shared check
//! operation binds a selection exactly as the binary always has.

#![allow(clippy::too_many_lines)]

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    path::{Path, PathBuf},
};

use crate::TypeFactsSession;

/// ADR 0284: the redirects for the dependency-source mode. Every project file
/// importing a selected package by its bare specifier is bound to the entry
/// the package loads, and each relative import inside a package to the file
/// it loads, when the compiler resolved them to declarations.
pub(crate) fn dependency_source_redirects(
    facts: &solid_facts::ProjectFacts,
    typescript: &mut TypeFactsSession,
    sources: &crate::dependency_sources::PreparedDependencyProgram,
) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    for resolution in &sources.selection.resolutions {
        match &resolution.importer {
            Some(importer) => {
                rows.push((
                    importer.clone(),
                    resolution.specifier.clone(),
                    resolution.target.clone(),
                ));
            }
            None => {
                for file in &facts.files {
                    let Ok(path) = Path::new(file.path.as_str()).canonicalize() else {
                        continue;
                    };
                    if !(file
                        .ast
                        .imports
                        .iter()
                        .any(|import| import.module.as_str() == resolution.specifier)
                        || file.ast.exports.iter().any(|export| {
                            export.module.as_deref() == Some(resolution.specifier.as_str())
                        }))
                    {
                        continue;
                    }
                    rows.push((
                        path,
                        resolution.specifier.clone(),
                        resolution.target.clone(),
                    ));
                }
            }
        }
    }
    // Project barrels can be imported outside the configured roots. Their
    // exact relative edges are needed to carry a star's demanded names to
    // the selected runtime export, rather than to the declaration barrel.
    for file in &facts.files {
        let Ok(path) = Path::new(file.path.as_str()).canonicalize() else {
            continue;
        };
        if sources.contains(&path) {
            continue;
        }
        for module in file
            .ast
            .imports
            .iter()
            .filter(|edge| !edge.type_only)
            .map(|edge| edge.module.as_str())
            .chain(
                file.ast
                    .exports
                    .iter()
                    .filter(|edge| !edge.type_only)
                    .filter_map(|edge| edge.module.as_deref()),
            )
            .filter(|module| module.starts_with('.'))
        {
            if let Ok(target) = resolve_relative_export(facts, &path, module) {
                rows.push((path.clone(), module.to_owned(), target));
            }
        }
    }
    symbol_redirects(facts, typescript, rows, |_, target, declaration| {
        let declaration = declaration
            .canonicalize()
            .unwrap_or_else(|_| declaration.to_path_buf());
        (same_canonical_path(target, &declaration) && sources.project_files.contains(target))
            || sources
                .selection
                .roots
                .iter()
                .any(|root| target.starts_with(root) && declaration.starts_with(root))
    })
}

/// The redirects `rows` (importer, specifier, runtime target, all canonical)
/// state, for the rows `admitted` accepts: each runtime-referenced value
/// import the compiler resolved to declarations is bound to the export entity
/// of the runtime file it loads. A source bound to two targets binds none.
pub fn symbol_redirects(
    facts: &solid_facts::ProjectFacts,
    typescript: &mut TypeFactsSession,
    rows: Vec<(PathBuf, String, PathBuf)>,
    admitted: impl Fn(&Path, &Path, &Path) -> bool,
) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let import_paths = rows
        .iter()
        .map(|(importer, _, _)| importer.to_string_lossy().into_owned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let graph = typescript
        .module_graph(&typefacts::ModuleGraphDemand::default().import_paths(import_paths))?;
    if !graph.is_complete() {
        return Ok(HashMap::new());
    }
    let aliases = canonical_symbol_aliases(facts);
    let files_by_path = facts
        .files
        .iter()
        .filter_map(|file| {
            Path::new(file.path.as_str())
                .canonicalize()
                .ok()
                .map(|path| (path, file))
        })
        .collect::<HashMap<_, _>>();
    let entities_by_location = facts
        .typescript
        .entities()
        .map(|entity| (entity.location.clone(), entity))
        .collect::<HashMap<_, _>>();
    let mut declarations = HashMap::<(PathBuf, String), BTreeSet<PathBuf>>::new();
    for import in &graph.imports {
        if import.included_path.is_empty()
            && !import.resolved_path.is_empty()
            && let Ok(importer) = Path::new(import.specifier.path.as_ref()).canonicalize()
        {
            declarations
                .entry((importer, import.text.to_string()))
                .or_default()
                .insert(Path::new(import.resolved_path.as_ref()).canonicalize()?);
        }
    }
    let mut edge_targets = HashMap::<(PathBuf, String), BTreeSet<PathBuf>>::new();
    for (importer, specifier, target) in &rows {
        // Admission belongs to every edge, not just the outer import whose
        // symbol is redirected. Otherwise an admitted relative barrel can
        // traverse a row for a different installed copy (ADR 0297 round 3).
        let Some(declaration) = declarations
            .get(&(importer.clone(), specifier.clone()))
            .filter(|paths| paths.len() == 1)
            .and_then(|paths| paths.first())
        else {
            continue;
        };
        if !admitted(importer, target, declaration) {
            continue;
        }
        edge_targets
            .entry((importer.clone(), specifier.clone()))
            .or_default()
            .insert(target.clone());
    }
    let runtime_edges = edge_targets
        .into_iter()
        .filter_map(|(edge, targets)| {
            (targets.len() == 1).then(|| (edge, targets.into_iter().next().unwrap()))
        })
        .collect::<HashMap<_, _>>();
    let entity_at = |file: &solid_facts::FileFacts, span: solid_facts::core::Span| {
        entities_by_location
            .get(&typefacts::Location {
                path: file.path.to_string().into(),
                start_byte: u64::from(span.start),
                end_byte: u64::from(span.end),
            })
            .copied()
    };
    let mut redirects = HashMap::<String, String>::new();
    let mut ambiguous = HashSet::new();
    for (importer_path, specifier, target_path) in rows {
        // The declaration the compiler resolved this import to, which
        // `admitted` judges beside the runtime target: another installed
        // copy of the package resolves elsewhere (ADR 0284 review round 2).
        // Or, for a relative import inside a package analyzed as source, the
        // runtime file the compiler picked when the bundler loads another
        // (`./read` is `read.js` to TypeScript, `read.mjs` to Vite; review
        // round 1 of ADR 0290). `admitted` decides which kinds a caller takes.
        let Some(declaration) = declarations
            .get(&(importer_path.clone(), specifier.clone()))
            .filter(|paths| paths.len() == 1)
            .and_then(|paths| paths.first())
        else {
            continue;
        };
        if !admitted(&importer_path, &target_path, declaration) {
            continue;
        }
        let Some(importer) = files_by_path.get(&importer_path).copied() else {
            continue;
        };
        let mut pairs = Vec::new();
        // The whole spans of namespace members (`X.member`, `<X.Member>`): an
        // entity there names the member only when its symbol covers the span;
        // otherwise it is the leading token, the namespace (ADR 0376).
        let mut whole_members = HashSet::new();
        let imports = importer
            .ast
            .imports
            .iter()
            .filter(|import| !import.type_only && import.module.as_str() == specifier);
        for import in imports {
            let bindings = import.bindings.iter().filter(|binding| {
                binding.runtime_referenced
                    && !binding.type_only
                    && matches!(
                        binding.kind,
                        solid_facts::ast::ImportKind::Named | solid_facts::ast::ImportKind::Default
                    )
            });
            // ADR 0322: a namespace import whose every runtime reference is a
            // static member (`X.member`, `<X.Member>`) binds each member it
            // names, exactly as a named import of that export would.
            for binding in import.bindings.iter().filter(|binding| {
                binding.runtime_referenced
                    && !binding.type_only
                    && binding.kind == solid_facts::ast::ImportKind::Namespace
            }) {
                if let Some(members) = namespace_static_members(importer, binding.local.span) {
                    whole_members.extend(
                        members
                            .chunks(2)
                            .filter_map(|pair| pair.get(1))
                            .map(|(span, _)| *span),
                    );
                    pairs.extend(members);
                }
            }
            for binding in bindings {
                let exported = match binding.kind {
                    solid_facts::ast::ImportKind::Named => binding.imported.as_deref(),
                    solid_facts::ast::ImportKind::Default => Some("default"),
                    _ => None,
                };
                let Some(exported) = exported else {
                    continue;
                };
                pairs.push((binding.local.span, exported.to_owned()));
            }
        }
        for export in importer.ast.module_level_exports().filter(|export| {
            !export.type_only
                && export.module.as_deref() == Some(specifier.as_str())
                && export.namespace.is_none()
        }) {
            for binding in export
                .specifiers
                .iter()
                .filter(|binding| !binding.type_only)
            {
                if let Some(name) = importer.source_text(binding.local.span) {
                    pairs.push((binding.local.span, name.to_owned()));
                }
            }
        }
        for (span, exported) in pairs {
            let Some(source) = entity_at(importer, span) else {
                continue;
            };
            if whole_members.contains(&span) && !source.symbol_covers_span() {
                continue;
            }
            let Ok(Some(target)) = redirected_export_entity(
                facts,
                &files_by_path,
                &entities_by_location,
                &runtime_edges,
                &target_path,
                &exported,
                &mut HashSet::new(),
            ) else {
                continue;
            };
            let source = canonical_symbol(&source.symbol, &aliases);
            let target = canonical_symbol(&target.symbol, &aliases);
            if source.is_empty() || target.is_empty() || source == target {
                continue;
            }
            if redirects
                .get(&source)
                .is_some_and(|existing| existing != &target)
            {
                redirects.remove(&source);
                ambiguous.insert(source);
            } else if !ambiguous.contains(&source) {
                redirects.insert(source, target);
            }
        }
    }
    Ok(redirects)
}

/// ADR 0322: the static members a namespace import declared at `local`
/// reads, as `(span, exported name)` pairs: the member's property span and
/// its whole span, and a dotted JSX tag's property span. `None` when any
/// runtime reference is anything else: a computed member (`X[k]`), the
/// namespace handed, returned, stored, spread or aliased, `typeof X` in a
/// value. A reference inside a type query is erased and allowed.
pub(crate) fn namespace_static_members(
    file: &solid_facts::FileFacts,
    local: solid_facts::core::Span,
) -> Option<Vec<(solid_facts::core::Span, String)>> {
    let mut pairs = Vec::new();
    let mut referenced = false;
    for (reference, _) in file
        .ast
        .reference_declarations
        .iter()
        .filter(|(reference, declared)| *declared == local && *reference != local)
    {
        referenced = true;
        if file
            .ast
            .type_queries
            .iter()
            .chain(&file.ast.type_names)
            .any(|erased| erased.contains(*reference))
        {
            continue;
        }
        if let Some(member) = file.ast.members.iter().find(|member| {
            member.object == *reference
                && file
                    .ast
                    .computed_members
                    .binary_search(&member.span)
                    .is_err()
        }) {
            let name = file.source_text(member.property)?.to_owned();
            pairs.push((member.property, name.clone()));
            pairs.push((member.span, name));
            continue;
        }
        if let Some(element) = file.ast.jsx_elements.iter().find(|element| {
            element.member_object == Some(*reference)
                || element.closing_name.is_some_and(|closing| {
                    closing.start == reference.start && element.member_object.is_some()
                })
        }) {
            let property = element.member_property?;
            let name = file.source_text(property)?.to_owned();
            pairs.push((property, name.clone()));
            pairs.push((element.name.span, name));
            continue;
        }
        return None;
    }
    referenced.then_some(pairs)
}

/// Resolve the selected runtime graph, including bare re-exports. Explicit
/// exports win; distinct star bindings refuse rather than picking one.
/// `Ok(None)` is an enumerated absence. Missing, cyclic or ambiguous
/// providers return `Err`, which a competing star must never treat as absence.
fn redirected_export_entity<'a>(
    facts: &'a solid_facts::ProjectFacts,
    files: &HashMap<PathBuf, &'a solid_facts::FileFacts>,
    entities: &HashMap<typefacts::Location, &'a typefacts::EntityFact>,
    edges: &HashMap<(PathBuf, String), PathBuf>,
    path: &Path,
    name: &str,
    visiting: &mut HashSet<(PathBuf, String)>,
) -> Result<Option<&'a typefacts::EntityFact>, ()> {
    let path = path.to_path_buf();
    if !visiting.insert((path.clone(), name.to_owned())) {
        return Err(());
    }
    let file = files.get(&path).copied().ok_or(())?;
    let stars = validate_module_export_precedence(&file.ast, &path, name).map_err(|_| ())?;
    let mut candidates = Vec::new();
    for export in file
        .ast
        .module_level_exports()
        .filter(|edge| !edge.type_only)
    {
        if let Some(binding) = export
            .specifiers
            .iter()
            .chain(&export.declarations)
            .find(|binding| !binding.type_only && binding.exported == name)
        {
            if let Some(module) = export.module.as_deref()
                && let Some(target) = edges.get(&(path.clone(), module.to_owned()))
            {
                return redirected_export_entity(
                    facts,
                    files,
                    entities,
                    edges,
                    target,
                    file.source_text(binding.local.span).ok_or(())?,
                    visiting,
                )?
                .map(Some)
                .ok_or(());
            }
            return entry_export_entity_indexed(
                facts,
                files,
                entities,
                &path,
                name,
                &mut HashSet::new(),
            )
            .map(Some)
            .ok_or(());
        }
        if stars && name != "default" && is_bare_runtime_export_star(export) {
            let module = export.module.as_deref().ok_or(())?;
            // An unknown competing provider can also publish this name.
            // Missing resolution is not evidence that it exports nothing.
            let target = edges.get(&(path.clone(), module.to_owned())).ok_or(())?;
            if let Some(entity) = redirected_export_entity(
                facts,
                files,
                entities,
                edges,
                target,
                name,
                &mut visiting.clone(),
            )? {
                candidates.push(entity);
            }
        }
    }
    if let Some(first) = candidates.first() {
        return if candidates
            .iter()
            .all(|entity| entity.location == first.location)
        {
            Ok(Some(*first))
        } else {
            Err(())
        };
    }
    if stars {
        // No ESM export table is not proof that a CommonJS/opaque module
        // contributes no names to the namespace the bundler constructs.
        if file
            .ast
            .module_level_exports()
            .any(|export| !export.type_only)
        {
            Ok(None)
        } else {
            Err(())
        }
    } else {
        entry_export_entity_indexed(facts, files, entities, &path, name, &mut HashSet::new())
            .map(Some)
            .ok_or(())
    }
}

/// Whether this syntax entry contributes every non-default name from its
/// target module. `export * as namespace` is an explicit namespace binding,
/// never an export-star entry.
pub fn is_bare_runtime_export_star(export: &solid_facts::ast::ExportFact) -> bool {
    !export.type_only
        && export.kind == solid_facts::ast::ExportKind::All
        && export.namespace.is_none()
}

fn is_direct_runtime_namespace_declaration(
    ast: &solid_facts::ast::AstFacts,
    declaration: &solid_facts::ast::ExportSpecifierFact,
) -> bool {
    !declaration.type_only && ast.declares_namespace_at(declaration.local.span)
}

pub fn explicit_runtime_export_binding_count(
    export: &solid_facts::ast::ExportFact,
    name: &str,
) -> usize {
    if export.type_only {
        return 0;
    }
    if export.kind == solid_facts::ast::ExportKind::All {
        return usize::from(export.namespace.as_deref() == Some(name));
    }
    let bindings = export
        .specifiers
        .iter()
        .chain(&export.declarations)
        .filter(|specifier| !specifier.type_only && specifier.exported == name)
        .count();
    if bindings == 0 && export.kind == solid_facts::ast::ExportKind::Default && name == "default" {
        1
    } else {
        bindings
    }
}

/// Enforces the per-module explicit-before-star half of ECMA-262
/// ResolveExport and returns whether this module's bare star entries should be
/// consulted for `name`.
///
/// Candidate identity cannot enforce explicit-export uniqueness: two invalid
/// explicit entries may point at the same runtime identity and collapse in the
/// candidate map. Refuse that syntax before choosing either a local program
/// summary or an accepted re-export summary.
pub fn validate_module_export_precedence(
    ast: &solid_facts::ast::AstFacts,
    path: &Path,
    name: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    // The reviewed TypeScript class+namespace merge publishes one runtime
    // binding through two declaration statements. Do not generalize this by
    // name: illegal duplicate const/class declarations also share a spelling
    // (and may share a recovery symbol on a TypeScript-error program).
    // Specifier/default/namespace-export entries remain separate entries.
    let declaration_path = path.to_string_lossy();
    let is_declaration_module = [".d.ts", ".d.mts", ".d.cts"]
        .iter()
        .any(|suffix| declaration_path.ends_with(suffix));
    let mut declaration_entries = Vec::new();
    let mut explicit_count = 0;
    for export in ast.module_level_exports() {
        let count = explicit_runtime_export_binding_count(export, name);
        let declarations = if export.kind == solid_facts::ast::ExportKind::Named
            && export.module.is_none()
            && (!export.type_only || is_declaration_module)
        {
            export
                .declarations
                .iter()
                .filter(|declaration| !declaration.type_only && declaration.exported == name)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        // Oxc correctly marks a bodyless overload signature as type-only in a
        // runtime source file. A declaration module has no body by design, so
        // its direct declaration entries are counted below as the authenticated
        // type surface standing in for the separately materialized runtime.
        let declarations_already_counted = if export.type_only {
            0
        } else {
            declarations.len()
        };
        explicit_count += count - declarations_already_counted;
        declaration_entries.extend(declarations);
    }
    let reviewed_class_namespace_merge = match declaration_entries.as_slice() {
        [first, second] => {
            (ast.declares_class_at(first.local.span)
                && is_direct_runtime_namespace_declaration(ast, second))
                || (ast.declares_class_at(second.local.span)
                    && is_direct_runtime_namespace_declaration(ast, first))
        }
        _ => false,
    };
    let function_declarations = declaration_entries
        .iter()
        .map(|entry| ast.function_declaration_at(entry.local.span))
        .collect::<Option<Vec<_>>>();
    let reviewed_function_overload_set = function_declarations.as_ref().is_some_and(|entries| {
        let implementation_count = entries.iter().filter(|entry| entry.has_body).count();
        !entries.is_empty()
            && ((implementation_count == 1 && !is_declaration_module)
                || (implementation_count == 0 && is_declaration_module))
    });
    explicit_count += if reviewed_class_namespace_merge || reviewed_function_overload_set {
        1
    } else {
        declaration_entries.len()
    };
    if explicit_count > 1 {
        return Err(format!(
            "dependency-source redirects: module {} contains {explicit_count} explicit runtime export entries for {name:?}",
            path.display()
        )
        .into());
    }
    Ok(name != "default" && explicit_count == 0)
}

pub fn entry_export_entity_indexed<'a>(
    facts: &'a solid_facts::ProjectFacts,
    files_by_canonical_path: &HashMap<PathBuf, &'a solid_facts::FileFacts>,
    entities_by_location: &HashMap<typefacts::Location, &'a typefacts::EntityFact>,
    entry_file: &Path,
    name: &str,
    visiting: &mut HashSet<(PathBuf, String)>,
) -> Option<&'a typefacts::EntityFact> {
    let entry_file = entry_file.canonicalize().ok()?;
    if !visiting.insert((entry_file.clone(), name.to_owned())) {
        return None;
    }
    let file = files_by_canonical_path.get(&entry_file).copied()?;
    let consult_export_stars =
        validate_module_export_precedence(&file.ast, &entry_file, name).ok()?;
    for export in file
        .ast
        .module_level_exports()
        .filter(|export| !export.type_only)
    {
        if export.namespace.as_deref() == Some(name) {
            let binding = export.namespace_binding.as_ref()?;
            return entities_by_location
                .get(&typefacts::Location {
                    path: file.path.to_string().into(),
                    start_byte: u64::from(binding.span.start),
                    end_byte: u64::from(binding.span.end),
                })
                .copied();
        }
        if let Some(specifier) = export
            .specifiers
            .iter()
            .chain(&export.declarations)
            .find(|specifier| !specifier.type_only && specifier.exported == name)
        {
            let span = specifier.local.span;
            let location = typefacts::Location {
                path: file.path.to_string().into(),
                start_byte: u64::from(span.start),
                end_byte: u64::from(span.end),
            };
            if let Some(entity) = entities_by_location.get(&location) {
                return Some(*entity);
            }
            if let Some(module) = export.module.as_deref()
                && module.starts_with('.')
            {
                let target = resolve_relative_export(facts, &entry_file, module).ok()?;
                let local_name = file.source_text(specifier.local.span).unwrap_or(name);
                if let Some(entity) = entry_export_entity_indexed(
                    facts,
                    files_by_canonical_path,
                    entities_by_location,
                    &target,
                    local_name,
                    visiting,
                ) {
                    return Some(entity);
                }
            }
        }
        if consult_export_stars
            && is_bare_runtime_export_star(export)
            && let Some(module) = export.module.as_deref()
            && module.starts_with('.')
        {
            let target = resolve_relative_export(facts, &entry_file, module).ok()?;
            if let Some(entity) = entry_export_entity_indexed(
                facts,
                files_by_canonical_path,
                entities_by_location,
                &target,
                name,
                visiting,
            ) {
                return Some(entity);
            }
        }
    }
    None
}

pub fn resolve_relative_export(
    facts: &solid_facts::ProjectFacts,
    source: &Path,
    module: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let project_paths = facts.files.iter().map(|file| file.path.as_str());
    let Some(target) =
        solid_facts::resolve_relative_module_path(&source.to_string_lossy(), module, project_paths)
    else {
        return Err(format!(
            "dependency-source redirects: cannot resolve export-all {module:?} from {}",
            source.display()
        )
        .into());
    };
    Ok(PathBuf::from(target))
}

pub fn canonical_symbol_aliases(facts: &solid_facts::ProjectFacts) -> HashMap<String, String> {
    let mut aliases = facts
        .typescript
        .symbols()
        .filter(|symbol| !symbol.alias_target().is_empty())
        .map(|symbol| (symbol.id().to_owned(), symbol.alias_target().to_owned()))
        .collect::<HashMap<_, _>>();
    for _ in 0..aliases.len() {
        let previous = aliases.clone();
        let mut changed = false;
        for target in aliases.values_mut() {
            if let Some(next) = previous.get(target)
                && next != target
            {
                *target = next.clone();
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    aliases
}

pub fn canonical_symbol(symbol: &str, aliases: &HashMap<String, String>) -> String {
    aliases
        .get(symbol)
        .map_or_else(|| symbol.to_owned(), Clone::clone)
}

pub fn same_canonical_path(left: &Path, right: &Path) -> bool {
    left == right || left.canonicalize().is_ok_and(|left| left == right)
}
