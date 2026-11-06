//! Coherent fact generation assembled from Oxc structure, Solid compiler
//! execution semantics, and tsrs checker semantics.

use crate::ast::AstFacts;
use crate::compiler::ExecutionMap;
use crate::core::{Generation, SourceHash, SourcePath, Span};
use crate::resolution::AttestedImportIndex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use typefacts::{EntityFact, FactTable, FileFact, Location, SourceDigest, Symbol, SymbolFact};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileFacts {
    pub generation: Generation,
    pub path: SourcePath,
    pub source_hash: SourceHash,
    pub source: Arc<str>,
    pub ast: Arc<AstFacts>,
    pub compiler: Arc<ExecutionMap>,
}

/// What `tsc` says at one place of a file, as far as its text decides
/// ([`FileFacts::tsc_silence_at`]).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TscSilence {
    /// No directive or pragma reaches it: a diagnostic there is reported.
    Reports,
    /// One compiler may suppress it, or the file may be unchecked JavaScript.
    Maybe,
    /// Both compilers suppress a diagnostic there.
    Silent,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProjectFacts {
    pub generation: Generation,
    pub project_id: String,
    pub files: Vec<FileFacts>,
    pub typescript: TypeScriptTable,
    #[serde(skip)]
    pub typescript_changes: Option<TypeScriptChanges>,
    /// Where each import specifier actually resolves, when the analysis asked
    /// the compiler.
    ///
    /// `None` is not "nothing resolved": it says this analysis carries no
    /// resolution facts at all, so a package can only be matched by
    /// specifier name — the pre-attestation behavior, kept for an adapter with
    /// no Type Facts session of its own. `Some` binds packages by installed
    /// package identity and fails closed per specifier. See
    /// [`crate::resolution`].
    #[serde(skip)]
    pub resolved_imports: Option<AttestedImportIndex>,
    /// The project bundler's own resolution of each module load, when the
    /// host asked for it (ADR 0220). See [`crate::runtime_resolution`].
    #[serde(skip)]
    pub runtime_resolutions: Option<crate::runtime_resolution::RuntimeResolutionIndex>,
    /// Exact TypeScript-symbol redirects from a declaration-bound import to
    /// the package-local runtime implementation selected for that same static
    /// specifier.
    ///
    /// Empty for ordinary project analysis. Dependency-source analysis fills
    /// it only from the resolved importer/specifier/target map, after joining
    /// both ends to compiler entities. Reactive IR then treats
    /// the runtime implementation as the alias root; missing or ambiguous
    /// joins add no redirect and preserve the existing fail-closed result.
    #[serde(skip)]
    pub runtime_symbol_redirects: HashMap<String, String>,
    /// ADR 0284, experimental: canonical directories of the installed
    /// packages whose runtime source is analyzed as project code. Their
    /// files take part in every proof, but a finding located in one is not
    /// reported, and their own runtime-configuration uses do not veto the
    /// project's findings. Empty for ordinary analysis.
    #[serde(skip)]
    pub dependency_source_roots: Vec<String>,
    /// Review of ADR 0368: in a dependency-source program, the project calls
    /// the analyzed program answers `Recovery` (a call error) and the
    /// project's own program does not. TypeScript reports no such error on
    /// the project, so the call is not TypeScript-owned: analysis keeps it as
    /// an uncertifiable obligation instead of staying silent. Empty for
    /// ordinary analysis.
    #[serde(skip)]
    pub dependency_source_call_errors: Vec<typefacts::Location>,
}

#[derive(Clone)]
pub struct TypeScriptTable {
    retained: Option<FactTable>,
    synthetic: Option<Arc<TypeScriptSnapshot>>,
    file_overrides: Option<Arc<[FileFact]>>,
    /// Per path, entities that replace the table's own (ADR 0368 review: a
    /// project call's validity taken from the project's own program).
    entity_overrides: Option<Arc<HashMap<String, Arc<[EntityFact]>>>>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TypeScriptSnapshot {
    schema: u64,
    generation: u64,
    project_id: String,
    sources: Vec<SourceDigest>,
    entities: Vec<EntityFact>,
    symbols: Vec<SymbolFact>,
    files: Vec<FileFact>,
}

#[derive(Clone, Copy)]
pub enum TypeScriptSymbol<'a> {
    Retained(Symbol<'a>),
    Synthetic(&'a SymbolFact),
}

impl<'a> TypeScriptSymbol<'a> {
    #[must_use]
    pub fn id(self) -> &'a str {
        match self {
            Self::Retained(symbol) => symbol.id(),
            Self::Synthetic(symbol) => &symbol.id,
        }
    }

    #[must_use]
    pub fn alias_target(self) -> &'a str {
        match self {
            Self::Retained(symbol) => symbol.alias_target(),
            Self::Synthetic(symbol) => &symbol.alias_target,
        }
    }

