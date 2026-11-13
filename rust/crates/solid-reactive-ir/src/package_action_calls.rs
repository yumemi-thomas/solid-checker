//! ADR 0972: a call into code of a package the build does not read, written in an owned scope, may throw `ACTION_CALLED_IN_OWNED_SCOPE`.
//!
//! SC2002 placed only the actions the program itself creates. An installed package whose source the build does not read can call Solid's
//! `action` itself (`export const runStep = (cb) => action(function* () { cb(); })()`), and the program shows no action at the project's
//! call: Chrome throws at `runStep(() => {})` in a component body, a memo's compute or a `createRoot` body while the checker was silent.
//!
//! Owner ruling (option B): each such call is a site of SC2002 ([`sites`]). Its callee is decided by the compiler's resolution (a call's
//! resolved declaration, a tag's symbol through its alias): a declaration in an installed package or in a declaration file, outside the
//! standard library, outside the packages the build reads as source (their code is analyzed, ADR 0949 reports it at the project's site),
//! and outside the dialect's own runtime (its catalog answers for its exports: Solid's runtime calls no `action` of its own, checked on the
//! pinned release's source). The site ends silent only when
//!
//! - the backend decided from the package's source that its closure cannot reach a Solid package
//!   ([`solid_facts::package_reach::PackageReach::Clear`]: no Solid load, no computed load, no code generation, no unknown global); or
//! - the call's role allows an action (an event handler, a timer, an effect's apply, `onSettled`): the classifier SC2002's own rows use; or
//! - the call cannot run (module initialization, a function nothing reaches).
//!
//! Otherwise it gets an uncertifiable SC2002 row, blocker `package-may-call-action:<why>`, never a violation: nothing proves the package
//! does call an action. A component the program renders from such a package (`<Pkg />`) runs its body under an owner on every render, so
//! its tag is a site with no legal role. A function the program hands the package is the hand-off's concern (ADRs 0949, 0967), not this.
//!
//! A project binding that only names a package's function (`const run = pkg.runStep`, `const C = Pkg.Comp`) is followed through its single
//! initializer ([`ALIAS_DEPTH`] deep) to the declaration the compiler resolves it to, so an alias is the site its target is.
//!
//! The sites are found once per build ([`sites`]): the pipeline's row pass and the proof universe both read them.

use crate::{HashMap, HashSet};
use solid_facts::FileFacts;
use solid_facts::core::Span;
use solid_facts::package_reach::{
    PackageReach, ReachBlocker, is_declaration_file, package_label, package_root_of,
};

use crate::execution_role::RootBodyGuard;
use crate::pipeline::AnalysisContext;
use crate::proof_log::{Premise, PremiseKind};
use crate::site_accounting::{SiteEnds, SiteSilence};

/// How far a chain of aliases (`const a = pkg.fn; const b = a`) is followed to the package declaration.
const ALIAS_DEPTH: usize = 4;

/// How the project enters the package's code at a site.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Entry {
    /// A call or `new`.
    Call,
    /// A tagged template's tag.
    Tag,
    /// A JSX element whose tag is the package's component.
    Render,
    /// A reference to the package's function handed on as a value (`createMemo(pkgFn)`, `[1].forEach(pkgFn)`): where it runs is not
    /// followed, so it has no legal role.
    Value,
}

impl Entry {
    pub(crate) const fn via(self) -> &'static str {
        match self {
            Self::Call => "call",
            Self::Tag => "tag",
            Self::Render => "render",
            Self::Value => "value",
        }
    }
}

/// One project site that enters code of a package the build does not read, whose closure is not clear.
pub(crate) struct PackageSite {
    /// The call, tagged template or opening tag.
    pub(crate) at: Span,
    /// The callee, tag or element name.
    pub(crate) callee: Span,
    pub(crate) entry: Entry,
    pub(crate) reach: PackageReach,
    /// The package (its directory name), or the declaration file.
    pub(crate) package: String,
}

