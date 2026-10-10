//! Conservative file-selection inputs for admitted tsconfig-path resolvers.
//! Configs form a union, not the analyzed project's effective options: native
//! Vite discovers by importer and the paths plugin discovers workspace projects.

use crate::host_config::{self, Config};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct ResolverPaths {
    patterns: BTreeSet<String>,
    base_urls: BTreeSet<PathBuf>,
}

impl ResolverPaths {
    pub(super) fn could_rewrite(&self, request: &str, inputs: &mut Vec<PathBuf>) -> bool {
        let request = request.split(['?', '#']).next().unwrap_or(request);
        if self.patterns.iter().any(|pattern| {
            pattern
                .split_once('*')
                .map_or(pattern == request, |(prefix, suffix)| {
                    request.len() >= prefix.len() + suffix.len()
                        && request.starts_with(prefix)
                        && request.ends_with(suffix)
                })
        }) {
            return true;
        }
        // baseUrl is also a bare-request resolver, independently of paths.
        if request.starts_with(['.', '/']) || request.contains(['%', '\\']) {
            return false;
        }
        let mut possible = false;
        for base in &self.base_urls {
            let stem = base.join(request);
            for candidate in std::iter::once(stem.clone()).chain(
                ["mjs", "js", "mts", "ts", "jsx", "tsx", "json", "cjs", "cts"]
                    .into_iter()
                    .flat_map(|extension| {
                        [
                            PathBuf::from(format!("{}.{}", stem.display(), extension)),
                            stem.with_extension(extension),
                        ]
                    }),
            ) {
                inputs.push(candidate.clone());
                match fs::symlink_metadata(candidate) {
                    Ok(_) => possible = true,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => possible = true,
                }
            }
        }
        possible
    }
}

