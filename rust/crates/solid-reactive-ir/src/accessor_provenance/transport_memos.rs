//! Transport on memos (S7c; S7 design §4a): each file's transported calls
//! as a memo over the resolver's memo world, folded in file order.
//!
//! A call's transport answer ([`Transport`]) is the depth-24 value walk at its
//! callee, or the array-member census at a built-in `map`/`forEach`. Both run
//! over [`super::resolver_memos::MemoWorld::serving`], where every table of the
//! resolver is a recorded read of what its pass published (S7b), stage 4's
//! blocked contexts included. A row keeps the targets that need a summary
//! node ([`TransportRow`]): the graph's function set decides them when the
//! rows are folded, so the memo does not read it. A file's memo is reused
//! while its own source and every answer it read are unchanged, so an edit
//! recomputes the files whose walks reach what it moved.
//!
//! Served only when the resolver was constructed on its memo stages this
//! build (otherwise the tables the walks read were not published) and every
//! path has one slot; otherwise, and in shadow and verify-reused modes for
//! comparison, the resolver answers over the lookup ([`Resolver::calls`]).

use super::resolver_memos::{MemoWorld, path_file, served_this_build};
use super::resolver_queries::{
    Transport, TransportRow, TransportWorld, fold_transport_rows, transport_rows,
};
use super::*;
use crate::base::FileId;
use crate::base::memo::build::{BuildSchema, BuildTables, Input, Section};
use crate::base::memo::{Canonical, Ctx, Cycle, ErasedTable, Memo, Table, TableId};

/// The transport memo tables hold ids 384 to 447 (S7c).
pub(crate) const TRANSPORT_TABLES_FROM: u16 = 384;
pub(crate) const TRANSPORT_TABLES_END: u16 = 448;

/// The transport's memo tables, inside the build's.
#[derive(Default)]
pub(crate) struct TransportTables {
    calls: Table<BuildSchema, TransportedCalls>,
}

impl TransportTables {
    pub(crate) fn table(&self, id: TableId) -> Option<&dyn ErasedTable<BuildSchema>> {
        (id == TransportedCalls::ID).then_some(&self.calls as &dyn ErasedTable<BuildSchema>)
    }

    pub(crate) fn tables(&self) -> Vec<&dyn ErasedTable<BuildSchema>> {
        vec![&self.calls]
    }
}

impl Canonical for Callable {}
impl Canonical for Transport {}
impl Canonical for TransportRow {}

/// A file's calls with a transport answer, by callee span, in call order;
/// `None` when a read met a cycle (the build answers over the lookup).
type Rows = Option<Arc<Vec<(Span, TransportRow)>>>;

/// [`transport_rows`] of one file.
pub(crate) struct TransportedCalls;

impl Memo<BuildSchema> for TransportedCalls {
    type Key = FileId;
    type Value = Rows;
    const NAME: &'static str = "transported_calls";
    const ID: TableId = TableId(TRANSPORT_TABLES_FROM);

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        &tables.transport().calls
    }

    fn compute(ctx: &Ctx<'_, '_, BuildSchema>, id: &FileId) -> Result<Rows, Cycle> {
        let world = MemoWorld::serving(ctx);
        let rows = world
            .read(*id, 0)
            .map(|file| transport_rows(&world, file))
            .unwrap_or_default();
        world.finish(Some(Arc::new(rows)))
    }

    fn on_cycle(_: &FileId) -> Rows {
        None
    }

    fn implied(id: &FileId) -> Vec<Input> {
        vec![
            Input::File(*id, Section::Source),
            Input::File(*id, Section::Entities),
        ]
    }
}

impl<'w> TransportWorld<'w> for MemoWorld<'_, '_, 'w> {
    fn named_callback(&self, file: &'w FileFacts, span: Span) -> bool {
        let Some(id) = self.id(file) else {
            return true;
        };
        // Named callbacks' roles are not ported: the lookup answers, a read
        // of the whole program. Asked last, of the array-member census's
        // complete candidates only.
        self.ctx().project_files(|lookup, table| {
            path_file(lookup, table, id).is_none_or(|file| {
                crate::execution_role::named_callback_execution_role(file, span, lookup).is_some()
            })
        })
    }
}

impl<'a, 'facts: 'a> Resolver<'a, 'facts> {
    /// [`Self::calls`], from each file's [`TransportedCalls`] when the
    /// resolver's memo construction served this build.
    pub fn transported_calls(&self) -> (HashMap<Location, Callable>, HashSet<Location>) {
        let lookup = self.lookup;
        let store = lookup.memo_store();
        if !served_this_build(store) || crate::audit_off("transport") {
            return self.calls();
        }
        let mut files = Vec::with_capacity(self.facts.files.len());
        for (slot, file) in self.facts.files.iter().enumerate() {
            let (id, rank) = lookup.memo_slot_of(slot);
            if rank != 0 {
                return self.calls();
            }
            files.push((id, file));
        }
        let world = lookup.memo_world();
        let answers = crate::pipeline::parallel_balanced_results(&files, |(id, _)| {
            world.answer::<TransportedCalls>(id)
        });
        let mut calls = HashMap::default();
        let mut unavailable = HashSet::default();
        for ((_, file), rows) in files.iter().zip(answers) {
            let Some(rows) = rows else {
                return self.calls();
            };
            fold_transport_rows(
                &mut calls,
                &mut unavailable,
                &file.path.shared(),
                &rows,
                &|path, span| self.functions.contains(&(path.to_owned(), span)),
            );
        }
        if store.check_solvers() {
            let (fresh_calls, fresh_unavailable) = self.calls();
            if fresh_calls != calls || fresh_unavailable != unavailable {
                store.poison(
                    "IR transport verify mismatch: the transported calls on memos differ from the resolver's",
                );
                return (fresh_calls, fresh_unavailable);
            }
        }
        (calls, unavailable)
    }
}