/// How a site's callee is resolved to the code it enters.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Resolve {
    /// A call, `new` or tagged template: the compiler's resolved declaration of the call, else the callee's symbol.
    Call,
    /// A JSX tag: the tag's symbol, else the import it names, through the project bindings that only alias it.
    Tag,
}

/// What an import binding of a file names.
struct ImportBinding<'f> {
    module: &'f str,
    /// The span of the import declaration (the compiler's attestation of the import is keyed by it).
    declaration: Span,
    namespace: bool,
    /// The module is the project's (a relative path) or the dialect's runtime: the project's analysis and the dialect's catalog decide it.
    decided_elsewhere: bool,
}

/// The value import bindings of `file` by the span of the local name, for an O(1) answer to "does this reference name an import".
fn import_bindings<'f>(
    ctx: &AnalysisContext<'_>,
    file: &'f FileFacts,
) -> HashMap<Span, ImportBinding<'f>> {
    let mut bindings = HashMap::default();
    for import in &file.ast.imports {
        if import.type_only {
            continue;
        }
        let module = import.module.as_str();
        let decided_elsewhere = module.starts_with('.')
            || module.starts_with('/')
            || is_runtime_module(ctx, module)
            || module.starts_with("node:");
        for binding in &import.bindings {
            if binding.type_only {
                continue;
            }
            bindings.insert(
                binding.local.span,
                ImportBinding {
                    module,
                    declaration: import.span,
                    namespace: binding.kind == solid_facts::ast::ImportKind::Namespace,
                    decided_elsewhere,
                },
            );
        }
    }
    bindings
}

/// Whether `module` is one of the dialect's runtime modules: its catalog answers for what the module exports.
fn is_runtime_module(ctx: &AnalysisContext<'_>, module: &str) -> bool {
    ctx.dialect
        .primitive_defining_packages()
        .contains(&package_name(module))
        || ctx.dialect.modules().contains(&module)
}

/// The sites of every analyzed file, by position in [`crate::indexes::SemanticLookup::files`], found once per build.
pub(crate) fn sites<'c>(ctx: &'c AnalysisContext<'_>) -> &'c [Vec<PackageSite>] {
    ctx.package_sites.get_or_init(|| {
        let files = ctx.semantic_lookup.files();
        // The files the program analyzes itself (a package read as source, a JavaScript file `allowJs` brings in): their code is read, not assumed.
        let analyzed = files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<HashSet<_>>();
        files
            .iter()
            .map(|file| file_sites(ctx, &analyzed, file))
            .collect()
    })
}

