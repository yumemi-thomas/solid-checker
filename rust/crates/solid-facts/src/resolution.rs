//! Attested module-resolution facts: which installed package each import
//! specifier actually resolves to, as the compiler resolved it.
//!
//! Oxc sees the specifier *text*; nothing in the syntax facts says where that
//! text resolves. A package contract describes one installed package, so
//! applying it to an import needs the resolution, not the name — a tsconfig
//! `paths` entry, a `baseUrl` mapping, or a project-local reimplementation can
//! own a bare specifier while a package of that same name is installed beside
//! it. This table carries the compiler's own answer for that question and
//! nothing derived from path shape.
//!
//! The table is deliberately optional on [`crate::ProjectFacts`]. An analysis
//! that carries it binds contracts by installed identity; one that does not
//! (an adapter with no Type Facts session of its own) keeps the older
//! name-matched behavior, which is a documented limitation of that adapter and
//! not a silent upgrade. Absence of the *whole table* and absence of *one row*
//! are different facts and are answered differently: see
//! [`AttestedImportIndex::specifier`].

use crate::core::Span;
use compact_str::CompactString;
use std::collections::HashMap;
use std::sync::Arc;

/// What the compiler's resolver recorded about the shape of one resolution.
///
/// Every variant mirrors `typefacts::ModuleResolution`, which reads them off
/// the compiler's own `ResolvedModule`. None is inferred from a path.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ImportResolution {
    /// The program holds no resolution for this specifier. The only variant
    /// with an empty [`AttestedImport::resolved_path`].
    #[default]
    Unresolved,
    /// A relative or rooted specifier: no package lookup participated.
    Relative,
    /// The resolver landed inside a `node_modules` tree.
    NodeModules,
    /// A bare specifier that resolved outside every `node_modules` tree — a
    /// `paths` or `baseUrl` mapping, a package self-name, or a
    /// project-reference redirect. `ResolvedModule` does not record which, so
    /// this variant never claims one.
    NonRelative,
}

/// One import specifier's resolution, as the compiler resolved it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttestedImport {
    /// The specifier string literal's own span, quotes included.
    pub span: Span,
    /// The specifier as written, after string-literal unescaping.
    pub text: CompactString,
    pub resolution: ImportResolution,
    /// The file the resolver selected, realpath-normalized where resolution
    /// walked a symlink. Empty exactly when `resolution` is
    /// [`ImportResolution::Unresolved`].
    pub resolved_path: Arc<str>,
    /// The file the compiler actually included when it redirected the
    /// resolver-selected file (for example a project-reference declaration
    /// output) to its source input. Empty when no redirect occurred.
    pub included_path: Arc<str>,
    /// The pre-realpath spelling recorded by the resolver for a symlinked
    /// package target. Empty when resolution observed no symlink divergence.
    pub symlink_path: Arc<str>,
    /// Exact resolver extension (`.js`, `.d.ts`, and so on).
    pub extension: Arc<str>,
    /// The `name` of the nearest `package.json` above `resolved_path`, and
    /// `None` when there is no such manifest or it declares no name. An
    /// unnamed nested manifest — the `{"type":"module"}` one a published
    /// package ships beside its ESM output — is routine, so an absent name is
    /// never read as a disagreement.
    pub package_name: Option<CompactString>,
    /// Version from the same nearest owning manifest as `package_name`.
    pub package_version: Option<CompactString>,
    /// The nearest `package.json` above `resolved_path`, and `None` when there
    /// is none.
    pub package_manifest: Option<Arc<str>>,
    /// The package name the *resolver itself* recorded while resolving this
    /// specifier. It is a different fact from `package_name` and the two can
    /// disagree: this one names the package whose manifest the resolution
    /// consulted, which for a subpath export or a nested install is not always
    /// the nearest manifest above the selected file. A consumer comparing a
    /// contract against a package must say which of the two it means — see
    /// `solid_reactive_ir::contracts`, which says so at the comparison.
    pub resolver_package_name: Option<CompactString>,
    /// Version recorded by the resolver's own package identity.
    pub resolver_package_version: Option<CompactString>,
}

/// The attested resolution of every import specifier in the files the answer
/// covered.
///
/// Rows are grouped per importing file and kept sorted by specifier start
/// byte, which is how [`Self::specifier`] joins one row to one syntax fact.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AttestedImportIndex {
    by_file: HashMap<Arc<str>, Vec<AttestedImport>>,
}

/// The answer to "may a contract for this package be applied to this
/// specifier?" before the package identity is compared.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpecifierAttestation<'a> {
    /// The file's resolutions were attested and exactly one row is this
    /// specifier.
    Attested(&'a AttestedImport),
    /// The answer covered this file and no row is this specifier, the answer
    /// did not cover this file at all, or more than one row could be it. All
    /// three are the same thing to a consumer: the resolution of this
    /// specifier is unknown, so nothing may be certified from it.
    Unattested,
}

impl AttestedImportIndex {
    /// Records one importing file's rows. `imports` need not be sorted.
    ///
    /// Recording a file with no imports is meaningful and must not be skipped:
    /// it is how "this file imports nothing" is told apart from "this file was
    /// never answered for".
    pub fn insert_file(&mut self, path: impl Into<Arc<str>>, mut imports: Vec<AttestedImport>) {
        imports.sort_by_key(|import| (import.span.start, import.span.end));
        self.by_file.insert(path.into(), imports);
    }

    /// The number of importing files the answer covered.
    #[must_use]
    pub fn files(&self) -> usize {
        self.by_file.len()
    }

    /// The number of attested specifiers.
    #[must_use]
    pub fn specifiers(&self) -> usize {
        self.by_file.values().map(Vec::len).sum()
    }

