//! Whether a project reaches the exports a scoped installation gap is about.
//!
//! Most gaps in what a vocabulary knows about an installation are about the
//! installation: whatever the project imports, the answer is unknown. Some are
//! about named exports only. `solid-js@2.0.0-rc.9`'s typings re-export five
//! names their own declarations no longer declare, so under `skipLibCheck`
//! those names do not resolve and a call through one is not the primitive; a
//! project that never reaches one of them loses nothing to that gap. The
//! dialect says which gap is scoped and to which exports
//! ([`solid_dialect::GapScope`]); this module answers, from the project's
//! syntax facts alone, whether the project reaches them. It knows how an
//! ECMAScript module reaches a named export, never why a name matters.
//!
//! The census fails closed. A use of the module that does not name the
//! exports it reaches -- a namespace object used other than as `S.name`, a
//! default import, `export *`, a literal dynamic `import()` or `require` --
//! keeps the gap due, because the export it reaches cannot be ruled out.
//!
//! A *nonliteral* load (`import(path)`) is not counted. The gap is about names
//! TypeScript would resolve through the module's declarations and does not on
//! this release; a nonliteral specifier resolves through no declarations on
//! any release, so what it loses it loses whether or not the gap is open.

use std::collections::{HashMap, HashSet};

use solid_dialect::GapScope;
use solid_facts::ast::{AstFacts, ExportKind, ImportKind, JsxElementFact};
use solid_facts::core::Span;
use solid_facts::{AttestedImportIndex, ProjectFacts, SpecifierAttestation};

use crate::dialect::{GapReach, ReleaseNotice};

/// How one site reaches a scoped gap's module.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Reach {
    /// One of the scope's exports, by name.
    Export(&'static str),
    /// A use that does not name the exports it reaches.
    Unbounded(Unbounded),
}

/// The uses of a module that reach an export set the facts cannot bound.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Unbounded {
    /// `import * as S` whose `S` is read other than as `S.name` or `<S.Name>`:
    /// passed, stored, destructured, re-exported, or named in a type.
    NamespaceEscapes,
    /// `import S from`: the module declares no default export, so what the
    /// binding holds is the bundler's interop, not a named export.
    DefaultImport,
    /// `export * from`, which re-exports every name to importers this census
    /// does not follow.
    ExportStar,
    /// `export * as S from`, a namespace object handed to importers.
    NamespaceReExport,
    /// A literal `import("…")` or `require("…")`, whose namespace object is
    /// not followed.
    ModuleLoad,
    /// `export { "…" } from` whose name the facts cannot spell exactly.
    UnspelledReExport,
}

impl Unbounded {
    fn describe(self, specifier: &str) -> String {
        match self {
            Self::NamespaceEscapes => {
                format!("a namespace import of {specifier} used other than as a member name")
            }
            Self::DefaultImport => format!("a default import of {specifier}"),
            Self::ExportStar => format!("export * from {specifier}"),
            Self::NamespaceReExport => format!("export * as a namespace from {specifier}"),
            Self::ModuleLoad => format!("a dynamic import or require of {specifier}"),
            Self::UnspelledReExport => {
                format!("a re-export from {specifier} whose name is not spelled plainly")
            }
        }
    }
}

/// One site where a project reaches a scoped gap's module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ScopeSite {
    pub path: String,
    pub span: Span,
    pub reach: Reach,
}

