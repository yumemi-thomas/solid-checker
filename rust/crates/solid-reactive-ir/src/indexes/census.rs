//! The component census on memos (study S5c): each function's component
//! identity, the call sites and tags that enter it, and the per-file tables
//! the identity reads, each an L2 answer with its reads recorded
//! ([`crate::base::memo`]).
//!
//! Two kinds of table:
//! - **census memos**, whose reads are files (their sections), L1 index keys
//!   (references by symbol, calls and tags by callee), build constants and
//!   other memos. A function's identity reads its own file, the files that
//!   reference or enter it and their per-file answers, so an edit elsewhere
//!   leaves it unrun;
//! - **adapters** over a lookup table not yet ported (the function each
//!   symbol names, primitives, TypeScript symbols and descriptors, routes,
//!   hand-outs, the program-wide vetoes compound holders read), each one
//!   [`Ctx::project`] read (`Input::Project`, which moves whenever the
//!   program does): the census memos that read one keep their answers by
//!   early cutoff when it answers as before. An adapter answers per file or
//!   per symbol, never per call site, so a build re-asks few. Where another
//!   slice holds the table, the census reads it instead: initialization
//!   through S5d's memo, the module census and the `eval` hazard through
//!   S5b's probes.
//!
//! **Premise.** The slots of one path carry one source: the join checks each
//! file against its path's TypeFacts source digest, and everything a census
//! memo reads of a file is a function of that source and that path's TypeFacts
//! rows. So the memos read a path's first slot wherever the lookup read its
//! last one (`file_by_path`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use solid_dialect::Primitive;

use super::*;
use crate::base::memo::build::{BuildSchema, BuildTables, Constant, Input, Section};
use crate::base::memo::{Canonical, Ctx, Cycle, ErasedTable, Memo, Table, TableId};
use crate::base::{CalleeSite, FileId, FileTable};

/// What [`ComponentStatusAt`] decides for one function.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ComponentIdentity {
    No,
    Proven,
    /// Proven but for a reference that enters it some other way.
    Escaped,
    /// Called, but entered some way the census cannot follow (ADR 0331).
    Possible,
}

impl Canonical for ComponentIdentity {}

/// One entry the call graph names for a project function: a call expression
/// or a JSX tag that resolves to it ([`SemanticLookup::function_call_sites`]).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct CallSite {
    pub(crate) file: FileId,
    pub(crate) rank: u32,
    /// The callee, or the tag's name.
    pub(crate) callee: Span,
    /// The closing tag's name, which one render spells as a second reference.
    pub(crate) also_referenced: Option<Span>,
}

impl Canonical for CallSite {}

/// How the TypeFacts references of a symbol are read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SymbolReferences {
    /// TypeFacts does not know the symbol: no reference census.
    Unknown,
    /// The references index lists them; `foreign` when some reference lies
    /// outside the analyzed files (or past `u32` bytes), which no census
    /// accounts for.
    Indexed { foreign: bool },
    /// The table lists the id more than once (the lookup keeps the last
    /// symbol): its references, `None` for a foreign one.
    Listed(Arc<CensusReferences>),
}

impl Canonical for SymbolReferences {}

/// A dialect loader's `default` candidate in a file
/// ([`crate::owners::lazy_default_render_candidate`]): the component's
/// ordinal and span, the property and its value.
pub(crate) type LazyCandidate = (u32, Span, Span, Span);

/// The first table id of the census (ids 0-1 are the identity wrappers',
/// 20-31 member resolution's, 32-63 the project-wide answers').
pub(crate) const CENSUS_TABLES_FROM: u8 = 64;

macro_rules! census_tables {
    ($($field:ident: $memo:ty = $id:literal;)*) => {
        /// The census's memo tables, inside the build's
        /// ([`crate::base::memo::build::BuildTables`]).
        #[derive(Default)]
        pub(crate) struct CensusTables {
            $($field: Table<BuildSchema, $memo>,)*
        }

        impl CensusTables {
            pub(crate) fn table(&self, id: TableId) -> Option<&dyn ErasedTable<BuildSchema>> {
                match id.0 {
                    $($id => Some(&self.$field),)*
                    _ => None,
                }
            }

            pub(crate) fn tables(&self) -> Vec<&dyn ErasedTable<BuildSchema>> {
                vec![$(&self.$field),*]
            }
        }

        $(impl $memo {
            pub(super) const TABLE: TableId = TableId($id);

            pub(super) fn of(tables: &BuildTables) -> &Table<BuildSchema, Self> {
                &tables.census().$field
            }
        })*
    };
}

census_tables! {
    component_status: ComponentStatusAt = 64;
    file_component_statuses: FileComponentStatuses = 65;
    member_tag: ProgramMemberTag = 66;
    assign_patched: ObjectAssignPatched = 67;
    assign_calls: AssignBuiltinCalls = 68;
    call_sites: CallSitesOf = 69;
    tag_rendered: TagRendered = 70;
    prop_render_sites: PropRenderSitesIn = 71;
    lazy_candidates: LazyCandidatesIn = 72;
    lazy_witnesses: LazyWitnessesIn = 73;
    forwarded_from: ForwardedFromIn = 74;
    forwarded: ResultAccessForwardedAll = 75;
    function_symbols: FileFunctionSymbols = 76;
    uninitialized: UninitializedMentions = 77;
    call_primitives: CallPrimitives = 78;
    component_type: ProjectComponentType = 79;
    component_type_names: ComponentTypeNames = 80;
    symbol_references: SymbolReferencesOf = 81;
    hand_outs: HandOuts = 82;
    route_values: RouteValues = 83;
    compound_values: super::compound::CompoundValuesIn = 84;
    prototype_write: PrototypeWriteVeto = 85;
    alias_rendered: AliasRendered = 86;
    package_names: PackageNames = 87;
    redirected_calls: RedirectedCalls = 88;
    redirected_callee: RedirectedCallee = 89;
    result_access: ResultAccessAnswered = 90;
}

/// A memo over a lookup table not yet ported: its whole computation is one
/// [`Ctx::project`] read.
macro_rules! adapter {
    (
        $(#[$doc:meta])*
        $name:ident: $key:ty => $value:ty, $label:literal,
        cycle: $cycle:expr,
        |$lookup:ident, $table:ident, $k:ident| $body:expr
    ) => {
        $(#[$doc])*
        pub(crate) struct $name;

        impl Memo<BuildSchema> for $name {
            type Key = $key;
            type Value = $value;
            const NAME: &'static str = $label;
            const ID: TableId = Self::TABLE;

            fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
                Self::of(tables)
            }

            fn compute(ctx: &Ctx<'_, '_, BuildSchema>, key: &$key) -> Result<$value, Cycle> {
                Ok(ctx.project_files(|$lookup, $table| {
                    let $k = key;
                    $body
                }))
            }

            fn on_cycle(_: &$key) -> $value {
                $cycle
            }
        }
    };
}

/// The first slot of `id` as the lookup holds it, for an adapter.
fn path_file<'l>(
    lookup: &SemanticLookup<'l>,
    table: &FileTable,
    id: FileId,
) -> Option<&'l FileFacts> {
    let slot = *table.slots_of(id).first()?;
    lookup.files().get(slot)
}

adapter! {
    /// The symbols the lookup resolves to each function of a file
    /// (`functions_by_symbol`, inverted), by the function's span.
    FileFunctionSymbols: FileId => Arc<BTreeMap<Span, Vec<Arc<str>>>>, "census_function_symbols",
    cycle: Arc::default(),
    |lookup, table, id| {
        path_file(lookup, table, *id)
            .and_then(|file| lookup.census.function_symbols(lookup).get(file.path.as_str()).cloned())
            .map(Arc::new)
            .unwrap_or_default()
    }
}

