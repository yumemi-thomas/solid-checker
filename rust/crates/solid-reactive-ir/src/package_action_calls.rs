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

use solid_facts::FileFacts;
use solid_facts::core::Span;
use solid_facts::package_reach::{
    PackageReach, ReachBlocker, is_declaration_file, package_root_of,
};

use crate::execution_role::RootBodyGuard;
use crate::pipeline::AnalysisContext;
use crate::proof_log::{Premise, PremiseKind};
use crate::site_accounting::{SiteEnds, SiteSilence};

/// How the project enters the package's code at a site.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Entry {
    /// A call or `new`.
    Call,
    /// A tagged template's tag.
    Tag,
    /// A JSX element whose tag is the package's component.
    Render,
}

impl Entry {
    pub(crate) const fn via(self) -> &'static str {
        match self {
            Self::Call => "call",
            Self::Tag => "tag",
            Self::Render => "render",
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

/// The sites of one file, in source order.
pub(crate) fn file_sites(ctx: &AnalysisContext<'_>, file: &FileFacts) -> Vec<PackageSite> {
    let lookup = ctx.semantic_lookup;
    let mut sites = Vec::new();
    let mut push = |at: Span, callee: Span, entry: Entry, declaration: Option<DeclarationAt>| {
        let Some(declaration) = declaration else {
            return;
        };
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
            .and_then(|root| {
                root.rsplit_once("/node_modules/")
                    .map(|(_, name)| name.to_owned())
            })
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
        if lookup.primitive_at_call(file, call.span).is_some() {
            continue;
        }
        push(
            call.span,
            call.callee,
            Entry::Call,
            package_declaration(ctx, file, call.callee, true),
        );
    }
    for template in &file.ast.tagged_templates {
        push(
            template.span,
            template.tag,
            Entry::Tag,
            package_declaration(ctx, file, template.tag, true),
        );
    }
    for element in &file.ast.jsx_elements {
        let opening = if element.opening.end > element.opening.start {
            element.opening
        } else {
            element.name.span
        };
        push(
            opening,
            element.name.span,
            Entry::Render,
            package_declaration(ctx, file, element.name.span, false),
        );
    }
    sites.sort_by_key(|site| (site.at.start, site.at.end));
    sites
}

struct DeclarationAt {
    path: String,
}

/// The declaration of the code `callee` enters, when it is a package's the build does not read: an installed file or a declaration file,
/// outside the standard library, the analyzed packages and the dialect's runtime. `None` for project code, the standard library, a callee
/// nothing resolves (the handed-callee obligations' concern) and Solid's own runtime.
fn package_declaration(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    callee: Span,
    call: bool,
) -> Option<DeclarationAt> {
    let lookup = ctx.semantic_lookup;
    let runtime = ctx.dialect.primitive_defining_packages();
    let is_runtime_module = |module: &str| {
        let name = package_name(module);
        runtime.contains(&name) || ctx.dialect.modules().contains(&module)
    };
    // The callee's root names an import of the dialect's runtime (`import { Show } from "solid-js"`): its catalog answers.
    if let Some(module) = imported_module(file, callee)
        && is_runtime_module(module)
    {
        return None;
    }
    let mut paths = Vec::<String>::new();
    if call && let Some(resolved) = lookup.resolved_callee_call(file, callee) {
        let declaration = resolved.declaration.as_ref()?;
        if declaration.standard_library {
            return None;
        }
        if !declaration.origin_module.is_empty() && is_runtime_module(&declaration.origin_module) {
            return None;
        }
        paths.push(declaration.location.path.to_string());
    } else {
        let symbol = lookup
            .entities()
            .at(file.path.as_str(), file.ast.peel_ts_sugar_span(callee))?;
        let declarations = lookup.symbol_declarations(symbol.as_str())?;
        paths.extend(
            declarations
                .iter()
                .map(|declaration| declaration.location.path.to_string()),
        );
        if let Some(target) = lookup.alias_target_of(symbol.as_str())
            && let Some(declarations) = lookup.symbol_declarations(target)
        {
            paths.extend(
                declarations
                    .iter()
                    .map(|declaration| declaration.location.path.to_string()),
            );
        }
    }
    // Project code (a body the analysis reads) decides nothing here; a package's or a declaration file's declaration does.
    let path = paths.into_iter().find(|path| {
        (path.contains("/node_modules/") || is_declaration_file(path))
            && !lookup.in_dependency_source(path)
    })?;
    if let Some(root) = package_root_of(&path)
        && let Some((_, name)) = root.rsplit_once("/node_modules/")
        && runtime.contains(&name)
    {
        return None;
    }
    Some(DeclarationAt { path })
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

/// The module the import that binds `callee`'s root identifier names (`pkg` in `pkg.run()` of `import * as pkg from "m"`).
fn imported_module<'f>(file: &'f FileFacts, callee: Span) -> Option<&'f str> {
    let ast = &file.ast;
    let mut at = ast.peel_ts_sugar_span(callee);
    for _ in 0..32 {
        match ast.member_at(at) {
            Some(member) => at = ast.peel_ts_sugar_span(member.object),
            None => break,
        }
    }
    let declaration = ast.reference_declaration(at)?;
    ast.imports
        .iter()
        .find(|import| {
            import
                .bindings
                .iter()
                .any(|binding| binding.local.span == declaration)
        })
        .map(|import| import.module.as_str())
}

/// The rows and silences of the sites ([`file_sites`]) of every analyzed file: an uncertifiable SC2002 row where the call may run under an
/// owner or nothing places it, a typed silence where its role allows an action or it cannot run.
pub(crate) fn package_action_calls(
    ctx: &AnalysisContext<'_>,
    rows: &mut Vec<crate::StaticViolation>,
    ends: &mut SiteEnds,
) {
    let lookup = ctx.semantic_lookup;
    for file in crate::base::reads::file_slots(ctx.facts) {
        let sites = file_sites(ctx, file);
        if sites.is_empty() {
            continue;
        }
        let allowed = crate::execution_role::allowed_callback_spans(file, lookup);
        for site in sites {
            if site.entry != Entry::Render {
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
            rows.push(row(file, &site));
        }
    }
}

fn row(file: &FileFacts, site: &PackageSite) -> crate::StaticViolation {
    let entered = match site.entry {
        Entry::Call => "this call runs code of",
        Entry::Tag => "this tagged template runs code of",
        Entry::Render => "this element renders a component of",
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

/// The sites of the proof universe: every [`file_sites`] site outside the analyzed packages, as an action site of [`crate::proof_universe`].
pub(crate) fn universe_sites<'a>(
    ctx: &AnalysisContext<'a>,
) -> Vec<(&'a FileFacts, Span, Span, &'static str)> {
    let lookup = ctx.semantic_lookup;
    let mut found = Vec::new();
    for file in crate::base::reads::file_slots(ctx.facts) {
        if lookup.in_dependency_source(file.path.as_str()) {
            continue;
        }
        for site in file_sites(ctx, file) {
            found.push((file, site.at, site.callee, site.entry.via()));
        }
    }
    found
}