/// Include directory membership and absent candidates, so adding a discoverable
/// config invalidates the answer. Includes/excludes and override precedence only
/// narrow this union; they never establish file identity here.
pub(super) fn discover(
    directory: &Path,
    config: &Config,
    inputs: &mut Vec<PathBuf>,
    sources: &[PathBuf],
) -> Option<ResolverPaths> {
    let mut result = ResolverPaths::default();
    if !config.tsconfig_paths {
        return Some(result);
    }
    let mut roots = BTreeSet::new();
    let mut workspace = None;
    for ancestor in directory.ancestors() {
        for name in ["tsconfig.json", "jsconfig.json"] {
            let path = ancestor.join(name);
            inputs.push(path.clone());
            match fs::symlink_metadata(&path) {
                Ok(_) => {
                    roots.insert(path);
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return None,
            }
        }
        if config.plugins.contains("vite-tsconfig-paths") && workspace.is_none() {
            let mut is_workspace = false;
            for name in [
                "pnpm-workspace.yaml",
                "lerna.json",
                "package.json",
                "deno.json",
                "deno.jsonc",
            ] {
                let path = ancestor.join(name);
                inputs.push(path.clone());
                // Vite's root-file markers use existence, not readable JSON
                // or even file type. Never fall back to a narrower scope.
                if name == "pnpm-workspace.yaml" || name == "lerna.json" {
                    match fs::symlink_metadata(&path) {
                        Ok(_) => is_workspace = true,
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(_) => return None,
                    }
                    continue;
                }
                let source = match fs::read_to_string(&path) {
                    Ok(source) => source,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(_) => return None,
                };
                // Vite uses JSON.parse and JavaScript truthiness here, even
                // for deno.jsonc. A guessed nearer root can omit real configs.
                let value: serde_json::Value = serde_json::from_str(&source).ok()?;
                is_workspace |= value
                    .get(if name == "package.json" {
                        "workspaces"
                    } else {
                        "workspace"
                    })
                    .is_some_and(|value| match value {
                        serde_json::Value::Null => false,
                        serde_json::Value::Bool(value) => *value,
                        serde_json::Value::Number(value) => value.as_f64() != Some(0.0),
                        serde_json::Value::String(value) => !value.is_empty(),
                        _ => true,
                    });
            }
            if is_workspace {
                workspace = Some(ancestor.to_owned());
            }
        }
    }
    // The admitted plugin accepts only default options: eager discovery of
    // tsconfig.json/jsconfig.json under Vite's workspace root. Native discovery
    // also sees nested importer configs. Symlinked scopes fail closed.
    scan(
        workspace.as_deref().unwrap_or(directory),
        inputs,
        &mut roots,
    )?;
    // Explicit project files may be in directories omitted by the ordinary
    // inventory. Native importer-based discovery still sees their ancestors.
    let ancestors = sources
        .iter()
        .filter_map(|source| source.parent())
        .flat_map(Path::ancestors)
        .collect::<BTreeSet<_>>();
    for ancestor in ancestors {
        for name in ["tsconfig.json", "jsconfig.json"] {
            let path = ancestor.join(name);
            inputs.push(path.clone());
            match fs::symlink_metadata(&path) {
                Ok(_) => {
                    roots.insert(path);
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return None,
            }
        }
    }
    let mut seen = BTreeSet::new();
    for path in roots {
        load(&path, inputs, &mut seen, &mut result)?;
    }
    Some(result)
}

fn scan(directory: &Path, inputs: &mut Vec<PathBuf>, roots: &mut BTreeSet<PathBuf>) -> Option<()> {
    inputs.push(directory.to_owned());
    for entry in fs::read_dir(directory).ok()? {
        let entry = entry.ok()?;
        if entry.file_name() == "node_modules" || entry.file_name() == ".git" {
            continue;
        }
        let kind = entry.file_type().ok()?;
        if kind.is_symlink() {
            inputs.push(entry.path());
            return None;
        }
        if kind.is_dir() {
            scan(&entry.path(), inputs, roots)?;
        } else if entry.file_name() == "tsconfig.json" || entry.file_name() == "jsconfig.json" {
            roots.insert(entry.path());
        }
    }
    Some(())
}

fn load(
    path: &Path,
    inputs: &mut Vec<PathBuf>,
    seen: &mut BTreeSet<PathBuf>,
    result: &mut ResolverPaths,
) -> Option<()> {
    inputs.push(path.to_owned());
    // Relative extends may use the logical config path rather than its
    // realpath. Do not silently choose an inheritance origin through a link.
    for ancestor in path.ancestors() {
        if fs::symlink_metadata(ancestor)
            .ok()?
            .file_type()
            .is_symlink()
        {
            inputs.push(ancestor.to_owned());
            return None;
        }
    }
    let canonical = fs::canonicalize(path).ok()?;
    inputs.push(canonical.clone());
    // Shared bases and reference cycles are a finite union, not execution edges.
    if !seen.insert(canonical.clone()) {
        return Some(());
    }
    let value = host_config::jsonc(&fs::read_to_string(&canonical).ok()?)?;
    let parent = canonical.parent()?;
    if let Some(options) = value.get("compilerOptions") {
        let options = options.as_object()?;
        if let Some(paths) = options.get("paths") {
            for pattern in paths.as_object()?.keys() {
                if pattern.matches('*').count() > 1 {
                    return None;
                }
                result.patterns.insert(pattern.clone());
            }
        }
        if let Some(base) = options.get("baseUrl") {
            result.base_urls.insert(parent.join(base.as_str()?));
        }
    }
    if let Some(extends) = value.get("extends") {
        let bases = if let Some(base) = extends.as_str() {
            vec![base]
        } else {
            extends
                .as_array()?
                .iter()
                .map(serde_json::Value::as_str)
                .collect::<Option<Vec<_>>>()?
        };
        for base in bases {
            // No guessed package-export config inheritance. Unsupported config
            // lookup refuses inference, including when startup is successful.
            if !base.starts_with('.') || !base.ends_with(".json") {
                return None;
            }
            load(&parent.join(base), inputs, seen, result)?;
        }
    }
    if let Some(references) = value.get("references") {
        for reference in references.as_array()? {
            let target = parent.join(reference.get("path")?.as_str()?);
            inputs.push(target.clone());
            let target = if target
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                target
            } else {
                target.join("tsconfig.json")
            };
            load(&target, inputs, seen, result)?;
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolver_scope_enrolls_workspace_nested_bases_references_and_absence() {
        let root = std::env::temp_dir().join(format!("host-resolver-scope-{}", std::process::id()));
        let app = root.join("app");
        fs::create_dir_all(app.join("src/nested")).unwrap();
        fs::create_dir_all(root.join("sibling")).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let app = root.join("app");
        fs::write(
            root.join("package.json"),
            r#"{"private":true,"workspaces":["*"]}"#,
        )
        .unwrap();
        fs::write(
            app.join("package.json"),
            r#"{"private":true,"workspaces":0}"#,
        )
        .unwrap();
        fs::write(app.join("tsconfig.app.json"), "{}").unwrap();
        fs::write(
            app.join("tsconfig.json"),
            r#"{"extends":["../base.json"],"references":[{"path":"../sibling/ref.json"}]}"#,
        )
        .unwrap();
        fs::write(
            root.join("base.json"),
            r#"{"compilerOptions":{"baseUrl":"./app","paths":{"@solidjs/*":["./shadow.js"]}}}"#,
        )
        .unwrap();
        fs::write(
            root.join("sibling/ref.json"),
            r#"{"compilerOptions":{"paths":{"referenced":["./shadow.js"]}}}"#,
        )
        .unwrap();
        fs::write(
            app.join("src/nested/jsconfig.json"),
            r#"{"compilerOptions":{"paths":{"nested":["./shadow.js"]}}}"#,
        )
        .unwrap();
        fs::write(
            root.join("sibling/tsconfig.json"),
            r#"{"compilerOptions":{"paths":{"sibling":["./shadow.js"]}}}"#,
        )
        .unwrap();
        for (source, sibling) in [
            (
                "export default {plugins:[],resolve:{tsconfigPaths:true}}",
                false,
            ),
            (
                "import paths from 'vite-tsconfig-paths';export default {plugins:[paths()]}",
                true,
            ),
        ] {
            let config = host_config::parse(&app.join("vite.config.ts"), source).unwrap();
            let mut inputs = Vec::new();
            let paths = discover(&app, &config, &mut inputs, &[]).unwrap();
            for request in ["@solidjs/web", "referenced", "nested"] {
                assert!(paths.could_rewrite(request, &mut Vec::new()));
            }
            assert!(paths.could_rewrite("@solidjs/web?raw", &mut Vec::new()));
            assert_eq!(paths.could_rewrite("sibling", &mut Vec::new()), sibling);
            assert!(!paths.could_rewrite("@solidjsweb", &mut inputs));
            for path in [
                app.join("tsconfig.json"),
                root.join("base.json"),
                root.join("sibling/ref.json"),
                app.join("src/nested/jsconfig.json"),
                app.join("src/nested"),
            ] {
                assert!(inputs.contains(&path), "{}", path.display());
            }
            assert!(inputs.contains(&app.join("@solidjsweb.js")));
        }
        let config = host_config::parse(
            &app.join("vite.config.ts"),
            "export default {plugins:[],resolve:{tsconfigPaths:true}}",
        )
        .unwrap();
        let mut inputs = Vec::new();
        let paths = discover(&app, &config, &mut inputs, &[]).unwrap();
        let before =
            crate::inferred_host::inferred_host_input_digest(&app.join("src/nested")).unwrap();
        fs::write(
            app.join("src/nested/tsconfig.json"),
            r#"{"compilerOptions":{"paths":{"new-path":["./shadow.js"]}}}"#,
        )
        .unwrap();
        assert_ne!(
            before,
            crate::inferred_host::inferred_host_input_digest(&app.join("src/nested")).unwrap()
        );
        assert!(!paths.could_rewrite("new-path", &mut Vec::new()));
        assert!(
            discover(&app, &config, &mut Vec::new(), &[])
                .unwrap()
                .could_rewrite("new-path", &mut Vec::new())
        );
        fs::write(app.join("base-shadow.js"), "export {};").unwrap();
        assert!(paths.could_rewrite("base-shadow", &mut Vec::new()));
        fs::write(app.join("substituted.ts"), "export {};").unwrap();
        assert!(paths.could_rewrite("substituted.js", &mut Vec::new()));
        let explicit = app.join("node_modules/local-source");
        fs::create_dir_all(&explicit).unwrap();
        fs::write(
            explicit.join("tsconfig.json"),
            r#"{"compilerOptions":{"paths":{"explicit-source":["./shadow.js"]}}}"#,
        )
        .unwrap();
        let mut inputs = Vec::new();
        let explicit_paths =
            discover(&app, &config, &mut inputs, &[explicit.join("entry.ts")]).unwrap();
        assert!(explicit_paths.could_rewrite("explicit-source", &mut Vec::new()));
        assert!(inputs.contains(&explicit.join("tsconfig.json")));
        fs::write(
            app.join("vite.config.ts"),
            "export default {plugins:[],resolve:{tsconfigPaths:true}};",
        )
        .unwrap();
        fs::write(explicit.join("entry.ts"), "export {};").unwrap();
        let cache_inputs = crate::inferred_host::inferred_host_input_paths_for_sources(
            &app,
            &app.join("tsconfig.app.json"),
            &[explicit.join("entry.ts")],
        );
        assert!(cache_inputs.contains(&explicit.join("tsconfig.json")));
        fs::write(root.join("package.json"), r#"{"private":true}"#).unwrap();
        fs::create_dir(root.join("lerna.json")).unwrap();
        let plugin = host_config::parse(
            &app.join("vite.config.ts"),
            "import paths from 'vite-tsconfig-paths';export default {plugins:[paths()]}",
        )
        .unwrap();
        assert!(
            discover(&app, &plugin, &mut Vec::new(), &[])
                .unwrap()
                .could_rewrite("sibling", &mut Vec::new())
        );
        fs::write(app.join("deno.jsonc"), r#"{/* comment */"workspace":[]}"#).unwrap();
        assert!(discover(&app, &plugin, &mut Vec::new(), &[]).is_none());
        fs::remove_file(app.join("deno.jsonc")).unwrap();
        #[cfg(unix)]
        {
            let link = root.join("linked-base.json");
            std::os::unix::fs::symlink(root.join("base.json"), &link).unwrap();
            fs::write(
                app.join("tsconfig.json"),
                r#"{"extends":"../linked-base.json"}"#,
            )
            .unwrap();
            assert!(discover(&app, &config, &mut Vec::new(), &[]).is_none());
            fs::remove_file(link).unwrap();
            fs::write(app.join("tsconfig.json"), r#"{"extends":"../base.json"}"#).unwrap();
        }
        fs::write(
            root.join("base.json"),
            r#"{"extends":"unknown-package/config"}"#,
        )
        .unwrap();
        assert!(discover(&app, &config, &mut Vec::new(), &[]).is_none());
        fs::remove_dir_all(root).unwrap();
    }
}
