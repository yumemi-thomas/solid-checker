//! Process-free Node-API/WASI entry point for browser and WebContainer hosts.

use std::path::Path;

#[cfg(feature = "reactor")]
use std::sync::Mutex;

#[cfg(feature = "napi-host")]
use napi_derive::napi;
use serde::Deserialize;
use solid_facts::TypeScriptTable;
use solid_facts_backend::SemanticDemandGroup;
use solid_facts_backend::{
    BackendError, SemanticDemandOptions, SourceFile, TypeFactsProvider, analyze_project,
    build_project_native_measured_with_demands,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CheckRequest {
    project_id: String,
    /// Stable id of the Solid dialect to check with; defaults to the
    /// backend's default dialect when the host names none.
    #[serde(default)]
    dialect: Option<String>,
    generation: u64,
    sources: Vec<SourceFile>,
    type_facts: TypeScriptTable,
    /// Where each import specifier resolves, as the host's own TypeScript
    /// engine resolved it.
    ///
    /// Optional, and its absence is the documented limitation of this entry
    /// point rather than a weaker analysis of the same request: a request
    /// without it binds package contracts by specifier *name*, exactly as this
    /// adapter always has. A host that supplies it gets contracts bound to the
    /// installed package each specifier actually resolves to, and a specifier
    /// this field does not cover is refused rather than name-matched. There is
    /// no third behavior: the field is never partially trusted.
    ///
    /// This entry point has no Type Facts session — the host has already run
    /// TypeScript and hands the finished tables in — so the resolution facts
    /// arrive the same way the type facts do.
    #[serde(default)]
    resolved_imports: Option<HostResolvedImports>,
}

/// The host's answer for where the specifiers of some importing files resolve.
///
/// Every file listed is *covered*: a file with no `imports` says "this file
/// imports nothing", which is a different fact from a file that is absent, and
/// an absent file's specifiers are refused. Spans are byte offsets into the
/// source the same request carries, and name the specifier string literal
/// itself, quotes included.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HostResolvedImports {
    #[serde(default)]
    files: Vec<HostResolvedImportFile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HostResolvedImportFile {
    path: String,
    #[serde(default)]
    imports: Vec<HostResolvedImport>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HostResolvedImport {
    start_byte: u32,
    end_byte: u32,
    text: String,
    /// `unresolved`, `relative`, `nodeModules`, or `nonRelative`, mirroring the
    /// Type Facts producer's own `ModuleResolution`. An unrecognized value is
    /// refused rather than guessed at.
    resolution: String,
    #[serde(default)]
    resolved_path: String,
    #[serde(default)]
    included_path: String,
    #[serde(default)]
    symlink_path: String,
    #[serde(default)]
    extension: String,
    /// The `name` of the nearest `package.json` above `resolvedPath`. Absent or
    /// empty means the manifest declares none, which is never read as a
    /// disagreement.
    #[serde(default)]
    package_name: String,
    #[serde(default)]
    package_version: String,
    #[serde(default)]
    package_manifest: String,
    /// The package name the host's resolver itself recorded for this
    /// specifier, which can differ from `packageName`.
    #[serde(default)]
    resolver_package_name: String,
    #[serde(default)]
    resolver_package_version: String,
}

impl HostResolvedImports {
    /// Decodes the host's rows, rejecting the ones no consumer could tell apart
    /// from a correct answer.
    ///
    /// Both checks exist because this interface's failure mode is *silence*: a
    /// contract is bound by joining one row to one declaration, and a row that
    /// cannot be joined refuses the contract exactly as a project with no
    /// contract would. A host mistake would therefore show up as contract
    /// coverage quietly varying from file to file, which is indistinguishable
    /// from a project whose contracts genuinely do not apply. So a row that
    /// cannot be right is an error here rather than a refusal later:
    ///
    /// - **`resolution` and `resolvedPath` must agree.** Empty exactly when the
    ///   resolution is `unresolved`, which is the documented invariant. A row
    ///   labelled `unresolved` is *accepted* by contract binding — nothing
    ///   resolved means nothing else claimed the specifier — so a host that
    ///   labels resolutions it did not perform would get every contract
    ///   applied. It is the one host mistake in this interface that fails open.
    /// - **The span must name the specifier in the source this request
    ///   carries.** Byte offsets, not UTF-16 code units: TypeScript reports
    ///   positions in code units, so a host that forwards them unconverted
    ///   produces spans that are correct for ASCII text and silently wrong
    ///   after the first non-ASCII character — binding for short files and not
    ///   for long ones. Comparing the source at the span against `text` is what
    ///   makes that a loud error. A specifier written with a string escape does
    ///   not compare equal (`text` is unescaped) and is refused here too:
    ///   fail-closed and loud, rather than a silent partial answer.
    fn into_index(
        self,
        sources: &[SourceFile],
    ) -> Result<solid_facts::AttestedImportIndex, String> {
        let mut index = solid_facts::AttestedImportIndex::default();
        for file in self.files {
            let mut imports = Vec::with_capacity(file.imports.len());
            for import in file.imports {
                let resolution = match import.resolution.as_str() {
                    "unresolved" => solid_facts::ImportResolution::Unresolved,
                    "relative" => solid_facts::ImportResolution::Relative,
                    "nodeModules" => solid_facts::ImportResolution::NodeModules,
                    "nonRelative" => solid_facts::ImportResolution::NonRelative,
                    other => {
                        return Err(format!("unknown module resolution {other:?}"));
                    }
                };
                let unresolved = resolution == solid_facts::ImportResolution::Unresolved;
                if unresolved != import.resolved_path.is_empty() {
                    return Err(format!(
                        "resolved import {:?} in {:?} is {:?} with resolvedPath {:?}: the path is \
                         empty exactly when the resolution is \"unresolved\"",
                        import.text, file.path, import.resolution, import.resolved_path
                    ));
                }
                verify_specifier_span(sources, &file.path, &import)?;
                imports.push(solid_facts::AttestedImport {
                    span: solid_facts::core::Span::new(import.start_byte, import.end_byte),
                    text: import.text.into(),
                    resolution,
                    resolved_path: import.resolved_path.into(),
                    included_path: import.included_path.into(),
                    symlink_path: import.symlink_path.into(),
                    extension: import.extension.into(),
                    package_name: Some(import.package_name)
                        .filter(|name| !name.is_empty())
                        .map(Into::into),
                    package_version: Some(import.package_version)
                        .filter(|version| !version.is_empty())
                        .map(Into::into),
                    package_manifest: Some(import.package_manifest)
                        .filter(|path| !path.is_empty())
                        .map(Into::into),
                    resolver_package_name: Some(import.resolver_package_name)
                        .filter(|name| !name.is_empty())
                        .map(Into::into),
                    resolver_package_version: Some(import.resolver_package_version)
                        .filter(|version| !version.is_empty())
                        .map(Into::into),
                });
            }
            index.insert_file(file.path, imports);
        }
        Ok(index)
    }
}

/// Whether one row's span really names its specifier in the source this request
/// carries.
///
/// The producer's own rows name the string literal with its quotes; a host that
/// hands the literal's interior instead is equally unambiguous evidence that
/// the offsets are bytes into this source, so both are accepted. Anything else
/// — a span past the end of the source, one that lands mid-character, or one
/// whose text is something other than this specifier — is a host mistake, and
/// this is the only place it can be told apart from a project whose contracts
/// legitimately do not apply.
fn verify_specifier_span(
    sources: &[SourceFile],
    path: &str,
    import: &HostResolvedImport,
) -> Result<(), String> {
    let Some(file) = sources.iter().find(|source| source.path == path) else {
        return Err(format!(
            "resolved imports name file {path:?}, which this request does not carry"
        ));
    };
    let source: &str = &file.source;
    let start = import.start_byte as usize;
    let end = import.end_byte as usize;
    let found = (start <= end && end <= source.len())
        .then(|| source.get(start..end))
        .flatten()
        .ok_or_else(|| {
            format!(
                "resolved import {:?} in {path:?} spans bytes {start}..{end}, which is not inside \
                 that source (offsets are UTF-8 bytes, not UTF-16 code units)",
                import.text
            )
        })?;
    let quoted = found
        .strip_prefix(['"', '\'', '`'])
        .and_then(|inner| inner.strip_suffix(['"', '\'', '`']));
    if found == import.text || quoted == Some(import.text.as_str()) {
        return Ok(());
    }
    Err(format!(
        "resolved import {:?} in {path:?} spans bytes {start}..{end}, where the source reads \
         {found:?} (offsets are UTF-8 bytes, not UTF-16 code units)",
        import.text
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlanRequest {
    project_id: String,
    #[serde(default)]
    dialect: Option<String>,
    generation: u64,
    sources: Vec<SourceFile>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanResponse {
    project_id: String,
    generation: u64,
    demands: Vec<typefacts::v3::EntityDemand>,
}

/// The host has already run TypeScript and hands the finished table in, so
/// there is no producer and no demand round trip: the one table answers the
/// single analysis this entry point performs.
struct InMemoryTypeFacts(Option<TypeScriptTable>);

impl TypeFactsProvider for InMemoryTypeFacts {
    fn semantic_grouped(
        &mut self,
        _groups: &[SemanticDemandGroup<'_>],
    ) -> Result<TypeScriptTable, BackendError> {
        self.0
            .take()
            .ok_or_else(|| BackendError::Process("TypeFacts response was already consumed".into()))
    }
}

/// Error text the planning provider ends the build with once the demand plan
/// is in hand. It is an internal signal, never a host-visible failure:
/// [`plan`] recognizes its own stop and answers with the captured plan.
const PLANNING_STOP: &str = "demand plan captured";

/// Records the demand plan the backend computes, then stops the build.
///
/// The backend has no plan-only entry point, and the demands exist for exactly
/// as long as the one round trip that would hand them to a producer. Ending
/// that round trip in an error is the cheap early stop: it skips the hydrate,
/// join, and `ProjectFacts` that planning would only discard, and it avoids
/// fabricating a Type Facts table whose project identity and source digests
/// nothing would ever certify.
#[derive(Default)]
struct PlanningTypeFacts {
    demands: Option<Vec<typefacts::v3::EntityDemand>>,
}

impl TypeFactsProvider for PlanningTypeFacts {
    fn semantic_grouped(
        &mut self,
        groups: &[SemanticDemandGroup<'_>],
    ) -> Result<TypeScriptTable, BackendError> {
        self.demands = Some(
            groups
                .iter()
                .flat_map(|group| group.demands.iter().cloned())
                .collect(),
        );
        Err(BackendError::Process(PLANNING_STOP.into()))
    }
}

/// Prototype seam for a self-contained browser host.
///
/// It runs the in-process syntax and Solid compiler passes and returns the
/// exact semantic demands a browser-side TypeScript-Go reactor must answer.
#[cfg(feature = "napi-host")]
#[napi]
pub fn plan_sync(request_json: String) -> napi::Result<String> {
    plan(&request_json).map_err(|error| napi::Error::from_reason(error.to_string()))
}

/// Known prototype approximation: a host that plans and then checks parses and
/// compiles the same sources twice, because the demand plan is a by-product of
/// the fact build rather than an output of its own backend entry point. The
/// stop below removes the Type Facts round trip and everything after it, not
/// the syntax and Solid compiler passes that precede it.
pub fn plan(request_json: &str) -> Result<String, Box<dyn std::error::Error>> {
    let request: PlanRequest = serde_json::from_str(request_json)?;
    let dialect = match request.dialect.as_deref() {
        None => solid_facts_backend::dialect::default_dialect(),
        Some(id) => solid_facts_backend::dialect::by_id(id)
            .ok_or_else(|| format!("unknown dialect {id:?}"))?,
    };
    let mut typescript = PlanningTypeFacts::default();
    let built = build_project_native_measured_with_demands(
        dialect,
        request.project_id.clone(),
        request.generation,
        request.sources,
        &mut typescript,
        SemanticDemandOptions::PREFERENCES,
    )
    .map(|(facts, _)| facts);
    let demands = match (typescript.demands, built) {
        // The planned stop: the demands were captured, so the error that ended
        // the build is this function's own signal rather than a failure.
        (Some(demands), _) => demands,
        (None, Err(error)) => return Err(error.into()),
        (None, Ok(_)) => return Err("the fact build requested no Type Facts".into()),
    };
    Ok(serde_json::to_string(&PlanResponse {
        project_id: request.project_id,
        generation: request.generation,
        demands,
    })?)
}

/// Analyze an in-memory project without spawning native processes.
///
/// The host supplies the TypeFacts closure produced by the browser-side
/// TypeScript engine. The result is the same JSON snapshot emitted by the CLI.
#[cfg(feature = "napi-host")]
#[napi]
pub fn check_sync(request_json: String) -> napi::Result<String> {
    check(&request_json).map_err(|error| napi::Error::from_reason(error.to_string()))
}

pub fn check(request_json: &str) -> Result<String, Box<dyn std::error::Error>> {
    let request: CheckRequest = serde_json::from_str(request_json)?;
    let dialect = match request.dialect.as_deref() {
        None => solid_facts_backend::dialect::default_dialect(),
        Some(id) => solid_facts_backend::dialect::by_id(id)
            .ok_or_else(|| format!("unknown dialect {id:?}"))?,
    };
    let mut typescript = InMemoryTypeFacts(Some(request.type_facts));
    let (mut facts, _) = build_project_native_measured_with_demands(
        dialect,
        request.project_id.clone(),
        request.generation,
        request.sources.clone(),
        &mut typescript,
        SemanticDemandOptions::PREFERENCES,
    )?;
    // Absent field, absent table: package contracts stay name-matched, which
    // is what this adapter has always done and is stated as its limitation in
    // docs/package-contracts.md. Supplying the field is the only way to get
    // identity-bound contracts here, and it is all-or-nothing per specifier.
    facts.resolved_imports = request
        .resolved_imports
        .map(|resolved| resolved.into_index(&request.sources))
        .transpose()?;
    let analysis = analyze_project(
        dialect,
        Path::new(&request.project_id),
        &request.sources,
        &facts,
        &[],
    )?;
    Ok(serde_json::to_string(&analysis.snapshot)?)
}

#[cfg(feature = "reactor")]
static REACTOR_INPUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());

#[cfg(feature = "reactor")]
static REACTOR_OUTPUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// Allocate the request buffer used by the process-free WASI reactor.
#[cfg(feature = "reactor")]
#[unsafe(no_mangle)]
pub extern "C" fn allocate_input(size: u32) -> u32 {
    let mut input = REACTOR_INPUT.lock().expect("reactor input lock poisoned");
    input.resize(size as usize, 0);
    input.as_mut_ptr() as u32
}

#[cfg(feature = "reactor")]
fn run_reactor(operation: fn(&str) -> Result<String, Box<dyn std::error::Error>>) -> u32 {
    let result = {
        let input = REACTOR_INPUT.lock().expect("reactor input lock poisoned");
        std::str::from_utf8(&input)
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
            .and_then(operation)
    };
    let (status, encoded) = match result {
        Ok(response) => (0, response.into_bytes()),
        Err(error) => (
            1,
            serde_json::to_vec(&serde_json::json!({ "error": error.to_string() }))
                .expect("serialize reactor error"),
        ),
    };
    *REACTOR_OUTPUT.lock().expect("reactor output lock poisoned") = encoded;
    status
}

/// Plan the exact Type Facts demands for the encoded source request.
#[cfg(feature = "reactor")]
#[unsafe(no_mangle)]
pub extern "C" fn run_plan() -> u32 {
    run_reactor(plan)
}

/// Analyze the encoded source and Type Facts request.
#[cfg(feature = "reactor")]
#[unsafe(no_mangle)]
pub extern "C" fn run_check() -> u32 {
    run_reactor(check)
}

#[cfg(feature = "reactor")]
#[unsafe(no_mangle)]
pub extern "C" fn output_pointer() -> u32 {
    REACTOR_OUTPUT
        .lock()
        .expect("reactor output lock poisoned")
        .as_ptr() as u32
}

#[cfg(feature = "reactor")]
#[unsafe(no_mangle)]
pub extern "C" fn output_length() -> u32 {
    REACTOR_OUTPUT
        .lock()
        .expect("reactor output lock poisoned")
        .len() as u32
}
