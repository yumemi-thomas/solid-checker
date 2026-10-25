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

    /// Returns the UTF-8 source text covered by a fact span.
    ///
    /// Fact consumers use this instead of retaining owned copies of verbatim
    /// source names. Invalid or non-character-boundary spans fail closed.
    #[must_use]
    pub fn source_text(&self, span: Span) -> Option<&str> {
        self.source.get(span.start as usize..span.end as usize)
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
