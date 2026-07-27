//! Read-optimized project indexes used by every analysis stage.
//!
//! This module hides AST and TypeScript table layout from rule discovery. The
//! builder asks semantic questions here instead of repeatedly scanning facts.

use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

use solid_facts::{FileFacts, ProjectFacts};
use solid_facts_core::Span;
use typefacts::{EntityFact, FactTable, FileFact, Location, SymbolFact, TypeDescriptor};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct EntitySymbols {
    pub(super) by_path: HashMap<String, HashMap<(u64, u64), String>>,
}

impl EntitySymbols {
    pub(super) fn get(&self, location: &Location) -> Option<&String> {
        self.by_path
            .get(location.path.as_ref())
            .and_then(|entities| entities.get(&(location.start_byte, location.end_byte)))
    }

    pub(super) fn at(&self, path: &str, span: Span) -> Option<&String> {
        self.by_path
            .get(path)
            .and_then(|entities| entities.get(&(u64::from(span.start), u64::from(span.end))))
    }
}

pub(super) struct ProjectIndexes<'a> {
    pub(super) files_by_path: HashMap<&'a str, &'a FileFacts>,
    pub(super) ast_files_by_path: HashMap<&'a str, &'a CachedAstFileIndex>,
    typescript: &'a FactTable,
    pub(super) symbols_by_id: HashMap<&'a str, &'a SymbolFact>,
}

impl<'a> ProjectIndexes<'a> {
    pub(super) fn new(
        facts: &'a ProjectFacts,
        ast_indexes: &'a HashMap<String, CachedAstFileIndex>,
    ) -> Self {
        let files_by_path = facts
            .files
            .iter()
            .map(|file| (file.path.as_str(), file))
            .collect();
        let ast_files_by_path = facts
            .files
            .iter()
            .filter_map(|file| {
                ast_indexes
                    .get(file.path.as_str())
                    .map(|index| (file.path.as_str(), index))
            })
            .collect();
        let symbols_by_id = facts
            .typescript
            .symbols
            .iter()
            .map(|symbol| (symbol.id.as_ref(), symbol))
            .collect();
        Self {
            files_by_path,
            ast_files_by_path,
            typescript: &facts.typescript,
            symbols_by_id,
        }
    }

    pub(super) fn typescript_file(&self, path: &str) -> Option<&'a FileFact> {
        self.typescript
            .files
            .binary_search_by(|file| file.path.as_ref().cmp(path))
            .ok()
            .map(|index| &self.typescript.files[index])
    }

    pub(super) fn entities_for_path(&self, path: &str) -> &'a [EntityFact] {
        let start = self
            .typescript
            .entities
            .partition_point(|entity| entity.location.path.as_ref() < path);
        let end = self
            .typescript
            .entities
            .partition_point(|entity| entity.location.path.as_ref() <= path);
        &self.typescript.entities[start..end]
    }
}

pub(super) struct CachedAstFileIndex {
    pub(super) ast: Arc<solid_ast_facts::AstFacts>,
    calls_by_span: HashMap<Span, usize>,
    calls_by_callee: HashMap<Span, Vec<usize>>,
    direct_calls_by_callee: HashMap<Span, usize>,
    functions_by_span: HashMap<Span, usize>,
}

impl CachedAstFileIndex {
    pub(super) fn new(file: &FileFacts) -> Self {
        let mut calls_by_span = HashMap::new();
        let mut calls_by_callee = HashMap::<Span, Vec<_>>::new();
        let mut direct_calls_by_callee = HashMap::new();
        for (index, call) in file.ast.calls.iter().enumerate() {
            calls_by_span.entry(call.span).or_insert(index);
            calls_by_callee.entry(call.callee).or_default().push(index);
            if call.direct_callee {
                direct_calls_by_callee.entry(call.callee).or_insert(index);
            }
        }
        let mut functions_by_span = HashMap::new();
        for (index, function) in file.ast.functions.iter().enumerate() {
            functions_by_span.entry(function.span).or_insert(index);
        }
        Self {
            ast: file.ast.clone(),
            calls_by_span,
            calls_by_callee,
            direct_calls_by_callee,
            functions_by_span,
        }
    }

