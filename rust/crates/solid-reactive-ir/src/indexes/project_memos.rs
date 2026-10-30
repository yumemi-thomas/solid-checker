//! Project-wide answers as memos (P5 study S5d): the effect-apply
//! execution tables, exported primitive constants, withheld initialization
//! mentions, parameter-completion and never-returning calls, the exempt
//! store-key census, and the any-file vetoes (intrinsics, string code, tuple
//! iteration, the `Function.prototype.bind` patch, objects assigned into, the
//! router-parameters premise).
//!
//! Reads, by table:
//! - [`NeverReturning`], per file: the file's own source and call signatures,
//!   nothing else (exact);
//! - every other table reads the whole program through
//!   [`Ctx::project`](crate::base::memo::Ctx::project): the lookup tables they
//!   walk (callee resolution, call sites, member targets, module census,
//!   assigned members) are not memos yet, so the answer is recorded as a read
//!   of [`Input::Project`], which moves whenever any file, the TypeScript
//!   table or a build constant does. Coarse and exact: such an answer is
//!   reused only across builds in which nothing changed, and a table's
//!   narrower reads arrive as the tables under it port (study S5a-c, S7).
//!   Fixed points (parameter completion) and the recursive initialization
//!   proof stay whole-program answers until their cone recomputation (S7).
//!
//! Answers are canonical (sorted vectors, never a hash map's order); the
//! lookup keeps per-build views (hash maps and sets over the answers) for its
//! hot queries.

use super::*;
use crate::base::FileId;
use crate::base::memo::build::{BuildSchema, BuildTables, Input, Section};
use crate::base::memo::{Canonical, Ctx, Cycle, ErasedTable, Memo, Table, TableId, TableSet};
use crate::effect_execution::{ApplyCallRows, ApplyRegionRows};

/// The first table id of the project-wide answers (the lower ids are the
/// identity wrappers', and the other S5 slices').
pub(crate) const PROJECT_TABLES_FROM: u16 = 32;

impl Canonical for crate::router_params::RouterParamsStatus {}
impl Canonical for crate::execution_role::RootBodyGuard {}

macro_rules! project_tables {
    ($($id:literal $field:ident: $memo:ident),* $(,)?) => {
        /// The project-wide answers' tables.
        #[derive(Default)]
        pub(crate) struct ProjectTables {
            $($field: Table<BuildSchema, $memo>,)*
        }

        impl TableSet<BuildSchema> for ProjectTables {
            fn table(&self, id: TableId) -> &dyn ErasedTable<BuildSchema> {
                match id.0 {
                    $($id => &self.$field,)*
                    _ => unreachable!("project memo table {id:?}"),
                }
            }

            fn tables(&self) -> Vec<&dyn ErasedTable<BuildSchema>> {
                vec![$(&self.$field),*]
            }
        }

        $(
            impl ProjectTables {
                fn $field(&self) -> &Table<BuildSchema, $memo> {
                    &self.$field
                }
            }
        )*
    };
}

project_tables! {
    32 never_returning: NeverReturning,
    33 effect_apply_calls: EffectApplyCalls,
    34 effect_apply_regions: EffectApplyRegions,
    36 uninitialized_references: UninitializedReferences,
    37 parameter_completion_calls: ParameterCompletionCalls,
    58 parameter_completion_slots: ParameterCompletionSlot,
    39 intrinsics: IntrinsicsTripwires,
    40 string_code: StringCodeVeto,
    41 tuple_iteration: TupleIterationVeto,
    42 function_bind: FunctionBindPatched,
    43 objects_assigned_into: ObjectsAssignedInto,
    44 router_params: RouterParams,
    45 source_texts: SourceTexts,
    46 dispatch_veto: DispatchVetoAnswer,
    47 write_flow_answers: WriteFlowAnswer,
    48 module_scope: ModuleScopeRuns,
}

