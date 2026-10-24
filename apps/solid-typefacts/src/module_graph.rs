//! The `modules` operation over a tsrs program (Go: `tsgo/module_graph.go`).
//!
//! Nothing here consults the file system or reasons about path shape: every
//! field is read off the program, a source file, or the resolver's
//! `ResolvedModule`, so an absent answer is the compiler's absence and not
//! this adapter's. The one exception is [`root_positions`], which repeats the
//! loader's root lookup (`fileloader.rs` `source_file_from_reference`) against
//! the compiler host's own file system, because tsrs keeps the root include
//! records private.

use crate::backend::{BackendError, ModuleInventory};
use crate::context::Context;
use crate::gocompat::{clean, dir};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tsrs_ast::SourceFile;
use tsrs_compiler::Program;
use tsrs_core::{ModuleKind, P, Pattern, PatternKind, try_parse_pattern, tspath};
use tsrs_scanner::skip_trivia;
use tsrs_tsoptions as tsoptions;
use typefacts::v3::ModuleGraphRequest;
use typefacts::{
    Location, ModuleFact, ModuleFormat, ModuleImportFact, ModuleResolution, PackageIdentity,
    ProjectReferenceMapping, ResolverPackageId,
};

/// Go: `moduleGraphLocked`.
pub fn module_graph(
    ctx: &Context,
    program: &Program,
    demand: &ModuleGraphRequest,
) -> Result<ModuleInventory, BackendError> {
    let files = program.source_files();
    let mut inventory = ModuleInventory {
        modules: Vec::with_capacity(files.len()),
        ..ModuleInventory::default()
    };
    let (roots, unmapped) = root_positions(program);
    inventory.unmapped_roots = unmapped;
    for file in files {
        check(ctx)?;
        let mut fact = module_fact(program, *file);
        fact.root = roots.get(&file.addr()).copied();
        inventory.modules.push(fact);
    }
    inventory
        .modules
        .sort_by(|left, right| left.path.cmp(&right.path));
    if !demand.imports {
        return Ok(inventory);
    }

    let (answered, unknown) = import_scope(files, &demand.import_paths);
    inventory.unknown_import_paths = unknown.into_iter().map(Arc::from).collect();
    let mut packages = demand.packages.then(|| PackageScopeCache::new(program));
    let patterns = PathsPatterns::of(program);
    for file in answered {
        check(ctx)?;
        if !demand.distinct_bare_imports {
            append_import_facts(
                &mut inventory.imports,
                program,
                file,
                &patterns,
                packages.as_mut(),
            )?;
            continue;
        }
        let mut facts = Vec::new();
        append_import_facts(&mut facts, program, file, &patterns, packages.as_mut())?;
        facts.sort_by_key(|fact| fact.specifier.start_byte);
        let mut seen = HashSet::new();
        inventory.imports.extend(facts.into_iter().filter(|fact| {
            !fact.text.starts_with('.')
                && !fact.text.starts_with('/')
                && seen.insert((
                    Arc::clone(&fact.text),
                    Arc::clone(&fact.resolved_path),
                    Arc::clone(&fact.included_path),
                    Arc::clone(&fact.symlink_path),
                ))
        }));
    }
    inventory.imports.sort_by(|left, right| {
        left.specifier
            .path
            .cmp(&right.specifier.path)
            .then(left.specifier.start_byte.cmp(&right.specifier.start_byte))
    });
    Ok(inventory)
}

fn check(ctx: &Context) -> Result<(), BackendError> {
    match ctx.err() {
        Some(reason) => Err(BackendError(reason.to_owned())),
        None => Ok(()),
    }
}