/// Every site in `files` that reaches `scope`: one of its exports by name, or
/// its module in a way that does not name the export. Empty means the project
/// reaches none of them.
///
/// `files` is `(path, source, facts)`; `resolved` is the compiler's own
/// answer for where each specifier resolves, when the analysis carries it,
/// so a `paths` alias that lands in the scope's package counts as the module.
pub(crate) fn scope_sites<'a>(
    scope: &GapScope,
    files: impl IntoIterator<Item = (&'a str, &'a str, &'a AstFacts)>,
    resolved: Option<&AttestedImportIndex>,
) -> Vec<ScopeSite> {
    let mut sites = Vec::new();
    for (path, source, ast) in files {
        let mut push = |span: Span, reach: Reach| {
            sites.push(ScopeSite {
                path: path.to_owned(),
                span,
                reach,
            });
        };
        let is_scope_module = |declaration: Span, text: &str| {
            is_scope_module(scope, path, declaration, text, resolved)
        };
        let mut namespaces = Vec::new();
        for import in &ast.imports {
            if !is_scope_module(import.span, &import.module) {
                continue;
            }
            for binding in &import.bindings {
                match binding.kind {
                    ImportKind::SideEffect => {}
                    ImportKind::Named => {
                        if let Some(export) = binding
                            .imported
                            .as_deref()
                            .and_then(|imported| scope_export(scope, imported))
                        {
                            push(binding.local.span, Reach::Export(export));
                        }
                    }
                    ImportKind::Default => {
                        push(
                            binding.local.span,
                            Reach::Unbounded(Unbounded::DefaultImport),
                        );
                    }
                    ImportKind::Namespace => namespaces.push(binding.local.span),
                }
            }
        }
        for export in &ast.exports {
            let Some(module) = export.module.as_deref() else {
                continue;
            };
            if !is_scope_module(export.span, module) {
                continue;
            }
            match export.kind {
                ExportKind::All if export.namespace.is_some() => {
                    push(export.span, Reach::Unbounded(Unbounded::NamespaceReExport))
                }
                ExportKind::All => push(export.span, Reach::Unbounded(Unbounded::ExportStar)),
                ExportKind::Named | ExportKind::Default => {
                    for specifier in &export.specifiers {
                        match spelled_name(source, specifier.local.span) {
                            Some(name) => {
                                if let Some(export) = scope_export(scope, name) {
                                    push(specifier.local.span, Reach::Export(export));
                                }
                            }
                            None => push(
                                specifier.local.span,
                                Reach::Unbounded(Unbounded::UnspelledReExport),
                            ),
                        }
                    }
                }
            }
        }
        for load in &ast.module_loads {
            if load
                .specifier
                .as_deref()
                .is_some_and(|specifier| is_scope_module(load.span, specifier))
            {
                push(load.span, Reach::Unbounded(Unbounded::ModuleLoad));
            }
        }
        if !namespaces.is_empty() {
            let members = member_names(source, ast);
            for (reference, declaration) in &ast.reference_declarations {
                if !namespaces.contains(declaration) {
                    continue;
                }
                match members.get(reference) {
                    Some((span, name)) => {
                        if let Some(export) = scope_export(scope, name) {
                            push(*span, Reach::Export(export));
                        }
                    }
                    None => push(*reference, Reach::Unbounded(Unbounded::NamespaceEscapes)),
                }
            }
        }
    }
    sites.sort_by(|left, right| {
        (&left.path, left.span.start, left.span.end).cmp(&(
            &right.path,
            right.span.start,
            right.span.end,
        ))
    });
    // A local `export { S }` reaches the binder's table twice for one span.
    sites.dedup();
    sites
}

/// Whether the specifier of the declaration at `declaration` is the scope's
/// module: spelled exactly as the scope names it, or attested to resolve into
/// the package of that name under another spelling (a `paths` alias). A
/// subpath of the package (`solid-js/internal`) is a different entry.
fn is_scope_module(
    scope: &GapScope,
    path: &str,
    declaration: Span,
    text: &str,
    resolved: Option<&AttestedImportIndex>,
) -> bool {
    if text == scope.specifier {
        return true;
    }
    if text
        .strip_prefix(scope.specifier)
        .is_some_and(|rest| rest.starts_with('/'))
    {
        return false;
    }
    let Some(SpecifierAttestation::Attested(row)) =
        resolved.map(|index| index.specifier(path, declaration, text))
    else {
        return false;
    };
    [&row.resolver_package_name, &row.package_name]
        .into_iter()
        .any(|name| name.as_deref() == Some(scope.specifier))
}

fn scope_export(scope: &GapScope, name: &str) -> Option<&'static str> {
    scope.exports.iter().copied().find(|export| *export == name)
}

/// The exact name written at `span`: an identifier as written, or a string
/// literal's contents when it holds no escape. `None` when the span does not
/// hold a name spelled plainly, which the caller treats as unbounded.
fn spelled_name(source: &str, span: Span) -> Option<&str> {
    let text = source.get(span.start as usize..span.end as usize)?;
    let unquoted = text
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .or_else(|| {
            text.strip_prefix('\'')
                .and_then(|rest| rest.strip_suffix('\''))
        });
    match unquoted {
        Some(inner) if inner.contains('\\') => None,
        Some(inner) => Some(inner),
        None if !text.is_empty()
            && text.chars().all(|character| {
                character == '$' || character == '_' || character.is_alphanumeric()
            }) =>
        {
            Some(text)
        }
        None => None,
    }
}