/// The sites of one file, in source order.
fn file_sites(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
) -> Vec<PackageSite> {
    let lookup = ctx.semantic_lookup;
    let bound = import_bindings(ctx, file);
    let mut sites = Vec::new();
    let mut seen = HashSet::<(Span, Entry)>::default();
    let mut push = |at: Span, callee: Span, entry: Entry, declaration: Option<DeclarationAt>| {
        let Some(declaration) = declaration else {
            return;
        };
        // A site two routes find (an identifier and the member it is the object of) stands once.
        if !seen.insert((at, entry)) {
            return;
        }
        let reach = match ctx.package_reach {
            Some(index) => index.for_declaration(&declaration.path),
            None => PackageReach::Undecided {
                blocker: ReachBlocker::NoFilesystem,
                detail: "this adapter reads no installed package".into(),
            },
        };
        if reach.is_clear() {
            return;
        }
        let package = package_root_of(&declaration.path)
            .map(|root| installed_name(ctx, root))
            .unwrap_or_else(|| declaration.path.clone());
        sites.push(PackageSite {
            at,
            callee,
            entry,
            reach,
            package,
        });
    };
    for call in &file.ast.calls {
        // A dialect primitive is the catalog's; a reviewed test runner's registration (`describe`, `it`) runs the callback on its own stack and calls no action.
        if lookup.primitive_at_call(file, call.span).is_some()
            || lookup.reviewed_test_runner_registration(file, call)
        {
            continue;
        }
        push(
            call.span,
            call.callee,
            Entry::Call,
            package_declaration(ctx, analyzed, file, &bound, call.callee, Resolve::Call),
        );
    }
    for template in &file.ast.tagged_templates {
        push(
            template.span,
            template.tag,
            Entry::Tag,
            package_declaration(ctx, analyzed, file, &bound, template.tag, Resolve::Call),
        );
    }
    for element in &file.ast.jsx_elements {
        // An intrinsic element (`<button>`) is the DOM's; its tag names no code.
        if crate::object_members::intrinsic_element(file, element) {
            continue;
        }
        let opening = if element.opening.end > element.opening.start {
            element.opening
        } else {
            element.name.span
        };
        // `<Ns.Comp>`: a member of an import is decided by the import's package; any other member tag (`<props.Comp>`) names no declaration this
        // pass reads. `<Comp>`: the symbol, else the import, through the bindings that only alias it.
        let declaration = match element.member_object {
            Some(object) => file
                .ast
                .reference_declaration(file.ast.peel_ts_sugar_span(object))
                .and_then(|declaration| bound.get(&declaration))
                .and_then(|import| import_declaration(ctx, analyzed, file, import)),
            None => {
                package_declaration(ctx, analyzed, file, &bound, element.name.span, Resolve::Tag)
            }
        };
        push(opening, element.name.span, Entry::Render, declaration);
    }
    // A reference to an import binding handed on as a value: `createMemo(pkgFn)`, `const run = pkgFn`, `[1].map(Pkg.fn)`, `{ pkgFn }`, `take(Pkg)`.
    // A binding that only names it (`const run = pkgFn`) is no new site: its initializer is one, and everything the binding reaches began there.
    for identifier in &file.ast.identifiers {
        if identifier.role != solid_facts::ast::IdentifierRole::Reference {
            continue;
        }
        let Some(import) = file
            .ast
            .reference_declaration(identifier.span)
            .and_then(|declaration| bound.get(&declaration))
            .filter(|import| !import.decided_elsewhere)
        else {
            continue;
        };
        if (import.namespace || may_be_function(ctx, file, identifier.span, identifier.span))
            && handed_as_value(ctx, file, identifier.span)
        {
            push(
                identifier.span,
                identifier.span,
                Entry::Value,
                import_declaration(ctx, analyzed, file, import),
            );
        }
    }
    for member in &file.ast.members {
        let object = file.ast.peel_ts_sugar_span(member.object);
        let Some(import) = file
            .ast
            .reference_declaration(object)
            .and_then(|declaration| bound.get(&declaration))
            .filter(|import| import.namespace && !import.decided_elsewhere)
        else {
            continue;
        };
        if may_be_function(ctx, file, member.span, member.span)
            && handed_as_value(ctx, file, member.span)
        {
            push(
                member.span,
                member.span,
                Entry::Value,
                import_declaration(ctx, analyzed, file, import),
            );
        }
    }
    for property in &file.ast.object_properties {
        // The facts answer the property's symbol at a shorthand's span, never the binding's: the binding is read through its import.
        let Some(binding) = property.shorthand_binding else {
            continue;
        };
        let Some(import) = bound
            .get(&binding)
            .filter(|import| !import.decided_elsewhere)
        else {
            continue;
        };
        if import.namespace || may_be_function(ctx, file, property.value, binding) {
            push(
                property.value,
                property.value,
                Entry::Value,
                import_declaration(ctx, analyzed, file, import),
            );
        }
    }
    sites.sort_by_key(|site| (site.at.start, site.at.end));
    sites
}

