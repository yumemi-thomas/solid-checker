//! Native-only, non-evaluating application discovery (ADR 0270).

use crate::host_config::{self, Config, EntryMode};
use crate::{BackendError, NestedCatalogs};
use sha2::{Digest, Sha256};
use solid_facts::ast::{
    AST_FACTS_SCHEMA, BindingShape, ExportKind, FunctionFact, FunctionKind, HostConstantIdentity,
    HostDefaultInitializer, HostExecutionSiteKind, ImportFact, ImportKind, ModuleHazardKind,
    ModuleLoadKind,
};
use solid_facts::core::Span;
use solid_facts::resolution::{ImportResolution, SpecifierAttestation};
use solid_facts::{FileFacts, ProjectFacts};
use solid_reactive_ir::contract_semantics::AcceptedContractIndex;
use solid_reactive_ir::hosts::{
    BrowserRootManifest, HostExecutionRegion, HostModule, HostScope, ProjectHostIndex,
};
use solid_reactive_ir::{ProgramBoundary, RuntimeEnvironment};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Opt-in timings include refused paths; dropping the clock closes the stage.
pub(crate) struct HostStage(&'static str, Option<std::time::Instant>);

impl HostStage {
    pub(crate) fn new(stage: &'static str) -> Self {
        Self(
            stage,
            std::env::var_os("SOLID_CHECKER_TIMINGS").map(|_| std::time::Instant::now()),
        )
    }
}

impl Drop for HostStage {
    fn drop(&mut self) {
        if let Some(started) = self.1 {
            eprintln!(
                "{}",
                serde_json::json!({"hostStage": self.0, "elapsedNs": started.elapsed().as_nanos()})
            );
        }
    }
}

const CONFIGS: &[&str] = &[
    "vite.config.ts",
    "vite.config.js",
    "vite.config.mts",
    "vite.config.mjs",
    "vite.config.cts",
    "vite.config.cjs",
];
const PLUGIN_FILES: &[&str] = &["package.json", "dist/esm/index.mjs", "dist/cjs/index.cjs"];
// Synthetic fixture premise. Production always uses the complete installed
// closure catalog in host_plugins; only external fixture-issuer trust permits
// these three small files to stand in for the audited root provider.
const PLUGIN_DIGESTS: &[[&str; 3]] = &[[
    "fed4bed6f4a6ce0719e72e8308ea5ada389bc930519349c354be337f9b4eadce",
    "b2060696e245071869811d26c121e7cc4b0b1de94e0eba6c4193f1f241488682",
    "cab1d97f7d2ab2b149a1e58d34f6686ffbf878f031c9b5b0e3be0f91e6e3434f",
]];

/// Discovery refusal is explanation only, never diagnostic authority.
#[derive(Debug)]
struct DiscoveryRefusal {
    path: PathBuf,
    line: Option<usize>,
    reason: String,
}

impl DiscoveryRefusal {
    fn new(path: &Path, reason: impl Into<String>) -> Self {
        Self {
            path: path.to_owned(),
            line: None,
            reason: reason.into(),
        }
    }

    fn at(path: &Path, source: &str, offset: usize, reason: impl Into<String>) -> Self {
        let mut refusal = Self::new(path, reason);
        refusal.line = Some(
            1 + source.as_bytes()[..offset.min(source.len())]
                .iter()
                .filter(|byte| **byte == b'\n')
                .count(),
        );
        refusal
    }

    fn message(&self) -> String {
        let line = self
            .line
            .map_or_else(String::new, |line| format!(":{line}"));
        format!("{}{}: {}", self.path.display(), line, self.reason)
    }
}

trait Required<T> {
    fn required(self, path: &Path, reason: &str) -> Result<T, DiscoveryRefusal>;
}

impl<T> Required<T> for Option<T> {
    fn required(self, path: &Path, reason: &str) -> Result<T, DiscoveryRefusal> {
        self.ok_or_else(|| DiscoveryRefusal::new(path, reason))
    }
}

/// Presence is an input too. Daemons retain this closure and fingerprint it
/// before their shortcut; discovery itself runs only on a cache miss.
#[must_use]
pub fn inferred_host_input_paths(directory: &Path) -> Vec<PathBuf> {
    inferred_host_input_paths_for_project(directory, &directory.join("tsconfig.json"))
}

/// Include the analyzed config and its exact inheritance chain. Application
/// configs may have names such as tsconfig.app.json.
#[must_use]
pub fn inferred_host_input_paths_for_project(directory: &Path, project: &Path) -> Vec<PathBuf> {
    inferred_host_input_paths_for_sources(directory, project, &[])
}

/// Daemons also know explicit project sources omitted by the ordinary inventory.
#[must_use]
pub fn inferred_host_input_paths_for_sources(
    directory: &Path,
    project: &Path,
    sources: &[PathBuf],
) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if application_inputs(directory, project, &mut paths).is_err() {
        paths.sort();
        paths.dedup();
        return paths;
    }
    // The fixture marker is not authority, but enumerating with the broadest
    // possible fixture premise cannot omit a successful caller's inputs.
    // An audit refusal under that premise is an exact refusal witness too.
    let marker = directory.join(".solid-checker/shared-host-plugin.json");
    paths.push(marker.clone());
    let fixture_possible = fs::read(&marker)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        == Some(serde_json::Value::Bool(true));
    let config = match configuration(directory, &mut paths, fixture_possible) {
        Ok(config) => config,
        Err(_) => {
            paths.sort();
            paths.dedup();
            return paths;
        }
    };
    inferred_host_graph_input_paths(directory, project, paths, &config, None, None, sources)
}

