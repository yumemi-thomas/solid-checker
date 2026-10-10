//! Static client linking only. This premise grants no package call semantics,
//! owner/tracking facts, completion, artifact authentication or certification.

use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use solid_facts::resolution::{AttestedImport, ImportResolution};
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug)]
pub(super) enum PackageLoadability {
    AuditedDialect(Vec<PathBuf>),
    Installed(Vec<PathBuf>),
}

impl PackageLoadability {
    pub(super) fn entries(&self) -> &[PathBuf] {
        match self {
            Self::AuditedDialect(entries) | Self::Installed(entries) => entries,
        }
    }
}

// serde_json::Value sorts object keys in this workspace. Conditional exports
// instead use author order, including default before browser/import. Keep that
// order locally without changing the workspace's JSON serialization contract.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Target {
    String(String),
    Object(Ordered),
    // Arrays are deliberately unsupported: do not guess fallback semantics.
    Other(serde_json::Value),
}

#[derive(Debug)]
struct Ordered(Vec<(String, Target)>);

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct OrderedVisitor;
        impl<'de> Visitor<'de> for OrderedVisitor {
            type Value = Ordered;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an ordered exports object without duplicate keys")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Ordered, M::Error> {
                let mut rows = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, Target>()? {
                    if rows.iter().any(|(previous, _)| previous == &key) {
                        return Err(serde::de::Error::custom("duplicate exports key"));
                    }
                    rows.push((key, value));
                }
                Ok(Ordered(rows))
            }
        }
        deserializer.deserialize_map(OrderedVisitor)
    }
}

// None means no applicable condition; Some(None) means an applicable but
// unsupported/blocked target. Never fall through a selected null to default.
fn conditional(target: &Target, production: bool, solid: bool) -> Option<Option<&str>> {
    match target {
        Target::String(target) => Some(Some(target)),
        Target::Object(Ordered(rows)) => {
            if rows
                .iter()
                .any(|(key, _)| key.starts_with('.') || key.parse::<u32>().is_ok())
            {
                return Some(None);
            }
            for (key, value) in rows {
                if (["module", "browser", "import", "default"].contains(&key.as_str())
                    || (solid && key == "solid")
                    || key
                        == if production {
                            "production"
                        } else {
                            "development"
                        })
                    && let Some(selected) = conditional(value, production, solid)
                {
                    return Some(selected);
                }
            }
            None
        }
        Target::Other(value) => {
            // Retain the unsupported value in the parsed representation so
            // false/null/arrays cannot be confused with an absent exports field.
            let _ = value;
            Some(None)
        }
    }
}

fn exported(exports: &Target, key: &str, production: bool, solid: bool) -> Option<String> {
    if let Target::Object(Ordered(rows)) = exports
        && rows.iter().any(|(key, _)| key.starts_with('.'))
    {
        if rows.iter().any(|(key, _)| !key.starts_with('.')) {
            return None;
        }
        if let Some((_, target)) = rows
            .iter()
            .find(|(name, _)| name == key && !name.contains('*'))
        {
            return conditional(target, production, solid)?.map(str::to_owned);
        }
        // Node/Vite specificity: longest pattern base, then longest whole key.
        // Match the specifier's exact subpath; never treat a wildcard as trust.
        let (_, target, capture) = rows
            .iter()
            .filter_map(|(pattern, target)| {
                let (prefix, suffix) = pattern.split_once('*')?;
                if suffix.contains('*') {
                    return None;
                }
                let capture = key.strip_prefix(prefix)?.strip_suffix(suffix)?;
                (!capture.is_empty()).then_some((pattern, target, capture))
            })
            .max_by_key(|(pattern, _, _)| (pattern.find('*').unwrap_or(0), pattern.len()))?;
        return conditional(target, production, solid)?.map(|target| target.replace('*', capture));
    }
    (key == ".")
        .then(|| conditional(exports, production, solid))
        .flatten()
        .flatten()
        .map(str::to_owned)
}

pub(super) fn request(text: &str) -> Option<(&str, String)> {
    if text.is_empty() || text.contains(['?', '#', '%', '\\', ':']) || text.starts_with(['.', '/'])
    {
        return None;
    }
    let end = if text.starts_with('@') {
        let scope = text.find('/')?;
        if scope == 1 {
            return None;
        }
        text[scope + 1..]
            .find('/')
            .map_or(text.len(), |end| scope + 1 + end)
    } else {
        text.find('/').unwrap_or(text.len())
    };
    let name = &text[..end];
    if name.ends_with('/')
        || name
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return None;
    }
    let key = format!(".{}", &text[end..]);
    if key != "." && !clean_relative(&key) {
        return None;
    }
    Some((name, key))
}