/// Whether the value of the reference at `span` may be a function: the compiler types it callable or constructable, or says nothing (the demand
/// plan asks for callability at a few spans only) and the declaration it names is no type-level one (an interface, a type alias, an enum or a
/// namespace holds no function value of its own). Known to be neither callable nor constructable, it is not.
fn may_be_function(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    span: Span,
    symbol_at: Span,
) -> bool {
    use typefacts::{Callability, Constructability};
    let lookup = ctx.semantic_lookup;
    if let Some(entity) = lookup.entity_at(file.path.as_str(), span) {
        if matches!(
            entity.constructability,
            Some(Constructability::Constructable)
        ) {
            return true;
        }
        if let Some(callability) = entity.callability {
            return callability != Callability::NonCallable;
        }
    }
    // No callability fact: the kind of the declaration the symbol names, through its import alias. The producer's kinds are `variable`, `function`,
    // `class` and `declaration` (everything else): only the last is no function value. `symbol_at` is where the import's own symbol is asked (a
    // shorthand property's span holds the property's).
    lookup
        .alias_root_declarations(file, symbol_at)
        .is_none_or(|declarations| {
            declarations.is_empty()
                || declarations
                    .iter()
                    .any(|declaration| declaration.kind.as_ref() != "declaration")
        })
}

/// Whether the reference at `span` hands its value on: not a callee, a tag, the object of a member (`pkg.run`, decided as that member's
/// call; `.call`, `.apply` and `.bind` hand it), an import, export or type position, an element's tag, the value of an intrinsic element's
/// event attribute (the browser's listener) or a direct argument of a standard-library scheduler that runs it on a fresh stack.
fn handed_as_value(ctx: &AnalysisContext<'_>, file: &FileFacts, span: Span) -> bool {
    let ast = &file.ast;
    let lookup = ctx.semantic_lookup;
    if ast.calls_with_peeled_callee(span).next().is_some()
        || ast
            .tagged_templates
            .iter()
            .any(|template| ast.peel_ts_sugar_span(template.tag) == span)
    {
        return false;
    }
    if let Some(member) = ast.members_with_peeled_object(span).next() {
        let property = file.source_text(member.property).unwrap_or_default();
        return matches!(property, "call" | "apply" | "bind");
    }
    if ast.imports.iter().any(|import| import.span.contains(span))
        // An export specifier (`export { runStep }`) or a re-export names the binding; the span of `export function f() { … }` holds the whole body.
        || ast.exports.iter().any(|export| {
            (export.module.is_some() && export.span.contains(span))
                || export
                    .specifiers
                    .iter()
                    .any(|specifier| specifier.local.span.contains(span))
        })
        || ast.type_names.iter().any(|name| name.contains(span))
        || ast.type_queries.iter().any(|query| query.contains(span))
        || ast
            .type_imports
            .iter()
            .any(|import| import.span.contains(span))
    {
        return false;
    }
    for element in &ast.jsx_elements {
        if element.name.span.contains(span)
            || element
                .closing_name
                .is_some_and(|closing| closing.contains(span))
        {
            return false;
        }
        if crate::object_members::intrinsic_element(file, element)
            && element.attributes.iter().any(|attribute| {
                attribute
                    .expression
                    .is_some_and(|value| ast.peel_ts_sugar_span(value) == span)
                    && file
                        .source_text(attribute.name)
                        .is_some_and(|name| name.starts_with("on"))
            })
        {
            return false;
        }
    }
    for (call, index) in ast.arguments_containing(span) {
        if ast.peel_ts_sugar_span(call.arguments[index].span) != span {
            continue;
        }
        let Some(resolved) = lookup.resolved_callee_call(file, call.callee) else {
            continue;
        };
        let callability = lookup
            .entity_at(file.path.as_str(), span)
            .and_then(|entity| entity.callability);
        if matches!(
            crate::runtime_semantics::argument_behavior(resolved, callability, index),
            Some(crate::runtime_semantics::RuntimeArgumentBehavior::FreshStackCallback)
        ) {
            return false;
        }
    }
    true
}