/// Go: `importScope`. An explicitly requested path the program does not hold
/// is reported, never silently dropped: a caller cannot otherwise distinguish
/// a file with no imports from a file that was never in the program.
fn import_scope(
    files: &'static [P<SourceFile>],
    requested: &[String],
) -> (Vec<P<SourceFile>>, Vec<String>) {
    if requested.is_empty() {
        return (files.to_vec(), Vec::new());
    }
    let by_path: HashMap<String, P<SourceFile>> = files
        .iter()
        .map(|file| (clean(file.file_name()), *file))
        .collect();
    let mut answered = Vec::with_capacity(requested.len());
    let mut unknown = Vec::new();
    let mut seen: HashSet<String> = HashSet::with_capacity(requested.len());
    for path in requested {
        let path = clean(path);
        if !seen.insert(path.clone()) {
            continue;
        }
        match by_path.get(&path) {
            Some(file) => answered.push(*file),
            None => unknown.push(path),
        }
    }
    unknown.sort();
    (answered, unknown)
}

/// Each configured root file's position in `ParsedCommandLine.fileNames`,
/// TypeScript's own root order, keyed by the program's source file, and the
/// configured roots the program holds no file for although the loader's
/// lookup finds one (review of ADR 0368).
///
/// A root is mapped as the loader maps it: a name with an extension is the
/// file (an unsupported extension loads nothing), an extensionless name is the
/// first `name + extension` of the first supported group that exists
/// (`files: ["src/main"]` loads `src/main.ts`), and a project-reference
/// redirect then replaces it (`get_source_file_for_resolved_module`). A root
/// whose lookup finds no file loaded none; it is no position and not
/// unmapped. A root that maps to a file the program does not hold is
/// unmapped: the caller cannot place it, and must not take path order for
/// root order. A file named twice keeps its first position, as the program
/// does.
fn root_positions(program: &Program) -> (HashMap<usize, u32>, Vec<Arc<str>>) {
    let command_line = program.command_line();
    let options = program.options();
    let host = program.host();
    let fs = host.fs();
    let supported = tsoptions::get_supported_extensions(
        Some(&options),
        &command_line.content_mapper_extensions(),
    );
    let supported_with_json = tsoptions::get_supported_extensions_with_json_if_resolve_json_module(
        Some(&options),
        &supported,
    );
    let allow_non_ts_extensions = options.allow_non_ts_extensions.is_true();
    let mut positions = HashMap::new();
    let mut unmapped = Vec::new();
    for (index, name) in command_line.file_names().iter().enumerate() {
        let absolute = tspath::get_normalized_absolute_path(name, host.get_current_directory());
        let looked_up = if tspath::has_extension(&absolute) {
            let canonical =
                tspath::get_canonical_file_name(&absolute, fs.use_case_sensitive_file_names());
            let supported_extension = supported_with_json.iter().any(|group| {
                let group = group.iter().map(String::as_str).collect::<Vec<_>>();
                tspath::file_extension_is_one_of(&canonical, &group)
            });
            ((allow_non_ts_extensions || supported_extension) && fs.file_exists(&absolute))
                .then_some(absolute)
        } else if allow_non_ts_extensions {
            fs.file_exists(&absolute).then_some(absolute)
        } else {
            supported.first().and_then(|group| {
                group
                    .iter()
                    .map(|extension| format!("{absolute}{extension}"))
                    .find(|candidate| fs.file_exists(candidate))
            })
        };
        let Some(looked_up) = looked_up else {
            continue;
        };
        match (
            program.get_source_file_for_resolved_module(&looked_up),
            u32::try_from(index),
        ) {
            (Some(file), Ok(index)) => {
                positions.entry(file.addr()).or_insert(index);
            }
            _ => unmapped.push(Arc::from(clean(name))),
        }
    }
    (positions, unmapped)
}