fn clean_relative(target: &str) -> bool {
    !target.is_empty()
        && !target.contains(['?', '#', '%', '\\', ':'])
        && !Path::new(target).is_absolute()
        && Path::new(target).components().all(|part| match part {
            Component::CurDir => true,
            Component::Normal(name) => name != "node_modules",
            _ => false,
        })
}

fn regular(root: &Path, target: &str, inputs: &mut Vec<PathBuf>) -> Option<PathBuf> {
    if !clean_relative(target) {
        return None;
    }
    let path = root.join(target);
    inputs.push(path.clone());
    // Fingerprint directory membership and every path component too. A parent
    // symlink retarget must invalidate daemon and in-flight reuse.
    inputs.extend(
        path.ancestors()
            .take_while(|path| path.starts_with(root))
            .map(Path::to_owned),
    );
    let physical = fs::canonicalize(&path).ok()?;
    inputs.push(physical.clone());
    if !physical.starts_with(root) || !fs::metadata(&physical).ok()?.is_file() {
        return None;
    }
    if [".d.ts", ".d.mts", ".d.cts", ".node"]
        .iter()
        .any(|suffix| physical.to_str().is_some_and(|path| path.ends_with(suffix)))
    {
        return None;
    }
    Some(physical)
}

fn legacy_file(root: &Path, target: &str, inputs: &mut Vec<PathBuf>) -> Option<PathBuf> {
    if !clean_relative(target) {
        return None;
    }
    // Vite selects the first existing target, then default extensions/index.
    // A directory package.json is a separate resolver branch, left unknown.
    let candidate = root.join(target);
    inputs.push(candidate.clone());
    if candidate.exists() && !candidate.is_dir() {
        return regular(root, target, inputs);
    }
    for extension in [".mjs", ".js", ".mts", ".ts", ".jsx", ".tsx", ".json"] {
        let name = format!("{target}{extension}");
        inputs.push(root.join(&name));
        if root.join(&name).is_file() {
            return regular(root, &name, inputs);
        }
    }
    inputs.push(candidate.join("package.json"));
    if candidate.join("package.json").exists() {
        return None;
    }
    for extension in [".mjs", ".js", ".mts", ".ts", ".jsx", ".tsx", ".json"] {
        let name = format!("{target}/index{extension}");
        inputs.push(root.join(&name));
        if root.join(&name).is_file() {
            return regular(root, &name, inputs);
        }
    }
    None
}

fn browser_map(manifest: &serde_json::Value, target: &str) -> Option<String> {
    let Some(browser) = manifest.get("browser") else {
        return Some(target.into());
    };
    if browser == &serde_json::Value::Bool(false) {
        return None;
    }
    if let Some(map) = browser.as_object() {
        // Vite also matches extension/index aliases. Refuse ambiguous mappings
        // rather than choosing sorted JSON order over author order.
        let normal = |text: &str| {
            text.trim_start_matches("./")
                .trim_end_matches("/index.js")
                .trim_end_matches(".js")
                .to_owned()
        };
        let matches = map
            .iter()
            .filter(|(key, _)| normal(key) == normal(target))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Some(target.into()),
            [(_, value)] => value.as_str().map(str::to_owned),
            _ => None,
        }
    } else if browser.is_string() {
        Some(target.into())
    } else {
        None
    }
}

fn entry(
    root: &Path,
    bytes: &[u8],
    key: &str,
    production: bool,
    solid: bool,
    inputs: &mut Vec<PathBuf>,
) -> Option<PathBuf> {
    let manifest: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    // Deserializing the whole object this way also refuses duplicate metadata.
    let Ordered(fields): Ordered = serde_json::from_slice(bytes).ok()?;
    if manifest.get("browser") == Some(&serde_json::Value::Bool(false)) {
        return None;
    }
    if let Some((_, exports)) = fields.iter().find(|(name, _)| name == "exports") {
        let target = exported(exports, key, production, solid)?;
        if !target.starts_with("./") {
            return None;
        }
        // Root entries may be browser-remapped by Vite after exports selection.
        let target = if key == "." {
            browser_map(&manifest, &target)?
        } else {
            target
        };
        return regular(root, &target, inputs);
    }
    let target = if key != "." {
        key.to_owned()
    } else {
        let browser = manifest.get("browser");
        let field = browser
            .and_then(serde_json::Value::as_str)
            .or_else(|| {
                browser
                    .and_then(serde_json::Value::as_object)
                    .and_then(|map| map.get("."))
                    .and_then(serde_json::Value::as_str)
            })
            .or_else(|| {
                ["module", "jsnext:main", "jsnext", "main"]
                    .iter()
                    .find_map(|key| manifest.get(key).and_then(serde_json::Value::as_str))
            });
        if browser
            .and_then(serde_json::Value::as_object)
            .is_some_and(|map| map.get(".") == Some(&serde_json::Value::Bool(false)))
        {
            return None;
        }
        field.unwrap_or("index.js").to_owned()
    };
    let target = browser_map(&manifest, &target)?;
    legacy_file(root, &target, inputs)
}

