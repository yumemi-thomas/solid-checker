//! Whether a release notice is due for a project at all, and whether a project
//! reaches the exports a scoped installation gap is about.
//!
//! **Whether the project uses the runtime.** Every gap is about the packages
//! the dialect names as deciding its release-dependent answers
//! ([`solid_dialect::Dialect::release_owners`]). A project that reaches none
//! of them -- no module reference into one, no JSX, and no dependency whose
//! installed closure depends on one -- asks the vocabulary no release-dependent
//! question, so no gap is due for it ([`solid_use`]). That census fails closed
//! too: a module reference whose resolution the facts do not carry, or a
//! dependency closure that cannot be read to its end, keeps the notice.
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
use std::path::{Path, PathBuf};

use solid_dialect::GapScope;
use solid_facts::ast::{AstFacts, ExportKind, ImportKind, JsxElementFact};
use solid_facts::core::Span;
use solid_facts::{
    AttestedImport, AttestedImportIndex, ImportResolution, ProjectFacts, SpecifierAttestation,
};

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
    /// `typeof import("…")` in a type position, which names the whole
    /// namespace rather than one export.
    NamespaceType,
    /// `export import S = require("…")`, a namespace object handed to
    /// importers.
    ExportedImportEquals,
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
            Self::NamespaceType => {
                format!("a type-position import(\"{specifier}\") naming no export")
            }
            Self::ExportedImportEquals => {
                format!("export import … = require(\"{specifier}\")")
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
        // `import S = require("…")` binds the namespace object exactly as
        // `import * as S` does, so its references are read the same way.
        for import in &ast.import_equals {
            if !is_scope_module(import.span, &import.module) {
                continue;
            }
            namespaces.push(import.local.span);
            if import.exported {
                push(
                    import.span,
                    Reach::Unbounded(Unbounded::ExportedImportEquals),
                );
            }
        }
        // A type-position `import("…")` resolves through the same declarations
        // a type-only import does, and is counted the same way.
        for import in &ast.type_imports {
            if !is_scope_module(import.span, &import.module) {
                continue;
            }
            match import.member.as_deref() {
                Some(member) => {
                    if let Some(export) = scope_export(scope, member) {
                        push(import.span, Reach::Export(export));
                    }
                }
                None => push(import.span, Reach::Unbounded(Unbounded::NamespaceType)),
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

/// Why a project cannot be ruled out as a user of the dialect's runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SolidUse {
    /// A module reference -- an import, a re-export, a literal dynamic
    /// `import()` or `require`, an `import S = require`, or a type-position
    /// `import()` -- that names a runtime package or one of its subpaths, or
    /// that the compiler resolved into one (a `paths` alias).
    RuntimeModule,
    /// JSX. The dialect's compiler lowers every JSX expression onto its own
    /// runtime whatever `jsxImportSource` the project configures, so JSX is an
    /// answer the vocabulary gives about that runtime.
    Jsx,
    /// A package whose installed dependency closure names a runtime package:
    /// its answers (a contract, a typing) can depend on the release.
    RuntimeDependency,
    /// A package reference whose resolution the facts do not carry, that
    /// resolved nowhere, or whose installed dependency closure could not be
    /// read to its end.
    Unknown,
}

/// The first site that keeps the project from being ruled out as a user of
/// the `runtime` packages, and why; `None` when the facts rule every use out.
///
/// `runtime` is the dialect's list of release-owning packages, never a list
/// shared code spells. Empty means the dialect named none, and nothing can
/// then be ruled out.
///
/// The census reads every module reference in every analyzed file:
///
/// - one naming a runtime package (or a subpath of one), or attested to
///   resolve into one, is a use;
/// - any JSX is a use;
/// - a relative specifier names a project file, which this census reads in
///   turn when it is analyzed and which the analysis asks nothing of when it
///   is not -- unless it walks into a `node_modules` tree, which makes it a
///   package reference;
/// - a `node:` specifier is a platform builtin, never a package;
/// - every other reference is to a package: it is a use unless the compiler's
///   attested resolution places it, and the installed dependency closure of
///   the package it lands in names no runtime package. An unattested or
///   unresolved package reference fails closed.
///
/// A nonliteral `import(path)` or `require(path)` is not counted, for the
/// reason [`scope_sites`] gives: it resolves through no declarations, so no
/// answer the vocabulary gives is asked of what it loads.
pub(crate) fn solid_use<'a>(
    runtime: &[&str],
    files: impl IntoIterator<Item = (&'a str, &'a AstFacts)> + Clone,
    resolved: Option<&AttestedImportIndex>,
) -> Option<(String, Span, SolidUse)> {
    if runtime.is_empty() {
        return Some((String::new(), Span::new(0, 0), SolidUse::Unknown));
    }
    let project_files = files
        .clone()
        .into_iter()
        .map(|(path, _)| path)
        .collect::<HashSet<_>>();
    let is_runtime_package = |name: &str| runtime.contains(&name);
    let names_runtime = |text: &str| {
        runtime.iter().any(|package| {
            text.strip_prefix(package)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
        })
    };
    // Syntax first: a direct use is the common answer and needs no
    // filesystem.
    let mut packages = Vec::new();
    for (path, ast) in files {
        if let Some(span) = ast
            .jsx_elements
            .first()
            .map(|element| element.span)
            .or_else(|| ast.jsx_fragments.first().copied())
        {
            return Some((path.to_owned(), span, SolidUse::Jsx));
        }
        for (declaration, text) in module_references(ast) {
            let row = match resolved.map(|index| index.specifier(path, declaration, text)) {
                Some(SpecifierAttestation::Attested(row)) => Some(row),
                _ => None,
            };
            let lands_in_runtime = row.is_some_and(|row| {
                [&row.resolver_package_name, &row.package_name]
                    .into_iter()
                    .any(|name| name.as_deref().is_some_and(is_runtime_package))
            });
            if names_runtime(text) || lands_in_runtime {
                return Some((path.to_owned(), declaration, SolidUse::RuntimeModule));
            }
            packages.push((path, declaration, text, row));
        }
    }
    let mut closure = DependencyClosure::new(runtime);
    for (path, declaration, text, row) in packages {
        let verdict = match row {
            Some(row) => closure.resolution(row, &project_files),
            None if text.starts_with("node:") => None,
            None if is_relative(text) => text
                .split(['/', '\\'])
                .any(|segment| segment == "node_modules")
                .then_some(SolidUse::Unknown),
            None => Some(SolidUse::Unknown),
        };
        if let Some(verdict) = verdict {
            return Some((path.to_owned(), declaration, verdict));
        }
    }
    None
}

/// Every module reference a file makes, as `(declaration span, specifier)`:
/// the span an attested resolution row is joined by.
fn module_references(ast: &AstFacts) -> impl Iterator<Item = (Span, &str)> {
    let imports = ast
        .imports
        .iter()
        .map(|import| (import.span, import.module.as_str()));
    let exports = ast
        .exports
        .iter()
        .filter_map(|export| export.module.as_deref().map(|module| (export.span, module)));
    let loads = ast
        .module_loads
        .iter()
        .filter_map(|load| load.specifier.as_deref().map(|module| (load.span, module)));
    let equals = ast
        .import_equals
        .iter()
        .map(|import| (import.span, import.module.as_str()));
    let types = ast
        .type_imports
        .iter()
        .map(|import| (import.span, import.module.as_str()));
    imports
        .chain(exports)
        .chain(loads)
        .chain(equals)
        .chain(types)
}

fn is_relative(specifier: &str) -> bool {
    specifier == "."
        || specifier == ".."
        || specifier.starts_with("./")
        || specifier.starts_with("../")
        || specifier.starts_with('/')
}

/// The installed dependency closures this census has read, per package
/// directory.
///
/// A package's closure is read from the manifests on disk the way Node
/// resolves a bare specifier from inside the package: every name in its
/// `dependencies`, `peerDependencies` and `optionalDependencies`, each looked
/// up in the nearest `node_modules/<name>` above the package's *real*
/// directory, and that package's closure in turn. A name that is a runtime
/// package is a use wherever it appears, installed or not. A dependency that
/// does not resolve is skipped only when the manifest says it is optional
/// (`optionalDependencies`, or `peerDependenciesMeta.<name>.optional`); any
/// other unresolved dependency, and any manifest that cannot be read, fails
/// closed.
struct DependencyClosure<'r> {
    runtime: &'r [&'r str],
    read: HashSet<PathBuf>,
}