/// Every reference span that is the object of a member read naming exactly
/// one property -- `S.name`, `S?.name`, `S["name"]` (the key's cooked value) --
/// or of a dotted JSX tag (`<S.Name>`, and its closing tag), mapped to the
/// member's span and property name. A computed member whose key is not a
/// literal is absent, so its object reads as an escape.
fn member_names<'a>(source: &'a str, ast: &'a AstFacts) -> HashMap<Span, (Span, &'a str)> {
    let literal_keys = ast
        .literal_computed_members
        .iter()
        .map(|member| (member.span, member.key.as_str()))
        .collect::<HashMap<_, _>>();
    let computed = ast.computed_members.iter().collect::<HashSet<_>>();
    let mut members = HashMap::new();
    for member in &ast.members {
        let name = if computed.contains(&member.span) {
            literal_keys.get(&member.span).copied()
        } else {
            spelled_name(source, member.property)
        };
        if let Some(name) = name {
            members.insert(member.object, (member.span, name));
        }
    }
    for element in &ast.jsx_elements {
        jsx_member(source, element, &mut members);
    }
    members
}

fn jsx_member<'a>(
    source: &'a str,
    element: &JsxElementFact,
    members: &mut HashMap<Span, (Span, &'a str)>,
) {
    let (Some(object), Some(property)) = (element.member_object, element.member_property) else {
        return;
    };
    let Some(name) = spelled_name(source, property) else {
        return;
    };
    // Only a one-level tag: the object must be exactly the identifier the
    // reference names, which `spelled_name` accepts and a dotted object is not.
    if spelled_name(source, object).is_none() {
        return;
    }
    members.insert(object, (element.name.span, name));
    // `</S.Name>` writes the same name again, and its object is a second
    // reference to the same binding at the start of the closing name.
    if let Some(closing) = element.closing_name {
        let closing_object = Span::new(closing.start, closing.start + (object.end - object.start));
        if source.get(closing_object.start as usize..closing_object.end as usize)
            == source.get(object.start as usize..object.end as usize)
        {
            members.insert(closing_object, (closing, name));
        }
    }
}

impl ReleaseNotice {
    /// The notice as due for the project `facts` describes: every scoped gap
    /// the project does not reach is dropped, and every one it does reach
    /// names what reached it. `None` when no gap is left.
    #[must_use]
    pub fn scoped_to(mut self, facts: &ProjectFacts) -> Option<Self> {
        let mut gaps = Vec::with_capacity(self.gaps.len());
        for mut gap in std::mem::take(&mut self.gaps) {
            let Some(scope) = gap.scope else {
                gaps.push(gap);
                continue;
            };
            let sites = scope_sites(
                &scope,
                facts
                    .files
                    .iter()
                    .map(|file| (file.path.as_str(), file.source.as_ref(), file.ast.as_ref())),
                facts.resolved_imports.as_ref(),
            );
            if sites.is_empty() {
                continue;
            }
            gap.gap = format!("{}; {}", gap.gap, reached_clause(&scope, &sites));
            let mut seen = Vec::new();
            for site in &sites {
                if seen.contains(&site.reach) {
                    continue;
                }
                seen.push(site.reach);
                self.reaches.push(GapReach {
                    message: match site.reach {
                        Reach::Export(export) => {
                            format!("{export} is reached from {} here", scope.specifier)
                        }
                        Reach::Unbounded(kind) => format!(
                            "{} here, so which of its exports the project reaches is not known",
                            kind.describe(scope.specifier)
                        ),
                    },
                    path: site.path.clone(),
                    span: site.span,
                });
            }
            gaps.push(gap);
        }
        self.gaps = gaps;
        (!self.gaps.is_empty()).then_some(self)
    }
}

/// What made a scoped gap due, as a clause of its sentence.
fn reached_clause(scope: &GapScope, sites: &[ScopeSite]) -> String {
    let exports = scope
        .exports
        .iter()
        .filter(|export| sites.iter().any(|site| site.reach == Reach::Export(export)))
        .map(|export| (*export).to_owned())
        .collect::<Vec<_>>();
    let mut unbounded = sites
        .iter()
        .filter_map(|site| match site.reach {
            Reach::Unbounded(kind) => Some(kind),
            Reach::Export(_) => None,
        })
        .collect::<Vec<_>>();
    unbounded.sort_unstable();
    unbounded.dedup();
    let mut clauses = Vec::new();
    if !exports.is_empty() {
        clauses.push(format!(
            "this project uses {} from {}",
            english_list(&exports),
            scope.specifier
        ));
    }
    if !unbounded.is_empty() {
        clauses.push(format!(
            "this project uses {} in a way that does not name the exports it reaches ({}), so the \
             gap stays open",
            scope.specifier,
            english_list(
                &unbounded
                    .iter()
                    .map(|kind| kind.describe(scope.specifier))
                    .collect::<Vec<_>>()
            )
        ));
    }
    clauses.join(", and ")
}

