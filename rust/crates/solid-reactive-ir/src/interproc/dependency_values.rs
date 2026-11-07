//! ADR 0822: what a selected package hands the project that no shape states.
//! A Get of a value the walk over the package's source does not close, and a
//! function the package installs on a global, are obligations: the source is
//! there, and it does not say that the read is plain.

use super::*;
use crate::owners::enclosing_render_function;

const GLOBAL_OBJECTS: [&str; 4] = ["globalThis", "window", "self", "global"];

pub(super) fn dependency_value_obligations(
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    provenance: &accessor_provenance::Resolver<'_, '_>,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> Vec<StaticDefect> {
    let mut obligations = Vec::new();
    // Every file, the packages' and the project's: computed fresh on each
    // analysis, never retained per file (ADR 0289's rule for this phase).
    let files = &facts.files;
    let (packages, project): (Vec<_>, Vec<_>) = files
        .iter()
        .partition(|file| facts.dependency_source_root(file.path.as_str()).is_some());
    let (globals, installed) = global_reads(facts, &packages, lookup, provenance);
    for file in project {
        for import in &file.ast.imports {
            if import.type_only {
                continue;
            }
            let Some(package) = globals.iter().find(|package| {
                import.module.as_str() == package.as_str()
                    || import
                        .module
                        .strip_prefix(package.as_str())
                        .is_some_and(|rest| rest.starts_with('/'))
            }) else {
                continue;
            };
            obligations.push(StaticDefect {
                kind: StaticDefectKind::ReactiveDispatchUnresolved {
                    callee: import.module.to_string(),
                    member: None,
                },
                location: location(file.path.shared(), import.span),
                analysis_context: crate::blockers::with_detail(
                    crate::blockers::DEPENDENCY_GLOBAL_READ,
                    &format!(
                        "{package} installs a function on a global, and it may read a source wherever it is called"
                    ),
                ),
                fixes: vec![],
                uncertain: true,
            });
        }
        // ADR 0862: each call of what a package installs on a global is a site
        // the function may read a source at: the row stands there too, not
        // only at the import.
        if !globals.is_empty() {
            let allowed = allowed_callback_spans(file, lookup);
            for call in &file.ast.calls {
                let callee = file.ast.peel_ts_sugar_span(call.callee);
                let Some(name) = installed_callee(file, callee, &installed) else {
                    continue;
                };
                let role =
                    semantic_execution_role(file, call.span, &allowed, entities, symbol_names, lookup);
                if !role.reports_untracked_read() && !role.reports_unplaced() {
                    continue;
                }
                obligations.push(StaticDefect {
                    kind: StaticDefectKind::ReactiveDispatchUnresolved {
                        callee: name.to_owned(),
                        member: None,
                    },
                    location: location(file.path.shared(), call.span),
                    analysis_context: crate::blockers::with_detail(
                        crate::blockers::DEPENDENCY_GLOBAL_READ,
                        &format!(
                            "{} installs {name} on a global, and it may read a source here",
                            globals.join(", ")
                        ),
                    ),
                    fixes: vec![],
                    uncertain: true,
                });
            }
        }
        let mut allowed = None;
        let mut untracked = |span: Span| {
            let allowed = allowed.get_or_insert_with(|| allowed_callback_spans(file, lookup));
            semantic_execution_role(file, span, allowed, entities, symbol_names, lookup)
                .reports_untracked_read()
        };
        let targets = file
            .ast
            .write_targets
            .iter()
            .chain(&file.ast.deleted_targets)
            .chain(&file.ast.iteration_targets)
            .chain(file.ast.assignments.iter().map(|a| &a.target))
            .map(|span| file.ast.peel_ts_sugar_span(*span))
            .collect::<HashSet<_>>();
        let callees = file
            .ast
            .calls
            .iter()
            .map(|call| file.ast.peel_ts_sugar_span(call.callee))
            .collect::<HashSet<_>>();
        let mut push = |span: Span, site: &Location| {
            obligations.push(StaticDefect {
                kind: StaticDefectKind::ReactiveDispatchUnresolved {
                    callee: file.source_text(span).unwrap_or("this Get").to_owned(),
                    member: None,
                },
                location: location(file.path.shared(), span),
                analysis_context: crate::blockers::with_detail(
                    crate::blockers::DEPENDENCY_VALUE_UNFOLLOWED,
                    &format!(
                        "the walk over the package ends at line {} of {}, and a Get of this value may run what it installs",
                        super::result_reads::getter_line(lookup, site),
                        site.path
                            .rsplit_once("node_modules/")
                            .map_or(site.path.as_ref(), |(_, rest)| rest),
                    ),
                ),
                fixes: vec![],
                uncertain: true,
            });
        };
        for member in &file.ast.members {
            if targets.contains(&member.span)
                || callees.contains(&member.span)
                || !enclosing_render_function(file, member.span, lookup)
            {
                continue;
            }
            let name = if file.ast.computed_members.contains(&member.span) {
                file.ast
                    .literal_computed_members
                    .iter()
                    .find(|key| key.span == member.span)
                    .map(|key| key.key.as_str())
            } else {
                file.source_text(member.property)
            };
            let Some(name) = name else {
                continue;
            };
            if !provenance
                .dependency_getters(file, member.object, name)
                .is_empty()
            {
                continue;
            }
            if let Some(site) = provenance.dependency_open_value(file, member.object, name)
                && untracked(member.span)
            {
                push(member.span, &site);
            }
        }
        for binding in &file.ast.bindings {
            let Some(initializer) = binding.initializer.filter(|_| {
                binding.shape == solid_facts::ast::BindingShape::Object
                    && enclosing_render_function(file, binding.pattern, lookup)
            }) else {
                continue;
            };
            for slot in &binding.object_slots {
                let name = slot.property.as_str();
                if !provenance
                    .dependency_getters(file, initializer, name)
                    .is_empty()
                {
                    continue;
                }
                if let Some(site) = provenance.dependency_open_value(file, initializer, name)
                    && untracked(binding.pattern)
                {
                    push(binding.pattern, &site);
                    break;
                }
            }
        }
    }
    obligations
}

/// ADR 0862: the global keys the packages install a function that may read a
/// source on. `any`: a key no text names, so every member of a global.
#[derive(Default)]
struct Installed {
    any: bool,
    keys: HashSet<String>,
}

/// The name of an installed function a call's callee names: `window.k` (or
/// another global object, unshadowed), or the bare global `k` itself.
fn installed_callee<'f>(
    file: &'f solid_facts::FileFacts,
    callee: Span,
    installed: &Installed,
) -> Option<&'f str> {
    let ast = &file.ast;
    let global = |at: Span| {
        file.source_text(at)
            .is_some_and(|text| GLOBAL_OBJECTS.contains(&text))
            && ast.reference_declaration(at).is_none()
    };
    if let Some(member) = ast.member_at(callee) {
        if !global(ast.peel_ts_sugar_span(member.object)) {
            return None;
        }
        let key = match ast
            .literal_computed_members
            .iter()
            .find(|literal| literal.span == member.span)
        {
            Some(literal) => Some(literal.key.as_str()),
            None if ast.computed_members.contains(&member.span) => None,
            None => file.source_text(member.property),
        };
        return match key {
            Some(key) if installed.any || installed.keys.contains(key) => Some(key),
            None if installed.any || !installed.keys.is_empty() => {
                file.source_text(member.span)
            }
            _ => None,
        };
    }
    let name = file.source_text(callee)?;
    (installed.keys.contains(name)
        && ast.reference_declaration(callee).is_none()
        && ast.identifiers_within(callee).any(|identifier| {
            identifier.span == callee
                && identifier.role == solid_facts::ast::IdentifierRole::Reference
        }))
    .then_some(name)
}