/// The mentions of a file that may run before their binding's declarator
/// (ADR 0339), sorted: the file's part of S5d's whole-program answer
/// ([`super::project_memos::UninitializedReferences`]), so a reader is
/// dirtied only when this file's mentions move.
pub(crate) struct UninitializedMentions;

impl Memo<BuildSchema> for UninitializedMentions {
    type Key = FileId;
    type Value = Arc<Vec<Span>>;
    const NAME: &'static str = "census_uninitialized_mentions";
    const ID: TableId = Self::TABLE;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        Self::of(tables)
    }

    fn compute(ctx: &Ctx<'_, '_, BuildSchema>, id: &FileId) -> Result<Arc<Vec<Span>>, Cycle> {
        let rejected = ctx.memo::<super::project_memos::UninitializedReferences>(&())?;
        let path = ctx.path_of(*id);
        Ok(Arc::new(
            rejected
                .iter()
                .find(|(candidate, _)| candidate.as_str() == path)
                .map(|(_, spans)| spans.clone())
                .unwrap_or_default(),
        ))
    }

    fn on_cycle(_: &FileId) -> Arc<Vec<Span>> {
        Arc::default()
    }
}

adapter! {
    /// The primitive resolved at each call span of a file
    /// ([`SemanticLookup::primitive_at_call`]), for the calls that have one.
    CallPrimitives: FileId => Arc<BTreeMap<Span, Primitive>>, "census_call_primitives",
    cycle: Arc::default(),
    |lookup, table, id| {
        let mut primitives = BTreeMap::new();
        if let Some(file) = path_file(lookup, table, *id)
            && let Some(index) = lookup.ast_file_index(file.path.as_str())
        {
            for call in &index.ast.calls {
                if let Some(primitive) = lookup.primitive_at_call(file, call.span) {
                    primitives.entry(call.span).or_insert(primitive);
                }
            }
        }
        Arc::new(primitives)
    }
}

adapter! {
    /// Whether some entity's type is an exact Solid component type alias.
    ProjectComponentType: () => bool, "census_project_component_type",
    cycle: false,
    |lookup, _table, _key| lookup.project_has_component_type()
}

adapter! {
    /// The binding names of a file's functions whose compiler-resolved type
    /// is an exact Solid component type alias, sorted.
    ComponentTypeNames: FileId => Arc<Vec<Span>>, "census_component_type_names",
    cycle: Arc::default(),
    |lookup, table, id| {
        let mut names = Vec::new();
        if let Some(file) = path_file(lookup, table, *id) {
            for function in &file.ast.functions {
                let name = crate::owners::component_binding_name(file, function);
                if let Some(name) = name
                    && lookup.binding_has_component_type(file, Some(name))
                {
                    names.push(name.span);
                }
            }
        }
        names.sort_unstable();
        names.dedup();
        Arc::new(names)
    }
}

adapter! {
    /// How the TypeFacts references of a symbol are read
    /// ([`SymbolReferences`]).
    SymbolReferencesOf: Arc<str> => SymbolReferences, "census_symbol_references",
    cycle: SymbolReferences::Unknown,
    |lookup, table, symbol| lookup.census.symbol_references(lookup, table, symbol)
}

adapter! {
    /// The closed program's namespace census for a module (ADR 0347): whether
    /// its namespace escapes unnamed, and which of its functions are handed
    /// out by name, sorted.
    HandOuts: FileId => (bool, Arc<Vec<Span>>), "census_hand_outs",
    cycle: (true, Arc::default()),
    |lookup, table, id| {
        let Some(file) = path_file(lookup, table, *id) else {
            return (false, Arc::default());
        };
        let path = file.path.as_str();
        let mut named = file
            .ast
            .functions
            .iter()
            .map(|function| function.span)
            .filter(|span| lookup.hands_out_by_name(path, *span))
            .collect::<Vec<_>>();
        named.sort_unstable();
        named.dedup();
        (lookup.escapes_unnamed(path), Arc::new(named))
    }
}

adapter! {
    /// The values in a file that an audited router renders as route
    /// components (ADR 0334), sorted.
    RouteValues: FileId => Arc<Vec<Span>>, "census_route_values",
    cycle: Arc::default(),
    |lookup, table, id| lookup.census.route_values(lookup, table, *id)
}

adapter! {
    /// Whether some analyzed file holds a JSX member tag (ADR 0342).
    ProgramMemberTag: () => bool, "census_member_tag",
    cycle: true,
    |lookup, _table, _key| lookup.program_has_member_tag()
}

adapter! {
    /// Whether the project may replace the standard library's
    /// `Object.assign` (ADR 0342).
    ObjectAssignPatched: () => bool, "census_object_assign_patched",
    cycle: true,
    |lookup, _table, _key| lookup.object_assign_may_be_patched()
}

adapter! {
    /// A file's `Object.assign` calls the standard library answers, sorted.
    AssignBuiltinCalls: FileId => Arc<Vec<Span>>, "census_assign_builtin_calls",
    cycle: Arc::default(),
    |lookup, table, id| {
        Arc::new(
            path_file(lookup, table, *id)
                .map(|file| lookup.assign_builtin_calls(file))
                .unwrap_or_default(),
        )
    }
}

adapter! {
    /// Whether some file holds a reflective write through a `prototype`
    /// (ADR 0292).
    PrototypeWriteVeto: () => bool, "census_prototype_write_veto",
    cycle: true,
    |lookup, _table, _key| crate::interproc::accessor_provenance::reflective_prototype_write(lookup)
}

adapter! {
    /// The functions some tag renders through an exact alias only
    /// (`const P2 = Plain; <P2/>`, ADR 0331), as `(file, span)`.
    AliasRendered: () => Arc<BTreeSet<(FileId, Span)>>, "census_alias_rendered",
    cycle: Arc::default(),
    |lookup, table, _key| Arc::new(lookup.alias_rendered_functions(table))
}

adapter! {
    /// The names of a symbol's declarations in the dialect's own packages, in
    /// declaration order; `None` when TypeFacts does not know the symbol.
    PackageNames: Arc<str> => Option<Vec<Arc<str>>>, "census_package_names",
    cycle: None,
    |lookup, _table, symbol| {
        let packages = lookup.dialect.primitive_defining_packages();
        lookup.symbols_by_id().get(symbol.as_ref()).map(|candidate| {
            candidate
                .declarations()
                .iter()
                .filter(|declaration| {
                    declared_in_package(declaration.location.path.as_ref(), packages)
                })
                .map(|declaration| Arc::from(declaration.name.as_ref()))
                .collect()
        })
    }
}

adapter! {
    /// Under runtime redirects, every call whose callee names a redirected
    /// export ([`SemanticLookup::redirected_callee_symbol`]): `(file, rank,
    /// call ordinal, symbol)`, in file and call order.
    RedirectedCalls: () => Arc<Vec<(FileId, u32, u32, Arc<str>)>>, "census_redirected_calls",
    cycle: Arc::default(),
    |lookup, table, _key| {
        let mut calls = Vec::new();
        for (slot, file) in lookup.files().iter().enumerate() {
            let id = table.slot_id(slot);
            let rank = u32::try_from(table.slot_rank(slot)).expect("fewer than 2^32 slots");
            for (ordinal, call) in file.ast.calls.iter().enumerate() {
                if let Some(symbol) = lookup.redirected_callee_symbol(file, call.callee) {
                    calls.push((id, rank, ordinal32(ordinal), Arc::from(symbol)));
                }
            }
        }
        Arc::new(calls)
    }
}

adapter! {
    /// Under runtime redirects, the redirected export one callee span names.
    RedirectedCallee: (FileId, Span) => Option<Arc<str>>, "census_redirected_callee",
    cycle: None,
    |lookup, table, key| {
        path_file(lookup, table, key.0)
            .and_then(|file| lookup.redirected_callee_symbol(file, key.1))
            .map(Arc::from)
    }
}

