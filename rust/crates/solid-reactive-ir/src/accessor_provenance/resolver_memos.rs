//! The resolver on memos (S7b; S7 design §3): its stages as per-file memos
//! with recorded reads, folded on kept state.
//!
//! A build ([`build`]) runs the stages of [`super::resolver_index`] in order,
//! each as a memo per file over the memo world ([`MemoWorld`]), and folds
//! their rows in path order on kept folds (K1), publishing each table the
//! next stage reads per key ([`crate::base::memo::derived`]):
//! 1. the build's accessors and source kinds, published as they are, and
//!    what the walks ask of the lookup about each file's spans and calls
//!    ([`FileQuestions`]: binding resolutions, call primitives, context
//!    provider targets, iteration receivers), computed by the pass and
//!    published per file;
//! 2. [`ResolverBindings`] and [`ResolverParameters`], folded into the
//!    binding locations (last wins; a parameter key overwrites);
//! 3. [`ResolverCensus`], folded into the direct use census (a join) and the
//!    providers (in file order); its handoff edges solved on the kept K2
//!    handoff solver, which publishes [`HandoffUses`];
//! 4. [`ResolverOrigins`] (the origins and blocked contexts, over a world
//!    with no blocked context), reading [`ResolverObjectContext`] per context.
//!
//! A walk is recorded into the memo it runs in; only the stages' top-level
//! answers are memos. A question of a table not ported yet is published by
//! the pass (a reader is dirtied only when its key moves), a probe, or, for
//! a parameter channel's Type Facts half and a vetoed dependency member, an
//! adapter memo whose computation is one [`Ctx::project`] read (recomputed
//! in every build in which the program moved, cut off when equal: its
//! readers are verified in every such build).
//!
//! A build whose files list a path twice, or whose symbol names are not the
//! lookup's, builds fresh over the lookup ([`Resolver::fresh`]); so does one
//! in which a memo met a cycle. In shadow and verify-reused modes the memo
//! build is compared with the fresh one; a difference names the first
//! differing table and key, poisons the store, and the fresh one is served.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::base::fixpoint::{FileOrder, KeyedFold, Lattice};
use crate::base::memo::build::{BuildSchema, BuildTables, Constant, Input, Section};
use crate::base::memo::derived::{Derived, DerivedId, SolverId};
use crate::base::memo::{Canonical, Ctx, Cycle, ErasedTable, Memo, Table, TableId};
use crate::base::{FileId, FileTable, Hazard};

/// The resolver's memo tables hold ids 240 to 255.
pub(crate) const RESOLVER_TABLES_FROM: u8 = 240;

macro_rules! resolver_tables {
    ($($field:ident: $memo:ty = $id:literal;)*) => {
        /// The resolver's memo tables, inside the build's
        /// ([`crate::base::memo::build::BuildTables`]).
        #[derive(Default)]
        pub(crate) struct ResolverTables {
            $($field: Table<BuildSchema, $memo>,)*
        }

        impl ResolverTables {
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
            const TABLE: TableId = TableId($id);

            fn of(tables: &BuildTables) -> &Table<BuildSchema, Self> {
                &tables.resolver().$field
            }
        })*
    };
}

resolver_tables! {
    bindings: ResolverBindings = 240;
    parameters: ResolverParameters = 241;
    census: ResolverCensus = 242;
    origins: ResolverOrigins = 243;
    object_context: ResolverObjectContext = 244;
    member_stable: DependencyMemberStable = 245;
    parameter_typefacts: ParameterTypefacts = 246;
}

impl Canonical for BindingRows {}
impl Canonical for ParameterRows {}
impl Canonical for CensusRows {}
impl Canonical for OriginRows {}

/// A location as a canonical value or key.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Site(pub(crate) Location);

impl Canonical for Site {}

impl Ord for Site {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        location_order(&self.0).cmp(&location_order(&other.0))
    }
}

