//! Static client linking only. This premise grants no package call semantics,
//! owner/tracking facts, completion, artifact authentication or certification.

use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use solid_facts::resolution::{AttestedImport, ImportResolution};
use std::fs;
use std::path::{Component, Path, PathBuf};

type PackageCandidates = (PathBuf, Vec<u8>, Vec<PathBuf>);

/// Exact importer-directory observations, confined to one discovery pass.
/// Replaying a lookup also replays every positive and negative input path.
#[derive(Default)]
pub(super) struct PackageLoads {
    candidates:
        std::collections::BTreeMap<(PathBuf, String), (Option<PackageCandidates>, Vec<PathBuf>)>,
}

impl PackageLoads {
    pub(super) fn candidates(
        &mut self,
        importer: &Path,
        text: &str,
        inputs: &mut Vec<PathBuf>,
    ) -> Option<PackageCandidates> {
        let key = (importer.parent()?.to_owned(), text.to_owned());
        let (selected, paths) = self.candidates.entry(key).or_insert_with(|| {
            let mut paths = Vec::new();
            let selected = candidates(importer, text, &mut paths);
            (selected, paths)
        });
        inputs.extend(paths.iter().cloned());
        selected.clone()
    }

    pub(super) fn loadable(
        &mut self,
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
        let selected = self.candidates(importer, text, inputs)?;
        loadable_selected(text, row, dialect, inputs, excluded, selected)
    }
}

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
fn conditional(target: &Target, production: bool, solid: bool, css: bool) -> Option<Option<&str>> {
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
                if ((if css {
                    &["style", "import", "default"][..]
                } else {
                    &["module", "browser", "import", "default"][..]
                })
                .contains(&key.as_str())
                    || (solid && key == "solid")
                    || key
                        == if production {
                            "production"
                        } else {
                            "development"
                        })
                    && let Some(selected) = conditional(value, production, solid, css)
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

fn exported(
    exports: &Target,
    key: &str,
    production: bool,
    solid: bool,
    css: bool,
) -> Option<String> {
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
            return conditional(target, production, solid, css)?.map(str::to_owned);
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
        return conditional(target, production, solid, css)?
            .map(|target| target.replace('*', capture));
    }
    (key == ".")
        .then(|| conditional(exports, production, solid, css))
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

struct PackageManifest {
    value: serde_json::Value,
    ordered: Ordered,
}

impl PackageManifest {
    fn parse(bytes: &[u8]) -> Option<Self> {
        Some(Self {
            value: serde_json::from_slice(bytes).ok()?,
            // Preserve author order and the refusal of duplicate metadata.
            ordered: serde_json::from_slice(bytes).ok()?,
        })
    }
}

#[cfg(test)]
fn entry(
    root: &Path,
    bytes: &[u8],
    key: &str,
    production: bool,
    solid: bool,
    inputs: &mut Vec<PathBuf>,
) -> Option<PathBuf> {
    entry_for_manifest(
        root,
        &PackageManifest::parse(bytes)?,
        key,
        production,
        solid,
        inputs,
    )
}

fn entry_for_manifest(
    root: &Path,
    parsed: &PackageManifest,
    key: &str,
    production: bool,
    solid: bool,
    inputs: &mut Vec<PathBuf>,
) -> Option<PathBuf> {
    let manifest = &parsed.value;
    let Ordered(fields) = &parsed.ordered;
    if manifest.get("browser") == Some(&serde_json::Value::Bool(false)) {
        return None;
    }
    if let Some((_, exports)) = fields.iter().find(|(name, _)| name == "exports") {
        let target = exported(exports, key, production, solid, false)?;
        if !target.starts_with("./") {
            return None;
        }
        // Root entries may be browser-remapped by Vite after exports selection.
        let target = if key == "." {
            browser_map(manifest, &target)?
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
    let target = browser_map(manifest, &target)?;
    legacy_file(root, &target, inputs)
}

/// ADR 0270 successful-build premise: acquisition grants static linking only,
/// never generated export, callback, component or initialization behavior.
pub(super) struct Resource {
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
                "json",
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

/// Classify the canonical selected file, never the authored package spelling.
/// A successful build discharges loader success, not target selection. Native
/// addons/declarations and unknown loaders remain outside the Vite premise.
pub(super) fn static_entry(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ["js", "mjs", "cjs", "jsx", "ts", "mts", "cts", "tsx"].contains(&ext))
        || path.to_str().is_some_and(resource_request)
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
    // A package must not redirect a .css/.svg request to executable JavaScript.
    if query.is_empty()
        && entries
            .iter()
            .any(|path| path.extension().and_then(|ext| ext.to_str()) != Some(requested))
    {
        return None;
    }
    // Raw acquisition executes no stylesheet loader. Successful compilation
    // cannot discharge the independent refusal of executable plugin/config
    // side inputs. Resource contents never add analyzed JavaScript edges.
    if query != "raw" && !stylesheet_closure(&entries, config, inputs) {
        return None;
    }
    Some(Resource { entries })
}

fn stylesheet(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            [
                "css", "pcss", "postcss", "less", "sass", "scss", "styl", "stylus", "sss",
            ]
            .contains(&ext)
        })
}

// Keep the existing executable CSS plugin/config veto, including escaped and
// comment-separated spellings. This is not a transform-success predicate:
// @import/@reference/@apply, composition and malformed CSS are loadable under
// the successful-build premise, and never supply behavior authority.
fn css_without_executable_inputs(source: &str) -> bool {
    let normalized = normalized_css(source)
        .to_ascii_lowercase()
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect::<String>();
    !["@config", "@plugin"]
        .iter()
        .any(|directive| normalized.contains(directive))
}

fn normalized_css(source: &str) -> String {
    let mut normalized = String::new();
    let mut chars = source.chars().peekable();
    let mut quote = None;
    while let Some(c) = chars.next() {
        // CSS loaders pass quoted resolver strings through differing parsers.
        // Preserve their raw bytes; css_candidates refuses escape interpretation.
        // Directive normalization outside strings must not choose a path.
        if quote.is_some() && c == '\\' {
            normalized.push(c);
            if let Some(next) = chars.next() {
                normalized.push(next);
            }
            continue;
        }
        if quote.is_none() && c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(c) = chars.next() {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
            if !normalized.ends_with(' ') {
                normalized.push(' ');
            }
        } else if c == '\\' {
            // Escaped newlines continue a CSS token, including directives.
            if chars
                .peek()
                .is_some_and(|c| ['\n', '\r', '\u{c}'].contains(c))
            {
                if chars.next() == Some('\r') && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                continue;
            }
            let mut hex = String::new();
            while hex.len() < 6 && chars.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
                hex.push(chars.next().unwrap());
            }
            if hex.is_empty() {
                if let Some(c) = chars.next() {
                    if ['\'', '"'].contains(&c) {
                        normalized.push('\\');
                    }
                    normalized.push(c);
                }
            } else {
                let decoded = u32::from_str_radix(&hex, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .unwrap_or('\u{fffd}');
                if ['\'', '"'].contains(&decoded) {
                    normalized.push('\\');
                }
                normalized.push(decoded);
                if chars.peek().is_some_and(|c| c.is_whitespace()) {
                    chars.next();
                }
            }
        } else {
            if quote.is_none() && c.is_ascii_whitespace() {
                if !normalized.ends_with(' ') {
                    normalized.push(' ');
                }
                continue;
            }
            if quote == Some(c) {
                quote = None;
            } else if quote.is_none() && ['\'', '"'].contains(&c) {
                quote = Some(c);
            }
            normalized.push(c);
        }
    }
    normalized
}

// Independent executable-side-input closure, not a compilation-success proof.
// Imported/reference/composed CSS may introduce Tailwind @plugin/@config. The
// bounded resolver uses CSS style conditions rather than JS entry conditions.
pub(super) fn stylesheet_closure(
    entries: &[PathBuf],
    config: &crate::host_config::Config,
    inputs: &mut Vec<PathBuf>,
) -> bool {
    let mut pending = entries
        .iter()
        .filter(|path| stylesheet(path))
        .cloned()
        .collect::<Vec<_>>();
    let mut seen = std::collections::BTreeSet::new();
    let mut bytes = 0usize;
    while let Some(path) = pending.pop() {
        inputs.push(path.clone());
        inputs.extend(path.ancestors().map(Path::to_owned));
        let Ok(path) = fs::canonicalize(&path) else {
            return false;
        };
        inputs.push(path.clone());
        if !seen.insert(path.clone()) {
            continue;
        }
        if seen.len() > 256 {
            return false;
        }
        let Ok(metadata) = fs::metadata(&path) else {
            return false;
        };
        if !metadata.is_file() || metadata.len() > 2_097_152 {
            return false;
        }
        let Ok(source) = fs::read_to_string(&path) else {
            return false;
        };
        bytes = bytes.saturating_add(source.len());
        if bytes > 8_388_608 || !css_without_executable_inputs(&source) {
            return false;
        }
        let normalized = normalized_css(&source);
        let lower = normalized.to_ascii_lowercase();
        // Unsupported preprocessor module/evaluation forms cannot authenticate
        // their executable side-input closure. Ordinary declarations still link.
        if lower.contains('@')
            && ["@use", "@forward", "@require"]
                .iter()
                .any(|token| lower.contains(token))
            || path.extension().is_some_and(|ext| ext == "less") && source.contains('`')
        {
            return false;
        }
        let mut requests = Vec::new();
        for directive in ["@import", "@reference"] {
            for (offset, _) in lower.match_indices(directive) {
                let tail = normalized[offset + directive.len()..].trim_start();
                let tail = tail
                    .strip_prefix("url(")
                    .map(str::trim_start)
                    .unwrap_or(tail);
                let Some(quote) = tail.chars().next().filter(|c| ['\'', '"'].contains(c)) else {
                    return false;
                };
                let Some(end) = tail[1..].find(quote).map(|end| end + 1) else {
                    return false;
                };
                requests.push((&tail[1..end], true));
                // Reject escaped quote/path truncation or opaque qualifiers;
                // reviewed layer/supports/media suffixes cannot load Node code.
                let suffix = tail[end + 1..].trim_start();
                if !suffix.starts_with([';', ')'])
                    && ![
                        "layer",
                        "supports",
                        "screen",
                        "print",
                        "all",
                        "not ",
                        "only ",
                        "(",
                        "source",
                        "theme",
                        "prefix",
                        "important",
                        "reference",
                    ]
                    .iter()
                    .any(|prefix| suffix.starts_with(prefix))
                {
                    return false;
                }
            }
        }
        for statement in normalized.split(';') {
            let lower = statement.to_ascii_lowercase();
            if ["composes", "compose-with", "@value"]
                .iter()
                .any(|token| lower.contains(token))
            {
                let Some(offset) = lower.find(" from ") else {
                    if lower.contains("from") || statement.contains(['\'', '"']) {
                        return false;
                    }
                    continue;
                };
                let tail = statement[offset + 6..].trim();
                let Some(quote) = tail.chars().next().filter(|c| ['\'', '"'].contains(c)) else {
                    return false;
                };
                let Some(end) = tail[1..].find(quote).map(|end| end + 1) else {
                    return false;
                };
                if !tail[end + 1..].trim().trim_matches('}').trim().is_empty() {
                    return false;
                }
                requests.push((&tail[1..end], false));
            }
        }
        if lower.contains(":import") {
            return false;
        }
        for (text, allow_absent_literal) in requests {
            // Remote CSS is browser data; it does not run build-time Node hooks.
            if ["http://", "https://"]
                .iter()
                .any(|prefix| text.starts_with(prefix))
            {
                continue;
            }
            let Some(targets) = css_candidates(&path, text, config, inputs, allow_absent_literal)
            else {
                return false;
            };
            pending.extend(targets);
        }
    }
    true
}

fn css_candidates(
    importer: &Path,
    text: &str,
    config: &crate::host_config::Config,
    inputs: &mut Vec<PathBuf>,
    allow_absent_literal: bool,
) -> Option<Vec<PathBuf>> {
    if text.is_empty()
        || text.contains(['?', '#', '%', '\\', ':'])
        || text.starts_with('/')
        || !config.default_extensions
        || importer
            .extension()
            .and_then(|ext| ext.to_str())
            .is_none_or(|ext| !["css", "pcss", "postcss"].contains(&ext))
        || config.package_dedupe
        || config.aliases.keys().any(|find| {
            text == find
                || text
                    .strip_prefix(find.as_str())
                    .is_some_and(|tail| tail.starts_with('/'))
        })
    {
        return None;
    }
    // Tailwind's audited CSS resolver is preferRelative, extensions [.css],
    // style main-field/conditions, tryIndex:false. Both modes must agree on
    // inspectable files; no JS resolver/Type Facts wildcard chooses CSS.
    let local = importer.parent()?.join(text);
    for path in [
        local.clone(),
        PathBuf::from(format!("{}.css", local.display())),
    ] {
        inputs.push(path.clone());
        inputs.extend(path.ancestors().map(Path::to_owned));
        if path.is_file() {
            let physical = fs::canonicalize(path).ok()?;
            inputs.push(physical.clone());
            return stylesheet(&physical).then_some(vec![physical]);
        }
        if path.exists() {
            return None;
        }
    }
    if text.starts_with('.') || Path::new(text).is_absolute() {
        // An absent exact literal is a failed transform, excluded by the named
        // premise. Its absence stays an input; no substitute file is guessed.
        return allow_absent_literal.then(Vec::new);
    }
    let (name, key) = request(text)?;
    let root = crate::host_plugins::installed(importer.parent()?, name, inputs)?;
    let manifest = root.join("package.json");
    inputs.push(manifest.clone());
    let bytes = fs::read(&manifest).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    // CSS browser-map substitutions are a separate resolver branch. A style
    // field/exports key cannot stand in for that selected target.
    if value.get("browser").is_some() {
        return None;
    }
    let Ordered(fields): Ordered = serde_json::from_slice(&bytes).ok()?;
    let mut targets = Vec::new();
    for production in [false, true] {
        let target = if let Some((_, exports)) = fields.iter().find(|(name, _)| name == "exports") {
            exported(exports, &key, production, false, true)?
        } else if key != "." {
            key.clone()
        } else {
            let (_, Target::String(style)) = fields.iter().find(|(name, _)| name == "style")?
            else {
                return None;
            };
            style.clone()
        };
        let file = regular(&root, &target, inputs)?;
        if !stylesheet(&file) {
            return None;
        }
        targets.push(file);
    }
    Some(targets)
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
    let manifest = PackageManifest::parse(&bytes)?;
    let mut entries = Vec::new();
    // No build mode is inferred. Both deployment/default serve choices must
    // resolve, even if their selected files differ. Resolver opt-in can then
    // join the actual choice without overriding a failed default proof.
    for production in [false, true] {
        for solid in [false, true] {
            entries.push(entry_for_manifest(
                &root, &manifest, &key, production, solid, inputs,
            )?);
        }
    }
    Some((root, bytes, entries))
}

#[cfg(test)]
fn loadable(
    importer: &Path,
    text: &str,
    row: &AttestedImport,
    dialect: &dyn solid_dialect::Dialect,
    inputs: &mut Vec<PathBuf>,
    excluded: &std::collections::BTreeSet<String>,
) -> Option<PackageLoadability> {
    PackageLoads::default().loadable(importer, text, row, dialect, inputs, excluded)
}

fn loadable_selected(
    text: &str,
    row: &AttestedImport,
    dialect: &dyn solid_dialect::Dialect,
    inputs: &mut Vec<PathBuf>,
    excluded: &std::collections::BTreeSet<String>,
    (root, bytes, entries): PackageCandidates,
) -> Option<PackageLoadability> {
    if row.resolution != ImportResolution::NodeModules || !row.included_path.is_empty() {
        return None;
    }
    let (name, _) = request(text)?;
    // Includes root exports, extensionless subpaths, legacy fields, browser
    // redirects and every condition projection, after canonical selection.
    if !entries.iter().all(|path| static_entry(path)) {
        return None;
    }
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
mod tests;