    /// Every attested row with its exact importing-file identity. This is the
    /// host/Type Facts adapter input for artifact resolution; order is not
    /// semantic and callers that serialize it must canonicalize explicitly.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &AttestedImport)> {
        self.by_file
            .iter()
            .flat_map(|(path, imports)| imports.iter().map(move |import| (path.as_ref(), import)))
    }

    /// Whether this file's resolutions were answered at all.
    #[must_use]
    pub fn covers(&self, path: &str) -> bool {
        self.by_file.contains_key(path)
    }

    /// The row for the specifier of one import or export-from declaration.
    ///
    /// The producer emits one row per specifier occurrence and intends a
    /// consumer to join by exact span. The syntax facts carry the *declaration*
    /// span rather than the literal's own, so the join is the containment of
    /// one span in the other together with exact specifier text — a declaration
    /// holds exactly one module specifier, so the pair selects one row. Two
    /// candidate rows would mean that assumption is wrong somewhere, and the
    /// answer is then [`SpecifierAttestation::Unattested`] rather than a guess.
    #[must_use]
    pub fn specifier(&self, path: &str, declaration: Span, text: &str) -> SpecifierAttestation<'_> {
        let Some(rows) = self.by_file.get(path) else {
            return SpecifierAttestation::Unattested;
        };
        let first = rows.partition_point(|row| row.span.start < declaration.start);
        let mut found = None;
        for row in &rows[first..] {
            if row.span.start > declaration.end {
                break;
            }
            if !declaration.contains(row.span) || row.text != text {
                continue;
            }
            if found.is_some() {
                return SpecifierAttestation::Unattested;
            }
            found = Some(row);
        }
        found.map_or(
            SpecifierAttestation::Unattested,
            SpecifierAttestation::Attested,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn import(start: u32, end: u32, text: &str) -> AttestedImport {
        AttestedImport {
            span: Span::new(start, end),
            text: text.into(),
            resolution: ImportResolution::NodeModules,
            resolved_path: "/p/node_modules/pkg/index.d.ts".into(),
            included_path: "".into(),
            symlink_path: "".into(),
            extension: ".d.ts".into(),
            package_name: Some("pkg".into()),
            package_version: Some("1.0.0".into()),
            package_manifest: Some("/p/node_modules/pkg/package.json".into()),
            resolver_package_name: Some("pkg".into()),
            resolver_package_version: Some("1.0.0".into()),
        }
    }

    #[test]
    fn a_declaration_span_selects_its_own_specifier() {
        let mut index = AttestedImportIndex::default();
        index.insert_file(
            "/p/a.ts",
            vec![import(20, 25, "pkg"), import(46, 53, "other")],
        );
        assert!(matches!(
            index.specifier("/p/a.ts", Span::new(0, 26), "pkg"),
            SpecifierAttestation::Attested(row) if row.span == Span::new(20, 25)
        ));
        assert!(matches!(
            index.specifier("/p/a.ts", Span::new(27, 54), "other"),
            SpecifierAttestation::Attested(row) if row.span == Span::new(46, 53)
        ));
    }

    #[test]
    fn a_file_the_answer_never_covered_is_unattested() {
        let index = AttestedImportIndex::default();
        assert_eq!(
            index.specifier("/p/a.ts", Span::new(0, 26), "pkg"),
            SpecifierAttestation::Unattested
        );
        assert!(!index.covers("/p/a.ts"));
    }

    #[test]
    fn a_covered_file_with_no_row_for_the_specifier_is_unattested() {
        let mut index = AttestedImportIndex::default();
        index.insert_file("/p/a.ts", vec![]);
        assert!(index.covers("/p/a.ts"));
        assert_eq!(
            index.specifier("/p/a.ts", Span::new(0, 26), "pkg"),
            SpecifierAttestation::Unattested
        );
    }

    #[test]
    fn text_must_match_even_inside_the_declaration_span() {
        let mut index = AttestedImportIndex::default();
        index.insert_file("/p/a.ts", vec![import(20, 25, "pkg")]);
        assert_eq!(
            index.specifier("/p/a.ts", Span::new(0, 26), "pkg-other"),
            SpecifierAttestation::Unattested
        );
    }

    #[test]
    fn two_candidate_rows_refuse_rather_than_guess() {
        let mut index = AttestedImportIndex::default();
        index.insert_file(
            "/p/a.ts",
            vec![import(20, 25, "pkg"), import(30, 35, "pkg")],
        );
        assert_eq!(
            index.specifier("/p/a.ts", Span::new(0, 40), "pkg"),
            SpecifierAttestation::Unattested
        );
    }

    #[test]
    fn ordinary_analysis_preserves_every_resolver_identity_field() {
        let mut index = AttestedImportIndex::default();
        let mut row = import(20, 25, "pkg");
        row.included_path = "/p/packages/pkg/src/index.ts".into();
        row.symlink_path = "/p/node_modules/pkg/index.d.ts".into();
        index.insert_file("/p/a.ts", vec![row]);

        let rows = index.iter().collect::<Vec<_>>();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "/p/a.ts");
        assert_eq!(
            rows[0].1.included_path.as_ref(),
            "/p/packages/pkg/src/index.ts"
        );
        assert_eq!(
            rows[0].1.symlink_path.as_ref(),
            "/p/node_modules/pkg/index.d.ts"
        );
        assert_eq!(rows[0].1.extension.as_ref(), ".d.ts");
        assert_eq!(rows[0].1.package_version.as_deref(), Some("1.0.0"));
        assert_eq!(rows[0].1.resolver_package_version.as_deref(), Some("1.0.0"));
    }
}