fn inferred_host_graph_input_paths(
    directory: &Path,
    project: &Path,
    mut paths: Vec<PathBuf>,
    resource_config: &Config,
    facts: Option<&ProjectFacts>,
    inventory: Option<Vec<PathBuf>>,
    source_paths: &[PathBuf],
) -> Vec<PathBuf> {
    paths.extend([
        directory.join("package.json"),
        project.to_owned(),
        directory.join("index.html"),
    ]);
    paths.extend(
        directory
            .ancestors()
            .skip(1)
            .map(|ancestor| ancestor.join("package.json")),
    );
    paths.extend(CONFIGS.iter().map(|name| directory.join(name)));
    let _ = tsconfig_inputs(project, &mut paths, &mut BTreeSet::new());
    for stem in [
        "entry-client",
        "entry-server",
        "App",
        "app",
        "Document",
        "middleware",
    ] {
        for extension in ["tsx", "jsx", "ts", "js", "mjs", "tsrx"] {
            paths.push(directory.join(format!("src/{stem}.{extension}")));
        }
    }
    if let Some(inventory) = inventory {
        paths.extend(inventory);
    } else if CONFIGS.iter().any(|name| directory.join(name).exists())
        || directory.join("index.html").exists()
    {
        let _ = inventory_paths(directory, &mut paths);
    }
    let observed = facts
        .into_iter()
        .flat_map(|facts| &facts.files)
        .map(|file| (file.path.as_str(), file))
        .collect::<BTreeMap<_, _>>();
    // Plugin implementations are audited as an installed dependency closure.
    // They are not application importers. Preserve every plugin fingerprint,
    // but resolve requests only for app inventory and actual analyzed files.
    let sources = paths
        .iter()
        .filter(|path| {
            path.starts_with(directory)
                && !path
                    .components()
                    .any(|part| part.as_os_str() == "node_modules")
        })
        .cloned()
        .chain(observed.keys().map(|path| PathBuf::from(*path)))
        .chain(source_paths.iter().cloned())
        .collect::<BTreeSet<_>>();
    let host_constant_ancestors = sources
        .iter()
        .filter_map(|source| source.parent())
        .flat_map(Path::ancestors)
        .filter(|ancestor| ancestor.starts_with(directory))
        .map(Path::to_path_buf)
        .collect::<BTreeSet<_>>();
    // The narrow provably-unloadable ~/ request proof also observes package
    // candidates above the application. Cover it before daemon cache reuse.
    // Each distinct ancestor directory once: walking every path's ancestors
    // was quadratic in the input set and dominated whole runs.
    paths.sort();
    paths.dedup();
    let ancestors = sources
        .iter()
        .filter_map(|source| source.parent())
        .flat_map(Path::ancestors)
        .map(Path::to_path_buf)
        .collect::<BTreeSet<_>>();
    for ancestor in &ancestors {
        // Importer/enclosing browser maps are file-selection inputs, including
        // absent nested manifests and manifests above the application.
        paths.push(ancestor.join("package.json"));
        paths.extend(
            ["~", "~.js", "~.json", "~.node"]
                .iter()
                .map(|name| ancestor.join("node_modules").join(name)),
        );
    }
    // Static package linking consumes manifests and selected client files even
    // when no behavior contract exists. Observe the same candidates before a
    // daemon shortcut, including absent/shadowing installs and store symlinks.
    let mut resolved_requests = BTreeSet::new();
    let mut package_loads = crate::host_loadable::PackageLoads::default();
    for source in sources {
        if !source
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| {
                ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"].contains(&value)
            })
        {
            continue;
        }
        let observed = source.to_str().and_then(|path| observed.get(path));
        // A light parse gives the same specifiers; full extraction only when
        // it declines, so the input set never shrinks.
        let requests = if let Some(file) = observed {
            Some(static_fact_requests(&file.ast))
        } else if let Ok(text) = fs::read_to_string(&source) {
            solid_facts::ast::static_module_requests(&source, &text).or_else(|| {
                let ast =
                    solid_facts::ast::extract(source.to_string_lossy().into_owned(), &text).ok()?;
                Some(static_fact_requests(&ast))
            })
        } else {
            None
        };
        let Some(requests) = requests else {
            continue;
        };
        for text in requests.iter().map(String::as_str) {
            // Resolution depends on the importer directory and request, not
            // the source filename. Resource reads keep their occurrence path.
            let _ = crate::host_loadable::resource(
                directory,
                &source,
                text,
                resource_config,
                &mut paths,
            );
            for importer in [&source, &directory.join("index.ts")] {
                if resolved_requests
                    .insert((importer.parent().map(Path::to_path_buf), text.to_owned()))
                    && let Some((_, _, entries)) =
                        package_loads.candidates(importer, text, &mut paths)
                {
                    let _ = crate::host_loadable::stylesheet_closure(
                        &entries,
                        resource_config,
                        &mut paths,
                    );
                }
            }
        }
    }
    // Presence and content of every possible host-constant package are part
    // of the pre-session daemon shortcut too, not just the final manifest.
    {
        for ancestor in &host_constant_ancestors {
            for candidate in solid_dialect::DIALECTS
                .iter()
                .flat_map(|dialect| dialect.host_boolean_exports())
            {
                let package = ancestor.join("node_modules").join(candidate.module);
                paths.push(package.join("package.json"));
                paths.extend(
                    candidate
                        .runtime_targets
                        .iter()
                        .map(|target| package.join(target)),
                );
            }
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

fn static_fact_requests(ast: &solid_facts::ast::AstFacts) -> Vec<String> {
    ast.imports
        .iter()
        .filter(|import| !import.type_only)
        .map(|import| import.module.to_string())
        .chain(
            ast.exports
                .iter()
                .filter(|export| !export.type_only)
                .filter_map(|export| export.module.as_deref().map(str::to_owned)),
        )
        .collect()
}

fn identity(path: &Path) -> Option<String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Some("absent".into()),
        Err(_) => None,
    };
    if metadata
        .as_ref()
        .is_some_and(|metadata| metadata.file_type().is_symlink())
    {
        return Some(format!(
            "symlink:{}:{}",
            fs::read_link(path).ok()?.display(),
            fs::canonicalize(path).ok()?.display()
        ));
    }
    if metadata
        .as_ref()
        .map_or_else(|| path.is_dir(), fs::Metadata::is_dir)
    {
        let mut entries = fs::read_dir(path)
            .ok()?
            .map(|entry| {
                let entry = entry.ok()?;
                let name = entry.file_name().to_str()?.to_owned();
                if name == ".git" || name == "node_modules" {
                    return Some(None);
                }
                let kind = entry.file_type().ok()?;
                Some(Some((
                    name,
                    kind.is_dir(),
                    kind.is_file(),
                    kind.is_symlink(),
                )))
            })
            .collect::<Option<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        entries.sort();
        return Some(format!(
            "directory:{}:{:x}",
            fs::canonicalize(path).ok()?.display(),
            Sha256::digest(serde_json::to_vec(&entries).ok()?)
        ));
    }
    match fs::read(path) {
        Ok(bytes) => Some(format!("sha256:{:x}", Sha256::digest(bytes))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some("absent".into()),
        Err(_) => None,
    }
}

/// One graph pass observes each input once. Its manifest is re-read at every
/// transaction boundary, and the post-manifest graph has a fresh observation
/// map, so this never substitutes an earlier generation's bytes for validation.
fn graph_identity(manifest: &mut BrowserRootManifest, path: &Path) -> Option<String> {
    let key = path.to_str()?;
    if let Some(observed) = manifest.inputs.get(key) {
        return Some(observed.clone());
    }
    let observed = identity(path)?;
    manifest.inputs.insert(key.into(), observed.clone());
    Some(observed)
}

// Every pass still reads current bytes and path identities. Bound parallel I/O
// only; never reuse an earlier pass's hash or a metadata-only observation.
pub(super) fn identities(paths: &[&Path]) -> Vec<Option<String>> {
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(8);
    if paths.len() < 128 || workers == 1 {
        return paths.iter().map(|path| identity(path)).collect();
    }
    std::thread::scope(|scope| {
        // Interleave paths so a large package's adjacent files cannot all land
        // on one worker. Preserve the caller's order when joining observations.
        let jobs = (0..workers)
            .map(|worker| {
                scope.spawn(move || {
                    (worker..paths.len())
                        .step_by(workers)
                        .map(|index| (index, identity(paths[index])))
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>();
        let mut observations = jobs
            .into_iter()
            .flat_map(|job| job.join().expect("input fingerprint worker panicked"))
            .collect::<Vec<_>>();
        observations.sort_by_key(|(index, _)| *index);
        observations
            .into_iter()
            .map(|(_, identity)| identity)
            .collect()
    })
}

/// Directory membership is a cache input, independently of existing file
/// contents. A new excluded server importer must invalidate a browser answer.
#[must_use]
pub fn inferred_host_directory_digest(path: &Path) -> Option<[u8; 32]> {
    inferred_host_input_digest(path)
}

/// Content, absence, directory membership, and symlink identity all matter.
/// Metadata alone is never sufficient to reuse inferred authority.
#[must_use]
pub fn inferred_host_input_digest(path: &Path) -> Option<[u8; 32]> {
    Some(Sha256::digest(identity(path)?.as_bytes()).into())
}

fn inventory_paths(directory: &Path, paths: &mut Vec<PathBuf>) -> Option<()> {
    paths.push(directory.to_path_buf());
    for entry in fs::read_dir(directory).ok()? {
        let entry = entry.ok()?;
        if entry.file_name() == ".git" || entry.file_name() == "node_modules" {
            continue;
        }
        let kind = entry.file_type().ok()?;
        if kind.is_symlink() {
            paths.push(entry.path());
            continue;
        }
        if kind.is_dir() {
            inventory_paths(&entry.path(), paths)?;
        } else {
            let path = entry.path();
            if path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    [
                        "ts", "tsx", "tsrx", "mts", "cts", "js", "jsx", "mjs", "cjs", "json",
                        "html", "css", "pcss", "postcss", "less", "sass", "scss", "styl", "stylus",
                        "sss",
                    ]
                    .contains(&extension)
                })
            {
                paths.push(path);
            }
        }
    }
    Some(())
}

fn input_manifest(
    directory: &Path,
    facts: &ProjectFacts,
    config: &Config,
    inputs: Vec<PathBuf>,
) -> Result<BTreeMap<String, String>, DiscoveryRefusal> {
    let _stage = HostStage::new("manifest-identity");
    let mut inventory = Vec::new();
    inventory_paths(directory, &mut inventory)
        .required(directory, "cannot inventory application inputs")?;
    let mut paths = inferred_host_graph_input_paths(
        directory,
        Path::new(&facts.project_id),
        inputs,
        config,
        Some(facts),
        Some(inventory),
        &[],
    );
    // The graph-input pass inventories the application. Omitted
    // implementations remain inputs, never evidence against a positive edge.
    let mut source_identities = BTreeMap::new();
    for file in &facts.files {
        let path = Path::new(file.path.as_str());
        if fs::canonicalize(path)
            .ok()
            .required(path, "cannot resolve analyzed source")?
            .as_path()
            != path
        {
            return Err(DiscoveryRefusal::new(
                path,
                "analyzed source path is not canonical",
            ));
        }
        paths.push(path.to_path_buf());
        for ancestor in path
            .parent()
            .required(path, "analyzed source has no parent")?
            .ancestors()
        {
            paths.push(ancestor.join("package.json"));
        }
        let source = fs::read_to_string(path)
            .ok()
            .required(path, "cannot read analyzed source")?;
        if solid_facts::core::SourceHash::of(&source) != file.source_hash {
            return Err(DiscoveryRefusal::new(path, "analyzed source bytes changed"));
        }
        source_identities.insert(path.to_path_buf(), file.source_hash.as_str().to_owned());
    }
    paths.sort();
    paths.dedup();
    let fingerprints = identities(&paths.iter().map(PathBuf::as_path).collect::<Vec<_>>());
    paths
        .into_iter()
        .zip(fingerprints)
        .map(|(path, fingerprint)| {
            Ok((
                path.to_str()
                    .required(&path, "non-UTF-8 input path")?
                    .to_owned(),
                source_identities
                    .remove(&path)
                    .or(fingerprint)
                    .required(&path, "cannot fingerprint input")?,
            ))
        })
        .collect()
}

/// Called before/after solving. Stale inferred authority is refused, never
/// attached to facts from another input generation.
pub(crate) fn validate_inputs(index: &ProjectHostIndex) -> Result<(), BackendError> {
    let _stage = HostStage::new("input-validation");
    let paths = index
        .manifest
        .inputs
        .keys()
        .map(Path::new)
        .collect::<Vec<_>>();
    for ((_, expected), actual) in index.manifest.inputs.iter().zip(identities(&paths)) {
        if actual.as_ref() != Some(expected) {
            return Err(BackendError::Contract(
                "inferred browser inputs changed; retry analysis".into(),
            ));
        }
    }
    Ok(())
}

fn unpublished(package: &serde_json::Value) -> bool {
    ["exports", "main", "module", "bin", "types", "typings"]
        .iter()
        .all(|key| package.get(*key).is_none_or(serde_json::Value::is_null))
}

#[cfg(test)]
fn application(directory: &Path, project: &Path) -> Result<(), DiscoveryRefusal> {
    application_inputs(directory, project, &mut Vec::new())
}

fn application_inputs(
    directory: &Path,
    project: &Path,
    inputs: &mut Vec<PathBuf>,
) -> Result<(), DiscoveryRefusal> {
    let path = directory.join("package.json");
    inputs.push(path.clone());
    if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_symlink()) {
        return Err(DiscoveryRefusal::new(
            &path,
            "symlinked application manifest has unknown config selection",
        ));
    }
    let package: serde_json::Value = serde_json::from_slice(
        &fs::read(&path)
            .ok()
            .required(&path, "cannot read application manifest")?,
    )
    .ok()
    .required(&path, "invalid application manifest JSON")?;
    if !unpublished(&package) {
        return Err(DiscoveryRefusal::new(
            &path,
            "published entry points permit unknown execution hosts",
        ));
    }
    if let Some(scripts) = package.get("scripts")
        && let Some(reason) = crate::host_invocation::refusal(
            scripts,
            crate::host_invocation::ManifestRole::Application,
        )
    {
        return Err(DiscoveryRefusal::new(&path, reason));
    }
    // A workspace leaf has its own application roots and project inventory.
    // Published enclosing packages can still expose the leaf to unknown hosts.
    for ancestor in directory.ancestors().skip(1) {
        let path = ancestor.join("package.json");
        inputs.push(path.clone());
        if path.exists() {
            if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_symlink()) {
                return Err(DiscoveryRefusal::new(
                    &path,
                    "symlinked enclosing manifest has unknown config selection",
                ));
            }
            let package: serde_json::Value = serde_json::from_slice(
                &fs::read(&path)
                    .ok()
                    .required(&path, "cannot read enclosing manifest")?,
            )
            .ok()
            .required(&path, "invalid enclosing manifest JSON")?;
            if !unpublished(&package) {
                return Err(DiscoveryRefusal::new(
                    &path,
                    "published enclosing package permits unknown execution hosts",
                ));
            }
            if let Some(scripts) = package.get("scripts")
                && let Some(reason) = crate::host_invocation::refusal(
                    scripts,
                    crate::host_invocation::ManifestRole::Enclosing,
                )
            {
                return Err(DiscoveryRefusal::new(&path, reason));
            }
        }
    }
    tsconfig_inputs(project, inputs, &mut BTreeSet::new())?;
    Ok(())
}

fn tsconfig_inputs(
    path: &Path,
    inputs: &mut Vec<PathBuf>,
    seen: &mut BTreeSet<PathBuf>,
) -> Result<(), DiscoveryRefusal> {
    inputs.push(path.to_owned());
    let path = fs::canonicalize(path)
        .ok()
        .required(path, "cannot resolve tsconfig")?;
    if !seen.insert(path.clone()) {
        return Err(DiscoveryRefusal::new(&path, "cyclic tsconfig inheritance"));
    }
    inputs.push(path.clone());
    let source = fs::read_to_string(&path)
        .ok()
        .required(&path, "cannot read tsconfig")?;
    let config = host_config::jsonc(&source).required(&path, "invalid tsconfig JSONC")?;
    if let Some(references) = config.get("references") {
        let references = references
            .as_array()
            .required(&path, "project references is not an array")?;
        if !references.is_empty() {
            return Err(DiscoveryRefusal::at(
                &path,
                &source,
                source.find("\"references\"").unwrap_or(0),
                "project references need a complete execution inventory",
            ));
        }
    }
    if let Some(options) = config
        .get("compilerOptions")
        .and_then(serde_json::Value::as_object)
        && ["rootDirs", "moduleSuffixes", "customConditions", "plugins"]
            .iter()
            .any(|key| options.contains_key(*key))
    {
        let key = ["rootDirs", "moduleSuffixes", "customConditions", "plugins"]
            .into_iter()
            .find(|key| options.contains_key(*key))
            .expect("matched option");
        return Err(DiscoveryRefusal::at(
            &path,
            &source,
            source.find(&format!("\"{key}\"")).unwrap_or(0),
            format!("unsupported resolution option {key}"),
        ));
    }
    if let Some(extends) = config.get("extends") {
        let extends = extends
            .as_str()
            .required(&path, "nonliteral tsconfig extends")?;
        // No guessed package export resolution for config inheritance.
        if !extends.starts_with('.') || !extends.ends_with(".json") {
            return Err(DiscoveryRefusal::at(
                &path,
                &source,
                source.find("\"extends\"").unwrap_or(0),
                "tsconfig extends requires an exact relative JSON path",
            ));
        }
        tsconfig_inputs(
            &path
                .parent()
                .required(&path, "tsconfig has no parent")?
                .join(extends),
            inputs,
            seen,
        )?;
    }
    Ok(())
}

fn audited_start_plugin(directory: &Path, inputs: &mut Vec<PathBuf>) -> Option<()> {
    let root = crate::host_plugins::installed(directory, "@solidjs/vite-plugin", inputs)?;
    let digests = PLUGIN_FILES
        .iter()
        .map(|name| {
            let path = root.join(name);
            inputs.push(path.clone());
            Some(format!("{:x}", Sha256::digest(fs::read(path).ok()?)))
        })
        .collect::<Option<Vec<_>>>()?;
    PLUGIN_DIGESTS
        .iter()
        .any(|pin| pin.iter().zip(&digests).all(|(pin, digest)| *pin == digest))
        .then_some(())
}

fn side_inputs(directory: &Path, inputs: &mut Vec<PathBuf>) -> Result<(), DiscoveryRefusal> {
    // Vite's default PostCSS discovery loads JavaScript and named plugins in
    // Node. Include absence above the leaf too; JSON is not an inert exemption.
    for ancestor in directory.ancestors() {
        let package = ancestor.join("package.json");
        inputs.push(package.clone());
        if package.exists() {
            let package: serde_json::Value = serde_json::from_slice(
                &fs::read(&package)
                    .ok()
                    .required(&package, "cannot read PostCSS discovery manifest")?,
            )
            .ok()
            .required(&package, "invalid PostCSS discovery manifest")?;
            if package.get("postcss").is_some() {
                return Err(DiscoveryRefusal::new(
                    &ancestor.join("package.json"),
                    "executable PostCSS configuration",
                ));
            }
        }
        for stem in [".postcssrc", "postcss.config"] {
            for suffix in [
                "", ".json", ".yaml", ".yml", ".ts", ".cts", ".mts", ".js", ".cjs", ".mjs",
            ] {
                let path = ancestor.join(format!("{stem}{suffix}"));
                inputs.push(path.clone());
                if path.exists() {
                    return Err(DiscoveryRefusal::new(
                        &path,
                        "executable PostCSS side input",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn configuration(
    directory: &Path,
    inputs: &mut Vec<PathBuf>,
    fixture_authority: bool,
) -> Result<Config, DiscoveryRefusal> {
    configuration_in_generation(directory, inputs, fixture_authority, None)
}

fn configuration_in_generation(
    directory: &Path,
    inputs: &mut Vec<PathBuf>,
    fixture_authority: bool,
    generation: Option<&BTreeMap<String, String>>,
) -> Result<Config, DiscoveryRefusal> {
    side_inputs(directory, inputs)?;
    // Observe directory membership even on refusal: adding an alternate or
    // mode-specific config must invalidate a cached conventional answer.
    inputs.push(directory.to_owned());
    let entries = fs::read_dir(directory)
        .ok()
        .required(directory, "cannot enumerate Vite config variants")?;
    let mut variants = Vec::new();
    for entry in entries {
        let entry = entry
            .ok()
            .required(directory, "cannot enumerate Vite config variant")?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .required(directory, "non-UTF-8 config candidate")?;
        if name.starts_with("vite.")
            && (name.contains(".config.") || name.starts_with("vite.config."))
            && !CONFIGS.contains(&name)
        {
            variants.push(entry.path());
        }
    }
    variants.sort();
    inputs.extend(variants.iter().cloned());
    if let Some(path) = variants.first() {
        return Err(DiscoveryRefusal::new(
            path,
            "non-conventional or mode-specific Vite config",
        ));
    }
    let configs = CONFIGS
        .iter()
        .map(|name| directory.join(name))
        .filter(|path| path.exists())
        .collect::<Vec<_>>();
    inputs.extend(CONFIGS.iter().map(|name| directory.join(name)));
    let [path] = configs.as_slice() else {
        return Err(DiscoveryRefusal::new(
            directory,
            format!("expected one Vite config, found {}", configs.len()),
        ));
    };
    if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_symlink()) {
        return Err(DiscoveryRefusal::new(
            path,
            "symlinked Vite config is outside the closed selection grammar",
        ));
    }
    let source = fs::read_to_string(path)
        .ok()
        .required(path, "cannot read Vite config")?;
    let config = host_config::parse_detailed(path, &source)
        .map_err(|reason| DiscoveryRefusal::at(path, &source, reason.offset, reason.reason))?;
    let mut audited_packages = crate::host_plugins::PackageAudits::new();
    for name in &config.plugins {
        if name == "@solidjs/vite-plugin" && config.mode == EntryMode::ClientStart {
            if !fixture_authority || audited_start_plugin(directory, inputs).is_none() {
                crate::host_plugins::audit(
                    directory,
                    name,
                    inputs,
                    &mut audited_packages,
                    generation,
                )
                .map_err(|reason| DiscoveryRefusal::new(path, reason))?;
            }
        } else {
            crate::host_plugins::audit(directory, name, inputs, &mut audited_packages, generation)
                .map_err(|reason| DiscoveryRefusal::new(path, reason))?;
        }
    }
    if config.plugins.contains("@tanstack/router-plugin/vite") {
        // JSON can point at executable virtual-route modules. Options alone
        // do not disable generator discovery of this file.
        let path = directory.join("tsr.config.json");
        inputs.push(path.clone());
        if path.exists() {
            return Err(DiscoveryRefusal::new(
                &path,
                "executable virtual-route configuration",
            ));
        }
        let mut routes = Vec::new();
        let root = directory.join("src/routes");
        inputs.push(root.clone());
        if root.exists() {
            inventory_paths(&root, &mut routes).required(&root, "cannot inventory routes")?;
            if routes.iter().any(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains("__virtual."))
            }) {
                let path = routes
                    .iter()
                    .find(|path| {
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| name.contains("__virtual."))
                    })
                    .expect("matched virtual route");
                return Err(DiscoveryRefusal::new(
                    path,
                    "executable virtual-route module",
                ));
            }
            inputs.extend(routes);
        }
    }
    if config.tailwind {
        let mut inventory = Vec::new();
        inventory_paths(directory, &mut inventory)
            .required(directory, "cannot inventory Tailwind side inputs")?;
        // Keep the complete inventory in cache identity. Successful builds
        // discharge CSS compilation only; executable plugin/config side inputs
        // still cannot enter the reviewed configuration through a stylesheet.
        inputs.extend(inventory.iter().cloned());
        if !crate::host_loadable::stylesheet_closure(&inventory, &config, inputs) {
            return Err(DiscoveryRefusal::new(
                directory,
                "executable stylesheet plugin/config side-input closure is unproved",
            ));
        }
    }
    Ok(config)
}

fn route_file(directory: &Path, config: &Config, file: &FileFacts) -> bool {
    config
        .routes
        .as_ref()
        .is_some_and(|dir| Path::new(file.path.as_str()).starts_with(directory.join(dir)))
}

fn server_route(ast: &solid_facts::ast::AstFacts) -> bool {
    ast.module_level_exports().any(|export| {
        export.kind == ExportKind::All
            || export
                .specifiers
                .iter()
                .chain(&export.declarations)
                .any(|name| {
                    [
                        "GET", "HEAD", "POST", "PUT", "DELETE", "PATCH", "OPTIONS", "CONNECT",
                        "TRACE", "config", "route",
                    ]
                    .contains(&name.exported.as_str())
                })
    })
}

/// Audited filesystem-routing emits a static named import for every $$route
/// ref, even with code splitting on. Inventory excluded files too; the analyzed
/// file set alone cannot prove that no generated eager ref exists.
fn lazy_route_manifest(directory: &Path, config: &Config, inputs: &mut Vec<PathBuf>) -> bool {
    fn lazy_tree(path: &Path, inputs: &mut Vec<PathBuf>) -> bool {
        inputs.push(path.to_owned());
        let Ok(metadata) = fs::symlink_metadata(path) else {
            return false;
        };
        if metadata.file_type().is_symlink() {
            return false;
        }
        if metadata.is_dir() {
            let Ok(entries) = fs::read_dir(path) else {
                return false;
            };
            return entries
                .into_iter()
                .all(|entry| entry.is_ok_and(|entry| lazy_tree(&entry.path(), inputs)));
        }
        if !metadata.is_file() {
            return false;
        }
        if !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| ["js", "jsx", "ts", "tsx"].contains(&extension))
        {
            return true;
        }
        let Some(ast) = fs::read_to_string(path).ok().and_then(|source| {
            solid_facts::ast::extract(path.to_string_lossy().into_owned(), &source).ok()
        }) else {
            return false;
        };
        !ast.module_level_exports().any(|export| {
            export.kind == ExportKind::All
                || export
                    .specifiers
                    .iter()
                    .chain(&export.declarations)
                    .any(|specifier| specifier.exported == "route")
        })
    }
    config.plugins.contains("filesystem-routing/vite")
        && config.lazy_routes
        && config
            .routes
            .as_ref()
            .is_some_and(|routes| lazy_tree(&directory.join(routes), inputs))
}

fn default_scope(
    file: &FileFacts,
    facts: &ProjectFacts,
    functions: &BTreeMap<String, String>,
) -> Option<String> {
    let defaults = file
        .ast
        .module_level_exports()
        .filter(|export| export.kind == ExportKind::Default)
        .collect::<Vec<_>>();
    let [export] = defaults.as_slice() else {
        return None;
    };
    let [declaration] = export.declarations.as_slice() else {
        return None;
    };
    let span = unwrapped(file, declaration.local.span);
    let symbol = exact_symbol(facts, file.path.as_str(), span);
    file.ast
        .functions
        .iter()
        .find(|function| {
            function
                .name
                .as_ref()
                .map_or(function.span, |name| name.span)
                == span
                || symbol.as_ref().and_then(|symbol| functions.get(symbol))
                    == Some(&scope_id(file.path.as_str(), Some(function)))
        })
        .map(|function| scope_id(file.path.as_str(), Some(function)))
}

/// Restricted HTML tokenizer: no unproved interfering script, base, template/noscript,
/// active foreign content or entity-bearing URLs. Balanced passive SVG shapes
/// are inert; script/integration elements, handlers and external refs refuse.
/// Comments/raw text are consumed as
/// such, so a script-shaped string in either never becomes a root.
#[cfg(test)]
fn html_entries(html: &str) -> Option<Vec<String>> {
    html_entries_at(Path::new("index.html"), html).ok()
}

fn html_entries_at(path: &Path, source: &str) -> Result<Vec<String>, DiscoveryRefusal> {
    let mut html = source;
    let mut foreign = Vec::<String>::new();
    let mut entries = Vec::new();
    while let Some(start) = html.find('<') {
        html = &html[start..];
        let offset = source.len() - html.len();
        let refusal = |reason: &str| DiscoveryRefusal::at(path, source, offset, reason);
        if html.starts_with("<!--") {
            html = &html[html
                .find("-->")
                .ok_or_else(|| refusal("unterminated HTML comment"))?
                + 3..];
            continue;
        }
        if html
            .get(..15)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("<!doctype html>"))
        {
            html = &html[15..];
            continue;
        }
        let end = html
            .find('>')
            .ok_or_else(|| refusal("unterminated HTML tag"))?;
        let tag = html
            .get(1..end)
            .ok_or_else(|| refusal("invalid HTML tag boundary"))?;
        if tag.contains('<') {
            return Err(refusal("nested markup in HTML tag"));
        }
        html = &html[end + 1..];
        let tag = tag.trim();
        let closing = tag.starts_with('/');
        let tag = tag.trim_start_matches('/');
        let name_end = tag
            .find(|c: char| c.is_ascii_whitespace() || c == '/')
            .unwrap_or(tag.len());
        let name = tag[..name_end].to_ascii_lowercase();
        if ![
            "html", "head", "body", "meta", "link", "title", "style", "div", "main", "script",
            "svg", "defs", "symbol", "path", "rect", "line", "polygon", "use",
        ]
        .contains(&name.as_str())
        {
            return Err(refusal(&format!(
                "unsupported HTML/foreign element <{name}>"
            )));
        }
        if !foreign.is_empty()
            && ![
                "svg", "defs", "symbol", "path", "rect", "line", "polygon", "use",
            ]
            .contains(&name.as_str())
        {
            return Err(refusal("active or integration element inside SVG"));
        }
        if closing {
            if !tag[name_end..].trim().is_empty() {
                return Err(refusal("attributes on a closing HTML tag"));
            }
            if !foreign.is_empty() && foreign.pop().as_deref() != Some(name.as_str()) {
                return Err(refusal("unbalanced passive SVG subtree"));
            }
            continue;
        }
        let mut attributes = BTreeMap::new();
        let mut rest = tag[name_end..].trim();
        while !rest.is_empty() && rest != "/" {
            let end = rest
                .find(|c: char| c.is_ascii_whitespace() || c == '=')
                .ok_or_else(|| refusal("unquoted or boolean HTML attribute"))?;
            let key = rest[..end].to_ascii_lowercase();
            rest = rest[end..].trim_start();
            rest = rest
                .strip_prefix('=')
                .ok_or_else(|| refusal("boolean HTML attribute is outside the entry grammar"))?
                .trim_start();
            let quote = rest
                .chars()
                .next()
                .ok_or_else(|| refusal("missing HTML attribute value"))?;
            if quote != '\'' && quote != '"' {
                return Err(refusal("unquoted HTML attribute"));
            }
            rest = &rest[1..];
            let end = rest
                .find(quote)
                .ok_or_else(|| refusal("unterminated HTML attribute value"))?;
            let value = &rest[..end];
            if value.contains('&') || attributes.insert(key, value.to_owned()).is_some() {
                return Err(refusal("entity-bearing or duplicate HTML attribute"));
            }
            rest = rest[end + 1..].trim_start();
        }
        if attributes.keys().any(|key| key.starts_with("on")) {
            return Err(refusal("executable inline event attribute"));
        }
        if name == "svg" || !foreign.is_empty() {
            if attributes.iter().any(|(key, value)| {
                ["href", "xlink:href"].contains(&key.as_str()) && !value.starts_with('#')
            }) {
                return Err(refusal("external SVG reference"));
            }
            if !tag.ends_with('/') {
                foreign.push(name);
            }
            continue;
        }
        if ["defs", "symbol", "path", "rect", "line", "polygon", "use"].contains(&name.as_str()) {
            return Err(refusal("foreign element outside a passive SVG subtree"));
        }
        // A source-authored CSP can forbid every module script. The presence
        // of a script element then supplies no feasible window entry. Other
        // http-equiv policies are also outside this closed HTML grammar.
        if name == "meta" && attributes.contains_key("http-equiv") {
            return Err(refusal(
                "authored http-equiv policy may forbid module execution",
            ));
        }
        if name == "script" {
            if attributes
                .keys()
                .any(|key| !["type", "src"].contains(&key.as_str()))
            {
                return Err(refusal("unsupported module-script attribute"));
            }
            let end = html
                .find("</script>")
                .ok_or_else(|| refusal("missing literal script closing tag"))?;
            let body = &html[..end];
            let module = attributes
                .get("type")
                .is_some_and(|value| value == "module");
            if !module {
                if attributes.contains_key("src")
                    || attributes
                        .get("type")
                        .is_some_and(|value| !value.eq_ignore_ascii_case("text/javascript"))
                    || !crate::host_inline::independent(body)
                {
                    return Err(refusal(
                        "classic script may interfere with HTML module entries",
                    ));
                }
                html = &html[end + 9..];
                continue;
            }
            let Some(source) = attributes.get("src") else {
                // This element supplies no analyzed root. Independent siblings
                // can still execute, even after this script throws. Preserve
                // the interference guard; arbitrary opaque module code is not
                // permission to navigate away before another entry executes.
                if !crate::host_inline::independent(body) {
                    return Err(refusal(
                        "inline module has unproved interference with external entries",
                    ));
                }
                html = &html[end + 9..];
                continue;
            };
            if source.contains(['?', '#', ':', '%', '\\']) || source.starts_with("//") {
                return Err(refusal("module URL is external or carries a query/escape"));
            }
            entries.push(source.trim_start_matches('/').to_owned());
            if !body.trim().is_empty() {
                return Err(refusal("inline module-script body"));
            }
            html = &html[end + 9..];
        } else if name == "style" {
            // Vite can turn inline styles into CSS proxy modules. Their
            // executable preprocessor side inputs have no transform proof.
            return Err(refusal("inline stylesheet preprocessing is unproved"));
        } else if name == "title" {
            let close = format!("</{name}>");
            html = &html[html
                .find(&close)
                .ok_or_else(|| refusal("unterminated HTML title"))?
                + close.len()..];
        }
    }
    if !foreign.is_empty() {
        return Err(DiscoveryRefusal::new(
            path,
            "unterminated passive SVG subtree",
        ));
    }
    if entries.is_empty() {
        return Err(DiscoveryRefusal::new(
            path,
            "no external module-script roots",
        ));
    }
    Ok(entries)
}

fn scope_id(file: &str, function: Option<&FunctionFact>) -> String {
    function.map_or_else(
        || format!("{file}#init"),
        |function| format!("{file}#{}:{}", function.span.start, function.span.end),
    )
}

fn client_root(exists: impl Fn(&str) -> bool) -> Option<(String, bool)> {
    for extension in ["tsx", "jsx", "ts", "js", "mjs", "tsrx"] {
        let path = format!("src/entry-client.{extension}");
        if exists(&path) {
            return Some((path, false));
        }
    }
    for stem in ["App", "app"] {
        for extension in ["tsx", "jsx", "ts", "js", "tsrx"] {
            let path = format!("src/{stem}.{extension}");
            if exists(&path) {
                return Some((path, true));
            }
        }
    }
    None
}

fn at_scope(file: &FileFacts, span: Span) -> String {
    let function = file
        .ast
        .functions
        .iter()
        .filter(|function| function.span.contains(span))
        .min_by_key(|function| function.span.end - function.span.start);
    if let Some(default) = file
        .ast
        .host_defaults
        .iter()
        .filter(|default| {
            default.span.contains(span)
                && function.is_some_and(|function| function.span == default.function)
        })
        .min_by_key(|default| default.span.end - default.span.start)
    {
        return default_scope_id(file.path.as_str(), default);
    }
    scope_id(file.path.as_str(), function)
}

fn default_scope_id(path: &str, default: &HostDefaultInitializer) -> String {
    format!("{path}#default:{}:{}", default.span.start, default.span.end)
}

/// A default runs only on a proven undefined slot. A spread before the slot
/// makes its runtime position unknown. Only a normalized void-zero value is
/// positive undefined evidence; unresolved names and type casts are not.
/// Other supplied arguments must be inert literal values: a spread/getter/call
/// can exit before the callee is entered, even after the defaulted slot.
fn omitted_or_undefined(
    call: &solid_facts::ast::CallFact,
    index: usize,
    undefined_values: &[Span],
) -> bool {
    inert_arguments(call, undefined_values)
        && call.arguments.get(index).is_none_or(|argument| {
            undefined_values.contains(&argument.value_span.unwrap_or(argument.span))
        })
}

fn inert_arguments(call: &solid_facts::ast::CallFact, undefined_values: &[Span]) -> bool {
    call.arguments.iter().all(|argument| {
        !argument.spread
            && (undefined_values.contains(&argument.value_span.unwrap_or(argument.span))
                || matches!(
                    &argument.literal_value,
                    solid_facts::ast::ArgumentLiteralFact::Null
                        | solid_facts::ast::ArgumentLiteralFact::Boolean(_)
                        | solid_facts::ast::ArgumentLiteralFact::Integer(_)
                        | solid_facts::ast::ArgumentLiteralFact::String(_)
                        | solid_facts::ast::ArgumentLiteralFact::Function
                ))
    })
}

fn parameter_initialization_complete(
    ast: &solid_facts::ast::AstFacts,
    function: &FunctionFact,
    call: &solid_facts::ast::CallFact,
    index: usize,
) -> bool {
    let parameter = &function.parameters[index];
    parameter.shape == BindingShape::Identifier
        && (parameter.initializer.is_none()
            || call.arguments.get(index).is_some_and(|argument| {
                matches!(
                    &argument.literal_value,
                    solid_facts::ast::ArgumentLiteralFact::Null
                        | solid_facts::ast::ArgumentLiteralFact::Boolean(_)
                        | solid_facts::ast::ArgumentLiteralFact::Integer(_)
                        | solid_facts::ast::ArgumentLiteralFact::String(_)
                        | solid_facts::ast::ArgumentLiteralFact::Function
                )
            })
            || ast.host_defaults.iter().any(|default| {
                default.function == function.span
                    && default.argument_index == Some(index)
                    && default.inert
            }))
}

fn call_edges(
    facts: &ProjectFacts,
    caller: &FileFacts,
    owner: &str,
    target: &str,
    call: &solid_facts::ast::CallFact,
    scopes: &mut BTreeMap<String, HostScope>,
) -> Option<()> {
    let target_scope = scopes.get(target)?;
    let target_path = target_scope.path.clone();
    let target_span = target_scope.span?;
    let file = facts
        .files
        .iter()
        .find(|file| file.path.as_str() == target_path)?;
    let function = file
        .ast
        .functions
        .iter()
        .find(|function| function.span == target_span)?;
    // Argument evaluation and earlier defaults precede this initializer. An
    // arbitrary previous initializer/destructuring step may throw. Do not
    // promote later defaults or the body from that uncertain transition.
    let defaults = file
        .ast
        .host_defaults
        .iter()
        .filter(|default| {
            default.function == target_span
                && default.argument_index.is_some_and(|index| {
                    omitted_or_undefined(call, index, &caller.ast.host_undefined_arguments)
                        && function
                            .parameters
                            .get(index)
                            .is_some_and(|parameter| parameter.shape == BindingShape::Identifier)
                        && (0..index).all(|previous| {
                            parameter_initialization_complete(&file.ast, function, call, previous)
                        })
                })
        })
        .map(|default| default_scope_id(&target_path, default))
        .collect::<Vec<_>>();
    let scope = scopes.get_mut(owner)?;
    if inert_arguments(call, &caller.ast.host_undefined_arguments)
        && (0..function.parameters.len())
            .all(|index| parameter_initialization_complete(&file.ast, function, call, index))
    {
        scope.edges.insert(target.into());
    }
    scope.edges.extend(defaults);
    Some(())
}

/// JSX supplies a props object, not an exact ordinary call argument list.
/// Destructuring/getters/defaults can throw before the body is entered.
fn jsx_body_ready(facts: &ProjectFacts, scopes: &BTreeMap<String, HostScope>, id: &str) -> bool {
    let Some(scope) = scopes.get(id) else {
        return false;
    };
    facts
        .files
        .iter()
        .find(|file| file.path.as_str() == scope.path)
        .and_then(|file| {
            file.ast
                .functions
                .iter()
                .find(|function| Some(function.span) == scope.span)
        })
        .is_some_and(|function| {
            !function.generator
                && !function.rest_parameter
                && function.parameters.iter().all(|parameter| {
                    parameter.shape == BindingShape::Identifier && parameter.initializer.is_none()
                })
        })
}

/// The dialect supplies the exact candidate export/artifact vocabulary.
/// The shared facts contain no Solid spelling or package behavior. Aliases,
/// missing attestations, another artifact, and absent runtime bytes leave it
/// unknown. Record every byte/presence premise in the inference fingerprint.
fn execution_constants(
    prepared: &DiscoveryInputs,
    facts: &ProjectFacts,
    file: &FileFacts,
    dialect: &dyn solid_dialect::Dialect,
    manifest: &mut BrowserRootManifest,
) -> BTreeMap<Span, (bool, bool)> {
    let directory = &prepared.directory;
    let config = &prepared.config;
    let mut values = BTreeMap::new();
    let Some(resolutions) = &facts.resolved_imports else {
        return values;
    };
    let browser = std::cell::OnceCell::new();
    let server = std::cell::OnceCell::new();
    for candidate_export in dialect.host_boolean_exports() {
        for import in file
            .ast
            .imports
            .iter()
            .filter(|import| !import.type_only && import.module == candidate_export.module)
        {
            if (config.tsconfig_paths
                || browser_selection_required(Path::new(file.path.as_str()), manifest))
                && facts.runtime_resolutions.is_none()
            {
                continue;
            }
            if config.aliases.keys().any(|alias| {
                import.module == alias.as_str()
                    || import
                        .module
                        .strip_prefix(alias.as_str())
                        .is_some_and(|tail| tail.starts_with('/'))
            }) {
                continue;
            }
            let SpecifierAttestation::Attested(row) =
                resolutions.specifier(file.path.as_str(), import.span, import.module.as_str())
            else {
                continue;
            };
            if row.resolution != ImportResolution::NodeModules
                || !row.symlink_path.is_empty()
                || !row.included_path.is_empty()
                || row.package_name.as_deref() != Some(candidate_export.module)
                || row.package_version.as_deref() != Some(candidate_export.version)
            {
                continue;
            }
            let Some(package_json) = row.package_manifest.as_deref().map(Path::new) else {
                continue;
            };
            let Some(package) = package_json.parent() else {
                continue;
            };
            let candidate = Path::new(file.path.as_str()).parent().and_then(|parent| {
                parent.ancestors().find_map(|ancestor| {
                    let candidate = ancestor.join("node_modules").join(candidate_export.module);
                    let path = candidate.join("package.json");
                    let observed =
                        graph_identity(manifest, &path).unwrap_or_else(|| "unreadable".into());
                    manifest
                        .inputs
                        .insert(path.to_string_lossy().into_owned(), observed);
                    path.exists().then_some(candidate)
                })
            });
            if candidate.as_deref() != Some(package)
                || !package.starts_with(directory)
                || fs::canonicalize(package).ok().as_deref() != Some(package)
            {
                continue;
            }
            let Some(package_identity) = graph_identity(manifest, package_json) else {
                continue;
            };
            manifest.inputs.insert(
                package_json.to_string_lossy().into_owned(),
                package_identity,
            );
            let Ok(bytes) = fs::read(package_json) else {
                continue;
            };
            let Ok(metadata) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                continue;
            };
            if metadata.get("name").and_then(serde_json::Value::as_str)
                != Some(candidate_export.module)
                || metadata.get("version").and_then(serde_json::Value::as_str)
                    != Some(candidate_export.version)
            {
                continue;
            }
            for target in candidate_export.runtime_targets {
                let path = package.join(target);
                let observed =
                    graph_identity(manifest, &path).unwrap_or_else(|| "unreadable".into());
                manifest
                    .inputs
                    .insert(path.to_string_lossy().into_owned(), observed);
            }
            // Keep every package/path identity above even when this import
            // does not use the candidate. Runtime parsing cannot contribute a
            // constant without a named value binding for this exact export.
            if !import.bindings.iter().any(|binding| {
                !binding.type_only
                    && binding.kind == ImportKind::Named
                    && binding.imported.as_deref() == Some(candidate_export.export)
            }) {
                continue;
            }
            let browser = browser.get_or_init(|| {
                crate::host_constants::host_constants_of_module(
                    Path::new(file.path.as_str()),
                    &file.source,
                    solid_dialect::HostTargetCondition::Browser,
                    Some(directory),
                )
            });
            let server = server.get_or_init(|| {
                crate::host_constants::host_constants_of_module(
                    Path::new(file.path.as_str()),
                    &file.source,
                    solid_dialect::HostTargetCondition::Node,
                    Some(directory),
                )
            });
            let proved = |constants: &[crate::host_constants::ResolvedHostConstant], expected| {
                constants.iter().any(|constant| {
                    constant.fold.specifier == candidate_export.module
                        && constant.fold.imported == candidate_export.export
                        && constant.package_version == candidate_export.version
                        && constant.fold.value == expected
                        && constant.targets.iter().all(|(target, digest)| {
                            let path = package.join(target);
                            candidate_export.runtime_targets.contains(&target.as_str())
                                && fs::canonicalize(&path).ok().as_deref() == Some(path.as_path())
                                && manifest.inputs.get(path.to_string_lossy().as_ref())
                                    == Some(&format!("sha256:{digest}"))
                        })
                })
            };
            if !proved(browser, candidate_export.client) || !proved(server, candidate_export.server)
            {
                continue;
            }
            // The resolver must select one of the very browser files whose
            // bytes proved this constant. Successful build/start is not a
            // file-selection premise, nor is a declaration in this package.
            if facts.runtime_resolutions.as_ref().is_some_and(|runtime| {
                !matches!(runtime.outcome(file.path.as_str(), import.span, &import.module),
                    solid_facts::runtime_resolution::RuntimeOutcome::File { path, physical_path }
                    if path == physical_path && browser.iter().any(|constant|
                        constant.fold.specifier == candidate_export.module
                            && constant.fold.imported == candidate_export.export
                            && constant.targets.iter().any(|(target, _)| package.join(target) == Path::new(path.as_ref()))))
            }) {
                continue;
            }
            for binding in import.bindings.iter().filter(|binding| {
                !binding.type_only
                    && binding.kind == ImportKind::Named
                    && binding.imported.as_deref() == Some(candidate_export.export)
            }) {
                // Canonical Type Facts identity is required at this exact binding,
                // in addition to the Oxc declaration identity carried by the fact.
                if let Some(symbol) = exact_symbol(facts, file.path.as_str(), binding.local.span)
                    && let Some(symbol) = facts.typescript.symbol(&symbol)
                    && !symbol.declarations().is_empty()
                    && symbol.declarations().iter().all(|declaration| {
                        declaration.name.as_ref() == candidate_export.export
                            && Path::new(declaration.location.path.as_ref()).starts_with(package)
                    })
                {
                    values.insert(
                        binding.local.span,
                        (candidate_export.client, candidate_export.server),
                    );
                }
            }
        }
    }
    values
}

fn static_loads<'a>(
    facts: &'a ProjectFacts,
    file: &'a FileFacts,
) -> impl Iterator<Item = (Span, &'a str, bool)> {
    file.ast
        .imports
        .iter()
        .filter(|import| {
            !import.type_only
                && (import.bindings.is_empty()
                    || import.bindings.iter().any(|binding| !binding.type_only))
        })
        .map(|import| {
            (
                import.span,
                import.module.as_str(),
                runtime_import(facts, file, import),
            )
        })
        .chain(file.ast.exports.iter().filter_map(|export| {
            export
                .module
                .as_deref()
                .filter(|_| {
                    !export.type_only
                        && (export.kind == ExportKind::All
                            || export.namespace.is_some()
                            || export.specifiers.is_empty()
                            || export
                                .specifiers
                                .iter()
                                .any(|specifier| !specifier.type_only))
                })
                .map(|text| (export.span, text, true))
        }))
}

