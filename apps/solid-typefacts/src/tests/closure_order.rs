//! Ports `closure_order_test.go`.

use crate::backend::{BackendError, ClosureBackend};
use crate::context::Context;
use crate::fake::FakeBackend;
use crate::model::{SymbolId, blank_symbol};
use crate::symbols::closure::{ClosureBuilder, order_symbol_facts};
use crate::symbols::intern::{SymbolHandleSet, SymbolInterner};
use crate::symbols::store::SymbolFactStore;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use typefacts::{Declaration, Location, SymbolFact};

fn location(path: &str, start: u64, end: u64) -> Location {
    super::location(path, start, end)
}

#[test]
fn order_symbol_facts_reuses_canonical_order_across_delta() {
    let mut updated = blank_symbol("d".into());
    updated.alias_target = "updated".into();
    let facts = vec![
        updated.clone(),
        blank_symbol("b".into()),
        blank_symbol("a".into()),
    ];
    let interner = SymbolInterner::new_shared();
    let mut fact_index = vec![0_i32; facts.len()];
    for (position, fact) in facts.iter().enumerate() {
        let handle = interner.borrow_mut().handle(&fact.id);
        fact_index[usize::try_from(handle).unwrap()] = i32::try_from(position).unwrap() + 1;
    }

    let previous = SymbolFactStore::new(vec![
        blank_symbol("a".into()),
        blank_symbol("c".into()),
        blank_symbol("d".into()),
    ]);
    let got = order_symbol_facts(facts, &mut fact_index, &interner, Some(&previous));
    assert_eq!(
        got,
        vec![blank_symbol("a".into()), blank_symbol("b".into()), updated],
        "ordered facts"
    );
}

fn with_declaration(id: &str, at: &Location, references: &[Location]) -> SymbolFact {
    let mut fact = blank_symbol(id.into());
    fact.declarations = Arc::from(vec![Declaration {
        name: "".into(),
        kind: "".into(),
        location: at.clone(),
    }]);
    fact.references = references.to_vec().into();
    fact
}

#[test]
fn patch_canonical_symbol_store_changes_only_delta_rows() {
    let old_reference = location("old.ts", 1, 2);
    let new_reference = location("new.ts", 3, 4);
    let previous = vec![
        with_declaration("a", &old_reference, std::slice::from_ref(&old_reference)),
        with_declaration("b", &old_reference, std::slice::from_ref(&old_reference)),
    ];
    let current = vec![
        with_declaration("b", &new_reference, &[]),
        with_declaration("a", &old_reference, &[]),
    ];
    let cached_references = HashMap::from([
        (SymbolId::from("a"), vec![old_reference.clone()]),
        (SymbolId::from("b"), vec![old_reference.clone()]),
    ]);

    // Go's `canonicalPatchBackend`: an exact delta naming "b" and its new list.
    let backend = FakeBackend {
        changed: (vec!["b".into()], true),
        ..FakeBackend::default()
    };
    struct Patch {
        inner: FakeBackend,
        references: HashMap<SymbolId, Vec<Location>>,
    }
    impl ClosureBackend for Patch {
        fn source_files(
            &mut self,
            ctx: &Context,
        ) -> Result<Vec<typefacts::v3::SourceFile>, BackendError> {
            self.inner.source_files(ctx)
        }
        fn update(
            &mut self,
            ctx: &Context,
            changes: &[typefacts::v3::FileChange],
        ) -> Result<crate::backend::UpdateOutcome, BackendError> {
            self.inner.update(ctx, changes)
        }
        fn close(&mut self) -> Result<(), BackendError> {
            self.inner.close()
        }
        fn resolve_alias(
            &mut self,
            ctx: &Context,
            id: &str,
        ) -> Result<Option<SymbolId>, BackendError> {
            self.inner.resolve_alias(ctx, id)
        }
        fn declarations(
            &mut self,
            ctx: &Context,
            id: &str,
        ) -> Result<Option<Vec<Declaration>>, BackendError> {
            self.inner.declarations(ctx, id)
        }
        fn semantic_demand_runs(
            &mut self,
            ctx: &Context,
            runs: &[crate::model::SemanticDemandRun<'_>],
            scope: &crate::model::SemanticScope<'_>,
        ) -> Result<Vec<crate::model::SemanticDemandRunResult>, BackendError> {
            self.inner.semantic_demand_runs(ctx, runs, scope)
        }
        fn async_functions_at(
            &mut self,
            ctx: &Context,
            locations: &[Location],
        ) -> Result<Vec<typefacts::AsyncFunctionFact>, BackendError> {
            self.inner.async_functions_at(ctx, locations)
        }
        fn references_batch(
            &mut self,
            _: &Context,
            _: &[SymbolId],
        ) -> Result<HashMap<SymbolId, Vec<Location>>, BackendError> {
            Ok(self.references.clone())
        }
        fn changed_references(
            &mut self,
            ctx: &Context,
        ) -> Result<(Vec<SymbolId>, bool), BackendError> {
            self.inner.changed_references(ctx)
        }
    }
    let mut patch = Patch {
        inner: backend,
        references: HashMap::from([("b".into(), vec![new_reference.clone()])]),
    };

    let interner = SymbolInterner::new_shared();
    let mut builder = ClosureBuilder::new(&mut patch, &interner, None);
    builder.symbol_seen = {
        let mut set = SymbolHandleSet::new(&interner, Vec::new());
        set.add_id(&"a".into());
        set.add_id(&"b".into());
        set
    };
    builder.full_tier = {
        let mut set = SymbolHandleSet::new(&interner, Vec::new());
        set.add_id(&"a".into());
        set.add_id(&"b".into());
        set
    };
    builder.cached_references = Some(cached_references);
    builder.cached_canonical_store = Some(Arc::new(SymbolFactStore::new(previous)));
    builder.changed_symbols.add(&"b".into());

    let store = builder
        .patch_canonical_symbol_store(&Context::new(), &current)
        .unwrap()
        .expect("stable canonical table did not take the retained patch path");
    assert_eq!(
        store.facts(),
        vec![
            with_declaration("a", &old_reference, std::slice::from_ref(&old_reference)),
            with_declaration("b", &new_reference, std::slice::from_ref(&new_reference)),
        ],
        "patched facts"
    );
    assert!(
        builder.patched_symbol_rows != 0,
        "retained patch path did not report any patched rows"
    );
    let _ = HashSet::<()>::new();
}
