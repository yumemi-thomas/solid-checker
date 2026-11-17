//! ADR 0981: a value a package the build does not read returned, read where nothing tracks it, may be a reactive read.
//!
//! A package whose source the build does not select is trusted to be non-reactive only as far as its source says so, and nothing reads it:
//! `const g = wrap(() => count()); g()` calls an accessor the project handed the package and got back, `const { search } = useLocation()` reads
//! a getter the package defined over a memo. Chrome warns `STRICT_READ_UNTRACKED` at each in a component body, and the analysis read the
//! returned value as plain, so the read had no row. Owner ruling: a value that is the result of a call into such a package (a call, `new`,
//! a member of one, a project binding that only holds one: [`value_of`]), read where nothing tracks it, is an uncertifiable SC1001, never a
//! violation: nothing proves the value is reactive, and nothing proves it is not.
//!
//! A read is a member get (the first of a chain), a call of the value, a pattern that destructures it, an iteration over it (a `for…of`, an
//! array or argument spread, an array pattern) and an object spread. Its role is the one a reactive read has ([`read_execution_role`]):
//! the row stands where the role is untracked or where nothing places the read, and nowhere a tracked expression, a handler that Solid
//! does not warn in, or a deleted region covers. The package is the one the call's callee resolves to ([`origin_of`]): the dialect's
//! runtime, a package read as source, the host's own objects and globals are not it.
//!
//! What would win the rows back, recorded in `docs/open-work.md`: proving from the package's readable source that a returned value cannot
//! come from a function or an object the program hands it, and defines no accessor.

use super::{
    DeclarationAt, ImportBinding, ORIGIN_DEPTH, Origin, import_bindings, installed_name, is_pseudo,
    origin_of, value_initializer,
};
use crate::pipeline::AnalysisContext;
use crate::{HashMap, HashSet};
use solid_facts::FileFacts;
use solid_facts::core::Span;
use solid_facts::package_reach::package_root_of;

/// How many hops a value is followed from the read to the call that made it (members, aliases, operands).
const VALUE_DEPTH: usize = 12;

/// The package whose call made the value at `span`, when it is a value a package returned: the result of a call or `new` into a package the
/// build does not read, a member of one, an operand of a conditional, logical or comma expression that is one, or a project binding of one plain
/// name whose initializer is one. The function itself (`pkgFn`, `ns.pkgFn`) is no value of a call: calling it is SC2002's site.
fn value_of(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
    bound: &HashMap<Span, ImportBinding<'_>>,
    span: Span,
    depth: usize,
) -> Option<DeclarationAt> {
    if depth == 0 {
        return None;
    }
    let ast = &file.ast;
    let span = ast.peel_ts_sugar_span(span);
    if let Some(call) = ast.call_at(span) {
        // ADR 1063: a call the reviewed reading shows throws on every call returns no value: nothing read off it runs.
        if crate::reviewed_api::call_always_throws(ctx, file, call) {
            return None;
        }
        return match origin_of(ctx, analyzed, file, bound, call.span, ORIGIN_DEPTH) {
            Origin::Package(declaration) => Some(declaration),
            _ => None,
        };
    }
    if let Some(member) = ast.member_at(span) {
        return value_of(ctx, analyzed, file, bound, member.object, depth - 1);
    }
    if let Some(conditional) = ast.conditional_at(span) {
        return value_of(
            ctx,
            analyzed,
            file,
            bound,
            conditional.consequent,
            depth - 1,
        )
        .or_else(|| value_of(ctx, analyzed, file, bound, conditional.alternate, depth - 1));
    }
    if let Some(logical) = ast.logical_at(span) {
        return value_of(ctx, analyzed, file, bound, logical.left, depth - 1)
            .or_else(|| value_of(ctx, analyzed, file, bound, logical.right, depth - 1));
    }
    if let Some(sequence) = ast.sequences.iter().find(|sequence| sequence.span == span) {
        return value_of(ctx, analyzed, file, bound, sequence.tail, depth - 1);
    }
    let declaration = ast.reference_declaration(span)?;
    if bound.contains_key(&declaration) {
        return None;
    }
    let (next, initializer) = value_initializer(ctx.semantic_lookup, file, span)?;
    if std::ptr::eq(next, file) {
        value_of(ctx, analyzed, file, bound, initializer, depth - 1)
    } else {
        let next_bound = import_bindings(ctx, next);
        value_of(ctx, analyzed, next, &next_bound, initializer, depth - 1)
    }
}