    #[must_use]
    pub fn declarations(self) -> &'a [typefacts::Declaration] {
        match self {
            Self::Retained(symbol) => symbol.declarations(),
            Self::Synthetic(symbol) => &symbol.declarations,
        }
    }

    pub fn references(self) -> impl Iterator<Item = &'a Location> {
        let retained = match self {
            Self::Retained(symbol) => Some(symbol),
            Self::Synthetic(_) => None,
        };
        let synthetic = match self {
            Self::Synthetic(symbol) => Some(symbol),
            Self::Retained(_) => None,
        };
        retained.into_iter().flat_map(Symbol::references).chain(
            synthetic
                .into_iter()
                .flat_map(|symbol| symbol.references.iter()),
        )
    }
}

impl TypeScriptTable {
    #[must_use]
    pub fn retained(table: FactTable) -> Self {
        Self {
            retained: Some(table),
            synthetic: None,
            file_overrides: None,
            entity_overrides: None,
        }
    }

    #[must_use]
    pub fn from_parts(
        schema: u64,
        generation: u64,
        project_id: impl Into<String>,
        sources: Vec<SourceDigest>,
        entities: Vec<EntityFact>,
        symbols: Vec<SymbolFact>,
        files: Vec<FileFact>,
    ) -> Self {
        Self {
            retained: None,
            synthetic: Some(Arc::new(TypeScriptSnapshot {
                schema,
                generation,
                project_id: project_id.into(),
                sources,
                entities,
                symbols,
                files,
            })),
            file_overrides: None,
            entity_overrides: None,
        }
    }

    /// This table with `path`'s entities replaced, for each path given (each
    /// a path the table holds entities for, in the table's own order).
    #[must_use]
    pub fn with_entity_overrides(mut self, overrides: HashMap<String, Vec<EntityFact>>) -> Self {
        let mut merged = self
            .entity_overrides
            .as_deref()
            .cloned()
            .unwrap_or_default();
        merged.extend(
            overrides
                .into_iter()
                .map(|(path, entities)| (path, Arc::from(entities))),
        );
        self.entity_overrides = (!merged.is_empty()).then(|| Arc::new(merged));
        self
    }

    #[must_use]
    pub fn with_files(mut self, mut files: Vec<FileFact>) -> Self {
        // Sorted by path, as the retained table already yields them, so a
        // lookup is a binary search rather than a scan of every file.
        files.sort_by(|left, right| left.path.cmp(&right.path));
        self.file_overrides = Some(files.into());
        self
    }

    #[must_use]
    pub fn schema(&self) -> u64 {
        self.retained
            .as_ref()
            .map_or_else(|| self.synthetic().schema, FactTable::schema)
    }

    #[must_use]
    pub fn generation(&self) -> u64 {
        self.retained
            .as_ref()
            .map_or_else(|| self.synthetic().generation, FactTable::generation)
    }

    /// Whether the program the table was analyzed under applies
    /// `strictNullChecks`, as the producer's compiler resolves it (ADR 0465).
    /// `None` is no answer (a synthetic table, a producer that did not say).
    #[must_use]
    pub fn strict_null_checks(&self) -> Option<bool> {
        self.retained
            .as_ref()
            .and_then(FactTable::strict_null_checks)
    }

    #[must_use]
    pub fn project_id(&self) -> &str {
        self.retained.as_ref().map_or_else(
            || self.synthetic().project_id.as_str(),
            FactTable::project_id,
        )
    }

    #[must_use]
    pub fn source(&self, path: &str) -> Option<&SourceDigest> {
        self.retained.as_ref().map_or_else(
            || {
                self.synthetic()
                    .sources
                    .iter()
                    .find(|source| source.path.as_ref() == path)
            },
            |table| table.source(path),
        )
    }

    #[must_use]
    pub fn entities_for_path(&self, path: &str) -> &[EntityFact] {
        if let Some(entities) = self
            .entity_overrides
            .as_deref()
            .and_then(|overrides| overrides.get(path))
        {
            return entities;
        }
        self.retained.as_ref().map_or_else(
            || {
                let entities = &self.synthetic().entities;
                let start = entities.partition_point(|entity| entity.location.path.as_ref() < path);
                let end = entities.partition_point(|entity| entity.location.path.as_ref() <= path);
                &entities[start..end]
            },
            |table| table.entities_for_path(path),
        )
    }

    /// [`Self::entities_for_path`] as a shared slice: the table's own (a
    /// retained table hands out the rows it holds, no copy), an override's, or
    /// a copy of a synthetic table's. A holder keeps it across tables, and
    /// replaces it for each path a table delta names.
    #[must_use]
    pub fn entities_shared_for_path(&self, path: &str) -> Arc<[EntityFact]> {
        if let Some(entities) = self
            .entity_overrides
            .as_deref()
            .and_then(|overrides| overrides.get(path))
        {
            return Arc::clone(entities);
        }
        match &self.retained {
            Some(table) => table
                .entities_shared_for_path(path)
                .map_or_else(|| Arc::from([]), Arc::clone),
            None => Arc::from(self.entities_for_path(path)),
        }
    }