/// A whole-program memo: key `()`, computed from the lookup in one
/// [`Ctx::project`] read, with the fail-closed answer for a cycle.
macro_rules! whole_program {
    (
        $(#[$doc:meta])*
        $memo:ident, $id:literal, $field:ident, $name:literal, $value:ty,
        cycle: $cycle:expr,
        direct: $direct:expr,
        |$ctx:ident| $body:expr
    ) => {
        $(#[$doc])*
        pub(crate) struct $memo;

        impl Memo<BuildSchema> for $memo {
            type Key = ();
            type Value = $value;
            const NAME: &'static str = $name;
            const ID: TableId = TableId($id);

            fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
                tables.project().$field()
            }

            fn compute($ctx: &Ctx<'_, '_, BuildSchema>, _: &()) -> Result<$value, Cycle> {
                $body
            }

            fn on_cycle(_: &()) -> $value {
                $cycle
            }
        }

        impl WholeProgram for $memo {
            fn direct(lookup: &SemanticLookup<'_>) -> $value {
                let direct: fn(&SemanticLookup<'_>) -> $value = $direct;
                direct(lookup)
            }
        }
    };
}

/// A whole-program memo's answer computed straight from a lookup, without the
/// store: what a memo probe (`indexes::memo_probes`) gets, since a probe is
/// re-asked while a build begins and may not read a memo. The memo's compute
/// is `ctx.project(direct)` after reading other whole-program memos whose
/// answers `direct` re-derives the same way, so both are the same function
/// of the lookup.
pub(crate) trait WholeProgram: Memo<BuildSchema, Key = ()> {
    fn direct(lookup: &SemanticLookup<'_>) -> Self::Value;
}

/// ADR 0278: the `never`-returning and `asserts` calls written in one path,
/// sorted by span. Per file, reading only the file.
/// A file's source bytes, as its slots' hashes: what a reader records when it
/// re-parses another file's text beyond the AST tables (ADR 0280's argument
/// sites and parameter uses), which the late stages' input match does not
/// compare ([`crate::indexes::SemanticLookup::note_source_read`]).
pub(crate) struct SourceTexts;

impl Memo<BuildSchema> for SourceTexts {
    type Key = FileId;
    type Value = Vec<String>;
    const NAME: &'static str = "source_texts";
    const ID: TableId = TableId(45);

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        tables.project().source_texts()
    }

    fn compute(ctx: &Ctx<'_, '_, BuildSchema>, id: &FileId) -> Result<Vec<String>, Cycle> {
        let mut hashes = Vec::new();
        let mut rank = 0;
        while let Some(read) = ctx.file(*id, rank) {
            hashes.push(read.facts().source_hash.as_str().to_owned());
            rank += 1;
        }
        Ok(hashes)
    }

    fn on_cycle(_: &FileId) -> Vec<String> {
        Vec::new()
    }

    fn implied(id: &FileId) -> Vec<Input> {
        vec![Input::File(*id, Section::Source)]
    }
}

pub(crate) struct NeverReturning;

impl Memo<BuildSchema> for NeverReturning {
    type Key = FileId;
    type Value = Vec<Span>;
    const NAME: &'static str = "never_returning_calls";
    const ID: TableId = TableId(32);

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        tables.project().never_returning()
    }

    fn compute(ctx: &Ctx<'_, '_, BuildSchema>, id: &FileId) -> Result<Vec<Span>, Cycle> {
        let mut spans = Vec::new();
        let mut rank = 0;
        while let Some(read) = ctx.file(*id, rank) {
            for call in &read.facts().ast.calls {
                // The compiler's own type flag, not the display text
                // (docs/typefacts/compiler-semantic-facts.md).
                if read
                    .resolved_callee_call(call.callee)
                    .is_some_and(|resolved| resolved.never_returns || resolved.assertion_signature)
                {
                    spans.push(call.span);
                }
            }
            rank += 1;
        }
        spans.sort_unstable();
        Ok(spans)
    }

    fn on_cycle(_: &FileId) -> Vec<Span> {
        Vec::new()
    }

    fn implied(id: &FileId) -> Vec<Input> {
        vec![
            Input::File(*id, Section::Source),
            Input::File(*id, Section::Signatures),
        ]
    }
}

whole_program! {
    /// ADR 0302: the execution of each apply argument of a split effect,
    /// `(path, call, argument, execution)`. A call with no row is unknown.
    EffectApplyCalls, 33, effect_apply_calls, "effect_apply_calls", ApplyCallRows,
    cycle: Vec::new(),
    direct: crate::effect_execution::calls,
    |ctx| Ok(ctx.project(crate::effect_execution::calls))
}

whole_program! {
    /// ADR 0302: the regions of each path that run only under an apply that is
    /// not proven, `(path, [(function, execution)])`. Reads the calls' answer.
    EffectApplyRegions, 34, effect_apply_regions, "effect_apply_regions", ApplyRegionRows,
    cycle: Vec::new(),
    direct: crate::effect_execution::regions,
    |ctx| {
        ctx.memo::<EffectApplyCalls>(&())?;
        Ok(ctx.project(crate::effect_execution::regions))
    }
}

whole_program! {
    /// ADR 0339, 0464: by path, the mentions of a function-valued or
    /// pattern-declared binding the call graph withholds, and those of them
    /// not proven to follow the declarator, all sorted. A cycle cannot occur
    /// (the proof's own recursion lives inside the one answer), so the empty
    /// answer is never served: it would be reported and the store emptied.
    UninitializedReferences, 36, uninitialized_references, "initialization_rejected",
    Vec<(String, Vec<Span>, Vec<Span>)>,
    cycle: Vec::new(),
    direct: |lookup| lookup.compute_uninitialized_references(),
    |ctx| Ok(ctx.project(|lookup| lookup.compute_uninitialized_references()))
}

whole_program! {
    /// ADR 0277: `(path, function, parameter)` for each synchronous project
    /// function parameter called on its every completion path, sorted: the
    /// fixed point over every slot's part ([`ParameterCompletionSlot`]).
    ParameterCompletionCalls, 37, parameter_completion_calls, "parameter_completion_calls",
    Vec<(String, Span, usize)>,
    cycle: Vec::new(),
    direct: |lookup| lookup.compute_parameter_completion_calls(),
    |ctx| {
        let mut slots = Vec::new();
        for id in ctx.ids() {
            for rank in 0..ctx.slots(id).len() {
                let rank = u32::try_from(rank).expect("fewer than 2^32 slots of one path");
                slots.push((ctx.path_of(id), ctx.memo::<ParameterCompletionSlot>(&(id, rank))?));
            }
        }
        Ok(super::completion::parameter_completion_fixpoint(
            slots.iter().map(|(path, slot)| (*path, &**slot)),
        ))
    }
}

/// One slot's part of [`ParameterCompletionCalls`]: its own file, and the
/// callees its forwards reach through the lookup's recorded questions
/// ([`Ctx::project_file_recorded`]).
pub(crate) struct ParameterCompletionSlot;

impl Memo<BuildSchema> for ParameterCompletionSlot {
    type Key = (FileId, u32);
    type Value = Arc<super::completion::CompletionSlot>;
    const NAME: &'static str = "parameter_completion_slot";
    const ID: TableId = TableId(58);

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        tables.project().parameter_completion_slots()
    }

    fn compute(
        ctx: &Ctx<'_, '_, BuildSchema>,
        (id, rank): &(FileId, u32),
    ) -> Result<Self::Value, Cycle> {
        Ok(ctx.project_file_recorded(*id, |lookup, table| {
            table
                .slots_of(*id)
                .get(*rank as usize)
                .and_then(|slot| lookup.files().get(*slot))
                .map(|file| Arc::new(lookup.parameter_completion_slot(file)))
                .unwrap_or_default()
        }))
    }

    fn on_cycle(_: &(FileId, u32)) -> Self::Value {
        Arc::default()
    }
}

