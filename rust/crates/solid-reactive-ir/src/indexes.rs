//! Read-optimized project indexes used by every analysis stage.
//!
//! This module hides AST and TypeScript table layout from rule discovery. The
//! builder asks semantic questions here instead of repeatedly scanning facts.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, OnceLock},
};

use sha2::{Digest, Sha256};
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

    /// The position of the call with exactly this span in `file.ast.calls`.
    ///
    /// The primitive, execution-role, and owner tables are index-aligned with
    /// that array, so every classifier that starts from a `CallFact` and needs
    /// its resolved primitive has to translate a span back into an index. Doing
    /// that with `calls.iter().position(..)` is a linear scan per processed
    /// call, which is quadratic in a file's call count.
    pub(super) fn call_index_by_span(&self, span: Span) -> Option<usize> {
        self.calls_by_span.get(&span).copied()
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
    file_named_callback_roles: OnceLock<Vec<OnceLock<super::NamedCallbackRoles>>>,
    project_primitives: OnceLock<HashSet<solid_dialect::Primitive>>,
    callback_capabilities: OnceLock<DialectCallbackCapabilities>,
    returned_callback_proof_digest: OnceLock<Option<CrossFileProofDigest>>,
}

/// Which parts of the returned-adapter contract this build's dialect can
/// actually answer, for the primitives this project names.
///
/// Solid 2.0 leaves every method behind these flags at its `false`/`None`
/// default: no primitive routes callbacks through a returned adapter, and none
/// stores a function argument as a value. The engine still has to *ask* those
/// questions per call, and the asking is not free — proving a returned adapter
/// is invoked walks binding chains across every project file. One probe of the
/// vocabulary per build lets the passes skip machinery that provably cannot
/// answer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct DialectCallbackCapabilities {
    /// Some primitive routes a callback through a function it returns, so
    /// call-site proofs of that returned function's use are meaningful.
    pub(super) returned_callbacks: bool,
    /// Some primitive stores a function argument instead of invoking it, so
    /// dormant-argument classification is meaningful.
    pub(super) stored_function_arguments: bool,
}

/// Digest of the project facts the cross-file returned-callback proofs read.
pub(super) type CrossFileProofDigest = [u8; 32];