fn module_fact(program: &Program, file: P<SourceFile>) -> ModuleFact {
    let mut project_reference = None;
    // The only declaration-to-implementation pairing TypeScript records is a
    // configured project reference's input and its declaration output. The
    // program holds one of the two, so both mapper directions are asked. A
    // hand-written or published `x.d.ts` beside an `x.js` answers neither,
    // because resolution selected the declaration file and never opened the
    // implementation. Reconstructing that pairing from the two file names is
    // exactly the substitution this fact exists to avoid.
    let reference = program
        .get_project_reference_from_source(file.path())
        .or_else(|| program.get_project_reference_from_output_dts(file.path()));
    if let Some(reference) = reference
        && !reference.source.is_empty()
        && !reference.output_dts.is_empty()
    {
        project_reference = Some(ProjectReferenceMapping {
            source: clean(&reference.source).into(),
            output_dts: clean(&reference.output_dts).into(),
        });
    }
    let targets = program.get_redirect_targets(file.path());
    let mut redirect_targets: Vec<Arc<str>> = Vec::new();
    if !targets.is_empty() {
        let mut cleaned: Vec<String> = targets.iter().map(|target| clean(target)).collect();
        cleaned.sort();
        redirect_targets = cleaned.into_iter().map(Arc::from).collect();
    }
    ModuleFact {
        path: clean(file.file_name()).into(),
        root: None,
        declaration_file: file.is_declaration_file(),
        format: module_format(program.get_emit_module_format_of_file(file)),
        project_reference,
        redirect_targets: redirect_targets.into(),
    }
}

/// Go: `moduleFormat`. Only the emit formats that describe a real runtime
/// shape are reported. `GetEmitModuleFormatOfFile` resolves the node16/nodenext
/// family to CommonJS or an ES kind through the file's implied node format, so
/// the legacy kinds are the only ones that reach the default arm, and they are
/// answered as unknown rather than characterized.
fn module_format(kind: ModuleKind) -> ModuleFormat {
    if kind == ModuleKind::COMMON_JS {
        ModuleFormat::Commonjs
    } else if kind.is_non_node_esm() {
        ModuleFormat::Esm
    } else if kind == ModuleKind::PRESERVE {
        ModuleFormat::Preserve
    } else {
        ModuleFormat::Unknown
    }
}

fn append_import_facts(
    facts: &mut Vec<ModuleImportFact>,
    program: &Program,
    file: P<SourceFile>,
    patterns: &PathsPatterns,
    mut packages: Option<&mut PackageScopeCache<'_>>,
) -> Result<(), BackendError> {
    let specifiers = file.imports();
    if specifiers.is_empty() {
        return Ok(());
    }
    let path: Arc<str> = clean(file.file_name()).into();
    let text = file.text();
    for specifier in specifiers {
        let specifier_text = specifier.text();
        // A specifier the loader synthesized (a JSX runtime import) has no
        // source position. Its span would be a made-up one, so the answer
        // fails rather than carry it.
        let (Some(start), Some(end)) = (
            specifier.pos().source_offset(),
            specifier.end().source_offset(),
        ) else {
            return Err(BackendError(format!(
                "module specifier {specifier_text:?} of {path} has no source position"
            )));
        };
        let mut fact = ModuleImportFact {
            specifier: Location {
                path: Arc::clone(&path),
                start_byte: u64::from(skip_trivia(text, start)),
                end_byte: u64::from(end),
            },
            text: specifier_text.into(),
            resolution: ModuleResolution::Unresolved,
            resolved_path: "".into(),
            included_path: "".into(),
            symlink_path: "".into(),
            extension: "".into(),
            ts_extension: false,
            paths_pattern: patterns.matching(specifier_text).into(),
            package: None,
            resolver_package: None,
        };
        if let Some(resolved) = program.get_resolved_module_from_module_specifier(file, *specifier)
            && !resolved.resolved_file_name.is_empty()
        {
            let resolved_path = clean(resolved.resolved_file_name);
            fact.extension = resolved.extension.into();
            fact.ts_extension = resolved.resolved_using_ts_extension;
            if !resolved.original_path.is_empty() {
                fact.symlink_path = clean(resolved.original_path).into();
            }
            // The program's own redirect for the resolved name. This is the
            // one mechanism that joins a specifier resolved to a declaration
            // file back to an implementation, and it fires only for a
            // configured project reference's output.
            let redirect = program.get_parse_file_redirect(resolved.resolved_file_name);
            if !redirect.is_empty() {
                let cleaned = clean(&redirect);
                if cleaned != resolved_path {
                    fact.included_path = cleaned.into();
                }
            }
            fact.resolution = if resolved.is_external_library_import {
                ModuleResolution::NodeModules
            } else if is_relative_specifier(specifier_text) {
                ModuleResolution::Relative
            } else {
                ModuleResolution::NonRelative
            };
            if let Some(packages) = packages.as_deref_mut() {
                fact.package = packages.identity_for(&resolved_path);
                let package_id = &resolved.package_id;
                if !package_id.name.is_empty() || !package_id.version.is_empty() {
                    fact.resolver_package = Some(ResolverPackageId {
                        name: package_id.name.into(),
                        subpath: package_id.sub_module_name.into(),
                        version: package_id.version.into(),
                        peer_dependencies: package_id.peer_dependencies.into(),
                    });
                }
            }
            fact.resolved_path = resolved_path.into();
        }
        facts.push(fact);
    }
    Ok(())
}