/// Vite 8.3.0's cssPostPlugin/assetPlugin generate inert export maps or
/// strings. A resource grants linking only, never callback/component behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ResourceKind {
    Stylesheet,
    CssModule,
    String,
}

pub(super) struct Resource {
    pub kind: ResourceKind,
    pub entries: Vec<PathBuf>,
}

impl Resource {
    pub(super) fn matches_outcome(
        &self,
        outcome: &solid_facts::runtime_resolution::RuntimeOutcome,
    ) -> bool {
        matches!(outcome, solid_facts::runtime_resolution::RuntimeOutcome::File { path, physical_path }
            if path == physical_path && self.entries.iter().any(|entry| Path::new(path.as_ref()) == entry.as_path()))
    }
}

pub(super) fn resource_request(text: &str) -> bool {
    let (path, query) = text.split_once('?').unwrap_or((text, ""));
    if !query.is_empty() {
        return ["raw", "url"].contains(&query);
    }
    Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|ext| {
            [
                "css",
                "pcss",
                "postcss",
                "less",
                "sass",
                "scss",
                "styl",
                "stylus",
                "sss",
                "apng",
                "bmp",
                "png",
                "jpg",
                "jpeg",
                "jfif",
                "pjpeg",
                "pjp",
                "gif",
                "svg",
                "ico",
                "webp",
                "avif",
                "cur",
                "jxl",
                "mp4",
                "webm",
                "ogg",
                "mp3",
                "wav",
                "flac",
                "aac",
                "opus",
                "mov",
                "m4a",
                "vtt",
                "woff",
                "woff2",
                "eot",
                "ttf",
                "otf",
                "webmanifest",
                "pdf",
                "txt",
            ]
            .contains(&ext)
        })
}