fn execution_regions(
    file: &FileFacts,
    scope: Option<Span>,
    constants: &BTreeMap<Span, (bool, bool)>,
    module_startup: bool,
) -> Vec<HostExecutionRegion> {
    let value = |identity: &HostConstantIdentity, server: bool| match identity {
        // Discovery has already admitted the closed Vite client configuration.
        HostConstantIdentity::ViteSsr => Some(server),
        HostConstantIdentity::Import { declaration } => constants
            .get(declaration)
            .map(|(client, node)| if server { *node } else { *client }),
    };
    file.ast
        .host_execution
        .iter()
        .filter(|fact| {
            fact.scope == scope
                && !matches!(
                    fact.kind,
                    HostExecutionSiteKind::ModuleCompletion
                        | HostExecutionSiteKind::ModuleStartupCompletion
                )
        })
        .map(|fact| HostExecutionRegion {
            span: fact.span,
            client: fact
                .predicate
                .evaluate_with_startup(&|identity| value(identity, false), module_startup),
            server: fact.predicate.evaluate(&|identity| value(identity, true)),
        })
        .collect()
}

fn unwrapped(file: &FileFacts, mut span: Span) -> Span {
    for _ in 0..file.ast.transparent_wrappers.len() {
        let mut wrappers = file
            .ast
            .transparent_wrappers
            .iter()
            .filter(|wrapper| wrapper.span == span);
        let Some(wrapper) = wrappers.next() else {
            break;
        };
        if wrappers.next().is_some() || wrapper.inner == span {
            break;
        }
        span = wrapper.inner;
    }
    span
}

fn direct_identifier(file: &FileFacts, span: Span) -> bool {
    let span = unwrapped(file, span);
    file.ast
        .identifiers
        .iter()
        .any(|identifier| identifier.span == span)
}

fn exact_symbol(facts: &ProjectFacts, path: &str, span: Span) -> Option<String> {
    let mut matches = facts
        .typescript
        .entities_for_path(path)
        .iter()
        .filter(|entity| {
            entity.location.start_byte == u64::from(span.start)
                && entity.location.end_byte == u64::from(span.end)
                && !entity.symbol_unresolved
                && !entity.symbol.is_empty()
        });
    let mut symbol = matches.next()?.symbol.to_string();
    if matches.next().is_some() {
        return None;
    }
    let mut seen = BTreeSet::new();
    loop {
        if !seen.insert(symbol.clone()) {
            return None;
        }
        let record = facts.typescript.symbol(&symbol)?;
        if record.alias_target().is_empty() || record.alias_target() == symbol {
            return Some(symbol);
        }
        symbol = record.alias_target().to_owned();
    }
}

fn normalized(path: &Path) -> Option<PathBuf> {
    let mut result = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !result.pop() {
                    return None;
                }
            }
            other => result.push(other.as_os_str()),
        }
    }
    Some(result)
}

fn paths_targets(
    directory: &Path,
    project: &Path,
) -> Result<BTreeMap<String, Vec<String>>, DiscoveryRefusal> {
    let mut inputs = Vec::new();
    tsconfig_inputs(project, &mut inputs, &mut BTreeSet::new())?;
    let mut base = directory.to_path_buf();
    let mut paths = BTreeMap::new();
    // tsconfig_inputs records the derived config before its base.
    for input in inputs.into_iter().rev() {
        let source = fs::read_to_string(&input)
            .ok()
            .required(&input, "cannot read inherited path mappings")?;
        let value = host_config::jsonc(&source).required(&input, "invalid path-mapping JSONC")?;
        let Some(options) = value.get("compilerOptions") else {
            continue;
        };
        if let Some(value) = options.get("baseUrl") {
            base = normalized(
                &input
                    .parent()
                    .required(&input, "path-mapping config has no parent")?
                    .join(value.as_str().required(&input, "baseUrl is not a string")?),
            )
            .required(&input, "baseUrl escapes its filesystem root")?;
        }
        if let Some(value) = options.get("paths") {
            paths.clear();
            let origin = if options.get("baseUrl").is_some() || base != directory {
                &base
            } else {
                input
                    .parent()
                    .required(&input, "paths config has no parent")?
            };
            for (key, values) in value
                .as_object()
                .required(&input, "paths is not an object")?
            {
                let targets = values
                    .as_array()
                    .required(&input, "paths target list is not an array")?
                    .iter()
                    .map(|value| {
                        let target = normalized(
                            &origin.join(
                                value
                                    .as_str()
                                    .required(&input, "paths target is not a string")?,
                            ),
                        )
                        .required(&input, "paths target escapes its filesystem root")?;
                        Ok(target
                            .to_str()
                            .required(&input, "non-UTF-8 paths target")?
                            .to_owned())
                    })
                    .collect::<Result<Vec<_>, DiscoveryRefusal>>()?;
                paths.insert(key.clone(), targets);
            }
        }
    }
    Ok(paths)
}

fn congruent_alias(
    config: &Config,
    paths: &BTreeMap<String, Vec<String>>,
    specifier: &str,
) -> bool {
    let aliases = config
        .aliases
        .iter()
        .filter(|(find, _)| {
            specifier == find.as_str()
                || specifier
                    .strip_prefix(find.as_str())
                    .is_some_and(|tail| tail.starts_with('/'))
        })
        .collect::<Vec<_>>();
    if aliases.is_empty() {
        return true;
    }
    // Vite's explicit alias plugin precedes user pre-plugins, including
    // vite-tsconfig-paths. Its presence cannot authorize a conflicting rewrite.
    // Multiple matching Vite entries require ordering/priority facts, not guessing.
    let [(find, Some(replacement))] = aliases.as_slice() else {
        return false;
    };
    if !Path::new(replacement).is_absolute() {
        return false;
    }
    let Some(vite) = normalized(Path::new(&format!(
        "{replacement}{}",
        &specifier[find.len()..]
    ))) else {
        return false;
    };
    let targets = paths
        .iter()
        .filter_map(|(pattern, targets)| {
            if pattern == specifier {
                return Some((pattern.len(), targets, None));
            }
            let (prefix, suffix) = pattern.split_once('*')?;
            if suffix.contains('*') {
                return None;
            }
            let tail = specifier.strip_prefix(prefix)?.strip_suffix(suffix)?;
            Some((prefix.len(), targets, Some(tail)))
        })
        .collect::<Vec<_>>();
    let Some(longest) = targets.iter().map(|(length, _, _)| *length).max() else {
        // No TS paths rewrite competes with this literal Vite alias. The
        // exact_local_target join still must agree with the resolved file.
        return true;
    };
    let mut selected = targets.iter().filter(|(length, _, _)| *length == longest);
    let Some((_, targets, tail)) = selected.next() else {
        return false;
    };
    if selected.next().is_some() {
        return false;
    }
    let [target] = targets.as_slice() else {
        return false;
    };
    let target = match tail {
        Some(tail) if target.matches('*').count() == 1 => target.replace('*', tail),
        None if !target.contains('*') => target.clone(),
        _ => return false,
    };
    normalized(Path::new(&target)).is_some_and(|target| target == vite)
}

// TypeScript resolution is not a runtime-resolution attestation. In particular,
// extension substitution and tsconfig-only paths can select another file than
// Vite. After the already-congruence-checked literal alias, admit an exact file
// or the first ordinary Vite extension/index probe that agrees with Type
// Facts. Directory package entries require a separate package-resolution proof.
// Audited installed Vite 8.3.0: chunks/node.js DEFAULT_EXTENSIONS and
// tryCleanFsResolve/tryResolveRealFileWithExtensions. Vite 6/7 use the same list.
const VITE_DEFAULT_EXTENSIONS: &[&str] = &[".mjs", ".js", ".mts", ".ts", ".jsx", ".tsx", ".json"];

fn probe_metadata(path: &Path) -> Option<Option<fs::Metadata>> {
    match fs::metadata(path) {
        Ok(metadata) => Some(Some(metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // A dangling symlink is not absence authority.
            match fs::symlink_metadata(path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(None),
                _ => None,
            }
        }
        Err(_) => None,
    }
}

fn default_local_file(config: &Config, path: &Path) -> Option<PathBuf> {
    let path = normalized(path)?;
    if path.extension().is_some() {
        return path
            .extension()
            .is_some_and(|extension| {
                ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"]
                    .iter()
                    .any(|allowed| extension == *allowed)
            })
            .then_some(path);
    }
    let metadata = probe_metadata(&path)?;
    if !config.default_extensions || metadata.as_ref().is_some_and(fs::Metadata::is_file) {
        return None;
    }
    for extension in VITE_DEFAULT_EXTENSIONS {
        let candidate = PathBuf::from(format!("{}{extension}", path.to_str()?));
        if probe_metadata(&candidate)?
            .as_ref()
            .is_some_and(fs::Metadata::is_file)
        {
            // Vite selects the first existing file. A later candidate cannot
            // make that choice ambiguous; the Type Facts target must still
            // equal this exact selected canonical file.
            return (fs::canonicalize(&candidate).ok().as_deref() == Some(candidate.as_path()))
                .then_some(candidate);
        }
    }
    if metadata.as_ref().is_some_and(fs::Metadata::is_dir) {
        // Vite attempts package.json before index. Never silently skip it,
        // including broken/symlink manifests or unknown conditional exports.
        match fs::symlink_metadata(path.join("package.json")) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => return None,
        }
        for extension in VITE_DEFAULT_EXTENSIONS {
            let candidate = path.join(format!("index{extension}"));
            if probe_metadata(&candidate)?
                .as_ref()
                .is_some_and(fs::Metadata::is_file)
            {
                return (fs::canonicalize(&candidate).ok().as_deref() == Some(candidate.as_path()))
                    .then_some(candidate);
            }
        }
    }
    None
}

fn exact_local_target(config: &Config, importer: &str, text: &str, target: &str) -> bool {
    if text.contains(['?', '#', '%', '\\']) {
        return false;
    }
    let matching = config
        .aliases
        .iter()
        .filter(|(find, _)| {
            text == find.as_str()
                || text
                    .strip_prefix(find.as_str())
                    .is_some_and(|tail| tail.starts_with('/'))
        })
        .collect::<Vec<_>>();
    let runtime = match matching.as_slice() {
        [] if text.starts_with('.') => Path::new(importer).parent().map(|parent| parent.join(text)),
        [(find, Some(replacement))] => Some(PathBuf::from(format!(
            "{replacement}{}",
            &text[find.len()..]
        ))),
        _ => None,
    };
    runtime
        .and_then(|path| default_local_file(config, &path))
        .is_some_and(|path| {
            path == Path::new(target)
                && fs::canonicalize(&path).ok().as_deref() == Some(path.as_path())
        })
}