impl PartialOrd for Site {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// A source kind as a canonical value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Kind(crate::ReactiveSourceKind);

impl Canonical for Kind {}

/// One file's binding resolutions as the lookup holds them
/// (`bindings_by_reference`): `((start, end), (file, slot rank, binding
/// ordinal, symbol))`, sorted by span.
type Resolutions = crate::indexes::BindingResolutionRows;

/// A context provider's rendered value: its file and span.
type Provider = Option<(String, Span)>;

/// The function a symbol names: its file, slot rank and ordinal.
type FunctionAt = Option<(FileId, u32, u32)>;

// The derived tables: what the resolver's passes publish to its memos.

/// The build's accessors (source discovery), by symbol: the declaration.
struct Accessors;

impl Derived for Accessors {
    type Key = SymbolId;
    type Value = Site;
    const NAME: &'static str = "resolver_accessors";
    const ID: DerivedId = DerivedId(240);
}

/// The build's source kinds (source discovery), by symbol.
struct SourceKinds;

impl Derived for SourceKinds {
    type Key = SymbolId;
    type Value = Kind;
    const NAME: &'static str = "resolver_source_kinds";
    const ID: DerivedId = DerivedId(241);
}

/// Each symbol's binding location (stage 2's fold).
struct BindingLocations;

impl Derived for BindingLocations {
    type Key = SymbolId;
    type Value = Site;
    const NAME: &'static str = "resolver_binding_locations";
    const ID: DerivedId = DerivedId(242);
}

/// Each binding key's direct use census (stage 3's fold, before handoffs).
struct DirectUses;

impl Derived for DirectUses {
    type Key = Location;
    type Value = ClosedUses;
    const NAME: &'static str = "resolver_direct_uses";
    const ID: DerivedId = DerivedId(243);
}

/// Each context's providers in file order (stage 3's fold).
struct Providers;

impl Derived for Providers {
    type Key = Location;
    type Value = Arc<Vec<Option<(String, Span)>>>;
    const NAME: &'static str = "resolver_providers";
    const ID: DerivedId = DerivedId(244);
}

/// Whether some provider is unkeyed, and whether some context has one.
struct ProviderFlags;

impl Derived for ProviderFlags {
    type Key = ();
    type Value = (bool, bool);
    const NAME: &'static str = "resolver_provider_flags";
    const ID: DerivedId = DerivedId(245);
}

/// The resolver's kept state between builds.
const RESOLVER_STATE: SolverId = SolverId(240);

/// The answer that proves nothing: a stage memo that met a cycle has none,
/// and the build falls back to the fresh construction.
type Rows<T> = Option<Arc<T>>;

macro_rules! stage_memo {
    (
        $(#[$doc:meta])*
        $name:ident: $key:ty => $rows:ty, $label:literal,
        |$world:ident, $k:ident| $body:expr
    ) => {
        $(#[$doc])*
        pub(super) struct $name;

        impl Memo<BuildSchema> for $name {
            type Key = $key;
            type Value = Rows<$rows>;
            const NAME: &'static str = $label;
            const ID: TableId = Self::TABLE;

            fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
                Self::of(tables)
            }

            fn compute(ctx: &Ctx<'_, '_, BuildSchema>, key: &$key) -> Result<Rows<$rows>, Cycle> {
                let $world = MemoWorld::new(ctx);
                let $k = key;
                let rows: $rows = $body;
                $world.finish(Some(Arc::new(rows)))
            }

            fn on_cycle(_: &$key) -> Rows<$rows> {
                None
            }

            fn implied(key: &$key) -> Vec<Input> {
                stage_implied(key)
            }
        }
    };
}

/// The inputs a stage memo's key names: its file's own source and entities.
fn stage_implied(id: &FileId) -> Vec<Input> {
    vec![
        Input::File(*id, Section::Source),
        Input::File(*id, Section::Entities),
    ]
}

stage_memo! {
    /// Stage 2: a file's [`BindingRows`].
    ResolverBindings: FileId => BindingRows, "resolver_bindings",
    |world, id| world.read(*id, 0).map(|file| binding_rows(&world, file)).unwrap_or_default()
}

stage_memo! {
    /// Stage 2: a file's [`ParameterRows`].
    ResolverParameters: FileId => ParameterRows, "resolver_parameters",
    |world, id| world.read(*id, 0).map(|file| parameter_rows(&world, file)).unwrap_or_default()
}

stage_memo! {
    /// Stage 3: a file's [`CensusRows`], with its parameter handoffs.
    ResolverCensus: FileId => CensusRows, "resolver_census",
    |world, id| match (world.read(*id, 0), world.parameters(*id)) {
        (Some(file), Some(parameters)) => {
            let forwarded = forwarded_arguments(&parameters);
            let mut rows = census_rows(&world, file, &|span| forwarded.contains(&span));
            rows.edges.extend(parameter_edges(&world, file, &parameters));
            rows
        }
        _ => CensusRows::default(),
    }
}

stage_memo! {
    /// Stage 4: a file's [`OriginRows`].
    ResolverOrigins: FileId => OriginRows, "resolver_origins",
    |world, id| match (world.read(*id, 0), world.parameters(*id)) {
        (Some(file), Some(parameters)) => {
            let forwarded = forwarded_arguments(&parameters);
            origin_rows(
                &world,
                file,
                &|span| forwarded.contains(&span),
                &mut |context| world.object_context(context),
            )
        }
        _ => OriginRows::default(),
    }
}

fn forwarded_arguments(parameters: &ParameterRows) -> HashSet<Span> {
    parameters
        .inputs
        .iter()
        .map(|(_, argument)| *argument)
        .collect()
}

/// Stage 4: [`object_context`] for one context.
pub(super) struct ResolverObjectContext;

impl Memo<BuildSchema> for ResolverObjectContext {
    type Key = Site;
    type Value = Option<bool>;
    const NAME: &'static str = "resolver_object_context";
    const ID: TableId = Self::TABLE;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        Self::of(tables)
    }

    fn compute(ctx: &Ctx<'_, '_, BuildSchema>, context: &Site) -> Result<Option<bool>, Cycle> {
        let world = MemoWorld::new(ctx);
        let object = object_context(&world, &context.0);
        world.finish(Some(object))
    }

    fn on_cycle(_: &Site) -> Option<bool> {
        None
    }
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
        pub(super) struct $name;

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

/// What the census and the walks ask of the lookup about one file's
/// spans and calls, in one adapter: the binding resolutions of its spans
/// (`bindings_by_reference`, sorted), the dialect primitive of each call by
/// ordinal (`call_primitive_name` over the lookup's symbol names), its JSX
/// provider targets that are context references (`is_context_reference`,
/// sorted), and the built-in `map`/`forEach` receiver of each call that has
/// one ([`array_iteration_receiver`]), by ordinal.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct FileQuestions {
    resolutions: Resolutions,
    primitives: Vec<Option<solid_dialect::Primitive>>,
    context_targets: Vec<Span>,
    array_receivers: Vec<(u32, Span)>,
}

impl Canonical for FileQuestions {}

/// The span a JSX element provides a context through, if it can.
fn provider_target(
    dialect: &dyn solid_dialect::Dialect,
    file: &FileFacts,
    element: &ast::JsxElementFact,
) -> Option<Span> {
    if let Some(member) = dialect.context_provider_member() {
        (element.member_property.and_then(|p| file.source_text(p)) == Some(member))
            .then_some(element.member_object)
            .flatten()
    } else {
        Some(element.name.span)
    }
}

/// Whether `call` may have a built-in iteration receiver: its callee is a
/// non-computed member named `map` or `forEach`, which
/// [`array_iteration_receiver`] requires before it asks the signature.
fn may_iterate(file: &FileFacts, call: &ast::CallFact) -> bool {
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    !call.construct
        && file
            .ast
            .member_at(callee)
            .and_then(|member| file.source_text(member.property))
            .is_some_and(|property| matches!(property, "map" | "forEach"))
        && !file.ast.computed_members.contains(&callee)
}

fn file_questions(lookup: &SemanticLookup<'_>, file: &FileFacts) -> FileQuestions {
    let mut context_targets = file
        .ast
        .jsx_elements
        .iter()
        .filter_map(|element| provider_target(lookup.dialect, file, element))
        .filter(|target| lookup.is_context_reference(file.path.as_str(), *target))
        .collect::<Vec<_>>();
    context_targets.sort_unstable();
    context_targets.dedup();
    FileQuestions {
        resolutions: lookup.binding_resolution_rows(file.path.as_str()),
        primitives: file
            .ast
            .calls
            .iter()
            .map(|call| {
                crate::call_primitive_name_cached(
                    lookup,
                    file,
                    call,
                    lookup.entities(),
                    lookup.symbol_names(),
                    lookup.dialect,
                )
                .and_then(|name| name.primitive())
            })
            .collect(),
        context_targets,
        array_receivers: file
            .ast
            .calls
            .iter()
            .enumerate()
            .filter(|(_, call)| may_iterate(file, call))
            .filter_map(|(ordinal, call)| {
                let receiver = array_iteration_receiver(file, call, lookup)?;
                Some((
                    u32::try_from(ordinal).expect("fewer than 2^32 calls"),
                    receiver,
                ))
            })
            .collect(),
    }
}

/// Each file's [`FileQuestions`], computed by the pass from the lookup and
/// published per file: a reader is dirtied only when its files' answers
/// move, not in every build in which the program did.
struct FileQuestionsOf;

impl Derived for FileQuestionsOf {
    type Key = FileId;
    type Value = Arc<FileQuestions>;
    const NAME: &'static str = "resolver_file_questions";
    const ID: DerivedId = DerivedId(246);
}

/// Whether a visible mutation may change the array iteration protocol
/// (member stability's `tuple_iteration`), published as one key.
struct TupleIteration;

impl Derived for TupleIteration {
    type Key = ();
    type Value = bool;
    const NAME: &'static str = "resolver_tuple_iteration";
    const ID: DerivedId = DerivedId(247);
}

adapter! {
    /// [`dependency_member_stable`] for a member of an object literal.
    DependencyMemberStable: (FileId, Span, Arc<str>) => bool, "resolver_dependency_member_stable",
    cycle: false,
    |lookup, table, key| {
        let (id, object, name) = key;
        dependency_member_stable(lookup, lookup.member_stability(), table.path_of(*id), *object, name)
    }
}

/// A parameter channel's function (file, ordinal), its name, and its one
/// call (caller file, call ordinal, callee).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct Channel(FileId, u32, Span, FileId, u32, Span);

impl Canonical for Channel {}

adapter! {
    /// [`parameter_typefacts_admit_in`] for a parameter channel.
    ParameterTypefacts: Channel => bool, "resolver_parameter_typefacts",
    cycle: false,
    |lookup, table, key| {
        let Channel(file, function, name, caller, call, callee) = *key;
        let (Some(file), Some(caller)) = (path_file(lookup, table, file), path_file(lookup, table, caller)) else {
            return false;
        };
        match (file.ast.functions.get(function as usize), caller.ast.calls.get(call as usize)) {
            (Some(function), Some(call)) => {
                parameter_typefacts_admit_in(lookup, file, function, name, caller, call, callee)
            }
            _ => false,
        }
    }
}

/// The position of `item` in `items`, when it is one of them (by address).
fn ordinal_in<T>(items: &[T], item: &T) -> Option<usize> {
    let size = std::mem::size_of::<T>();
    let base = items.as_ptr() as usize;
    let address = std::ptr::from_ref(item) as usize;
    if size == 0 || address < base {
        return None;
    }
    let offset = address - base;
    (offset.is_multiple_of(size) && offset / size < items.len()).then_some(offset / size)
}

/// The resolver's world in a memo computation: every answer a recorded
/// read, each made once per memo where a walk repeats it.
pub(super) struct MemoWorld<'r, 'c, 'w: 'c> {
    ctx: &'r Ctx<'c, 'w, BuildSchema>,
    /// The files the walk holds: by path, and each read through the context.
    ids: RefCell<HashMap<&'w str, FileId>>,
    files: RefCell<BTreeMap<(FileId, u32), Option<&'w FileFacts>>>,
    paths: RefCell<HashMap<String, Option<&'w FileFacts>>>,
    symbols: RefCell<HashMap<(FileId, Span), Option<&'w SymbolId>>>,
    questions: RefCell<BTreeMap<FileId, Arc<FileQuestions>>>,
    parameters: RefCell<BTreeMap<FileId, Option<Arc<ParameterRows>>>>,
    functions: RefCell<HashMap<String, FunctionAt>>,
    uses: RefCell<HashMap<Location, Option<ClosedUses>>>,
    binding_locations: RefCell<HashMap<SymbolId, Option<Location>>>,
    /// The first cycle a memo read met: the computation fails closed.
    cycle: RefCell<Option<Cycle>>,
}

impl<'r, 'c, 'w: 'c> MemoWorld<'r, 'c, 'w> {
    fn new(ctx: &'r Ctx<'c, 'w, BuildSchema>) -> Self {
        Self {
            ctx,
            ids: RefCell::default(),
            files: RefCell::default(),
            paths: RefCell::default(),
            symbols: RefCell::default(),
            questions: RefCell::default(),
            parameters: RefCell::default(),
            functions: RefCell::default(),
            uses: RefCell::default(),
            binding_locations: RefCell::default(),
            cycle: RefCell::default(),
        }
    }

