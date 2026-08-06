//! Read-optimized project indexes used by every analysis stage.
//!
//! This module hides AST and TypeScript table layout from rule discovery. The
//! builder asks semantic questions here instead of repeatedly scanning facts.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, OnceLock},
};

use solid_facts::core::Span;
use solid_facts::{FileFacts, ProjectFacts, TypeScriptSymbol, TypeScriptTable};
use typefacts::{Callability, EntityFact, FileFact, Location, ResolvedCall, TypeDescriptor};

use super::{SymbolId, SymbolName};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct EntitySymbols {
    pub(super) by_path: HashMap<String, HashMap<(u64, u64), SymbolId>>,
}

impl EntitySymbols {
    pub(super) fn get(&self, location: &Location) -> Option<&SymbolId> {
        self.by_path
            .get(location.path.as_ref())
            .and_then(|entities| entities.get(&(location.start_byte, location.end_byte)))
    }

    pub(super) fn at(&self, path: &str, span: Span) -> Option<&SymbolId> {
        self.by_path
            .get(path)
            .and_then(|entities| entities.get(&(u64::from(span.start), u64::from(span.end))))
    }
}

pub(super) struct ProjectIndexes<'a> {
    pub(super) files_by_path: HashMap<&'a str, &'a FileFacts>,
    pub(super) ast_files_by_path: HashMap<&'a str, &'a CachedAstFileIndex>,
    typescript: &'a TypeScriptTable,
    pub(super) symbols_by_id: HashMap<&'a str, TypeScriptSymbol<'a>>,
}

impl<'a> ProjectIndexes<'a> {
    pub(super) fn new(
        facts: &'a ProjectFacts,
        ast_indexes: &'a HashMap<solid_facts::core::SourcePath, CachedAstFileIndex>,
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
            .symbols()
            .map(|symbol| (symbol.id(), symbol))
            .collect();
        Self {
            files_by_path,
            ast_files_by_path,
            typescript: &facts.typescript,
            symbols_by_id,
        }
    }

    pub(super) fn typescript_file(&self, path: &str) -> Option<&'a FileFact> {
        self.typescript.file(path)
    }

    pub(super) fn entities_for_path(&self, path: &str) -> &'a [EntityFact] {
        self.typescript.entities_for_path(path)
    }
}