struct DeclarationAt {
    path: String,
}

/// The prefix of the path of an import the compiler's attestation does not cover: no file, so the reach index has no verdict for it.
const UNATTESTED: &str = "unattested:";

/// The declaration of the code `callee` enters, when it is a package's the build does not read: an installed file or a declaration file,
/// outside the standard library, the analyzed packages and the dialect's runtime. `None` for project code, the standard library, a callee
/// nothing resolves (the handed-callee obligations' concern) and Solid's own runtime.
fn package_declaration(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
    bound: &HashMap<Span, ImportBinding<'_>>,
    callee: Span,
    resolve: Resolve,
) -> Option<DeclarationAt> {
    let lookup = ctx.semantic_lookup;
    let runtime = ctx.dialect.primitive_defining_packages();
    // `action(step)()`, `createMemo(f)()`: the inner call is the dialect's, whose catalog answers for what it returns.
    if let Some(inner) = file.ast.call_at(file.ast.peel_ts_sugar_span(callee))
        && lookup.primitive_at_call(file, inner.span).is_some()
    {
        return None;
    }
    let mut paths = Vec::<String>::new();
    if resolve == Resolve::Call
        && let Some(declaration) = lookup
            .resolved_callee_call(file, callee)
            .and_then(|resolved| resolved.declaration.as_ref())
    {
        if declaration.standard_library {
            return None;
        }
        if !declaration.origin_module.is_empty()
            && is_runtime_module(ctx, &declaration.origin_module)
        {
            return None;
        }
        paths.push(declaration.location.path.to_string());
    }
    // The value's own declaration: the symbol the compiler gives it, and the file of the import its root names. A tag follows the project bindings
    // that only name another value (`const View = Pkg.Comp`) to that value's.
    let mut unattested = None;
    let mut hop = (file, file.ast.peel_ts_sugar_span(callee));
    for _ in 0..=if resolve == Resolve::Tag {
        ALIAS_DEPTH
    } else {
        0
    } {
        let (hop_file, span) = hop;
        let hop_imports;
        let hop_table = if std::ptr::eq(hop_file, file) {
            bound
        } else {
            hop_imports = import_bindings(ctx, hop_file);
            &hop_imports
        };
        let import = root_import(hop_file, hop_table, span);
        if import.is_some_and(|import| import.module.starts_with("node:")) {
            return None;
        }
        if import.is_some_and(|import| is_runtime_module(ctx, import.module)) {
            return None;
        }
        if let Some(declarations) = lookup.alias_root_declarations(hop_file, span) {
            paths.extend(
                declarations
                    .iter()
                    .map(|declaration| declaration.location.path.to_string()),
            );
        }
        if let Some(import) = import.filter(|import| !import.decided_elsewhere)
            && let Some(declaration) = import_declaration(ctx, analyzed, hop_file, import)
        {
            if declaration.path.starts_with(UNATTESTED) {
                unattested.get_or_insert(declaration.path);
            } else {
                paths.push(declaration.path);
            }
        }
        if resolve != Resolve::Tag {
            break;
        }
        let Some((next_file, initializer)) = lookup.exact_initializer_at_reference(hop_file, span)
        else {
            break;
        };
        hop = (next_file, next_file.ast.peel_ts_sugar_span(initializer));
    }
    // Installed code only: a declaration under `node_modules` (a linked workspace package is found through its import's `node_modules` spelling),
    // not a file the program analyzes itself (a package read as source, or a JavaScript file `allowJs` brings into the program). A declaration in
    // a project declaration file stands for code no installed package holds (a global a script provides, a `declare module` shim: the import's file
    // stands for the shim's package), so it is no site.
    let installed = paths.into_iter().find(|path| {
        // The standard library's files are the producer's `bundled:` ones.
        !path.starts_with("bundled:")
            && path.contains("/node_modules/")
            && !lookup.in_dependency_source(path)
            && (is_declaration_file(path) || !analyzed.contains(path.as_str()))
    });
    let path = match installed {
        Some(path) => path,
        // An import the compiler's attestation does not cover (its answer is unknown, not "unresolved"): undecided, so a site.
        None => unattested?,
    };
    if let Some(root) = package_root_of(&path)
        && runtime.contains(&installed_name(ctx, root).as_str())
    {
        return None;
    }
    Some(DeclarationAt { path })
}

