//! Host constant resolution (ADR 0166): the value a declared host's resolution
//! of a package fixes for one of its exports, read from the resolved runtime
//! module's bytes.
//!
//! `@solidjs/web@2.0.0-rc.9` publishes `const isServer = false` in every file
//! its `exports` map selects for `browser` (`dist/web.js`, `web.dev.js`,
//! `web.observe.js`) and `const isServer = true` in every file it selects for
//! `node` (`dist/server.js`, `server.dev.js`, `server.observe.js`). Under a
//! certification that declares one of those hosts, an `if (isServer)` in a
//! module that imports it from there has one dead arm, and the proposal, the
//! producer's reachability and its `unconditional` fact all read it that way.
//!
//! Nothing here reads a name for its meaning. A value is folded only when:
//!
//! - the certification declares exactly one host, `browser` or `node` (a
//!   host-free case describes no runtime, so both arms stay live);
//! - the importing module names the export in a named value import from a bare
//!   specifier;
//! - the specifier's package is found by Node's lookup from the importer, and
//!   its `exports` map resolves the subpath, under the host, to targets that
//!   exist for *every* setting of the conditions the host does not fix
//!   (`development`, `observe`, `require`, a bundler's own) -- so a consumer
//!   admitted to this host's case loads one of exactly these files;
//! - every one of those targets exports the name as a module-level `const`
//!   bound to the same `true` or `false` literal
//!   ([`solid_facts::ast::exported_boolean_constant`]).
//!
//! A package with no `exports` map, a legacy `browser` field, a pattern or
//! array target, a target that re-exports the name, or targets that disagree
//! (`isDev` differs between `web.js` and `web.dev.js`) folds nothing.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use sha2::{Digest, Sha256};
use solid_dialect::HostTargetCondition;
use solid_facts::ast::{HostConstantFold, exported_boolean_constant, named_value_imports};

/// The host a certification's conditions declare, when they declare exactly
/// one and it is a host a fold is defined for.
#[must_use]
pub fn declared_host(conditions: &[String]) -> Option<HostTargetCondition> {
    let hosts = conditions
        .iter()
        .filter_map(|condition| HostTargetCondition::from_condition(condition))
        .collect::<BTreeSet<_>>();
    match hosts.into_iter().collect::<Vec<_>>().as_slice() {
        [host @ (HostTargetCondition::Browser | HostTargetCondition::Node)] => Some(*host),
        _ => None,
    }
}

/// One proved host constant of one importing module, with the evidence it
/// rests on.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ResolvedHostConstant {
    pub fold: HostConstantFold,
    pub package_name: String,
    pub package_version: String,
    /// Every runtime target the host's resolution may select, with the SHA-256
    /// of its bytes, package-relative.
    pub targets: Vec<(String, String)>,
}