/// ADR 0220 is optional. A requested but missing/disagreeing row is unknown,
/// never permission to reuse the default congruence guess.
fn local_edge(
    facts: &ProjectFacts,
    config: &Config,
    paths: &BTreeMap<String, Vec<String>>,
    importer: &str,
    span: Span,
    text: &str,
    target: &str,
) -> bool {
    if let Some(runtime) = &facts.runtime_resolutions {
        return matches!(runtime.outcome(importer, span, text),
            solid_facts::runtime_resolution::RuntimeOutcome::File { path, physical_path }
            if path.as_ref() == target && physical_path.as_ref() == target
                && fs::canonicalize(target).ok().as_deref() == Some(Path::new(target)));
    }
    !resolver_selection_required(config, text)
        && congruent_alias(config, paths, text)
        && exact_local_target(config, importer, text, target)
}

fn body_symbol(
    facts: &ProjectFacts,
    file: &FileFacts,
    function: &FunctionFact,
    written_symbols: &BTreeSet<String>,
) -> Option<String> {
    if function.method_name.is_some()
        || function.generator
        || file
            .ast
            .classes
            .iter()
            .any(|class| class.span.contains(function.span))
        || file
            .ast
            .module_blocks
            .iter()
            .any(|block| block.contains(function.span))
        || file
            .ast
            .module_hazards
            .iter()
            .any(|hazard| hazard.kind != ModuleHazardKind::NonliteralDynamicLoading)
    {
        return None;
    }
    let name = function.name.as_ref().or_else(|| {
        file.ast
            .bindings
            .iter()
            .find(|binding| {
                binding.immutable
                    && binding.initializer_function
                    && binding.names.len() == 1
                    && binding
                        .initializer
                        .is_some_and(|span| unwrapped(file, span) == function.span)
            })
            .and_then(|binding| binding.names.first())
    })?;
    let symbol = exact_symbol(facts, file.path.as_str(), name.span)?;
    // Exact writes invalidate declaration-to-runtime identity, even in other files.
    if written_symbols.contains(&symbol) {
        return None;
    }
    Some(symbol)
}

fn literal_body(file: &FileFacts, span: Span) -> Option<&FunctionFact> {
    let span = unwrapped(file, span);
    let mut functions = file.ast.functions.iter().filter(|function| {
        function.span == span
            && function.kind != FunctionKind::Declaration
            && function.method_name.is_none()
            && !function.generator
    });
    let result = functions.next()?;
    functions.next().is_none().then_some(result)
}

fn value_reference(facts: &ProjectFacts, file: &FileFacts, span: Span) -> bool {
    if file
        .ast
        .type_queries
        .iter()
        .any(|query| query.contains(span))
    {
        return false;
    }
    let mut matches = facts
        .typescript
        .entities_for_path(file.path.as_str())
        .iter()
        .filter(|entity| {
            entity.location.start_byte == u64::from(span.start)
                && entity.location.end_byte == u64::from(span.end)
        });
    let Some(entity) = matches.next() else {
        return false;
    };
    matches.next().is_none()
        && matches!(
            entity.reference_space,
            Some(typefacts::ReferenceSpace::Value | typefacts::ReferenceSpace::Both)
        )
}

fn import_references_are_client_only(ast: &solid_facts::ast::AstFacts, binding: Span) -> bool {
    // ReferenceSpace and runtime_referenced summarize a binding's uses. A
    // value use inside a removed server function cannot authenticate a type
    // use outside it. Mixed client/server bindings need per-use lowering
    // facts; do not infer the surviving import from the aggregate.
    !ast.reference_declarations
        .iter()
        .any(|(reference, declaration)| {
            *declaration == binding
                && ast.functions.iter().any(|function| {
                    function.span.contains(*reference) && function.has_directive("use server")
                })
        })
}

fn runtime_import(facts: &ProjectFacts, file: &FileFacts, import: &ImportFact) -> bool {
    !import.type_only
        && (import.bindings.is_empty()
            || import.bindings.iter().any(|binding| {
                if binding.type_only {
                    return false;
                }
                if binding.kind == ImportKind::SideEffect {
                    return true;
                }
                binding.runtime_referenced
                    && import_references_are_client_only(&file.ast, binding.local.span)
                    && file
                        .ast
                        .reference_declarations
                        .iter()
                        .any(|(reference, declaration)| {
                            *declaration == binding.local.span
                                && value_reference(facts, file, *reference)
                                && !file.ast.functions.iter().any(|function| {
                                    function.span.contains(*reference)
                                        && function.has_directive("use server")
                                })
                        })
            }))
}

// A resolved file can still fail ESM linking if a named value export is absent.
// Direct authored exports are proved here; indirect surfaces retain baseline
// until their exact reexport/binding trail has been authenticated.
fn explicit_static_surface(
    facts: &ProjectFacts,
    file: &FileFacts,
    span: Span,
    target: &FileFacts,
) -> bool {
    let mut named = file
        .ast
        .imports
        .iter()
        .filter(|import| import.span == span)
        .flat_map(|import| &import.bindings)
        .filter(|binding| {
            !binding.type_only && matches!(binding.kind, ImportKind::Named | ImportKind::Default)
        })
        .map(|binding| (binding.imported.as_deref(), binding.local.span))
        .chain(
            file.ast
                .exports
                .iter()
                .filter(|export| export.span == span)
                .flat_map(|export| &export.specifiers)
                .filter(|specifier| !specifier.type_only)
                .map(|specifier| (file.source_text(specifier.local.span), specifier.local.span)),
        );
    named.all(|(name, source)| {
        let (Some(name), Some(symbol)) = (name, exact_symbol(facts, file.path.as_str(), source))
        else {
            return false;
        };
        target
            .ast
            .module_level_exports()
            .filter(|export| export.module.is_none() && !export.type_only)
            .flat_map(|export| export.declarations.iter().chain(&export.specifiers))
            .any(|export| {
                !export.type_only
                    && export.exported == name
                    && (exact_symbol(facts, target.path.as_str(), export.local.span).as_deref()
                        == Some(symbol.as_str())
                        || (facts
                            .typescript
                            .entities_for_path(target.path.as_str())
                            .iter()
                            .all(|entity| {
                                entity.location.start_byte != u64::from(export.local.span.start)
                                    || entity.location.end_byte != u64::from(export.local.span.end)
                            })
                            && facts.typescript.symbol(&symbol).is_some_and(|record| {
                                // An exported binding need not have its own entity
                                // row. Join the canonical symbol's one declaration
                                // exactly; never use containment or spelling.
                                let [declaration] = record.declarations() else {
                                    return false;
                                };
                                declaration.location.path.as_ref() == target.path.as_str()
                                    && declaration.location.start_byte
                                        == u64::from(export.local.span.start)
                                    && declaration.location.end_byte
                                        == u64::from(export.local.span.end)
                            })))
            })
    })
}

fn empty_graph_refusal(
    facts: &ProjectFacts,
    config: &Config,
    roots: &[PathBuf],
    modules: &[HostModule],
    loads: &BTreeMap<(String, String), &str>,
) -> DiscoveryRefusal {
    let mut pending = roots
        .iter()
        .filter_map(|path| path.to_str().map(str::to_owned))
        .collect::<BTreeSet<_>>();
    let mut visited = BTreeSet::new();
    while let Some(path) = pending.pop_first() {
        if !visited.insert(path.clone()) {
            continue;
        }
        let Some(module) = modules.iter().find(|module| module.path == path) else {
            return DiscoveryRefusal::new(
                Path::new(&path),
                "root/dependency absent from analyzed module inventory",
            );
        };
        pending.extend(module.static_dependencies.iter().cloned());
        if module.completion == Some(false) {
            return DiscoveryRefusal::new(
                Path::new(&path),
                "certain top-level exit contradicts the app-starts premise; browser root withheld",
            );
        }
        if let Some(((path, text), reason)) =
            loads.iter().find(|((importer, _), _)| importer == &path)
        {
            let file = facts.files.iter().find(|file| file.path.as_str() == path);
            let offset = file
                .and_then(|file| {
                    file.ast
                        .imports
                        .iter()
                        .find(|import| import.module.as_str() == text)
                        .map(|import| import.span.start)
                        .or_else(|| {
                            file.ast
                                .exports
                                .iter()
                                .find(|export| export.module.as_deref() == Some(text.as_str()))
                                .map(|export| export.span.start)
                        })
                })
                .unwrap_or(0) as usize;
            let source = fs::read_to_string(path).unwrap_or_default();
            return DiscoveryRefusal::at(
                Path::new(path),
                &source,
                offset,
                format!("{reason}: {text}"),
            );
        }
        if module.refused {
            let file = facts.files.iter().find(|file| file.path.as_str() == path);
            let source_cause = file.and_then(|file| {
                file.ast
                    .module_directives
                    .iter()
                    .find(|directive| directive.value == "use server")
                    .map(|directive| (directive.span, "server-only module directive"))
                    .or_else(|| {
                        file.ast
                            .module_hazards
                            .iter()
                            .find(|hazard| {
                                hazard.kind != ModuleHazardKind::NonliteralDynamicLoading
                            })
                            .map(|hazard| (hazard.span, "opaque module evaluation hazard"))
                    })
                    .or_else(|| {
                        file.ast
                            .module_loads
                            .iter()
                            .find(|load| load.kind == ModuleLoadKind::Require)
                            .map(|load| {
                                (
                                    load.span,
                                    "CommonJS execution is not attested by the ESM root",
                                )
                            })
                    })
                    .or_else(|| {
                        file.ast
                            .import_equals
                            .iter()
                            .find(|import| !import.type_only)
                            .map(|import| {
                                (
                                    import.span,
                                    "CommonJS execution is not attested by the ESM root",
                                )
                            })
                    })
            });
            if let Some((span, reason)) = source_cause {
                let source = fs::read_to_string(&path).unwrap_or_default();
                return DiscoveryRefusal::at(
                    Path::new(&path),
                    &source,
                    span.start as usize,
                    reason,
                );
            }
            let reason = if !Path::new(&path).starts_with(
                Path::new(&facts.project_id)
                    .parent()
                    .unwrap_or(Path::new(".")),
            ) {
                "module lies outside the application"
            } else if file.is_some_and(|file| {
                file.ast
                    .module_directives
                    .iter()
                    .any(|directive| directive.value == "use server")
            }) {
                "server-only module directive"
            } else if file.is_some_and(|file| {
                file.ast
                    .module_hazards
                    .iter()
                    .any(|hazard| hazard.kind != ModuleHazardKind::NonliteralDynamicLoading)
            }) {
                "opaque module evaluation hazard"
            } else if file.is_some_and(|file| {
                file.ast
                    .module_loads
                    .iter()
                    .any(|load| load.kind == ModuleLoadKind::Require)
                    || file
                        .ast
                        .import_equals
                        .iter()
                        .any(|import| !import.type_only)
            }) {
                "CommonJS execution is not attested by the ESM root"
            } else if facts
                .resolved_imports
                .as_ref()
                .is_none_or(|rows| !rows.covers(&path))
            {
                "missing import-resolution inventory for module"
            } else if let Some(package) = Path::new(&path)
                .parent()
                .into_iter()
                .flat_map(Path::ancestors)
                .take_while(|ancestor| Some(*ancestor) != Path::new(&facts.project_id).parent())
                .map(|ancestor| ancestor.join("package.json"))
                .find(|path| path.exists())
            {
                return DiscoveryRefusal::new(&package, "nested package execution boundary");
            } else if [".d.ts", ".d.mts", ".d.cts"]
                .iter()
                .any(|suffix| path.ends_with(suffix))
            {
                "declaration-only module has no runtime implementation"
            } else if Path::new(&path)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(|stem| ["entry-server", "middleware"].contains(&stem))
            {
                "server or middleware entry has a separate execution model"
            } else if config.mode == EntryMode::ClientStart
                && Path::new(&path)
                    .file_stem()
                    .is_some_and(|stem| stem == "Document")
            {
                "generated document is not a client-executed root"
            } else if file.is_some_and(|file| {
                route_file(
                    Path::new(&facts.project_id)
                        .parent()
                        .unwrap_or(Path::new(".")),
                    config,
                    file,
                )
            }) {
                "route client-only export/default callable model is unproved"
            } else {
                "module refusal has no recorded load or execution premise"
            };
            return DiscoveryRefusal::new(Path::new(&path), reason);
        }
        for dependency in &module.static_dependencies {
            let completion = modules
                .iter()
                .find(|module| &module.path == dependency)
                .and_then(|module| module.completion);
            if completion != Some(true) {
                return DiscoveryRefusal::new(
                    Path::new(dependency),
                    if completion == Some(false) {
                        "certain top-level exit contradicts the app-starts premise; browser root withheld"
                    } else {
                        "dependency initialization completion is unproved"
                    },
                );
            }
        }
    }
    DiscoveryRefusal::new(
        Path::new(&facts.project_id),
        "cyclic static initialization prevents browser scope admission",
    )
}

struct DiscoveryInputs {
    directory: PathBuf,
    config: Config,
    paths: BTreeMap<String, Vec<String>>,
    config_inputs: Vec<PathBuf>,
}

/// Browser object maps can select bare requests, resolved relative files,
/// extension probes and false modules. Until that dispatch is modeled exactly,
/// any nonempty enclosing object map withholds every default request selection.
/// String browser entry points do not rewrite the importer's requests.
fn browser_selection_required(importer: &Path, manifest: &mut BrowserRootManifest) -> bool {
    let Some(parent) = importer.parent() else {
        return true;
    };
    for ancestor in parent.ancestors() {
        let path = ancestor.join("package.json");
        let Some(observed) = graph_identity(manifest, &path) else {
            return true;
        };
        if observed == "absent" {
            continue;
        }
        // Pointer identity is not target content. Do not consume mutable
        // browser-map bytes behind a link as default selection authority.
        if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_symlink()) {
            return true;
        }
        let Some(package) = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        else {
            return true;
        };
        if let Some(browser) = package.get("browser") {
            match browser {
                serde_json::Value::Object(map) if map.is_empty() => {}
                serde_json::Value::String(_) | serde_json::Value::Null => {}
                _ => return true,
            }
        }
    }
    false
}

fn resolver_selection_required(config: &Config, request: &str) -> bool {
    config.tsconfig_paths
        && (!(request.starts_with("./") || request.starts_with("../") || request.starts_with('/'))
            || config.aliases.keys().any(|alias| {
                request == alias
                    || request
                        .strip_prefix(alias)
                        .is_some_and(|tail| tail.starts_with('/'))
            }))
}

fn package_selection_agrees(
    facts: &ProjectFacts,
    importer: &str,
    span: Span,
    text: &str,
    entries: &[PathBuf],
    resolver_active: bool,
) -> bool {
    facts.runtime_resolutions.as_ref().map_or(!resolver_active, |runtime| {
        matches!(runtime.outcome(importer, span, text),
            solid_facts::runtime_resolution::RuntimeOutcome::File { path, physical_path }
            if path == physical_path && entries.iter().any(|entry| Path::new(path.as_ref()) == entry.as_path()
                && fs::canonicalize(entry).ok().as_deref() == Some(entry.as_path())))
    })
}

fn discovery_inputs(
    directory: &Path,
    facts: &ProjectFacts,
    dialect: &dyn solid_dialect::Dialect,
    fixture_authority: bool,
) -> Result<DiscoveryInputs, DiscoveryRefusal> {
    let _stage = HostStage::new("host-discovery-config");
    if !dialect.models_server_functions() {
        return Err(DiscoveryRefusal::new(
            directory,
            "dialect has no server-function execution model",
        ));
    }
    let directory = fs::canonicalize(directory)
        .ok()
        .required(directory, "cannot resolve application directory")?;
    let project = fs::canonicalize(&facts.project_id).ok().required(
        Path::new(&facts.project_id),
        "cannot resolve analyzed tsconfig",
    )?;
    if project.parent() != Some(directory.as_path()) || !project.is_file() {
        return Err(DiscoveryRefusal::new(
            Path::new(&facts.project_id),
            "analyzed tsconfig is not a file owned by this application directory",
        ));
    }
    let mut paths_observed = Vec::new();
    application_inputs(&directory, &project, &mut paths_observed)?;
    paths_observed.push(directory.join(".solid-checker/shared-host-plugin.json"));
    // Refusal consumes only its witness. No app inventory, request parsing,
    // or manifest hashing is needed before the configuration can be admitted.
    let config = configuration(&directory, &mut paths_observed, fixture_authority)?;
    let paths = paths_targets(&directory, &project)?;
    Ok(DiscoveryInputs {
        directory,
        config,
        paths,
        config_inputs: paths_observed,
    })
}

#[cfg(test)]
fn discovered_index(
    directory: &Path,
    facts: &ProjectFacts,
    dialect: &dyn solid_dialect::Dialect,
    fixture_authority: bool,
    packages: Option<&AcceptedContractIndex>,
) -> Result<ProjectHostIndex, DiscoveryRefusal> {
    let inputs = discovery_inputs(directory, facts, dialect, fixture_authority)?;
    let mut index = discovered_index_with_inputs(&inputs, facts, dialect, packages)?;
    if !index.is_empty() {
        attach_discovery_inputs(
            &mut index,
            &inputs,
            facts,
            dialect,
            fixture_authority,
            packages,
        )?;
    }
    Ok(index)
}

fn attach_discovery_inputs(
    index: &mut ProjectHostIndex,
    prepared: &DiscoveryInputs,
    facts: &ProjectFacts,
    dialect: &dyn solid_dialect::Dialect,
    fixture_authority: bool,
    packages: Option<&AcceptedContractIndex>,
) -> Result<(), DiscoveryRefusal> {
    let _stage = HostStage::new("host-discovery-replay");
    let directory = &prepared.directory;
    let inputs = input_manifest(
        directory,
        facts,
        &prepared.config,
        prepared.config_inputs.clone(),
    )?;
    // A graph may refuse without ever needing a full manifest. On success,
    // recheck the earlier semantic premises against this input generation.
    // Never overwrite a fingerprint already observed while constructing edges.
    for (path, identity) in inputs {
        if index
            .manifest
            .inputs
            .get(&path)
            .is_some_and(|observed| observed != &identity)
        {
            return Err(DiscoveryRefusal::new(
                Path::new(&path),
                "graph inputs changed; retry analysis",
            ));
        }
        index.manifest.inputs.insert(path, identity);
    }
    let project = Path::new(&facts.project_id);
    let config_replay = HostStage::new("host-discovery-config-replay");
    let mut current_inputs = Vec::new();
    application_inputs(directory, project, &mut current_inputs)?;
    current_inputs.push(directory.join(".solid-checker/shared-host-plugin.json"));
    let current = configuration_in_generation(
        directory,
        &mut current_inputs,
        fixture_authority,
        Some(&index.manifest.inputs),
    )?;
    let paths = paths_targets(directory, project)?;
    current_inputs.sort();
    current_inputs.dedup();
    let mut previous_inputs = prepared.config_inputs.clone();
    previous_inputs.sort();
    previous_inputs.dedup();
    if current != prepared.config || paths != prepared.paths || current_inputs != previous_inputs {
        return Err(DiscoveryRefusal::new(
            directory,
            "configuration inputs changed; retry analysis",
        ));
    }
    drop(config_replay);
    // Probe decisions such as an absent nested package or Vite extension
    // precedence must also be made *after* the full input generation was
    // captured. Rebuild scopes now; the preliminary graph only selected
    // whether this work was necessary, and supplies no final authority.
    let mut current = discovered_index_with_inputs(prepared, facts, dialect, packages)?;
    for (path, identity) in &index.manifest.inputs {
        if current
            .manifest
            .inputs
            .get(path)
            .is_some_and(|observed| observed != identity)
        {
            return Err(DiscoveryRefusal::new(
                Path::new(path),
                "graph inputs changed; retry analysis",
            ));
        }
        current
            .manifest
            .inputs
            .insert(path.clone(), identity.clone());
    }
    *index = current;
    Ok(())
}

