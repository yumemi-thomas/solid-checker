//! Coherent fact generation assembled from Oxc structure, Solid compiler
//! execution semantics, and TypeScript-Go checker semantics.

use serde::{Deserialize, Serialize};
use solid_ast_facts::AstFacts;
use solid_compiler_facts::ExecutionMap;
use solid_facts_core::{Generation, SourceHash, SourcePath, Span};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use typefacts::{EntityFact, FactTable, FileFact, Location, SourceDigest, Symbol, SymbolFact};

pub use solid_ast_facts;
pub use solid_compiler_facts;
pub use solid_facts_core as core;
pub use typefacts;

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
}

#[derive(Clone)]
pub struct TypeScriptTable {
    retained: Option<FactTable>,
    synthetic: Option<Arc<TypeScriptSnapshot>>,
    file_overrides: Option<Arc<[FileFact]>>,
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
        }
    }

    #[must_use]
    pub fn with_files(mut self, files: Vec<FileFact>) -> Self {
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

    #[must_use]
    pub fn file(&self, path: &str) -> Option<&FileFact> {
        if let Some(files) = &self.file_overrides {
            return files.iter().find(|file| file.path.as_ref() == path);
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
        self.retained.iter().flat_map(FactTable::entities).chain(
            self.synthetic
                .iter()
                .flat_map(|table| table.entities.iter()),
        )
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
            .map(|span| {
                Ok(Location {
                    path: self.path.shared(),
                    start_byte: u64::from(span.start),
                    end_byte: u64::from(span.end),
                })
            })
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

    #[must_use]
    pub fn structural_seed_locations(&self) -> Vec<Location> {
        self.ast
            .structural_seed_spans()
            .into_iter()
            .map(|span| Location {
                path: self.path.shared(),
                start_byte: u64::from(span.start),
                end_byte: u64::from(span.end),
            })
            .collect()
    }
}

impl ProjectFacts {
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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solid_ast_facts::extract;
    use solid_compiler_facts::COMPILER_FACTS_PROTOCOL;
    #[test]
    fn joins_one_coherent_generation() {
        let source = "const value = 1;";
        let ast = extract("src/a.ts", source).unwrap();
        let compiler = ExecutionMap {
            compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
            source_hash: SourceHash::of(source),
            tracked_regions: vec![],
            untracked_regions: vec![],
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
            tracked_regions: vec![],
            untracked_regions: vec![],
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
            tracked_regions: vec![],
            untracked_regions: vec![],
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
        assert!(std::mem::size_of::<solid_ast_facts::IdentifierFact>() <= 16);
        assert!(std::mem::size_of::<solid_ast_facts::ReturnFact>() <= 64);
        assert!(std::mem::size_of::<solid_ast_facts::BooleanPropertyFact>() <= 12);
        assert!(std::mem::size_of::<solid_ast_facts::MemberFact>() <= 24);
        assert!(std::mem::size_of::<solid_ast_facts::NamedSpan>() <= 8);
        assert!(std::mem::size_of::<solid_ast_facts::CallFact>() <= 64);
    }
}
