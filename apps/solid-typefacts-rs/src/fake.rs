//! A backend with fixed answers, standing in for a compiler. It exists so the
//! session, service loop and process behaviour can be exercised live before
//! S3 provides a real program; the `solid-typefacts-fake` binary serves it.
//! It is not an authority on anything and is never used by `solid-typefacts`.

use crate::backend::{
    Backend, BackendError, ClosureBackend, ModuleGraph, ModuleInventory, OpenBackend,
    SymbolEvidence, UpdateOutcome,
};
use crate::context::Context;
use std::sync::{Arc, Mutex};
use typefacts::SymbolFact;
use typefacts::v3::{FileChange, ModuleGraphRequest, SourceFile, SymbolQuery};

#[derive(Default)]
pub struct FakeBackend {
    pub closed: Arc<Mutex<u32>>,
    pub fail_update: bool,
}

impl ClosureBackend for FakeBackend {
    fn update(
        &mut self,
        _: &Context,
        changes: &[FileChange],
    ) -> Result<UpdateOutcome, BackendError> {
        if self.fail_update {
            return Err(BackendError("boom".to_owned()));
        }
        Ok(UpdateOutcome {
            affected: changes.iter().map(|change| change.path.clone()).collect(),
        })
    }

    fn source_files(&mut self, _: &Context) -> Result<Vec<SourceFile>, BackendError> {
        Ok(vec![
            SourceFile {
                path: "/p/a.ts".into(),
                local: false,
                source: b"let a".to_vec(),
            },
            SourceFile {
                path: "/p/b.ts".into(),
                local: false,
                source: Vec::new(),
            },
        ])
    }

    fn close(&mut self) -> Result<(), BackendError> {
        *self.closed.lock().unwrap() += 1;
        Ok(())
    }
}

impl SymbolEvidence for FakeBackend {
    fn symbol_evidence(
        &mut self,
        _: &Context,
        _: &[SymbolQuery],
    ) -> Result<Vec<SymbolFact>, BackendError> {
        Ok(Vec::new())
    }
}

impl ModuleGraph for FakeBackend {
    fn module_graph(
        &mut self,
        ctx: &Context,
        _: &ModuleGraphRequest,
    ) -> Result<ModuleInventory, BackendError> {
        ctx.cancel();
        Err(BackendError("interrupted".to_owned()))
    }
}

/// Opens a [`FakeBackend`] for any project.
pub fn fake_opener() -> OpenBackend {
    Box::new(|_ctx, _project, _config| Ok(Box::new(FakeBackend::default()) as Box<dyn Backend>))
}