/// Go: `isRelativeSpecifier`, mirroring `tspath.IsExternalModuleNameRelative`:
/// a specifier whose first term is `.` or `..`, or a rooted disk path, is never
/// searched for in node_modules and never matched against `paths`. Only the
/// two-character prefixes and the bare `.`/`..` forms are relative; `./` and
/// `.\` both count, because TypeScript accepts either separator in a
/// specifier.
#[must_use]
pub fn is_relative_specifier(text: &str) -> bool {
    let bytes = text.as_bytes();
    let separator = |byte: u8| byte == b'/' || byte == b'\\';
    match bytes {
        [b'.'] | [b'.', b'.'] => true,
        [b'.', second, ..] if separator(*second) => true,
        [b'.', b'.', third, ..] if separator(*third) => true,
        [first, ..] if separator(*first) => true,
        [_, b':', third, ..] if separator(*third) => true,
        _ => false,
    }
}

/// The compiler's parsed `paths` keys for one program.
struct PathsPatterns {
    eligible: bool,
    patterns: Vec<Pattern>,
}

impl PathsPatterns {
    fn of(program: &Program) -> Self {
        let options = program.options();
        let Some(paths) = options.paths.as_ref().filter(|paths| !paths.is_empty()) else {
            return Self {
                eligible: false,
                patterns: Vec::new(),
            };
        };
        Self {
            eligible: true,
            patterns: paths
                .keys()
                .map(|key| try_parse_pattern(key))
                .filter(Pattern::is_valid)
                .collect(),
        }
    }

    /// Go: `pathsPatterns.match`: the configured `paths` key selected for a
    /// specifier under TypeScript's own rules. `paths` is consulted only for a
    /// non-relative specifier, an exact key wins outright, and among star
    /// patterns the longest literal prefix wins. The predicate and the star
    /// classification are the compiler's own pattern; only the selection loop
    /// is written out here.
    fn matching(&self, text: &str) -> String {
        if !self.eligible || is_relative_specifier(text) {
            return String::new();
        }
        let mut best = "";
        let mut longest_prefix: Option<usize> = None;
        for pattern in &self.patterns {
            if let PatternKind::Wildcard(star) = pattern.kind
                && longest_prefix.is_some_and(|longest| star <= longest)
            {
                continue;
            }
            if !pattern.matches(text) {
                continue;
            }
            match pattern.kind {
                PatternKind::Wildcard(star) => {
                    best = &pattern.text;
                    longest_prefix = Some(star);
                }
                _ => return pattern.text.clone(),
            }
        }
        best.to_owned()
    }
}

/// Which `package.json` owns a file, through the compiler's own package-scope
/// lookup, memoized per directory for one graph answer. The compiler caches
/// manifest reads itself; this only avoids repeating the ancestor walk once
/// per import row.
struct PackageScopeCache<'a> {
    program: &'a Program,
    by_directory: HashMap<String, Option<PackageIdentity>>,
    by_package_directory: HashMap<String, Option<PackageIdentity>>,
}

impl<'a> PackageScopeCache<'a> {
    fn new(program: &'a Program) -> Self {
        Self {
            program,
            by_directory: HashMap::new(),
            by_package_directory: HashMap::new(),
        }
    }