/// Argument positions, argument counts, and tuple slots wide enough to cover
/// every Solid signature either dialect models. Each probe is one table
/// lookup, and the whole sweep runs once per build over the handful of
/// primitives a project actually names.
const PROBED_ARGUMENTS: usize = 8;
const PROBED_RESULT_SLOTS: usize = 4;

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
            file_named_callback_roles: OnceLock::new(),
            project_primitives: OnceLock::new(),
            callback_capabilities: OnceLock::new(),
            returned_callback_proof_digest: OnceLock::new(),
        }
    }

    /// Every primitive this build can resolve at a call site.
    ///
    /// `primitive_name` answers only through the symbol-name table, so a
    /// primitive absent from this set cannot appear at any call, JSX tag, or
    /// binding initializer in this project. That makes the set a sound domain
    /// for probing what the dialect models.
    fn project_primitives(&self) -> &HashSet<solid_dialect::Primitive> {
        self.project_primitives.get_or_init(|| {
            self.symbol_names
                .values()
                .filter_map(|name| self.dialect.primitive(name.as_str()))
                .collect()
        })
    }

    pub(super) fn callback_capabilities(&self) -> DialectCallbackCapabilities {
        *self.callback_capabilities.get_or_init(|| {
            let mut capabilities = DialectCallbackCapabilities::default();
            for primitive in self.project_primitives() {
                for argument in 0..PROBED_ARGUMENTS {
                    capabilities.returned_callbacks |= self
                        .dialect
                        .callback_requires_return_invocation(*primitive, argument);
                    capabilities.stored_function_arguments |= self
                        .dialect
                        .stores_function_argument_as_value(*primitive, argument);
                    for count in 0..PROBED_ARGUMENTS {
                        for slot in std::iter::once(None).chain((0..PROBED_RESULT_SLOTS).map(Some))
                        {
                            capabilities.returned_callbacks |= self
                                .dialect
                                .returned_callback_execution_at(*primitive, slot, argument, count)
                                .is_some()
                                || self
                                    .dialect
                                    .returned_callback_owner_at(*primitive, slot, argument, count)
                                    .is_some();
                        }
                    }
                }
            }
            capabilities
        })
    }

    /// Whether the dialect routes any callback of a primitive this project
    /// names through a function the primitive returns.
    pub(super) fn models_returned_callbacks(&self) -> bool {
        self.callback_capabilities().returned_callbacks
    }

    /// Whether the dialect stores any function argument of a primitive this
    /// project names as a plain value instead of invoking it.
    pub(super) fn models_stored_function_arguments(&self) -> bool {
        self.callback_capabilities().stored_function_arguments
    }

    /// Whether the cross-file proofs can read anything at all from this file.
    ///
    /// Every read either machine makes of a file *other* than the one it was
    /// asked about is drawn from one of four AST tables: `calls` (invocation
    /// sites and the `factory(...)()` shape), `jsx_elements` (a rendered
    /// adapter), `members` (`lazyResult.preload()`), and `bindings` (the factory
    /// seed, its destructured tuple slots, and every identifier alias the
    /// closure walks). A file with all four empty is never resolved against, so
    /// its TypeScript facts are never consulted either — and editing it cannot
    /// move any proof.
    fn participates_in_returned_callback_proofs(file: &FileFacts) -> bool {
        !file.ast.calls.is_empty()
            || !file.ast.jsx_elements.is_empty()
            || !file.ast.members.is_empty()
            || !file.ast.bindings.is_empty()
    }

    /// Identity of the whole-project facts that the cross-file
    /// returned-callback proofs read, or `None` when no such proof can exist.
    ///
    /// Those proofs — "is the function `mapArray` returned in file A ever
    /// invoked?" — scan every project file's calls, members, JSX tags and
    /// bindings, yet their answers land in *per-file* cache fragments keyed on
    /// that one file's source hash. Editing the only file that invokes the
    /// adapter would otherwise leave the factory file's fragment untouched and
    /// stale. Folding this digest into those fragments' reuse identity closes
    /// that hole.
    ///
    /// One thing narrows it, provable from the machinery's loop headers alone: a
    /// file contributes only if a proof can read anything from it at all (see
    /// [`Self::participates_in_returned_callback_proofs`]). Editing a
    /// declaration-only module therefore invalidates nothing.
    ///
    /// # Why the contribution is still the whole source hash
    ///
    /// The obvious next step — digest each participating file's *proof-relevant
    /// projection* (its call callees, member invocations and alias bindings)
    /// instead of its source — is not sound, for two independent reasons.
    ///
    /// The proofs resolve every position through the project-wide TypeScript
    /// symbol and reference tables: `returned_binding_reference` accepts a span
    /// because some adapter symbol's *reference list* covers it, and
    /// `returned_primitive_invocation` walks binding chains through
    /// `binding_at_reference`. Those tables are a whole-project product, so an
    /// edit to file A can change what a position in file B resolves to while B's
    /// own AST is byte-identical. A projection of B alone cannot see that.
    ///
    /// And the local-access fragments depend on more than whether a site exists:
    /// `returned_factory_callback_execution_role` classifies each site's span
    /// *in the using file*, so the answer folds in that file's whole execution
    /// context. A projection would have to carry the classifier's entire input,
    /// which is the file.
    ///
    /// Decomposing the digest per file therefore needs the project-wide reverse
    /// index from adapter bindings to their use sites that the machinery does
    /// not build. Until it exists, a participating file's contribution stays its
    /// source hash: coarse, but never stale.
    pub(super) fn returned_callback_proof_digest(&self) -> Option<CrossFileProofDigest> {
        *self.returned_callback_proof_digest.get_or_init(|| {
            if !self.models_returned_callbacks() {
                return None;
            }
            let mut inputs = self
                .facts
                .files
                .iter()
                .filter(|file| Self::participates_in_returned_callback_proofs(file))
                .map(|file| (file.path.as_str(), file.source_hash.as_str()))
                .collect::<Vec<_>>();
            // Facts arrive in configured-source order; sort so a reordered
            // source set cannot read as a changed one.
            inputs.sort_unstable();
            let mut hasher = Sha256::new();
            for (path, source_hash) in inputs {
                hasher.update(u64::try_from(path.len()).unwrap_or(u64::MAX).to_le_bytes());
                hasher.update(path.as_bytes());
                hasher.update(source_hash.as_bytes());
            }
            Some(hasher.finalize().into())
        })
    }

    /// The position of a call in `file.ast.calls`, for the primitive tables
    /// that are index-aligned with it.
    pub(super) fn call_index(&self, file: &FileFacts, span: Span) -> Option<usize> {
        self.ast_indexes
            .get(file.path.as_str())
            .and_then(|index| index.call_index_by_span(span))
    }

    /// The primitive resolved for the call occupying exactly `span`.
    pub(super) fn primitive_at_call(
        &self,
        file: &FileFacts,
        span: Span,
    ) -> Option<solid_dialect::Primitive> {
        let index = self.call_index(file, span)?;
        super::known_primitive(&self.primitives(file).calls[index])
    }

    /// Whether a member expression occupies exactly `span`, which distinguishes
    /// `factory(...).member()` from `factory(...)()`.
    pub(super) fn is_member_span(&self, file: &FileFacts, span: Span) -> bool {
        self.member_property_at(file, span).is_some()
    }

    /// The property span of the member expression occupying exactly `span`.
    pub(super) fn member_property_at(&self, file: &FileFacts, span: Span) -> Option<Span> {
        self.ast_indexes
            .get(file.path.as_str())
            .and_then(|index| index.member_property(span))
    }

    /// The position of a file in project facts, for the per-file memo tables.
    fn file_index(&self, file: &FileFacts) -> usize {
        *self
            .files_by_path
            .get_or_init(|| {
                self.facts
                    .files
                    .iter()
                    .enumerate()
                    .map(|(index, file)| (file.path.as_str(), index))
                    .collect()
            })
            .get(file.path.as_str())
            .expect("per-file lookup for a file outside project facts")
    }

    /// How this file's callback positions name its own functions, memoized.
    ///
    /// The answer depends only on the file, so a read-by-read derivation was
    /// re-scanning every call and JSX element in the file for every read the
    /// classifier was asked about.
    pub(super) fn named_callback_roles(&self, file: &FileFacts) -> &super::NamedCallbackRoles {
        let index = self.file_index(file);
        let slots = self
            .file_named_callback_roles
            .get_or_init(|| self.facts.files.iter().map(|_| OnceLock::new()).collect());
        slots[index].get_or_init(|| {
            super::named_callback_roles(
                &self.facts.files[index],
                self.entities,
                self.symbol_names,
                self,
            )
        })
    }

    /// The memoized primitive names for one project file.
    pub(super) fn primitives(&self, file: &FileFacts) -> &FilePrimitives {
        let index = self.file_index(file);
        let slots = self
            .file_primitives
            .get_or_init(|| self.facts.files.iter().map(|_| OnceLock::new()).collect());
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