    /// The `rank`-th slot of `id`, read (its source).
    fn read(&self, id: FileId, rank: u32) -> Option<&'w FileFacts> {
        if let Some(file) = self.files.borrow().get(&(id, rank)) {
            return *file;
        }
        let file = self.ctx.file(id, rank as usize).map(|read| read.facts());
        if let Some(file) = file {
            self.ids.borrow_mut().insert(file.path.as_str(), id);
        }
        self.files.borrow_mut().insert((id, rank), file);
        file
    }

    /// The id of a file the walk holds.
    fn id(&self, file: &FileFacts) -> Option<FileId> {
        self.ids.borrow().get(file.path.as_str()).copied()
    }

    fn memo<M: Memo<BuildSchema>>(&self, key: &M::Key) -> Option<M::Value> {
        match self.ctx.memo::<M>(key) {
            Ok(value) => Some(value),
            Err(cycle) => {
                self.cycle.borrow_mut().get_or_insert(cycle);
                None
            }
        }
    }

    /// A file's parameter rows (stage 2).
    fn parameters(&self, id: FileId) -> Option<Arc<ParameterRows>> {
        if let Some(rows) = self.parameters.borrow().get(&id) {
            return rows.clone();
        }
        let rows = self.memo::<ResolverParameters>(&id).flatten();
        if rows.is_none() {
            // A stage answer that proves nothing fails the reader closed.
            self.fail();
        }
        self.parameters.borrow_mut().insert(id, rows.clone());
        rows
    }

    fn object_context(&self, context: &Location) -> bool {
        match self
            .memo::<ResolverObjectContext>(&Site(context.clone()))
            .flatten()
        {
            Some(object) => object,
            None => {
                self.fail();
                true
            }
        }
    }

    /// What the lookup answers about `id`'s spans and calls.
    fn questions(&self, id: FileId) -> Arc<FileQuestions> {
        if let Some(questions) = self.questions.borrow().get(&id) {
            return Arc::clone(questions);
        }
        let questions = self.ctx.derived::<FileQuestionsOf>(&id).unwrap_or_default();
        self.questions
            .borrow_mut()
            .insert(id, Arc::clone(&questions));
        questions
    }

    fn function_for_symbol(&self, symbol: &str) -> Option<(&'w FileFacts, &'w ast::FunctionFact)> {
        let cached = self.functions.borrow().get(symbol).copied();
        let found = cached.unwrap_or_else(|| {
            let found = self.ctx.function_for_symbol(symbol);
            self.functions.borrow_mut().insert(symbol.to_owned(), found);
            found
        });
        let (id, rank, ordinal) = found?;
        let file = self.read(id, rank)?;
        Some((file, file.ast.functions.get(ordinal as usize)?))
    }

    /// Marks the computation failed: a stage it read proves nothing.
    fn fail(&self) {
        self.cycle.borrow_mut().get_or_insert(Cycle {
            path: vec!["resolver stage without an answer".to_owned()],
        });
    }

    /// `value`, unless a read met a cycle.
    fn finish<T>(self, value: T) -> Result<T, Cycle> {
        match self.cycle.into_inner() {
            Some(cycle) => Err(cycle),
            None => Ok(value),
        }
    }
}