    #[must_use]
    pub fn file(&self, path: &str) -> Option<&FileFact> {
        if let Some(files) = &self.file_overrides {
            let index = files.partition_point(|file| file.path.as_ref() < path);
            return files.get(index).filter(|file| file.path.as_ref() == path);
        }
        self.retained.as_ref().map_or_else(
            || {
                self.synthetic()
                    .files
                    .iter()
                    .find(|file| file.path.as_ref() == path)
            },
            |table| table.file(path),
        )
    }

    pub fn sources(&self) -> impl Iterator<Item = &SourceDigest> {
        self.retained
            .iter()
            .flat_map(FactTable::sources)
            .chain(self.synthetic.iter().flat_map(|table| table.sources.iter()))
    }

    pub fn entities(&self) -> impl Iterator<Item = &EntityFact> {
        // Path order is kept: a replaced path's entities take the place of
        // the table's own, which stay sorted by path.
        let overrides = self.entity_overrides.as_deref();
        let mut replaced: Option<&str> = None;
        self.retained
            .iter()
            .flat_map(FactTable::entities)
            .chain(
                self.synthetic
                    .iter()
                    .flat_map(|table| table.entities.iter()),
            )
            .flat_map(move |entity| {
                let path = entity.location.path.as_ref();
                let Some(entities) = overrides.and_then(|overrides| overrides.get(path)) else {
                    return std::slice::from_ref(entity).iter();
                };
                if replaced == Some(path) {
                    return [].iter();
                }
                replaced = Some(path);
                entities.iter()
            })
    }

    pub fn files(&self) -> impl Iterator<Item = &FileFact> {
        self.file_overrides
            .iter()
            .flat_map(|files| files.iter())
            .chain(
                self.retained
                    .iter()
                    .filter(|_| self.file_overrides.is_none())
                    .flat_map(FactTable::files),
            )
            .chain(
                self.synthetic
                    .iter()
                    .filter(|_| self.file_overrides.is_none())
                    .flat_map(|table| table.files.iter()),
            )
    }

    pub fn symbols(&self) -> impl Iterator<Item = TypeScriptSymbol<'_>> {
        self.retained
            .iter()
            .flat_map(FactTable::symbols)
            .map(TypeScriptSymbol::Retained)
            .chain(
                self.synthetic
                    .iter()
                    .flat_map(|table| table.symbols.iter())
                    .map(TypeScriptSymbol::Synthetic),
            )
    }

    /// The ids of the symbols whose alias target or declarations differ
    /// between `previous` and this table, added and removed ones included:
    /// for two retained tables, from the index leaves they do not share
    /// ([`FactTable::symbols_with_moved_declarations`]); otherwise by
    /// comparing every symbol of both.
    #[must_use]
    pub fn symbols_with_moved_declarations(&self, previous: &Self) -> Vec<Arc<str>> {
        if let (Some(current), Some(previous), None, None) = (
            &self.retained,
            &previous.retained,
            &self.synthetic,
            &previous.synthetic,
        ) {
            return current.symbols_with_moved_declarations(previous);
        }
        type Rows<'t> = HashMap<&'t str, (&'t str, &'t [typefacts::Declaration])>;
        fn rows(table: &TypeScriptTable) -> Rows<'_> {
            let mut rows = Rows::new();
            for symbol in table.symbols() {
                // The first row of an id is the one `symbol` answers.
                rows.entry(symbol.id())
                    .or_insert((symbol.alias_target(), symbol.declarations()));
            }
            rows
        }
        let (current, before) = (rows(self), rows(previous));
        let mut moved = current
            .iter()
            .filter(|(id, row)| before.get(*id) != Some(*row))
            .map(|(id, _)| *id)
            .chain(
                before
                    .keys()
                    .filter(|id| !current.contains_key(*id))
                    .copied(),
            )
            .map(Arc::<str>::from)
            .collect::<Vec<_>>();
        moved.sort_unstable();
        moved
    }

    #[must_use]
    pub fn symbol(&self, id: &str) -> Option<TypeScriptSymbol<'_>> {
        self.retained
            .as_ref()
            .and_then(|table| table.symbol(id))
            .map(TypeScriptSymbol::Retained)
            .or_else(|| {
                self.synthetic
                    .iter()
                    .flat_map(|table| table.symbols.iter())
                    .find(|symbol| symbol.id.as_ref() == id)
                    .map(TypeScriptSymbol::Synthetic)
            })
    }

    fn synthetic(&self) -> &TypeScriptSnapshot {
        self.synthetic
            .as_deref()
            .expect("TypeScript table must be retained or synthetic")
    }
}

impl From<FactTable> for TypeScriptTable {
    fn from(table: FactTable) -> Self {
        Self::retained(table)
    }
}

impl std::fmt::Debug for TypeScriptTable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TypeScriptTable")
            .field("schema", &self.schema())
            .field("generation", &self.generation())
            .field("project_id", &self.project_id())
            .field("sources", &self.sources().count())
            .field("entities", &self.entities().count())
            .field("symbols", &self.symbols().count())
            .field("files", &self.files().count())
            .finish()
    }
}