/// The uncertifiable SC1001 rows of every analyzed project file, found once per build.
pub(crate) fn rows(ctx: &AnalysisContext<'_>, rows: &mut Vec<crate::StaticViolation>) {
    let lookup = ctx.semantic_lookup;
    let analyzed = lookup
        .files()
        .iter()
        .map(|file| file.path.as_str())
        .collect::<HashSet<_>>();
    for file in lookup.files() {
        // A package read as source is analyzed with the project: its values are proven, not assumed.
        if lookup.in_dependency_source(file.path.as_str()) {
            continue;
        }
        let ast = &file.ast;
        let bound = import_bindings(ctx, file);
        // The reads of a file, each a (site, value) pair, a site once.
        let mut reads = Vec::<(Span, Span)>::new();
        // `value.key`, `value[key]`: the first member of a chain is the read; the members after it read what that returned.
        for member in &ast.members {
            let object = ast.peel_ts_sugar_span(member.object);
            if ast.member_at(object).is_none() {
                reads.push((member.span, object));
            }
        }
        // `value()`: the callee is the value of a call, an alias of one, or a call's own result (`wrap(f)()`).
        for call in &ast.calls {
            let callee = ast.peel_ts_sugar_span(call.callee);
            if ast.member_at(callee).is_none() {
                reads.push((call.span, callee));
            }
        }
        // `const { a } = value`, `[a] = value`: each key is read off the value at the pattern.
        for &(pattern, value) in &ast.pattern_values {
            reads.push((pattern, value));
        }
        // `for (const x of value)`, `[...value]`, `f(...value)`: the iterator is read; `{ ...value }` enumerates (and reads) every key.
        for &operand in &ast.iterated_operands {
            reads.push((operand, operand));
        }
        for spread in &ast.spreads {
            reads.push((spread.span, spread.argument));
        }
        let mut seen = HashSet::<Span>::default();
        let mut allowed = None;
        for (at, value) in reads {
            if !seen.insert(at) {
                continue;
            }
            let Some(declaration) = value_of(ctx, &analyzed, file, &bound, value, VALUE_DEPTH)
            else {
                continue;
            };
            // A package the build reads as source returns values its analysis proves (a violation or an obligation of its own rule), so no row stands for them here.
            if is_pseudo(&declaration.path) || lookup.in_dependency_source(&declaration.path) {
                continue;
            }
            let allowed = allowed
                .get_or_insert_with(|| crate::execution_role::allowed_callback_spans(file, lookup));
            let role = crate::execution_role::read_execution_role(
                file,
                at,
                allowed,
                ctx.entities,
                ctx.symbol_names,
                lookup,
            );
            if !(role.reports_untracked_read() || role.reports_unplaced()) {
                continue;
            }
            let package = package_root_of(&declaration.path).map_or_else(
                || declaration.path.clone(),
                |root| installed_name(ctx, root),
            );
            rows.push(row(file, at, &package, role.reports_untracked_read()));
        }
    }
    flowed_rows(ctx, &analyzed, rows);
}

/// ADR 0998: the reads the backward walk above does not reach. [`value_of`] follows a read back through a call, a member, an operand and a plain
/// binding; a value that goes through an element of a literal, an assignment to an existing binding, a parameter of a project function or a copy
/// reaches its read by a route it does not follow, and Chrome warns there all the same. The value-use census walks every such value FORWARD as
/// a value past an escape ([`crate::value_use::Census::package_value_start`]): each call or Get through it in project code that no role clears,
/// and each position that walk cannot follow, is an uncertifiable SC1001 here unless a row of the walk above already stands on the same span.
fn flowed_rows(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    rows: &mut Vec<crate::StaticViolation>,
) {
    let lookup = ctx.semantic_lookup;
    let files = lookup
        .files()
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect::<HashMap<_, _>>();
    // The spans this pass already filed, by path: a site stands once.
    let mut filed = HashMap::<String, HashSet<(u64, u64)>>::default();
    for row in rows.iter().filter(|row| {
        row.analysis_context
            .starts_with(crate::blockers::PACKAGE_VALUE_READ)
    }) {
        filed
            .entry(row.location.path.to_string())
            .or_default()
            .insert((row.location.start_byte, row.location.end_byte));
    }
    let no_calls = HashMap::default();
    let no_rows = crate::value_use::RowIndex::default();
    let mut census = crate::value_use::Census::new(
        lookup,
        ctx.entities,
        ctx.symbol_names,
        HashSet::default(),
        &no_calls,
        crate::attribution::ExportedCallers::new(lookup, ctx.aliases, ctx.symbols_by_root),
        &no_rows,
    );
    for file in lookup.files() {
        if lookup.in_dependency_source(file.path.as_str()) {
            continue;
        }
        let bound = import_bindings(ctx, file);
        for call in &file.ast.calls {
            // ADR 1063: no value leaves a call that throws on every call.
            if crate::reviewed_api::call_always_throws(ctx, file, call) {
                continue;
            }
            let Origin::Package(declaration) =
                origin_of(ctx, analyzed, file, &bound, call.span, ORIGIN_DEPTH)
            else {
                continue;
            };
            if is_pseudo(&declaration.path) || lookup.in_dependency_source(&declaration.path) {
                continue;
            }
            let package = package_root_of(&declaration.path).map_or_else(
                || declaration.path.clone(),
                |root| installed_name(ctx, root),
            );
            let answer =
                census.package_value_start(file, call.span, crate::blockers::PACKAGE_VALUE_READ);
            let budget = answer.cut.then(|| {
                (
                    call.span.location(file.path.as_str()),
                    crate::blockers::VALUE_USE_UNCLASSIFIED,
                )
            });
            for (location, blocker) in answer
                .uses
                .into_iter()
                .chain(answer.escaped_uses)
                .chain(budget)
            {
                // A value handed to code that is no function of the project (a package, the standard library, a constructor, a component, a
                // primitive, an `await`) is that code's to read: no claim of this rule (`docs/open-work.md`). What stays is a call or Get through
                // the value in project code, and a position inside the project the walk cannot follow.
                if matches!(
                    blocker,
                    crate::blockers::VALUE_TO_UNRESOLVED_CALLEE
                        | crate::blockers::VALUE_THROUGH_NEW
                        | crate::blockers::VALUE_IN_CARRIER
                        | crate::blockers::VALUE_THROUGH_AWAIT
                ) {
                    continue;
                }
                let Some(target) = files.get(location.path.as_ref()) else {
                    continue;
                };
                if lookup.in_dependency_source(target.path.as_str()) {
                    continue;
                }
                let (Ok(start), Ok(end)) = (
                    u32::try_from(location.start_byte),
                    u32::try_from(location.end_byte),
                ) else {
                    continue;
                };
                if !filed
                    .entry(location.path.to_string())
                    .or_default()
                    .insert((location.start_byte, location.end_byte))
                {
                    continue;
                }
                rows.push(flowed_row(target, Span::new(start, end), &package, blocker));
            }
        }
    }
}