impl<'w> World<'w> for MemoWorld<'_, '_, 'w> {
    fn dialect(&self) -> &'w dyn solid_dialect::Dialect {
        self.ctx.dialect()
    }

    fn program_closed(&self) -> bool {
        self.ctx.constant(Constant::ProgramClosed)
    }

    fn file(&self, path: &str) -> Option<&'w FileFacts> {
        if let Some(file) = self.paths.borrow().get(path) {
            return *file;
        }
        let file = self
            .ctx
            .path_file(path)
            .and_then(|(id, rank)| self.read(id, rank));
        self.paths.borrow_mut().insert(path.to_owned(), file);
        file
    }

    fn symbol_at(&self, file: &'w FileFacts, span: Span) -> Option<&'w SymbolId> {
        let id = self.id(file)?;
        if let Some(symbol) = self.symbols.borrow().get(&(id, span)) {
            return *symbol;
        }
        let symbol = self.ctx.file(id, 0).and_then(|read| read.symbol_at(span));
        self.symbols.borrow_mut().insert((id, span), symbol);
        symbol
    }

    fn binding_at_reference(
        &self,
        file: &'w FileFacts,
        span: Span,
    ) -> Option<(&'w FileFacts, &'w ast::BindingFact, SymbolId)> {
        let id = self.id(file)?;
        let questions = self.questions(id);
        let rows = &questions.resolutions;
        let key = |span: Span| (u64::from(span.start), u64::from(span.end));
        let at = |span: Span| {
            rows.binary_search_by_key(&key(span), |(key, _)| *key)
                .ok()
                .map(|index| &rows[index].1)
        };
        // As `SemanticLookup::binding_resolution_at`: a missed wrapped span
        // resolves as the one identifier it peels to.
        let resolution = at(span).or_else(|| {
            let peeled = file.ast.peel_ts_sugar_span(span);
            let identifier = file
                .ast
                .identifiers
                .binary_search_by_key(&peeled, |id| id.span)
                .is_ok()
                || file.ast.reference_declaration(peeled).is_some();
            (peeled != span && identifier).then(|| at(peeled)).flatten()
        })?;
        let (bound, rank, binding, symbol) = resolution.clone();
        let bound = self.read(bound, rank)?;
        Some((bound, bound.ast.bindings.get(binding as usize)?, symbol))
    }

    fn callee_function(
        &self,
        file: &'w FileFacts,
        span: Span,
    ) -> Option<(&'w FileFacts, &'w ast::FunctionFact)> {
        let id = self.id(file)?;
        // Runtime redirects (ADR 0322) can rename a member callee of a
        // namespace import: under them the lookup answers.
        if self.ctx.constant(Constant::RuntimeRedirects) {
            let found = self.ctx.project_files(|lookup, table| {
                let file = path_file(lookup, table, id)?;
                let (target, function) = lookup
                    .callee_symbol(file, span)
                    .and_then(|symbol| lookup.function_for_symbol(symbol))?;
                let ordinal = ordinal_in(&target.ast.functions, function)?;
                let (target, rank) = lookup.memo_slot_of(table.last_slot(target.path.as_str())?);
                Some((target, rank, u32::try_from(ordinal).ok()?))
            });
            let (target, rank, ordinal) = found?;
            let target = self.read(target, rank)?;
            return Some((target, target.ast.functions.get(ordinal as usize)?));
        }
        let symbol = self.ctx.file(id, 0)?.callee_symbol(span)?;
        self.function_for_symbol(symbol)
    }

    fn function_called_at(
        &self,
        file: &'w FileFacts,
        callee: Span,
    ) -> Option<(&'w FileFacts, &'w ast::FunctionFact)> {
        let id = self.id(file)?;
        let symbol = self.ctx.file(id, 0)?.value_symbol(callee)?;
        self.function_for_symbol(symbol)
    }

    fn namespace_member_function(
        &self,
        file: &'w FileFacts,
        callee: Span,
    ) -> Option<(&'w FileFacts, &'w ast::FunctionFact)> {
        // As `SemanticLookup::namespace_member_function`.
        let callee = file.ast.peel_ts_sugar_span(callee);
        if file.ast.computed_members.binary_search(&callee).is_ok() {
            return None;
        }
        let member = file.ast.member_at(callee)?;
        let receiver = file.ast.peel_ts_sugar_span(member.object);
        let declaration = file.ast.reference_declaration(receiver)?;
        let namespace = file.ast.imports.iter().any(|import| {
            !import.type_only
                && import.bindings.iter().any(|binding| {
                    !binding.type_only
                        && binding.kind == ast::ImportKind::Namespace
                        && binding.local.span == declaration
                })
        });
        if !namespace {
            return None;
        }
        let symbol = self.symbol_at(file, member.property)?;
        self.function_for_symbol(symbol)
            .filter(|(target_file, function)| {
                function.method_name.is_none()
                    && crate::indexes::function_value_is_current_in(target_file, function)
            })
    }

    fn primitive(
        &self,
        file: &'w FileFacts,
        call: &'w ast::CallFact,
    ) -> Option<solid_dialect::Primitive> {
        let id = self.id(file)?;
        let ordinal = ordinal_in(&file.ast.calls, call)?;
        self.questions(id)
            .primitives
            .get(ordinal)
            .copied()
            .flatten()
    }

    fn binding_has_write(&self, file: &'w FileFacts, symbol: &SymbolId) -> bool {
        // As `value_identity::binding_has_write`.
        file.ast.assignments.iter().any(|assignment| {
            let span = file.ast.peel_ts_sugar_span(assignment.target);
            file.ast
                .reference_declaration(span)
                .and_then(|declaration| self.symbol_at(file, declaration))
                .or_else(|| self.symbol_at(file, span))
                == Some(symbol)
        })
    }

    fn slot_closure(&self, file: &'w FileFacts, factory: Span, element: Span) -> Option<Span> {
        // As [`slot_closure`].
        let (arrow, declaration) = slot_arrow(file, factory, element)?;
        if let Some(declaration) = declaration {
            let symbol = self.symbol_at(file, declaration)?;
            if self.binding_has_write(file, symbol) {
                return None;
            }
        }
        Some(arrow)
    }

    fn source_has_runtime_witness(&self, symbol: &SymbolId) -> bool {
        // As [`source_has_runtime_witness`].
        let Some(Site(declaration)) = self.ctx.derived::<Accessors>(symbol) else {
            return false;
        };
        let Some(file) = self.file(declaration.path.as_ref()) else {
            return false;
        };
        let (Ok(start), Ok(end)) = (
            u32::try_from(declaration.start_byte),
            u32::try_from(declaration.end_byte),
        ) else {
            return false;
        };
        let span = Span::new(start, end);
        if self.symbol_at(file, span) != Some(symbol)
            || self.binding_has_write(file, symbol)
            || crate::indexes::binding_written(file, span)
        {
            return false;
        }
        file.ast.bindings.iter().any(|binding| {
            binding
                .names
                .iter()
                .any(|name| location(file.path.shared(), name.span) == declaration)
                && binding.call_initializer.is_some_and(|initializer| {
                    file.ast.call_at(initializer).is_some_and(|call| {
                        self.primitive(file, call).is_some_and(|primitive| {
                            self.dialect().creates_reactive_source(primitive)
                        })
                    })
                })
        })
    }

    fn member_vetoed(&self, owner: &str, name: &str) -> bool {
        let owner = self
            .file(owner)
            .and_then(|file| self.id(file))
            .or_else(|| self.ctx.path_file(owner).map(|(id, _)| id));
        owner.is_none_or(|owner| !self.ctx.member_stable(owner, name))
    }

    fn dependency_member_stable(&self, path: &str, object: Span, name: &str) -> bool {
        let Some(id) = self.file(path).and_then(|file| self.id(file)) else {
            return false;
        };
        self.memo::<DependencyMemberStable>(&(id, object, Arc::from(name)))
            .unwrap_or(false)
    }

    fn tuple_iteration(&self) -> bool {
        self.ctx.derived::<TupleIteration>(&()).unwrap_or(true)
    }

    fn receiver_channels_opaque(&self) -> bool {
        self.ctx.hazard(Hazard::Eval) || self.ctx.hazard(Hazard::CodeEvaluatingCall)
    }

    fn accessor(&self, symbol: &SymbolId) -> bool {
        self.ctx.derived::<Accessors>(symbol).is_some()
    }

    fn source_kind(&self, symbol: &SymbolId) -> Option<crate::ReactiveSourceKind> {
        self.ctx
            .derived::<SourceKinds>(symbol)
            .map(|Kind(kind)| kind)
    }

    fn parameter_input(
        &self,
        file: &'w FileFacts,
        declaration: Span,
    ) -> Option<(Option<&'w FileFacts>, Span)> {
        let id = self.id(file)?;
        let rows = self.parameters(id)?;
        rows.inputs
            .iter()
            .find(|(pattern, _)| *pattern == declaration)
            .map(|(_, argument)| (Some(file), *argument))
    }

    fn binding_location(&self, symbol: &SymbolId) -> Option<Location> {
        if let Some(found) = self.binding_locations.borrow().get(symbol) {
            return found.clone();
        }
        let found = self
            .ctx
            .derived::<BindingLocations>(symbol)
            .map(|Site(location)| location);
        self.binding_locations
            .borrow_mut()
            .insert(symbol.clone(), found.clone());
        found
    }

    fn uses_of(&self, key: &Location) -> Option<ClosedUses> {
        if let Some(found) = self.uses.borrow().get(key) {
            return *found;
        }
        // A handoff binding's census is its joined one: the solver holds
        // every binding on a handoff; any other binding's is direct.
        let found = self
            .ctx
            .derived::<HandoffUses>(key)
            .or_else(|| self.ctx.derived::<DirectUses>(key));
        self.uses.borrow_mut().insert(key.clone(), found);
        found
    }

    fn providers_of(&self, key: &Location) -> Option<Cow<'_, ProviderValues>> {
        self.ctx
            .derived::<Providers>(key)
            .map(|providers| Cow::Owned(providers.as_ref().clone()))
    }

    fn unkeyed_provider(&self) -> bool {
        self.ctx
            .derived::<ProviderFlags>(&())
            .is_none_or(|(unkeyed, _)| unkeyed)
    }

    fn any_providers(&self) -> bool {
        self.ctx
            .derived::<ProviderFlags>(&())
            .is_none_or(|(_, any)| any)
    }

    fn blocked(&self, _: &Location) -> bool {
        // The memo world serves construction only, which sees no blocked
        // context.
        false
    }

    fn is_context_reference(&self, file: &'w FileFacts, span: Span) -> bool {
        // Answered for provider targets, the census's only question.
        let Some(id) = self.id(file) else {
            return false;
        };
        self.questions(id)
            .context_targets
            .binary_search(&span)
            .is_ok()
    }

    fn array_iteration_receiver(
        &self,
        file: &'w FileFacts,
        call: &'w ast::CallFact,
    ) -> Option<Span> {
        if !may_iterate(file, call) {
            return None;
        }
        let id = self.id(file)?;
        let ordinal = u32::try_from(ordinal_in(&file.ast.calls, call)?).ok()?;
        let questions = self.questions(id);
        let receivers = &questions.array_receivers;
        receivers
            .binary_search_by_key(&ordinal, |(ordinal, _)| *ordinal)
            .ok()
            .map(|index| receivers[index].1)
    }

    fn function_call_sites(
        &self,
        file: &'w FileFacts,
        function: Span,
    ) -> Vec<(&'w FileFacts, Span)> {
        let Some(id) = self.id(file) else {
            return Vec::new();
        };
        let Some(sites) = self.memo::<crate::indexes::CallSitesOf>(&(id, function)) else {
            return Vec::new();
        };
        sites
            .iter()
            .filter_map(|site| Some((self.read(site.file, site.rank)?, site.callee)))
            .collect()
    }

    fn parameter_typefacts_admit(
        &self,
        file: &'w FileFacts,
        function: &'w ast::FunctionFact,
        name: Span,
        caller: &'w FileFacts,
        call: &'w ast::CallFact,
        callee: Span,
    ) -> bool {
        let (Some(id), Some(caller_id), Some(function), Some(call)) = (
            self.id(file),
            self.id(caller),
            ordinal_in(&file.ast.functions, function),
            ordinal_in(&caller.ast.calls, call),
        ) else {
            return false;
        };
        let (Ok(function), Ok(call)) = (u32::try_from(function), u32::try_from(call)) else {
            return false;
        };
        self.memo::<ParameterTypefacts>(&Channel(id, function, name, caller_id, call, callee))
            .unwrap_or(false)
    }
}