impl Serialize for TypeScriptTable {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct SerializableSymbol<'a> {
            id: &'a str,
            #[serde(rename = "aliasTarget")]
            alias_target: &'a str,
            declarations: &'a [typefacts::Declaration],
            references: Vec<&'a Location>,
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct SerializableTable<'a> {
            schema: u64,
            generation: u64,
            project_id: &'a str,
            sources: Vec<&'a SourceDigest>,
            entities: Vec<&'a EntityFact>,
            symbols: Vec<SerializableSymbol<'a>>,
            files: Vec<&'a FileFact>,
        }

        SerializableTable {
            schema: self.schema(),
            generation: self.generation(),
            project_id: self.project_id(),
            sources: self.sources().collect(),
            entities: self.entities().collect(),
            symbols: self
                .symbols()
                .map(|symbol| SerializableSymbol {
                    id: symbol.id(),
                    alias_target: symbol.alias_target(),
                    declarations: symbol.declarations(),
                    references: symbol.references().collect(),
                })
                .collect(),
            files: self.files().collect(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TypeScriptTable {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self {
            retained: None,
            synthetic: Some(Arc::new(TypeScriptSnapshot::deserialize(deserializer)?)),
            file_overrides: None,
            entity_overrides: None,
        })
    }
}

/// Process-local description of how the retained TypeFacts table changed.
/// It is not part of the TypeFacts wire protocol; the sidecar adapter derives
/// it from the already-validated full/reuse/delta response.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeScriptChanges {
    pub unchanged: bool,
    /// A whole-table replacement: the path lists name only the new table's
    /// paths, so they are no delta (see `typefacts::session::TableChanges`).
    pub full: bool,
    pub entity_paths: Vec<String>,
    pub symbol_ids: Vec<String>,
    pub file_paths: Vec<String>,
}

#[derive(Debug, Error)]
pub enum JoinError {
    #[error("AST and compiler source hashes differ")]
    CompilerSourceHash,
    #[error("TypeFacts project identity does not match")]
    ProjectIdentity,
    #[error("TypeFacts generation does not match")]
    Generation,
    #[error("TypeFacts source is missing for {0}")]
    MissingTypeScriptSource(String),
    #[error("TypeFacts source hash differs for {0}")]
    TypeScriptSourceHash(String),
    #[error("compiler seed span cannot be represented by TypeFacts: {0:?}")]
    SpanWidth(Span),
}

/// An object whose members the standard library's calls read (ADR 0781).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StandardRoot {
    /// `globalThis`, `window`, `self`, `global`.
    Global,
    /// `Name.prototype`.
    Prototype(String),
    /// `Name`, bound nowhere in the file.
    Constructor(String),
}

impl FileFacts {
    pub fn new(
        generation: Generation,
        source: impl Into<Arc<str>>,
        ast: impl Into<Arc<AstFacts>>,
        compiler: impl Into<Arc<ExecutionMap>>,
    ) -> Result<Self, JoinError> {
        let ast = ast.into();
        let compiler = compiler.into();
        if ast.source.hash != compiler.source_hash {
            return Err(JoinError::CompilerSourceHash);
        }
        Ok(Self {
            generation,
            path: ast.source.path.clone(),
            source_hash: ast.source.hash.clone(),
            source: source.into(),
            ast,
            compiler,
        })
    }

    pub fn compiler_seed_locations(&self) -> Result<Vec<Location>, JoinError> {
        self.compiler
            .seed_spans()
            .into_iter()
            .map(|span| Ok(span.location(self.path.shared())))
            .collect()
    }

    /// Whether `tsc` reports a diagnostic that starts at byte `offset` of this
    /// file, as far as the file's own text decides (ADR 0830).
    ///
    /// The walk is tsgo's (`program.rs` `get_diagnostics_with_preceding_directives`,
    /// tsc 5's `markPrecedingCommentDirectiveLine`): from the line above the
    /// diagnostic upwards, a directive line suppresses it, and a line that is
    /// neither blank nor begins (past spaces and tabs) with `//` ends the walk.
    /// `Silent` only where both compilers agree: an exact directive, or a
    /// TypeScript file with a canonical leading `// @ts-nocheck`. A JavaScript
    /// file is `Maybe` (whether `checkJs` applies is not in the facts), and so
    /// is a line only one compiler reads as suppressed.
    #[must_use]
    pub fn tsc_silence_at(&self, offset: u32) -> TscSilence {
        if self.ast.typescript_check_pragma == Some(false) && !self.javascript() {
            return TscSilence::Silent;
        }
        if self.javascript() || self.ast.typescript_nocheck_comment {
            return TscSilence::Maybe;
        }
        if self.ast.typescript_directives.is_empty() {
            return TscSilence::Reports;
        }
        let lines = crate::ast::LineTable::new(&self.source);
        self.silence_of_line(&lines, lines.line(offset).saturating_sub(1))
    }