/// The import binding a value's root names: the identifier `span` is, or the object of the member chain it is (`Pkg` of `Pkg.a.b`).
fn root_import<'t, 'f>(
    file: &FileFacts,
    bound: &'t HashMap<Span, ImportBinding<'f>>,
    span: Span,
) -> Option<&'t ImportBinding<'f>> {
    let ast = &file.ast;
    let mut at = ast.peel_ts_sugar_span(span);
    let mut hops = 0;
    while let Some(member) = ast.member_at(at) {
        hops += 1;
        if hops > 32 {
            return None;
        }
        at = ast.peel_ts_sugar_span(member.object);
    }
    bound.get(&ast.reference_declaration(at)?)
}

/// The package declaration an import loads, as the compiler attested it: `None` for the project's own modules, the dialect's runtime and an import
/// the compiler could not resolve (TypeScript reports it, and the boundary's SC9005 stands for the missing source). An import the attestation does
/// not cover is `unattested:<module>`, for which the reach index has no verdict, so it is undecided. A linked workspace package is named by the
/// `node_modules` spelling of its path, which the backend walks through the link.
fn import_declaration(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
    import: &ImportBinding<'_>,
) -> Option<DeclarationAt> {
    if import.decided_elsewhere {
        return None;
    }
    let lookup = ctx.semantic_lookup;
    let path = match lookup
        .resolved_imports()
        .map(|attested| attested.specifier(file.path.as_str(), import.declaration, import.module))
    {
        Some(solid_facts::SpecifierAttestation::Attested(row)) if row.resolved_path.is_empty() => {
            return None;
        }
        Some(solid_facts::SpecifierAttestation::Attested(row)) => {
            if row.symlink_path.is_empty() {
                row.resolved_path.to_string()
            } else {
                row.symlink_path.to_string()
            }
        }
        _ => format!("{UNATTESTED}{}", import.module),
    };
    if path.starts_with(UNATTESTED) {
        return Some(DeclarationAt { path });
    }
    (!path.starts_with("bundled:")
        && path.contains("/node_modules/")
        && !lookup.in_dependency_source(&path)
        && (is_declaration_file(&path) || !analyzed.contains(path.as_str())))
    .then_some(DeclarationAt { path })
}

/// What the package at `root` is called: the `name` its manifest gives (the backend read it), else the directory's own name.
fn installed_name(ctx: &AnalysisContext<'_>, root: &str) -> String {
    ctx.package_reach
        .and_then(|index| index.name_of(root))
        .map_or_else(|| package_label(root), str::to_owned)
}

/// The package name of a module specifier (`@solidjs/web/storage` is `@solidjs/web`).
fn package_name(module: &str) -> &str {
    let mut parts = module.splitn(3, '/');
    let first = parts.next().unwrap_or_default();
    if first.starts_with('@') {
        let second = parts.next().unwrap_or_default();
        &module[..(first.len() + 1 + second.len()).min(module.len())]
    } else {
        first
    }
}

