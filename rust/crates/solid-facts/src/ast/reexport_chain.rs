//! The entry names an import binding in a *sibling* module is published under,
//! through an exact re-export chain (ADR 0133, extended by ADR 0169).
//!
//! [`crate::ast::reexport_only_import_names`] answers for a binding in the
//! entry file itself. A bundler also emits the barrel one module down:
//!
//! ```js
//! // dist/index.js                 // dist/transform.js
//! import { number } from "./transform.js";
//! export { number };               import { number } from "dep";
//!                                  export { number };
//! ```
//!
//! An obligation filed at `transform.js`'s `number` binding belongs to the
//! entry names that chain publishes, and to no others -- when the chain is
//! exact. Exact means every entry name is decided, by the binder's own
//! resolution of each export specifier and by [`ModuleGraph::landing`] for each
//! relative specifier, to reach the binding or not to reach it. One name left
//! undecided refuses the whole answer, because that name might be one the
//! binding reaches.

use crate::ast::{AstFacts, ExportFact, ExportKind, ImportKind, reexport_only_import_names};
use crate::core::Span;
use std::collections::BTreeSet;
use std::path::Path;

/// Where a module specifier lands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModuleLanding {
    /// A specifier that names another package. It never lands on a module of
    /// the package under analysis.
    Bare,
    /// The analyzed module it lands on, by the path the graph knows it under.
    File(String),
    /// A relative specifier no single analyzed file answers, or one the graph
    /// cannot decide. Never treated as "lands elsewhere".
    Unresolved,
}

/// The modules a package analysis holds, and how its specifiers resolve.
pub trait ModuleGraph {
    /// The facts and bytes of an analyzed module.
    fn module(&self, path: &str) -> Option<(&AstFacts, &str)>;
    /// Where `specifier`, written in `importer`, lands.
    fn landing(&self, importer: &str, specifier: &str) -> ModuleLanding;
}

/// The deepest chain of re-exporting modules followed.
const MAX_DEPTH: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Reach {
    /// The name is the binding.
    Binding,
    /// The name is exported, and is not the binding.
    Elsewhere,
    /// The module does not export the name.
    Absent,
    /// Undecided: an unresolved edge, a cycle, an ambiguous star, or a form
    /// this walk does not model.
    Undecided,
}

/// The names `entry` publishes that are the import binding at `binding` in
/// `module`, or `None` unless every one of these holds:
///
/// - `module` is not `entry`, and [`reexport_only_import_names`] proves that
///   `binding` is used there only by module-level `export { … }` lists;
/// - every name `entry` exports (explicitly or through an `export *` whose
///   landing is exact) is decided: it reaches `binding` or it does not;
/// - at least one name reaches it.
///
/// A chain is followed through named re-exports (`export { a as b } from`,
/// or an import the module only re-exports) and through `export *`. An
/// `export *` is followed only when no explicit export names the same thing
/// and exactly one star target provides it: two providers, or a star of another
/// package beside them, leave the name undecided. A cycle, a relative
/// specifier without an exact landing, a namespace import or `export * as` of
/// an analyzed module (it exposes the binding under every name its module
/// has), and a type-only export of the same name leave it undecided too.
#[must_use]
pub fn entry_names_publishing_import(
    graph: &dyn ModuleGraph,
    entry: &str,
    module: &str,
    binding: Span,
) -> Option<Vec<String>> {
    if entry == module {
        return None;
    }
    let (_, source) = graph.module(module)?;
    reexport_only_import_names(Path::new(module), source, binding)?;
    let walk = Walk {
        graph,
        module,
        binding,
    };
    let mut names = Vec::new();
    for name in walk.exported_names(entry, &mut Vec::new())? {
        match walk.reach(entry, &name, &mut Vec::new()) {
            Reach::Binding => names.push(name),
            Reach::Elsewhere => {}
            Reach::Absent | Reach::Undecided => return None,
        }
    }
    (!names.is_empty()).then_some(names)
}

struct Walk<'a> {
    graph: &'a dyn ModuleGraph,
    module: &'a str,
    binding: Span,
}

fn is_star(export: &ExportFact) -> bool {
    export.kind == ExportKind::All && export.namespace.is_none()
}