/// The row of a site or an end [`flowed_rows`] found: `flowed` for a call or Get through the value, else the position the walk could not follow.
fn flowed_row(
    file: &FileFacts,
    at: Span,
    package: &str,
    blocker: &'static str,
) -> crate::StaticViolation {
    let text = file.source_text(at).unwrap_or_default();
    let first = text.lines().next().unwrap_or_default().trim();
    let shown = if first.chars().count() > 48 {
        format!("{}…", first.chars().take(47).collect::<String>())
    } else {
        first.to_owned()
    };
    let site = blocker == crate::blockers::PACKAGE_VALUE_READ;
    crate::StaticViolation {
        id: "SC1001".into(),
        rule: "strict-read-untracked".into(),
        message: if site {
            format!(
                "`{shown}` reads, or calls, a value that `{package}` returned and that reached this place through the program (a binding, a literal, an assignment or a parameter); `{package}` is a package this build does not read as source, so the value may be a getter or an accessor it defines, or a function or object this program handed it, and whether Solid warns STRICT_READ_UNTRACKED here is not proven"
            )
        } else {
            format!(
                "`{shown}` holds a value that `{package}` returned, where this analysis does not follow it further; `{package}` is a package this build does not read as source, so the value may be a getter or an accessor it defines, and whether a read of it past this place warns STRICT_READ_UNTRACKED is not proven"
            )
        },
        hint: "Read the value inside a tracked expression (JSX, an effect's compute function or a memo), or let the build read the package as source (a declared host with the Solid Vite \
               plugin reads the packages that name Solid, or whose code imports it)."
            .into(),
        location: crate::location(file.path.shared(), at),
        analysis_context: crate::blockers::with_detail(
            crate::blockers::PACKAGE_VALUE_READ,
            if site { "flowed" } else { blocker },
        ),
        fixes: Vec::new(),
        uncertain: true,
    }
}

fn row(file: &FileFacts, at: Span, package: &str, untracked: bool) -> crate::StaticViolation {
    let text = file.source_text(at).unwrap_or_default();
    let first = text.lines().next().unwrap_or_default().trim();
    let shown = if first.chars().count() > 48 {
        format!("{}…", first.chars().take(47).collect::<String>())
    } else {
        first.to_owned()
    };
    crate::StaticViolation {
        id: "SC1001".into(),
        rule: "strict-read-untracked".into(),
        message: format!(
            "`{shown}` reads a value returned by `{package}`, a package this build does not read as source, {}; the value may be a getter or an accessor the package defines, or a \
             function or object this program handed it, so whether Solid warns STRICT_READ_UNTRACKED here is not proven",
            if untracked {
                "where nothing tracks the read"
            } else {
                "where nothing places the read"
            },
        ),
        hint: "Read the value inside a tracked expression (JSX, an effect's compute function or a memo), or let the build read the package as source (a declared host with the Solid Vite \
               plugin reads the packages that name Solid, or whose code imports it)."
            .into(),
        location: crate::location(file.path.shared(), at),
        analysis_context: crate::blockers::with_detail(
            crate::blockers::PACKAGE_VALUE_READ,
            if untracked { "untracked" } else { "unplaced" },
        ),
        fixes: Vec::new(),
        uncertain: true,
    }
}