    /// Whether `tsc` may say nothing about some line of `span`: any line of it
    /// is not [`TscSilence::Reports`]. The premise an uncertifiable result may
    /// use where a diagnostic's exact line is not known (ADR 0830).
    #[must_use]
    pub fn tsc_may_be_silent_within(&self, span: Span) -> bool {
        if self.javascript() || self.ast.typescript_nocheck_comment {
            return true;
        }
        if self.ast.typescript_directives.is_empty() {
            return false;
        }
        let lines = crate::ast::LineTable::new(&self.source);
        let first = lines.line(span.start).saturating_sub(1);
        let last = lines
            .line(span.end.saturating_sub(1).max(span.start))
            .saturating_sub(1);
        (first..=last).any(|line| self.silence_of_line(&lines, line) != TscSilence::Reports)
    }

    /// The zero-based line byte `offset` falls on, by ECMAScript line
    /// terminators.
    #[must_use]
    pub fn line_of(&self, offset: u32) -> u32 {
        crate::ast::LineTable::new(&self.source)
            .line(offset)
            .saturating_sub(1)
    }

    /// Whether any place of the file may be one `tsc` says nothing about: it
    /// is JavaScript, or holds a directive or a `@ts-nocheck` pragma.
    #[must_use]
    pub fn tsc_may_be_silent_somewhere(&self) -> bool {
        self.javascript()
            || self.ast.typescript_nocheck_comment
            || !self.ast.typescript_directives.is_empty()
    }

    fn javascript(&self) -> bool {
        [".js", ".jsx", ".mjs", ".cjs"]
            .iter()
            .any(|extension| self.path.as_str().ends_with(extension))
    }

    fn silence_of_line(&self, lines: &crate::ast::LineTable, line: u32) -> TscSilence {
        let directives = &self.ast.typescript_directives;
        let mut answer = TscSilence::Reports;
        let mut current = line;
        while let Some(above) = current.checked_sub(1) {
            current = above;
            for directive in directives {
                if directive.exact && directive.first_line == above {
                    return if answer == TscSilence::Maybe {
                        TscSilence::Maybe
                    } else {
                        TscSilence::Silent
                    };
                }
                if directive.first_line == above || directive.last_line == above {
                    answer = TscSilence::Maybe;
                }
            }
            let Some(text) = lines.text_of(&self.source, above) else {
                break;
            };
            // tsgo: spaces and tabs, then the end, a line break or `//`.
            // tsc 5: the whole line trimmed of any whitespace, empty or `//`.
            let go = text.trim_start_matches([' ', '\t']);
            let go = go.is_empty() || go.starts_with(['\r', '\n']) || go.starts_with("//");
            let tsc = text.trim();
            let tsc = tsc.is_empty() || tsc.starts_with("//");
            match (go, tsc) {
                (true, true) => {}
                (false, false) => break,
                _ => answer = TscSilence::Maybe,
            }
        }
        answer
    }

    /// Returns the UTF-8 source text covered by a fact span.
    ///
    /// Fact consumers use this instead of retaining owned copies of verbatim
    /// source names. Invalid or non-character-boundary spans fail closed.
    #[must_use]
    pub fn source_text(&self, span: Span) -> Option<&str> {
        self.source.get(span.start as usize..span.end as usize)
    }

    /// The expression inside any parentheses around `span`; `None` when a type
    /// assertion (`as`, `<T>`, `!`, `satisfies`) wraps it on the way. Unlike
    /// [`AstFacts::peel_ts_sugar_span`], which peels both, this is the operand
    /// TypeScript itself sees through (`skipParentheses`).
    #[must_use]
    pub fn peel_parentheses(&self, span: Span) -> Option<Span> {
        let mut value = span;
        while let Ok(index) = self
            .ast
            .transparent_wrappers
            .binary_search_by_key(&value, |wrapper| wrapper.span)
        {
            let wrapper = &self.ast.transparent_wrappers[index];
            if wrapper.inner == value {
                break;
            }
            let parenthesized = wrapper.inner.start > value.start
                && self.source.as_bytes().get(value.start as usize) == Some(&b'(');
            if !parenthesized {
                return None;
            }
            value = wrapper.inner;
        }
        Some(value)
    }

    /// The value under a type assertion (`as`, `<T>`) written on `span`, past
    /// every other wrapper; `None` when none is. `satisfies` and `!` assert
    /// nothing about what the value is.
    #[must_use]
    pub fn asserted_value(&self, span: Span) -> Option<Span> {
        let mut value = span;
        let mut asserted = false;
        while let Ok(index) = self
            .ast
            .transparent_wrappers
            .binary_search_by_key(&value, |wrapper| wrapper.span)
        {
            let wrapper = &self.ast.transparent_wrappers[index];
            if wrapper.inner == value {
                break;
            }
            let after = self
                .source_text(Span {
                    start: wrapper.inner.end,
                    end: value.end,
                })
                .unwrap_or_default()
                .trim_start();
            asserted |= after
                .strip_prefix("as")
                .is_some_and(|rest| rest.starts_with(|c: char| c.is_whitespace()))
                || self.source.as_bytes().get(value.start as usize) == Some(&b'<');
            value = wrapper.inner;
        }
        asserted.then_some(value)
    }