whole_program! {
    /// ADR 0349: whether the Promise and array-iteration premises hold, as
    /// `(promise, array_iteration)`.
    IntrinsicsTripwires, 39, intrinsics, "intrinsics", (bool, bool),
    cycle: (false, false),
    direct: |lookup| {
        let found = super::intrinsics::tripwires(lookup);
        (found.promise, found.array_iteration)
    },
    |ctx| Ok(ctx.project(|lookup| {
        let found = super::intrinsics::tripwires(lookup);
        (found.promise, found.array_iteration)
    }))
}

whole_program! {
    /// ADR 0505: the project files whose module scope may run code, those
    /// that only create nodes, the runtime imports between project files (ADR
    /// 0510), and whether a file loads a module no analyzed file shows.
    ModuleScopeRuns, 48, module_scope, "module_scope_runs", (Vec<String>, Vec<String>, Vec<(String, String)>, bool),
    cycle: (Vec::new(), Vec::new(), Vec::new(), true),
    direct: |lookup| super::module_scope::census(lookup),
    |ctx| Ok(ctx.project(super::module_scope::census))
}

whole_program! {
    /// Whether any visible code constructs code from a string.
    StringCodeVeto, 40, string_code, "string_code_veto", bool,
    cycle: true,
    direct: |lookup| {
        crate::interproc::accessor_provenance::visible_string_code(lookup)
    },
    |ctx| Ok(ctx.project(|lookup| {
        crate::interproc::accessor_provenance::visible_string_code(lookup)
    }))
}