adapter! {
    /// Whether the dialect answers a result-access slot for some primitive
    /// the project names ([`SemanticLookup::result_access_forwarded_arguments`]).
    ResultAccessAnswered: () => bool, "census_result_access_answered",
    cycle: true,
    |lookup, _table, _key| lookup.result_access_slots_answered()
}

fn ordinal32(index: usize) -> u32 {
    u32::try_from(index).expect("fewer than 2^32 items in one file")
}

/// A census memo's one-file read: the first slot of `id`.
fn first<'r, 'c, 'w>(
    ctx: &'r Ctx<'c, 'w, BuildSchema>,
    id: FileId,
) -> Option<crate::base::memo::build::FileRead<'r, 'c, 'w>> {
    ctx.file(id, 0)
}

/// A symbol's references as a census reads them: each as its file and span,
/// `None` for one outside the analyzed files.
pub(super) type CensusReferences = Vec<Option<(FileId, Span)>>;

/// The references of `symbol` a census reads: `None` when TypeFacts does not
/// know it. Each `(file, span)` once.
pub(super) fn symbol_references(
    ctx: &Ctx<'_, '_, BuildSchema>,
    symbol: &Arc<str>,
) -> Result<Option<CensusReferences>, Cycle> {
    Ok(match ctx.memo::<SymbolReferencesOf>(symbol)? {
        SymbolReferences::Unknown => None,
        SymbolReferences::Listed(references) => Some(references.as_ref().clone()),
        SymbolReferences::Indexed { foreign } => {
            let mut seen = BTreeSet::new();
            let mut references = ctx
                .references(symbol)
                .into_iter()
                .filter(|(file, _, span)| seen.insert((*file, *span)))
                .map(|(file, _, span)| Some((file, span)))
                .collect::<Vec<_>>();
            if foreign {
                references.push(None);
            }
            Some(references)
        }
    })
}

/// The symbols of the function at `span` in `id`.
fn function_symbols(
    ctx: &Ctx<'_, '_, BuildSchema>,
    (id, span): (FileId, Span),
) -> Result<Vec<Arc<str>>, Cycle> {
    Ok(ctx
        .memo::<FileFunctionSymbols>(&id)?
        .get(&span)
        .cloned()
        .unwrap_or_default())
}

/// [`SemanticLookup::may_precede_initialization`] over a file's mentions.
fn may_precede(file: &FileFacts, mentions: &[Span], span: Span) -> bool {
    if mentions.binary_search(&span).is_ok() {
        return true;
    }
    let peeled = file.ast.peel_ts_sugar_span(span);
    if peeled != span && mentions.binary_search(&peeled).is_ok() {
        return true;
    }
    file.ast
        .members
        .iter()
        .find(|member| member.span == peeled)
        .is_some_and(|member| mentions.binary_search(&member.property).is_ok())
}

/// The call graph's entries for the function at `(file, span)`
/// ([`SemanticLookup::function_call_sites`]): call expressions first, then
/// render sites, each in file order. A call or tag that may run before its
/// callee's declarator is none (ADR 0339).
///
/// Its reads: the function's symbols, each symbol's key of the callee index,
/// the slots of the files holding entries (their order), and those files
/// (the sites' spans and mentions). A file that neither calls nor renders the
/// function is not read.
pub(crate) struct CallSitesOf;

impl Memo<BuildSchema> for CallSitesOf {
    type Key = (FileId, Span);
    type Value = Arc<Vec<CallSite>>;
    const NAME: &'static str = "function_call_sites";
    const ID: TableId = Self::TABLE;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        Self::of(tables)
    }

    fn compute(
        ctx: &Ctx<'_, '_, BuildSchema>,
        target: &(FileId, Span),
    ) -> Result<Arc<Vec<CallSite>>, Cycle> {
        let symbols = function_symbols(ctx, *target)?;
        let mut calls = Vec::new();
        let mut tags = Vec::new();
        for symbol in &symbols {
            for (file, rank, site) in ctx.callees(symbol) {
                let rank = u32::try_from(rank).expect("fewer than 2^32 slots");
                match site {
                    CalleeSite::Call(ordinal) => calls.push((file, rank, ordinal)),
                    CalleeSite::Jsx(ordinal) => tags.push((file, rank, ordinal)),
                }
            }
        }
        // ADR 0322: a redirected export is a call's symbol in place of the
        // one the index files it under.
        if ctx.constant(Constant::RuntimeRedirects) {
            let redirected = ctx.memo::<RedirectedCalls>(&())?;
            calls.retain(|(file, rank, ordinal)| {
                !redirected
                    .iter()
                    .any(|(candidate, candidate_rank, call, _)| {
                        (candidate, candidate_rank, call) == (file, rank, ordinal)
                    })
            });
            calls.extend(
                redirected
                    .iter()
                    .filter(|(.., symbol)| symbols.contains(symbol))
                    .map(|(file, rank, ordinal, _)| (*file, *rank, *ordinal)),
            );
        }
        if calls.is_empty() && tags.is_empty() {
            return Ok(Arc::default());
        }
        let mut files = calls
            .iter()
            .chain(&tags)
            .map(|(file, ..)| *file)
            .collect::<Vec<_>>();
        files.sort_unstable();
        files.dedup();
        let slots = files
            .iter()
            .map(|file| (*file, ctx.slots(*file)))
            .collect::<BTreeMap<_, _>>();
        let order = |(file, rank, ordinal): &(FileId, u32, u32)| {
            let slot = slots
                .get(file)
                .and_then(|slots| slots.get(*rank as usize))
                .copied()
                .unwrap_or(usize::MAX);
            (slot, *ordinal)
        };
        calls.sort_by_key(order);
        calls.dedup();
        tags.sort_by_key(order);
        let mut mentions = BTreeMap::new();
        let mut sites = Vec::with_capacity(calls.len() + tags.len());
        for (index, (file, rank, ordinal)) in calls.iter().chain(&tags).enumerate() {
            let tag = index >= calls.len();
            let Some(read) = ctx.file(*file, *rank as usize) else {
                continue;
            };
            let facts = read.facts();
            let (callee, also_referenced) = if tag {
                let Some(element) = facts.ast.jsx_elements.get(*ordinal as usize) else {
                    continue;
                };
                (element.name.span, element.closing_name)
            } else {
                let Some(call) = facts.ast.calls.get(*ordinal as usize) else {
                    continue;
                };
                (call.callee, None)
            };
            let mentions = match mentions.entry(*file) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(ctx.memo::<UninitializedMentions>(file)?)
                }
            };
            if may_precede(facts, mentions, callee) {
                continue;
            }
            sites.push(CallSite {
                file: *file,
                rank: *rank,
                callee,
                also_referenced,
            });
        }
        Ok(Arc::new(sites))
    }

    fn on_cycle(_: &(FileId, Span)) -> Arc<Vec<CallSite>> {
        Arc::default()
    }
}

/// The functions of a file a JSX tag renders, by span, sorted: a tag whose
/// name resolves to one, or to an exact alias of it (ADR 0331).
///
/// Its reads: the file's functions' symbols, each symbol's key of the callee
/// index, and the alias renders. Not the file itself.
pub(crate) struct TagRendered;