    /// The asserted value a call's written type rests on (ADR 0781): an
    /// assertion on the callee, on a member callee's receiver, or in the
    /// initializer of a binding that holds either.
    #[must_use]
    pub fn asserted_call_value(&self, callee: Span) -> Option<Span> {
        if let Some(value) = self.asserted_value(callee) {
            return Some(value);
        }
        let peeled = self.ast.peel_ts_sugar_span(callee);
        let mut value = self
            .ast
            .member_at(peeled)
            .map_or(peeled, |member| member.object);
        // A chain of aliases ends where a binding holds no identifier, and an
        // alias of itself is a program `tsc` rejects, so the bound is no limit
        // on any real program.
        for _ in 0..16 {
            if let Some(inner) = self.asserted_value(value) {
                return Some(inner);
            }
            let declaration = self
                .ast
                .reference_declaration(self.ast.peel_ts_sugar_span(value))?;
            value = self.ast.identifier_binding(declaration)?.initializer?;
        }
        None
    }

    /// What the value at `span` is when it is an object the standard library's
    /// calls read (ADR 0781), through assertions and bindings that hold it.
    #[must_use]
    pub fn standard_root(&self, span: Span) -> Option<StandardRoot> {
        const GLOBAL_ROOTS: [&str; 4] = ["globalThis", "window", "self", "global"];
        let ast = &self.ast;
        let mut value = ast.peel_ts_sugar_span(span);
        for _ in 0..16 {
            let Some(declaration) = ast.reference_declaration(value) else {
                break;
            };
            value = ast
                .identifier_binding(declaration)?
                .initializer
                .map(|initializer| ast.peel_ts_sugar_span(initializer))?;
        }
        let mut root = value;
        while let Some(member) = ast.member_at(root) {
            root = ast.peel_ts_sugar_span(member.object);
        }
        if ast.reference_declaration(root).is_some() {
            return None;
        }
        let text = self.source_text(value)?.trim();
        let text = GLOBAL_ROOTS
            .iter()
            .find_map(|name| text.strip_prefix(name)?.strip_prefix('.'))
            .unwrap_or(text);
        if GLOBAL_ROOTS.contains(&text) {
            return Some(StandardRoot::Global);
        }
        let plain = |name: &str| {
            name != "this"
                && name
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        };
        match text.split_once('.') {
            Some((name, "prototype")) if plain(name) => {
                Some(StandardRoot::Prototype(name.to_owned()))
            }
            None if plain(text) => Some(StandardRoot::Constructor(text.to_owned())),
            _ => None,
        }
    }

    /// The spans of this file's calls whose callee, past transparent
    /// TypeScript wrappers, is spelled `eval`. Computed once per file.
    #[must_use]
    pub fn eval_call_spans(&self) -> &[Span] {
        self.ast.span_index().eval_calls_with(|| {
            self.ast
                .calls
                .iter()
                .filter(|call| {
                    self.source_text(self.ast.peel_ts_sugar_span(call.callee)) == Some("eval")
                })
                .map(|call| call.span)
                .collect()
        })
    }

    #[must_use]
    pub fn structural_seed_locations(&self) -> Vec<Location> {
        self.ast
            .structural_seed_spans()
            .into_iter()
            .map(|span| span.location(self.path.shared()))
            .collect()
    }
}

impl ProjectFacts {
    /// ADR 0284: the package directory, among
    /// [`Self::dependency_source_roots`], that holds `path`, compared by path
    /// component so `/x/router-extra` is not inside `/x/router`.
    #[must_use]
    pub fn dependency_source_root(&self, path: &str) -> Option<&str> {
        self.dependency_source_roots
            .iter()
            .find(|root| std::path::Path::new(path).starts_with(root.as_str()))
            .map(String::as_str)
    }