impl<'r> DependencyClosure<'r> {
    fn new(runtime: &'r [&'r str]) -> Self {
        Self {
            runtime,
            read: HashSet::new(),
        }
    }

    /// The verdict for one attested resolution: `None` when it lands in an
    /// analyzed project file, or in a package whose closure names no runtime
    /// package.
    fn resolution(
        &mut self,
        row: &AttestedImport,
        project_files: &HashSet<&str>,
    ) -> Option<SolidUse> {
        if row.resolution == ImportResolution::Unresolved {
            return Some(SolidUse::Unknown);
        }
        let landed = [&row.resolved_path, &row.included_path, &row.symlink_path];
        if landed
            .iter()
            .any(|path| !path.is_empty() && project_files.contains(path.as_ref()))
        {
            return None;
        }
        // A relative specifier that lands outside every `node_modules` tree is
        // a project file this analysis does not read (a stylesheet, data, a
        // script outside `include`), exactly as when it is not attested.
        if row.resolution == ImportResolution::Relative
            && !Path::new(row.resolved_path.as_ref())
                .components()
                .any(|component| component.as_os_str() == "node_modules")
        {
            return None;
        }
        let Some(root) = package_root(Path::new(row.resolved_path.as_ref())) else {
            return Some(SolidUse::Unknown);
        };
        self.package(root)
    }

    fn package(&mut self, root: PathBuf) -> Option<SolidUse> {
        let mut pending = vec![root];
        while let Some(directory) = pending.pop() {
            if !self.read.insert(directory.clone()) {
                continue;
            }
            let Some(manifest) = read_manifest(&directory) else {
                return Some(SolidUse::Unknown);
            };
            if manifest
                .get("name")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|name| self.runtime.contains(&name))
            {
                return Some(SolidUse::RuntimeDependency);
            }
            let optional = optional_dependencies(&manifest);
            for field in ["dependencies", "peerDependencies", "optionalDependencies"] {
                let Some(names) = manifest.get(field) else {
                    continue;
                };
                let Some(names) = names.as_object() else {
                    return Some(SolidUse::Unknown);
                };
                for name in names.keys() {
                    if self.runtime.contains(&name.as_str()) {
                        return Some(SolidUse::RuntimeDependency);
                    }
                    match installed_dependency(&directory, name) {
                        Some(found) => pending.push(found),
                        None if optional.contains(name.as_str()) => {}
                        None => return Some(SolidUse::Unknown),
                    }
                }
            }
        }
        None
    }
}