impl Memo<BuildSchema> for TagRendered {
    type Key = FileId;
    type Value = Arc<Vec<Span>>;
    const NAME: &'static str = "jsx_call_sites";
    const ID: TableId = Self::TABLE;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        Self::of(tables)
    }

    fn compute(ctx: &Ctx<'_, '_, BuildSchema>, id: &FileId) -> Result<Arc<Vec<Span>>, Cycle> {
        let mut rendered = Vec::new();
        for (span, symbols) in ctx.memo::<FileFunctionSymbols>(id)?.iter() {
            if symbols.iter().any(|symbol| {
                ctx.callees(symbol)
                    .iter()
                    .any(|(_, _, site)| matches!(site, CalleeSite::Jsx(_)))
            }) {
                rendered.push(*span);
            }
        }
        rendered.extend(
            ctx.memo::<AliasRendered>(&())?
                .iter()
                .filter(|(file, _)| file == id)
                .map(|(_, span)| *span),
        );
        rendered.sort_unstable();
        rendered.dedup();
        Ok(Arc::new(rendered))
    }

    fn on_cycle(_: &FileId) -> Arc<Vec<Span>> {
        Arc::default()
    }
}

/// The rendering-prop flows of one file by value reference, each with the
/// render sites it reaches ([`SemanticLookup::prop_render_sites_at`]).
pub(crate) struct PropRenderSitesIn;

impl Memo<BuildSchema> for PropRenderSitesIn {
    type Key = FileId;
    type Value = Arc<BTreeMap<Span, Vec<Span>>>;
    const NAME: &'static str = "prop_render_sites";
    const ID: TableId = Self::TABLE;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        Self::of(tables)
    }

    fn compute(
        ctx: &Ctx<'_, '_, BuildSchema>,
        id: &FileId,
    ) -> Result<Arc<BTreeMap<Span, Vec<Span>>>, Cycle> {
        let mut map = BTreeMap::new();
        let dialect = ctx.dialect();
        let renderers = dialect.component_prop_renderers();
        if renderers.is_empty() {
            return Ok(Arc::default());
        }
        let mut props = renderers.iter().map(|(_, prop)| *prop).collect::<Vec<_>>();
        props.sort_unstable();
        props.dedup();
        let Some(read) = first(ctx, *id) else {
            return Ok(Arc::default());
        };
        let file = read.facts();
        // Only a module that spells a rendering component and its prop can
        // hold one of these flows; the rest are not parsed again.
        if !renderers
            .iter()
            .any(|(component, prop)| file.source.contains(component) && file.source.contains(prop))
        {
            return Ok(Arc::default());
        }
        let Some(flows) = solid_facts::ast::component_value_flows(
            std::path::Path::new(file.path.as_str()),
            &file.source,
            &props,
        ) else {
            return Ok(Arc::default());
        };
        let primitives = ctx.memo::<CallPrimitives>(id)?;
        for flow in flows {
            let exact = flow.holder_calls.iter().all(|call| {
                primitives
                    .get(call)
                    .is_some_and(|primitive| dialect.accessor_yields_only_its_compute(*primitive))
            });
            if !exact {
                continue;
            }
            let mut renders = true;
            for site in &flow.sites {
                if !renders_prop(ctx, &read, *id, site)? {
                    renders = false;
                    break;
                }
            }
            if renders {
                map.insert(
                    flow.reference,
                    flow.sites.iter().map(|site| site.site).collect(),
                );
            }
        }
        Ok(Arc::new(map))
    }

    fn on_cycle(_: &FileId) -> Arc<BTreeMap<Span, Vec<Span>>> {
        Arc::default()
    }

    fn implied(id: &FileId) -> Vec<Input> {
        vec![Input::File(*id, Section::Source)]
    }
}

/// Whether `site` renders its component with the dialect's semantics for its
/// prop: the component is exactly a dialect export that renders that prop,
/// and a call form's callee is exactly a dialect renderer of its first
/// argument.
fn renders_prop(
    ctx: &Ctx<'_, '_, BuildSchema>,
    read: &crate::base::memo::build::FileRead<'_, '_, '_>,
    id: FileId,
    site: &solid_facts::ast::ComponentPropSite,
) -> Result<bool, Cycle> {
    let dialect = ctx.dialect();
    if let Some(renderer) = site.renderer {
        let symbol = if ctx.constant(Constant::RuntimeRedirects)
            && let Some(redirected) = ctx.memo::<RedirectedCallee>(&(id, renderer))?
        {
            Some(redirected)
        } else {
            read.callee_symbol(renderer).map(Arc::from)
        };
        let rendered = match symbol {
            Some(symbol) => ctx.memo::<PackageNames>(&symbol)?.and_then(|names| {
                names
                    .iter()
                    .find_map(|name| dialect.renders_component_argument(name))
            }),
            None => None,
        };
        if rendered != Some(0) {
            return Ok(false);
        }
    }
    let Some(component) = read.symbol_at(site.component) else {
        return Ok(false);
    };
    let renderers = dialect.component_prop_renderers();
    Ok(ctx
        .memo::<PackageNames>(&component.shared())?
        .unwrap_or_default()
        .iter()
        .any(|declaration| {
            renderers
                .iter()
                .any(|(name, prop)| *name == declaration.as_ref() && *prop == site.prop.as_str())
        }))
}

/// The dialect loaders' `default` candidates of one file, in call order
/// ([`crate::owners::lazy_default_render_candidate`]).
pub(crate) struct LazyCandidatesIn;

impl Memo<BuildSchema> for LazyCandidatesIn {
    type Key = FileId;
    type Value = Arc<Vec<LazyCandidate>>;
    const NAME: &'static str = "lazy_default_candidates";
    const ID: TableId = Self::TABLE;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        Self::of(tables)
    }

    fn compute(
        ctx: &Ctx<'_, '_, BuildSchema>,
        id: &FileId,
    ) -> Result<Arc<Vec<LazyCandidate>>, Cycle> {
        let Some(read) = first(ctx, *id) else {
            return Ok(Arc::default());
        };
        let file = read.facts();
        if file.ast.calls.is_empty() {
            return Ok(Arc::default());
        }
        let primitives = ctx.memo::<CallPrimitives>(id)?;
        let dialect = ctx.dialect();
        let mut candidates = Vec::new();
        for call in &file.ast.calls {
            let Some((component, property, value)) = crate::owners::lazy_default_render_candidate(
                file,
                call,
                &|span| primitives.get(&span).copied(),
                dialect,
            ) else {
                continue;
            };
            let Some(ordinal) = file
                .ast
                .functions
                .iter()
                .position(|function| std::ptr::eq(function, component))
            else {
                continue;
            };
            candidates.push((ordinal32(ordinal), component.span, property, value));
        }
        Ok(Arc::new(candidates))
    }

    fn on_cycle(_: &FileId) -> Arc<Vec<LazyCandidate>> {
        Arc::default()
    }

    fn implied(id: &FileId) -> Vec<Input> {
        vec![Input::File(*id, Section::Source)]
    }
}

/// ADR 0292: `(function span, property span)` for each object property of a
/// file proven to be the one route by which a project component reaches anything:
/// the `default` member of the module object a dialect component loader
/// renders ([`solid_dialect::Dialect::renders_loaded_default_argument`]).
///
/// ```tsx
/// const Page = lazy(async () => {
///   function Cube() { start(); return <div />; }
///   return { default: Cube };
/// });
/// <Page />
/// ```
///
/// The witness holds only when all of these do:
///
/// - the loader is the call's whole argument at the dialect's slot, an
///   `async` non-generator function literal, so nothing but the call holds it;
/// - its one unconditional return (its own, not a nested function's) is a
///   closed object literal: data properties only, no spread, computed key,
///   duplicate key or `__proto__`, and no `then` (a thenable is assimilated
///   when the async return resolves, so it is not what `lazy` receives).
///   Its `default` property's value, under transparent TypeScript wrappers,
///   is a plain identifier;
/// - that identifier names a proven, synchronous, non-generator component
///   declared directly in the loader: a function declaration or a `const`
///   initialized with the function literal itself, unwritten, and referenced
///   nowhere else;
/// - the call initializes an unexported, unwritten `const`, and every
///   reference to it is a JSX tag name (an opening or closing name, never a
///   dotted one), with at least one opening tag. A `.preload()` would hand
///   the module promise to its caller, and any other use could reach it;
/// - no analyzed file holds a reflective write through a `prototype`, the
///   visible veto ADR 0288 keeps for a fresh literal.
///
/// The module object is then fresh, reachable only by the runtime that
/// selects its `default` and renders it, so the component's only invocations
/// are those renders.
pub(crate) struct LazyWitnessesIn;