impl Walk<'_> {
    /// Every name `file` exports at run time: its explicit ones and those of
    /// each star target. `None` when a star cannot be enumerated.
    fn exported_names(&self, file: &str, path: &mut Vec<String>) -> Option<BTreeSet<String>> {
        if path.len() >= MAX_DEPTH || path.iter().any(|held| held == file) {
            return None;
        }
        let (ast, _) = self.graph.module(file)?;
        path.push(file.to_owned());
        let mut names = BTreeSet::new();
        let mut result = Some(());
        for export in &ast.exports {
            if export.type_only {
                continue;
            }
            if let Some(namespace) = &export.namespace {
                names.insert(namespace.to_string());
            }
            for specifier in export.specifiers.iter().chain(&export.declarations) {
                if !specifier.type_only {
                    names.insert(specifier.exported.to_string());
                }
            }
            if is_star(export) {
                let Some(specifier) = export.module.as_deref() else {
                    result = None;
                    break;
                };
                match self.graph.landing(file, specifier) {
                    // A dependency's names never reach this package's module.
                    // `reach` keeps any name it might also provide undecided.
                    ModuleLanding::Bare => {}
                    ModuleLanding::Unresolved => {
                        result = None;
                        break;
                    }
                    ModuleLanding::File(target) => match self.exported_names(&target, path) {
                        Some(found) => {
                            names.extend(found.into_iter().filter(|name| name != "default"));
                        }
                        None => {
                            result = None;
                            break;
                        }
                    },
                }
            }
        }
        path.pop();
        result.map(|()| names)
    }

    fn reach(&self, file: &str, name: &str, path: &mut Vec<String>) -> Reach {
        if path.len() >= MAX_DEPTH || path.iter().any(|held| held == file) {
            return Reach::Undecided;
        }
        let Some((ast, source)) = self.graph.module(file) else {
            return Reach::Undecided;
        };
        path.push(file.to_owned());
        let answer = self.reach_in(file, ast, source, name, path);
        path.pop();
        answer
    }

    fn reach_in(
        &self,
        file: &str,
        ast: &AstFacts,
        source: &str,
        name: &str,
        path: &mut Vec<String>,
    ) -> Reach {
        // Explicit exports shadow every star.
        let mut explicit = None;
        for export in &ast.exports {
            if let Some(namespace) = &export.namespace
                && namespace.as_str() == name
            {
                if export.type_only {
                    return Reach::Undecided;
                }
                // The namespace of an analyzed module exposes the binding under
                // its own name; of another package's module, nothing here.
                let landing = export
                    .module
                    .as_deref()
                    .map(|specifier| self.graph.landing(file, specifier));
                return match landing {
                    Some(ModuleLanding::Bare) => Reach::Elsewhere,
                    _ => Reach::Undecided,
                };
            }
            for specifier in export.specifiers.iter().chain(&export.declarations) {
                if specifier.exported.as_str() != name {
                    continue;
                }
                if export.type_only || specifier.type_only || explicit.is_some() {
                    return Reach::Undecided;
                }
                explicit = Some((export, specifier));
            }
        }
        if let Some((export, specifier)) = explicit {
            if export
                .declarations
                .iter()
                .any(|held| std::ptr::eq(held, specifier))
            {
                // A declaration of this module, not an import.
                return Reach::Elsewhere;
            }
            return match export.module.as_deref() {
                Some(from) => {
                    let Some(imported) = source
                        .get(specifier.local.span.start as usize..specifier.local.span.end as usize)
                        .filter(|text| is_identifier_name(text))
                    else {
                        return Reach::Undecided;
                    };
                    self.follow(file, from, imported, path)
                }
                None => self.reach_local(file, ast, specifier.local.span, path),
            };
        }
        if name == "default" {
            return Reach::Absent;
        }
        // No explicit export: the name can only come from a star.
        let mut providers = Vec::new();
        for export in ast.exports.iter().filter(|export| is_star(export)) {
            if export.type_only {
                continue;
            }
            let Some(specifier) = export.module.as_deref() else {
                return Reach::Undecided;
            };
            match self.graph.landing(file, specifier) {
                // A star of another package might provide this name too, and a
                // name two stars provide is not exported at all.
                ModuleLanding::Bare | ModuleLanding::Unresolved => return Reach::Undecided,
                ModuleLanding::File(target) => match self.reach(&target, name, path) {
                    Reach::Absent => {}
                    Reach::Undecided => return Reach::Undecided,
                    provided => providers.push(provided),
                },
            }
        }
        match providers.as_slice() {
            [] => Reach::Absent,
            [only] => *only,
            _ => Reach::Undecided,
        }
    }

    /// `export { local as … }` with no `from`: the binder says what `local` is.
    fn reach_local(
        &self,
        file: &str,
        ast: &AstFacts,
        local: Span,
        path: &mut Vec<String>,
    ) -> Reach {
        let Some(declaration) = ast.reference_declaration(local) else {
            return Reach::Undecided;
        };
        if file == self.module && declaration == self.binding {
            return Reach::Binding;
        }
        let Some((import, binding)) = ast.imports.iter().find_map(|import| {
            import
                .bindings
                .iter()
                .find(|binding| binding.local.span == declaration)
                .map(|binding| (import, binding))
        }) else {
            // Not an import of this module: a declaration of its own.
            return Reach::Elsewhere;
        };
        if import.type_only || binding.type_only {
            return Reach::Undecided;
        }
        match binding.kind {
            ImportKind::SideEffect => Reach::Undecided,
            ImportKind::Namespace => match self.graph.landing(file, &import.module) {
                ModuleLanding::Bare => Reach::Elsewhere,
                _ => Reach::Undecided,
            },
            ImportKind::Default => self.follow(file, &import.module, "default", path),
            ImportKind::Named => match binding.imported.as_deref() {
                Some(imported) => self.follow(file, &import.module, imported, path),
                None => Reach::Undecided,
            },
        }
    }

    fn follow(&self, file: &str, specifier: &str, imported: &str, path: &mut Vec<String>) -> Reach {
        match self.graph.landing(file, specifier) {
            ModuleLanding::Bare => Reach::Elsewhere,
            ModuleLanding::Unresolved => Reach::Undecided,
            ModuleLanding::File(target) => match self.reach(&target, imported, path) {
                // A module that does not export what is imported from it is not
                // a chain this walk can call decided (a CommonJS module states
                // its exports nowhere `exports` looks).
                Reach::Absent => Reach::Undecided,
                answer => answer,
            },
        }
    }
}