/// The directory of the package that owns `file`: the nearest ancestor whose
/// `package.json` carries a `name`, real-path normalized. An unnamed nested
/// manifest (`{"type":"module"}` beside a package's ESM output) is passed
/// over. `None` when no named manifest is found before the walk leaves a
/// `node_modules` tree or reaches the filesystem root.
fn package_root(file: &Path) -> Option<PathBuf> {
    for directory in file.ancestors().skip(1) {
        if directory
            .file_name()
            .is_some_and(|name| name == "node_modules")
        {
            return None;
        }
        let Some(manifest) = read_manifest(directory) else {
            continue;
        };
        if manifest
            .get("name")
            .is_some_and(serde_json::Value::is_string)
        {
            return std::fs::canonicalize(directory).ok();
        }
    }
    None
}

fn read_manifest(directory: &Path) -> Option<serde_json::Value> {
    let bytes = std::fs::read(directory.join("package.json")).ok()?;
    let value = serde_json::from_slice::<serde_json::Value>(&bytes).ok()?;
    value.is_object().then_some(value)
}

fn optional_dependencies(manifest: &serde_json::Value) -> HashSet<&str> {
    let mut optional = manifest
        .get("optionalDependencies")
        .and_then(serde_json::Value::as_object)
        .map(|names| names.keys().map(String::as_str).collect::<HashSet<_>>())
        .unwrap_or_default();
    if let Some(meta) = manifest
        .get("peerDependenciesMeta")
        .and_then(serde_json::Value::as_object)
    {
        optional.extend(
            meta.iter()
                .filter(|(_, entry)| {
                    entry.get("optional").and_then(serde_json::Value::as_bool) == Some(true)
                })
                .map(|(name, _)| name.as_str()),
        );
    }
    optional
}