impl Memo<BuildSchema> for LazyWitnessesIn {
    type Key = FileId;
    type Value = Arc<Vec<(Span, Span)>>;
    const NAME: &'static str = "lazy_default_render_witnesses";
    const ID: TableId = Self::TABLE;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        Self::of(tables)
    }

    fn compute(
        ctx: &Ctx<'_, '_, BuildSchema>,
        id: &FileId,
    ) -> Result<Arc<Vec<(Span, Span)>>, Cycle> {
        let candidates = ctx.memo::<LazyCandidatesIn>(id)?;
        let mut witnesses = Vec::new();
        for (ordinal, component, property, _) in candidates.iter() {
            if matches!(
                ctx.memo::<FileComponentStatuses>(&(*id, 0))?
                    .get(*ordinal as usize),
                Some(ComponentIdentity::Proven | ComponentIdentity::Escaped)
            ) {
                witnesses.push((*component, *property));
            }
        }
        // The veto is program-wide: with it, no file has a witness.
        if witnesses.is_empty() || ctx.memo::<PrototypeWriteVeto>(&())? {
            return Ok(Arc::default());
        }
        witnesses.sort_unstable();
        witnesses.dedup();
        Ok(Arc::new(witnesses))
    }

    fn on_cycle(_: &FileId) -> Arc<Vec<(Span, Span)>> {
        Arc::default()
    }
}

/// The arguments a wrapper function of one file forwards, unchanged, into a
/// slot the dialect runs on result access: `(caller file, argument,
/// primitive)`, in the order the lookup's walk finds them
/// ([`SemanticLookup::result_access_forwarded_arguments`]).
pub(crate) struct ForwardedFromIn;

impl Memo<BuildSchema> for ForwardedFromIn {
    type Key = FileId;
    type Value = Arc<Vec<(FileId, Span, Primitive)>>;
    const NAME: &'static str = "result_access_forwarded_from";
    const ID: TableId = Self::TABLE;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        Self::of(tables)
    }

    fn compute(
        ctx: &Ctx<'_, '_, BuildSchema>,
        id: &FileId,
    ) -> Result<Arc<Vec<(FileId, Span, Primitive)>>, Cycle> {
        let Some(read) = first(ctx, *id) else {
            return Ok(Arc::default());
        };
        let file = read.facts();
        if file.ast.calls.is_empty() {
            return Ok(Arc::default());
        }
        let primitives = ctx.memo::<CallPrimitives>(id)?;
        let dialect = ctx.dialect();
        let mut forwarded = Vec::new();
        for call in &file.ast.calls {
            let Some(primitive) = primitives.get(&call.span).copied() else {
                continue;
            };
            let count = call.arguments.len();
            for (index, argument) in call.arguments.iter().enumerate() {
                if argument.spread
                    || argument.value != solid_facts::ast::ArgumentValueKind::Identifier
                    || !dialect.callback_runs_on_result_access(primitive, index, count)
                {
                    continue;
                }
                let Some(declaration) = argument.binding_declaration else {
                    continue;
                };
                for function in file.ast.functions_body_containing(call.span) {
                    let Some(parameter) = function.parameters.iter().position(|binding| {
                        binding.names.len() == 1 && binding.names[0].span == declaration
                    }) else {
                        continue;
                    };
                    for site in ctx.memo::<CallSitesOf>(&(*id, function.span))?.iter() {
                        let Some(caller) = ctx.file(site.file, site.rank as usize) else {
                            continue;
                        };
                        let Some(found) = caller
                            .facts()
                            .ast
                            .calls
                            .iter()
                            .find(|candidate| candidate.callee == site.callee)
                        else {
                            continue;
                        };
                        // A spread up to the slot moves arguments into it.
                        if found
                            .arguments
                            .iter()
                            .take(parameter.saturating_add(1))
                            .any(|candidate| candidate.spread)
                        {
                            continue;
                        }
                        if let Some(forwarded_argument) = found.arguments.get(parameter) {
                            forwarded.push((site.file, forwarded_argument.span, primitive));
                        }
                    }
                }
            }
        }
        Ok(Arc::new(forwarded))
    }

    fn on_cycle(_: &FileId) -> Arc<Vec<(FileId, Span, Primitive)>> {
        Arc::default()
    }

    fn implied(id: &FileId) -> Vec<Input> {
        vec![Input::File(*id, Section::Source)]
    }
}

/// Every forwarded argument by caller file, each file's sorted by span with
/// one entry per argument, as the lookup's index holds them.
pub(crate) struct ResultAccessForwardedAll;

impl Memo<BuildSchema> for ResultAccessForwardedAll {
    type Key = ();
    type Value = Arc<BTreeMap<FileId, Vec<(Span, Primitive)>>>;
    const NAME: &'static str = "result_access_forwarded";
    const ID: TableId = Self::TABLE;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        Self::of(tables)
    }

    fn compute(
        ctx: &Ctx<'_, '_, BuildSchema>,
        _: &(),
    ) -> Result<Arc<BTreeMap<FileId, Vec<(Span, Primitive)>>>, Cycle> {
        // Most dialect variants answer no slot at all; skip the walk.
        if !ctx.memo::<ResultAccessAnswered>(&())? {
            return Ok(Arc::default());
        }
        let mut forwarded = BTreeMap::<FileId, Vec<(Span, Primitive)>>::new();
        for id in ctx.ids() {
            for (caller, argument, primitive) in ctx.memo::<ForwardedFromIn>(&id)?.iter() {
                forwarded
                    .entry(*caller)
                    .or_default()
                    .push((*argument, *primitive));
            }
        }
        for entries in forwarded.values_mut() {
            entries.sort_unstable_by_key(|(argument, _)| (argument.start, argument.end));
            entries.dedup_by_key(|(argument, _)| *argument);
        }
        Ok(Arc::new(forwarded))
    }

    fn on_cycle(_: &()) -> Arc<BTreeMap<FileId, Vec<(Span, Primitive)>>> {
        Arc::default()
    }
}

/// The entries an exact alias's references give the value it holds.
#[derive(Default)]
struct AliasEntries {
    /// A JSX tag names the alias: a component render.
    rendered: bool,
    /// Some reference is none of these, nor a call whose whole callee is the
    /// alias (read attribution follows those calls), a declaration, module
    /// surface, type query or a further exact alias.
    unfollowed: bool,
}

/// The component identity of every function of one slot of a file, by
/// ordinal: what its own file decides ([`local_identity`]), and for a
/// function whose direct JSX return needs the census of its references,
/// [`ComponentStatusAt`].
pub(crate) struct FileComponentStatuses;

impl Memo<BuildSchema> for FileComponentStatuses {
    type Key = (FileId, u32);
    type Value = Arc<Vec<ComponentIdentity>>;
    const NAME: &'static str = "component_functions";
    const ID: TableId = Self::TABLE;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        Self::of(tables)
    }

    fn compute(
        ctx: &Ctx<'_, '_, BuildSchema>,
        (id, rank): &(FileId, u32),
    ) -> Result<Arc<Vec<ComponentIdentity>>, Cycle> {
        let Some(read) = ctx.file(*id, *rank as usize) else {
            return Ok(Arc::default());
        };
        let file = read.facts();
        let mut identities = Vec::with_capacity(file.ast.functions.len());
        for (ordinal, function) in file.ast.functions.iter().enumerate() {
            identities.push(match local_identity(ctx, file, *id, function)? {
                Some(identity) => identity,
                None => ctx.memo::<ComponentStatusAt>(&(*id, *rank, ordinal32(ordinal)))?,
            });
        }
        Ok(Arc::new(identities))
    }

    fn on_cycle(_: &(FileId, u32)) -> Arc<Vec<ComponentIdentity>> {
        Arc::default()
    }

    fn implied((id, _): &(FileId, u32)) -> Vec<Input> {
        // What the file decides reads its syntax only.
        vec![Input::File(*id, Section::Source)]
    }
}