    fn call(&self, index: usize) -> &solid_ast_facts::CallFact {
        &self.ast.calls[index]
    }

    fn function(&self, index: usize) -> &solid_ast_facts::FunctionFact {
        &self.ast.functions[index]
    }

    pub(super) fn call_by_span(&self, span: Span) -> Option<&solid_ast_facts::CallFact> {
        self.calls_by_span.get(&span).map(|index| self.call(*index))
    }

    pub(super) fn direct_call_by_callee(&self, span: Span) -> Option<&solid_ast_facts::CallFact> {
        self.direct_calls_by_callee
            .get(&span)
            .map(|index| self.call(*index))
    }

    pub(super) fn calls_by_callee(
        &self,
        span: Span,
    ) -> impl Iterator<Item = &solid_ast_facts::CallFact> {
        self.calls_by_callee
            .get(&span)
            .into_iter()
            .flatten()
            .map(|index| self.call(*index))
    }

    pub(super) fn function_by_span(&self, span: Span) -> Option<&solid_ast_facts::FunctionFact> {
        self.functions_by_span
            .get(&span)
            .map(|index| self.function(*index))
    }
}

/// A resolution of a checker symbol to the project function it names.
///
/// `Aborted` reproduces the legacy scan's early return: a matching
/// function-initialized binding without a recorded initializer span ends the
/// project search with no result, even if later files also match.
#[derive(Clone, Copy)]
enum SymbolFunction {
    Resolved { file: usize, function: usize },
    Aborted,
}

/// Whether any JSX call site renders a function, and whether one of those
/// call sites is wrapped in a Loading boundary in its caller file.
#[derive(Clone, Copy, Default)]
pub(super) struct CallSiteLoading {
    pub(super) any: bool,
    pub(super) loading_wrapped: bool,
}