/// Used before the daemon shortcut and again at admission. Exact package
/// exports (including browser/condition refusals) or a literal local filename,
/// not a .d.ts wildcard or an unresolved Type Facts row, prove acquisition.
pub(super) fn resource(
    directory: &Path,
    importer: &Path,
    text: &str,
    config: &crate::host_config::Config,
    inputs: &mut Vec<PathBuf>,
) -> Option<Resource> {
    if !resource_request(text) || text.contains(['#', '%', '\\']) {
        return None;
    }
    // Config is already audited and closed. Only plugins whose reviewed paths
    // do not claim these resource extensions can share the builtin loader.
    // A future SVG-component plugin must explicitly override this proof.
    if config.plugins.iter().any(|name| {
        ![
            // Config.plugins includes audited defineConfig imports as well as
            // registered plugins. These do not install a resource transform;
            // the closed config grammar preserves Vite's builtin CSS loader.
            "vite",
            "vitest/config",
            "@solidjs/vite-plugin",
            "vite-plugin-solid",
            "@tailwindcss/vite",
            "filesystem-routing/vite",
            "vite-tsconfig-paths",
            "@tanstack/router-plugin/vite",
            "unocss/vite",
            "@unocss/vite",
        ]
        .contains(&name.as_str())
    }) {
        return None;
    }
    let (request, query) = text.split_once('?').unwrap_or((text, ""));
    if !query.is_empty() && !["raw", "url"].contains(&query) {
        return None;
    }
    if config.aliases.keys().any(|find| {
        request == find
            || request
                .strip_prefix(find.as_str())
                .is_some_and(|tail| tail.starts_with('/'))
    }) {
        // Alias resource linking requires its own exact resolver proof.
        return None;
    }
    let entries = if request.starts_with('.') {
        let path = importer.parent()?.join(request);
        inputs.push(path.clone());
        inputs.extend(
            path.ancestors()
                .take_while(|path| path.starts_with(directory))
                .map(Path::to_owned),
        );
        let physical = fs::canonicalize(&path).ok()?;
        inputs.push(physical.clone());
        if !physical.starts_with(directory) || !fs::metadata(&physical).ok()?.is_file() {
            return None;
        }
        vec![physical]
    } else {
        if config.package_dedupe {
            return None;
        }
        candidates(importer, request, inputs)?.2
    };
    let requested = Path::new(request)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");
    let stylesheet = [
        "css", "pcss", "postcss", "less", "sass", "scss", "styl", "stylus", "sss",
    ]
    .contains(&requested);
    // A package must not redirect a .css/.svg request to executable JavaScript.
    if query.is_empty()
        && entries
            .iter()
            .any(|path| path.extension().and_then(|ext| ext.to_str()) != Some(requested))
    {
        return None;
    }
    let is_module = |path: &Path| {
        path.to_str().is_some_and(|path| {
            [
                ".module.css",
                ".module.pcss",
                ".module.postcss",
                ".module.less",
                ".module.sass",
                ".module.scss",
                ".module.styl",
                ".module.stylus",
                ".module.sss",
            ]
            .iter()
            .any(|suffix| path.ends_with(suffix))
        })
    };
    if query == "url" {
        // Vite explicitly rejects module CSS ?url; ordinary CSS ?url still
        // compiles the stylesheet in builds, so it needs the CSS closure proof.
        for path in &entries {
            if is_module(path) {
                return None;
            }
            let ext = path.extension().and_then(|ext| ext.to_str());
            let css = ext.is_some_and(|ext| {
                [
                    "css", "pcss", "postcss", "less", "sass", "scss", "styl", "stylus", "sss",
                ]
                .contains(&ext)
            });
            if css
                && (ext.is_none_or(|ext| !["css", "pcss", "postcss"].contains(&ext))
                    || !passive_css(
                        &fs::read_to_string(path).ok()?,
                        config.tailwind && request.starts_with('.'),
                    ))
            {
                return None;
            }
        }
    }
    let kind = if !query.is_empty() {
        ResourceKind::String
    } else if stylesheet {
        // Preprocessor availability/side inputs need independent proof;
        // their files can still be imported explicitly as raw/url strings.
        if !["css", "pcss", "postcss"].contains(&requested) {
            return None;
        }
        for path in &entries {
            let source = fs::read_to_string(path).ok()?;
            if !passive_css(&source, config.tailwind && request.starts_with('.')) {
                return None;
            }
        }
        let modules = entries.iter().all(|path| is_module(path));
        if !modules && entries.iter().any(|path| is_module(path)) {
            return None;
        }
        if modules {
            ResourceKind::CssModule
        } else {
            ResourceKind::Stylesheet
        }
    } else {
        ResourceKind::String
    };
    Some(Resource { kind, entries })
}

// Loader closure identity proves which code runs, not that arbitrary CSS
// successfully transforms. Until a bounded transform-success fact exists,
// compiled stylesheets (including CSS ?url) supply no linking authority.
// Raw CSS remains an inert string and never calls this predicate. This also
// withholds every directive/escape and every external stylesheet closure.
fn passive_css(_source: &str, _tailwind: bool) -> bool {
    false
}

/// Record even failed candidate lookups before the daemon's shortcut. The same
/// resolver is used for admission and cache inputs; it never executes a package.
pub(super) fn candidates(
    importer: &Path,
    text: &str,
    inputs: &mut Vec<PathBuf>,
) -> Option<(PathBuf, Vec<u8>, Vec<PathBuf>)> {
    let (name, key) = request(text)?;
    let root = crate::host_plugins::installed(importer.parent()?, name, inputs)?;
    let manifest = root.join("package.json");
    inputs.push(root.clone());
    inputs.push(manifest.clone());
    let bytes = fs::read(manifest).ok()?;
    let mut entries = Vec::new();
    // No build mode is inferred. Both deployment/default serve choices must
    // resolve, even if their selected files differ. Resolver opt-in can then
    // join the actual choice without overriding a failed default proof.
    for production in [false, true] {
        for solid in [false, true] {
            entries.push(entry(&root, &bytes, &key, production, solid, inputs)?);
        }
    }
    Some((root, bytes, entries))
}