fn english_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [only] => only.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCOPE: GapScope = GapScope {
        specifier: "solid-js",
        exports: &[
            "createErrorBoundary",
            "createLoadingBoundary",
            "createRevealOrder",
            "sharedConfig",
            "$DEVCOMP",
        ],
    };

    fn reaches(files: &[(&str, &str)]) -> Vec<Reach> {
        let facts = files
            .iter()
            .map(|(path, source)| {
                (
                    *path,
                    *source,
                    solid_facts::ast::extract(*path, source).expect("the source parses"),
                )
            })
            .collect::<Vec<_>>();
        scope_sites(
            &SCOPE,
            facts
                .iter()
                .map(|(path, source, ast)| (*path, *source, ast)),
            None,
        )
        .into_iter()
        .map(|site| site.reach)
        .collect()
    }

    fn one(source: &str) -> Vec<Reach> {
        reaches(&[("/p/App.tsx", source)])
    }

    #[test]
    fn a_project_that_names_none_of_the_exports_reaches_nothing() {
        assert_eq!(
            one(r#"import { createSignal, createMemo } from "solid-js";
                   import "solid-js";
                   import { createErrorBoundary } from "solid-js/internal";
                   import { createErrorBoundary as other } from "./local";
                   export { createErrorBoundary } from "some-other-package";
                   const [a] = createSignal(1);
                   const b = createMemo(() => a());
                   const load = (name: string) => import(name);"#),
            []
        );
    }

    #[test]
    fn every_import_form_that_names_an_export_reaches_it() {
        // Named, aliased, type-only, and a string-literal import name.
        assert_eq!(
            one(
                r#"import { createErrorBoundary, sharedConfig as config } from "solid-js";
                   import type { createRevealOrder } from "solid-js";
                   import { "$DEVCOMP" as brand } from "solid-js";"#
            ),
            [
                Reach::Export("createErrorBoundary"),
                Reach::Export("sharedConfig"),
                Reach::Export("createRevealOrder"),
                Reach::Export("$DEVCOMP"),
            ]
        );
        // A project module re-exporting one, aliased or not.
        assert_eq!(
            one(r#"export { createLoadingBoundary as Loading, createSignal } from "solid-js";"#),
            [Reach::Export("createLoadingBoundary")]
        );
    }

    #[test]
    fn a_namespace_import_reaches_only_the_members_it_names() {
        assert_eq!(
            one(r#"import * as S from "solid-js";
                   const [a] = S.createSignal(1);
                   S.createErrorBoundary(() => a(), () => null);
                   const view = <S.Show when={a()}>x</S.Show>;"#),
            [Reach::Export("createErrorBoundary")]
        );
        // A literal computed key names exactly one export; `S[0]` names none.
        assert_eq!(
            one(r#"import * as S from "solid-js";
                   const a = S["sharedConfig"];
                   const b = S[0];"#),
            [Reach::Export("sharedConfig")]
        );
        // Escaping into a call, a destructuring, a re-export, or a type.
        for escape in [
            "consume(S);",
            "const { createErrorBoundary } = S;",
            "export { S };",
            "type T = typeof S;",
            "declare const key: string; const k = S[key];",
            "const k = S[\"untrack\" as const];",
        ] {
            assert_eq!(
                one(&format!("import * as S from \"solid-js\";\n{escape}")),
                [Reach::Unbounded(Unbounded::NamespaceEscapes)],
                "{escape}"
            );
        }
    }

    #[test]
    fn uses_that_name_no_export_keep_the_gap_open() {
        assert_eq!(
            one(r#"import Solid from "solid-js";"#),
            [Reach::Unbounded(Unbounded::DefaultImport)]
        );
        assert_eq!(
            one(r#"export * from "solid-js";"#),
            [Reach::Unbounded(Unbounded::ExportStar)]
        );
        assert_eq!(
            one(r#"export * as Solid from "solid-js";"#),
            [Reach::Unbounded(Unbounded::NamespaceReExport)]
        );
        assert_eq!(
            one(r#"const solid = await import("solid-js");"#),
            [Reach::Unbounded(Unbounded::ModuleLoad)]
        );
    }

    #[test]
    fn a_shadowed_namespace_name_is_not_the_import() {
        // The inner `S` is a parameter: its member read is not the namespace's.
        assert_eq!(
            one(r#"import * as S from "solid-js";
                   const f = (S: { createErrorBoundary(): void }) => S.createErrorBoundary();
                   S.createSignal(1);"#),
            []
        );
    }

    #[test]
    fn a_re_export_through_a_project_module_is_seen_in_the_module_that_re_exports() {
        assert_eq!(
            reaches(&[
                (
                    "/p/boundary.ts",
                    r#"export { createErrorBoundary } from "solid-js";"#
                ),
                (
                    "/p/App.tsx",
                    r#"import { createErrorBoundary } from "./boundary";
                       createErrorBoundary(() => 1, () => 2);"#
                ),
            ]),
            [Reach::Export("createErrorBoundary")]
        );
    }
}
