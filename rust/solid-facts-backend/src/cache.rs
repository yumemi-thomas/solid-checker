//! Retained facts keyed by source identity.

use std::{collections::HashMap, sync::Arc};

use solid_compiler_facts::ExecutionMap;

#[derive(Default)]
pub struct FactsCache {
    pub(crate) ast: HashMap<String, Arc<solid_ast_facts::AstFacts>>,
    pub(crate) compiler: HashMap<String, Arc<ExecutionMap>>,
    pub(crate) semantic_demands: HashMap<String, Arc<[typefacts::v3::EntityDemand]>>,
    pub(crate) structural_functions: HashMap<String, Vec<typefacts::SourceFunction>>,
    pub(crate) semantic_table: Option<(u64, solid_facts::TypeScriptTable)>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CacheStats {
    pub ast_entries: usize,
    pub compiler_entries: usize,
}

impl FactsCache {
    #[must_use]
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            ast_entries: self.ast.len(),
            compiler_entries: self.compiler.len(),
        }
    }

    pub fn clear(&mut self) {
        self.ast.clear();
        self.compiler.clear();
        self.semantic_demands.clear();
        self.structural_functions.clear();
        self.semantic_table = None;
    }

    pub fn invalidate_path(&mut self, path: &str) {
        let prefix = format!("{path}\0");
        self.ast.retain(|key, _| !key.starts_with(&prefix));
        self.compiler.retain(|key, _| !key.starts_with(&prefix));
        self.semantic_demands
            .retain(|key, _| !key.starts_with(&prefix));
        self.structural_functions
            .retain(|key, _| !key.starts_with(&prefix));
        self.semantic_table = None;
    }
}