pub(super) fn loadable(
    importer: &Path,
    text: &str,
    row: &AttestedImport,
    dialect: &dyn solid_dialect::Dialect,
    inputs: &mut Vec<PathBuf>,
    excluded: &std::collections::BTreeSet<String>,
) -> Option<PackageLoadability> {
    if row.resolution != ImportResolution::NodeModules || !row.included_path.is_empty() {
        return None;
    }
    let (name, _) = request(text)?;
    let (root, bytes, entries) = candidates(importer, text, inputs)?;
    let manifest: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    inputs.push(PathBuf::from(row.resolved_path.as_ref()));
    if let Some(path) = &row.package_manifest {
        inputs.push(PathBuf::from(path.as_ref()));
    }
    let installed_name = manifest.get("name")?.as_str()?;
    let version = manifest.get("version")?.as_str()?;
    if installed_name != name
        || row
            .resolver_package_name
            .as_deref()
            .or(row.package_name.as_deref())
            != Some(name)
        || row
            .resolver_package_version
            .as_deref()
            .or(row.package_version.as_deref())
            != Some(version)
        || !Path::new(row.resolved_path.as_ref()).starts_with(&root)
        || fs::canonicalize(row.resolved_path.as_ref()).ok()?.as_path()
            != Path::new(row.resolved_path.as_ref())
        || !Path::new(row.resolved_path.as_ref()).is_file()
        || fs::canonicalize(row.package_manifest.as_deref()?)
            .ok()?
            .parent()
            .is_none_or(|parent| !parent.starts_with(&root))
    {
        return None;
    }
    // Reuse the dialect's single audited triple; do not bake an rc number or
    // package list into shared discovery. Unaudited releases are equally
    // loadable under the published-export premise; SC9014 still owns semantics.
    let audited = dialect
        .audited_installation()
        .iter()
        .any(|(package, audited_version)| *package == name && *audited_version == version);
    for file in &entries {
        let commonjs = file.extension().is_some_and(|extension| extension == "cjs")
            || (file.extension().is_some_and(|extension| extension == "js")
                && manifest.get("type").and_then(serde_json::Value::as_str) != Some("module"));
        // Workspace-linked packages outside node_modules are source to Vite,
        // not automatically prebundled dependencies.
        if commonjs
            && !root
                .components()
                .any(|part| matches!(part, Component::Normal(name) if name == "node_modules"))
        {
            return None;
        }
        if commonjs
            && excluded.iter().any(|request| {
                text == request
                    || name == request
                    || text
                        .strip_prefix(request)
                        .is_some_and(|tail| tail.starts_with('/'))
            })
        {
            return None;
        }
    }
    Some(if audited {
        PackageLoadability::AuditedDialect(entries)
    } else {
        PackageLoadability::Installed(entries)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exported(exports: &Target, key: &str, production: bool) -> Option<String> {
        super::exported(exports, key, production, false)
    }

    fn attested(root: &Path, name: &str, version: &str) -> AttestedImport {
        AttestedImport {
            span: solid_facts::core::Span::new(0, 1),
            text: name.into(),
            resolution: ImportResolution::NodeModules,
            resolved_path: root.join("index.d.ts").to_str().unwrap().into(),
            included_path: "".into(),
            symlink_path: "".into(),
            extension: ".d.ts".into(),
            package_name: Some(name.into()),
            package_version: Some(version.into()),
            package_manifest: Some(root.join("package.json").to_str().unwrap().into()),
            resolver_package_name: Some(name.into()),
            resolver_package_version: Some(version.into()),
        }
    }

    #[test]
    fn vite_resource_files_and_refusal_twins() {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let app = std::env::temp_dir().join(format!(
            "host-resources-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(app.join("node_modules/styles/dist")).unwrap();
        let app = fs::canonicalize(app).unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/reactive-ir/inferred-host-spa/resource-cases.json");
        let cases: Vec<serde_json::Value> =
            serde_json::from_slice(&fs::read(fixture).unwrap()).unwrap();
        for case in cases {
            fs::write(
                app.join("node_modules/styles/package.json"),
                case["manifest"].as_str().unwrap(),
            )
            .unwrap();
            fs::write(
                app.join("node_modules/styles/dist/style.css"),
                case["css"].as_str().unwrap(),
            )
            .unwrap();
            fs::write(app.join("node_modules/styles/dist/runtime.js"), "throw 0").unwrap();
            fs::write(app.join("local.css"), case["css"].as_str().unwrap()).unwrap();
            fs::write(app.join("local.module.css"), case["css"].as_str().unwrap()).unwrap();
            fs::write(app.join("logo.svg"), "<svg/>").unwrap();
            fs::write(app.join("style.scss"), ".x{color:red}").unwrap();
            fs::write(app.join("payload.txt"), "throw 0").unwrap();
            let mut config = crate::host_config::parse(
                Path::new("vite.config.ts"),
                case["config"]
                    .as_str()
                    .unwrap_or("export default {plugins:[]}"),
            )
            .unwrap();
            if let Some(plugin) = case["plugin"].as_str() {
                config.plugins.insert(plugin.into());
            }
            let mut inputs = Vec::new();
            let result = resource(
                &app,
                &app.join("main.ts"),
                case["request"].as_str().unwrap(),
                &config,
                &mut inputs,
            );
            assert_eq!(
                result.is_some(),
                case["loadable"].as_bool().unwrap(),
                "{}",
                case["name"]
            );
            if let Some(result) = result {
                assert!(!result.entries.is_empty());
                assert!(result.entries.iter().all(|path| inputs.contains(path)));
                if let Some(kind) = case["kind"].as_str() {
                    assert_eq!(format!("{:?}", result.kind), kind, "{}", case["name"]);
                }
            }
        }
        let config =
            crate::host_config::parse(Path::new("vite.config.ts"), "export default {plugins:[]}")
                .unwrap();
        let mut inputs = Vec::new();
        assert!(
            resource(
                &app,
                &app.join("main.ts"),
                "./absent.svg",
                &config,
                &mut inputs
            )
            .is_none()
        );
        assert!(
            inputs.contains(&app.join("./absent.svg")),
            "absence is a cache input"
        );
        assert!(!passive_css("@\\69mport 'hidden.css';", false));
        assert!(!passive_css("@con/**/fig 'hidden.ts';", false));
        assert!(!passive_css("@import \"tailwindcss\"; .x{color:red}", true));
        assert!(!passive_css("@import \"tailwindcss\";", false));
        fs::remove_dir_all(app).unwrap();
    }

    #[test]
    fn resource_resolution_is_exact_per_occurrence_and_never_overrides_a_requested_unknown() {
        use solid_facts::runtime_resolution::{RuntimeOutcome, RuntimeResolutionIndex};
        let resource = Resource {
            kind: ResourceKind::String,
            entries: vec![PathBuf::from("/app/logo.svg")],
        };
        let span = solid_facts::core::Span::new(7, 17);
        let mut runtime = RuntimeResolutionIndex::default();
        assert!(!resource.matches_outcome(runtime.outcome("/app/main.ts", span, "./logo.svg")));
        runtime.insert(
            "/app/main.ts",
            span,
            "./logo.svg",
            RuntimeOutcome::File {
                path: "/app/logo.svg".into(),
                physical_path: "/app/logo.svg".into(),
            },
        );
        assert!(resource.matches_outcome(runtime.outcome("/app/main.ts", span, "./logo.svg")));
        assert!(!resource.matches_outcome(runtime.outcome("/app/main.ts", span, "./logo.svg?url")));
        for outcome in [
            RuntimeOutcome::Unknown,
            RuntimeOutcome::External,
            RuntimeOutcome::Builtin,
            RuntimeOutcome::File {
                path: "/app/other.svg".into(),
                physical_path: "/app/other.svg".into(),
            },
            RuntimeOutcome::File {
                path: "/app/logo.svg".into(),
                physical_path: "/store/logo.svg".into(),
            },
        ] {
            assert!(!resource.matches_outcome(&outcome));
        }
    }

    #[test]
    fn attested_install_core_triple_commonjs_and_store_twins() {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let app = std::env::temp_dir().join(format!(
            "host-installed-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&app).unwrap();
        let app = fs::canonicalize(app).unwrap();
        let importer = app.join("main.ts");
        let dialect = crate::dialect::default_dialect().vocabulary;
        for &(name, version) in dialect.audited_installation() {
            let root = app.join("node_modules").join(name);
            fs::create_dir_all(&root).unwrap();
            fs::write(
                root.join("index.d.ts"),
                "export declare const value: number;",
            )
            .unwrap();
            fs::write(root.join("client.js"), "export const value = 1;").unwrap();
            let manifest = |version: &str| {
                serde_json::to_vec(&serde_json::json!({"name":name,"version":version,"type":"module","exports":{".":{"browser":"./client.js","default":"./missing.js"}}})).unwrap()
            };
            fs::write(root.join("package.json"), manifest(version)).unwrap();
            let mut row = attested(&root, name, version);
            let load = |row: &AttestedImport| {
                loadable(
                    &importer,
                    name,
                    row,
                    dialect,
                    &mut Vec::new(),
                    &Default::default(),
                )
            };
            assert!(matches!(
                load(&row),
                Some(PackageLoadability::AuditedDialect(_))
            ));
            row.resolver_package_version = Some("wrong".into());
            assert!(load(&row).is_none());
            fs::write(root.join("package.json"), manifest("2.0.0-rc.999")).unwrap();
            let row = attested(&root, name, "2.0.0-rc.999");
            assert!(matches!(load(&row), Some(PackageLoadability::Installed(_))));
            fs::remove_file(root.join("client.js")).unwrap();
            assert!(load(&row).is_none());
        }
        let name = "commonjs-package";
        let root = app.join("node_modules").join(name);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"name":"commonjs-package","version":"1.0.0","main":"./index.cjs"}"#,
        )
        .unwrap();
        fs::write(root.join("index.cjs"), "module.exports={value:1};").unwrap();
        fs::write(
            root.join("index.d.ts"),
            "export declare const value: number;",
        )
        .unwrap();
        let row = attested(&root, name, "1.0.0");
        assert!(
            loadable(
                &importer,
                name,
                &row,
                dialect,
                &mut Vec::new(),
                &Default::default()
            )
            .is_some()
        );
        assert!(
            loadable(
                &importer,
                name,
                &row,
                dialect,
                &mut Vec::new(),
                &std::collections::BTreeSet::from([name.into()])
            )
            .is_none()
        );
        #[cfg(unix)]
        {
            let store =
                app.join("node_modules/.pnpm/client-package@1.0.0/node_modules/client-package");
            fs::create_dir_all(&store).unwrap();
            let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../fixtures/reactive-ir/inferred-host-reachability");
            fs::copy(
                fixture.join("package-client.json"),
                store.join("package.json"),
            )
            .unwrap();
            fs::write(store.join("client.js"), "export const value=1;").unwrap();
            fs::write(
                store.join("index.d.ts"),
                "export declare const value:number;",
            )
            .unwrap();
            std::os::unix::fs::symlink(&store, app.join("node_modules/client-package")).unwrap();
            let mut row = attested(&store, "client-package", "1.0.0");
            row.symlink_path = app
                .join("node_modules/client-package/index.d.ts")
                .to_str()
                .unwrap()
                .into();
            let load = || {
                loadable(
                    &importer,
                    "client-package",
                    &row,
                    dialect,
                    &mut Vec::new(),
                    &Default::default(),
                )
            };
            assert!(load().is_some());
            for name in ["package-hidden.json", "package-browser-false.json"] {
                fs::copy(fixture.join(name), store.join("package.json")).unwrap();
                assert!(load().is_none(), "{name}");
            }
            fs::copy(
                fixture.join("package-client.json"),
                store.join("package.json"),
            )
            .unwrap();
            let mut inputs = Vec::new();
            assert!(
                loadable(
                    &importer,
                    "client-package",
                    &row,
                    dialect,
                    &mut inputs,
                    &Default::default()
                )
                .is_some()
            );
            assert!(inputs.contains(&store.join("client.js")));
            assert!(inputs.contains(&store.join("package.json")));
            assert!(inputs.contains(&app.join("node_modules/client-package")));
            // A shallower install shadows the store. Exact declaration root
            // disagreement must refuse instead of name/version matching it.
            fs::create_dir_all(app.join("src/node_modules/client-package")).unwrap();
            fs::copy(
                fixture.join("package-client.json"),
                app.join("src/node_modules/client-package/package.json"),
            )
            .unwrap();
            fs::write(
                app.join("src/node_modules/client-package/client.js"),
                "export const value=2;",
            )
            .unwrap();
            assert!(
                loadable(
                    &app.join("src/main.ts"),
                    "client-package",
                    &row,
                    dialect,
                    &mut Vec::new(),
                    &Default::default()
                )
                .is_none()
            );
            // Bun's store has the same canonical-root premise; retargeting an
            // install changes both declaration identity and cached input paths.
            let bun =
                app.join("node_modules/.bun/client-package@1.0.0/node_modules/client-package");
            fs::create_dir_all(bun.parent().unwrap()).unwrap();
            fs::remove_file(app.join("node_modules/client-package")).unwrap();
            fs::rename(&store, &bun).unwrap();
            std::os::unix::fs::symlink(&bun, app.join("node_modules/client-package")).unwrap();
            assert!(
                loadable(
                    &importer,
                    "client-package",
                    &row,
                    dialect,
                    &mut Vec::new(),
                    &Default::default()
                )
                .is_none()
            );
            let bun_row = attested(&bun, "client-package", "1.0.0");
            assert!(
                loadable(
                    &importer,
                    "client-package",
                    &bun_row,
                    dialect,
                    &mut Vec::new(),
                    &Default::default()
                )
                .is_some()
            );
        }
        fs::remove_dir_all(app).unwrap();
    }

    #[test]
    fn client_export_order_and_closed_subpaths() {
        let parse = |json: &str| serde_json::from_str::<Target>(json).unwrap();
        let exports = parse(
            r#"{".":{"node":"./server.js","browser":{"development":"./dev.js","default":"./client.js"},"default":"./fallback.js"},"./hidden":null,"./features/*":"./features/*.js"}"#,
        );
        assert_eq!(exported(&exports, ".", false).as_deref(), Some("./dev.js"));
        assert_eq!(
            exported(&exports, ".", true).as_deref(),
            Some("./client.js")
        );
        assert_eq!(exported(&exports, "./hidden", false), None);
        assert_eq!(exported(&exports, "./missing", false), None);
        assert_eq!(
            exported(&exports, "./features/a", false).as_deref(),
            Some("./features/a.js")
        );
        assert_eq!(
            exported(
                &parse(r#"{"./*":"./broad/*.js","./features/*":"./specific/*.js"}"#),
                "./features/a",
                false
            )
            .as_deref(),
            Some("./specific/a.js")
        );
        assert_eq!(
            exported(
                &parse(r#"{"default":"./first.js","browser":"./second.js"}"#),
                ".",
                false
            )
            .as_deref(),
            Some("./first.js")
        );
        assert_eq!(
            exported(
                &parse(r#"{"import":null,"default":"./fallback.js"}"#),
                ".",
                false
            ),
            None
        );
        assert_eq!(
            exported(
                &parse(r#"{"browser":{"unknown":"./no.js"},"import":"./yes.js"}"#),
                ".",
                false
            )
            .as_deref(),
            Some("./yes.js")
        );
        assert_eq!(exported(&parse(r#"["./a.js","./b.js"]"#), ".", false), None);
        assert!(
            serde_json::from_str::<Ordered>(r#"{"exports":"./a.js","exports":"./b.js"}"#).is_err()
        );
    }

    #[test]
    fn manifest_entry_regular_file_and_refusal_twins() {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "host-loadable-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        fs::write(root.join("client.js"), "export const value = 1;").unwrap();
        fs::write(root.join("index.cjs"), "module.exports = {value:1};").unwrap();
        let fixture = |manifest: &str, key: &str| {
            entry(
                &root,
                manifest.as_bytes(),
                key,
                false,
                false,
                &mut Vec::new(),
            )
        };
        assert_eq!(
            fixture(
                r#"{"exports":{".":{"browser":"./client.js","default":"./missing.js"}}}"#,
                "."
            ),
            Some(root.join("client.js"))
        );
        assert_eq!(
            fixture(r#"{"main":"index.cjs"}"#, "."),
            Some(root.join("index.cjs"))
        );
        assert_eq!(
            fixture(r#"{"module":"./client.js","main":"./missing.js"}"#, "."),
            Some(root.join("client.js"))
        );
        for manifest in [
            r#"{"exports":{".":"./missing.js"},"main":"client.js"}"#,
            r#"{"exports":{"./other":"./client.js"},"main":"client.js"}"#,
            r#"{"exports":"./client.js","browser":false}"#,
            r#"{"exports":"./client.js","browser":{"./client.js":false}}"#,
            r#"{"exports":"../outside.js"}"#,
            r#"{"exports":"./index.d.ts"}"#,
        ] {
            assert_eq!(fixture(manifest, "."), None, "{manifest}");
        }
        assert_eq!(
            fixture(r#"{"exports":{"./public":"./client.js"}}"#, "./public"),
            Some(root.join("client.js"))
        );
        assert_eq!(
            fixture(r#"{"exports":{"./public":"./client.js"}}"#, "./hidden"),
            None
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("client.js"), root.join("internal.js")).unwrap();
            assert_eq!(
                fixture(r#"{"exports":"./internal.js"}"#, "."),
                Some(root.join("client.js"))
            );
            std::os::unix::fs::symlink(std::env::temp_dir(), root.join("escape")).unwrap();
            assert_eq!(fixture(r#"{"exports":"./escape/other.js"}"#, "."), None);
        }
        fs::remove_dir_all(root).unwrap();
    }
}
