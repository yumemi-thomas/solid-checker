//! The producer-side fact table (Go: `facttable.go`, the `FactTable` of
//! `model.go`). It is the exact input to a transition; the client keeps its own
//! retained [`crate::FactTable`].

use std::collections::HashMap;
use std::sync::Arc;

use sha2::{Digest, Sha256};

use super::TransportChanges;
use crate::v3::TYPE_FACTS_SCHEMA_V1;
use crate::{EntityFact, FileFact, SourceDigest, SourceHash, SymbolFact};

/// Entity rows, either flat or already grouped into one run per file.
///
/// Both orders are canonical: ascending by path, then by location within a
/// path, with a path's rows contiguous. `Runs` lets a producer lend its
/// per-file contribution rows without copying them into one table-wide vector
/// (Go: `entityRuns`); the two encode to the same bytes.
#[derive(Clone, Debug)]
pub enum EntityRows {
    Flat(Vec<EntityFact>),
    /// One non-empty run per path, ascending by path.
    Runs(Vec<Arc<[EntityFact]>>),
}

impl Default for EntityRows {
    fn default() -> Self {
        Self::Flat(Vec::new())
    }
}

impl EntityRows {
    /// Groups a flat, canonically ordered row list into per-path runs.
    #[must_use]
    pub fn into_runs(self) -> Self {
        let Self::Flat(entities) = self else {
            return self;
        };
        let mut runs: Vec<Arc<[EntityFact]>> = Vec::new();
        let mut start = 0;
        while start < entities.len() {
            let end = path_end(&entities, start);
            runs.push(entities[start..end].to_vec().into());
            start = end;
        }
        Self::Runs(runs)
    }

    /// Total entity rows.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Flat(entities) => entities.len(),
            Self::Runs(runs) => runs.iter().map(|run| run.len()).sum(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Visits every row exactly as the wire layer does: the per-file runs when
    /// the rows are grouped, otherwise the flat list (Go: `rangeWireEntities`).
    pub fn for_each(&self, mut visit: impl FnMut(&EntityFact)) {
        match self {
            Self::Flat(entities) => entities.iter().for_each(&mut visit),
            Self::Runs(runs) => runs.iter().flat_map(|run| run.iter()).for_each(&mut visit),
        }
    }

    /// The cursor space of the path walk: entity indexes when flat, run
    /// indexes when grouped.
    pub(super) fn cursor_len(&self) -> usize {
        match self {
            Self::Flat(entities) => entities.len(),
            Self::Runs(runs) => runs.len(),
        }
    }

    pub(super) fn cursor_path(&self, index: usize) -> &str {
        match self {
            Self::Flat(entities) => &entities[index].location.path,
            Self::Runs(runs) => &runs[index][0].location.path,
        }
    }

    pub(super) fn cursor_run(&self, index: usize) -> &[EntityFact] {
        match self {
            Self::Flat(entities) => &entities[index..path_end(entities, index)],
            Self::Runs(runs) => &runs[index],
        }
    }

    pub(super) fn cursor_next(&self, index: usize) -> usize {
        match self {
            Self::Flat(entities) => path_end(entities, index),
            Self::Runs(_) => index + 1,
        }
    }

    /// The rows of one path, empty when it has none.
    pub(super) fn path(&self, path: &str) -> &[EntityFact] {
        match self {
            Self::Flat(entities) => {
                let start = entities.partition_point(|entity| entity.location.path.as_ref() < path);
                if entities
                    .get(start)
                    .is_none_or(|entity| entity.location.path.as_ref() != path)
                {
                    return &[];
                }
                &entities[start..path_end(entities, start)]
            }
            Self::Runs(runs) => {
                let index = runs.partition_point(|run| run[0].location.path.as_ref() < path);
                match runs.get(index) {
                    Some(run) if run[0].location.path.as_ref() == path => run,
                    _ => &[],
                }
            }
        }
    }
}

fn path_end(entities: &[EntityFact], start: usize) -> usize {
    let path = &entities[start].location.path;
    let mut end = start + 1;
    while end < entities.len() && entities[end].location.path == *path {
        end += 1;
    }
    end
}

/// Everything one transition needs to know about a table. Rows are canonical:
/// `sources`, `files` and `symbols` ascend by path or id, and entity rows are
/// ordered as [`EntityRows`] says.
#[derive(Clone, Debug)]
pub struct ProducerTable {
    /// The producer-internal schema, [`TYPE_FACTS_SCHEMA_V1`]; not the wire
    /// table schema, which the encoder carries.
    pub schema: u64,
    pub generation: u64,
    pub project_id: Arc<str>,
    pub sources: Vec<SourceDigest>,
    pub entities: EntityRows,
    pub files: Vec<FileFact>,
    pub symbols: Vec<SymbolFact>,
    /// Identifies one private retained table state. Several demand snapshots
    /// can materialize within one source generation, so the generation alone
    /// cannot authenticate an exact transport manifest.
    pub state_id: u64,
    pub transport: Option<TransportChanges>,
    /// The compact path-to-symbols view a retained table keeps once the client
    /// owns the expanded path rows; consulted before the rows themselves.
    pub path_symbols: Option<HashMap<Arc<str>, Vec<Arc<str>>>>,
}

impl ProducerTable {
    #[must_use]
    pub fn new(generation: u64, project_id: &str) -> Self {
        Self {
            schema: TYPE_FACTS_SCHEMA_V1,
            generation,
            project_id: project_id.into(),
            sources: Vec::new(),
            entities: EntityRows::default(),
            files: Vec::new(),
            symbols: Vec::new(),
            state_id: 0,
            transport: None,
            path_symbols: None,
        }
    }

    pub(super) fn source(&self, path: &str) -> Option<&SourceDigest> {
        let index = self
            .sources
            .partition_point(|source| source.path.as_ref() < path);
        self.sources
            .get(index)
            .filter(|source| source.path.as_ref() == path)
    }

    pub(super) fn file(&self, path: &str) -> Option<&FileFact> {
        let index = self.files.partition_point(|file| file.path.as_ref() < path);
        self.files
            .get(index)
            .filter(|file| file.path.as_ref() == path)
    }

    /// The entity rows of one path, empty when it has none.
    #[must_use]
    pub fn entities_in(&self, path: &str) -> &[EntityFact] {
        self.entities.path(path)
    }

    /// Go: `canonicalSymbol`.
    #[must_use]
    pub fn symbol(&self, id: &str) -> Option<&SymbolFact> {
        let index = self
            .symbols
            .partition_point(|symbol| symbol.id.as_ref() < id);
        self.symbols
            .get(index)
            .filter(|symbol| symbol.id.as_ref() == id)
    }
}

/// The identity the client's retained table keeps for a source file.
#[must_use]
pub fn source_digest(path: &str, source: &[u8]) -> SourceDigest {
    let hash = SourceHash::parse(format!("sha256:{:x}", Sha256::digest(source)))
        .expect("a SHA-256 digest is canonical");
    SourceDigest {
        path: path.into(),
        sha256: hash,
    }
}