/// The package names of the selected roots that assign a member of a global
/// object a function that may read a source, and the keys they assign.
fn global_reads(
    facts: &ProjectFacts,
    files: &[&solid_facts::FileFacts],
    lookup: &SemanticLookup<'_>,
    provenance: &accessor_provenance::Resolver<'_, '_>,
) -> (Vec<String>, Installed) {
    let mut packages = Vec::new();
    let mut installed = Installed::default();
    for file in files {
        let Some(root) = facts.dependency_source_root(file.path.as_str()) else {
            continue;
        };
        let Some((_, package)) = root.rsplit_once("node_modules/") else {
            continue;
        };
        let package = package.trim_end_matches('/').to_owned();
        let assigns = file.ast.assignments.iter().filter(|assignment| {
            let Some(member) = file
                .ast
                .member_at(file.ast.peel_ts_sugar_span(assignment.target))
            else {
                return false;
            };
            let object = file.ast.peel_ts_sugar_span(member.object);
            if !file
                .source_text(object)
                .is_some_and(|text| GLOBAL_OBJECTS.contains(&text))
                || file.ast.reference_declaration(object).is_some()
            {
                return false;
            }
            let value = file.ast.peel_ts_sugar_span(assignment.value_span);
            if let Some(function) = file.ast.function_at(value) {
                return provenance.may_read_source(file, function);
            }
            if let Some((target, span)) =
                crate::owners::exact_function_argument(file, value, lookup)
            {
                return provenance.package_file(&target).is_none_or(|target| {
                    target
                        .ast
                        .function_at(span)
                        .is_none_or(|function| provenance.may_read_source(target, function))
                });
            }
            // A literal holds the functions written in it; any other value is
            // what no shape states.
            if file
                .ast
                .object_get_shapes
                .iter()
                .any(|shape| shape.span == value)
            {
                return file
                    .ast
                    .functions_within(value)
                    .any(|function| provenance.may_read_source(file, function));
            }
            !file
                .source_text(value)
                .is_none_or(accessor_provenance::plain_value_text)
        });
        let mut any = false;
        for assignment in assigns {
            any = true;
            let Some(member) = file
                .ast
                .member_at(file.ast.peel_ts_sugar_span(assignment.target))
            else {
                continue;
            };
            let key = match file
                .ast
                .literal_computed_members
                .iter()
                .find(|literal| literal.span == member.span)
            {
                Some(literal) => Some(literal.key.as_str()),
                None if file.ast.computed_members.contains(&member.span) => None,
                None => file.source_text(member.property),
            };
            match key {
                Some(key) => {
                    installed.keys.insert(key.to_owned());
                }
                None => installed.any = true,
            }
        }
        if any && !packages.contains(&package) {
            packages.push(package);
        }
    }
    (packages, installed)
}