/// A key of the kept folds: a location, ordered.
type Key = Site;

/// What the resolver keeps between builds.
#[derive(Default)]
struct ResolverState {
    /// The last answer of each file's stage memos, to find the files whose
    /// rows moved.
    bindings: BTreeMap<FileId, (Arc<BindingRows>, Arc<ParameterRows>)>,
    census: BTreeMap<FileId, Arc<CensusRows>>,
    /// Folds of the rows (K1), in path order.
    binding_locations: KeyedFold<SymbolId, (bool, Location), Location>,
    declarations: KeyedFold<SymbolId, (SymbolId, Location), (SymbolId, Location)>,
    direct_uses: KeyedFold<Key, ClosedUses, ClosedUses>,
    providers: KeyedFold<Key, Provider, Arc<Vec<Provider>>>,
    unkeyed: BTreeSet<FileId>,
    /// The handoff arguments of the last build: their census is the joined one.
    arguments: HashSet<Location>,
    /// The resolver's tables as the last build served them, patched by the
    /// keys the folds report moved.
    locations: Arc<HashMap<SymbolId, Location>>,
    displays: Arc<HashMap<SymbolId, (SymbolId, Location)>>,
    uses: Arc<HashMap<Location, ClosedUses>>,
    /// The published inputs as the last build published them.
    questions: BTreeSet<FileId>,
    accessors: HashMap<SymbolId, Location>,
    source_kinds: HashMap<SymbolId, crate::ReactiveSourceKind>,
}