/// Lazy project-wide lookups that replace repeated whole-project scans.
///
/// Every map is built at most once per build, on first use, in the exact
/// file/declaration order the scans it replaces used, so first-match and
/// first-writer results are unchanged. Warm builds that never ask a question
/// never pay for an index.
pub(super) struct SemanticLookup<'a> {
    facts: &'a ProjectFacts,
    entities: &'a EntitySymbols,
    symbol_names: &'a HashMap<String, String>,
    functions_by_symbol: OnceLock<HashMap<&'a str, SymbolFunction>>,
    entities_by_location: OnceLock<HashMap<(&'a str, u64, u64), usize>>,
    jsx_call_sites: OnceLock<HashMap<(&'a str, Span), CallSiteLoading>>,
    files_by_path: OnceLock<HashMap<&'a str, usize>>,
    file_primitives: OnceLock<Vec<OnceLock<FilePrimitives>>>,
}

/// Resolved Solid primitive names for one file's calls and JSX elements,
/// index-aligned with `file.ast.calls` / `file.ast.jsx_elements`. Computed
/// once per file per build so per-call classifier scans stop re-resolving
/// (and re-allocating) the same names.
pub(super) struct FilePrimitives {
    pub(super) calls: Vec<Option<super::PrimitiveName>>,
    pub(super) jsx: Vec<Option<super::PrimitiveName>>,
}

impl<'a> SemanticLookup<'a> {
    pub(super) fn new(
        facts: &'a ProjectFacts,
        entities: &'a EntitySymbols,
        symbol_names: &'a HashMap<String, String>,
    ) -> Self {
        debug_assert!(
            facts
                .typescript
                .entities
                .windows(2)
                .all(|pair| pair[0].location.path <= pair[1].location.path),
            "entity table must be sorted by path for per-path containment slices"
        );
        Self {
            facts,
            entities,
            symbol_names,
            functions_by_symbol: OnceLock::new(),
            entities_by_location: OnceLock::new(),
            jsx_call_sites: OnceLock::new(),
            files_by_path: OnceLock::new(),
            file_primitives: OnceLock::new(),
        }
    }

    /// The memoized primitive names for one project file.
    pub(super) fn primitives(&self, file: &FileFacts) -> &FilePrimitives {
        let files_by_path = self.files_by_path.get_or_init(|| {
            self.facts
                .files
                .iter()
                .enumerate()
                .map(|(index, file)| (file.path.as_str(), index))
                .collect()
        });
        let slots = self
            .file_primitives
            .get_or_init(|| self.facts.files.iter().map(|_| OnceLock::new()).collect());
        let index = *files_by_path
            .get(file.path.as_str())
            .expect("primitive lookup for a file outside project facts");
        slots[index].get_or_init(|| {
            let file = &self.facts.files[index];
            FilePrimitives {
                calls: file
                    .ast
                    .calls
                    .iter()
                    .map(|call| {
                        super::primitive_name(
                            file.path.as_str(),
                            call.callee,
                            call.static_callee(&file.source),
                            self.entities,
                            self.symbol_names,
                        )
                    })
                    .collect(),
                jsx: file
                    .ast
                    .jsx_elements
                    .iter()
                    .map(|element| {
                        super::jsx_primitive_name(file, element, self.entities, self.symbol_names)
                    })
                    .collect(),
            }
        })
    }

    pub(super) fn entities(&self) -> &'a EntitySymbols {
        self.entities
    }

    pub(super) fn function_called_at(
        &self,
        path: &str,
        callee: Span,
    ) -> Option<(&'a FileFacts, &'a solid_ast_facts::FunctionFact)> {
        let symbol = self.entities.at(path, callee)?;
        self.function_for_symbol(symbol)
    }

    pub(super) fn function_for_symbol(
        &self,
        symbol: &str,
    ) -> Option<(&'a FileFacts, &'a solid_ast_facts::FunctionFact)> {
        match self.functions_by_symbol().get(symbol)? {
            SymbolFunction::Resolved { file, function } => {
                let file = &self.facts.files[*file];
                Some((file, &file.ast.functions[*function]))
            }
            SymbolFunction::Aborted => None,
        }
    }

    pub(super) fn entity_at(&self, path: &str, span: Span) -> Option<&'a EntityFact> {
        self.entities_by_location()
            .get(&(path, u64::from(span.start), u64::from(span.end)))
            .map(|index| &self.facts.typescript.entities[*index])
    }

    pub(super) fn typescript_file(&self, path: &str) -> Option<&'a FileFact> {
        let files = &self.facts.typescript.files;
        files
            .binary_search_by(|file| file.path.as_ref().cmp(path))
            .ok()
            .map(|index| &files[index])
    }

    /// The symbol a callee span resolves to: the exact entity at the span,
    /// falling back to the smallest symbol-bearing entity contained in it.
    pub(super) fn callee_symbol(&self, path: &str, callee: Span) -> Option<&'a str> {
        self.entities
            .at(path, callee)
            .map(String::as_str)
            .or_else(|| {
                self.smallest_contained(path, callee, |entity| !entity.symbol.is_empty())
                    .map(|entity| entity.symbol.as_ref())
            })
    }

    /// The type descriptor of the smallest typed entity contained in a span.
    pub(super) fn smallest_contained_descriptor(
        &self,
        path: &str,
        span: Span,
    ) -> Option<&'a TypeDescriptor> {
        self.smallest_contained(path, span, |entity| entity.type_descriptor.is_some())
            .and_then(|entity| entity.type_descriptor.as_ref())
    }

    /// Whether any JSX call site renders the function at `(path, function)`,
    /// and whether one of those call sites sits under a Loading boundary.
    pub(super) fn jsx_call_site_loading(&self, path: &str, function: Span) -> CallSiteLoading {
        self.jsx_call_sites()
            .get(&(path, function))
            .copied()
            .unwrap_or_default()
    }

    fn entities_for_path(&self, path: &str) -> &'a [EntityFact] {
        let entities = &self.facts.typescript.entities;
        let start = entities.partition_point(|entity| entity.location.path.as_ref() < path);
        let end = entities.partition_point(|entity| entity.location.path.as_ref() <= path);
        &entities[start..end]
    }

    fn smallest_contained(
        &self,
        path: &str,
        span: Span,
        predicate: impl Fn(&EntityFact) -> bool,
    ) -> Option<&'a EntityFact> {
        self.entities_for_path(path)
            .iter()
            .filter(|entity| {
                u64::from(span.start) <= entity.location.start_byte
                    && entity.location.end_byte <= u64::from(span.end)
                    && predicate(entity)
            })
            .min_by_key(|entity| entity.location.end_byte - entity.location.start_byte)
    }

    fn functions_by_symbol(&self) -> &HashMap<&'a str, SymbolFunction> {
        self.functions_by_symbol.get_or_init(|| {
            let mut map = HashMap::new();
            for (file_index, file) in self.facts.files.iter().enumerate() {
                for (function_index, function) in file.ast.functions.iter().enumerate() {
                    let Some(name) = function.name.as_ref() else {
                        continue;
                    };
                    let Some(symbol) = self.entities.at(file.path.as_str(), name.span) else {
                        continue;
                    };
                    map.entry(symbol.as_str())
                        .or_insert(SymbolFunction::Resolved {
                            file: file_index,
                            function: function_index,
                        });
                }
                for binding in &file.ast.bindings {
                    if !binding.initializer_function {
                        continue;
                    }
                    let mut outcome = None;
                    for name in &binding.names {
                        let Some(symbol) = self.entities.at(file.path.as_str(), name.span) else {
                            continue;
                        };
                        if map.contains_key(symbol.as_str()) {
                            continue;
                        }
                        let outcome = *outcome.get_or_insert_with(|| match binding.initializer {
                            None => Some(SymbolFunction::Aborted),
                            Some(initializer) => file
                                .ast
                                .functions
                                .iter()
                                .enumerate()
                                .filter(|(_, function)| initializer.contains(function.span))
                                .max_by_key(|(_, function)| function.span.end - function.span.start)
                                .map(|(function_index, _)| SymbolFunction::Resolved {
                                    file: file_index,
                                    function: function_index,
                                }),
                        });
                        if let Some(outcome) = outcome {
                            map.insert(symbol.as_str(), outcome);
                        }
                    }
                }
            }
            map
        })
    }

    fn jsx_call_sites(&self) -> &HashMap<(&'a str, Span), CallSiteLoading> {
        self.jsx_call_sites.get_or_init(|| {
            let mut map = HashMap::<(&'a str, Span), CallSiteLoading>::new();
            for caller_file in &self.facts.files {
                for element in &caller_file.ast.jsx_elements {
                    let Some((target_file, target)) =
                        self.function_called_at(caller_file.path.as_str(), element.name.span)
                    else {
                        continue;
                    };
                    let entry = map
                        .entry((target_file.path.as_str(), target.span))
                        .or_default();
                    entry.any = true;
                    if !entry.loading_wrapped {
                        entry.loading_wrapped =
                            caller_file.ast.jsx_elements.iter().any(|boundary| {
                                boundary.span.contains(element.span)
                                    && boundary.span != element.span
                                    && super::jsx_element_is_loading(
                                        caller_file,
                                        boundary,
                                        self.entities,
                                        self.symbol_names,
                                    )
                            });
                    }
                }
            }
            map
        })
    }

    fn entities_by_location(&self) -> &HashMap<(&'a str, u64, u64), usize> {
        self.entities_by_location.get_or_init(|| {
            let entities = &self.facts.typescript.entities;
            let mut map = HashMap::with_capacity(entities.len());
            for (index, entity) in entities.iter().enumerate() {
                map.entry((
                    entity.location.path.as_ref(),
                    entity.location.start_byte,
                    entity.location.end_byte,
                ))
                .or_insert(index);
            }
            map
        })
    }
}