whole_program! {
    /// ADR 0282: whether the tuple-iteration protocol may be mutated. Reads
    /// the string-code veto's answer.
    TupleIterationVeto, 41, tuple_iteration, "tuple_iteration_veto", bool,
    cycle: true,
    direct: |lookup| {
            crate::interproc::accessor_provenance::tuple_iteration_write(lookup)
        },
    |ctx| {
        ctx.memo::<StringCodeVeto>(&())?;
        Ok(ctx.project(|lookup| {
            crate::interproc::accessor_provenance::tuple_iteration_write(lookup)
        }))
    }
}

whole_program! {
    /// ADR 0338: whether any file may patch `Function.prototype.bind`.
    FunctionBindPatched, 42, function_bind, "function_bind_patched", bool,
    cycle: true,
    direct: |lookup| lookup.compute_function_bind_patched(),
    |ctx| Ok(ctx.project(|lookup| lookup.compute_function_bind_patched()))
}

whole_program! {
    /// ADR 0320: whether any code may copy keys into an existing object.
    ObjectsAssignedInto, 43, objects_assigned_into, "objects_assigned_into", bool,
    cycle: true,
    direct: |lookup| lookup.compute_objects_assigned_into(),
    |ctx| Ok(ctx.project(|lookup| lookup.compute_objects_assigned_into()))
}

whole_program! {
    /// ADR 0324: whether the router-parameters premise applies.
    RouterParams, 44, router_params, "router_params",
    crate::router_params::RouterParamsStatus,
    cycle: crate::router_params::RouterParamsStatus::Replaced {
        spelling: "a failed-closed memo cycle",
    },
    direct: |lookup| {
        crate::router_params::router_params_status(
            lookup.facts,
            lookup.dialect,
            &lookup.router_install,
        )
    },
    |ctx| Ok(ctx.project(|lookup| {
        crate::router_params::router_params_status(
            lookup.facts,
            lookup.dialect,
            &lookup.router_install,
        )
    }))
}

whole_program! {
    /// ADR 0450: the classified dispatch veto (ADR 0356, 0435, 0437, 0438) as
    /// `(owner proofs, read proofs)`. The veto is a computation over the whole
    /// lookup (the dispatch sites, the receivers' declarations, the callers'
    /// positions, the two function censuses), so this memo reads the whole
    /// program and reruns in every build in which it moved; a reader of the
    /// veto records this answer, not the program, and reruns only when the two
    /// bits do (early cutoff: an equal rerun keeps its revision). Fail
    /// closed on a cycle: both proofs withheld.
    DispatchVetoAnswer, 46, dispatch_veto, "dispatch_veto", (bool, bool),
    cycle: (true, true),
    direct: |lookup| lookup.dispatch_veto(),
    |ctx| Ok(ctx.project(|lookup| lookup.dispatch_veto()))
}

/// ADR 0442, per function and guard (ADR 0450): the write-flow answer of the
/// function at `(path, span)`, `guard` the write's [`RootBodyGuard`]. Read
/// through the lookup, so it reruns in every build in which the program moved
/// and keeps its revision when its answer is equal; a reader of the answer
/// records this one, and only the functions it asks about are computed.
pub(crate) struct WriteFlowAnswer;

impl Memo<BuildSchema> for WriteFlowAnswer {
    type Key = (String, Span, crate::execution_role::RootBodyGuard);
    type Value = Option<(crate::ExecutionRole, bool)>;
    const NAME: &'static str = "write_flow_answer";
    const ID: TableId = TableId(47);

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        tables.project().write_flow_answers()
    }

    fn compute(
        ctx: &Ctx<'_, '_, BuildSchema>,
        (path, span, guard): &(String, Span, crate::execution_role::RootBodyGuard),
    ) -> Result<Self::Value, Cycle> {
        Ok(ctx.project(|lookup| {
            crate::execution_role::write_flow_answer(lookup, path, *span, *guard)
        }))
    }

    fn on_cycle(_: &(String, Span, crate::execution_role::RootBodyGuard)) -> Self::Value {
        None
    }
}

impl crate::base::memo::build::ProjectAccess for SemanticLookup<'_> {
    fn run(&self, f: &mut dyn for<'x> FnMut(&SemanticLookup<'x>)) {
        f(self);
    }
}