/// One stage's memo answers for every file, in file order, or `None` when
/// one of them proves nothing.
fn answers<M, T>(
    world: &crate::base::memo::build::BuildWorld<'_>,
    files: &[(FileId, &FileFacts)],
) -> Option<Vec<Arc<T>>>
where
    M: Memo<BuildSchema, Key = FileId, Value = Rows<T>>,
    T: Canonical,
{
    crate::pipeline::parallel_balanced_results(files, |(id, _)| world.answer::<M>(id))
        .into_iter()
        .collect()
}

/// Publishes the entries of `current` that differ from `kept` (and the
/// removal of each key `current` lacks), and keeps `current`.
fn publish_map<D, V>(
    store: &crate::base::memo::build::MemoStore,
    kept: &mut HashMap<SymbolId, V>,
    current: HashMap<SymbolId, V>,
    value: impl Fn(&V) -> D::Value,
) where
    D: Derived<Key = SymbolId>,
    V: Clone + Eq,
{
    let mut changes = current
        .iter()
        .filter(|(symbol, current)| kept.get(*symbol) != Some(*current))
        .map(|(symbol, current)| (symbol.clone(), Some(value(current))))
        .collect::<Vec<_>>();
    changes.extend(
        kept.keys()
            .filter(|symbol| !current.contains_key(*symbol))
            .map(|symbol| (symbol.clone(), None)),
    );
    store.publish::<D>(changes);
    *kept = current;
}

/// Brings `mirror` to `fold` at the keys `changed` names.
fn patch<K, C, V, M>(
    mirror: &mut HashMap<M, V>,
    fold: &KeyedFold<K, C, V>,
    changed: &[K],
    key: impl Fn(&K) -> M,
) where
    K: Ord + Clone,
    C: Eq,
    V: Eq + Clone,
    M: Eq + std::hash::Hash,
{
    for changed in changed {
        match fold.values().get(changed) {
            Some(value) => {
                mirror.insert(key(changed), value.clone());
            }
            None => {
                mirror.remove(&key(changed));
            }
        }
    }
}

/// The resolver for this build: from the memo stages on kept state, or, when
/// those cannot serve it, fresh.
pub(super) fn build<'a, 'facts: 'a>(
    resolver: Resolver<'a, 'facts>,
    timer: &mut ResolverTimer,
) -> Resolver<'a, 'facts> {
    let lookup = resolver.lookup;
    // The build's files in file order, each with its memo id and slot rank:
    // the order the folds keep, and the keys of the stage memos.
    let files = lookup
        .files()
        .iter()
        .enumerate()
        .map(|(slot, file)| (lookup.memo_slot_of(slot), file))
        .collect::<Vec<_>>();
    // A path listed twice, or symbol names other than the lookup's (which
    // the published primitives answer with), is outside what the memo stages
    // key.
    if files.iter().any(|((_, rank), _)| *rank != 0)
        || !std::ptr::eq(resolver.names, lookup.symbol_names())
    {
        return resolver.fresh(timer);
    }
    let files = files
        .into_iter()
        .map(|((id, _), file)| (id, file))
        .collect::<Vec<_>>();
    let world = lookup.memo_world();
    let store = lookup.memo_store();
    let mut state = store
        .derived()
        .solver::<ResolverState, _>(RESOLVER_STATE, |state, _| std::mem::take(state));
    let Some(tables) = build_on(&resolver, &world, &files, &mut state, timer) else {
        // A stage proved nothing: the kept state is dropped (the next build
        // folds from scratch) and this one builds fresh.
        return resolver.fresh(timer);
    };
    store
        .derived()
        .solver::<ResolverState, _>(RESOLVER_STATE, |kept, _| *kept = state);
    let resolver = tables.into_resolver(resolver);
    if store.check_solvers() {
        let fresh = resolver.sibling().fresh(&mut ResolverTimer::off());
        if let Some(difference) = first_difference(&resolver, &fresh) {
            store.poison(&format!(
                "IR resolver verify mismatch: {difference}: the memo build differs from a fresh one"
            ));
            return Resolver {
                functions: resolver.functions,
                ..fresh
            };
        }
    }
    resolver
}

/// The resolver one build constructed ([`built_this_build`]): its store
/// revision, the identity of its inputs, its tables and what building it read.
struct Built {
    revision: crate::base::memo::Revision,
    inputs: [usize; 4],
    tables: Tables,
    reads: crate::base::reads::Reads,
}

/// The kept slot of [`Built`].
const RESOLVER_BUILT: SolverId = SolverId(241);

fn built_inputs(resolver: &Resolver<'_, '_>) -> [usize; 4] {
    [
        std::ptr::from_ref(resolver.facts) as usize,
        std::ptr::from_ref(resolver.accessors) as usize,
        std::ptr::from_ref(resolver.source_kinds) as usize,
        std::ptr::from_ref(resolver.names) as usize,
    ]
}

/// The resolver this build already constructed over the same inputs, with
/// `resolver`'s functions; its construction's reads are replayed to the
/// caller's recording. Construction publishes the derived tables, which a
/// build does once.
pub(super) fn built_this_build<'a, 'facts: 'a>(
    resolver: Resolver<'a, 'facts>,
) -> Option<Resolver<'a, 'facts>> {
    let store = resolver.lookup.memo_store();
    let revision = store.revision();
    let inputs = built_inputs(&resolver);
    let found = store
        .derived()
        .solver::<Option<Built>, _>(RESOLVER_BUILT, |kept, _| {
            kept.as_ref()
                .filter(|built| built.revision == revision && built.inputs == inputs)
                .map(|built| (built.tables.clone(), built.reads.clone()))
        })?;
    let (tables, reads) = found;
    crate::base::reads::replay(&reads);
    Some(tables.into_resolver(resolver))
}

/// Keeps what this build constructed, for [`built_this_build`].
pub(super) fn keep_built(resolver: &Resolver<'_, '_>, reads: crate::base::reads::Reads) {
    let store = resolver.lookup.memo_store();
    let built = Built {
        revision: store.revision(),
        inputs: built_inputs(resolver),
        tables: Tables::of(resolver),
        reads,
    };
    store
        .derived()
        .solver::<Option<Built>, _>(RESOLVER_BUILT, |kept, _| *kept = Some(built));
}

/// The tables a memo build found.
#[derive(Clone)]
struct Tables {
    binding_locations: Arc<HashMap<SymbolId, Location>>,
    declarations: Arc<HashMap<SymbolId, (SymbolId, Location)>>,
    parameter_inputs: HashMap<Location, (String, Span)>,
    uses: Arc<HashMap<Location, ClosedUses>>,
    providers: HashMap<Location, Vec<Option<(String, Span)>>>,
    unkeyed_provider: bool,
    source_origins: HashSet<SymbolId>,
    runtime_source_origins: HashSet<SymbolId>,
    blocked_contexts: HashSet<Location>,
}