fn is_identifier_name(text: &str) -> bool {
    let mut characters = text.chars();
    characters
        .next()
        .is_some_and(|first| first == '_' || first == '$' || first.is_alphabetic())
        && characters.all(|next| next == '_' || next == '$' || next.is_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::extract;
    use std::collections::HashMap;

    struct Files {
        modules: HashMap<String, (AstFacts, String)>,
        /// `(importer, specifier)` to a landing, for the relative specifiers
        /// the test decides; every other relative one is `Unresolved`.
        landings: HashMap<(String, String), ModuleLanding>,
    }

    impl ModuleGraph for Files {
        fn module(&self, path: &str) -> Option<(&AstFacts, &str)> {
            self.modules
                .get(path)
                .map(|(ast, source)| (ast, source.as_str()))
        }

        fn landing(&self, importer: &str, specifier: &str) -> ModuleLanding {
            if !specifier.starts_with('.') {
                return ModuleLanding::Bare;
            }
            self.landings
                .get(&(importer.to_owned(), specifier.to_owned()))
                .cloned()
                .unwrap_or(ModuleLanding::Unresolved)
        }
    }

    /// `files`: `(path, source)`. Every relative specifier of the form `./x`
    /// lands on `x.js` unless `unresolved` names `(importer, specifier)`.
    fn graph(files: &[(&str, &str)], unresolved: &[(&str, &str)]) -> Files {
        let modules = files
            .iter()
            .map(|(path, source)| {
                (
                    (*path).to_owned(),
                    (extract(*path, source).unwrap(), (*source).to_owned()),
                )
            })
            .collect::<HashMap<_, _>>();
        let mut landings = HashMap::new();
        for (importer, (ast, _)) in &modules {
            let specifiers = ast
                .imports
                .iter()
                .map(|import| import.module.to_string())
                .chain(
                    ast.exports
                        .iter()
                        .filter_map(|export| export.module.as_ref().map(ToString::to_string)),
                );
            for specifier in specifiers.filter(|specifier| specifier.starts_with("./")) {
                let target = format!("{}.js", specifier.trim_start_matches("./"));
                if unresolved.contains(&(importer.as_str(), specifier.as_str())) {
                    continue;
                }
                landings.insert(
                    (importer.clone(), specifier.clone()),
                    if modules.contains_key(&target) {
                        ModuleLanding::File(target)
                    } else {
                        ModuleLanding::Unresolved
                    },
                );
            }
        }
        Files { modules, landings }
    }

    fn published(
        files: &Files,
        entry: &str,
        binding_in: &str,
        binding: &str,
    ) -> Option<Vec<String>> {
        let (_, source) = files.module(binding_in).unwrap();
        let start = u32::try_from(source.find(binding).unwrap()).unwrap();
        let span = Span {
            start,
            end: start + u32::try_from(binding.len()).unwrap(),
        };
        entry_names_publishing_import(files, entry, binding_in, span)
    }

    const BARREL: &str = "import { number, json } from \"dep\";\nexport { number, json };\n";

    fn named(names: &[&str]) -> Option<Vec<String>> {
        Some(names.iter().map(|name| (*name).to_owned()).collect())
    }

    #[test]
    fn an_entry_import_of_the_barrel_publishes_the_binding_under_its_own_name() {
        let files = graph(
            &[
                (
                    "index.js",
                    "import { number, json } from \"./transform\";\nexport { number, json };\n",
                ),
                ("transform.js", BARREL),
            ],
            &[],
        );
        assert_eq!(
            published(&files, "index.js", "transform.js", "number"),
            named(&["number"])
        );
    }

    #[test]
    fn a_rename_on_the_way_is_followed() {
        let files = graph(
            &[
                (
                    "index.js",
                    "import { number as n } from \"./transform\";\nexport { n as count };\nexport { json as parse } from \"./transform\";\n",
                ),
                ("transform.js", BARREL),
            ],
            &[],
        );
        assert_eq!(
            published(&files, "index.js", "transform.js", "number"),
            named(&["count"])
        );
        assert_eq!(
            published(&files, "index.js", "transform.js", "json"),
            named(&["parse"])
        );
    }

    #[test]
    fn a_re_export_specifier_and_a_second_barrel_are_followed() {
        let files = graph(
            &[
                (
                    "index.js",
                    "export { number } from \"./middle\";\nexport const local = 1;\n",
                ),
                ("middle.js", "export * from \"./transform\";\n"),
                ("transform.js", BARREL),
            ],
            &[],
        );
        assert_eq!(
            published(&files, "index.js", "transform.js", "number"),
            named(&["number"])
        );
    }

    #[test]
    fn an_unambiguous_star_publishes_the_name() {
        let files = graph(
            &[
                (
                    "index.js",
                    "export * from \"./transform\";\nexport * from \"./other\";\n",
                ),
                ("transform.js", BARREL),
                ("other.js", "export const other = 1;\n"),
            ],
            &[],
        );
        assert_eq!(
            published(&files, "index.js", "transform.js", "number"),
            named(&["number"])
        );
    }

    #[test]
    fn a_star_beside_an_explicit_export_of_the_same_name_publishes_the_explicit_one() {
        let files = graph(
            &[
                (
                    "index.js",
                    "export * from \"./transform\";\nexport const number = 1;\n",
                ),
                (
                    "transform.js",
                    "import { number } from \"dep\";\nexport { number };\n",
                ),
            ],
            &[],
        );
        // The explicit local declaration shadows the star, so `number` is
        // not the binding and no name reaches it.
        assert_eq!(
            published(&files, "index.js", "transform.js", "number"),
            None
        );
    }

    #[test]
    fn an_ambiguous_star_refuses() {
        // Two stars provide `number`: it is not exported at all.
        let files = graph(
            &[
                (
                    "index.js",
                    "export * from \"./transform\";\nexport * from \"./twin\";\n",
                ),
                (
                    "transform.js",
                    "import { number } from \"dep\";\nexport { number };\n",
                ),
                ("twin.js", "export const number = 2;\n"),
            ],
            &[],
        );
        assert_eq!(
            published(&files, "index.js", "transform.js", "number"),
            None
        );
        // A star of another package might provide the name as well.
        let files = graph(
            &[
                (
                    "index.js",
                    "export * from \"./transform\";\nexport * from \"dep\";\n",
                ),
                (
                    "transform.js",
                    "import { number } from \"dep\";\nexport { number };\n",
                ),
            ],
            &[],
        );
        assert_eq!(
            published(&files, "index.js", "transform.js", "number"),
            None
        );
    }

    #[test]
    fn a_cycle_refuses() {
        let files = graph(
            &[
                ("index.js", "export * from \"./a\";\n"),
                (
                    "a.js",
                    "export * from \"./b\";\nexport * from \"./transform\";\n",
                ),
                ("b.js", "export * from \"./a\";\n"),
                (
                    "transform.js",
                    "import { number } from \"dep\";\nexport { number };\n",
                ),
            ],
            &[],
        );
        assert_eq!(
            published(&files, "index.js", "transform.js", "number"),
            None
        );
    }

    #[test]
    fn an_unresolved_edge_refuses() {
        let source = "import { number, json } from \"./transform\";\nexport { number, json };\n";
        let files = graph(
            &[("index.js", source), ("transform.js", BARREL)],
            &[("index.js", "./transform")],
        );
        assert_eq!(
            published(&files, "index.js", "transform.js", "number"),
            None
        );
        // An edge the walk does not need to follow still decides: a name
        // whose own edge is unresolved could be the binding.
        let files = graph(
            &[
                (
                    "index.js",
                    "import { number } from \"./transform\";\nexport { number };\nexport { x } from \"./ghost\";\n",
                ),
                ("transform.js", BARREL),
            ],
            &[],
        );
        assert_eq!(
            published(&files, "index.js", "transform.js", "number"),
            None
        );
    }

    #[test]
    fn a_namespace_of_the_barrel_refuses() {
        for entry in [
            "export * as t from \"./transform\";\nexport { number } from \"./transform\";\n",
            "import * as t from \"./transform\";\nexport { t };\nexport { number } from \"./transform\";\n",
        ] {
            let files = graph(&[("index.js", entry), ("transform.js", BARREL)], &[]);
            assert_eq!(
                published(&files, "index.js", "transform.js", "number"),
                None,
                "{entry}"
            );
        }
    }

    #[test]
    fn a_barrel_that_uses_the_binding_or_an_entry_that_does_not_publish_it_refuses() {
        let published_by_entry = "export { number } from \"./transform\";\n";
        let used = "import { number } from \"dep\";\nexport { number };\nexport function f() { return number(); }\n";
        let files = graph(
            &[("index.js", published_by_entry), ("transform.js", used)],
            &[],
        );
        assert_eq!(
            published(&files, "index.js", "transform.js", "number"),
            None
        );
        // The entry never publishes the barrel's name.
        let files = graph(
            &[
                ("index.js", "export const other = 1;\n"),
                ("transform.js", BARREL),
            ],
            &[],
        );
        assert_eq!(
            published(&files, "index.js", "transform.js", "number"),
            None
        );
        // The entry is the module itself: the entry rung answers, not this.
        let files = graph(&[("transform.js", BARREL)], &[]);
        assert_eq!(
            published(&files, "transform.js", "transform.js", "number"),
            None
        );
    }

    #[test]
    fn a_type_only_or_unmodelled_export_of_the_name_refuses() {
        let files = graph(
            &[
                (
                    "index.ts",
                    "export { number } from \"./transform\";\nexport type { number as number2 } from \"./transform\";\n",
                ),
                ("transform.js", BARREL),
            ],
            &[],
        );
        // `number2` is type-only and does not publish a value.
        assert_eq!(
            published(&files, "index.ts", "transform.js", "number"),
            named(&["number"])
        );
        let files = graph(
            &[
                (
                    "index.ts",
                    "export { number } from \"./transform\";\nexport type { number } from \"./transform\";\n",
                ),
                ("transform.js", BARREL),
            ],
            &[],
        );
        assert_eq!(
            published(&files, "index.ts", "transform.js", "number"),
            None
        );
    }
}