/// The component identity of one function that directly returns JSX: the
/// `ordinal`-th function of the `rank`-th slot of a file (ADR 0331 and its
/// successors; see [`SemanticLookup::function_component_status`]).
///
/// `Proven` or `No`; `Escaped` when only an escaping reference withholds the
/// direct-JSX-return proof; `Possible` when a call would make it `No` but the
/// census of its entries is incomplete.
///
/// Its reads: its own file (free), its file's tag answer and its call sites,
/// the references of its binding (the reference index's key, and each
/// referencing file with its per-file answers), the aliases those references
/// bind, and the adapters it consults. A file that does not reference the
/// function, call it or render it is not read.
pub(crate) struct ComponentStatusAt;

impl Memo<BuildSchema> for ComponentStatusAt {
    type Key = (FileId, u32, u32);
    type Value = ComponentIdentity;
    const NAME: &'static str = "component_census";
    const ID: TableId = Self::TABLE;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        Self::of(tables)
    }

    fn compute(
        ctx: &Ctx<'_, '_, BuildSchema>,
        (id, rank, ordinal): &(FileId, u32, u32),
    ) -> Result<ComponentIdentity, Cycle> {
        let Some(read) = ctx.file(*id, *rank as usize) else {
            return Ok(ComponentIdentity::No);
        };
        let file = read.facts();
        let Some(function) = file.ast.functions.get(*ordinal as usize) else {
            return Ok(ComponentIdentity::No);
        };
        if let Some(identity) = local_identity(ctx, file, *id, function)? {
            return Ok(identity);
        }
        component_census(ctx, &read, *id, function)
    }

    fn on_cycle(_: &(FileId, u32, u32)) -> ComponentIdentity {
        ComponentIdentity::Possible
    }

    fn implied((id, _, _): &(FileId, u32, u32)) -> Vec<Input> {
        // The binding's symbol is read (and recorded) as an entity row.
        vec![
            Input::File(*id, Section::Source),
            Input::File(*id, Section::Compiler),
        ]
    }
}

/// Whether `span` in `id` is a function value a compound holder stores and a
/// JSX member tag renders (ADR 0342).
fn compound_value(ctx: &Ctx<'_, '_, BuildSchema>, id: FileId, span: Span) -> Result<bool, Cycle> {
    Ok(ctx
        .memo::<super::compound::CompoundValuesIn>(&id)?
        .binary_search(&span)
        .is_ok())
}

/// Whether `span` in `id` is a route component value an audited router
/// renders (ADR 0334).
fn route_value(ctx: &Ctx<'_, '_, BuildSchema>, id: FileId, span: Span) -> Result<bool, Cycle> {
    Ok(ctx.memo::<RouteValues>(&id)?.binary_search(&span).is_ok())
}

/// Whether the binding name's compiler-resolved type is an exact Solid
/// component type alias.
fn has_component_type(
    ctx: &Ctx<'_, '_, BuildSchema>,
    id: FileId,
    name: Option<&solid_facts::ast::NamedSpan>,
) -> Result<bool, Cycle> {
    let Some(name) = name else {
        return Ok(false);
    };
    Ok(ctx
        .memo::<ComponentTypeNames>(&id)?
        .binary_search(&name.span)
        .is_ok())
}

/// What a function's own file decides of its identity: a tag renders it, an
/// audited router or a compound holder renders it as an anonymous value, or
/// it returns no JSX directly (then only an exact component type proves it).
/// `None` when a direct JSX return needs the census of its references.
fn local_identity(
    ctx: &Ctx<'_, '_, BuildSchema>,
    file: &FileFacts,
    id: FileId,
    function: &solid_facts::ast::FunctionFact,
) -> Result<Option<ComponentIdentity>, Cycle> {
    use ComponentIdentity::{No, Proven};
    if ctx
        .memo::<TagRendered>(&id)?
        .binary_search(&function.span)
        .is_ok()
    {
        return Ok(Some(Proven));
    }
    // ADR 0334/0335: a function written as an audited router's component
    // value is rendered there, and an anonymous inline value has no other
    // reference (a named function expression has its own name in its body,
    // which can hand it on: `invoke = Inline`).
    if function.name.is_none() && route_value(ctx, id, function.span)? {
        return Ok(Some(Proven));
    }
    // ADR 0342: an anonymous function stored in a compound component that
    // some JSX member tag renders has no other reference.
    if function.name.is_none() && compound_value(ctx, id, function.span)? {
        return Ok(Some(Proven));
    }
    // Every remaining runtime proof only decides whether a direct JSX return
    // is used as a component value or as an ordinary callback. Without such
    // a return, only an exact compiler-resolved Component type can still
    // establish component identity.
    if !directly_returns_jsx(file, function) {
        let binding_name = crate::owners::component_binding_name(file, function);
        return Ok(Some(
            if ctx.memo::<ProjectComponentType>(&())? && has_component_type(ctx, id, binding_name)?
            {
                Proven
            } else {
                No
            },
        ));
    }
    Ok(None)
}

/// Whether some return of `function` itself (not of a nested function)
/// directly holds JSX.
fn directly_returns_jsx(file: &FileFacts, function: &solid_facts::ast::FunctionFact) -> bool {
    let directly_contains_jsx = |span: Span| {
        file.ast.jsx_within(span).any(|element| {
            crate::owners::containing_ast_function(&file.ast, element.span)
                .is_some_and(|owner| owner.span == function.span)
        }) || file.ast.jsx_fragments.iter().any(|fragment| {
            span.contains(*fragment)
                && crate::owners::containing_ast_function(&file.ast, *fragment)
                    .is_some_and(|owner| owner.span == function.span)
        })
    };
    function
        .expression_return
        .as_ref()
        .and_then(|returned| returned.argument)
        .is_some_and(directly_contains_jsx)
        || file.ast.returns_within(function.body).any(|returned| {
            returned.argument.is_some_and(directly_contains_jsx)
                && crate::owners::containing_ast_function(&file.ast, returned.span)
                    .is_some_and(|owner| owner.span == function.span)
        })
}