impl Tables {
    fn of(resolver: &Resolver<'_, '_>) -> Self {
        Self {
            binding_locations: Arc::clone(&resolver.binding_locations),
            declarations: Arc::clone(&resolver.declarations),
            parameter_inputs: resolver.parameter_inputs.clone(),
            uses: Arc::clone(&resolver.uses),
            providers: resolver.providers.clone(),
            unkeyed_provider: resolver.unkeyed_provider,
            source_origins: resolver.source_origins.clone(),
            runtime_source_origins: resolver.runtime_source_origins.clone(),
            blocked_contexts: resolver.blocked_contexts.clone(),
        }
    }

    fn into_resolver<'a, 'facts: 'a>(self, resolver: Resolver<'a, 'facts>) -> Resolver<'a, 'facts> {
        Resolver {
            binding_locations: self.binding_locations,
            declarations: self.declarations,
            parameter_inputs: self.parameter_inputs,
            uses: self.uses,
            providers: self.providers,
            unkeyed_provider: self.unkeyed_provider,
            source_origins: self.source_origins,
            runtime_source_origins: self.runtime_source_origins,
            blocked_contexts: self.blocked_contexts,
            ..resolver
        }
    }
}

/// The memo build on `state`; `None` when a stage proves nothing.
fn build_on(
    resolver: &Resolver<'_, '_>,
    world: &crate::base::memo::build::BuildWorld<'_>,
    files: &[(FileId, &FileFacts)],
    state: &mut ResolverState,
    timer: &mut ResolverTimer,
) -> Option<Tables> {
    let store = resolver.lookup.memo_store();
    let order = FileOrder::new(&files.iter().map(|(id, _)| *id).collect::<Vec<_>>());
    // Stage 1: the inputs from source discovery.
    publish_map::<Accessors, _>(
        store,
        &mut state.accessors,
        resolver
            .accessors
            .iter()
            .map(|(symbol, (_, declaration))| (symbol.clone(), declaration.clone()))
            .collect(),
        |declaration| Site(declaration.clone()),
    );
    publish_map::<SourceKinds, _>(
        store,
        &mut state.source_kinds,
        resolver
            .source_kinds
            .iter()
            .map(|(symbol, kind)| (symbol.clone(), *kind))
            .collect(),
        |kind| Kind(*kind),
    );
    // What the census and the walks ask of the lookup about each file.
    let lookup = resolver.lookup;
    let questions = crate::pipeline::parallel_balanced_results(files, |(_, file)| {
        Arc::new(file_questions(lookup, file))
    });
    let current = files.iter().map(|(id, _)| *id).collect::<BTreeSet<_>>();
    let gone = state
        .questions
        .difference(&current)
        .map(|id| (*id, None))
        .collect::<Vec<_>>();
    store.publish::<FileQuestionsOf>(
        files
            .iter()
            .zip(questions)
            .map(|((id, _), questions)| (*id, Some(questions)))
            .chain(gone),
    );
    state.questions = current;
    store.publish::<TupleIteration>([((), Some(lookup.member_stability().tuple_iteration))]);
    timer.lap("inputs");
    // Stage 2: binding locations (a parameter key overwrites) and
    // declarations, last wins in file order.
    let bindings = answers::<ResolverBindings, _>(world, files)?;
    let parameters = answers::<ResolverParameters, _>(world, files)?;
    let mut locations = Vec::new();
    let mut declarations = Vec::new();
    for (((id, file), bindings), parameters) in files.iter().zip(&bindings).zip(&parameters) {
        let same = state
            .bindings
            .get(id)
            .is_some_and(|(kept, kept_parameters)| {
                (Arc::ptr_eq(kept, bindings) || kept == bindings)
                    && (Arc::ptr_eq(kept_parameters, parameters) || kept_parameters == parameters)
            });
        if same {
            continue;
        }
        state
            .bindings
            .insert(*id, (Arc::clone(bindings), Arc::clone(parameters)));
        let path = file.path.shared();
        let mut rows = bindings
            .locations
            .iter()
            .map(|(symbol, pattern)| (symbol.clone(), (false, location(path.clone(), *pattern))))
            .collect::<Vec<_>>();
        rows.extend(
            parameters.symbols.iter().map(|(symbol, pattern)| {
                (symbol.clone(), (true, location(path.clone(), *pattern)))
            }),
        );
        locations.push((*id, Some(rows)));
        declarations.push((
            *id,
            Some(
                bindings
                    .declarations
                    .iter()
                    .map(|(symbol, display, name)| {
                        (
                            symbol.clone(),
                            (display.clone(), location(path.clone(), *name)),
                        )
                    })
                    .collect(),
            ),
        ));
    }
    state.bindings.retain(|id, _| order.rank(*id).is_some());
    timer.count("bindingFilesMoved", locations.len());
    let located = state
        .binding_locations
        .update(order.clone(), locations, &|_,
                                            values: &[&(
            bool,
            Location,
        )]| {
            values
                .iter()
                .rev()
                .find(|(parameter, _)| *parameter)
                .or_else(|| values.last())
                .map(|(_, location)| location.clone())
        });
    let declared = state.declarations.update(order.clone(), declarations, &|_,
                                                                            values: &[&(
        SymbolId,
        Location,
    )]| {
        values.last().map(|value| (*value).clone())
    });
    store.publish::<BindingLocations>(located.changed.iter().map(|symbol| {
        (
            symbol.clone(),
            state
                .binding_locations
                .values()
                .get(symbol)
                .cloned()
                .map(Site),
        )
    }));
    patch(
        Arc::make_mut(&mut state.locations),
        &state.binding_locations,
        &located.changed,
        Clone::clone,
    );
    patch(
        Arc::make_mut(&mut state.displays),
        &state.declarations,
        &declared.changed,
        Clone::clone,
    );
    let mut parameter_inputs = HashMap::default();
    for ((_, file), parameters) in files.iter().zip(&parameters) {
        for (pattern, argument) in &parameters.inputs {
            parameter_inputs.insert(
                location(file.path.shared(), *pattern),
                (file.path.to_string(), *argument),
            );
        }
    }
    timer.lap("bindings");
    // Stage 3: the receiver census, joined; the providers in file order.
    let census = answers::<ResolverCensus, _>(world, files)?;
    let mut uses_rows = Vec::new();
    let mut provider_rows = Vec::new();
    for ((id, _), rows) in files.iter().zip(&census) {
        let same = state
            .census
            .get(id)
            .is_some_and(|kept| Arc::ptr_eq(kept, rows) || kept == rows);
        if same {
            continue;
        }
        state.census.insert(*id, Arc::clone(rows));
        if rows.unkeyed {
            state.unkeyed.insert(*id);
        } else {
            state.unkeyed.remove(id);
        }
        uses_rows.push((
            *id,
            Some(
                rows.uses
                    .iter()
                    .map(|(key, uses)| (Site(key.clone()), *uses))
                    .collect(),
            ),
        ));
        provider_rows.push((
            *id,
            Some(
                rows.providers
                    .iter()
                    .map(|(key, provider)| (Site(key.clone()), provider.clone()))
                    .collect(),
            ),
        ));
    }
    state.census.retain(|id, _| order.rank(*id).is_some());
    state.unkeyed.retain(|id| order.rank(*id).is_some());
    timer.count("censusFilesMoved", uses_rows.len());
    let direct =
        state
            .direct_uses
            .update(order.clone(), uses_rows, &|_, values: &[&ClosedUses]| {
                let mut joined = ClosedUses::bottom();
                for value in values {
                    joined.join(value);
                }
                Some(joined)
            });
    let provided = state.providers.update(order.clone(), provider_rows, &|_,
                                                                          values: &[&Option<
        (String, Span),
    >]| {
        Some(Arc::new(
            values.iter().map(|value| (*value).clone()).collect(),
        ))
    });
    store.publish::<DirectUses>(
        direct
            .changed
            .iter()
            .map(|key| (key.0.clone(), state.direct_uses.values().get(key).copied())),
    );
    store.publish::<Providers>(
        provided
            .changed
            .iter()
            .map(|key| (key.0.clone(), state.providers.values().get(key).cloned())),
    );
    let unkeyed_provider = !state.unkeyed.is_empty();
    let any_providers = !state.providers.values().is_empty();
    store.publish::<ProviderFlags>([((), Some((unkeyed_provider, any_providers)))]);
    let providers = state
        .providers
        .values()
        .iter()
        .map(|(Site(key), providers)| (key.clone(), providers.as_ref().clone()))
        .collect();
    timer.lap("census");
    // The handoff join on the kept K2 solver. A binding's census is the
    // joined one when it is a handoff's argument, its direct one otherwise.
    let edges = files
        .iter()
        .flat_map(|(id, _)| state.census[id].edges.iter().cloned())
        .collect::<Vec<_>>();
    let direct_uses = &state.direct_uses;
    let (definitions, arguments) = handoff_definitions(edges, |key| {
        direct_uses.values().get(&Site(key.clone())).copied()
    });
    let arguments = arguments.into_iter().collect::<HashSet<_>>();
    let mut touched = direct
        .changed
        .into_iter()
        .map(|Site(key)| key)
        .chain(state.arguments.symmetric_difference(&arguments).cloned())
        .collect::<Vec<_>>();
    let finals = store.solve_monotone(
        HANDOFF_SOLVER,
        definitions,
        &ClosedUses::handed_back,
        |solver, solved| {
            store.publish::<HandoffUses>(
                solved
                    .changed
                    .iter()
                    .map(|key| (key.clone(), solver.value(key).copied())),
            );
            touched.extend(solved.changed.iter().cloned());
            touched
                .into_iter()
                .map(|key| {
                    let value = if arguments.contains(&key) {
                        Some(solver.value(&key).copied().unwrap_or_default())
                    } else {
                        direct_uses.values().get(&Site(key.clone())).copied()
                    };
                    (key, value)
                })
                .collect::<Vec<_>>()
        },
    );
    {
        let uses = Arc::make_mut(&mut state.uses);
        for (key, value) in finals {
            match value {
                Some(value) => {
                    uses.insert(key, value);
                }
                None => {
                    uses.remove(&key);
                }
            }
        }
    }
    state.arguments = arguments;
    timer.lap("handoffs");
    // Stage 4: origins and blocked contexts.
    let origins = answers::<ResolverOrigins, _>(world, files)?;
    let mut source_origins = HashSet::default();
    let mut runtime_source_origins = HashSet::default();
    let mut blocked_contexts = HashSet::default();
    for rows in &origins {
        runtime_source_origins.extend(rows.runtime.iter().cloned());
        source_origins.extend(rows.source.iter().cloned());
        blocked_contexts.extend(rows.blocked.iter().cloned());
    }
    timer.lap("origins");
    Some(Tables {
        binding_locations: Arc::clone(&state.locations),
        declarations: Arc::clone(&state.displays),
        parameter_inputs,
        uses: Arc::clone(&state.uses),
        providers,
        unkeyed_provider,
        source_origins,
        runtime_source_origins,
        blocked_contexts,
    })
}