fn discovered_index_with_inputs(
    prepared: &DiscoveryInputs,
    facts: &ProjectFacts,
    dialect: &dyn solid_dialect::Dialect,
    // None is provisional only. The final pass enforces static package loads
    // independently of whether the behavior index has a contract for them.
    packages: Option<&AcceptedContractIndex>,
) -> Result<ProjectHostIndex, DiscoveryRefusal> {
    let _stage = HostStage::new(if packages.is_some() {
        "host-discovery-admitted"
    } else {
        "host-discovery-provisional"
    });
    let directory = prepared.directory.clone();
    let config = prepared.config.clone();
    let paths = prepared.paths.clone();
    let inputs = BTreeMap::new();
    let mut root_files = Vec::new();
    let mut default_body = false;
    match config.mode {
        EntryMode::ClientStart => {
            let (root, generated) = client_root(|path| directory.join(path).exists()).required(
                &directory,
                "no audited client entry (src/entry-client or src/App)",
            )?;
            root_files.push(directory.join(root));
            default_body = generated;
        }
        EntryMode::Html => {
            let html_path = directory.join("index.html");
            let source = fs::read_to_string(&html_path)
                .ok()
                .required(&html_path, "cannot read HTML entry")?;
            for entry in html_entries_at(&html_path, &source)? {
                let path = directory.join(entry);
                if fs::canonicalize(&path)
                    .ok()
                    .required(&path, "missing HTML module entry")?
                    != path
                    || !path.starts_with(&directory)
                {
                    return Err(DiscoveryRefusal::new(
                        &path,
                        "HTML entry escapes the application or follows a symlink",
                    ));
                }
                root_files.push(path);
            }
        }
    }
    let resolutions = facts
        .resolved_imports
        .as_ref()
        .required(&directory, "missing Type Facts import-resolution inventory")?;
    // Select only exact static JS/TS edges. Loadability, export surfaces,
    // hazards and cycles still independently gate the final root admission.
    // Dynamic imports and type-only references never enlarge this premise.
    let mut pending = root_files
        .iter()
        .filter_map(|path| path.to_str().map(str::to_owned))
        .collect::<BTreeSet<_>>();
    let mut startup_modules = BTreeSet::new();
    while let Some(path) = pending.pop_first() {
        if !startup_modules.insert(path.clone()) {
            continue;
        }
        let Some(file) = facts.files.iter().find(|file| file.path.as_str() == path) else {
            continue;
        };
        for (span, text, runtime) in static_loads(facts, file) {
            if !runtime {
                // An unused/type-space/server-erased value binding does not
                // establish startup evaluation, even if the same module is
                // dynamically imported in a later turn.
                continue;
            }
            let SpecifierAttestation::Attested(row) = resolutions.specifier(&path, span, text)
            else {
                continue;
            };
            let target = row.resolved_path.as_ref();
            if matches!(
                row.resolution,
                ImportResolution::Relative | ImportResolution::NonRelative
            ) && row.symlink_path.is_empty()
                && row.included_path.is_empty()
                && !text.contains(['?', '#', '%', '\\'])
                && !crate::host_loadable::resource_request(text)
                && local_edge(facts, &config, &paths, &path, span, text, target)
                && facts.files.iter().any(|file| file.path.as_str() == target)
                && fs::canonicalize(target).ok().as_deref() == Some(Path::new(target))
            {
                pending.insert(target.to_owned());
            }
        }
    }
    let mut manifest = BrowserRootManifest {
        inputs,
        reason: match config.mode {
            EntryMode::ClientStart => {
                "inferred browser: audited Solid client start entry probe order"
            }
            EntryMode::Html => "inferred browser: Vite index.html module entry",
        }
        .into(),
        ..BrowserRootManifest::default()
    };
    manifest.runtime_resolution = facts.runtime_resolutions.as_ref().map(|index| {
        let rows = serde_json::to_vec(&index.identity_rows()).expect("runtime rows serialize");
        format!("sha256:{:x}", Sha256::digest(rows))
    });
    let mut modules = Vec::new();
    let mut scopes = BTreeMap::<String, HostScope>::new();
    let mut functions = BTreeMap::<String, String>::new();
    let mut blocked_imports = BTreeSet::new();
    let mut unproven_semantics = BTreeSet::new();
    let mut withheld_loads = BTreeMap::new();
    let mut package_loads = crate::host_loadable::PackageLoads::default();
    let scope_selection = HostStage::new("scope-host-view-selection");
    let mut constants_by_file = BTreeMap::new();
    // Writes are generation facts, independent of the function being checked.
    // Resolve them once rather than joining every function with every write.
    let written_symbols = facts
        .files
        .iter()
        .flat_map(|file| {
            file.ast
                .assignments
                .iter()
                .filter_map(|write| exact_symbol(facts, file.path.as_str(), write.target))
        })
        .collect::<BTreeSet<_>>();
    for file in &facts.files {
        if file.ast.schema != AST_FACTS_SCHEMA {
            return Err(DiscoveryRefusal::new(
                Path::new(file.path.as_str()),
                "unsupported AST fact schema",
            ));
        }
        let constants = execution_constants(prepared, facts, file, dialect, &mut manifest);
        // Contents with separate execution models cannot inherit initialization.
        for region in file
            .ast
            .classes
            .iter()
            .map(|class| class.span)
            .chain(file.ast.module_blocks.iter().copied())
        {
            let id = format!("{}#region:{}:{}", file.path, region.start, region.end);
            scopes.insert(
                id.clone(),
                HostScope {
                    id,
                    path: file.path.to_string(),
                    span: Some(region),
                    refused: true,
                    ..HostScope::default()
                },
            );
        }
        for function in &file.ast.functions {
            if let Some(symbol) = body_symbol(facts, file, function, &written_symbols)
                && functions
                    .insert(symbol, scope_id(file.path.as_str(), Some(function)))
                    .is_some()
            {
                return Err(DiscoveryRefusal::new(
                    Path::new(file.path.as_str()),
                    "ambiguous callable symbol has multiple bodies",
                ));
            }
        }
        for function in std::iter::once(None).chain(file.ast.functions.iter().map(Some)) {
            let id = scope_id(file.path.as_str(), function);
            scopes.insert(
                id.clone(),
                HostScope {
                    id,
                    path: file.path.to_string(),
                    span: function.map(|function| function.span),
                    refused: function.is_some_and(|function| {
                        function.generator
                            || file
                                .ast
                                .classes
                                .iter()
                                .any(|class| class.span.contains(function.span))
                            || file
                                .ast
                                .module_blocks
                                .iter()
                                .any(|block| block.contains(function.span))
                            || file.ast.functions.iter().any(|ancestor| {
                                ancestor.span.contains(function.span)
                                    && ancestor.has_directive("use server")
                            })
                    }),
                    execution: execution_regions(
                        file,
                        function.map(|function| function.span),
                        &constants,
                        function.is_none() && startup_modules.contains(file.path.as_str()),
                    ),
                    ..HostScope::default()
                },
            );
        }
        for default in &file.ast.host_defaults {
            let id = default_scope_id(file.path.as_str(), default);
            let refused = default.argument_index.is_none()
                || scopes
                    .get(&format!(
                        "{}#{}:{}",
                        file.path, default.function.start, default.function.end
                    ))
                    .is_none_or(|scope| scope.refused);
            scopes.insert(
                id.clone(),
                HostScope {
                    id,
                    path: file.path.to_string(),
                    span: Some(default.span),
                    refused,
                    execution: execution_regions(file, Some(default.span), &constants, false),
                    ..HostScope::default()
                },
            );
        }
        constants_by_file.insert(file.path.as_str(), constants);
    }
    drop(scope_selection);
    for file in &facts.files {
        let path = file.path.as_str();
        let is_root = root_files.iter().any(|root| root.to_str() == Some(path));
        let filename = Path::new(path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .required(Path::new(path), "source has no UTF-8 file stem")?;
        let mut module = HostModule {
            path: path.into(),
            static_dependencies: BTreeSet::new(),
            completion: file
                .ast
                .host_execution
                .iter()
                .find(|fact| {
                    fact.kind
                        == if startup_modules.contains(path) {
                            HostExecutionSiteKind::ModuleStartupCompletion
                        } else {
                            HostExecutionSiteKind::ModuleCompletion
                        }
                        && fact.scope.is_none()
                })
                .and_then(|fact| {
                    let constants = &constants_by_file[path];
                    fact.predicate.evaluate_with_startup(
                        &|identity| match identity {
                            HostConstantIdentity::ViteSsr => Some(false),
                            HostConstantIdentity::Import { declaration } => {
                                constants.get(declaration).map(|(client, _)| *client)
                            }
                        },
                        startup_modules.contains(path),
                    )
                }),
            refused: !Path::new(path).starts_with(&directory)
                || [".d.ts", ".d.mts", ".d.cts"]
                    .iter()
                    .any(|suffix| path.ends_with(suffix))
                || !resolutions.covers(path)
                || file
                    .ast
                    .module_directives
                    .iter()
                    .any(|directive| directive.value == "use server")
                || ["entry-server", "middleware"].contains(&filename)
                || (config.mode == EntryMode::ClientStart && filename == "Document"),
        };
        if startup_modules.contains(path) && module.completion == Some(false) {
            module.refused = true;
        }
        if file
            .ast
            .module_hazards
            .iter()
            .any(|hazard| hazard.kind != ModuleHazardKind::NonliteralDynamicLoading)
        {
            // Opaque evaluation (notably eval) can replace a later local call
            // target. Withholding only this file's findings cannot authenticate
            // execution of a separately analyzed callee reached through it.
            unproven_semantics.insert(path.into());
            module.refused = true;
        }
        if file
            .ast
            .module_loads
            .iter()
            .any(|load| load.kind == ModuleLoadKind::Require)
            || file
                .ast
                .import_equals
                .iter()
                .any(|import| !import.type_only)
        {
            // Authored CommonJS lowering/host availability is not attested by
            // the admitted ESM roots. A Require syntax fact is unknown here,
            // including a shadowed implementation; never guess a Node host.
            module.refused = true;
            unproven_semantics.insert(path.into());
        }
        for ancestor in Path::new(path)
            .parent()
            .required(Path::new(path), "source has no parent directory")?
            .ancestors()
            .take_while(|ancestor| *ancestor != directory.as_path())
        {
            if ancestor.join("package.json").exists() {
                module.refused = true;
            }
        }
        let is_route = route_file(&directory, &config, file);
        if is_route && (server_route(&file.ast) || default_scope(file, facts, &functions).is_none())
        {
            // Picked mixed-route lowering needs separate compiler authority.
            module.refused = true;
        }
        let mut allowed_functions = functions
            .iter()
            .filter(|(_, id)| scopes.get(*id).is_some_and(|scope| scope.path == path))
            .map(|(symbol, _)| symbol.clone())
            .collect::<BTreeSet<_>>();
        for (span, text, runtime) in static_loads(facts, file) {
            // Resolver activity is app-wide. Do not approximate tsconfig
            // discovery, inheritance, tokens, patterns or negative lookups.
            // Only ADR 0220 can authenticate bare/aliased file selection.
            let browser_map = browser_selection_required(Path::new(path), &mut manifest);
            if (resolver_selection_required(&config, text) || browser_map)
                && facts.runtime_resolutions.is_none()
            {
                module.refused = true;
                blocked_imports.insert((path.into(), text.into()));
                unproven_semantics.insert(path.into());
                withheld_loads.insert(
                    (path.to_owned(), text.to_owned()),
                    if browser_map {
                        "importer/enclosing browser map requires authenticated file selection"
                    } else {
                        "active tsconfig resolver requires authenticated file selection"
                    },
                );
                continue;
            }
            // Even an unused value binding can survive client lowering.
            // Without exact erasure facts, every authored value import and
            // export-from must be loadable before this module can evaluate.
            if facts.runtime_resolutions.is_none() && !congruent_alias(&config, &paths, text) {
                blocked_imports.insert((path.into(), text.into()));
                unproven_semantics.insert(path.into());
                module.refused = true;
                withheld_loads.insert(
                    (path.to_owned(), text.to_owned()),
                    "Vite alias and Type Facts paths disagree",
                );
                continue;
            }
            if text == "virtual:file-routes" && config.routes.is_some() {
                // A manifest contains lazy/eager references, not executed page
                // bodies. Exact picked exports and invocation facts are needed.
                unproven_semantics.insert(path.into());
                // The exact audited provider always resolves this id. Only its
                // all-lazy variant is independently loadable: $$route exports
                // and codeSplitting:false emit eager picked static imports.
                let mut route_inputs = Vec::new();
                let loadable = lazy_route_manifest(&directory, &config, &mut route_inputs)
                    && facts.runtime_resolutions.is_none();
                for input in route_inputs {
                    let key = input
                        .to_str()
                        .required(&input, "non-UTF-8 route manifest input")?
                        .to_owned();
                    let observed = graph_identity(&mut manifest, &input)
                        .required(&input, "cannot fingerprint route manifest input")?;
                    manifest.inputs.insert(key, observed);
                }
                if !loadable {
                    module.refused = true;
                    withheld_loads.insert((path.to_owned(), text.to_owned()), "generated route manifest has unproved eager dependencies or requested resolver answer");
                }
                continue;
            }
            if crate::host_loadable::resource_request(text) {
                let mut resource_inputs = Vec::new();
                let resource = crate::host_loadable::resource(
                    &directory,
                    Path::new(path),
                    text,
                    &config,
                    &mut resource_inputs,
                );
                for input in resource_inputs {
                    let key = input
                        .to_str()
                        .required(&input, "non-UTF-8 resource input")?
                        .to_owned();
                    let observed = graph_identity(&mut manifest, &input)
                        .required(&input, "cannot fingerprint resource input")?;
                    manifest.inputs.insert(key, observed);
                }
                let admitted = resource.as_ref().is_some_and(|resource| {
                    facts.runtime_resolutions.as_ref().is_none_or(|runtime| {
                        resource.matches_outcome(runtime.outcome(path, span, text))
                    })
                });
                if !admitted {
                    module.refused = true;
                    unproven_semantics.insert(path.into());
                    blocked_imports.insert((path.into(), text.into()));
                    withheld_loads.insert((path.to_owned(), text.to_owned()), "Vite resource target, executable side input, or requested resolution is unproved");
                }
                // No analyzed implementation or callable edge: generated CSS
                // maps and asset strings are a linking premise only.
                continue;
            }
            let SpecifierAttestation::Attested(row) = resolutions.specifier(path, span, text)
            else {
                blocked_imports.insert((path.into(), text.into()));
                unproven_semantics.insert(path.into());
                module.refused = true;
                withheld_loads.insert(
                    (path.to_owned(), text.to_owned()),
                    "missing exact Type Facts resolution row",
                );
                continue;
            };
            if (row.resolution != ImportResolution::NodeModules && !row.symlink_path.is_empty())
                || !row.included_path.is_empty()
                || text.contains(['?', '#', '%', '\\'])
            {
                blocked_imports.insert((path.into(), text.into()));
                unproven_semantics.insert(path.into());
                module.refused = true;
                withheld_loads.insert(
                    (path.to_owned(), text.to_owned()),
                    "redirected, symlinked local, or query-bearing module target",
                );
                continue;
            }
            if matches!(
                row.resolution,
                ImportResolution::Relative | ImportResolution::NonRelative
            ) {
                let target = row.resolved_path.as_ref();
                if !local_edge(facts, &config, &paths, path, span, text, target) {
                    blocked_imports.insert((path.into(), text.into()));
                    unproven_semantics.insert(path.into());
                    module.refused = true;
                    // With an empty, closed plugin/alias set a TS-only paths
                    // request has no Vite resolver. Other mismatches are unknown,
                    // including requested resolver failures and divergent files.
                    let unloaded = facts.runtime_resolutions.is_none()
                        && config.plugins.is_empty()
                        && config.aliases.is_empty()
                        && text.starts_with("~/")
                        && row.resolution == ImportResolution::NonRelative
                        && Path::new(path).parent().required(Path::new(path), "source has no parent directory")?.ancestors().all(|ancestor| {
                            ["~", "~.js", "~.json", "~.node"].iter().all(|name| {
                                matches!(fs::symlink_metadata(ancestor.join("node_modules").join(name)),
                                    Err(error) if matches!(error.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory))
                            })
                        });
                    withheld_loads.insert(
                        (path.to_owned(), text.to_owned()),
                        if unloaded {
                            "provably unloadable TS-only paths request"
                        } else {
                            "Vite and Type Facts runtime targets are not congruent"
                        },
                    );
                    continue;
                }
                if let Some(target_file) =
                    facts.files.iter().find(|file| file.path.as_str() == target)
                    && fs::canonicalize(target).ok().as_deref() == Some(Path::new(target))
                {
                    module.static_dependencies.insert(target.to_owned());
                    if !explicit_static_surface(facts, file, span, target_file) {
                        module.refused = true;
                        withheld_loads.insert(
                            (path.to_owned(), text.to_owned()),
                            "unproved direct runtime export surface",
                        );
                        continue;
                    }
                    if runtime {
                        scopes
                            .get_mut(&scope_id(path, None))
                            .required(Path::new(path), "missing initialization scope")?
                            .edges
                            .insert(scope_id(target, None));
                    }
                    for binding in file
                        .ast
                        .imports
                        .iter()
                        .filter(|import| import.span == span)
                        .flat_map(|import| &import.bindings)
                        .filter(|binding| !binding.type_only)
                    {
                        if let Some(symbol) = exact_symbol(facts, path, binding.local.span)
                            && functions
                                .get(&symbol)
                                .and_then(|id| scopes.get(id))
                                .is_some_and(|scope| scope.path == target)
                        {
                            // Canonical symbol identity alone loses reexport
                            // provenance. A barrel can have an incongruent edge
                            // even when this definition is initialized elsewhere.
                            // Indirect callable exports need an exact value trail.
                            allowed_functions.insert(symbol);
                        }
                    }
                } else {
                    module.refused = true;
                    withheld_loads.insert(
                        (path.to_owned(), text.to_owned()),
                        "resolved local target is not a canonical analyzed implementation",
                    );
                }
            } else if row.resolution == ImportResolution::NodeModules
                && config.aliases.keys().any(|find| {
                    text == find.as_str()
                        || text
                            .strip_prefix(find.as_str())
                            .is_some_and(|tail| tail.starts_with('/'))
                })
            {
                // Local resolver edges do not authenticate a substituted
                // package export's runtime surface. Keep package admission
                // independent; an explicit package alias cannot strengthen it.
                blocked_imports.insert((path.into(), text.into()));
                unproven_semantics.insert(path.into());
                module.refused = true;
                withheld_loads.insert(
                    (path.to_owned(), text.to_owned()),
                    "package alias has no authenticated runtime export surface",
                );
            } else if row.resolution == ImportResolution::Unresolved {
                blocked_imports.insert((path.into(), text.into()));
                unproven_semantics.insert(path.into());
                module.refused = true;
                withheld_loads.insert(
                    (path.to_owned(), text.to_owned()),
                    "unresolved static module request",
                );
            } else if row.resolution == ImportResolution::NodeModules {
                let mut package_inputs = Vec::new();
                let resolver_active = config.tsconfig_paths || browser_map;
                if packages.is_none() {
                    // The provisional graph schedules package analysis; its
                    // optimistic edges are not final linking authority. Even
                    // here a selected file must be handled by reviewed Vite.
                    let unsupported_loader = package_loads
                        .candidates(Path::new(path), text, &mut package_inputs)
                        .is_some_and(|(_, _, entries)| {
                            (resolver_active
                                && !package_selection_agrees(
                                    facts, path, span, text, &entries, true,
                                ))
                                || !entries
                                    .iter()
                                    .all(|entry| crate::host_loadable::static_entry(entry))
                                || !crate::host_loadable::stylesheet_closure(
                                    &entries,
                                    &config,
                                    &mut package_inputs,
                                )
                        });
                    for input in package_inputs {
                        let key = input
                            .to_str()
                            .required(&input, "non-UTF-8 provisional package input")?
                            .to_owned();
                        let observed = graph_identity(&mut manifest, &input)
                            .required(&input, "cannot fingerprint provisional package input")?;
                        manifest.inputs.insert(key, observed);
                    }
                    if unsupported_loader
                        || (resolver_active && facts.runtime_resolutions.is_none())
                    {
                        module.refused = true;
                        blocked_imports.insert((path.into(), text.into()));
                        withheld_loads.insert(
                            (path.to_owned(), text.to_owned()),
                            "canonical installed package entry uses an unproved Vite loader or tsconfig resolver selection",
                        );
                    }
                    continue;
                }
                let entries = package_loads
                    .loadable(
                        Path::new(path),
                        text,
                        row,
                        dialect,
                        &mut package_inputs,
                        &config.optimize_exclude,
                    )
                    .filter(|entries| {
                        crate::host_loadable::stylesheet_closure(
                            entries.entries(),
                            &config,
                            &mut package_inputs,
                        )
                    });
                // The audited provider adds runtime-owner dedupe in dev. The
                // declaration package must also be the root-selected install.
                let provider = config.plugins.iter().any(|plugin| {
                    ["@solidjs/vite-plugin", "vite-plugin-solid"].contains(&plugin.as_str())
                });
                let core = crate::host_loadable::request(text).is_some_and(|(name, _)| {
                    dialect
                        .audited_installation()
                        .iter()
                        .any(|(owner, _)| *owner == name)
                });
                let deduped = !provider
                    || !core
                    || package_loads
                        .loadable(
                            &directory.join("index.ts"),
                            text,
                            row,
                            dialect,
                            &mut package_inputs,
                            &config.optimize_exclude,
                        )
                        .is_some();
                for input in package_inputs {
                    let key = input
                        .to_str()
                        .required(&input, "non-UTF-8 static package input")?
                        .to_owned();
                    let observed = graph_identity(&mut manifest, &input)
                        .required(&input, "cannot fingerprint static package input")?;
                    manifest.inputs.insert(key, observed);
                }
                let loadable = deduped
                    && !config.package_dedupe
                    && entries.is_some_and(|entries| {
                        package_selection_agrees(
                            facts,
                            path,
                            span,
                            text,
                            entries.entries(),
                            resolver_active,
                        )
                    });
                if !loadable {
                    module.refused = true;
                    blocked_imports.insert((path.into(), text.into()));
                    withheld_loads.insert((path.to_owned(), text.to_owned()), "missing exact installed client package entry, tsconfig rewrite, or requested resolver disagreement");
                }
            }
        }
        // Dynamic import attestation joins to its literal's own exact span.
        // Nonliteral imports and require add no edge and never cancel another.
        for load in &file.ast.module_loads {
            if load.kind != ModuleLoadKind::DynamicImport {
                continue;
            }
            if !scopes
                .get(&at_scope(file, load.span))
                .required(Path::new(path), "missing dynamic-load execution scope")?
                .client_live_site(load.span)
            {
                continue;
            }
            let (Some(text), Some(span)) = (load.specifier.as_deref(), load.specifier_span) else {
                continue;
            };
            if (resolver_selection_required(&config, text)
                || browser_selection_required(Path::new(path), &mut manifest))
                && facts.runtime_resolutions.is_none()
            {
                unproven_semantics.insert(path.into());
                continue;
            }
            if facts.runtime_resolutions.is_none() && !congruent_alias(&config, &paths, text) {
                unproven_semantics.insert(path.into());
                continue;
            }
            let SpecifierAttestation::Attested(row) = resolutions.specifier(path, span, text)
            else {
                continue;
            };
            let target = row.resolved_path.as_ref();
            if matches!(
                row.resolution,
                ImportResolution::Relative | ImportResolution::NonRelative
            ) && row.symlink_path.is_empty()
                && row.included_path.is_empty()
                && local_edge(facts, &config, &paths, path, span, text, target)
                && !text.contains(['?', '#', '%', '\\'])
                && facts.files.iter().any(|file| file.path.as_str() == target)
                && fs::canonicalize(target).ok().as_deref() == Some(Path::new(target))
            {
                scopes
                    .get_mut(&at_scope(file, load.span))
                    .required(Path::new(path), "missing dynamic-load execution scope")?
                    .edges
                    .insert(scope_id(target, None));
            }
        }
        if is_root {
            manifest.roots.insert(scope_id(path, None));
            if default_body && module.completion == Some(true) {
                let body = default_scope(file, facts, &functions).required(
                    Path::new(path),
                    "generated client root has no exact default callable export",
                )?;
                if jsx_body_ready(facts, &scopes, &body) {
                    manifest.roots.insert(body);
                }
            }
        }
        for call in file.ast.calls.iter().filter(|call| !call.construct) {
            let owner = at_scope(file, call.span);
            if !scopes
                .get(&owner)
                .required(Path::new(path), "missing call/JSX execution scope")?
                .client_live_site(call.span)
            {
                continue;
            }
            if let Some(function) = literal_body(file, call.callee) {
                call_edges(
                    facts,
                    file,
                    &owner,
                    &scope_id(path, Some(function)),
                    call,
                    &mut scopes,
                )
                .required(Path::new(path), "inconsistent local callable scope")?;
            } else if direct_identifier(file, call.callee)
                && let Some(symbol) = exact_symbol(facts, path, unwrapped(file, call.callee))
                && allowed_functions.contains(&symbol)
                && let Some(target) = functions.get(&symbol)
            {
                call_edges(facts, file, &owner, target, call, &mut scopes)
                    .required(Path::new(path), "inconsistent local callable scope")?;
            }
            // Constructing/passing a function value does not execute its body.
            // Callback edges need exact invocation and execution-host facts.
        }
        for element in &file.ast.jsx_elements {
            let owner = at_scope(file, element.span);
            if !scopes
                .get(&owner)
                .required(Path::new(path), "missing call/JSX execution scope")?
                .client_live_site(element.span)
            {
                continue;
            }
            if element.member_object.is_none()
                && let Some(symbol) = exact_symbol(facts, path, element.name.span)
                && allowed_functions.contains(&symbol)
                && let Some(target) = functions.get(&symbol)
                && jsx_body_ready(facts, &scopes, target)
            {
                scopes
                    .get_mut(&owner)
                    .required(Path::new(path), "missing JSX execution scope")?
                    .edges
                    .insert(target.clone());
            }
            // JSX child/prop function values likewise require invocation proof.
        }
        modules.push(module);
    }
    if manifest.roots.is_empty()
        || root_files.iter().any(|root| {
            !facts
                .files
                .iter()
                .any(|file| root.to_str() == Some(file.path.as_str()))
        })
    {
        return Err(DiscoveryRefusal::new(
            &directory,
            "browser entry is absent from the analyzed Type Facts file set",
        ));
    }
    manifest.reason.push_str(&format!(
        "; roots: {}",
        root_files
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    if !withheld_loads.is_empty() {
        manifest
            .reason
            .push_str(&format!("; withheld static loads: {withheld_loads:?}"));
    }
    for module in modules
        .iter()
        .filter(|module| startup_modules.contains(&module.path) && module.completion == Some(false))
    {
        manifest.reason.push_str(&format!("; certain top-level exit contradicts the app-starts premise; browser root withheld: {}", module.path));
    }
    let mut index = ProjectHostIndex::build(
        manifest,
        &modules,
        &scopes.into_values().collect::<Vec<_>>(),
    );
    index.blocked_imports = blocked_imports;
    index.unproven_semantics = unproven_semantics;
    if index.is_empty() {
        index.manifest.reason =
            empty_graph_refusal(facts, &config, &root_files, &modules, &withheld_loads).message();
    }
    Ok(index)
}

/// The old acquisition remains the authority for both views. Explicit runtime
/// selections bypass discovery; browser admission is confined to its importer
/// partition and never becomes a no-target fallback.
#[allow(clippy::too_many_arguments)]
pub fn inferred_project_accepted_contracts(
    directory: &Path,
    catalogs: &[PathBuf],
    nested: &[NestedCatalogs],
    trust: Option<&crate::Policy2TrustConfiguration>,
    bundled: bool,
    runtime: &RuntimeEnvironment,
    dialect: &dyn solid_dialect::Dialect,
    facts: &ProjectFacts,
    requirements: AcceptedContractIndex,
) -> Result<AcceptedContractIndex, BackendError> {
    inferred_project_accepted_contracts_with_note(
        directory,
        catalogs,
        nested,
        trust,
        bundled,
        runtime,
        dialect,
        facts,
        requirements,
    )
    .map(|(contracts, _)| contracts)
}

/// Native run note. The caller must replay it through the existing note channel
/// on fresh analyses and cached daemon answers alike.
#[allow(clippy::too_many_arguments)]
pub fn inferred_project_accepted_contracts_with_note(
    directory: &Path,
    catalogs: &[PathBuf],
    nested: &[NestedCatalogs],
    trust: Option<&crate::Policy2TrustConfiguration>,
    bundled: bool,
    runtime: &RuntimeEnvironment,
    dialect: &dyn solid_dialect::Dialect,
    facts: &ProjectFacts,
    requirements: AcceptedContractIndex,
) -> Result<(AcceptedContractIndex, Option<String>), BackendError> {
    let conditions = runtime.selected_conditions();
    let admission = HostStage::new("contract-admission-baseline");
    let baseline = crate::project_accepted_contracts(
        directory,
        catalogs,
        nested,
        trust,
        bundled,
        &conditions,
        facts,
        requirements.clone(),
    )?;
    drop(admission);
    if runtime.target.is_some() {
        return Ok((baseline, None));
    }
    let refused = |reason: String| {
        Some(format!(
            "solid-checker: note: browser host not inferred: {reason}"
        ))
    };
    if !conditions.is_empty() {
        return Ok((
            baseline,
            refused(format!(
                "{}: explicit runtime conditions select {conditions:?}",
                facts.project_id
            )),
        ));
    }
    if runtime.program_boundary == Some(ProgramBoundary::Open) {
        return Ok((
            baseline,
            refused(format!(
                "{}: explicit open program boundary",
                facts.project_id
            )),
        ));
    }
    // External issuer trust, not a project marker, admits synthetic fixtures.
    let fixture_authority = trust.is_some_and(|trust| {
        trust.persistent_local_scope() == Some(crate::fixture_authorization::FIXTURE_ISSUER_SCOPE)
    }) && fs::read(
        directory.join(".solid-checker/shared-host-plugin.json"),
    )
    .ok()
    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        == Some(serde_json::Value::Bool(true));
    let inputs = match discovery_inputs(directory, facts, dialect, fixture_authority) {
        Ok(inputs) => inputs,
        Err(reason) => return Ok((baseline, refused(reason.message()))),
    };
    let mut index = match discovered_index_with_inputs(&inputs, facts, dialect, None) {
        Ok(index) if index.is_empty() => return Ok((baseline, refused(index.manifest.reason))),
        Ok(index) => index,
        Err(reason) => return Ok((baseline, refused(reason.message()))),
    };
    // Package checking only adds module refusals (provisional NodeModules
    // requests skip the final load proof). It cannot create a browser
    // scope in an already-empty graph. Report that proved refusal directly;
    // exact browser artifact admission is needed only for a surviving graph.
    let admission = HostStage::new("contract-admission-browser");
    let browser = match crate::project_accepted_contracts(
        directory,
        catalogs,
        nested,
        trust,
        bundled,
        &BTreeSet::from(["browser".into()]),
        facts,
        requirements,
    ) {
        Ok(browser) => browser,
        Err(error) => {
            return Ok((
                baseline,
                refused(format!(
                    "{}: browser artifact admission failed: {error}",
                    directory.display()
                )),
            ));
        }
    };
    drop(admission);
    // The provisional graph schedules manifest capture; it supplies no final
    // authority. Build the admitted graph only after that capture, inside the
    // replay below, and compare every observation with the complete generation.
    // Both complete contract indexes remain independent (ADR 0140).
    if let Err(reason) = attach_discovery_inputs(
        &mut index,
        &inputs,
        facts,
        dialect,
        fixture_authority,
        Some(&browser),
    ) {
        return Ok((baseline, refused(reason.message())));
    }
    if index.is_empty() {
        return Ok((baseline, refused(index.manifest.reason)));
    }
    // Invocation rows are read only after exact browser artifact admission.
    // No argument/child/prop edge is installed by native syntax discovery.
    let selection = HostStage::new("callback-host-selection");
    index = index.with_callback_invocations(
        solid_reactive_ir::callback_host::browser_callback_invocations(facts, dialect, &browser),
    );
    // Complete browser admission replaces host-free observations for browser
    // execution. No equality/join with other callers is a host premise.
    // Context-provider discovery remains global, so withhold just those
    // package imports until context proof itself can be partitioned.
    let blocked = facts
        .files
        .iter()
        .filter(|file| index.browser_at(file.path.as_str()))
        .flat_map(|file| {
            file.ast.imports.iter().filter_map(|import| {
                browser
                    .contract(file.path.as_str(), &import.module)
                    .ok()
                    .filter(|contract| {
                        contract
                            .artifact_case()
                            .exports
                            .values()
                            .any(|export| !export.call.context_premises().is_empty())
                    })
                    .map(|_| (file.path.to_string(), import.module.to_string()))
            })
        })
        .collect::<Vec<_>>();
    index.blocked_imports.extend(blocked);
    drop(selection);
    validate_inputs(&index)?;
    let note = format!(
        "solid-checker: note: browser host inferred for {} scopes from roots {}",
        index.browser_scope_count(),
        index
            .manifest
            .roots
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    );
    Ok((baseline.with_inferred_browser(index, browser), Some(note)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_observations_do_not_replace_boundary_validation() {
        let root =
            std::env::temp_dir().join(format!("host-graph-observation-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("input.ts");
        fs::write(&path, "before").unwrap();
        let mut manifest = BrowserRootManifest::default();
        let before = graph_identity(&mut manifest, &path).unwrap();
        fs::write(&path, "after!").unwrap();
        assert_eq!(graph_identity(&mut manifest, &path), Some(before.clone()));
        let index = ProjectHostIndex::build(manifest, &[], &[]);
        assert!(validate_inputs(&index).is_err());
        let mut replay = BrowserRootManifest::default();
        assert_ne!(graph_identity(&mut replay, &path), Some(before));
        fs::remove_file(path).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn parallel_input_fingerprints_keep_order_absence_and_content_changes() {
        let root = std::env::temp_dir().join(format!("host-fingerprints-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let files = (0..160)
            .map(|n| root.join(format!("{n}.ts")))
            .collect::<Vec<_>>();
        for (n, path) in files.iter().enumerate().take(159) {
            fs::write(path, format!("{n:03}")).unwrap();
        }
        let paths = files.iter().map(PathBuf::as_path).collect::<Vec<_>>();
        let before = identities(&paths);
        assert_eq!(before[159].as_deref(), Some("absent"));
        assert!(before[..159].iter().all(Option::is_some));
        assert!(before[..159].windows(2).all(|pair| pair[0] != pair[1]));
        fs::write(&files[31], "new").unwrap();
        fs::write(&files[159], "159").unwrap();
        let after = identities(&paths);
        let changed = before
            .iter()
            .zip(&after)
            .enumerate()
            .filter_map(|(n, (a, b))| (a != b).then_some(n))
            .collect::<Vec<_>>();
        assert_eq!(changed, vec![31, 159]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn independent_html_scripts_preserve_external_entries_only() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/reactive-ir/inferred-host-reachability");
        for name in ["classic-independent.html", "inline-module-independent.html"] {
            assert_eq!(
                html_entries(&fs::read_to_string(fixture.join(name)).unwrap()),
                Some(vec!["src/independent.ts".into()]),
                "{name}"
            );
        }
        for name in [
            "classic-interference.html",
            "inline-module-interference.html",
        ] {
            assert!(
                html_entries(&fs::read_to_string(fixture.join(name)).unwrap()).is_none(),
                "{name}"
            );
        }
        assert_eq!(
            html_entries("<script type='module'>throw 'error';</script>"),
            None
        );
        for source in [
            "location='/elsewhere'",
            "window.stop()",
            "document.querySelector('script').remove()",
            "const d=document; d['open']()",
            "unknown()",
        ] {
            assert!(
                html_entries(&format!(
                    "<script>{source}</script><script type='module' src='/src/main.ts'></script>"
                ))
                .is_none(),
                "{source}"
            );
        }
        assert!(html_entries("<script src='/external.js'></script><script type='module' src='/src/main.ts'></script>").is_none());
    }

    #[test]
    fn exact_audited_lazy_route_id_requires_no_eager_picks() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/reactive-ir/inferred-host-reachability");
        let config = |options: &str| {
            host_config::parse(Path::new("vite.config.ts"), &format!("import {{fileRoutes}} from 'filesystem-routing/vite';export default {{plugins:[fileRoutes({options})]}};")).unwrap()
        };
        let lazy = config("{dir:'route-lazy',types:true}");
        assert!(lazy_route_manifest(&directory, &lazy, &mut Vec::new()));
        assert!(!lazy_route_manifest(
            &directory,
            &config("{dir:'route-eager',types:true}"),
            &mut Vec::new()
        ));
        assert!(!lazy_route_manifest(
            &directory,
            &config("{dir:'route-lazy',codeSplitting:false}"),
            &mut Vec::new()
        ));
        assert!(!lazy_route_manifest(
            &directory,
            &config("{dir:'missing-routes'}"),
            &mut Vec::new()
        ));
        let mut absent = lazy.clone();
        absent.plugins.clear();
        assert!(!lazy_route_manifest(&directory, &absent, &mut Vec::new()));
        assert!(host_config::parse(Path::new("vite.config.ts"), "import {fileRoutes} from 'filesystem-routing/vite';export default {plugins:[fileRoutes({moduleId:'virtual:other'})]}").is_none());
    }

    #[test]
    fn default_extension_and_index_probes_require_the_first_congruent_real_file() {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "host-extensions-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        let directory = fs::canonicalize(directory).unwrap();
        let importer = directory.join("main.ts");
        let mut config =
            host_config::parse(Path::new("vite.config.ts"), "export default {plugins:[]}").unwrap();
        let agrees = |config: &Config, text: &str, target: &Path| {
            exact_local_target(
                config,
                importer.to_str().unwrap(),
                text,
                target.to_str().unwrap(),
            )
        };
        fs::write(directory.join("App.tsx"), "export default 1").unwrap();
        assert!(agrees(&config, "./App", &directory.join("App.tsx")));
        assert!(!agrees(&config, "./App", &directory.join("App.ts")));
        fs::write(directory.join("App.ts"), "export default 2").unwrap();
        assert!(agrees(&config, "./App", &directory.join("App.ts")));
        assert!(!agrees(&config, "./App", &directory.join("App.tsx")));
        fs::remove_file(directory.join("App.ts")).unwrap();
        fs::write(directory.join("App.js"), "export default 3").unwrap();
        assert!(agrees(&config, "./App", &directory.join("App.js")));
        assert!(!agrees(&config, "./App", &directory.join("App.tsx")));
        assert!(!agrees(&config, "./App.js", &directory.join("App.tsx")));
        fs::remove_file(directory.join("App.js")).unwrap();
        assert!(!agrees(&config, "./App?raw", &directory.join("App.tsx")));
        fs::create_dir_all(directory.join("folder")).unwrap();
        fs::write(directory.join("folder/index.ts"), "export default 1").unwrap();
        assert!(agrees(
            &config,
            "./folder",
            &directory.join("folder/index.ts")
        ));
        fs::write(directory.join("folder/index.js"), "export default 2").unwrap();
        assert!(agrees(
            &config,
            "./folder",
            &directory.join("folder/index.js")
        ));
        assert!(!agrees(
            &config,
            "./folder",
            &directory.join("folder/index.ts")
        ));
        fs::remove_file(directory.join("folder/index.js")).unwrap();
        fs::write(
            directory.join("folder/package.json"),
            "{\"main\":\"different.js\"}",
        )
        .unwrap();
        assert!(!agrees(
            &config,
            "./folder",
            &directory.join("folder/index.ts")
        ));
        fs::remove_file(directory.join("folder/package.json")).unwrap();
        fs::write(directory.join("folder.ts"), "export default 3").unwrap();
        assert!(agrees(&config, "./folder", &directory.join("folder.ts")));
        assert!(!agrees(
            &config,
            "./folder",
            &directory.join("folder/index.ts")
        ));
        fs::write(directory.join("App"), "runtime bytes").unwrap();
        assert!(!agrees(&config, "./App", &directory.join("App.tsx")));
        fs::remove_file(directory.join("App")).unwrap();
        config
            .aliases
            .insert("~".into(), Some(directory.to_str().unwrap().into()));
        assert!(agrees(&config, "~/App", &directory.join("App.tsx")));
        config.default_extensions = false;
        assert!(!agrees(&config, "./App", &directory.join("App.tsx")));
        assert!(agrees(&config, "./App.tsx", &directory.join("App.tsx")));
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(directory.join("App.tsx"), directory.join("link.tsx"))
                .unwrap();
            config.default_extensions = true;
            assert!(!agrees(&config, "./link", &directory.join("link.tsx")));
        }
        for source in [
            "export default {plugins:[],resolve:{tsconfigPaths:true}}",
            "import paths from 'vite-tsconfig-paths'; export default {plugins:[paths()]}",
            "export default mode ? {plugins:[]} : {plugins:[],resolve:{tsconfigPaths:true}}",
        ] {
            // A config factory's mode is admitted as inert config data.
            let source = if source.contains("mode ?") {
                "import {defineConfig} from 'vite'; export default defineConfig(({mode}) => mode ? {plugins:[]} : {plugins:[],resolve:{tsconfigPaths:true}})"
            } else {
                source
            };
            let parsed = host_config::parse(Path::new("vite.config.ts"), source).unwrap();
            assert!(!parsed.default_extensions);
            assert!(parsed.tsconfig_paths);
        }
        assert!(
            host_config::parse(
                Path::new("vite.config.ts"),
                "export default {plugins:[],resolve:{extensions:['.tsx']}}"
            )
            .is_none()
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn tsrx_probes_preserve_selected_root_instead_of_falling_through() {
        assert_eq!(
            client_root(|path| ["src/entry-client.tsrx", "src/App.tsx"].contains(&path)),
            Some(("src/entry-client.tsrx".into(), false))
        );
        assert_eq!(
            client_root(|path| ["src/App.tsrx", "src/app.tsx"].contains(&path)),
            Some(("src/App.tsrx".into(), true))
        );
    }

    #[test]
    fn previous_parameter_execution_must_complete_before_a_later_default_or_body() {
        for (source, complete) in [
            ("function f(x=throws(),y=later()){} f();", false),
            ("function f(x=throws(),y=later()){} f(false);", true),
            ("function f(x=throws(),y=later()){} f(void 0);", false),
            ("function f(x=true,y=later()){} f();", true),
            ("function f({x},y=later()){} f({x:true});", false),
        ] {
            let ast = solid_facts::ast::extract("inputs.ts", source).unwrap();
            let call = ast.calls.iter().max_by_key(|call| call.span.start).unwrap();
            assert_eq!(
                parameter_initialization_complete(&ast, &ast.functions[0], call, 0),
                complete,
                "{source}"
            );
        }
    }

    #[test]
    fn default_inputs_require_known_argument_position_and_runtime_undefined() {
        for (source, index, expected) in [
            ("f()", 0, true),
            ("f(false)", 0, false),
            ("f(void 0)", 0, true),
            ("f(undefined)", 0, false),
            ("f(...xs)", 0, false),
            ("f(false,...xs)", 0, false),
            ("f(void 0,...xs)", 0, false),
            ("f(void 0,throwing())", 0, false),
            ("f(...xs,void 0)", 1, false),
            ("f(false)", 1, true),
            ("f((void 0) as boolean | undefined)", 0, true),
        ] {
            let ast = solid_facts::ast::extract("inputs.ts", source).unwrap();
            assert_eq!(
                omitted_or_undefined(&ast.calls[0], index, &ast.host_undefined_arguments),
                expected,
                "{source}"
            );
        }
    }

    #[test]
    fn callback_fixture_requires_runtime_loadability_for_every_static_package() {
        let Ok(producer) = std::env::var("SOLID_TYPEFACTS_BIN") else {
            return;
        };
        fn copy_tree(from: &Path, to: &Path) {
            fs::create_dir_all(to).unwrap();
            for entry in fs::read_dir(from).unwrap() {
                let entry = entry.unwrap();
                let target = to.join(entry.file_name());
                if entry.file_type().unwrap().is_dir() {
                    copy_tree(&entry.path(), &target);
                } else {
                    fs::copy(entry.path(), target).unwrap();
                }
            }
        }
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let directory = repository.join(format!(
            "rust/target/inferred-callback-index-{}",
            std::process::id()
        ));
        copy_tree(
            &repository.join("fixtures/reactive-ir/inferred-host-callback-invocation"),
            &directory,
        );
        let directory = fs::canonicalize(directory).unwrap();
        let authorization = crate::fixture_authorization::authorize_fixture_contract(
            &directory,
            &crate::fixture_authorization::read_fixture_contract_request(&directory).unwrap(),
        )
        .unwrap();
        let trust =
            crate::decode_policy2_trust_configuration(&authorization.trust_configuration).unwrap();
        let project = directory.join("tsconfig.json").to_str().unwrap().to_owned();
        let typescript = crate::TypeFactsSession::open(&producer, &project, &[]).unwrap();
        let dialect = crate::dialect::default_dialect();
        let (mut session, _) =
            crate::NativeIncrementalSession::open_pipelined(dialect, project, typescript).unwrap();
        let facts = session.analyze().unwrap();
        let (contracts, note) = inferred_project_accepted_contracts_with_note(
            &directory,
            &[directory.join(crate::fixture_authorization::CATALOG)],
            &[],
            Some(&trust),
            false,
            &RuntimeEnvironment::default(),
            dialect.vocabulary,
            &facts,
            AcceptedContractIndex::default(),
        )
        .unwrap();
        // Only reactive-package is artifact-admitted here. The declaration-only
        // Solid/signals stubs do not establish runtime loadability. Callback
        // delivery tests remain in callback_host/hosts; this consumer refuses
        // evaluation before any of those registrations can gain authority.
        assert!(contracts.inferred_hosts().is_none());
        assert!(
            note.unwrap()
                .starts_with("solid-checker: note: browser host not inferred: ")
        );
        // An optional local-edge source must not silently authorize a package
        // replacement against the independently admitted declaration artifact.
        fs::write(
            directory.join("vite.config.ts"),
            r#"export default {plugins:[],resolve:{alias:{"reactive-package":"/aliased-runtime.js"}}};"#,
        )
        .unwrap();
        let mut observed = facts.as_ref().clone();
        observed.runtime_resolutions =
            Some(solid_facts::runtime_resolution::RuntimeResolutionIndex::default());
        let refused =
            discovered_index(&directory, &observed, dialect.vocabulary, true, None).unwrap();
        let main = directory.join("src/main.tsx").to_str().unwrap().to_owned();
        assert!(
            refused
                .blocked_imports
                .contains(&(main.clone(), "reactive-package".into()))
        );
        assert!(!refused.browser_proof_site(&main, 0, 1));
    }

    #[test]
    fn tsconfig_resolver_withholds_every_default_bare_or_aliased_selection() {
        let mut config = host_config::parse(
            Path::new("vite.config.ts"),
            "export default {plugins:[],resolve:{tsconfigPaths:true}}",
        )
        .unwrap();
        config.aliases.insert("/alias".into(), Some("/src".into()));
        config.aliases.insert("./alias".into(), Some("/src".into()));
        for (request, required) in [
            ("@solidjs/web", true),
            ("~/App.ts", true),
            (".package", true),
            ("/alias/App.ts", true),
            ("./alias/App.ts", true),
            ("./App.ts", false),
            ("../App.ts", false),
            ("/src/App.ts", false),
        ] {
            assert_eq!(
                resolver_selection_required(&config, request),
                required,
                "{request}"
            );
        }
        config.tsconfig_paths = false;
        assert!(!resolver_selection_required(&config, "@solidjs/web"));
        assert!(!resolver_selection_required(&config, "./alias/App.ts"));
    }

    #[test]
    fn tsconfig_resolver_selection_requires_exact_runtime_agreement() {
        use solid_facts::runtime_resolution::{RuntimeOutcome, RuntimeResolutionIndex};
        let Ok(producer) = std::env::var("SOLID_TYPEFACTS_BIN") else {
            return;
        };
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let scratch =
            std::env::temp_dir().join(format!("host-tsconfig-selection-{}", std::process::id()));
        let directory = scratch.join("app");
        fs::create_dir_all(directory.join("src")).unwrap();
        fs::create_dir_all(scratch.join("shadow-base/@solidjs")).unwrap();
        fs::write(
            scratch.join("shadow-base/@solidjs/web.js"),
            "export const isServer = true;",
        )
        .unwrap();
        let directory = fs::canonicalize(directory).unwrap();
        fs::write(directory.join("package.json"), "{\"private\":true}").unwrap();
        fs::write(
            directory.join("index.html"),
            "<script type='module' src='/src/main.ts'></script>",
        )
        .unwrap();
        let source = "import {isServer} from '@solidjs/web'; new Date(); if (!isServer) selected(); function selected() {}";
        let main = directory.join("src/main.ts");
        fs::write(&main, source).unwrap();
        fs::write(
            directory.join("src/shadow.js"),
            "export const isServer = true;",
        )
        .unwrap();
        let project = directory.join("tsconfig.app.json");
        fs::write(&project, r#"{"compilerOptions":{"strict":true,"module":"ESNext","moduleResolution":"Bundler","target":"ES2022"},"include":["src/*.ts"]}"#).unwrap();
        let package = directory.join("node_modules/@solidjs/web");
        fs::create_dir_all(package.join("dist")).unwrap();
        fs::write(package.join("package.json"), r#"{"name":"@solidjs/web","version":"2.0.0-rc.13","type":"module","exports":{".":{"types":"./index.d.ts","browser":{"development":"./dist/web.dev.js","observe":"./dist/web.observe.js","default":"./dist/web.js"},"node":{"development":"./dist/server.dev.js","observe":"./dist/server.observe.js","default":"./dist/server.js"},"default":"./dist/web.js"}}}"#).unwrap();
        // Exact published signature, not a relaxed callback or literal typing.
        fs::write(
            package.join("index.d.ts"),
            "export declare const isServer: boolean;",
        )
        .unwrap();
        let dialect = crate::dialect::default_dialect();
        for target in dialect.vocabulary.host_boolean_exports()[0].runtime_targets {
            fs::write(
                package.join(target),
                format!("export const isServer = {};", target.contains("server")),
            )
            .unwrap();
        }
        let typescript =
            crate::TypeFactsSession::open(&producer, project.to_str().unwrap(), &[]).unwrap();
        let (mut session, _) = crate::NativeIncrementalSession::open_pipelined(
            dialect,
            project.to_str().unwrap().into(),
            typescript,
        )
        .unwrap();
        let facts = session.analyze().unwrap();
        let file = facts
            .files
            .iter()
            .find(|file| file.path.as_str() == main.to_str().unwrap())
            .unwrap();
        let import = &file.ast.imports[0];
        let cases: serde_json::Value = serde_json::from_slice(
            &fs::read(repository.join(
                "fixtures/reactive-ir/inferred-host-reachability/resolver-selection-cases.json",
            ))
            .unwrap(),
        )
        .unwrap();
        for config_source in [
            "export default {plugins:[],resolve:{tsconfigPaths:true}}",
            "import {defineConfig} from 'vite'; export default defineConfig(({mode}) => ({plugins:[],resolve:{tsconfigPaths:mode}}))",
            "import {defineConfig} from 'vite'; export default defineConfig(({mode}) => mode ? {plugins:[]} : {plugins:[],resolve:{tsconfigPaths:true}})",
            "import paths from 'vite-tsconfig-paths'; export default {plugins:[paths()]}",
        ] {
            let config =
                host_config::parse(&directory.join("vite.config.ts"), config_source).unwrap();
            assert!(config.tsconfig_paths);
            for case in cases.as_array().unwrap() {
                fs::write(
                    directory.join("tsconfig.json"),
                    serde_json::to_vec(&case["tsconfig"]).unwrap(),
                )
                .unwrap();
                fs::write(
                    directory.join("inherited.json"),
                    r#"{"compilerOptions":{"paths":{"@solidjs/web":["./src/shadow.js"]}}}"#,
                )
                .unwrap();
                let config = if let Some(source) = case["config"].as_str() {
                    let parsed = host_config::parse(&directory.join("vite.config.ts"), source);
                    if case["configRefused"] == true {
                        assert!(parsed.is_none(), "{case}");
                        continue;
                    }
                    parsed.unwrap()
                } else if case["resolver"] == false {
                    host_config::parse(
                        &directory.join("vite.config.ts"),
                        "export default {plugins:[]}",
                    )
                    .unwrap()
                } else {
                    config.clone()
                };
                let prepared = DiscoveryInputs {
                    directory: directory.clone(),
                    config,
                    paths: BTreeMap::new(),
                    config_inputs: Vec::new(),
                };
                let mut observed = facts.as_ref().clone();
                if case["runtime"] != "absent" {
                    let outcome = match case["runtime"].as_str().unwrap() {
                        "client" => Some(package.join("dist/web.js")),
                        "server" => Some(package.join("dist/server.js")),
                        "shadow" => Some(directory.join("src/shadow.js")),
                        "token-shadow" => Some(
                            directory
                                .parent()
                                .unwrap()
                                .join("shadow-base/@solidjs/web.js"),
                        ),
                        _ => None,
                    }
                    .map_or(RuntimeOutcome::Unknown, |path| {
                        RuntimeOutcome::File {
                            path: path.to_str().unwrap().into(),
                            physical_path: path.to_str().unwrap().into(),
                        }
                    });
                    let mut runtime = RuntimeResolutionIndex::default();
                    runtime.insert(
                        main.to_str().unwrap(),
                        import.span,
                        import.module.as_str(),
                        outcome,
                    );
                    observed.runtime_resolutions = Some(runtime);
                }
                let constants = execution_constants(
                    &prepared,
                    &observed,
                    file,
                    dialect.vocabulary,
                    &mut BrowserRootManifest::default(),
                );
                let expected = case["browser"].as_bool().unwrap();
                assert_eq!(!constants.is_empty(), expected, "{config_source}: {case}");
                let index = discovered_index_with_inputs(
                    &prepared,
                    &observed,
                    dialect.vocabulary,
                    Some(&AcceptedContractIndex::default()),
                )
                .unwrap();
                let start = u64::try_from(source.find("selected()").unwrap()).unwrap();
                assert_eq!(
                    index.browser_proof_site(main.to_str().unwrap(), start, start + 10),
                    expected,
                    "{config_source}: {case}"
                );
            }
        }
        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn native_execution_fixture_sites_keep_function_boundaries_and_edge_refusals() {
        let Ok(producer) = std::env::var("SOLID_TYPEFACTS_BIN") else {
            return;
        };
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        for name in [
            "reachability",
            "alias-congruent",
            "alias-divergent",
            "certification-clean",
        ] {
            let directory = fs::canonicalize(
                repository.join(format!("fixtures/reactive-ir/inferred-host-{name}")),
            )
            .unwrap();
            let project = directory.join("tsconfig.json").to_str().unwrap().to_owned();
            let typescript = crate::TypeFactsSession::open(&producer, &project, &[]).unwrap();
            let dialect = crate::dialect::default_dialect();
            let (mut session, _) =
                crate::NativeIncrementalSession::open_pipelined(dialect, project, typescript)
                    .unwrap();
            let facts = session.analyze().unwrap();
            if name == "alias-congruent" {
                let entry = facts
                    .files
                    .iter()
                    .find(|file| {
                        file.path.as_str() == directory.join("src/main.tsx").to_str().unwrap()
                    })
                    .unwrap();
                let import = entry
                    .ast
                    .imports
                    .iter()
                    .find(|import| import.module == "~")
                    .unwrap();
                assert!(
                    runtime_import(&facts, entry, import),
                    "a proven value use installs the congruent alias execution edge"
                );
            }
            let provisional =
                discovered_index(&directory, &facts, dialect.vocabulary, false, None).unwrap();
            let prepared = discovery_inputs(&directory, &facts, dialect.vocabulary, false).unwrap();
            let package_checked = discovered_index_with_inputs(
                &prepared,
                &facts,
                dialect.vocabulary,
                Some(&AcceptedContractIndex::default()),
            )
            .unwrap();
            assert!(
                package_checked.browser_scope_count() <= provisional.browser_scope_count(),
                "package checks only withhold scopes: {name}"
            );
            if provisional.is_empty() {
                assert!(
                    package_checked.is_empty(),
                    "an empty provisional graph cannot gain authority"
                );
            }
            let index = provisional.with_callback_invocations(
                solid_reactive_ir::callback_host::browser_callback_invocations(
                    &facts,
                    dialect.vocabulary,
                    &AcceptedContractIndex::default(),
                ),
            );
            let cases: serde_json::Value =
                serde_json::from_slice(&fs::read(directory.join("host-cases.json")).unwrap())
                    .unwrap();
            let mut mismatches = Vec::new();
            for case in cases.as_array().unwrap() {
                let path = directory.join(case["path"].as_str().unwrap());
                let source = fs::read_to_string(&path).unwrap();
                let text = case["text"].as_str().unwrap();
                let start = u64::try_from(source.find(text).unwrap()).unwrap();
                let actual = index.browser_site(
                    path.to_str().unwrap(),
                    start,
                    start + u64::try_from(text.len()).unwrap(),
                );
                if actual != case["browser"].as_bool().unwrap() {
                    mismatches.push(format!("{name}: {case}; browser actual={actual}"));
                }
                if let Some(proof) = case.get("proof").and_then(serde_json::Value::as_bool) {
                    let actual = index.browser_proof_site(
                        path.to_str().unwrap(),
                        start,
                        start + u64::try_from(text.len()).unwrap(),
                    );
                    if actual != proof {
                        mismatches.push(format!("{name}: {case}; proof actual={actual}"));
                    }
                }
            }
            assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
            if name == "reachability" || name == "alias-divergent" {
                use solid_facts::runtime_resolution::{RuntimeOutcome, RuntimeResolutionIndex};
                let importer = directory.join(if name == "reachability" {
                    "src/runtime-entry.ts"
                } else {
                    "src/main.tsx"
                });
                let source = facts
                    .files
                    .iter()
                    .find(|file| file.path.as_str() == importer.to_str().unwrap())
                    .unwrap();
                let import = source
                    .ast
                    .imports
                    .iter()
                    .find(|import| {
                        import.module
                            == if name == "reachability" {
                                "./runtime-helper"
                            } else {
                                "~"
                            }
                    })
                    .unwrap();
                let target = directory.join(if name == "reachability" {
                    "src/runtime-helper.ts"
                } else {
                    "src/helper.ts"
                });
                for (outcome, expected) in [
                    (RuntimeOutcome::Unknown, false),
                    (
                        RuntimeOutcome::File {
                            path: target.to_str().unwrap().into(),
                            physical_path: directory.join("src/server.ts").to_str().unwrap().into(),
                        },
                        false,
                    ),
                    (
                        RuntimeOutcome::File {
                            path: target.to_str().unwrap().into(),
                            physical_path: target.to_str().unwrap().into(),
                        },
                        true,
                    ),
                ] {
                    let mut observed = facts.as_ref().clone();
                    let mut runtime = RuntimeResolutionIndex::default();
                    if name == "reachability" {
                        let main = facts
                            .files
                            .iter()
                            .find(|file| {
                                file.path.as_str()
                                    == directory.join("src/main.tsx").to_str().unwrap()
                            })
                            .unwrap();
                        let entry = main
                            .ast
                            .imports
                            .iter()
                            .find(|import| import.module == "./runtime-entry.ts")
                            .unwrap();
                        runtime.insert(
                            main.path.shared(),
                            entry.span,
                            entry.module.as_str(),
                            RuntimeOutcome::File {
                                path: importer.to_str().unwrap().into(),
                                physical_path: importer.to_str().unwrap().into(),
                            },
                        );
                    }
                    runtime.insert(
                        importer.to_str().unwrap(),
                        import.span,
                        import.module.as_str(),
                        outcome,
                    );
                    observed.runtime_resolutions = Some(runtime);
                    let index =
                        discovered_index(&directory, &observed, dialect.vocabulary, false, None)
                            .unwrap();
                    assert_eq!(
                        index.browser_site(target.to_str().unwrap(), 0, 1),
                        expected && name != "reachability",
                        "{name}"
                    );
                    assert!(index.manifest.runtime_resolution.is_some());
                }
            }
        }
    }

    #[test]
    fn literal_alias_congruence_requires_the_same_single_target() {
        let mut config = host_config::parse(
            Path::new("vite.config.ts"),
            "export default {plugins:[],resolve:{alias:{'~':'/app/src'}}};",
        )
        .unwrap();
        let paths = BTreeMap::from([("~/*".into(), vec!["/app/src/*".into()])]);
        assert!(congruent_alias(&config, &paths, "~/helper"));
        assert!(congruent_alias(&config, &paths, "./helper"));
        assert!(congruent_alias(&config, &BTreeMap::new(), "~/helper"));
        config
            .aliases
            .insert("~".into(), Some("/app/server".into()));
        assert!(!congruent_alias(&config, &paths, "~/helper"));
        config.aliases.insert("~".into(), None);
        assert!(!congruent_alias(&config, &paths, "~/helper"));
        config.plugins.insert("vite-tsconfig-paths".into());
        assert!(!congruent_alias(&config, &paths, "~/helper"));
        config.aliases.insert("~".into(), Some("/app/src".into()));
        assert!(congruent_alias(&config, &paths, "~/helper"));
    }

    #[test]
    fn aggregate_import_uses_cannot_cross_a_server_function_boundary() {
        for (source, client) in [
            (
                "import { value } from './helper'; console.log(value);",
                true,
            ),
            (
                "import { value } from './helper'; type T = typeof value; function server() { 'use server'; return value; }",
                false,
            ),
            (
                "import { value } from './helper'; console.log(value); function server() { 'use server'; return value; }",
                false,
            ),
            (
                "import { value } from './helper'; console.log(value); function server(value: number) { 'use server'; return value; }",
                true,
            ),
        ] {
            let ast = solid_facts::ast::extract("imports.ts", source).unwrap();
            let binding = &ast.imports[0].bindings[0];
            assert!(binding.runtime_referenced);
            assert_eq!(
                import_references_are_client_only(&ast, binding.local.span),
                client,
                "{source}"
            );
        }
        let ast = solid_facts::ast::extract(
            "imports.ts",
            "import { value } from './helper'; type T = typeof value;",
        )
        .unwrap();
        assert!(!ast.imports[0].bindings[0].runtime_referenced);
    }

    #[test]
    fn conventional_invocation_fixture_twins_name_the_refusal_site() {
        let scratch = std::env::temp_dir().join(format!(
            "host-conventional-selection-{}",
            std::process::id()
        ));
        fs::create_dir_all(scratch.join("app")).unwrap();
        let scratch = fs::canonicalize(scratch).unwrap();
        fs::create_dir_all(scratch.join("shadow-base")).unwrap();
        let app = fs::canonicalize(scratch.join("app")).unwrap();
        let project = app.join("tsconfig.json");
        fs::write(&project, "{}").unwrap();
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let cases: serde_json::Value = serde_json::from_slice(
            &fs::read(repository.join(
                "fixtures/reactive-ir/inferred-host-reachability/conventional-selection-cases.json",
            ))
            .unwrap(),
        )
        .unwrap();
        for case in cases.as_array().unwrap() {
            if case.get("browserMap").is_some() || case.get("workspaceBrowser").is_some() {
                continue;
            }
            if !cfg!(unix)
                && ["manifestSymlink", "workspaceSymlink", "configSymlink"]
                    .iter()
                    .any(|key| case[*key] == true)
            {
                continue;
            }
            for path in [
                app.join("package.json"),
                scratch.join("package.json"),
                app.join("vite.config.ts"),
            ] {
                if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_symlink()) {
                    fs::remove_file(path).unwrap();
                }
            }
            if let Some(config) = case["outsideConfig"].as_str() {
                fs::write(scratch.join("other.ts"), config).unwrap();
            }
            let parent_config = scratch.join("vite.config.ts");
            if let Some(config) = case["parentConfig"].as_str() {
                fs::write(&parent_config, config).unwrap();
            } else if parent_config.exists() {
                fs::remove_file(&parent_config).unwrap();
            }
            let mut package = serde_json::json!({"private":true,"name":"inferred-host-spa"});
            if let Some(scripts) = case.get("scripts") {
                package["scripts"] = scripts.clone();
            }
            fs::write(
                app.join("package.json"),
                serde_json::to_vec(&package).unwrap(),
            )
            .unwrap();
            let mut workspace = serde_json::json!({"private":true});
            if let Some(scripts) = case.get("workspaceScripts") {
                workspace["scripts"] = scripts.clone();
            }
            fs::write(
                scratch.join("package.json"),
                serde_json::to_vec(&workspace).unwrap(),
            )
            .unwrap();
            fs::write(
                app.join("vite.config.ts"),
                case["config"]
                    .as_str()
                    .unwrap_or("export default {plugins:[]};"),
            )
            .unwrap();
            if let Some(name) = case["configFile"].as_str() {
                fs::write(app.join(name), "export default {};").unwrap();
            }
            #[cfg(unix)]
            for (key, path, target) in [
                (
                    "manifestSymlink",
                    app.join("package.json"),
                    scratch.join("app-manifest.json"),
                ),
                (
                    "workspaceSymlink",
                    scratch.join("package.json"),
                    scratch.join("workspace-manifest.json"),
                ),
                (
                    "configSymlink",
                    app.join("vite.config.ts"),
                    scratch.join("linked-config.ts"),
                ),
            ] {
                if case[key] == true {
                    fs::rename(&path, &target).unwrap();
                    std::os::unix::fs::symlink(target, path).unwrap();
                }
            }
            let mut inputs = Vec::new();
            let decision = application_inputs(&app, &project, &mut inputs)
                .and_then(|()| configuration(&app, &mut inputs, false));
            assert_eq!(
                decision.is_ok(),
                case["browser"] == true,
                "{case}: {decision:?}"
            );
            if let Some(note) = case["note"].as_str() {
                let refusal = decision.unwrap_err();
                assert!(refusal.message().contains(note), "{case}: {refusal:?}");
                assert!(inputs.contains(&refusal.path));
                if case.get("workspaceScripts").is_some() {
                    assert_eq!(refusal.path, scratch.join("package.json"));
                }
            }
            if let Some(config) = case["mutatedParentConfig"].as_str() {
                let app_manifest = inferred_host_input_digest(&app.join("package.json"));
                let parent_manifest = inferred_host_input_digest(&scratch.join("package.json"));
                let before = inferred_host_input_digest(&parent_config);
                fs::write(&parent_config, config).unwrap();
                assert_ne!(before, inferred_host_input_digest(&parent_config));
                assert_eq!(
                    app_manifest,
                    inferred_host_input_digest(&app.join("package.json"))
                );
                assert_eq!(
                    parent_manifest,
                    inferred_host_input_digest(&scratch.join("package.json"))
                );
                let refusal = application_inputs(&app, &project, &mut Vec::new()).unwrap_err();
                assert_eq!(refusal.path, scratch.join("package.json"));
                assert!(refusal.message().contains(case["note"].as_str().unwrap()));
            }
            if case.get("outsideConfig").is_some() {
                // A refused invocation cannot regain authority when the
                // unselected outside config changes into an innocuous one.
                fs::write(scratch.join("other.ts"), "export default {plugins:[]};").unwrap();
                assert!(
                    application_inputs(&app, &project, &mut Vec::new())
                        .and_then(|()| configuration(&app, &mut Vec::new(), false))
                        .is_err(),
                    "{case}"
                );
            }
            #[cfg(unix)]
            for (key, path, target) in [
                (
                    "manifestSymlink",
                    app.join("package.json"),
                    scratch.join("app-manifest.json"),
                ),
                (
                    "workspaceSymlink",
                    scratch.join("package.json"),
                    scratch.join("workspace-manifest.json"),
                ),
                (
                    "configSymlink",
                    app.join("vite.config.ts"),
                    scratch.join("linked-config.ts"),
                ),
            ] {
                if case[key] == true {
                    let pointer = inferred_host_input_digest(&path).unwrap();
                    let bytes = inferred_host_input_digest(&target).unwrap();
                    fs::write(&target, "different mutable target bytes").unwrap();
                    assert_eq!(pointer, inferred_host_input_digest(&path).unwrap());
                    assert_ne!(bytes, inferred_host_input_digest(&target).unwrap());
                    assert!(
                        application_inputs(&app, &project, &mut Vec::new())
                            .and_then(|()| configuration(&app, &mut Vec::new(), false))
                            .is_err()
                    );
                }
            }
            if let Some(name) = case["configFile"].as_str() {
                fs::remove_file(app.join(name)).unwrap();
            }
        }
        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn conventional_selection_and_browser_map_inputs_invalidate_authority() {
        let scratch =
            std::env::temp_dir().join(format!("host-selection-cache-{}", std::process::id()));
        let app = scratch.join("app");
        fs::create_dir_all(app.join("src")).unwrap();
        let scratch = fs::canonicalize(scratch).unwrap();
        let app = fs::canonicalize(app).unwrap();
        let project = app.join("tsconfig.json");
        let package = app.join("package.json");
        fs::write(&project, "{}").unwrap();
        fs::write(
            &package,
            r#"{"private":true,"scripts":{"build":"vite build"}}"#,
        )
        .unwrap();
        fs::write(scratch.join("package.json"), r#"{"private":true}"#).unwrap();
        fs::write(app.join("vite.config.ts"), "export default {plugins:[]};").unwrap();
        let source = app.join("src/main.ts");
        fs::write(&source, "export const selected = true;").unwrap();
        let inputs = inferred_host_input_paths_for_project(&app, &project);
        assert!(inputs.contains(&app));
        assert!(inputs.contains(&package));
        assert!(inputs.contains(&scratch.join("package.json")));
        assert!(inputs.contains(&app.join("src/package.json")));
        let before = inferred_host_input_digest(&app).unwrap();
        let variant = app.join("vite.config.production.ts");
        fs::write(&variant, "export default {plugins:[]};").unwrap();
        assert_ne!(before, inferred_host_input_digest(&app).unwrap());
        assert!(inferred_host_input_paths_for_project(&app, &project).contains(&variant));
        fs::remove_file(&variant).unwrap();
        let before = inferred_host_input_digest(&package).unwrap();
        fs::write(
            &package,
            r#"{"private":true,"scripts":{"build":"vite build --mode production"}}"#,
        )
        .unwrap();
        assert_ne!(before, inferred_host_input_digest(&package).unwrap());
        assert!(application(&app, &project).is_err());
        for path in [
            &package,
            &scratch.join("package.json"),
            &app.join("src/package.json"),
        ] {
            let before = inferred_host_input_digest(path).unwrap();
            fs::write(path, r#"{"private":true,"browser":{"@solidjs/web":false}}"#).unwrap();
            assert_ne!(before, inferred_host_input_digest(path).unwrap());
            let mut manifest = BrowserRootManifest::default();
            assert!(browser_selection_required(&source, &mut manifest));
            assert!(manifest.inputs.contains_key(path.to_str().unwrap()));
            fs::write(path, r#"{"private":true,"browser":{}}"#).unwrap();
            assert!(!browser_selection_required(
                &source,
                &mut BrowserRootManifest::default()
            ));
        }
        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn tsconfig_resolver_activity_inputs_invalidate_cached_authority() {
        let scratch = std::env::temp_dir().join(format!(
            "host-resolver-activity-inputs-{}",
            std::process::id()
        ));
        fs::create_dir_all(scratch.join("src")).unwrap();
        let app = fs::canonicalize(&scratch).unwrap();
        let project = app.join("tsconfig.app.json");
        fs::write(app.join("package.json"), "{\"private\":true}").unwrap();
        fs::write(&project, "{}").unwrap();
        let vite = app.join("vite.config.ts");
        fs::write(&vite, "export default {plugins:[]}").unwrap();
        let inputs = inferred_host_input_paths_for_project(&app, &project);
        assert!(inputs.contains(&vite));
        let before = inferred_host_input_digest(&vite).unwrap();
        fs::write(
            &vite,
            "export default {plugins:[],resolve:{tsconfigPaths:true}}",
        )
        .unwrap();
        assert_ne!(before, inferred_host_input_digest(&vite).unwrap());
        fs::write(
            &vite,
            "import paths from 'vite-tsconfig-paths'; export default {plugins:[paths()]}",
        )
        .unwrap();
        let inputs = inferred_host_input_paths_for_project(&app, &project);
        let plugin = app.join("node_modules/vite-tsconfig-paths/package.json");
        assert!(inputs.contains(&vite));
        assert!(inputs.contains(&plugin));
        let absent = inferred_host_input_digest(&plugin).unwrap();
        fs::create_dir_all(plugin.parent().unwrap()).unwrap();
        fs::write(
            &plugin,
            "{\"name\":\"vite-tsconfig-paths\",\"version\":\"unknown\"}",
        )
        .unwrap();
        assert_ne!(absent, inferred_host_input_digest(&plugin).unwrap());
        assert!(inferred_host_input_paths_for_project(&app, &project).contains(&plugin));
        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn analyzed_config_name_and_inheritance_are_exact_discovery_inputs() {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let scratch = std::env::temp_dir().join(format!(
            "host-config-name-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let app = scratch.join("app");
        fs::create_dir_all(&app).unwrap();
        let app = fs::canonicalize(app).unwrap();
        fs::write(app.join("package.json"), "{\"private\":true}").unwrap();
        let base = app.parent().unwrap().join("tsconfig.base.json");
        fs::write(
            &base,
            "{\"compilerOptions\":{\"paths\":{\"~/*\":[\"./app/src/*\"]}}}",
        )
        .unwrap();
        let project = app.join("tsconfig.app.json");
        fs::write(&project, "{\"extends\":\"../tsconfig.base.json\"}").unwrap();
        assert!(application(&app, &project).is_ok());
        let inputs = inferred_host_input_paths_for_project(&app, &project);
        assert!(inputs.contains(&project));
        assert!(inputs.contains(&base));
        assert_eq!(
            paths_targets(&app, &project).unwrap()["~/*"],
            vec![app.join("src/*").to_str().unwrap().to_owned()]
        );
        fs::write(&base, "{\"references\":[]}").unwrap();
        assert!(application(&app, &project).is_ok());
        fs::write(&base, "{\"references\":[{\"path\":\"../external\"}]}").unwrap();
        let refusal = application(&app, &project).unwrap_err();
        assert_eq!(refusal.path, base);
        assert!(refusal.reason.contains("references"));
        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn refused_configuration_inputs_do_not_scale_with_application_inventory() {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let scratch = std::env::temp_dir().join(format!(
            "host-refusal-inputs-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        ));
        fs::create_dir_all(scratch.join("src")).unwrap();
        let app = fs::canonicalize(&scratch).unwrap();
        fs::write(app.join("package.json"), "{\"private\":true}").unwrap();
        let project = app.join("tsconfig.json");
        fs::write(&project, "{}").unwrap();
        let config = app.join("vite.config.ts");
        fs::write(
            &config,
            "import unknown from 'unknown-plugin'; export default unknown();",
        )
        .unwrap();
        let before = inferred_host_input_paths_for_project(&app, &project);
        for index in 0..256 {
            fs::write(
                app.join(format!("src/mod{index}.tsx")),
                "not valid TypeScript {{{",
            )
            .unwrap();
        }
        assert_eq!(
            before,
            inferred_host_input_paths_for_project(&app, &project)
        );
        // One package plus 20 PostCSS candidates per ancestor, six Vite
        // candidates, one tsconfig and the fixture premise marker.
        assert!(before.len() <= app.ancestors().count() * 21 + 9);
        assert!(CONFIGS.iter().all(|name| before.contains(&app.join(name))));
        assert!(before.contains(&project));
        assert!(!before.contains(&app.join("src")));
        fs::write(&config, "export default {plugins: []};").unwrap();
        let after = inferred_host_input_paths_for_project(&app, &project);
        assert!(after.contains(&app.join("src/mod255.tsx")));
        // A competing config must invalidate the successful grammar branch.
        fs::write(app.join("vite.config.js"), "export default {};").unwrap();
        let competing = inferred_host_input_paths_for_project(&app, &project);
        assert!(competing.contains(&app.join("vite.config.js")));
        assert!(!competing.contains(&app.join("src/mod255.tsx")));
        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn published_application_refusal_observes_only_its_manifest() {
        let scratch =
            std::env::temp_dir().join(format!("host-published-inputs-{}", std::process::id()));
        fs::create_dir_all(&scratch).unwrap();
        fs::write(scratch.join("package.json"), "{\"exports\":\"./index.js\"}").unwrap();
        assert_eq!(
            inferred_host_input_paths_for_project(&scratch, &scratch.join("tsconfig.json")),
            vec![scratch.join("package.json")],
        );
        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn admitted_raw_resources_and_absent_lookups_are_daemon_inputs() {
        let scratch = std::env::temp_dir().join(format!("host-raw-inputs-{}", std::process::id()));
        fs::create_dir_all(scratch.join("src")).unwrap();
        let app = fs::canonicalize(&scratch).unwrap();
        fs::write(app.join("package.json"), "{\"private\":true}").unwrap();
        fs::write(app.join("tsconfig.json"), "{}").unwrap();
        fs::write(app.join("vite.config.ts"), "export default {plugins: []};").unwrap();
        fs::write(
            app.join("index.html"),
            "<script type=\"module\" src=\"/src/main.ts\"></script>",
        )
        .unwrap();
        fs::write(
            app.join("src/main.ts"),
            "import './present.css?raw'; import './absent.css?raw';",
        )
        .unwrap();
        let present = app.join("src/present.css");
        let absent = app.join("src/absent.css");
        fs::write(&present, "@reference '../../outside.css';").unwrap();
        let inputs = inferred_host_input_paths_for_project(&app, &app.join("tsconfig.json"));
        assert!(inputs.contains(&present));
        assert!(inputs.contains(&absent));
        let paths = inputs.iter().map(PathBuf::as_path).collect::<Vec<_>>();
        let before = identities(&paths);
        fs::write(&present, "@apply unknown;").unwrap();
        fs::write(&absent, ".x{").unwrap();
        let after = identities(&paths);
        for path in [&present, &absent] {
            let index = inputs.iter().position(|input| input == path).unwrap();
            assert_ne!(before[index], after[index]);
        }
        // Raw acquisition never interprets @reference or observes its target.
        assert!(!inputs.contains(&app.parent().unwrap().join("outside.css")));
        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn analyzed_request_projection_matches_extraction() {
        let source = "import type {T} from 'types'; import {value} from 'runtime'; export type {T} from 'other-types'; export {value} from 'reexport'; export * from 'all';";
        let ast = solid_facts::ast::extract("requests.ts", source).unwrap();
        assert_eq!(static_fact_requests(&ast), ["runtime", "reexport", "all"]);
    }

    #[test]
    fn plugin_inventory_is_fingerprinted_without_becoming_an_application_importer() {
        let scratch =
            std::env::temp_dir().join(format!("host-plugin-request-inputs-{}", std::process::id()));
        let implementation = scratch.join("node_modules/audited-plugin/dist/index.js");
        fs::create_dir_all(implementation.parent().unwrap()).unwrap();
        fs::write(&implementation, "import 'plugin-only-request';").unwrap();
        let config =
            host_config::parse(Path::new("vite.config.ts"), "export default {plugins: []};")
                .unwrap();
        let inputs = inferred_host_graph_input_paths(
            &scratch,
            &scratch.join("tsconfig.json"),
            vec![implementation.clone()],
            &config,
            None,
            Some(vec![scratch.clone()]),
            &[],
        );
        assert!(inputs.contains(&implementation));
        assert!(!inputs.iter().any(|path| {
            path.components()
                .any(|part| part.as_os_str() == "plugin-only-request")
        }));
        assert!(
            !inputs.contains(&scratch.join("node_modules/node_modules/@solidjs/web/package.json"))
        );
        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn fixture_integration_identity_accepts_exact_bytes_and_refuses_a_modified_loader() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/reactive-ir/node_modules/@solidjs/vite-plugin");
        let scratch =
            std::env::temp_dir().join(format!("solid-host-plugin-{}", std::process::id()));
        let plugin = scratch.join("node_modules/@solidjs/vite-plugin");
        for name in PLUGIN_FILES {
            let target = plugin.join(name);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::copy(source.join(name), target).unwrap();
        }
        assert!(audited_start_plugin(&scratch, &mut Vec::new()).is_some());
        fs::write(
            plugin.join("dist/esm/index.mjs"),
            "import '../../../../src/App.tsx';",
        )
        .unwrap();
        assert!(audited_start_plugin(&scratch, &mut Vec::new()).is_none());
        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn postcss_discovery_refuses_executable_side_inputs_above_the_leaf() {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let scratch = std::env::temp_dir().join(format!(
            "solid-host-postcss-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let leaf = scratch.join("app");
        fs::create_dir_all(&leaf).unwrap();
        assert!(side_inputs(&leaf, &mut Vec::new()).is_ok());
        let config = scratch.join("postcss.config.ts");
        fs::write(&config, "import './app/src/App.tsx'; export default {};").unwrap();
        assert!(side_inputs(&leaf, &mut Vec::new()).is_err());
        fs::remove_file(config).unwrap();
        fs::write(
            scratch.join("package.json"),
            "{\"postcss\":{\"plugins\":{\"./app/src/App.tsx\":{}}}}",
        )
        .unwrap();
        assert!(side_inputs(&leaf, &mut Vec::new()).is_err());
        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn route_fixture_refuses_exact_server_export_spellings() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/reactive-ir/inferred-host-spa/route-cases.json");
        let cases: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        for case in cases.as_array().unwrap() {
            let ast =
                solid_facts::ast::extract("route.tsx", case["source"].as_str().unwrap()).unwrap();
            assert_eq!(server_route(&ast), case["server"].as_bool().unwrap());
        }
    }

    #[test]
    fn published_metadata_is_an_open_entry_even_without_private() {
        assert!(unpublished(&serde_json::json!({"name":"app"})));
        for key in ["exports", "main", "module", "bin", "types", "typings"] {
            let mut package = serde_json::json!({"private":true});
            package[key] = serde_json::json!("./dist/index.js");
            assert!(!unpublished(&package), "{key}");
        }
    }

    #[test]
    fn client_probe_order_is_the_installed_integrations_order() {
        let present = [
            "src/entry-server.tsx",
            "src/Document.tsx",
            "src/app.tsx",
            "src/App.js",
            "src/App.tsx",
        ];
        assert_eq!(
            client_root(|path| present.contains(&path)),
            Some(("src/App.tsx".into(), true))
        );
        assert_eq!(
            client_root(|path| present.contains(&path) || path == "src/entry-client.mjs"),
            Some(("src/entry-client.mjs".into(), false))
        );
        assert_eq!(
            client_root(|path| ["src/Document.tsx", "src/entry-server.tsx"].contains(&path)),
            None
        );
        assert_eq!(
            client_root(|path| path == "src/app.jsx"),
            Some(("src/app.jsx".into(), true))
        );
    }

    #[test]
    fn html_module_roots_are_parsed_without_comment_or_inline_roots() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/reactive-ir/inferred-host-reachability");
        assert_eq!(
            html_entries(&fs::read_to_string(fixtures.join("csp-clear.html")).unwrap()),
            Some(vec!["src/completion-cases.ts".into()])
        );
        assert_eq!(
            html_entries(&fs::read_to_string(fixtures.join("csp-denied.html")).unwrap()),
            None
        );
        assert_eq!(
            html_entries(
                "<!doctype html><html><head><title>App</title></head><body><div id='root'></div><script type='module' src='/src/main.ts'></script></body></html>"
            ),
            Some(vec!["src/main.ts".into()])
        );
        for text in [
            "<!-- <script type='module' src='/server.ts'></script> -->",
            "<script type='module'>import '/src/main.ts'</script>",
            "<template><script type='module' src='/src/main.ts'></script></template>",
            "<base href='/server/'><script type='module' src='src/main.ts'></script>",
            "<style>@plugin './src/App.tsx';</style><script type='module' src='/src/main.ts'></script>",
            "<script type='module' src='//remote/app.js'></script>",
            "<script type='module' src='/src/main.ts?server'></script>",
        ] {
            assert_eq!(html_entries(text), None);
        }
    }

    #[test]
    fn passive_svg_does_not_hide_or_create_module_roots() {
        let path = Path::new("index.html");
        let module = "<script type='module' src='/src/main.ts'></script>";
        let passive = "<svg><defs><symbol id='shape'><path d='M0 0'/></symbol></defs><use href='#shape'/></svg>";
        assert_eq!(
            html_entries_at(path, &format!("{passive}{module}")).unwrap(),
            vec!["src/main.ts"]
        );
        for foreign in [
            "<svg><script type='module' src='/src/hidden.ts'></script></svg>",
            "<svg><foreignObject><script type='module' src='/src/hidden.ts'></script></foreignObject></svg>",
            "<svg onload='disableModules()'></svg>",
            "<svg><use href='/external.svg#shape'/></svg>",
            "<svg><defs></svg>",
            "<svg><path/></svg><use href='#shape'/>",
        ] {
            let refusal = html_entries_at(path, &format!("\n{foreign}{module}")).unwrap_err();
            assert_eq!(refusal.path.as_path(), path);
            assert_eq!(refusal.line, Some(2));
        }
        let denied = html_entries_at(path, &format!("\n<meta http-equiv='Content-Security-Policy' content=\"script-src 'none'\">{module}")).unwrap_err();
        assert!(denied.reason.contains("http-equiv"));
        assert_eq!(denied.line, Some(2));
    }
}
