//! The retained import graph and the affected-set walk (Go: `moduleEdges`,
//! `rebuildImportGraph`, `patchImportGraph`, `affectedFromReverseDeps`,
//! `globalScopeChange`, `everySourcePath` in `tsgo/project.go`).
//!
//! `forward` holds each non-declaration file's cleaned resolved dependencies
//! and `reverse` the inverted index. A content edit whose resolved edges are
//! unchanged carries the graph directly; anything that can change membership
//! rebuilds it.

use crate::gocompat::clean;
use std::collections::{HashMap, HashSet, VecDeque};
use tsrs_ast::{SourceFile, is_external_module};
use tsrs_compiler::Program;
use tsrs_core::P;

pub type Reverse = HashMap<String, HashSet<String>>;

#[derive(Default)]
pub struct ImportGraph {
    pub forward: HashMap<String, Vec<String>>,
    pub reverse: Reverse,
}

/// Resolves one file's import specifiers to cleaned dependency paths.
/// Duplicate specifiers yield duplicate entries, which every consumer
/// tolerates.
#[must_use]
pub fn module_edges(program: &Program, file: P<SourceFile>) -> Vec<String> {
    let imports = file.imports();
    let mut edges = Vec::with_capacity(imports.len());
    for specifier in imports {
        let Some(resolved) = program.get_resolved_module_from_module_specifier(file, *specifier)
        else {
            continue;
        };
        edges.push(clean(resolved.resolved_file_name));
    }
    edges
}

impl ImportGraph {
    /// Go: `rebuildImportGraph`. The O(project) path; ordinary edits go
    /// through [`ImportGraph::edges_unchanged`] instead.
    #[must_use]
    pub fn rebuild(program: &Program) -> Self {
        let files = program.source_files();
        let mut graph = Self {
            forward: HashMap::with_capacity(files.len()),
            reverse: HashMap::with_capacity(files.len()),
        };
        for file in files {
            if file.is_declaration_file() {
                continue;
            }
            let importer = clean(file.file_name());
            let edges = module_edges(program, *file);
            for dependency in &edges {
                graph
                    .reverse
                    .entry(dependency.clone())
                    .or_default()
                    .insert(importer.clone());
            }
            if !edges.is_empty() {
                graph.forward.insert(importer, edges);
            }
        }
        graph
    }

    /// Go: `patchImportGraph`. Proves the common content-only edit from the
    /// edited file's own resolved edges: equal old and new edges imply
    /// unchanged program membership (with config and file-system membership
    /// fixed, only a changed import can pull a file in or let one leave).
    /// Edge changes take the conservative rebuild.
    #[must_use]
    pub fn edges_unchanged(&self, program: &Program, path: &str) -> bool {
        let path = clean(path);
        let Some(edited) = program.get_source_file(&path) else {
            return false;
        };
        if edited.is_declaration_file() {
            return false;
        }
        let edges = module_edges(program, edited);
        edges == self.forward.get(&path).map_or(&[][..], Vec::as_slice)
    }
}

/// Go: `affectedFromReverseDeps`: the reverse-dependency indexes walked from
/// the changed paths to a fixed point. Retention rests on the premise that a
/// changed declaring file puts every referencing file into the affected set,
/// which import edges establish only for external modules; callers guard the
/// shared-global-scope case before walking. The result is unordered.
#[must_use]
pub fn affected_from_reverse_deps(changed: &[String], graphs: &[&Reverse]) -> Vec<String> {
    let mut affected: HashSet<String> = HashSet::with_capacity(changed.len());
    let mut queue: VecDeque<String> = VecDeque::with_capacity(changed.len());
    for path in changed {
        let path = clean(path);
        if affected.insert(path.clone()) {
            queue.push_back(path);
        }
    }
    while let Some(dependency) = queue.pop_front() {
        for graph in graphs {
            let Some(importers) = graph.get(&dependency) else {
                continue;
            };
            for importer in importers {
                if affected.insert(importer.clone()) {
                    queue.push_back(importer.clone());
                }
            }
        }
    }
    affected.into_iter().collect()
}

/// Go: `globalScopeChange`: whether any changed file participates in the
/// shared global scope rather than being an external module. Script-kind files
/// (no imports and no exports) and global declaration files are referenced
/// without any import edge, so the reverse walk cannot find their referencing
/// files.
#[must_use]
pub fn global_scope_change(changed: &[String], programs: &[&Program]) -> bool {
    changed.iter().any(|path| {
        let path = clean(path);
        programs.iter().any(|program| {
            program
                .get_source_file(&path)
                .is_some_and(|file| !is_external_module(file))
        })
    })
}

/// Go: `everySourcePath`.
#[must_use]
pub fn every_source_path(programs: &[&Program]) -> Vec<String> {
    let mut paths: HashSet<String> = HashSet::new();
    for program in programs {
        for file in program.source_files() {
            paths.insert(clean(file.file_name()));
        }
    }
    paths.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reverse(edges: &[(&str, &[&str])]) -> Reverse {
        edges
            .iter()
            .map(|(dependency, importers)| {
                (
                    (*dependency).to_owned(),
                    importers.iter().map(|path| (*path).to_owned()).collect(),
                )
            })
            .collect()
    }

    fn sorted(mut paths: Vec<String>) -> Vec<String> {
        paths.sort();
        paths
    }

    #[test]
    fn the_walk_reaches_transitive_importers_once() {
        let graph = reverse(&[
            ("/p/c.ts", &["/p/b.ts"]),
            ("/p/b.ts", &["/p/a.ts", "/p/d.ts"]),
            // A cycle must terminate.
            ("/p/a.ts", &["/p/c.ts"]),
        ]);
        assert_eq!(
            sorted(affected_from_reverse_deps(&["/p/c.ts".into()], &[&graph])),
            ["/p/a.ts", "/p/b.ts", "/p/c.ts", "/p/d.ts"]
        );
        assert_eq!(
            sorted(affected_from_reverse_deps(&["/p//d.ts".into()], &[&graph])),
            ["/p/d.ts"]
        );
    }

    #[test]
    fn the_walk_unions_the_graphs_of_both_generations() {
        let old = reverse(&[("/p/gone.ts", &["/p/importer.ts"])]);
        let new = reverse(&[("/p/other.ts", &["/p/b.ts"])]);
        assert_eq!(
            sorted(affected_from_reverse_deps(
                &["/p/gone.ts".into(), "/p/other.ts".into()],
                &[&old, &new]
            )),
            ["/p/b.ts", "/p/gone.ts", "/p/importer.ts", "/p/other.ts"]
        );
    }
}