/// The census of a function that directly returns JSX and that its own file
/// does not decide ([`local_identity`]).
#[allow(clippy::too_many_lines)]
fn component_census(
    ctx: &Ctx<'_, '_, BuildSchema>,
    read: &crate::base::memo::build::FileRead<'_, '_, '_>,
    id: FileId,
    function: &solid_facts::ast::FunctionFact,
) -> Result<ComponentIdentity, Cycle> {
    use ComponentIdentity::{Escaped, No, Possible, Proven};
    let file = read.facts();
    let dialect = ctx.dialect();
    let target = (id, function.span);
    let binding_name = crate::owners::component_binding_name(file, function);
    let directly_returns_jsx = true;
    // Component identity is about the function value itself, not anything
    // lexically nested below a callback expression. The largest function
    // inside a callback/argument is the direct value; nested declarations
    // remain independently classifiable.
    let direct_function_value = |candidate: &FileFacts, container: Span| {
        candidate
            .ast
            .functions_within(container)
            .max_by_key(|candidate| candidate.span.end - candidate.span.start)
            .is_some_and(|candidate| candidate.span == function.span)
    };
    let own_primitives = ctx.memo::<CallPrimitives>(&id)?;
    let executes_or_stores = |primitives: &BTreeMap<Span, Primitive>,
                              call: &solid_facts::ast::CallFact,
                              index: usize| {
        primitives
            .get(&call.span)
            .map(|primitive| dialect.callback_semantics_at(*primitive, index, call.arguments.len()))
            .is_some_and(|semantics| semantics.execution.is_some() || semantics.stores_as_value)
    };
    let used_as_callback_value = file
        .compiler
        .callback_roles
        .iter()
        .any(|role| direct_function_value(file, role.span))
        || file
            .ast
            .arguments_containing(function.span)
            .any(|(call, index)| {
                direct_function_value(file, call.arguments[index].span)
                    && executes_or_stores(&own_primitives, call, index)
            });
    let used_as_any_call_argument = file
        .ast
        .arguments_containing(function.span)
        .any(|(call, index)| direct_function_value(file, call.arguments[index].span));
    // A value written at a prop spelled `component` is analyzed as a
    // component value, as it always was. It renders as one only where the
    // dialect says the receiving export renders that prop (`<Dynamic
    // component={…}/>`, ADR 0138), resolved through its declaration: only
    // that accounts for the reference in the census below; a project
    // component's `component` prop is just a prop.
    let component_value_operation = |candidate: &FileFacts, span: Span| {
        candidate.compiler.jsx_operations.iter().any(|operation| {
            if operation.kind == "component-property" && operation.span.contains(span) {
                return candidate.ast.jsx_elements.iter().any(|element| {
                    element.attributes.iter().any(|attribute| {
                        attribute.value.is_some_and(|value| value.contains(span))
                            && candidate.source_text(attribute.local_name) == Some("component")
                    })
                });
            }
            operation.kind == "component-spread"
                && operation.span.contains(span)
                && candidate.ast.object_properties.iter().any(|property| {
                    operation.span.contains(property.span)
                        && property.value.contains(span)
                        && candidate.source_text(property.key) == Some("component")
                })
        })
    };
    let used_as_render_value = file.compiler.jsx_operations.iter().any(|operation| {
        matches!(
            operation.kind.as_str(),
            "component-property" | "component-spread" | "component-child"
        ) && operation.span.contains(function.span)
            && direct_function_value(file, operation.span)
            && !component_value_operation(file, function.span)
    });
    let mut referenced_as_callback = false;
    let mut referenced_as_component_property = false;
    // Solid runs a function as a component only when `createComponent` calls
    // it (a tag, a dialect renderer prop, `lazy`); called directly, its body
    // runs in the caller's scope, tracked inside JSX. A direct JSX return
    // therefore proves identity only when every reference to the function is
    // accounted for: its declaration, its module surface, a dialect callback
    // value, a dialect renderer's prop (ADR 0138), a dialect loader's
    // `default` (ADR 0292), an audited router's route option (ADR 0334) or a
    // member of a compound component that only JSX member tags reach (ADR
    // 0342). Any other use hands it to code that may call it plainly. So does
    // a function with no resolvable binding, a reference in a file not
    // analyzed, and an object or class method, reached through its object,
    // whose aliases its name's references do not enumerate. Each leaves
    // identity undecided rather than proven.
    let method =
        function.name.is_none() && crate::owners::own_method_name(file, function).is_some();
    let call_sites = ctx.memo::<CallSitesOf>(&target)?;
    let directly_called = !call_sites.is_empty();
    // ADR 0331: a visible call proves the function is not a component only
    // when it is every entry. So a called function's references are a census
    // too: each must be a resolved call site, a reference the test above
    // accounts for, a type query, or an exact alias whose own references are
    // calls, tags, type queries or surface.
    let call_references = call_sites
        .iter()
        .flat_map(|site| {
            std::iter::once((site.file, site.callee))
                .chain(site.also_referenced.map(|span| (site.file, span)))
        })
        .collect::<BTreeSet<_>>();
    let mut alias_entries = AliasEntries::default();
    let mut reference_escapes = true;
    let symbol = if method {
        None
    } else {
        binding_name.and_then(|name| read.symbol_at(name.span))
    };
    let references = match symbol {
        Some(symbol) => symbol_references(ctx, &symbol.shared())?,
        None => None,
    };
    if let Some(references) = references {
        reference_escapes = false;
        alias_entries.unfollowed = false;
        let mut lazy_veto = None;
        for reference in references {
            let Some((candidate_id, span)) = reference else {
                reference_escapes = true;
                alias_entries.unfollowed = true;
                continue;
            };
            let Some(candidate_read) = first(ctx, candidate_id) else {
                reference_escapes = true;
                alias_entries.unfollowed = true;
                continue;
            };
            let candidate = candidate_read.facts();
            let compiler = candidate_read.compiler();
            let callback = compiler.callback_roles.iter().any(|role| {
                role.span.contains(span)
                    && candidate.ast.functions_within(role.span).next().is_none()
            }) || {
                let mut callback = false;
                for (call, index) in candidate.ast.arguments_containing(span) {
                    if candidate
                        .ast
                        .functions_within(call.arguments[index].span)
                        .next()
                        .is_none()
                        && executes_or_stores(
                            &*ctx.memo::<CallPrimitives>(&candidate_id)?,
                            call,
                            index,
                        )
                    {
                        callback = true;
                        break;
                    }
                }
                callback
            };
            let component_value = component_value_operation(candidate, span);
            referenced_as_callback |= callback;
            referenced_as_component_property |= component_value;
            let escapes = !(callback && !component_value)
                && !ctx
                    .memo::<PropRenderSitesIn>(&candidate_id)?
                    .contains_key(&span)
                && !reference_is_declaration_or_module_surface(candidate, span)
                && !lazy_default_value(ctx, candidate_id, function.span, span, &mut lazy_veto)?
                && !route_value(ctx, candidate_id, span)?
                && !compound_value(ctx, candidate_id, span)?;
            reference_escapes |= escapes;
            if !escapes
                || call_references.contains(&(candidate_id, span))
                || super::components::reference_in_type_query(candidate, span)
            {
                continue;
            }
            match exact_alias_of_reference(ctx, &candidate_read, span)? {
                Some(alias) => alias_census(
                    ctx,
                    &alias,
                    super::components::ALIAS_DEPTH,
                    &mut alias_entries,
                )?,
                None => alias_entries.unfollowed = true,
            }
        }
    } else {
        alias_entries.unfollowed = true;
    }
    // An open program's unseen importers of an exported function are entries
    // no census enumerates (ADR 0331 treats the module surface as no entry
    // only for the proof that it *is* a component). A closed program has
    // none, but a visible namespace read can still hand the export out with
    // no reference naming it (ADR 0347): a computed or otherwise unnamed
    // escape, or a named member or dynamic-import read.
    if directly_called {
        let exported =
            binding_name.is_some_and(|name| super::components::binding_exported(file, name.span));
        let handed_out = if exported {
            !ctx.constant(Constant::ProgramClosed) || ctx.memo::<HandOuts>(&id)?.0
        } else {
            false
        } || ctx
            .memo::<HandOuts>(&id)?
            .1
            .binary_search(&function.span)
            .is_ok();
        if handed_out {
            alias_entries.unfollowed = true;
        }
    }
    let hoc_bound =
        binding_name.is_some() && crate::owners::function_binding_name(file, function).is_none();
    let direct_return_proof = directly_returns_jsx
        && (dialect.direct_jsx_return_is_component()
            || hoc_bound
            || referenced_as_component_property)
        && !directly_called
        && (!used_as_callback_value || hoc_bound)
        && (!used_as_any_call_argument || hoc_bound)
        && !used_as_render_value
        && (!referenced_as_callback || referenced_as_component_property);
    Ok(
        if direct_return_proof && !reference_escapes || has_component_type(ctx, id, binding_name)? {
            Proven
        } else if direct_return_proof {
            Escaped
        } else if directly_called && (alias_entries.unfollowed || alias_entries.rendered) {
            // ADR 0331: called, but entered some way the census cannot
            // follow, so the call is no proof of every entry. (An alias tag
            // made it a component above, through the tag answer.)
            Possible
        } else {
            No
        },
    )
}

