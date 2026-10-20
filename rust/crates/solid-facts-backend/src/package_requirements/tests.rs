use std::{
    cell::RefCell,
    collections::BTreeMap,
    sync::atomic::{AtomicUsize, Ordering},
};

use solid_facts::{
    AttestedImport, AttestedImportIndex, FileFacts, ImportResolution, ProjectFacts,
    TypeScriptTable,
    compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap},
    core::{Generation, Span},
};

use super::*;
use crate::dependency_sources::SourceRefusal;

// characterization:begin
//
// These tests pin the public wrapper against real unique temporary trees.
// Every tree has a named root manifest, so an ownership walk always ends
// inside it and never reads a host ancestor.

/// A real temporary project tree and the facts that import from it.
struct Fixture {
    root: PathBuf,
    written: Vec<(PathBuf, Vec<u8>)>,
    absent: Vec<PathBuf>,
    rows: Vec<(String, AttestedImport)>,
    sources: Vec<(String, String)>,
    source_roots: Vec<String>,
    refusals: Vec<SourceRefusal>,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "solid-checker-package-requirements-{}-{}-{tag}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let mut fixture = Self {
            root,
            written: Vec::new(),
            absent: Vec::new(),
            rows: Vec::new(),
            sources: Vec::new(),
            source_roots: Vec::new(),
            refusals: Vec::new(),
        };
        fixture.file("package.json", r#"{"name":"fixture-root"}"#);
        fixture.absent("src/package.json");
        fixture
    }

    fn path(&self, rel: &str) -> String {
        self.root.join(rel).to_string_lossy().into_owned()
    }

    fn file(&mut self, rel: &str, contents: &str) {
        let path = self.root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents).unwrap();
        self.written.push((path, contents.as_bytes().to_vec()));
    }

    /// A path the owner queries that holds nothing on disk. The fake
    /// reader must be told so; it never infers absence.
    fn absent(&mut self, rel: &str) {
        self.absent.push(self.root.join(rel));
    }

    /// One import row of `importer`, resolved to `resolved` (empty for
    /// none), with `manifest` as its nearest installed manifest.
    fn import(
        &mut self,
        importer: &str,
        text: &str,
        resolution: ImportResolution,
        resolved: &str,
        package: Option<&str>,
        manifest: Option<&str>,
    ) -> usize {
        let start = u32::try_from(self.rows.len()).unwrap() * 10;
        let row = AttestedImport {
            span: Span::new(start, start + 5),
            text: text.into(),
            resolution,
            resolved_path: if resolved.is_empty() {
                "".into()
            } else {
                self.path(resolved).into()
            },
            included_path: "".into(),
            symlink_path: "".into(),
            extension: ".js".into(),
            package_name: package.map(Into::into),
            package_version: None,
            package_manifest: manifest.map(|rel| self.path(rel).into()),
            resolver_package_name: None,
            resolver_package_version: None,
        };
        self.rows.push((self.path(importer), row));
        self.rows.len() - 1
    }

    fn source(&mut self, importer: &str, source: &str) {
        self.sources.push((self.path(importer), source.to_owned()));
    }

    fn facts(&self) -> ProjectFacts {
        let mut by_importer: BTreeMap<&str, Vec<AttestedImport>> = BTreeMap::new();
        for (importer, row) in &self.rows {
            by_importer
                .entry(importer.as_str())
                .or_default()
                .push(row.clone());
        }
        let mut index = AttestedImportIndex::default();
        for (importer, rows) in by_importer {
            index.insert_file(importer, rows);
        }
        let files = self
            .sources
            .iter()
            .map(|(path, source)| {
                let ast = solid_facts::ast::extract(path.as_str(), source).unwrap();
                let compiler = ExecutionMap {
                    compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
                    source_hash: ast.source.hash.clone(),
                    semantic_model: Default::default(),
                    tracked_regions: vec![],
                    untracked_regions: vec![],
                    discarded_regions: vec![],
                    ownership_regions: vec![],
                    callback_roles: vec![],
                    jsx_operations: vec![],
                };
                FileFacts::new(Generation::new(1).unwrap(), source.as_str(), ast, compiler).unwrap()
            })
            .collect();
        let project_id = self.path("tsconfig.json");
        ProjectFacts {
            generation: Generation::new(1).unwrap(),
            project_id: project_id.clone(),
            files,
            typescript: TypeScriptTable::from_parts(
                3,
                1,
                project_id,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
            typescript_changes: None,
            resolved_imports: Some(index),
            runtime_resolutions: None,
            runtime_symbol_redirects: Default::default(),
            dependency_source_roots: self.source_roots.clone(),
        }
    }

    fn boundary(&self) -> PackageBoundary {
        external_package_boundary(&self.facts(), &self.refusals)
    }

    fn importer(&self, row: usize) -> &str {
        &self.rows[row].0
    }

    fn text(&self, row: usize) -> &str {
        &self.rows[row].1.text
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn kinds(boundary: &PackageBoundary, fixture: &Fixture, row: usize) -> Vec<UnanalyzableReason> {
    boundary
        .import(fixture.importer(row), fixture.text(row))
        .map(|import| import.reasons().iter().map(|reason| reason.kind).collect())
        .unwrap_or_default()
}

fn marker(boundary: &PackageBoundary, fixture: &Fixture, row: usize) -> bool {
    boundary.requires_legacy(fixture.importer(row), fixture.text(row))
}

/// The import raised the legacy requirement marker and its explanation.
fn kept(boundary: &PackageBoundary, fixture: &Fixture, row: usize) -> bool {
    marker(boundary, fixture, row)
        && kinds(boundary, fixture, row).contains(&UnanalyzableReason::LegacyRequirementFallback)
}

/// The import left no trace in the boundary at all.
fn untouched(boundary: &PackageBoundary, fixture: &Fixture, row: usize) -> bool {
    !marker(boundary, fixture, row)
        && boundary
            .import(fixture.importer(row), fixture.text(row))
            .is_none()
}

const SOLID_DEPENDENCY: &str = r#"{"name":"pkg","dependencies":{"solid-js":"^2"}}"#;

/// Relative edges between files of one installation. Each row would raise
/// a marker through its installed manifest unless the edge is local.
fn local_edges() -> (Fixture, Vec<(usize, bool)>) {
    let mut f = Fixture::new("local-edges");
    f.file("node_modules/pkg/package.json", SOLID_DEPENDENCY);
    f.file("node_modules/pkg/esm/package.json", r#"{"type":"module"}"#);
    f.file("node_modules/pkg/bad/package.json", "{not json");
    f.file("node_modules/pkg/array/package.json", "[1]");
    f.file("node_modules/pkg/numeric/package.json", r#"{"name":5}"#);
    f.file(
        "node_modules/pkg/vendor/package.json",
        r#"{"name":"vendored"}"#,
    );
    f.file("node_modules/other/package.json", r#"{"name":"other"}"#);
    f.absent("node_modules/pkg/dist/package.json");
    let manifest = "node_modules/pkg/package.json";
    let row = |f: &mut Fixture, importer: &str, text: &str, resolved: &str, local| {
        (
            f.import(
                importer,
                text,
                ImportResolution::NodeModules,
                resolved,
                Some("pkg"),
                Some(manifest),
            ),
            local,
        )
    };
    let rows = vec![
        row(
            &mut f,
            "node_modules/pkg/dist/a.js",
            "./b.js",
            "node_modules/pkg/dist/b.js",
            true,
        ),
        row(
            &mut f,
            "node_modules/pkg/esm/a.js",
            "../dist/b.js",
            "node_modules/pkg/dist/b.js",
            true,
        ),
        row(
            &mut f,
            "node_modules/pkg/bad/a.js",
            "../dist/b.js",
            "node_modules/pkg/dist/b.js",
            true,
        ),
        row(
            &mut f,
            "node_modules/pkg/array/a.js",
            "../dist/b.js",
            "node_modules/pkg/dist/b.js",
            true,
        ),
        row(
            &mut f,
            "node_modules/pkg/numeric/a.js",
            "../dist/b.js",
            "node_modules/pkg/dist/b.js",
            true,
        ),
        row(
            &mut f,
            "node_modules/pkg/dist/a.js",
            "../vendor/c.js",
            "node_modules/pkg/vendor/c.js",
            false,
        ),
        row(
            &mut f,
            "node_modules/pkg/dist/a.js",
            "../../other/x.js",
            "node_modules/other/x.js",
            false,
        ),
        row(
            &mut f,
            "node_modules/pkg/dist/a.js",
            "pkg/dist/b.js",
            "node_modules/pkg/dist/b.js",
            false,
        ),
        row(
            &mut f,
            "node_modules/pkg/dist/a.js",
            "./empty.js",
            "",
            false,
        ),
    ];
    (f, rows)
}

#[test]
fn a_relative_edge_inside_one_installation_raises_nothing() {
    let (f, rows) = local_edges();
    let boundary = f.boundary();
    for (row, local) in rows {
        if local {
            assert!(untouched(&boundary, &f, row), "{}", f.text(row));
        } else {
            assert!(kept(&boundary, &f, row), "{}", f.text(row));
        }
    }
}

/// Package identity comes from the resolver's own name first; the
/// primitive-defining packages raise nothing.
fn primitive_precedence() -> (Fixture, Vec<(usize, bool)>) {
    let mut f = Fixture::new("primitive-precedence");
    f.file("node_modules/dep/package.json", SOLID_DEPENDENCY);
    let manifest = "node_modules/dep/package.json";
    let importer = "src/main.ts";
    let add = |f: &mut Fixture,
               text: &str,
               resolution,
               package: Option<&str>,
               resolver: Option<&str>,
               kept| {
        let row = f.import(
            importer,
            text,
            resolution,
            "node_modules/dep/index.js",
            package,
            Some(manifest),
        );
        f.rows[row].1.resolver_package_name = resolver.map(Into::into);
        (row, kept)
    };
    let nm = ImportResolution::NodeModules;
    let rows = vec![
        add(&mut f, "a", nm, Some("lib"), Some("solid-js"), false),
        add(&mut f, "b", nm, Some("solid-js"), Some("lib"), true),
        add(&mut f, "c", nm, Some("@solidjs/web"), None, false),
        add(&mut f, "d", nm, Some("@solidjs/router"), None, true),
        add(&mut f, "e", nm, None, None, false),
        add(
            &mut f,
            "f",
            ImportResolution::NonRelative,
            Some("lib"),
            None,
            false,
        ),
    ];
    (f, rows)
}

#[test]
fn primitive_packages_are_skipped_before_anything_else() {
    let (f, rows) = primitive_precedence();
    let boundary = f.boundary();
    for (row, expect_kept) in rows {
        if expect_kept {
            assert!(kept(&boundary, &f, row), "{}", f.text(row));
        } else {
            assert!(untouched(&boundary, &f, row), "{}", f.text(row));
        }
    }
}

/// One row per installed-manifest shape, with and without source refusals.
fn manifest_fields(populated: bool) -> (Fixture, Vec<(usize, bool)>) {
    let mut f = Fixture::new(if populated {
        "manifest-fields-populated"
    } else {
        "manifest-fields"
    });
    let manifests = [
        ("m-deps", r#"{"dependencies":{"solid-js":"2"}}"#, true),
        (
            "m-peer",
            r#"{"peerDependencies":{"@solidjs/web":"2"}}"#,
            true,
        ),
        (
            "m-optional",
            r#"{"optionalDependencies":{"@solidjs/signals":"2"}}"#,
            true,
        ),
        (
            "m-scope",
            r#"{"dependencies":{"@solidjs/router":"1"}}"#,
            true,
        ),
        ("m-dev", r#"{"devDependencies":{"solid-js":"2"}}"#, false),
        ("m-other", r#"{"dependencies":{"left-pad":"1"}}"#, false),
        ("m-bad", "{", false),
        ("m-array", "[]", false),
    ];
    let mut rows = Vec::new();
    for (name, manifest, expect_kept) in manifests {
        f.file(&format!("node_modules/{name}/package.json"), manifest);
        rows.push((
            f.import(
                "src/main.ts",
                name,
                ImportResolution::NodeModules,
                &format!("node_modules/{name}/index.js"),
                Some(name),
                Some(&format!("node_modules/{name}/package.json")),
            ),
            expect_kept,
        ));
    }
    f.absent("node_modules/m-missing/package.json");
    rows.push((
        f.import(
            "src/main.ts",
            "m-missing",
            ImportResolution::NodeModules,
            "node_modules/m-missing/index.js",
            Some("m-missing"),
            Some("node_modules/m-missing/package.json"),
        ),
        false,
    ));
    rows.push((
        f.import(
            "src/main.ts",
            "m-unmanifested",
            ImportResolution::NodeModules,
            "node_modules/m-unmanifested/index.js",
            Some("m-unmanifested"),
            None,
        ),
        false,
    ));
    if populated {
        f.refusals = vec![
            SourceRefusal {
                name: "m-deps".into(),
                root: None,
                kind: UnanalyzableReason::ManifestUnavailable,
                detail: "by name".into(),
            },
            SourceRefusal {
                name: "other".into(),
                root: Some(f.root.join("node_modules/m-peer")),
                kind: UnanalyzableReason::ClosureUnavailable,
                detail: "by root".into(),
            },
            SourceRefusal {
                name: "m-optional".into(),
                root: Some(f.root.join("node_modules/elsewhere")),
                kind: UnanalyzableReason::ClosureLimit,
                detail: "another copy".into(),
            },
        ];
    }
    (f, rows)
}

#[test]
fn an_installed_manifest_naming_solid_raises_the_marker() {
    for populated in [false, true] {
        let (f, rows) = manifest_fields(populated);
        let boundary = f.boundary();
        for &(row, expect_kept) in &rows {
            if expect_kept {
                assert!(kept(&boundary, &f, row), "{}", f.text(row));
            } else {
                assert!(untouched(&boundary, &f, row), "{}", f.text(row));
            }
        }
        let has = |row: usize, kind| kinds(&boundary, &f, row).contains(&kind);
        assert_eq!(
            has(0, UnanalyzableReason::ManifestUnavailable),
            populated,
            "a refusal about the package name joins its import"
        );
        assert_eq!(
            has(1, UnanalyzableReason::ClosureUnavailable),
            populated,
            "a refusal about the installed root joins imports under it"
        );
        assert!(
            !has(2, UnanalyzableReason::ClosureLimit),
            "a refusal about another copy joins nothing"
        );
    }
}

/// A workspace package linked into `node_modules` resolves, by realpath,
/// to the project's own directory. When the program holds that file it is
/// project code and raises nothing; a target the program does not hold,
/// and an installed copy, keep the marker.
#[test]
fn a_workspace_link_to_a_program_file_is_project_code() {
    let mut f = Fixture::new("workspace-link");
    f.file("packages/ui/package.json", SOLID_DEPENDENCY);
    f.file("node_modules/pkg/package.json", SOLID_DEPENDENCY);
    f.source("packages/ui/src/utils.ts", "export const x = 1;");
    let nm = ImportResolution::NodeModules;
    let held = f.import(
        "src/main.ts",
        "pkg/utils",
        nm,
        "packages/ui/src/utils.ts",
        Some("pkg"),
        Some("packages/ui/package.json"),
    );
    let unheld = f.import(
        "src/main.ts",
        "pkg/other",
        nm,
        "packages/ui/src/other.ts",
        Some("pkg"),
        Some("packages/ui/package.json"),
    );
    let installed = f.import(
        "src/main.ts",
        "pkg/dist",
        nm,
        "node_modules/pkg/index.js",
        Some("pkg"),
        Some("node_modules/pkg/package.json"),
    );
    let boundary = f.boundary();
    assert!(untouched(&boundary, &f, held));
    assert!(kept(&boundary, &f, unheld));
    assert!(kept(&boundary, &f, installed));
}

/// A Vite configuration nothing imports is a build-time Node file. The same
/// import in any other file, or in a configuration the program imports,
/// keeps the marker.
#[test]
fn a_vite_config_nothing_imports_raises_nothing() {
    let mut f = Fixture::new("vite-config");
    f.file("node_modules/pkg/package.json", SOLID_DEPENDENCY);
    let nm = ImportResolution::NodeModules;
    let row = |f: &mut Fixture, importer: &str| {
        f.import(
            importer,
            "pkg",
            nm,
            "node_modules/pkg/index.js",
            Some("pkg"),
            Some("node_modules/pkg/package.json"),
        )
    };
    let config = row(&mut f, "vite.config.ts");
    let vitest = row(&mut f, "tests/vitest.config.mts");
    let app = row(&mut f, "src/main.ts");
    let lookalike = row(&mut f, "src/vite.config.helper.ts");
    let imported = row(&mut f, "web/vite.config.js");
    f.import(
        "src/entry.ts",
        "../web/vite.config.js",
        ImportResolution::Relative,
        "web/vite.config.js",
        None,
        None,
    );
    let boundary = f.boundary();
    assert!(untouched(&boundary, &f, config));
    assert!(untouched(&boundary, &f, vitest));
    assert!(kept(&boundary, &f, app));
    assert!(kept(&boundary, &f, lookalike));
    assert!(kept(&boundary, &f, imported));
}

#[test]
fn the_boundary_markers_are_exactly_the_kept_requirements() {
    let (f, rows) = manifest_fields(true);
    let boundary = f.boundary();
    for (row, expect_kept) in rows {
        assert_eq!(
            boundary.requires_legacy(f.importer(row), f.text(row)),
            expect_kept
        );
    }
}

/// Imports of packages analyzed as source: only a runtime namespace
/// import keeps an obligation, and the checks after it still run.
fn source_roots() -> (Fixture, Vec<usize>) {
    let mut f = Fixture::new("source-roots");
    let importers = [
        ("srcpkg-ns", "import * as ns from \"srcpkg-ns\";\n", false),
        (
            "srcpkg-ns-solid",
            "import * as ns from \"srcpkg-ns-solid\";\n",
            true,
        ),
        (
            "srcpkg-named",
            "import { a } from \"srcpkg-named\";\n",
            true,
        ),
        (
            "srcpkg-type",
            "import type * as ns from \"srcpkg-type\";\n",
            true,
        ),
    ];
    let mut rows = Vec::new();
    for (name, source, solid) in importers {
        let importer = format!("src/{name}.ts");
        f.source(&importer, source);
        f.file(
            &format!("node_modules/{name}/package.json"),
            if solid {
                SOLID_DEPENDENCY
            } else {
                r#"{"name":"quiet"}"#
            },
        );
        f.source_roots.push(f.path(&format!("node_modules/{name}")));
        rows.push(f.import(
            &importer,
            name,
            ImportResolution::NodeModules,
            &format!("node_modules/{name}/index.js"),
            Some(name),
            Some(&format!("node_modules/{name}/package.json")),
        ));
    }
    (f, rows)
}

#[test]
fn selected_source_is_redirected_except_a_runtime_namespace_import() {
    let (f, rows) = source_roots();
    let boundary = f.boundary();
    let namespace = UnanalyzableReason::UnsupportedBinding(BindingForm::NamespaceValue);
    // A quiet manifest: the namespace explanation alone, with no marker.
    assert_eq!(kinds(&boundary, &f, rows[0]), vec![namespace]);
    assert!(!marker(&boundary, &f, rows[0]));
    // A solid-naming manifest: the explanation and the later marker.
    assert!(kinds(&boundary, &f, rows[1]).contains(&namespace));
    assert!(kept(&boundary, &f, rows[1]));
    // Named and type-only namespace imports are redirected: no trace.
    assert!(untouched(&boundary, &f, rows[2]));
    assert!(untouched(&boundary, &f, rows[3]));
}

/// Every scenario of this section, for the differential below.
fn scenarios() -> Vec<(&'static str, Fixture)> {
    vec![
        ("local-edges", local_edges().0),
        ("primitive-precedence", primitive_precedence().0),
        ("manifest-fields", manifest_fields(false).0),
        ("manifest-fields-populated", manifest_fields(true).0),
        ("source-roots", source_roots().0),
    ]
}
// characterization:end

// fake-reader:begin
//
// An in-memory reader answers exactly the lookups the discovery makes. The
// discovery folds every read error into "no manifest", so an `Err` could
// not tell a test that a lookup was never tabled; a queried path outside
// the table therefore panics. That is a test harness fault, not a
// production policy for missing input, and there is no disk fallback.

enum Read {
    Bytes(Vec<u8>),
    Absent,
    Unreadable,
}

#[derive(Default)]
struct FakeReader {
    reads: BTreeMap<PathBuf, Read>,
    read_queries: RefCell<BTreeMap<PathBuf, usize>>,
}

impl FakeReader {
    fn read(mut self, path: &str, entry: Read) -> Self {
        self.reads.insert(PathBuf::from(path), entry);
        self
    }

    fn bytes(self, path: &str, contents: &str) -> Self {
        self.read(path, Read::Bytes(contents.as_bytes().to_vec()))
    }

    /// The table a fixture's real tree would answer from.
    fn of_tree(fixture: &Fixture) -> Self {
        let mut reader = Self::default();
        for (path, contents) in &fixture.written {
            reader
                .reads
                .insert(path.clone(), Read::Bytes(contents.clone()));
        }
        for path in &fixture.absent {
            reader.reads.insert(path.clone(), Read::Absent);
        }
        reader
    }

    fn queried_reads(&self) -> BTreeMap<PathBuf, usize> {
        self.read_queries.borrow().clone()
    }
}

impl ManifestReader for FakeReader {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        *self
            .read_queries
            .borrow_mut()
            .entry(path.to_path_buf())
            .or_default() += 1;
        match self.reads.get(path) {
            Some(Read::Bytes(bytes)) => Ok(bytes.clone()),
            Some(Read::Absent) => Err(io::ErrorKind::NotFound.into()),
            Some(Read::Unreadable) => Err(io::ErrorKind::PermissionDenied.into()),
            None => panic!("test reader: no table entry for read of {}", path.display()),
        }
    }
}

fn virtual_row(
    text: &str,
    resolution: ImportResolution,
    resolved: &str,
    package: Option<&str>,
    manifest: Option<&str>,
) -> AttestedImport {
    AttestedImport {
        span: Span::new(0, 5),
        text: text.into(),
        resolution,
        resolved_path: resolved.into(),
        included_path: "".into(),
        symlink_path: "".into(),
        extension: ".js".into(),
        package_name: package.map(Into::into),
        package_version: None,
        package_manifest: manifest.map(Into::into),
        resolver_package_name: None,
        resolver_package_version: None,
    }
}

/// Facts whose rows live at virtual absolute paths, for controls that
/// must not touch the disk (no named owner, the filesystem root).
fn virtual_facts(rows: Vec<(&str, AttestedImport)>) -> ProjectFacts {
    let mut by_importer: BTreeMap<&str, Vec<AttestedImport>> = BTreeMap::new();
    for (position, (importer, mut row)) in rows.into_iter().enumerate() {
        let start = u32::try_from(position).unwrap() * 10;
        row.span = Span::new(start, start + 5);
        by_importer.entry(importer).or_default().push(row);
    }
    let mut index = AttestedImportIndex::default();
    for (importer, rows) in by_importer {
        index.insert_file(importer, rows);
    }
    ProjectFacts {
        generation: Generation::new(1).unwrap(),
        project_id: "/virtual/tsconfig.json".into(),
        files: Vec::new(),
        typescript: TypeScriptTable::from_parts(
            3,
            1,
            "/virtual/tsconfig.json",
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        typescript_changes: None,
        resolved_imports: Some(index),
        runtime_resolutions: None,
        runtime_symbol_redirects: Default::default(),
        dependency_source_roots: Vec::new(),
    }
}

fn boundary_of(reader: &FakeReader, rows: Vec<(&str, AttestedImport)>) -> PackageBoundary {
    external_package_boundary_with(reader, &virtual_facts(rows), &[])
}

/// Each queried path and how often, as `path=count`. Path sets and counts
/// only: the discovery's query order is not defined.
fn counts(reader: &FakeReader) -> Vec<String> {
    reader
        .queried_reads()
        .into_iter()
        .map(|(path, count)| format!("{}={count}", path.display()))
        .collect()
}

fn named(name: &str) -> String {
    format!(r#"{{"name":"{name}"}}"#)
}

#[test]
fn the_disk_reader_and_a_table_of_the_same_tree_agree_on_every_scenario() {
    for (name, fixture) in scenarios() {
        let reader = FakeReader::of_tree(&fixture);
        let fake = external_package_boundary_with(&reader, &fixture.facts(), &fixture.refusals);
        let disk = fixture.boundary();
        assert!(!disk.is_empty(), "{name} must exercise the discovery");
        assert_eq!(disk, fake, "{name}");
    }
}

#[test]
fn an_importer_without_a_named_owner_never_walks_the_target() {
    let reader = FakeReader::default()
        .read("/a/package.json", Read::Absent)
        .read("/package.json", Read::Absent)
        .bytes("/b/package.json", &named("b"));
    let boundary = boundary_of(
        &reader,
        vec![(
            "/a/i.ts",
            virtual_row(
                "../b/t.ts",
                ImportResolution::Relative,
                "/b/t.ts",
                None,
                None,
            ),
        )],
    );
    assert!(boundary.is_empty());
    assert_eq!(counts(&reader), ["/a/package.json=1", "/package.json=1"]);
}

#[test]
fn the_filesystem_root_manifest_is_queried_and_can_own() {
    let row = || {
        vec![(
            "/x/i.ts",
            virtual_row(
                "./j.ts",
                ImportResolution::NodeModules,
                "/x/j.ts",
                Some("pkg"),
                Some("/m/package.json"),
            ),
        )]
    };
    let solid = r#"{"dependencies":{"solid-js":"2"}}"#;
    // A named root owns both files: the edge is local and the installed
    // manifest is never read.
    let reader = FakeReader::default()
        .read("/x/package.json", Read::Absent)
        .bytes("/package.json", &named("root"))
        .bytes("/m/package.json", solid);
    assert!(boundary_of(&reader, row()).is_empty());
    assert_eq!(counts(&reader), ["/package.json=1", "/x/package.json=1"]);
    // An unnamed root owns nothing: the edge is not local and the
    // installed manifest is read.
    let reader = FakeReader::default()
        .read("/x/package.json", Read::Absent)
        .bytes("/package.json", "{}")
        .bytes("/m/package.json", solid);
    let boundary = boundary_of(&reader, row());
    assert!(boundary.requires_legacy("/x/i.ts", "./j.ts"));
    assert_eq!(
        counts(&reader),
        ["/m/package.json=1", "/package.json=1", "/x/package.json=1"]
    );
}

#[test]
fn owner_lookups_are_memoized_per_directory_not_per_import() {
    let reader = FakeReader::default()
        .read("/x/package.json", Read::Absent)
        .read("/y/package.json", Read::Absent)
        .bytes("/package.json", &named("root"));
    let relative = |text: &str, resolved: &str| {
        (
            "/x/i.ts",
            virtual_row(text, ImportResolution::Relative, resolved, None, None),
        )
    };
    boundary_of(
        &reader,
        vec![
            relative("./a.ts", "/x/a.ts"),
            relative("./b.ts", "/x/b.ts"),
            relative("../y/c.ts", "/y/c.ts"),
        ],
    );
    assert_eq!(
        counts(&reader),
        ["/package.json=1", "/x/package.json=1", "/y/package.json=1"]
    );
}

#[test]
fn a_primitive_package_is_skipped_before_any_lookup() {
    let reader = FakeReader::default();
    let boundary = boundary_of(
        &reader,
        vec![(
            "/x/i.ts",
            virtual_row(
                "./a.ts",
                ImportResolution::NodeModules,
                "/x/a.ts",
                Some("solid-js"),
                Some("/m/package.json"),
            ),
        )],
    );
    assert!(boundary.is_empty());
    assert!(reader.queried_reads().is_empty());
}

#[test]
fn an_unreadable_manifest_is_folded_like_an_absent_one() {
    let solid = r#"{"dependencies":{"solid-js":"2"}}"#;
    // Unreadable importer directory: the walk continues to the root.
    let run = |importer_dir: Read, manifest: Read| {
        let reader = FakeReader::default()
            .read("/x/package.json", importer_dir)
            .bytes("/package.json", &named("root"))
            .bytes("/n/package.json", &named("n"))
            .read("/m/package.json", manifest);
        boundary_of(
            &reader,
            vec![(
                "/x/i.ts",
                virtual_row(
                    "../n/j.ts",
                    ImportResolution::NodeModules,
                    "/n/j.ts",
                    Some("pkg"),
                    Some("/m/package.json"),
                ),
            )],
        )
    };
    let solid_manifest = || Read::Bytes(solid.as_bytes().to_vec());
    let absent = run(Read::Absent, solid_manifest());
    assert!(!absent.is_empty());
    assert_eq!(run(Read::Unreadable, solid_manifest()), absent);
    // An unreadable installed manifest raises nothing, as an absent one.
    let quiet = run(Read::Absent, Read::Absent);
    assert!(quiet.is_empty());
    assert_eq!(run(Read::Absent, Read::Unreadable), quiet);
}

#[test]
#[should_panic(expected = "test reader: no table entry for read of /x/package.json")]
fn a_read_outside_the_table_is_a_harness_fault() {
    boundary_of(
        &FakeReader::default(),
        vec![(
            "/x/i.ts",
            virtual_row("./a.ts", ImportResolution::Relative, "/x/a.ts", None, None),
        )],
    );
}

// fake-reader:end