    fn identity_for(&mut self, resolved_path: &str) -> Option<PackageIdentity> {
        let directory = type_script_path_dir(resolved_path);
        if let Some(cached) = self.by_directory.get(&directory) {
            return cached.clone();
        }
        let identity = self.lookup(&directory);
        self.by_directory.insert(directory, identity.clone());
        identity
    }

    fn lookup(&mut self, directory: &str) -> Option<PackageIdentity> {
        let package_directory = self
            .program
            .get_nearest_ancestor_directory_with_package_json(directory);
        if package_directory.is_empty() {
            return None;
        }
        let package_directory = normalize_type_script_path(&package_directory);
        if let Some(cached) = self.by_package_directory.get(&package_directory) {
            return cached.clone();
        }
        let manifest_path = format!("{package_directory}/package.json");
        let mut identity = PackageIdentity {
            manifest_path: clean(&manifest_path).into(),
            name: "".into(),
            version: "".into(),
        };
        if let Some(contents) = self
            .program
            .get_package_json_info(&manifest_path)
            .and_then(|entry| entry.get_contents())
        {
            if contents.name.valid {
                identity.name = contents.name.value.as_str().into();
            }
            if contents.version.valid {
                identity.version = contents.version.value.as_str().into();
            }
        }
        let identity = Some(identity);
        self.by_package_directory
            .insert(package_directory, identity.clone());
        identity
    }
}

/// Go: `normalizeTypeScriptPath`. TypeScript paths always use forward slashes,
/// including on Windows; the OS file system accepts them, while the compiler's
/// path helpers do not consistently treat backslashes as separators.
#[must_use]
pub fn normalize_type_script_path(path: &str) -> String {
    let path = path.replace('\\', "/");
    if let Some(rest) = path.strip_prefix("//?/UNC/") {
        return format!("//{rest}");
    }
    path.strip_prefix("//?/").unwrap_or(&path).to_owned()
}

/// Go: `typeScriptPathDir`.
#[must_use]
pub fn type_script_path_dir(file_name: &str) -> String {
    dir(&normalize_type_script_path(file_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_specifiers_follow_the_compilers_rule() {
        for relative in [
            ".", "..", "./a", "../a", ".\\a", "..\\a", "/abs", "\\abs", "C:/x", "c:\\x",
        ] {
            assert!(is_relative_specifier(relative), "{relative}");
        }
        for bare in ["", "a", ".a", "..a", "...", "a/b", "@scope/p", "c:x", "~/a"] {
            assert!(!is_relative_specifier(bare), "{bare}");
        }
    }

    #[test]
    fn paths_selection_prefers_an_exact_key_then_the_longest_prefix() {
        let patterns = PathsPatterns {
            eligible: true,
            patterns: ["*", "shared/*", "shared/deep/*", "exact"]
                .iter()
                .map(|key| try_parse_pattern(key))
                .collect(),
        };
        assert_eq!(patterns.matching("exact"), "exact");
        assert_eq!(patterns.matching("shared/x"), "shared/*");
        assert_eq!(patterns.matching("shared/deep/x"), "shared/deep/*");
        assert_eq!(patterns.matching("other"), "*");
        assert_eq!(patterns.matching("./shared/x"), "");
        let none = PathsPatterns {
            eligible: false,
            patterns: Vec::new(),
        };
        assert_eq!(none.matching("exact"), "");
    }

    #[test]
    fn windows_paths_normalize_to_forward_slashes() {
        // Port of Go's `TestNormalizeTypeScriptPath`.
        const WINDOWS: &str = r"\\?\D:\a\typefacts\typefacts\fixtures\project\tsconfig.json";
        assert_eq!(
            normalize_type_script_path(WINDOWS),
            "D:/a/typefacts/typefacts/fixtures/project/tsconfig.json"
        );
        assert_eq!(
            type_script_path_dir(WINDOWS),
            "D:/a/typefacts/typefacts/fixtures/project"
        );
        assert_eq!(
            normalize_type_script_path(r"\\?\UNC\server\share\project\tsconfig.json"),
            "//server/share/project/tsconfig.json"
        );
    }
}