/// Whether `reference` is the `default` value of a module object a dialect
/// component loader renders, naming the function at `function` (ADR 0292):
/// that render is the reference's only use. A reflective prototype write
/// anywhere vetoes it, as it vetoes the render witness.
fn lazy_default_value(
    ctx: &Ctx<'_, '_, BuildSchema>,
    id: FileId,
    function: Span,
    reference: Span,
    veto: &mut Option<bool>,
) -> Result<bool, Cycle> {
    let named = ctx
        .memo::<LazyCandidatesIn>(&id)?
        .iter()
        .any(|(_, component, _, value)| *component == function && *value == reference);
    if !named {
        return Ok(false);
    }
    let vetoed = match veto {
        Some(vetoed) => *vetoed,
        None => *veto.insert(ctx.memo::<PrototypeWriteVeto>(&())?),
    };
    Ok(!vetoed)
}

/// The symbol of the exact alias binding whose initializer is the reference
/// at `span`: `const Alias = Value`.
fn exact_alias_of_reference(
    ctx: &Ctx<'_, '_, BuildSchema>,
    read: &crate::base::memo::build::FileRead<'_, '_, '_>,
    span: Span,
) -> Result<Option<Arc<str>>, Cycle> {
    let file = read.facts();
    let Some(binding) = file.ast.bindings.iter().find(|binding| {
        binding
            .initializer_identifier
            .as_ref()
            .is_some_and(|identifier| identifier.span == span)
    }) else {
        return Ok(None);
    };
    if super::components::exact_alias_initializer(file, binding).is_none() {
        return Ok(None);
    }
    // An open program's unseen importers may use an exported alias any way at
    // all; that is no exact alias of anything we can follow.
    if !ctx.constant(Constant::ProgramClosed)
        && super::components::binding_exported(file, binding.names[0].span)
    {
        return Ok(None);
    }
    let Some(symbol) = read.symbol_at(binding.names[0].span) else {
        return Ok(None);
    };
    let symbol = symbol.shared();
    Ok((!matches!(
        ctx.memo::<SymbolReferencesOf>(&symbol)?,
        SymbolReferences::Unknown
    ))
    .then_some(symbol))
}

/// How the references of an exact alias binding enter the value it holds
/// (ADR 0331), transitively through aliases of the alias.
fn alias_census(
    ctx: &Ctx<'_, '_, BuildSchema>,
    alias: &Arc<str>,
    depth: usize,
    entries: &mut AliasEntries,
) -> Result<(), Cycle> {
    let Some(references) = symbol_references(ctx, alias)? else {
        entries.unfollowed = true;
        return Ok(());
    };
    if depth == 0 {
        entries.unfollowed = true;
        return Ok(());
    }
    for reference in references {
        let Some((candidate_id, span)) = reference else {
            entries.unfollowed = true;
            continue;
        };
        let Some(candidate_read) = first(ctx, candidate_id) else {
            entries.unfollowed = true;
            continue;
        };
        let candidate = candidate_read.facts();
        if reference_is_declaration_or_module_surface(candidate, span)
            || super::components::reference_in_type_query(candidate, span)
        {
            continue;
        }
        if candidate
            .ast
            .jsx_elements
            .iter()
            .any(|element| element.name.span == span || element.closing_name == Some(span))
        {
            entries.rendered = true;
            continue;
        }
        if candidate.ast.calls.iter().any(|call| call.callee == span) {
            continue;
        }
        if let Some(next) = exact_alias_of_reference(ctx, &candidate_read, span)? {
            alias_census(ctx, &next, depth - 1, entries)?;
            continue;
        }
        entries.unfollowed = true;
    }
    Ok(())
}

/// A file's functions by span, with the symbols naming them.
type FunctionSymbolsBySpan = BTreeMap<Span, Vec<Arc<str>>>;

/// The lookup's side of the census: the unported tables the adapters read,
/// re-keyed once per build, and the answers its consumers read whole.
#[derive(Default)]
pub(super) struct State<'a> {
    /// `functions_by_symbol` inverted: each path's functions by span, with
    /// the symbols naming them, sorted.
    function_symbols: OnceLock<HashMap<&'a str, FunctionSymbolsBySpan>>,
    /// Symbol ids the TypeScript table lists more than once.
    duplicate_symbols: OnceLock<HashSet<&'a str>>,
    route_values: OnceLock<HashMap<FileId, Vec<Span>>>,
}

impl<'a> State<'a> {
    fn function_symbols(
        &self,
        lookup: &SemanticLookup<'a>,
    ) -> &HashMap<&'a str, FunctionSymbolsBySpan> {
        self.function_symbols.get_or_init(|| {
            let files = lookup.files();
            let mut inverted = HashMap::<&'a str, FunctionSymbolsBySpan>::default();
            for (symbol, function) in lookup.functions_by_symbol() {
                if let SymbolFunction::Resolved { file, function } = function {
                    let file = &files[*file];
                    inverted
                        .entry(file.path.as_str())
                        .or_default()
                        .entry(file.ast.functions[*function].span)
                        .or_default()
                        .push(Arc::from(*symbol));
                }
            }
            for functions in inverted.values_mut() {
                for symbols in functions.values_mut() {
                    symbols.sort_unstable();
                    symbols.dedup();
                }
            }
            inverted
        })
    }

    fn symbol_references(
        &self,
        lookup: &SemanticLookup<'a>,
        table: &FileTable,
        symbol: &str,
    ) -> SymbolReferences {
        let Some(candidate) = lookup.symbols_by_id().get(symbol) else {
            return SymbolReferences::Unknown;
        };
        let duplicates = self.duplicate_symbols.get_or_init(|| {
            let mut seen = HashSet::default();
            let mut duplicates = HashSet::default();
            for candidate in lookup.facts.typescript.symbols() {
                if !seen.insert(candidate.id()) {
                    duplicates.insert(candidate.id());
                }
            }
            duplicates
        });
        let analyzed = |reference: &typefacts::Location| {
            let id = table.id_of(reference.path.as_ref())?;
            let start = u32::try_from(reference.start_byte).ok()?;
            let end = u32::try_from(reference.end_byte).ok()?;
            Some((id, Span::new(start, end)))
        };
        if duplicates.contains(symbol) {
            return SymbolReferences::Listed(Arc::new(
                candidate.references().map(analyzed).collect(),
            ));
        }
        SymbolReferences::Indexed {
            foreign: candidate
                .references()
                .any(|reference| analyzed(reference).is_none()),
        }
    }

    fn route_values(
        &self,
        lookup: &SemanticLookup<'a>,
        table: &FileTable,
        id: FileId,
    ) -> Arc<Vec<Span>> {
        let values = self.route_values.get_or_init(|| {
            by_file(
                table,
                lookup
                    .route_component_values()
                    .iter()
                    .map(|(path, span)| (path.as_str(), *span)),
            )
        });
        Arc::new(values.get(&id).cloned().unwrap_or_default())
    }
}

/// `(path, span)` pairs by the path's id, each file's sorted.
fn by_file<'p>(
    table: &FileTable,
    values: impl Iterator<Item = (&'p str, Span)>,
) -> HashMap<FileId, Vec<Span>> {
    let mut by_file = HashMap::<FileId, Vec<Span>>::default();
    for (path, span) in values {
        if let Some(id) = table.id_of(path) {
            by_file.entry(id).or_default().push(span);
        }
    }
    for spans in by_file.values_mut() {
        spans.sort_unstable();
        spans.dedup();
    }
    by_file
}