pub(super) struct CachedAstFileIndex {
    pub(super) ast: Arc<solid_facts::ast::AstFacts>,
    calls_by_span: HashMap<Span, usize>,
    calls_by_callee: HashMap<Span, Vec<usize>>,
    direct_calls_by_callee: HashMap<Span, usize>,
    functions_by_span: HashMap<Span, usize>,
    member_properties_by_span: HashMap<Span, Span>,
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
        let member_properties_by_span = file
            .ast
            .members
            .iter()
            .map(|member| (member.span, member.property))
            .collect();
        Self {
            ast: file.ast.clone(),
            calls_by_span,
            calls_by_callee,
            direct_calls_by_callee,
            functions_by_span,
            member_properties_by_span,
        }
    }

    fn call(&self, index: usize) -> &solid_facts::ast::CallFact {
        &self.ast.calls[index]
    }

    fn function(&self, index: usize) -> &solid_facts::ast::FunctionFact {
        &self.ast.functions[index]
    }

    pub(super) fn call_by_span(&self, span: Span) -> Option<&solid_facts::ast::CallFact> {
        self.calls_by_span.get(&span).map(|index| self.call(*index))
    }

    pub(super) fn direct_call_by_callee(&self, span: Span) -> Option<&solid_facts::ast::CallFact> {
        self.direct_calls_by_callee
            .get(&span)
            .map(|index| self.call(*index))
    }

    pub(super) fn calls_by_callee(
        &self,
        span: Span,
    ) -> impl Iterator<Item = &solid_facts::ast::CallFact> {
        self.calls_by_callee
            .get(&span)
            .into_iter()
            .flatten()
            .map(|index| self.call(*index))
    }

    pub(super) fn call_by_callee(&self, span: Span) -> Option<&solid_facts::ast::CallFact> {
        self.calls_by_callee(span).next()
    }

    pub(super) fn function_by_span(&self, span: Span) -> Option<&solid_facts::ast::FunctionFact> {
        self.functions_by_span
            .get(&span)
            .map(|index| self.function(*index))
    }

    pub(super) fn member_property(&self, span: Span) -> Option<Span> {
        self.member_properties_by_span.get(&span).copied()
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

#[derive(Clone)]
struct BindingResolution {
    file: usize,
    binding: usize,
    symbol: SymbolId,
}

type BindingsByReference = HashMap<String, HashMap<(u64, u64), BindingResolution>>;

/// Lazy project-wide lookups that replace repeated whole-project scans.
///
/// Every map is built at most once per build, on first use, in the exact
/// file/declaration order the scans it replaces used, so first-match and
/// first-writer results are unchanged. Warm builds that never ask a question
/// never pay for an index.
pub(super) struct SemanticLookup<'a> {
    facts: &'a ProjectFacts,
    /// The Solid-version vocabulary this build analyzes with. Every consumer
    /// of the lookup shares one dialect; a build never mixes two.
    pub(super) dialect: &'a dyn solid_dialect::Dialect,
    ast_indexes: &'a HashMap<solid_facts::core::SourcePath, CachedAstFileIndex>,
    entities: &'a EntitySymbols,
    symbol_names: &'a HashMap<SymbolId, SymbolName>,
    functions_by_symbol: OnceLock<HashMap<&'a str, SymbolFunction>>,
    entities_by_location: OnceLock<HashMap<(&'a str, u64, u64), &'a EntityFact>>,
    contained_entities_by_path: OnceLock<HashMap<&'a str, Vec<&'a EntityFact>>>,
    descriptors_by_symbol: OnceLock<HashMap<&'a str, &'a TypeDescriptor>>,
    callability_by_symbol: OnceLock<HashMap<&'a str, Callability>>,
    symbols_by_id: OnceLock<HashMap<&'a str, solid_facts::TypeScriptSymbol<'a>>>,
    context_symbols: OnceLock<HashSet<&'a str>>,
    jsx_call_sites: OnceLock<HashMap<(&'a str, Span), CallSiteLoading>>,
    bindings_by_reference: OnceLock<BindingsByReference>,
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
        ast_indexes: &'a HashMap<solid_facts::core::SourcePath, CachedAstFileIndex>,
        entities: &'a EntitySymbols,
        symbol_names: &'a HashMap<SymbolId, SymbolName>,
        dialect: &'a dyn solid_dialect::Dialect,
    ) -> Self {
        debug_assert!(
            facts
                .typescript
                .entities()
                .map(|entity| entity.location.path.as_ref())
                .is_sorted(),
            "entity table must be sorted by path for per-path containment slices"
        );
        Self {
            facts,
            dialect,
            ast_indexes,
            entities,
            symbol_names,
            functions_by_symbol: OnceLock::new(),
            entities_by_location: OnceLock::new(),
            contained_entities_by_path: OnceLock::new(),
            descriptors_by_symbol: OnceLock::new(),
            callability_by_symbol: OnceLock::new(),
            symbols_by_id: OnceLock::new(),
            context_symbols: OnceLock::new(),
            jsx_call_sites: OnceLock::new(),
            bindings_by_reference: OnceLock::new(),
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
                            self.dialect,
                        )
                    })
                    .collect(),
                jsx: file
                    .ast
                    .jsx_elements
                    .iter()
                    .map(|element| {
                        super::jsx_primitive_name(
                            file,
                            element,
                            self.entities,
                            self.symbol_names,
                            self.dialect,
                        )
                    })
                    .collect(),
            }
        })
    }

    pub(super) fn entities(&self) -> &'a EntitySymbols {
        self.entities
    }

    pub(super) fn symbol_names(&self) -> &'a HashMap<SymbolId, SymbolName> {
        self.symbol_names
    }

    pub(super) fn files(&self) -> &'a [FileFacts] {
        &self.facts.files
    }

    pub(super) fn symbol_references(&self, symbol: &str) -> Vec<Location> {
        self.symbols_by_id
            .get_or_init(|| {
                self.facts
                    .typescript
                    .symbols()
                    .map(|candidate| (candidate.id(), candidate))
                    .collect()
            })
            .get(symbol)
            .map(|candidate| candidate.references().cloned().collect())
            .unwrap_or_default()
    }

    /// The binding declaration named by an exact canonical symbol reference.
    ///
    /// Returned functions can cross files and destructuring patterns before
    /// they are called. Building this reverse index once keeps that proof
    /// linear in project facts instead of rescanning every binding for every
    /// call site.
    pub(super) fn binding_at_reference(
        &self,
        path: &str,
        span: Span,
    ) -> Option<(&'a FileFacts, &'a solid_facts::ast::BindingFact, SymbolId)> {
        let resolution = self
            .bindings_by_reference()
            .get(path)?
            .get(&(u64::from(span.start), u64::from(span.end)))?;
        let file = &self.facts.files[resolution.file];
        Some((
            file,
            &file.ast.bindings[resolution.binding],
            resolution.symbol.clone(),
        ))
    }

    fn bindings_by_reference(&self) -> &BindingsByReference {
        self.bindings_by_reference.get_or_init(|| {
            let symbols = self.symbols_by_id.get_or_init(|| {
                self.facts
                    .typescript
                    .symbols()
                    .map(|candidate| (candidate.id(), candidate))
                    .collect()
            });
            let mut by_path = HashMap::<String, HashMap<(u64, u64), BindingResolution>>::new();
            let mut by_symbol = HashMap::<SymbolId, BindingResolution>::new();
            for (file_index, file) in self.facts.files.iter().enumerate() {
                for (binding_index, binding) in file.ast.bindings.iter().enumerate() {
                    for name in &binding.names {
                        let Some(symbol) = self.entities.at(file.path.as_str(), name.span) else {
                            continue;
                        };
                        let resolution = BindingResolution {
                            file: file_index,
                            binding: binding_index,
                            symbol: symbol.clone(),
                        };
                        by_symbol
                            .entry(symbol.clone())
                            .or_insert_with(|| resolution.clone());
                        by_path
                            .entry(file.path.to_string())
                            .or_default()
                            .entry((u64::from(name.span.start), u64::from(name.span.end)))
                            .or_insert_with(|| resolution.clone());
                        if let Some(candidate) = symbols.get(symbol.as_str()) {
                            for reference in candidate.references() {
                                by_path
                                    .entry(reference.path.to_string())
                                    .or_default()
                                    .entry((reference.start_byte, reference.end_byte))
                                    .or_insert_with(|| resolution.clone());
                            }
                        }
                    }
                }
            }
            // The symbol-reference table proves aliases even when no entity
            // was demanded at a use. Conversely, exact entity facts can exist
            // at a use omitted from that reference projection. Retain both
            // compiler proofs, matching the former direct-or-reference query.
            for (path, entities) in &self.entities.by_path {
                for ((start, end), symbol) in entities {
                    if let Some(resolution) = by_symbol.get(symbol) {
                        by_path
                            .entry(path.clone())
                            .or_default()
                            .entry((*start, *end))
                            .or_insert_with(|| resolution.clone());
                    }
                }
            }
            by_path
        })
    }

    /// Whether the last reference in this JSX member-object span resolves to
    /// a binding initialized by the dialect's `createContext` primitive
    /// anywhere in the project.
    ///
    /// Context providers are routinely declared in one module and rendered
    /// in another. Indexing canonical TypeScript symbols keeps that ordinary
    /// import/re-export boundary from erasing the runtime contract while
    /// still rejecting objects that merely expose a property named
    /// `Provider`. Matching the reference that ends with the object span also
    /// supports `<contexts.ValueContext.Provider>` without confusing it with
    /// `<ValueContext.someObject.Provider>`.
    pub(super) fn is_context_reference(&self, path: &str, span: Span) -> bool {
        self.context_symbols().iter().any(|symbol| {
            self.symbols_by_id
                .get_or_init(|| {
                    self.facts
                        .typescript
                        .symbols()
                        .map(|candidate| (candidate.id(), candidate))
                        .collect()
                })
                .get(symbol)
                .is_some_and(|candidate| {
                    candidate.references().any(|reference| {
                        reference.path.as_ref() == path
                            && reference.start_byte >= u64::from(span.start)
                            && reference.end_byte == u64::from(span.end)
                    })
                })
        })
    }

    fn context_symbols(&self) -> &HashSet<&'a str> {
        self.context_symbols.get_or_init(|| {
            self.facts
                .files
                .iter()
                .flat_map(|file| {
                    let primitives = self.primitives(file);
                    file.ast.bindings.iter().filter_map(move |binding| {
                        let initializer = binding.call_initializer?;
                        let call = file
                            .ast
                            .calls
                            .iter()
                            .position(|call| call.span == initializer)?;
                        (super::known_primitive(&primitives.calls[call])
                            == Some(solid_dialect::Primitive::CreateContext))
                        .then(|| {
                            binding.names.iter().find_map(|name| {
                                self.entities
                                    .at(file.path.as_str(), name.span)
                                    .map(|symbol| symbol.as_str())
                            })
                        })
                        .flatten()
                    })
                })
                .collect()
        })
    }

    pub(super) fn call_by_callee(
        &self,
        file: &FileFacts,
        callee: Span,
    ) -> Option<&solid_facts::ast::CallFact> {
        self.ast_indexes
            .get(file.path.as_str())
            .and_then(|index| index.call_by_callee(callee))
    }

    /// The compiler-selected signature and argument mapping for this call.
    ///
    /// Resolved-call facts are demanded at the complete callee expression,
    /// unlike member declaration identity which lives on its property span.
    pub(super) fn resolved_callee_call(
        &self,
        file: &FileFacts,
        callee: Span,
    ) -> Option<&'a ResolvedCall> {
        let call_span = self
            .call_by_callee(file, callee)
            .map_or(callee, |call| call.span);
        self.entity_at(file.path.as_str(), callee)
            .and_then(|entity| entity.resolved_call.as_deref())
            .or_else(|| {
                self.entity_at(file.path.as_str(), call_span)
                    .and_then(|entity| entity.resolved_call.as_deref())
            })
            .or_else(|| {
                self.smallest_contained(file.path.as_str(), callee, |entity| {
                    entity.resolved_call.is_some()
                })
                .and_then(|entity| entity.resolved_call.as_deref())
            })
    }

    pub(super) fn function_called_at(
        &self,
        path: &str,
        callee: Span,
    ) -> Option<(&'a FileFacts, &'a solid_facts::ast::FunctionFact)> {
        let symbol = self.entities.at(path, callee)?;
        self.function_for_symbol(symbol)
    }

    pub(super) fn function_for_symbol(
        &self,
        symbol: &str,
    ) -> Option<(&'a FileFacts, &'a solid_facts::ast::FunctionFact)> {
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
            .copied()
    }

    pub(super) fn typescript_file(&self, path: &str) -> Option<&'a FileFact> {
        self.facts.typescript.file(path)
    }

    /// The symbol a callee span resolves to: the exact entity at the span,
    /// falling back to the smallest symbol-bearing entity contained in it.
    pub(super) fn callee_symbol(&self, file: &FileFacts, callee: Span) -> Option<&'a str> {
        self.ast_indexes
            .get(file.path.as_str())
            .and_then(|index| index.member_property(callee))
            .and_then(|property| self.entities.at(file.path.as_str(), property))
            .map(SymbolId::as_str)
            .or_else(|| {
                self.entities
                    .at(file.path.as_str(), callee)
                    .map(SymbolId::as_str)
            })
            .or_else(|| {
                self.smallest_contained(file.path.as_str(), callee, |entity| {
                    !entity.symbol.is_empty()
                })
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
            .and_then(|entity| entity.type_descriptor.as_deref())
            .or_else(|| {
                let symbol = self.entities.at(path, span)?;
                self.descriptors_by_symbol
                    .get_or_init(|| {
                        self.facts
                            .typescript
                            .entities()
                            .filter_map(|entity| {
                                (!entity.symbol.is_empty())
                                    .then_some(entity.symbol.as_ref())
                                    .zip(entity.type_descriptor.as_deref())
                            })
                            .collect()
                    })
                    .get(symbol.as_str())
                    .copied()
            })
    }

    /// Compiler-derived callability for the smallest demanded entity in a
    /// span, falling back to another demanded occurrence of the same symbol.
    pub(super) fn smallest_contained_callability(
        &self,
        path: &str,
        span: Span,
    ) -> Option<Callability> {
        self.smallest_contained(path, span, |entity| entity.callability.is_some())
            .and_then(|entity| entity.callability)
            .or_else(|| {
                let symbol = self.entities.at(path, span)?;
                self.callability_by_symbol
                    .get_or_init(|| {
                        self.facts
                            .typescript
                            .entities()
                            .filter_map(|entity| {
                                (!entity.symbol.is_empty())
                                    .then_some(entity.symbol.as_ref())
                                    .zip(entity.callability)
                            })
                            .collect()
                    })
                    .get(symbol.as_str())
                    .copied()
            })
    }

    /// Whether any JSX call site renders the function at `(path, function)`,
    /// and whether one of those call sites sits under a Loading boundary.
    pub(super) fn jsx_call_site_loading(&self, path: &str, function: Span) -> CallSiteLoading {
        self.jsx_call_sites()
            .get(&(path, function))
            .copied()
            .unwrap_or_default()
    }

    fn smallest_contained(
        &self,
        path: &str,
        span: Span,
        predicate: impl Fn(&EntityFact) -> bool,
    ) -> Option<&'a EntityFact> {
        let start = u64::from(span.start);
        let end = u64::from(span.end);
        let entities = self.contained_entities_by_path().get(path)?;
        let first = entities.partition_point(|entity| entity.location.start_byte < start);
        let last = entities.partition_point(|entity| entity.location.start_byte <= end);
        entities[first..last]
            .iter()
            .enumerate()
            .filter_map(|(index, entity)| {
                (entity.location.end_byte <= end && predicate(entity)).then_some((index, *entity))
            })
            .min_by_key(|(index, entity)| {
                (
                    entity.location.end_byte - entity.location.start_byte,
                    *index,
                )
            })
            .map(|(_, entity)| entity)
    }

    fn contained_entities_by_path(&self) -> &HashMap<&'a str, Vec<&'a EntityFact>> {
        self.contained_entities_by_path.get_or_init(|| {
            let mut by_path = HashMap::<&str, Vec<&EntityFact>>::new();
            for entity in self.facts.typescript.entities() {
                by_path
                    .entry(entity.location.path.as_ref())
                    .or_default()
                    .push(entity);
            }
            for entities in by_path.values_mut() {
                entities.sort_by_key(|entity| entity.location.start_byte);
            }
            by_path
        })
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
                                        self.dialect,
                                    )
                            });
                    }
                }
            }
            map
        })
    }

    fn entities_by_location(&self) -> &HashMap<(&'a str, u64, u64), &'a EntityFact> {
        self.entities_by_location.get_or_init(|| {
            let mut map = HashMap::new();
            for entity in self.facts.typescript.entities() {
                map.entry((
                    entity.location.path.as_ref(),
                    entity.location.start_byte,
                    entity.location.end_byte,
                ))
                .or_insert(entity);
            }
            map
        })
    }
}