/// Where `name` resolves from inside the package at `directory`: the nearest
/// `node_modules/<name>` holding a `package.json`, real-path normalized, so a
/// linked install (pnpm) continues from the package's own store directory.
fn installed_dependency(directory: &Path, name: &str) -> Option<PathBuf> {
    directory.ancestors().find_map(|ancestor| {
        if ancestor
            .file_name()
            .is_some_and(|base| base == "node_modules")
        {
            return None;
        }
        let candidate = ancestor.join("node_modules").join(name);
        if candidate.join("package.json").is_file() {
            std::fs::canonicalize(&candidate).ok()
        } else {
            None
        }
    })
}

impl ReleaseNotice {
    /// The notice as due for the project `facts` describes: `None` when the
    /// project cannot be using the runtime the notice is about
    /// ([`solid_use`]); otherwise every scoped gap the project does not reach
    /// is dropped, and every one it does reach names what reached it. `None`
    /// when no gap is left.
    #[must_use]
    pub fn scoped_to(mut self, facts: &ProjectFacts) -> Option<Self> {
        let runtime = self
            .releases
            .iter()
            .map(|release| release.package)
            .collect::<Vec<_>>();
        solid_use(
            &runtime,
            facts
                .files
                .iter()
                .map(|file| (file.path.as_str(), file.ast.as_ref())),
            facts.resolved_imports.as_ref(),
        )?;
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

    #[test]
    fn an_import_equals_namespace_is_read_like_a_namespace_import() {
        assert_eq!(
            one(r#"import S = require("solid-js");
                   S.createSignal(1);
                   S.createErrorBoundary(() => 1, () => 2);"#),
            [Reach::Export("createErrorBoundary")]
        );
        assert_eq!(
            one(r#"import S = require("solid-js");
                   S.createSignal(1);"#),
            []
        );
        assert_eq!(
            one(r#"import S = require("solid-js");
                   consume(S);"#),
            [Reach::Unbounded(Unbounded::NamespaceEscapes)]
        );
        assert_eq!(
            one(r#"export import S = require("solid-js");"#),
            [Reach::Unbounded(Unbounded::ExportedImportEquals)]
        );
        // Another module's import-equals reaches nothing here.
        assert_eq!(
            one(r#"import S = require("./local");
                   S.createErrorBoundary(() => 1, () => 2);"#),
            []
        );
    }

    #[test]
    fn a_type_position_import_is_counted_like_a_type_only_import() {
        assert_eq!(
            one(r#"type A = typeof import("solid-js").createErrorBoundary;
                   type B = import("solid-js").Accessor<number>;"#),
            [Reach::Export("createErrorBoundary")]
        );
        assert_eq!(
            one(r#"type N = typeof import("solid-js");"#),
            [Reach::Unbounded(Unbounded::NamespaceType)]
        );
    }

    const RUNTIME: &[&str] = &["solid-js", "@solidjs/signals", "@solidjs/web"];

    fn solid_use_of(
        files: &[(&str, &str)],
        resolved: Option<&AttestedImportIndex>,
    ) -> Option<SolidUse> {
        let facts = files
            .iter()
            .map(|(path, source)| {
                (
                    *path,
                    solid_facts::ast::extract(*path, source).expect("the source parses"),
                )
            })
            .collect::<Vec<_>>();
        solid_use(
            RUNTIME,
            facts.iter().map(|(path, ast)| (*path, ast)),
            resolved,
        )
        .map(|(_, _, kind)| kind)
    }

    #[test]
    fn a_project_that_names_no_runtime_module_and_writes_no_jsx_uses_none() {
        assert_eq!(
            solid_use_of(
                &[
                    (
                        "/p/index.ts",
                        r#"import { helper } from "./helper";
                           import { readFileSync } from "node:fs";
                           const load = (name: string) => import(name);
                           export const value = helper(readFileSync);"#
                    ),
                    ("/p/helper.ts", "export const helper = (x: unknown) => x;"),
                ],
                None
            ),
            None
        );
    }

    #[test]
    fn every_module_reference_into_the_runtime_is_a_use() {
        for source in [
            r#"import { createSignal } from "solid-js";"#,
            r#"import type { Component } from "solid-js";"#,
            r#"import "@solidjs/web";"#,
            r#"import { jsx } from "@solidjs/web/jsx-runtime";"#,
            r#"import { createStore } from "@solidjs/signals";"#,
            r#"export * from "solid-js";"#,
            r#"export { untrack } from "solid-js";"#,
            r#"const solid = await import("solid-js");"#,
            r#"const solid = require("solid-js");"#,
            r#"import S = require("solid-js");"#,
            r#"type A = import("solid-js").Accessor<number>;"#,
        ] {
            assert_eq!(
                solid_use_of(&[("/p/index.ts", source)], None),
                Some(SolidUse::RuntimeModule),
                "{source}"
            );
        }
    }

    #[test]
    fn any_jsx_is_a_use_whatever_its_runtime() {
        assert_eq!(
            solid_use_of(&[("/p/App.tsx", "export const view = <div />;")], None),
            Some(SolidUse::Jsx)
        );
        assert_eq!(
            solid_use_of(&[("/p/App.tsx", "export const view = <></>;")], None),
            Some(SolidUse::Jsx)
        );
    }

    #[test]
    fn a_package_reference_the_facts_cannot_place_fails_closed() {
        // No attested resolution: which package `tailwindcss/plugin` is, and
        // what it depends on, is unknown.
        for source in [
            r#"import plugin from "tailwindcss/plugin";"#,
            r#"import type { StyleRule } from "@vanilla-extract/css";"#,
            r#"import T = require("some-package");"#,
            r#"import solid from "solid-jsx";"#,
            r#"import direct from "../node_modules/solid-js/dist/solid.js";"#,
        ] {
            assert_eq!(
                solid_use_of(&[("/p/index.ts", source)], None),
                Some(SolidUse::Unknown),
                "{source}"
            );
        }
        // A dialect that names no runtime package rules nothing out.
        assert!(solid_use(&[], std::iter::empty(), None).is_some());
    }

    fn row(
        text: &str,
        span: Span,
        resolved: &Path,
        resolution: ImportResolution,
    ) -> AttestedImport {
        AttestedImport {
            span,
            text: text.into(),
            resolution,
            resolved_path: resolved.display().to_string().into(),
            included_path: "".into(),
            symlink_path: "".into(),
            extension: ".d.ts".into(),
            package_name: None,
            package_version: None,
            package_manifest: None,
            resolver_package_name: None,
            resolver_package_version: None,
        }
    }

    fn write(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn a_package_reference_is_judged_by_its_installed_dependency_closure() {
        let root = std::env::temp_dir().join(format!(
            "solid-checker-release-scope-closure-{}",
            std::process::id()
        ));
        let modules = root.join("node_modules");
        // `free` -> `leaf` (and an optional peer and optional dependency that
        // are not installed): no runtime anywhere.
        write(
            &modules.join("free/package.json"),
            r#"{"name":"free","dependencies":{"leaf":"1"},"optionalDependencies":{"fsevents":"2"},
               "peerDependencies":{"maybe":"1"},"peerDependenciesMeta":{"maybe":{"optional":true}}}"#,
        );
        write(&modules.join("free/dist/index.d.ts"), "export {};");
        // An unnamed nested manifest beside the ESM output is passed over.
        write(
            &modules.join("free/dist/package.json"),
            r#"{"type":"module"}"#,
        );
        write(&modules.join("leaf/package.json"), r#"{"name":"leaf"}"#);
        // `wraps` -> `bridge` (installed in `wraps`'s own node_modules), which
        // peer-depends on the runtime.
        write(
            &modules.join("wraps/package.json"),
            r#"{"name":"wraps","dependencies":{"bridge":"1"}}"#,
        );
        write(&modules.join("wraps/index.d.ts"), "export {};");
        write(
            &modules.join("wraps/node_modules/bridge/package.json"),
            r#"{"name":"bridge","peerDependencies":{"solid-js":"^2"}}"#,
        );
        // `broken` names a required dependency that is not installed.
        write(
            &modules.join("broken/package.json"),
            r#"{"name":"broken","dependencies":{"missing":"1"}}"#,
        );
        write(&modules.join("broken/index.d.ts"), "export {};");
        let root = std::fs::canonicalize(&root).unwrap();
        let modules = root.join("node_modules");

        let case = |text: &str, resolved: &Path, resolution: ImportResolution| {
            let source = format!("import x from \"{text}\";\nexport default x;\n");
            let ast = solid_facts::ast::extract("/p/index.ts", &source).unwrap();
            let literal = source.find('"').unwrap() as u32;
            let span = Span::new(literal, literal + text.len() as u32 + 2);
            let mut index = AttestedImportIndex::default();
            index.insert_file("/p/index.ts", vec![row(text, span, resolved, resolution)]);
            solid_use(RUNTIME, [("/p/index.ts", &ast)], Some(&index)).map(|(_, _, kind)| kind)
        };
        assert_eq!(
            case(
                "free",
                &modules.join("free/dist/index.d.ts"),
                ImportResolution::NodeModules
            ),
            None
        );
        assert_eq!(
            case(
                "wraps",
                &modules.join("wraps/index.d.ts"),
                ImportResolution::NodeModules
            ),
            Some(SolidUse::RuntimeDependency)
        );
        assert_eq!(
            case(
                "broken",
                &modules.join("broken/index.d.ts"),
                ImportResolution::NodeModules
            ),
            Some(SolidUse::Unknown)
        );
        assert_eq!(
            case("absent", Path::new(""), ImportResolution::Unresolved),
            Some(SolidUse::Unknown)
        );
        // A relative specifier that lands in a file the analysis does not
        // read asks nothing of it; one that walks into a package is that
        // package.
        assert_eq!(
            case(
                "./styles.css",
                &root.join("styles.css"),
                ImportResolution::Relative
            ),
            None
        );
        assert_eq!(
            case(
                "./node_modules/wraps/index.js",
                &modules.join("wraps/index.d.ts"),
                ImportResolution::Relative
            ),
            Some(SolidUse::RuntimeDependency)
        );
        // A `paths` alias that the compiler resolved into a project file is
        // the project's own code, which the census reads directly.
        let ast = solid_facts::ast::extract("/p/index.ts", "import x from \"@app/x\";").unwrap();
        let other = solid_facts::ast::extract("/p/x.ts", "export default 1;").unwrap();
        let mut index = AttestedImportIndex::default();
        index.insert_file(
            "/p/index.ts",
            vec![row(
                "@app/x",
                Span::new(14, 22),
                Path::new("/p/x.ts"),
                ImportResolution::NonRelative,
            )],
        );
        assert_eq!(
            solid_use(
                RUNTIME,
                [("/p/index.ts", &ast), ("/p/x.ts", &other)],
                Some(&index)
            ),
            None
        );
        // An alias the compiler resolved into a runtime package is a use.
        let mut into_runtime = row(
            "@app/x",
            Span::new(14, 22),
            &modules.join("solid-js/types/index.d.ts"),
            ImportResolution::NonRelative,
        );
        into_runtime.package_name = Some("solid-js".into());
        let mut index = AttestedImportIndex::default();
        index.insert_file("/p/index.ts", vec![into_runtime]);
        assert_eq!(
            solid_use(RUNTIME, [("/p/index.ts", &ast)], Some(&index)).map(|(_, _, kind)| kind),
            Some(SolidUse::RuntimeModule)
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}