impl ResolvedHostConstant {
    /// The witness line a certification records for this fold.
    #[must_use]
    pub fn witness(&self, host: HostTargetCondition) -> String {
        format!(
            "host-constant:{}:{}@{}#{}={} ({})",
            host.as_str(),
            self.package_name,
            self.package_version,
            self.fold.imported,
            self.fold.value,
            self.targets
                .iter()
                .map(|(path, digest)| format!("{path} sha256:{digest}"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// The host constants `importer` (whose text is `source`) imports, under
/// `host`. Empty when none is proved; never an error, because an unproved
/// constant only leaves both arms live.
///
/// Node's lookup walks up from the importer to the filesystem root. A
/// certification passes the private project's root as `boundary`, so the walk
/// never leaves the tree the checker itself wrote from authenticated snapshots
/// and cannot reach a package some ancestor directory happens to hold.
#[must_use]
pub fn host_constants_of_module(
    importer: &Path,
    source: &str,
    host: HostTargetCondition,
    boundary: Option<&Path>,
) -> Vec<ResolvedHostConstant> {
    let Some(imports) = named_value_imports(importer, source) else {
        return Vec::new();
    };
    let mut resolved = Vec::new();
    for import in imports {
        let Some((package_name, subpath)) = split_bare_specifier(&import.specifier) else {
            continue;
        };
        let Some(package_root) = find_package_root(importer, &package_name, boundary) else {
            continue;
        };
        let Some(manifest) = std::fs::read(package_root.join("package.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Ordered>(&bytes).ok())
        else {
            continue;
        };
        if manifest.get("name").and_then(Ordered::as_str) != Some(package_name.as_str())
            || manifest.get("browser").is_some()
        {
            continue;
        }
        let Some(version) = manifest.get("version").and_then(Ordered::as_str) else {
            continue;
        };
        let Some(targets) = manifest
            .get("exports")
            .and_then(|exports| runtime_targets(exports, &subpath, host))
        else {
            continue;
        };
        let mut value = None;
        let mut evidence = Vec::new();
        let mut agreed = true;
        for target in &targets {
            let Some(relative) = target.strip_prefix("./") else {
                agreed = false;
                break;
            };
            if relative
                .split('/')
                .any(|part| part == ".." || part.is_empty())
            {
                agreed = false;
                break;
            }
            let path = package_root.join(relative);
            let Ok(bytes) = std::fs::read(&path) else {
                agreed = false;
                break;
            };
            let Ok(text) = std::str::from_utf8(&bytes) else {
                agreed = false;
                break;
            };
            let Some(found) = exported_boolean_constant(&path, text, &import.imported) else {
                agreed = false;
                break;
            };
            if value.is_some_and(|value| value != found) {
                agreed = false;
                break;
            }
            value = Some(found);
            evidence.push((relative.to_owned(), format!("{:x}", Sha256::digest(&bytes))));
        }
        let (true, Some(value)) = (agreed, value) else {
            continue;
        };
        evidence.sort();
        resolved.push(ResolvedHostConstant {
            fold: HostConstantFold {
                specifier: import.specifier.clone(),
                imported: import.imported.clone(),
                value,
            },
            package_name,
            package_version: version.to_owned(),
            targets: evidence,
        });
    }
    resolved
}

/// `@scope/name/sub` → (`@scope/name`, `./sub`); `name` → (`name`, `.`).
fn split_bare_specifier(specifier: &str) -> Option<(String, String)> {
    let mut parts = specifier.split('/');
    let first = parts.next()?;
    let name = if first.starts_with('@') {
        format!("{first}/{}", parts.next()?)
    } else {
        first.to_owned()
    };
    if name.is_empty() || name.starts_with('.') {
        return None;
    }
    let rest = parts.collect::<Vec<_>>();
    let subpath = if rest.is_empty() {
        ".".to_owned()
    } else {
        format!("./{}", rest.join("/"))
    };
    Some((name, subpath))
}

/// Node's lookup: the first `node_modules/<name>` directory above `importer`,
/// stopping after `boundary` when one is given (an importer outside it finds
/// nothing).
fn find_package_root(importer: &Path, name: &str, boundary: Option<&Path>) -> Option<PathBuf> {
    if boundary.is_some_and(|boundary| !importer.starts_with(boundary)) {
        return None;
    }
    let mut directory = importer.parent();
    while let Some(current) = directory {
        let candidate = current.join("node_modules").join(name);
        if candidate.join("package.json").is_file() {
            return Some(candidate);
        }
        if boundary.is_some_and(|boundary| current == boundary) {
            return None;
        }
        directory = current.parent();
    }
    None
}

/// Every runtime target `exports` may select for `subpath` under `host`,
/// whatever the conditions `host` does not fix. `None` when some setting of
/// them selects nothing, or the map uses a form this does not read (a pattern,
/// an array fallback, `null`).
fn runtime_targets(
    exports: &Ordered,
    subpath: &str,
    host: HostTargetCondition,
) -> Option<Vec<String>> {
    let entry = match exports {
        Ordered::Object(map) if map.iter().any(|(key, _)| key.starts_with('.')) => {
            if map.iter().any(|(key, _)| !key.starts_with('.')) {
                return None;
            }
            exports.get(subpath)?
        }
        _ if subpath == "." => exports,
        _ => return None,
    };
    let (mut targets, always) = conditional_targets(entry, host, 0)?;
    if !always || targets.is_empty() {
        return None;
    }
    targets.sort();
    targets.dedup();
    Some(targets)
}

/// The targets a conditional entry may select, and whether it selects one for
/// every setting of the unfixed conditions.
fn conditional_targets(
    entry: &Ordered,
    host: HostTargetCondition,
    depth: usize,
) -> Option<(Vec<String>, bool)> {
    if depth > 16 {
        return None;
    }
    match entry {
        Ordered::String(target) => {
            if target.contains('*') {
                return None;
            }
            Some((vec![target.clone()], true))
        }
        Ordered::Object(map) => {
            let mut targets = Vec::new();
            for (key, value) in map {
                if key.starts_with('.') {
                    return None;
                }
                // Declarations are not runtime.
                if key == "types" {
                    continue;
                }
                let other_host = HostTargetCondition::from_condition(key)
                    .is_some_and(|condition| condition != host);
                if other_host {
                    continue;
                }
                // Conditions every consumer of this case sets: its host, and
                // `import` (every certification requests it) -- and `default`.
                let fixed = key == host.as_str() || key == "import" || key == "default";
                let (inner, always) = conditional_targets(value, host, depth + 1)?;
                targets.extend(inner);
                if fixed && always {
                    return Some((targets, true));
                }
            }
            Some((targets, false))
        }
        _ => None,
    }
}

/// A JSON value that keeps object keys in document order, which an `exports`
/// map's meaning depends on (`serde_json::Value` sorts them).
#[derive(Clone, Debug)]
enum Ordered {
    String(String),
    Object(Vec<(String, Ordered)>),
    Other,
}

impl Ordered {
    fn get(&self, key: &str) -> Option<&Ordered> {
        match self {
            Self::Object(entries) => {
                let mut found = entries.iter().filter(|(name, _)| name == key);
                let first = found.next()?;
                // A duplicated key is resolved differently by different parsers.
                found.next().is_none().then_some(&first.1)
            }
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct OrderedVisitor;
        impl<'de> Visitor<'de> for OrderedVisitor {
            type Value = Ordered;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a JSON value")
            }
            fn visit_str<E>(self, value: &str) -> Result<Ordered, E> {
                Ok(Ordered::String(value.to_owned()))
            }
            fn visit_string<E>(self, value: String) -> Result<Ordered, E> {
                Ok(Ordered::String(value))
            }
            fn visit_bool<E>(self, _: bool) -> Result<Ordered, E> {
                Ok(Ordered::Other)
            }
            fn visit_i64<E>(self, _: i64) -> Result<Ordered, E> {
                Ok(Ordered::Other)
            }
            fn visit_u64<E>(self, _: u64) -> Result<Ordered, E> {
                Ok(Ordered::Other)
            }
            fn visit_f64<E>(self, _: f64) -> Result<Ordered, E> {
                Ok(Ordered::Other)
            }
            fn visit_unit<E>(self) -> Result<Ordered, E> {
                Ok(Ordered::Other)
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Ordered, A::Error> {
                while seq.next_element::<Ordered>()?.is_some() {}
                Ok(Ordered::Other)
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Ordered, A::Error> {
                let mut entries = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, Ordered>()? {
                    entries.push((key, value));
                }
                Ok(Ordered::Object(entries))
            }
        }
        deserializer.deserialize_any(OrderedVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `@solidjs/web@2.0.0-rc.9`'s `exports["."]`, byte for byte.
    const WEB_EXPORTS: &str = r#"{
  ".": {
    "worker": {
      "development": { "types": "./types/index.d.ts", "default": "./dist/server.dev.js" },
      "observe": { "types": "./types/index.d.ts", "default": "./dist/server.observe.js" },
      "types": "./types/index.d.ts",
      "default": "./dist/server.js"
    },
    "browser": {
      "development": { "types": "./types/index.d.ts", "default": "./dist/web.dev.js" },
      "observe": { "types": "./types/index.d.ts", "default": "./dist/web.observe.js" },
      "types": "./types/index.d.ts",
      "default": "./dist/web.js"
    },
    "deno": {
      "development": { "types": "./types/index.d.ts", "default": "./dist/server.dev.js" },
      "observe": { "types": "./types/index.d.ts", "default": "./dist/server.observe.js" },
      "types": "./types/index.d.ts",
      "default": "./dist/server.js"
    },
    "node": {
      "development": { "types": "./types/index.d.ts", "default": "./dist/server.dev.js" },
      "observe": { "types": "./types/index.d.ts", "default": "./dist/server.observe.js" },
      "types": "./types/index.d.ts",
      "default": "./dist/server.js"
    },
    "development": { "types": "./types/index.d.ts", "default": "./dist/web.dev.js" },
    "observe": { "types": "./types/index.d.ts", "default": "./dist/web.observe.js" },
    "types": "./types/index.d.ts",
    "default": "./dist/web.js"
  }
}"#;

    #[test]
    fn a_host_selects_every_target_its_unfixed_conditions_may() {
        let exports: Ordered = serde_json::from_str(WEB_EXPORTS).unwrap();
        assert_eq!(
            runtime_targets(&exports, ".", HostTargetCondition::Browser).unwrap(),
            [
                "./dist/web.dev.js",
                "./dist/web.js",
                "./dist/web.observe.js"
            ]
        );
        assert_eq!(
            runtime_targets(&exports, ".", HostTargetCondition::Node).unwrap(),
            [
                "./dist/server.dev.js",
                "./dist/server.js",
                "./dist/server.observe.js"
            ]
        );
        assert!(runtime_targets(&exports, "./missing", HostTargetCondition::Node).is_none());
        // A map that may select nothing folds nothing.
        let partial: Ordered = serde_json::from_str(r#"{".": {"development": "./a.js"}}"#).unwrap();
        assert!(runtime_targets(&partial, ".", HostTargetCondition::Browser).is_none());
        // Order decides: `default` before `browser` selects the default target.
        let ordered: Ordered =
            serde_json::from_str(r#"{".": {"default": "./a.js", "browser": "./b.js"}}"#).unwrap();
        assert_eq!(
            runtime_targets(&ordered, ".", HostTargetCondition::Browser).unwrap(),
            ["./a.js"]
        );
        assert!(runtime_targets(&partial, ".", HostTargetCondition::Browser).is_none());
    }

    #[test]
    fn only_one_browser_or_node_host_is_declared() {
        let conditions = |list: &[&str]| {
            list.iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            declared_host(&conditions(&["browser", "import"])),
            Some(HostTargetCondition::Browser)
        );
        assert_eq!(
            declared_host(&conditions(&["import", "node"])),
            Some(HostTargetCondition::Node)
        );
        assert_eq!(declared_host(&conditions(&["import"])), None);
        assert_eq!(declared_host(&conditions(&["deno", "import"])), None);
        assert_eq!(declared_host(&conditions(&["browser", "node"])), None);
    }

    #[test]
    fn a_resolved_module_folds_only_an_agreeing_literal() {
        let root = std::env::temp_dir().join(format!(
            "solid-checker-host-constants-{}",
            std::process::id()
        ));
        let web = root.join("node_modules/@solidjs/web");
        std::fs::create_dir_all(web.join("dist")).unwrap();
        std::fs::write(
            web.join("package.json"),
            format!(r#"{{"name":"@solidjs/web","version":"2.0.0-rc.9","exports":{WEB_EXPORTS}}}"#),
        )
        .unwrap();
        for (file, server) in [
            ("web.js", false),
            ("web.dev.js", false),
            ("web.observe.js", false),
            ("server.js", true),
            ("server.dev.js", true),
            ("server.observe.js", true),
        ] {
            std::fs::write(
                web.join("dist").join(file),
                format!("const isServer = {server};\nconst isDev = {};\nexport {{ isDev, isServer }};\n", file.contains("dev")),
            )
            .unwrap();
        }
        let importer = root.join("node_modules/lib/dist/index.js");
        std::fs::create_dir_all(importer.parent().unwrap()).unwrap();
        let source = "import { isDev, isServer } from \"@solidjs/web\";\n";
        let browser =
            host_constants_of_module(&importer, source, HostTargetCondition::Browser, None);
        assert_eq!(
            browser.len(),
            1,
            "isDev disagrees between web.js and web.dev.js"
        );
        assert_eq!(browser[0].fold.imported, "isServer");
        assert!(!browser[0].fold.value);
        assert_eq!(browser[0].targets.len(), 3);
        let node = host_constants_of_module(&importer, source, HostTargetCondition::Node, None);
        assert!(node[0].fold.value);
        // A boundary below the installation hides it: the lookup stops at the
        // boundary instead of finding the package an ancestor holds.
        let inner = root.join("node_modules/lib");
        assert!(
            host_constants_of_module(&importer, source, HostTargetCondition::Node, Some(&inner))
                .is_empty()
        );
        assert_eq!(
            host_constants_of_module(&importer, source, HostTargetCondition::Node, Some(&root))
                .len(),
            1
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}
