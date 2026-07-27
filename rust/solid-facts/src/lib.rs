//! Coherent fact generation assembled from Oxc structure, Solid compiler
//! execution semantics, and TypeScript-Go checker semantics.

use serde::{Deserialize, Serialize};
use solid_ast_facts::AstFacts;
use solid_compiler_facts::ExecutionMap;
use solid_facts_core::{Generation, SourceHash, SourcePath, Span};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use typefacts::{FactTable, Location};

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
    pub source: String,
    pub ast: Arc<AstFacts>,
    pub compiler: Arc<ExecutionMap>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectFacts {
    pub generation: Generation,
    pub project_id: String,
    pub files: Vec<FileFacts>,
    pub typescript: FactTable,
    #[serde(skip)]
    pub typescript_changes: Option<TypeScriptChanges>,
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
        source: impl Into<String>,
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
                    path: self.path.to_string().into(),
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
                path: self.path.to_string().into(),
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
        typescript: FactTable,
    ) -> Result<Self, JoinError> {
        let project_id = project_id.into();
        if typescript.project_id != project_id {
            return Err(JoinError::ProjectIdentity);
        }
        if typescript.generation != generation.get() {
            return Err(JoinError::Generation);
        }
        files.sort_by(|left, right| left.path.cmp(&right.path));
        let source_hashes = typescript
            .sources
            .iter()
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
    use typefacts::SourceDigest;

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
        let table = FactTable {
            schema: 2,
            generation: 1,
            project_id: "project".into(),
            sources: vec![SourceDigest {
                path: "src/a.ts".into(),
                sha256: typefacts::SourceHash::of(source),
            }]
            .into(),
            entities: vec![].into(),
            symbols: vec![].into(),
            files: vec![].into(),
        };
        let joined = ProjectFacts::join(generation, "project", vec![file], table).unwrap();
        assert_eq!(joined.files.len(), 1);
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