/// The rows and silences of the sites ([`file_sites`]) of every analyzed file: an uncertifiable SC2002 row where the call may run under an
/// owner or nothing places it, a typed silence where its role allows an action or it cannot run.
pub(crate) fn package_action_calls(
    ctx: &AnalysisContext<'_>,
    rows: &mut Vec<crate::StaticViolation>,
    ends: &mut SiteEnds,
) {
    let lookup = ctx.semantic_lookup;
    for (file, file_sites) in lookup.files().iter().zip(sites(ctx)) {
        if file_sites.is_empty() {
            continue;
        }
        let allowed = crate::execution_role::allowed_callback_spans(file, lookup);
        for site in file_sites {
            if matches!(site.entry, Entry::Call | Entry::Tag) {
                let reached = ctx
                    .reachable_calls
                    .contains_key(&crate::location(file.path.shared(), site.callee));
                if !reached && !crate::local_access::unreached_write_may_run(file, site.at, lookup)
                {
                    ends.end_with(file, site.at, SiteSilence::PackageCallCannotRun, || {
                        vec![Premise::at(PremiseKind::SiteReach, Some(site.at))]
                    });
                    continue;
                }
                let role = if reached {
                    let role = crate::execution_role::semantic_write_execution_role(
                        file,
                        site.callee,
                        &allowed,
                        ctx.entities,
                        ctx.symbol_names,
                        lookup,
                        RootBodyGuard::Rejects,
                    );
                    crate::local_access::handed_write_execution(
                        file,
                        site.callee,
                        &allowed,
                        ctx.entities,
                        ctx.symbol_names,
                        lookup,
                        RootBodyGuard::Rejects,
                        role,
                    )
                } else {
                    crate::ExecutionRole::Unknown
                };
                if !(role.reports_disallowed_write() || role.reports_unplaced()) {
                    ends.end_at(
                        file,
                        site.at,
                        SiteSilence::PackageCallRoleAllows,
                        PremiseKind::RoleDoesNotReport,
                        || Some(site.callee),
                    );
                    continue;
                }
            }
            rows.push(row(file, site));
        }
    }
}

fn row(file: &FileFacts, site: &PackageSite) -> crate::StaticViolation {
    let entered = match site.entry {
        Entry::Call => "this call runs code of",
        Entry::Tag => "this tagged template runs code of",
        Entry::Render => "this element renders a component of",
        Entry::Value => "this hands on a function of",
    };
    crate::StaticViolation {
        id: "SC2002".into(),
        rule: "action-called-in-owned-scope".into(),
        message: format!(
            "{entered} `{}`, a package this build does not read as source, where an owner may be active; {}, so whether it calls a Solid \
             action (which throws `ACTION_CALLED_IN_OWNED_SCOPE` in the dev build) is not proven",
            site.package,
            site.reach.describe()
        ),
        hint: "Call the package from an event handler, an effect's apply function or onSettled; or let the build read the package as \
               source (a declared host with the Solid Vite plugin reads the packages that name Solid)."
            .into(),
        location: crate::location(file.path.shared(), site.at),
        analysis_context: format!("{}:{}", crate::blockers::PACKAGE_MAY_CALL_ACTION, site.reach.word()),
        fixes: Vec::new(),
        uncertain: true,
    }
}

/// The sites of the proof universe: every site outside the analyzed packages, as an action site of [`crate::proof_universe`].
pub(crate) fn universe_sites<'a>(
    ctx: &AnalysisContext<'a>,
) -> Vec<(&'a FileFacts, Span, Span, &'static str)> {
    let lookup = ctx.semantic_lookup;
    let mut found = Vec::new();
    for (file, file_sites) in lookup.files().iter().zip(sites(ctx)) {
        if lookup.in_dependency_source(file.path.as_str()) {
            continue;
        }
        for site in file_sites {
            found.push((file, site.at, site.callee, site.entry.via()));
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::package_name;

    #[test]
    fn a_module_names_its_package() {
        assert_eq!(package_name("solid-js"), "solid-js");
        assert_eq!(package_name("solid-js/store"), "solid-js");
        assert_eq!(package_name("@solidjs/web/storage"), "@solidjs/web");
        assert_eq!(package_name("@solidjs/web"), "@solidjs/web");
        assert_eq!(package_name("fz-step"), "fz-step");
    }
}