/// The first table and key in which two resolvers differ, if any.
fn first_difference(memo: &Resolver<'_, '_>, fresh: &Resolver<'_, '_>) -> Option<String> {
    fn map<K: std::fmt::Debug + Eq + std::hash::Hash, V: std::fmt::Debug + PartialEq>(
        name: &str,
        memo: &HashMap<K, V>,
        fresh: &HashMap<K, V>,
    ) -> Option<String> {
        let mut differing = memo
            .iter()
            .filter(|(key, value)| fresh.get(*key) != Some(*value))
            .map(|(key, value)| format!("{name} {key:?}: {value:?} != {:?}", fresh.get(key)))
            .chain(
                fresh
                    .iter()
                    .filter(|(key, _)| !memo.contains_key(*key))
                    .map(|(key, value)| format!("{name} {key:?}: None != {value:?}")),
            )
            .collect::<Vec<_>>();
        differing.sort_unstable();
        differing.into_iter().next()
    }
    fn set<K: std::fmt::Debug + Eq + std::hash::Hash>(
        name: &str,
        memo: &HashSet<K>,
        fresh: &HashSet<K>,
    ) -> Option<String> {
        let mut differing = memo
            .symmetric_difference(fresh)
            .map(|key| format!("{name} {key:?}"))
            .collect::<Vec<_>>();
        differing.sort_unstable();
        differing.into_iter().next()
    }
    map(
        "binding_locations",
        &memo.binding_locations,
        &fresh.binding_locations,
    )
    .or_else(|| map("declarations", &memo.declarations, &fresh.declarations))
    .or_else(|| {
        map(
            "parameter_inputs",
            &memo.parameter_inputs,
            &fresh.parameter_inputs,
        )
    })
    .or_else(|| map("uses", &memo.uses, &fresh.uses))
    .or_else(|| map("providers", &memo.providers, &fresh.providers))
    .or_else(|| {
        set(
            "source_origins",
            &memo.source_origins,
            &fresh.source_origins,
        )
    })
    .or_else(|| {
        set(
            "runtime_source_origins",
            &memo.runtime_source_origins,
            &fresh.runtime_source_origins,
        )
    })
    .or_else(|| {
        set(
            "blocked_contexts",
            &memo.blocked_contexts,
            &fresh.blocked_contexts,
        )
    })
    .or_else(|| {
        (memo.unkeyed_provider != fresh.unkeyed_provider).then(|| {
            format!(
                "unkeyed_provider: {} != {}",
                memo.unkeyed_provider, fresh.unkeyed_provider
            )
        })
    })
}