    pub fn join(
        generation: Generation,
        project_id: impl Into<String>,
        mut files: Vec<FileFacts>,
        typescript: impl Into<TypeScriptTable>,
    ) -> Result<Self, JoinError> {
        let project_id = project_id.into();
        let typescript = typescript.into();
        if typescript.project_id() != project_id {
            return Err(JoinError::ProjectIdentity);
        }
        if typescript.generation() != generation.get() {
            return Err(JoinError::Generation);
        }
        files.sort_by(|left, right| left.path.cmp(&right.path));
        let source_hashes = typescript
            .sources()
            .map(|digest| (digest.path.replace('\\', "/"), &digest.sha256))
            .collect::<HashMap<_, _>>();
        for file in &files {
            let Some(source_hash) = source_hashes.get(file.path.as_str()) else {
                return Err(JoinError::MissingTypeScriptSource(file.path.to_string()));
            };
            // The producer and the checker each own a `SourceHash` newtype, so
            // identity is compared on the canonical `sha256:` text they share.
            if source_hash.as_str() != file.source_hash.as_str() {
                return Err(JoinError::TypeScriptSourceHash(file.path.to_string()));
            }
        }
        Ok(Self {
            generation,
            project_id,
            files,
            typescript,
            typescript_changes: None,
            resolved_imports: None,
            runtime_resolutions: None,
            runtime_symbol_redirects: HashMap::new(),
            dependency_source_roots: Vec::new(),
            dependency_source_call_errors: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::extract;
    use crate::compiler::COMPILER_FACTS_PROTOCOL;

    fn entity(path: &str, start: u64, symbol: &str) -> EntityFact {
        EntityFact {
            location: Location {
                path: path.into(),
                start_byte: start,
                end_byte: start + 1,
            },
            symbol: symbol.into(),
            symbol_claim: typefacts::SymbolClaim::Covering,
            type_descriptor: None,
            resolved_call: None,
            callability: None,
            constructability: None,
            runtime_binding_kind: None,
            runtime_value_domain: None,
            primitive_value_domain: typefacts::PrimitiveValueDomain::default(),
            primitive_literal_candidates: None,
            call_result_domain: None,
            constant_value: None,
            array_shape: None,
            tuple_shape: None,
            library_types: None,
            declared_keys: None,
            writable_optional_property: false,
            delete_operand_rejected: false,
            reference_space: None,
            runtime_identity: "".into(),
        }
    }

    /// ADR 0368 review: a replaced path's entities take its place, in path
    /// order (per-path containment slices need the table sorted by path).
    #[test]
    fn entity_overrides_keep_path_order() {
        let table = TypeScriptTable::from_parts(
            1,
            1,
            "p",
            Vec::new(),
            vec![
                entity("a.ts", 0, "a"),
                entity("b.ts", 0, "b0"),
                entity("b.ts", 2, "b2"),
                entity("c.ts", 0, "c"),
            ],
            Vec::new(),
            Vec::new(),
        )
        .with_entity_overrides(HashMap::from([(
            "b.ts".to_owned(),
            vec![entity("b.ts", 0, "B0"), entity("b.ts", 2, "B2")],
        )]));
        let symbols = table
            .entities()
            .map(|entity| entity.symbol.to_string())
            .collect::<Vec<_>>();
        assert_eq!(symbols, ["a", "B0", "B2", "c"]);
        assert_eq!(table.entities_for_path("b.ts")[1].symbol.as_ref(), "B2");
        assert_eq!(table.entities_for_path("c.ts")[0].symbol.as_ref(), "c");
    }
    #[test]
    fn joins_one_coherent_generation() {
        let source = "const value = 1;";
        let ast = extract("src/a.ts", source).unwrap();
        let compiler = ExecutionMap {
            compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
            source_hash: SourceHash::of(source),
            semantic_model: Default::default(),
            tracked_regions: vec![],
            untracked_regions: vec![],
            discarded_regions: vec![],
            ownership_regions: vec![],
            callback_roles: vec![],
            jsx_operations: vec![],
        };
        let generation = Generation::new(1).unwrap();
        let file = FileFacts::new(generation, source, ast, compiler).unwrap();
        let table = TypeScriptTable::from_parts(
            2,
            1,
            "project",
            vec![SourceDigest {
                path: "src/a.ts".into(),
                sha256: typefacts::SourceHash::of(source),
            }],
            vec![],
            vec![],
            vec![],
        );
        let joined = ProjectFacts::join(generation, "project", vec![file], table).unwrap();
        assert_eq!(joined.files.len(), 1);
    }

    fn facts_of(path: &str, source: &str) -> FileFacts {
        let ast = extract(path, source).unwrap();
        let compiler = ExecutionMap {
            compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
            source_hash: SourceHash::of(source),
            semantic_model: Default::default(),
            tracked_regions: vec![],
            untracked_regions: vec![],
            discarded_regions: vec![],
            ownership_regions: vec![],
            callback_roles: vec![],
            jsx_operations: vec![],
        };
        FileFacts::new(Generation::new(1).unwrap(), source, ast, compiler).unwrap()
    }

    /// ADR 0830: what tsc says at the start of the line holding `marker`.
    fn silence(path: &str, source: &str, marker: &str) -> TscSilence {
        let offset = u32::try_from(source.find(marker).unwrap()).unwrap();
        facts_of(path, source).tsc_silence_at(offset)
    }

    #[test]
    fn a_directive_silences_the_line_below_through_blank_and_comment_lines() {
        use TscSilence::*;
        let exact = "// @ts-ignore\nX;\n";
        assert_eq!(silence("a.ts", exact, "X"), Silent);
        assert_eq!(
            silence("a.ts", "/// @ts-expect-error\n\n  // note\nX;\n", "X"),
            Silent
        );
        assert_eq!(silence("a.ts", "f(); // @ts-ignore\nX;\n", "X"), Silent);
        assert_eq!(silence("a.ts", "/* @ts-ignore */\nX;\n", "X"), Silent);
        // Code between the directive and the line ends the walk.
        assert_eq!(silence("a.ts", "// @ts-ignore\nf();\nX;\n", "X"), Reports);
        // The directive's own line, and a line above it, are not covered.
        assert_eq!(silence("a.ts", "X; // @ts-ignore\n", "X"), Reports);
        assert_eq!(silence("a.ts", "X;\n// @ts-ignore\nf();\n", "X"), Reports);
        assert_eq!(silence("a.ts", "f();\nX;\n", "X"), Reports);
        // A string holding the words is no comment.
        assert_eq!(
            silence("a.ts", "const s = `\n// @ts-ignore\n`; X;\n", "X"),
            Reports
        );
    }

    #[test]
    fn only_a_reading_both_compilers_share_is_silent() {
        use TscSilence::*;
        // tsc 5 files a two-line block under its last line, tsgo under its first.
        assert_eq!(silence("a.ts", "/* @ts-ignore\n */\nX;\n", "X"), Maybe);
        // tsc 5 takes one extra slash; tsgo any number.
        assert_eq!(silence("a.ts", "//// @ts-ignore\nX;\n", "X"), Maybe);
        // tsc 5 trims a no-break space; tsgo stops there.
        assert_eq!(silence("a.ts", "// @ts-ignore\n\u{a0}\nX;\n", "X"), Maybe);
        // JavaScript: whether checkJs applies is not in the facts.
        assert_eq!(silence("a.js", "X;\n", "X"), Maybe);
    }

    #[test]
    fn a_leading_nocheck_pragma_silences_the_file_and_a_late_one_may() {
        use TscSilence::*;
        assert_eq!(silence("a.ts", "// @ts-nocheck\nf();\nX;\n", "X"), Silent);
        assert_eq!(
            silence("a.ts", "/* x */\n// @ts-nocheck\nf();\nX;\n", "X"),
            Silent
        );
        assert_eq!(
            silence("a.ts", "f();\n// @ts-nocheck\nf();\nX;\n", "X"),
            Maybe
        );
        assert_eq!(
            silence("a.ts", "// @ts-nocheck\n// @ts-check\nX;\n", "X"),
            Maybe
        );
        let file = facts_of("a.ts", "// @ts-ignore\nf();\nX;\nY;\n");
        let x = u32::try_from("// @ts-ignore\nf();\n".len()).unwrap();
        assert!(!file.tsc_may_be_silent_within(Span::new(x, x + 5)));
        assert!(file.tsc_may_be_silent_within(Span::new(x - 5, x + 5)));
        assert!(file.tsc_may_be_silent_somewhere());
    }

    #[test]
    fn file_facts_share_the_callers_source_buffer() {
        let source: Arc<str> = Arc::from("const value = 1;");
        let ast = extract("src/a.ts", source.as_ref()).unwrap();
        let compiler = ExecutionMap {
            compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
            source_hash: SourceHash::of(source.as_ref()),
            semantic_model: Default::default(),
            tracked_regions: vec![],
            untracked_regions: vec![],
            discarded_regions: vec![],
            ownership_regions: vec![],
            callback_roles: vec![],
            jsx_operations: vec![],
        };
        let file = FileFacts::new(
            Generation::new(1).unwrap(),
            Arc::clone(&source),
            ast,
            compiler,
        )
        .unwrap();

        assert!(Arc::ptr_eq(&source, &file.source));
    }

    #[test]
    fn resolves_fact_text_without_retaining_an_owned_name() {
        let source = "const café = 1; café;";
        let ast = extract("src/a.ts", source).unwrap();
        let compiler = ExecutionMap {
            compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
            source_hash: SourceHash::of(source),
            semantic_model: Default::default(),
            tracked_regions: vec![],
            untracked_regions: vec![],
            discarded_regions: vec![],
            ownership_regions: vec![],
            callback_roles: vec![],
            jsx_operations: vec![],
        };
        let generation = Generation::new(1).unwrap();
        let file = FileFacts::new(generation, source, ast, compiler).unwrap();

        let names = file
            .ast
            .identifiers
            .iter()
            .filter_map(|identifier| file.source_text(identifier.span))
            .collect::<Vec<_>>();
        assert_eq!(names, ["café", "café"]);
        assert_eq!(file.source_text(Span::new(9, 10)), None);
        assert_eq!(file.source_text(Span::new(0, u32::MAX)), None);
    }

    #[test]
    fn identifier_facts_remain_compact() {
        assert!(std::mem::size_of::<crate::ast::IdentifierFact>() <= 16);
        assert!(std::mem::size_of::<crate::ast::ReturnFact>() <= 64);
        assert!(std::mem::size_of::<crate::ast::BooleanPropertyFact>() <= 12);
        assert!(std::mem::size_of::<crate::ast::MemberFact>() <= 24);
        assert!(std::mem::size_of::<crate::ast::NamedSpan>() <= 8);
        assert!(std::mem::size_of::<crate::ast::CallFact>() <= 64);
    }
}
