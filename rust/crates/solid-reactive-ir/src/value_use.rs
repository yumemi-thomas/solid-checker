//! The value-use census (`sc9012-census.md`): every reference of a proven source
//! is in a position the analysis follows, or it is an obligation naming that
//! position. The default is the obligation: a form this file does not name is
//! `value-use-unclassified`, never a silent clean.
//!
//! Stores and props walk the same positions ([`Census::root_use`]). A store's
//! links are not this census: `const t = s.a; t.x = 1` is Track W's
//! forward flow.
//!
//! Computed per build and read only by `reactive_analysis.rs`, as F0's table is,
//! so no result chunk holds an answer that depends on another file.

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::collections::BTreeMap;

use solid_facts::FileFacts;
use solid_facts::ast::IdentifierRole;
use solid_facts::core::Span;
use typefacts::Location;

use crate::blockers;
use crate::indexes::SemanticLookup;
use crate::runtime_semantics::RuntimeArgumentBehavior;

/// How many positions one reference's walk may visit. Past it the answer is open.
const MAX_STEPS: usize = 512;

/// How many returns, props or contexts one walk may cross. Each ends in a
/// position of its own, so the limit bounds cost, not trust.
const MAX_HOPS: usize = 8;

/// What the value at a position is.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Mode {
    /// The source itself.
    Value,
    /// A literal or an alias of one that holds the source in a slot: a member
    /// Get of that slot hands the value on.
    Holder(Slot),
    /// A store or props root: its member Gets and Sets have their rows.
    Root,
    /// A literal holding a store or props root.
    RootHolder(Slot),
    /// A store or props root a project function received: no row sees its
    /// members there.
    RootParameter,
    /// ADR 0860: a value past a position the walk could not follow, which may
    /// be, or may hold, the source: a call, Get or write through it is a site
    /// with a row of its own, naming where the value escaped. `root`: the
    /// source is a store or props root, so a Get reads it.
    Escaped { root: bool, blocker: &'static str },
}

/// ADR 0860: an open position the walk goes on from, once the reference's own
/// walk has ended, so its answer and rows stay what they were.
struct Escape<'a> {
    file: &'a FileFacts,
    at: Span,
    row: Holder,
    root: bool,
    blocker: &'static str,
}

/// What a site past an escape does with the value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Site {
    Call,
    Read,
    Write,
}

/// The global objects a program's own globals are members of.
const GLOBAL_OBJECTS: [&str; 4] = ["globalThis", "window", "self", "global"];

/// How many names an escaped object may be traced back through
/// (`const y = x.self()`): each is a name the walk then follows forward.
const MAX_ALIASES: usize = 8;

/// Which slot of a holder holds the source.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Slot {
    /// A computed key, or a slot after a spread.
    Any,
    /// A static key, interned by [`Census::key`]. An array's index is its text.
    Key(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Position {
    Followed,
    Open(&'static str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Carried {
    Accessor,
    Writer,
    /// A tuple or a root: either.
    Either,
}

/// An argument of a call.
struct CallSlot {
    span: Span,
    index: usize,
}

/// What holds an expression, one row per table of `solid-facts` that names it.
#[derive(Clone, Copy, Debug)]
enum Holder {
    Callee(usize),
    Argument(usize, usize),
    MemberObject(usize),
    Returned(Span),
    ArrowBody(Span),
    Binding(usize),
    Assigned(usize),
    /// The target of a plain assignment: `props = merge(defaults, props)`
    /// holds no value there.
    Target,
    ArrayElement(Span, Option<usize>),
    Spread,
    /// A spread in an object literal, an array or a call's arguments.
    ObjectSpread(Span),
    ObjectValue(usize),
    ConditionalValue(Span),
    LogicalValue(Span),
    SequenceTail(Span),
    Test,
    /// A value nothing takes: a statement, `void`, `typeof`, `===` (schema 90).
    Inert,
    JsxValue(usize, usize),
    JsxChild,
    JsxSpread(usize),
    Coerced,
    TaggedSubstitution,
    Tag,
    ClassField,
    ParameterDefault,
    Awaited,
    Yielded,
    Iterated,
}

/// Ordered maps: a walk visits holders and references in source order, so two
/// runs over the same facts answer the same, whatever the insertion history.
#[derive(Default)]
struct FileIndex {
    holders: BTreeMap<Span, Vec<Holder>>,
    references: BTreeMap<Span, Vec<Span>>,
}

impl FileIndex {
    fn of(file: &FileFacts) -> Self {
        let ast = &file.ast;
        let peel = |span: Span| ast.peel_ts_sugar_span(span);
        let mut index = Self::default();
        let mut add = |span: Span, holder: Holder| {
            index.holders.entry(peel(span)).or_default().push(holder);
        };
        for (at, call) in ast.calls.iter().enumerate() {
            add(call.callee, Holder::Callee(at));
            for (argument, value) in call.arguments.iter().enumerate() {
                add(value.span, Holder::Argument(at, argument));
            }
        }
        for (at, member) in ast.members.iter().enumerate() {
            add(member.object, Holder::MemberObject(at));
        }
        for returned in &ast.returns {
            if let Some(argument) = returned.argument {
                add(argument, Holder::Returned(returned.span));
            }
        }
        for function in &ast.functions {
            if function.expression_body {
                add(function.body, Holder::ArrowBody(function.span));
            }
            for parameter in &function.parameters {
                if let Some(default) = parameter.initializer {
                    add(default, Holder::ParameterDefault);
                }
            }
        }
        for (at, binding) in ast.bindings.iter().enumerate() {
            if let Some(initializer) = binding.initializer {
                add(initializer, Holder::Binding(at));
            }
        }
        for (at, assignment) in ast.assignments.iter().enumerate() {
            add(assignment.value_span, Holder::Assigned(at));
            if !assignment.reads_target {
                add(assignment.target, Holder::Target);
            }
        }
        for literal in &ast.array_literal_elements {
            // Past a spread, an element's index is not its slot.
            let mut index = Some(0);
            for element in &literal.elements {
                let holder = if element.kind == solid_facts::ast::ArrayElementKind::Spread {
                    index = None;
                    Holder::Spread
                } else {
                    Holder::ArrayElement(literal.span, index)
                };
                index = index.map(|index| index + 1);
                add(element.span, holder);
            }
        }
        for (at, property) in ast.object_properties.iter().enumerate() {
            add(property.value, Holder::ObjectValue(at));
        }
        for spread in &ast.spreads {
            add(spread.argument, Holder::ObjectSpread(spread.span));
        }
        for conditional in &ast.conditional_expressions {
            add(conditional.test, Holder::Test);
            add(
                conditional.consequent,
                Holder::ConditionalValue(conditional.span),
            );
            add(
                conditional.alternate,
                Holder::ConditionalValue(conditional.span),
            );
        }
        for logical in &ast.logical_expressions {
            add(logical.left, Holder::LogicalValue(logical.span));
            add(logical.right, Holder::LogicalValue(logical.span));
        }
        for sequence in &ast.sequences {
            add(sequence.tail, Holder::SequenceTail(sequence.span));
        }
        for test in ast
            .conditional_tests
            .iter()
            .chain(&ast.nullish_test_operands)
        {
            add(*test, Holder::Test);
        }
        for operand in &ast.key_probed_operands {
            add(*operand, Holder::Test);
        }
        for operand in &ast.inert_operands {
            add(*operand, Holder::Inert);
        }
        for operand in &ast.coercive_operands {
            add(operand.span, Holder::Coerced);
        }
        for operand in &ast.coercing_operands {
            add(*operand, Holder::Coerced);
        }
        for (at, element) in ast.jsx_elements.iter().enumerate() {
            for (slot, attribute) in element.attributes.iter().enumerate() {
                if let Some(value) = attribute.expression.or(attribute.value) {
                    add(value, Holder::JsxValue(at, slot));
                }
            }
            for spread in &element.spreads {
                add(spread.argument, Holder::JsxSpread(at));
            }
        }
        for child in &ast.jsx_child_expressions {
            add(*child, Holder::JsxChild);
        }
        for template in &ast.template_literals {
            for substitution in &template.expressions {
                add(*substitution, Holder::Coerced);
            }
        }
        for tagged in &ast.tagged_templates {
            add(tagged.tag, Holder::Tag);
            for substitution in &tagged.substitutions {
                add(*substitution, Holder::TaggedSubstitution);
            }
        }
        for class in &ast.classes {
            for element in &class.elements {
                if let Some(value) = element.value {
                    add(value, Holder::ClassField);
                }
            }
        }
        for operand in &ast.await_operands {
            add(*operand, Holder::Awaited);
        }
        for yielded in &ast.yields {
            if let Some(operand) = yielded.operand {
                add(operand, Holder::Yielded);
            }
        }
        // `const [a] = v`: the pattern's own iteration is the binding's row.
        let unpacked = ast
            .bindings
            .iter()
            .filter(|binding| binding.shape == solid_facts::ast::BindingShape::Array)
            .filter_map(|binding| binding.initializer)
            .map(peel)
            .collect::<HashSet<_>>();
        for operand in &ast.iterated_operands {
            if !unpacked.contains(&peel(*operand)) {
                add(*operand, Holder::Iterated);
            }
        }
        for property in &ast.object_properties {
            if let Some(declaration) = property.shorthand_binding {
                index
                    .references
                    .entry(declaration)
                    .or_default()
                    .push(ast.peel_ts_sugar_span(property.value));
            }
        }
        for (reference, declaration) in &ast.reference_declarations {
            if reference != declaration {
                index
                    .references
                    .entry(*declaration)
                    .or_default()
                    .push(*reference);
            }
        }
        index
    }
}

/// The census of one analysis.
pub(crate) struct Census<'l, 'a> {
    lookup: &'l SemanticLookup<'a>,
    entities: &'l crate::EntitySymbols,
    symbol_names: &'l HashMap<crate::SymbolId, crate::SymbolId>,
    files: HashMap<String, FileIndex>,
    /// Positions met on this reference's walk, with their answer. A position
    /// met again while it is being answered is a cycle (a recursive helper),
    /// and a cycle is open.
    walked: HashMap<(String, Span, Mode), Option<Position>>,
    keys: HashMap<String, u32>,
    /// The sources this census walks: a call of one has its row.
    sources: HashSet<&'l str>,
    /// The calls transport resolved to a source: each has its row.
    resolved: &'l HashMap<Location, crate::interproc::accessor_provenance::Callable>,
    /// Where each file has a read, write or action row, or an obligation.
    rows: &'l RowIndex,
    /// ADR 0811: the store member writes SC2003 decides in a file, asked
    /// once per file the walk reaches a root's member in.
    member_writes: MemberWrites<'l>,
    decided_writes: HashMap<String, Vec<Span>>,
    /// What a call of the walked value does.
    carried: Carried,
    steps: usize,
    parameter_hops: usize,
    return_hops: usize,
    /// Whether an exported function's callers are all in view.
    exported: crate::attribution::ExportedCallers<'l, 'a>,
    /// The parameter references a hop of this start's walks starts from: a
    /// call of one has its row at the call that hands the value. An alias or
    /// pattern past it has none. Kept per start: a hop another start made
    /// says nothing about this start's walk (ADR 0909).
    hop_starts: HashSet<(String, Span)>,
    /// The walked source may be pending: a tracked position is SC5003's and
    /// SC5001's question then, which only a row answers (W's review H3).
    async_walk: bool,
    /// The uses a walk ended at, past the reference it started from: the
    /// obligation sits at the use too, so each has a row of its own.
    uses: Vec<(Location, &'static str)>,
    /// A walk of the current reference ran past `MAX_STEPS`: what lies past
    /// the cut is unknown, so the reference owes a budget row of its own
    /// ([`Self::take_cut`]), whatever its walks answered.
    cut: bool,
    /// ADR 0876: the calls a withheld member heads, walked on after every
    /// other walk: this walk's memo and budget decide those walks' rows.
    past_withheld: Vec<(&'a FileFacts, &'a solid_facts::ast::CallFact, Mode)>,
    /// ADR 0860: the open positions this reference's walk goes on from.
    escapes: Vec<Escape<'a>>,
    /// ADR 0860: the rows the walks past an escape placed. They come after
    /// every other row: a row another producer, or a use before an escape,
    /// placed at the same start keeps its place.
    escaped_uses: Vec<(Location, &'static str)>,
    /// Each file's callback spans, for the role of a site past an escape.
    allowed: HashMap<String, Vec<Span>>,
}

/// ADR 0811: the store member writes SC2003 decides in a file.
pub(crate) type MemberWrites<'l> = Box<dyn Fn(&FileFacts) -> Vec<Span> + 'l>;

/// Where each file has a read, write or action row, or an obligation: what
/// [`Census::has_row`] asks, built once and shared by every census of a run.
pub(crate) type RowIndex = BTreeMap<String, Vec<(u64, u64)>>;

/// The [`RowIndex`] of `located`.
pub(crate) fn row_index<'x>(located: impl Iterator<Item = &'x Location>) -> RowIndex {
    let mut rows = RowIndex::default();
    for location in located {
        rows.entry(location.path.to_string())
            .or_default()
            .push((location.start_byte, location.end_byte));
    }
    rows
}

/// What one start's walk (a reference, or a creating call no binding holds)
/// answered: its own obligation, whether a walk ran past the step budget, and
/// the uses its walks ended at, before and past an escape.
pub(crate) struct StartAnswer {
    pub(crate) open: Option<&'static str>,
    pub(crate) cut: bool,
    pub(crate) uses: Vec<(Location, &'static str)>,
    pub(crate) escaped_uses: Vec<(Location, &'static str)>,
}

impl<'l, 'a> Census<'l, 'a> {
    pub(crate) fn new(
        lookup: &'l SemanticLookup<'a>,
        entities: &'l crate::EntitySymbols,
        symbol_names: &'l HashMap<crate::SymbolId, crate::SymbolId>,
        sources: HashSet<&'l str>,
        resolved: &'l HashMap<Location, crate::interproc::accessor_provenance::Callable>,
        exported: crate::attribution::ExportedCallers<'l, 'a>,
        rows: &'l RowIndex,
    ) -> Self {
        Self {
            lookup,
            entities,
            symbol_names,
            files: HashMap::default(),
            walked: HashMap::default(),
            keys: HashMap::default(),
            sources,
            resolved,
            rows,
            member_writes: Box::new(no_member_writes),
            decided_writes: HashMap::default(),
            carried: Carried::Either,
            steps: 0,
            parameter_hops: 0,
            return_hops: 0,
            exported,
            hop_starts: HashSet::default(),
            async_walk: true,
            uses: Vec::new(),
            cut: false,
            past_withheld: Vec::new(),
            escapes: Vec::new(),
            escaped_uses: Vec::new(),
            allowed: HashMap::default(),
        }
    }

    /// Starts one start's walk from nothing this census walked before: a
    /// start's answer is a function of the build and the start alone, so the
    /// starts can be walked on any worker in any order. The caches kept
    /// across starts (`files`, `allowed`, `decided_writes`, `keys`, the
    /// exported-callers graph) answer from the build alone.
    fn begin_start(&mut self) {
        debug_assert_eq!(self.parameter_hops, 0, "a hop outlived its start");
        debug_assert_eq!(self.return_hops, 0, "a hop outlived its start");
        debug_assert!(self.escapes.is_empty() && self.past_withheld.is_empty());
        debug_assert!(self.uses.is_empty() && self.escaped_uses.is_empty() && !self.cut);
        self.walked.clear();
        self.steps = 0;
        self.hop_starts.clear();
    }

    /// The answer of the start just walked, taken out of the census.
    fn end_start(&mut self, open: Option<&'static str>) -> StartAnswer {
        StartAnswer {
            open,
            cut: self.take_cut(),
            uses: self.take_uses(),
            escaped_uses: self.take_escaped_uses(),
        }
    }

    /// [`Self::callable_use`] as a [`StartAnswer`].
    pub(crate) fn callable_start(
        &mut self,
        reference: &Location,
        accessor: bool,
        async_source: bool,
    ) -> StartAnswer {
        let open = self.callable_use(reference, accessor, async_source);
        self.end_start(open)
    }

    /// [`Self::root_use`] as a [`StartAnswer`].
    pub(crate) fn root_start(&mut self, reference: &Location, async_source: bool) -> StartAnswer {
        let open = self.root_use(reference, async_source);
        self.end_start(open)
    }

    /// [`Self::created_use`] as a [`StartAnswer`].
    pub(crate) fn created_start(
        &mut self,
        file: &'a FileFacts,
        call: Span,
        store: bool,
        tuple: bool,
    ) -> StartAnswer {
        let open = self.created_use(file, call, store, tuple);
        self.end_start(open)
    }

    /// Whether a walk since the last call ran past the step budget. The
    /// caller files a budget row at the walk's root for it, which nothing
    /// deduplicates: a cut walk is never silent.
    pub(crate) fn take_cut(&mut self) -> bool {
        std::mem::take(&mut self.cut)
    }

    /// The uses the walks so far ended at, each with the position it is in.
    pub(crate) fn take_uses(&mut self) -> Vec<(Location, &'static str)> {
        std::mem::take(&mut self.uses)
    }

    /// ADR 0860: the rows the walks past an escape placed so far.
    pub(crate) fn take_escaped_uses(&mut self) -> Vec<(Location, &'static str)> {
        std::mem::take(&mut self.escaped_uses)
    }

    /// The obligation one reference of an accessor, setter or action owes.
    pub(crate) fn callable_use(
        &mut self,
        reference: &Location,
        accessor: bool,
        async_source: bool,
    ) -> Option<&'static str> {
        self.async_walk = async_source;
        self.carried = if accessor {
            Carried::Accessor
        } else {
            Carried::Writer
        };
        self.reference_use(reference, Mode::Value)
    }

    /// The obligation one reference of a store or props root owes.
    pub(crate) fn root_use(
        &mut self,
        reference: &Location,
        async_source: bool,
    ) -> Option<&'static str> {
        self.async_walk = async_source;
        self.carried = Carried::Either;
        self.reference_use(reference, Mode::Root)
    }

    /// P13: the obligation of a creating call no binding holds, so no reference
    /// starts the walk: `{ a: createMemo(f) }`, `createStore(v)[0]`.
    pub(crate) fn created_use(
        &mut self,
        file: &'a FileFacts,
        call: Span,
        store: bool,
        tuple: bool,
    ) -> Option<&'static str> {
        // A binding's names are sources with references of their own, except a
        // whole tuple's: `const pair = createSignal(1)` names no accessor.
        let named = self.holders(file, call).iter().any(|row| match row {
            Holder::Binding(binding) => {
                !tuple
                    || file.ast.bindings[*binding].shape
                        != solid_facts::ast::BindingShape::Identifier
            }
            _ => false,
        });
        if named {
            return None;
        }
        // A created source may be pending unless its first argument is proved
        // sync (ADR 0665): a data value, or a function whose return is closed.
        self.async_walk = !file
            .ast
            .call_at(call)
            .and_then(|fact| fact.arguments.first())
            .is_some_and(|argument| {
                crate::owners::computation_provenance(self.lookup, file, argument)
                    == crate::owners::ComputationProvenance::Sync
            });
        // A tuple holds a setter too.
        self.carried = if tuple {
            Carried::Either
        } else {
            Carried::Accessor
        };
        self.begin_start();
        let mode = match (store, tuple) {
            (false, false) => Mode::Value,
            (false, true) => Mode::Holder(Slot::Any),
            (true, false) => Mode::Root,
            (true, true) => Mode::RootHolder(Slot::Any),
        };
        let answer = match self.position(file, call, mode) {
            Position::Followed => None,
            Position::Open(blocker) => Some(blocker),
        };
        self.run_escapes();
        answer
    }

    fn reference_use(&mut self, reference: &Location, mode: Mode) -> Option<&'static str> {
        // A reference this cannot read is no silence (1c).
        let Some((file, span)) = self.place(reference) else {
            return Some(blockers::VALUE_USE_UNCLASSIFIED);
        };
        self.begin_start();
        let answer = if is_value_reference(file, span) {
            match self.position(file, span, mode) {
                Position::Followed => None,
                Position::Open(blocker) => Some(blocker),
            }
        } else {
            self.unnamed_reference(file, span, mode)
        };
        self.run_escapes();
        answer
    }

    /// ADR 0860: the walk goes on past each open position it met, once the
    /// reference's own walk has ended: its answer, its rows and its budget
    /// are what they were. The escapes share one budget of their own.
    fn run_escapes(&mut self) {
        let escapes = std::mem::take(&mut self.escapes);
        let before = self.uses.len();
        // One budget for all of them: a position past it is a use.
        self.steps = 0;
        for escape in escapes {
            self.parameter_hops = 0;
            self.return_hops = 0;
            let mode = Mode::Escaped {
                root: escape.root,
                blocker: escape.blocker,
            };
            let recorded = self.uses.len();
            if let Position::Open(blocker) =
                self.escaped_through(escape.file, escape.at, escape.row, mode)
                && self.uses.len() == recorded
            {
                self.uses
                    .push((escape.at.location(escape.file.path.as_str()), blocker));
            }
        }
        self.escapes.clear();
        let escaped = self.uses.split_off(before);
        self.escaped_uses.extend(escaped);
        self.run_past_withheld();
    }

    /// ADR 0876: what a withheld call may hand on, by a walk of its own that
    /// only adds rows.
    fn run_past_withheld(&mut self) {
        if self.past_withheld.is_empty() {
            return;
        }
        let walked = std::mem::take(&mut self.walked);
        let before = self.uses.len();
        self.steps = 0;
        while let Some((file, call, mode)) = self.past_withheld.pop() {
            self.parameter_hops = 0;
            self.return_hops = 0;
            self.position(file, call.span, mode);
            self.callback_parameters(file, call, mode);
        }
        self.escapes.clear();
        let past = self.uses.split_off(before);
        self.escaped_uses.extend(past);
        self.walked = walked;
    }

    /// ADR 0860: an open position past which the walk goes on as an escaped
    /// value. A walk that is already past one goes on by itself.
    fn escape(
        &mut self,
        file: &'a FileFacts,
        at: Span,
        row: Holder,
        mode: Mode,
        blocker: &'static str,
    ) {
        let root = match mode {
            Mode::Value | Mode::Holder(_) => false,
            Mode::Root | Mode::RootHolder(_) | Mode::RootParameter => true,
            Mode::Escaped { .. } => return,
        };
        self.escapes.push(Escape {
            file,
            at,
            row,
            root,
            blocker,
        });
    }

    /// Review item 6: a reference that is no identifier read. A type query
    /// or a declaration's own name holds no value; a namespace member
    /// (`S.count`, the property naming the export) is the member's value; a
    /// pattern slot naming it holds it. Anything else is open, never skipped.
    fn unnamed_reference(
        &mut self,
        file: &'a FileFacts,
        span: Span,
        mode: Mode,
    ) -> Option<&'static str> {
        let ast = &file.ast;
        if ast.type_queries.iter().any(|query| query.contains(span)) {
            return None;
        }
        if let Some(member) = ast
            .members
            .iter()
            .find(|member| member.property == span && !ast.computed_members.contains(&member.span))
        {
            return match self.position(file, member.span, mode) {
                Position::Followed => None,
                Position::Open(blocker) => Some(blocker),
            };
        }
        let slot = ast.bindings.iter().any(|binding| {
            binding
                .object_slots
                .iter()
                .any(|slot| slot.local.span == span || slot.local.span.contains(span))
        });
        if slot {
            return Some(blockers::VALUE_HELD_IN_BINDING);
        }
        let declared = ast.identifiers_within(span).any(|identifier| {
            identifier.span == span && identifier.role == IdentifierRole::Binding
        });
        if declared {
            return None;
        }
        Some(blockers::VALUE_USE_UNCLASSIFIED)
    }

    fn key(&mut self, key: &str) -> Slot {
        let next = u32::try_from(self.keys.len()).unwrap_or(u32::MAX);
        Slot::Key(*self.keys.entry(key.to_owned()).or_insert(next))
    }

    fn place(&self, reference: &Location) -> Option<(&'a FileFacts, Span)> {
        Some((
            self.lookup.file_by_path(reference.path.as_ref())?,
            Span::new(
                u32::try_from(reference.start_byte).ok()?,
                u32::try_from(reference.end_byte).ok()?,
            ),
        ))
    }

    fn holders(&mut self, file: &FileFacts, at: Span) -> Vec<Holder> {
        self.files
            .entry(file.path.to_string())
            .or_insert_with(|| FileIndex::of(file))
            .holders
            .get(&at)
            .cloned()
            .unwrap_or_default()
    }

    fn references(&mut self, file: &FileFacts, declaration: Span) -> Vec<Span> {
        self.files
            .entry(file.path.to_string())
            .or_insert_with(|| FileIndex::of(file))
            .references
            .get(&declaration)
            .cloned()
            .unwrap_or_default()
    }

    /// Where the value at `at` goes.
    fn position(&mut self, file: &'a FileFacts, at: Span, mode: Mode) -> Position {
        self.steps += 1;
        if self.steps > MAX_STEPS {
            self.cut = true;
            return Position::Open(blockers::VALUE_USE_UNCLASSIFIED);
        }
        let entry = self.steps;
        let escaped = matches!(mode, Mode::Escaped { .. });
        let key = (file.path.to_string(), at, mode);
        match self.walked.get(&key) {
            Some(Some(answer)) => return *answer,
            // Past an escape a cycle is a position this walk already covers.
            Some(None) if escaped => return Position::Followed,
            Some(None) => return Position::Open(blockers::VALUE_USE_UNCLASSIFIED),
            None => {}
        }
        self.walked.insert(key.clone(), None);
        let recorded = self.uses.len();
        let holders = self.holders(file, at);
        // A value several tables name is as open as the first open one. The
        // walk goes on past it: every use it reaches gets its row.
        let mut answer = if holders.is_empty() {
            if escaped {
                self.escaped_unheld(file, at, mode)
            } else {
                self.unheld(file, at, mode)
            }
        } else {
            Position::Followed
        };
        for row in holders {
            let position = if escaped {
                self.escaped_through(file, at, row, mode)
            } else {
                self.through(file, at, row, mode)
            };
            if answer == Position::Followed {
                answer = position;
            }
        }
        // The walk ended here, past the reference it started from, and no use
        // further on was named: this position is the use.
        if let Position::Open(blocker) = answer
            && (entry > 1 || escaped)
            && self.uses.len() == recorded
        {
            self.uses.push((at.location(file.path.as_str()), blocker));
        }
        self.walked.insert(key, Some(answer));
        answer
    }

    /// A position no table holds: an export or a name.
    fn unheld(&self, file: &FileFacts, at: Span, mode: Mode) -> Position {
        let ast = &file.ast;
        if jsx_name(file, at) {
            // `<H />` calls what `H` holds, and no row reads it. A root's tag
            // is a member Get of the root.
            return if mode == Mode::Value {
                Position::Open(blockers::VALUE_TO_UNRESOLVED_CALLEE)
            } else {
                Position::Followed
            };
        }
        if ast.deleted_targets.contains(&at) {
            return Position::Followed;
        }
        // An export of a name: its importers' references are the name's own,
        // walked with it. `export default` of a literal or a call has no name,
        // and its importers are walked by nothing; a holder's default export
        // is no name the walk follows into them either.
        if exported_directly(file, at) {
            let specifier = ast
                .exports
                .iter()
                .flat_map(|export| &export.specifiers)
                .any(|specifier| specifier.local.span == at);
            let named = is_value_reference(file, at)
                && (specifier || matches!(mode, Mode::Value | Mode::Root));
            return if named {
                Position::Followed
            } else {
                Position::Open(blockers::VALUE_EXPORTED_UNNAMED)
            };
        }
        Position::Open(blockers::VALUE_USE_UNCLASSIFIED)
    }

    #[deny(clippy::wildcard_enum_match_arm)]
    fn through(&mut self, file: &'a FileFacts, at: Span, row: Holder, mode: Mode) -> Position {
        let ast = &file.ast;
        match row {
            Holder::Callee(call) => {
                if ast.calls[call].construct {
                    Position::Open(blockers::VALUE_THROUGH_NEW)
                } else if mode == Mode::Value
                    && self.steps > 1
                    && !(self.parameter_hops > 0
                        && self.hop_starts.contains(&(file.path.to_string(), at)))
                    && !self.has_row(file, ast.calls[call].callee)
                    && !self.role_clears(file, ast.calls[call].callee)
                {
                    // Walked into a name no row reads: `const { c: h } = o; h()`.
                    Position::Open(blockers::VALUE_HELD_IN_BINDING)
                } else {
                    Position::Followed
                }
            }
            Holder::Test | Holder::Inert | Holder::Target => Position::Followed,
            Holder::JsxChild => self.jsx_child(file, at, mode),
            // A tag is a call no row reads (P11).
            Holder::Tag => Position::Open(blockers::VALUE_TO_UNRESOLVED_CALLEE),
            // A root's coercion reads its keys, a holder's coerces what it
            // holds (`[st] + ""`, `${{ toString: c }}`); an accessor's own has
            // SC1005.
            Holder::Coerced => {
                if mode != Mode::Value {
                    Position::Open(blockers::VALUE_IN_CARRIER)
                } else {
                    Position::Followed
                }
            }
            Holder::MemberObject(member) => {
                let recorded = self.uses.len();
                let answer = self.member(file, at, member, mode);
                self.escape_if_here(file, at, row, mode, answer, recorded);
                answer
            }
            Holder::JsxValue(element, slot) => self.jsx_value(file, element, Some(slot), mode),
            Holder::JsxSpread(element) => self.jsx_value(file, element, None, mode),
            Holder::Binding(binding) => self.bound(file, at, binding, mode),
            Holder::Assigned(assignment) => {
                let answer = assigned(file, assignment);
                // ADR 0860: an object that holds the value in a member hands
                // it to every use of the object.
                if let Position::Open(blocker) = answer
                    && blocker == blockers::VALUE_HELD_IN_MEMBER
                {
                    self.escape(file, at, row, mode, blocker);
                }
                // `r = source`: the name's uses hold it as a binding's do.
                let target = ast.peel_ts_sugar_span(ast.assignments[assignment].target);
                if ast.member_at(target).is_none()
                    && let Some(declaration) = ast.reference_declaration(target)
                {
                    self.name_uses(file, declaration, mode);
                }
                answer
            }
            Holder::Returned(returned) => {
                let function = crate::owners::containing_ast_function(ast, returned);
                let recorded = self.uses.len();
                let answer = self.returned(file, function.map(|function| function.span), mode);
                self.escape_if_here(file, at, row, mode, answer, recorded);
                answer
            }
            Holder::ArrowBody(function) => {
                let recorded = self.uses.len();
                let answer = self.returned(file, Some(function), mode);
                self.escape_if_here(file, at, row, mode, answer, recorded);
                answer
            }
            Holder::ArrayElement(literal, index) => {
                // A literal in a literal: no row walks two member steps.
                if matches!(mode, Mode::Holder(_) | Mode::RootHolder(_)) {
                    self.escape(file, at, row, mode, blockers::VALUE_IN_CARRIER);
                    return Position::Open(blockers::VALUE_IN_CARRIER);
                }
                let slot = match index {
                    Some(index) => self.key(&index.to_string()),
                    None => Slot::Any,
                };
                self.held_in_literal(file, literal, slot, mode)
            }
            Holder::ObjectValue(property) => {
                if matches!(mode, Mode::Holder(_) | Mode::RootHolder(_)) {
                    self.escape(file, at, row, mode, blockers::VALUE_IN_CARRIER);
                    return Position::Open(blockers::VALUE_IN_CARRIER);
                }
                let property = &ast.object_properties[property];
                let text = file
                    .source_text(property.key)
                    .filter(|_| !property.computed);
                // `{ __proto__: v }` installs `v` as the prototype: every Get
                // of the literal may read it. A shorthand defines an own
                // property.
                if text.is_some_and(|key| key.trim_matches(['"', '\'']) == "__proto__")
                    && property.key != property.value
                {
                    self.escape(file, at, row, mode, blockers::VALUE_IN_CARRIER);
                    return Position::Open(blockers::VALUE_IN_CARRIER);
                }
                let slot = match text.and_then(written_key) {
                    Some(key) => self.key(key),
                    None => Slot::Any,
                };
                let recorded = self.uses.len();
                let answer = self.held_in_literal(file, property.span, slot, mode);
                self.escape_if_here(file, at, row, mode, answer, recorded);
                answer
            }
            Holder::Spread => {
                self.escape(file, at, row, mode, blockers::VALUE_IN_CARRIER);
                Position::Open(blockers::VALUE_IN_CARRIER)
            }
            // `{ ...state }` copies a holder's slots into a holder: what it
            // held stays one member step away.
            Holder::ObjectSpread(spread) => match (mode, object_spread_literal(file, spread)) {
                (Mode::Holder(_) | Mode::RootHolder(_), Some(literal))
                    if !literal_reads_this(file, literal) =>
                {
                    self.position(file, literal, mode)
                }
                _ => {
                    self.escape(file, at, row, mode, blockers::VALUE_IN_CARRIER);
                    Position::Open(blockers::VALUE_IN_CARRIER)
                }
            },
            // A composite's value goes no further than a position that reads it
            // whole: an alias of it has no row.
            Holder::ConditionalValue(value)
            | Holder::LogicalValue(value)
            | Holder::SequenceTail(value) => {
                let rows = self.holders(file, value);
                // JSX reads the composite whole only on an intrinsic element.
                if !rows.is_empty()
                    && rows.iter().all(|row| {
                        matches!(row, Holder::Test | Holder::Inert | Holder::Coerced)
                            || matches!(row, Holder::Callee(call) if !ast.calls[*call].construct)
                            || if let Holder::JsxValue(element, _) | Holder::JsxSpread(element) =
                                row
                            {
                                intrinsic(file, *element)
                            } else {
                                matches!(row, Holder::JsxChild)
                                    && jsx_child_parent(file, value)
                                        .is_some_and(|element| intrinsic(file, element))
                            }
                    })
                {
                    Position::Followed
                } else {
                    self.escape(file, at, row, mode, blockers::VALUE_THROUGH_LOGICAL);
                    Position::Open(blockers::VALUE_THROUGH_LOGICAL)
                }
            }
            Holder::Argument(call, index) => {
                let recorded = self.uses.len();
                let answer = self.argument(file, call, index, mode);
                self.escape_if_here(file, at, row, mode, answer, recorded);
                answer
            }
            Holder::TaggedSubstitution => Position::Open(blockers::VALUE_TO_UNRESOLVED_CALLEE),
            Holder::ClassField => {
                self.class_field_uses(file, at, mode);
                Position::Open(blockers::VALUE_IN_CLASS_FIELD)
            }
            Holder::ParameterDefault => {
                self.escape(file, at, row, mode, blockers::VALUE_AS_PARAMETER_DEFAULT);
                Position::Open(blockers::VALUE_AS_PARAMETER_DEFAULT)
            }
            Holder::Awaited => Position::Open(blockers::VALUE_THROUGH_AWAIT),
            Holder::Yielded => Position::Open(blockers::VALUE_THROUGH_GENERATOR),
            Holder::Iterated => Position::Open(blockers::VALUE_IN_CARRIER),
        }
    }

    /// ADR 0860: a site past an escape, a call, Get or write through a value
    /// that may be, or may hold, the source, has a row of its own unless its
    /// role proves the operation legal there. A store write is legal only in
    /// its setter's callback, whose draft no escaped value is: it always has one.
    fn site(&mut self, file: &'a FileFacts, span: Span, site: Site, blocker: &'static str) {
        let path = file.path.as_str();
        if !self.allowed.contains_key(path) {
            let spans = crate::execution_role::allowed_callback_spans(file, self.lookup);
            self.allowed.insert(path.to_owned(), spans);
        }
        let role = crate::execution_role::read_execution_role(
            file,
            span,
            self.allowed.get(path).map_or(&[], Vec::as_slice),
            self.entities,
            self.symbol_names,
            self.lookup,
        );
        // W3: a deferred callback clears a read only where it is proved to
        // run on a fresh stack: a literal a standard function takes at such a
        // slot (`setTimeout`, `.then`). Any other may run on the caller's.
        let deferred_unproven = role == crate::ExecutionRole::DeferredCallback
            && !self.fresh_stack_callback(file, span);
        // A compute's read past an await runs with no observer, whatever the
        // role says of the compute: no role clears it there.
        let suspended = crate::execution_role::past_suspension(file, span, self.lookup).is_some();
        let reads = !self.async_walk
            && role != crate::ExecutionRole::Unknown
            && !deferred_unproven
            && !suspended
            && !role.reports_untracked_read();
        let writes = role != crate::ExecutionRole::Unknown && !role.reports_disallowed_write();
        let clear = match (site, self.carried) {
            (Site::Read, _) | (Site::Call, Carried::Accessor) => reads,
            (Site::Call, Carried::Writer) => writes,
            (Site::Call, Carried::Either) => reads && writes,
            (Site::Write, _) => false,
        };
        if !clear {
            self.uses.push((span.location(path), blocker));
        }
    }

    /// ADR 0860: where a value past an escape goes. No row reads it there, so
    /// the walk follows every way it may still be the source, or hold it, to
    /// the calls, Gets and writes through it ([`Self::site`]). A position this
    /// cannot follow is a use, as before the escape.
    #[deny(clippy::wildcard_enum_match_arm)]
    fn escaped_through(
        &mut self,
        file: &'a FileFacts,
        at: Span,
        row: Holder,
        mode: Mode,
    ) -> Position {
        let Mode::Escaped { root, blocker } = mode else {
            return self.through(file, at, row, mode);
        };
        let ast = &file.ast;
        match row {
            Holder::Callee(call) => {
                let call = ast.calls[call].span;
                self.site(file, call, Site::Call, blocker);
                self.position(file, call, mode);
                Position::Followed
            }
            Holder::Test | Holder::Inert => Position::Followed,
            Holder::Target => {
                if root && ast.member_at(at).is_some() {
                    self.site(file, at, Site::Write, blocker);
                }
                Position::Followed
            }
            // A root's coercion reads its keys; a function's runs nothing.
            Holder::Coerced => {
                if root {
                    self.site(file, at, Site::Read, blocker);
                }
                Position::Followed
            }
            Holder::MemberObject(member) => self.escaped_member(file, member, mode),
            Holder::Binding(binding) => {
                let binding = &ast.bindings[binding];
                // A pattern over a root reads it, and its slots hold what it read.
                if root && binding.shape != solid_facts::ast::BindingShape::Identifier {
                    self.site(file, binding.pattern, Site::Read, blocker);
                    return Position::Followed;
                }
                for name in &binding.names {
                    self.name_uses(file, name.span, mode);
                }
                Position::Followed
            }
            Holder::Assigned(assignment) => {
                let target = ast.peel_ts_sugar_span(ast.assignments[assignment].target);
                if let Some(member) = ast.member_at(target) {
                    self.escaped_object(file, member.object, member_key(file, member), mode, 0);
                } else if let Some(declaration) = ast.reference_declaration(target) {
                    self.name_uses(file, declaration, mode);
                }
                Position::Followed
            }
            Holder::Returned(returned) => {
                let function = crate::owners::containing_ast_function(ast, returned)
                    .map(|function| function.span);
                self.escaped_returned(file, function, mode)
            }
            Holder::ArrowBody(function) => self.escaped_returned(file, Some(function), mode),
            Holder::ArrayElement(literal, _) => self.position(file, literal, mode),
            Holder::ObjectValue(property) => {
                match enclosing_literal(file, ast.object_properties[property].span) {
                    Some(literal) => self.position(file, literal, mode),
                    None => Position::Open(blockers::VALUE_IN_CARRIER),
                }
            }
            Holder::Spread => {
                let literal = ast
                    .array_literal_elements
                    .iter()
                    .find(|literal| {
                        literal
                            .elements
                            .iter()
                            .any(|element| ast.peel_ts_sugar_span(element.span) == at)
                    })
                    .map(|literal| literal.span);
                match literal {
                    Some(literal) => self.position(file, literal, mode),
                    None => Position::Open(blockers::VALUE_IN_CARRIER),
                }
            }
            Holder::ObjectSpread(spread) => match object_spread_literal(file, spread) {
                Some(literal) => self.position(file, literal, mode),
                None => Position::Open(blockers::VALUE_IN_CARRIER),
            },
            Holder::ConditionalValue(value)
            | Holder::LogicalValue(value)
            | Holder::SequenceTail(value) => self.position(file, value, mode),
            Holder::Argument(call, index) => self.escaped_argument(file, at, call, index, mode),
            // An element inserts its child tracked, unless it may be pending.
            Holder::JsxChild => match jsx_child_parent(file, at) {
                Some(element) if intrinsic(file, element) && !self.async_walk => Position::Followed,
                _ => Position::Open(blockers::VALUE_IN_CARRIER),
            },
            Holder::JsxValue(element, _) | Holder::JsxSpread(element) => {
                if intrinsic(file, element) {
                    Position::Followed
                } else {
                    Position::Open(blockers::VALUE_TO_UNRESOLVED_CALLEE)
                }
            }
            Holder::Tag | Holder::TaggedSubstitution => {
                Position::Open(blockers::VALUE_TO_UNRESOLVED_CALLEE)
            }
            Holder::ClassField => Position::Open(blockers::VALUE_IN_CLASS_FIELD),
            Holder::ParameterDefault => {
                let parameter = ast
                    .functions
                    .iter()
                    .flat_map(|function| &function.parameters)
                    .find(|parameter| {
                        parameter
                            .initializer
                            .map(|value| ast.peel_ts_sugar_span(value))
                            == Some(at)
                    });
                let Some(parameter) = parameter else {
                    return Position::Open(blockers::VALUE_AS_PARAMETER_DEFAULT);
                };
                for name in &parameter.names {
                    self.name_uses(file, name.span, mode);
                }
                Position::Followed
            }
            Holder::Awaited => Position::Open(blockers::VALUE_THROUGH_AWAIT),
            Holder::Yielded => Position::Open(blockers::VALUE_THROUGH_GENERATOR),
            Holder::Iterated => Position::Open(blockers::VALUE_IN_CARRIER),
        }
    }

    /// ADR 0860: a position no table holds, past an escape.
    fn escaped_unheld(&mut self, file: &'a FileFacts, at: Span, mode: Mode) -> Position {
        let Mode::Escaped { root, blocker } = mode else {
            return self.unheld(file, at, mode);
        };
        if jsx_name(file, at) {
            // `<H />` calls what `H` holds; a root's tag is a Get of it.
            let site = if root { Site::Read } else { Site::Call };
            self.site(file, at, site, blocker);
            return Position::Followed;
        }
        if file.ast.deleted_targets.contains(&at) || exported_directly(file, at) {
            return Position::Followed;
        }
        Position::Open(blockers::VALUE_USE_UNCLASSIFIED)
    }

    /// ADR 0860: a member of a value past an escape. A Get of a root reads it;
    /// a Get of anything else may answer the source; a standard method hands
    /// what the value holds on through what it answers and to its callbacks.
    fn escaped_member(&mut self, file: &'a FileFacts, member: usize, mode: Mode) -> Position {
        let Mode::Escaped { root, blocker } = mode else {
            return Position::Followed;
        };
        let ast = &file.ast;
        let member = &ast.members[member];
        if let Some(call) = ast.calls_with_peeled_callee(member.span).next() {
            let property = member_key(file, member);
            // `h.call(null)` calls `h`; `h.bind(null)` holds it.
            if matches!(property, Some("call" | "apply")) {
                self.site(file, call.span, Site::Call, blocker);
                self.position(file, call.span, mode);
                return Position::Followed;
            }
            if property == Some("bind") {
                self.position(file, call.span, mode);
                return Position::Followed;
            }
            if self
                .lookup
                .standard_library_declaration(file, call, typefacts::CallKind::Call)
                .is_some()
            {
                if root {
                    self.site(file, member.span, Site::Read, blocker);
                }
                self.position(file, call.span, mode);
                self.callback_parameters(file, call, mode);
                return Position::Followed;
            }
            // ADR 0876: a call a reached patch withholds may hand it on just
            // the same. Its row stands at the member below; this only opens
            // further rows.
            if self.lookup.reached_patch_withheld(file, call.callee) {
                self.past_withheld.push((file, call, mode));
            }
        }
        let span = member.span;
        if ast.deleted_targets.contains(&span) {
            if root {
                self.site(file, span, Site::Write, blocker);
            }
            return Position::Followed;
        }
        let written = ast.assignments.iter().any(|assignment| {
            !assignment.reads_target && ast.peel_ts_sugar_span(assignment.target) == span
        });
        // A root's Get is the read, as the root's own reference's is: what it
        // answers is the read rules' (a nested store is read there too).
        if root && !written {
            self.site(file, span, Site::Read, blocker);
            return Position::Followed;
        }
        self.position(file, span, mode)
    }

    /// ADR 0860: the parameters of each callback a call is handed, written in
    /// place or named: a carrier's method hands them what it holds.
    fn callback_parameters(
        &mut self,
        file: &'a FileFacts,
        call: &'a solid_facts::ast::CallFact,
        mode: Mode,
    ) {
        for argument in &call.arguments {
            let value = file.ast.peel_ts_sugar_span(argument.span);
            let function = match file.ast.function_at(value) {
                Some(function) => Some((file, function)),
                None => crate::owners::exact_function_argument(file, value, self.lookup).and_then(
                    |(path, span)| {
                        let (target, span) = self.place(&span.location(path.as_str()))?;
                        Some((target, target.ast.function_at(span)?))
                    },
                ),
            };
            let Some((holder, function)) = function else {
                continue;
            };
            for parameter in &function.parameters {
                for name in &parameter.names {
                    self.name_uses(holder, name.span, mode);
                }
            }
        }
    }

    /// ADR 0860: an argument past an escape. A project function's parameter
    /// holds it there; a standard function, a constructor or a callee nothing
    /// resolves may hand it on through what it answers, its receiver, its
    /// other arguments and its callbacks.
    fn escaped_argument(
        &mut self,
        file: &'a FileFacts,
        at: Span,
        call: usize,
        index: usize,
        mode: Mode,
    ) -> Position {
        let Mode::Escaped { root, blocker } = mode else {
            return Position::Followed;
        };
        let ast = &file.ast;
        let call = &ast.calls[call];
        // A primitive may call it, read it or hold it: the argument is a use,
        // as it is before an escape.
        if self.lookup.primitive_at_call(file, call.span).is_some() {
            return Position::Open(blockers::VALUE_IN_CARRIER);
        }
        if let Some(resolved) =
            self.lookup
                .resolved_callee_call(file, call.callee)
                .filter(|resolved| {
                    resolved
                        .declaration
                        .as_ref()
                        .is_some_and(|declaration| declaration.standard_library)
                })
        {
            if crate::runtime_semantics::reads_keys_only(resolved) {
                return Position::Followed;
            }
            let callability = self
                .lookup
                .entity_at(file.path.as_str(), call.arguments[index].span)
                .and_then(|entity| entity.callability);
            if crate::runtime_semantics::argument_behavior(resolved, callability, index)
                == Some(RuntimeArgumentBehavior::FreshStackCallback)
            {
                return Position::Followed;
            }
            if root {
                self.site(file, at, Site::Read, blocker);
            }
            self.handed_on(file, call, index, mode);
            return Position::Followed;
        }
        if !call.construct
            && !call.arguments[index].spread
            && let Some((callee_file, callee, true)) =
                crate::execution_role::project_callee_at_parameter(file, call, index, self.lookup)
            && let Some(parameter) = callee.parameters.get(index).filter(|parameter| {
                parameter.shape == solid_facts::ast::BindingShape::Identifier
                    && !reads_arguments(callee_file, callee.span)
            })
        {
            if self.parameter_hops >= MAX_HOPS {
                return Position::Open(blockers::VALUE_FORWARDED);
            }
            self.parameter_hops += 1;
            for name in &parameter.names {
                self.name_uses(callee_file, name.span, mode);
            }
            self.parameter_hops -= 1;
            return Position::Followed;
        }
        let blocker = if call.construct {
            blockers::VALUE_THROUGH_NEW
        } else {
            blockers::VALUE_TO_UNRESOLVED_CALLEE
        };
        self.uses.push((at.location(file.path.as_str()), blocker));
        self.handed_on(file, call, index, mode);
        Position::Followed
    }

    /// ADR 0860: where a call may put an argument past an escape: what it
    /// answers, its receiver, its other arguments, its callbacks' parameters.
    fn handed_on(
        &mut self,
        file: &'a FileFacts,
        call: &'a solid_facts::ast::CallFact,
        index: usize,
        mode: Mode,
    ) {
        let ast = &file.ast;
        self.position(file, call.span, mode);
        if let Some(member) = ast.member_at(ast.peel_ts_sugar_span(call.callee)) {
            self.escaped_object(file, member.object, None, mode, 0);
        }
        for (other, argument) in call.arguments.iter().enumerate() {
            if other != index {
                self.escaped_object(file, argument.span, None, mode, 0);
            }
        }
        self.callback_parameters(file, call, mode);
    }

    /// ADR 0860: an object that may now hold the value, written into it or
    /// handed to a call with it: every use of the object, and of what it was
    /// read out of (`const y = x.self()` may be `x`). A global object's slot
    /// is every Get of that key on a global object, in every file.
    fn escaped_object(
        &mut self,
        file: &'a FileFacts,
        object: Span,
        key: Option<&'a str>,
        mode: Mode,
        depth: usize,
    ) {
        let ast = &file.ast;
        let object = ast.peel_ts_sugar_span(object);
        // W3: an object the walk cannot follow further is a use, never a
        // silent end.
        let Mode::Escaped { blocker, .. } = mode else {
            return;
        };
        if depth > MAX_ALIASES {
            self.uses
                .push((object.location(file.path.as_str()), blocker));
            return;
        }
        if global_object(file, object) {
            self.global_slot_uses(key, mode);
            return;
        }
        if let Some(member) = ast.member_at(object) {
            self.escaped_object(file, member.object, None, mode, depth + 1);
            return;
        }
        let Some(declaration) = ast.reference_declaration(object) else {
            // A write's target that is no name (`box().v = c`): what the
            // project function it calls returns, or else the use itself. An
            // argument that is no name hands nothing on past the call's result.
            if written_target(file, object) {
                self.escaped_returned_object(file, object, mode, depth);
            }
            return;
        };
        self.name_uses(file, declaration, mode);
        let initializer = ast
            .bindings
            .iter()
            .find(|binding| {
                binding.shape == solid_facts::ast::BindingShape::Identifier
                    && binding.names.iter().any(|name| name.span == declaration)
            })
            .and_then(|binding| binding.initializer)
            .map(|initializer| ast.peel_ts_sugar_span(initializer));
        let Some(initializer) = initializer else {
            return;
        };
        match ast.call_at(initializer) {
            // A method may answer its receiver.
            Some(call) => {
                if let Some(member) = ast.member_at(ast.peel_ts_sugar_span(call.callee)) {
                    self.escaped_object(file, member.object, None, mode, depth + 1);
                }
            }
            None => self.escaped_object(file, initializer, None, mode, depth + 1),
        }
    }

    /// W3: the object of a member write that is no name: a call of a project
    /// function stands for each value it returns; anything else is a use.
    fn escaped_returned_object(
        &mut self,
        file: &'a FileFacts,
        object: Span,
        mode: Mode,
        depth: usize,
    ) {
        let Mode::Escaped { blocker, .. } = mode else {
            return;
        };
        let callee = file
            .ast
            .call_at(object)
            .filter(|call| !call.construct)
            .and_then(|call| {
                self.lookup.function_called_at(
                    file.path.as_str(),
                    file.ast.peel_ts_sugar_span(call.callee),
                )
            });
        // W5 (R19a): `function_called_at` does not follow a call's result
        // (`const box = make(() => 0); box()`), so its answer is taken only
        // when it is the callee name's own function in this file: its
        // declaration, or a binding initialized with that literal.
        let own = callee.is_some_and(|(callee_file, function)| {
            std::ptr::eq(callee_file, file) && own_function(file, object, function.span)
        });
        let Some((callee_file, function)) = callee.filter(|_| own) else {
            // `const box = make(() => 0); box()`: what the functions `make`
            // returns return, when `make` is a project function by its own
            // name; anything else is the use.
            if !self.made_returns(file, object, mode, depth) {
                self.uses
                    .push((object.location(file.path.as_str()), blocker));
            }
            return;
        };
        let mut returned = callee_file
            .ast
            .returns
            .iter()
            .filter(|returned| {
                crate::owners::containing_ast_function(&callee_file.ast, returned.span)
                    .is_some_and(|holder| holder.span == function.span)
            })
            .filter_map(|returned| returned.argument)
            .collect::<Vec<_>>();
        if function.expression_body {
            returned.push(function.body);
        }
        if returned.is_empty() {
            self.uses
                .push((object.location(file.path.as_str()), blocker));
        }
        for value in returned {
            self.escaped_object(callee_file, value, None, mode, depth + 1);
        }
    }

    /// W5 (R19a): the callee of the call at `call` is a `const` bound to a
    /// call of a project function by its own name; each function literal it
    /// returns stands for what it returns. False when that is not proved.
    fn made_returns(&mut self, file: &'a FileFacts, call: Span, mode: Mode, depth: usize) -> bool {
        let ast = &file.ast;
        let Some(callee) = ast
            .call_at(call)
            .map(|call| ast.peel_ts_sugar_span(call.callee))
        else {
            return false;
        };
        let Some(declaration) = ast.reference_declaration(callee) else {
            return false;
        };
        let Some(maker) = ast
            .bindings
            .iter()
            .find(|binding| {
                binding.immutable && binding.names.iter().any(|name| name.span == declaration)
            })
            .and_then(|binding| binding.initializer)
            .map(|value| ast.peel_ts_sugar_span(value))
            .filter(|value| ast.call_at(*value).is_some())
        else {
            return false;
        };
        let made = ast.call_at(maker).and_then(|inner| {
            self.lookup
                .function_called_at(file.path.as_str(), ast.peel_ts_sugar_span(inner.callee))
        });
        let Some((maker_file, maker_function)) = made.filter(|(maker_file, function)| {
            std::ptr::eq(*maker_file, file) && own_function(file, maker, function.span)
        }) else {
            return false;
        };
        let returned = returned_values(maker_file, maker_function);
        let functions = returned
            .iter()
            .filter_map(|value| {
                maker_file
                    .ast
                    .function_at(maker_file.ast.peel_ts_sugar_span(*value))
            })
            .collect::<Vec<_>>();
        if functions.is_empty() || functions.len() != returned.len() {
            return false;
        }
        for function in functions {
            for value in returned_values(maker_file, function) {
                self.escaped_object(maker_file, value, None, mode, depth + 1);
            }
        }
        true
    }

    /// W3: whether the function holding `span` is a literal a standard
    /// function takes at a fresh-stack slot.
    fn fresh_stack_callback(&self, file: &FileFacts, span: Span) -> bool {
        let ast = &file.ast;
        let Some(function) = crate::owners::containing_ast_function(ast, span) else {
            return false;
        };
        ast.arguments_containing(function.span)
            .any(|(call, index)| {
                ast.peel_ts_sugar_span(call.arguments[index].span) == function.span
                    && self
                        .lookup
                        .resolved_callee_call(file, call.callee)
                        .filter(|resolved| {
                            resolved
                                .declaration
                                .as_ref()
                                .is_some_and(|declaration| declaration.standard_library)
                        })
                        .is_some_and(|resolved| {
                            let callability = self
                                .lookup
                                .entity_at(file.path.as_str(), call.arguments[index].span)
                                .and_then(|entity| entity.callability);
                            crate::runtime_semantics::argument_behavior(
                                resolved,
                                callability,
                                index,
                            ) == Some(RuntimeArgumentBehavior::FreshStackCallback)
                        })
            })
    }

    /// ADR 0860: every Get of `key` on a global object, and every reference of
    /// the global of that name. `None`: a key no text names, so every Get.
    fn global_slot_uses(&mut self, key: Option<&'a str>, mode: Mode) {
        let slots = self.lookup.slot_entries(self.lookup.base_members());
        for (holder, _) in slots {
            // Both member keys and unresolved identifier text affect the census.
            self.lookup.note_source_read(holder);
            let ast = &holder.ast;
            for member in &ast.members {
                if !global_object(holder, ast.peel_ts_sugar_span(member.object)) {
                    continue;
                }
                if let (Some(key), Some(named)) = (key, member_key(holder, member))
                    && key != named
                {
                    continue;
                }
                self.position(holder, member.span, mode);
            }
            let Some(key) = key else {
                continue;
            };
            for identifier in &ast.identifiers {
                if identifier.role == IdentifierRole::Reference
                    && holder.source_text(identifier.span) == Some(key)
                    && ast.reference_declaration(identifier.span).is_none()
                {
                    self.position(holder, identifier.span, mode);
                }
            }
        }
    }

    /// ADR 0860: what a function past an escape returns goes where each call
    /// of it puts it, the calls in view; a primitive's callback's, and an
    /// IIFE's, where the call puts it.
    fn escaped_returned(
        &mut self,
        file: &'a FileFacts,
        function: Option<Span>,
        mode: Mode,
    ) -> Position {
        let Some(function) = function else {
            return Position::Open(blockers::VALUE_USE_UNCLASSIFIED);
        };
        let ast = &file.ast;
        let primitive = ast
            .arguments_containing(function)
            .find(|(call, index)| {
                ast.peel_ts_sugar_span(call.arguments[*index].span) == function
                    && self.lookup.primitive_at_call(file, call.span).is_some()
            })
            .map(|(call, _)| call.span);
        if let Some(call) = primitive.or_else(|| {
            ast.calls_with_peeled_callee(function)
                .next()
                .map(|call| call.span)
        }) {
            return self.position(file, call, mode);
        }
        if self.return_hops >= MAX_HOPS {
            return Position::Open(blockers::VALUE_FORWARDED);
        }
        self.return_hops += 1;
        let mut reached = false;
        for (caller, callee) in self
            .lookup
            .function_call_sites(file.path.as_str(), function)
        {
            let calls = caller
                .ast
                .calls
                .iter()
                .filter(|call| caller.ast.peel_ts_sugar_span(call.callee) == callee)
                .map(|call| call.span)
                .collect::<Vec<_>>();
            for call in calls {
                reached = true;
                self.position(caller, call, mode);
            }
        }
        self.return_hops -= 1;
        if reached {
            Position::Followed
        } else {
            Position::Open(blockers::VALUE_TO_UNRESOLVED_CALLEE)
        }
    }

    /// ADR 0860: an open answer this row gave itself, not one a use further on
    /// gave (that use has its row, and its own escape).
    fn escape_if_here(
        &mut self,
        file: &'a FileFacts,
        at: Span,
        row: Holder,
        mode: Mode,
        answer: Position,
        recorded: usize,
    ) {
        if let Position::Open(blocker) = answer
            && self.uses.len() == recorded
        {
            self.escape(file, at, row, mode, blocker);
        }
    }

    /// Whether a call of `callee` has a read row: it names a source, or
    /// transport resolved it to one.
    fn has_row(&self, file: &FileFacts, callee: Span) -> bool {
        let callee = file.ast.peel_ts_sugar_span(callee);
        let (start, end) = (u64::from(callee.start), u64::from(callee.end));
        self.resolved
            .contains_key(&callee.location(file.path.as_str()))
            || self
                .lookup
                .callee_symbol(file, callee)
                .is_some_and(|symbol| self.sources.contains(symbol))
            || self.rows.get(file.path.as_str()).is_some_and(|rows| {
                // The row is the call's own: at its callee or at the call. A
                // row inside the callee (`get().c`) or around it is another's.
                let calls = file
                    .ast
                    .calls_with_peeled_callee(callee)
                    .map(|call| (u64::from(call.span.start), u64::from(call.span.end)))
                    .collect::<Vec<_>>();
                rows.iter()
                    .any(|row| *row == (start, end) || calls.contains(row))
            })
    }

    /// ADR 0811: where the census asks which store member writes SC2003
    /// decides in a file. Without it, none.
    pub(crate) fn with_member_writes(mut self, member_writes: MemberWrites<'l>) -> Self {
        self.member_writes = member_writes;
        self
    }

    /// ADR 0811: whether a store member write SC2003 decided stands on the
    /// chain of `member`.
    fn write_decided(&mut self, file: &FileFacts, member: Span) -> bool {
        let member_writes = &self.member_writes;
        self.decided_writes
            .entry(file.path.to_string())
            .or_insert_with(|| member_writes(file))
            .iter()
            .any(|written| written.start == member.start && written.end >= member.end)
    }

    /// Whether the call at `callee` runs where the rules of what it may do do
    /// not speak: an accessor's read, a setter's or an action's write. No row
    /// says so at a call the walk reached through a name.
    fn role_clears(&self, file: &FileFacts, callee: Span) -> bool {
        let role = crate::execution_role::read_execution_role(
            file,
            callee,
            &crate::execution_role::allowed_callback_spans(file, self.lookup),
            self.entities,
            self.symbol_names,
            self.lookup,
        );
        let reads = !self.async_walk
            && role != crate::ExecutionRole::Unknown
            && crate::execution_role::past_suspension(file, callee, self.lookup).is_none()
            && !role.reports_untracked_read();
        let writes = role != crate::ExecutionRole::Unknown && !role.reports_disallowed_write();
        match self.carried {
            Carried::Accessor => reads,
            Carried::Writer => writes,
            Carried::Either => reads && writes,
        }
    }

    fn member(&mut self, file: &'a FileFacts, at: Span, member: usize, mode: Mode) -> Position {
        let ast = &file.ast;
        let member = &ast.members[member];
        match mode {
            Mode::Value => {}
            // The root's own reference: its member Gets and Sets have their
            // rows. Reached through a slot, an alias or a prop, a member has
            // one only where a row stands on its chain (W's review H4).
            Mode::Root => {
                if self.steps <= 1 {
                    return Position::Followed;
                }
                let (start, end) = (u64::from(member.span.start), u64::from(member.span.end));
                let rowed = self.rows.get(file.path.as_str()).is_some_and(|rows| {
                    rows.iter()
                        .any(|(row_start, row_end)| *row_start == start && *row_end >= end)
                }) || self.write_decided(file, member.span);
                return if rowed {
                    Position::Followed
                } else {
                    Position::Open(blockers::VALUE_HELD_IN_BINDING)
                };
            }
            Mode::RootParameter => return Position::Open(blockers::VALUE_ROOT_IN_HELPER),
            // Past an escape a member is `escaped_member`'s.
            Mode::Escaped { .. } => return Position::Followed,
            Mode::Holder(slot) | Mode::RootHolder(slot) => {
                // A standard method of the holder hands the slot on through
                // what it answers, unless a callback takes it, which the
                // handed net follows (`handed_value`).
                let carried = ast
                    .calls_with_peeled_callee(member.span)
                    .next()
                    .is_some_and(|call| {
                        self.lookup
                            .placing_standard_declaration(file, call)
                            .is_some()
                            && call.arguments.first().is_none_or(|argument| {
                                ast.function_at(ast.peel_ts_sugar_span(argument.span))
                                    .is_none()
                            })
                    });
                if carried {
                    return Position::Open(blockers::VALUE_IN_CARRIER);
                }
                // Another slot's Get holds nothing of this one.
                let key = match ast
                    .literal_computed_members
                    .iter()
                    .find(|literal| literal.span == member.span)
                {
                    Some(literal) => Some(literal.key.as_str()),
                    None if ast.computed_members.contains(&member.span) => None,
                    None => file.source_text(member.property),
                };
                if let (Slot::Key(_), Some(key)) = (slot, key)
                    && self.key(key) != slot
                {
                    return Position::Followed;
                }
                let got = if matches!(mode, Mode::Holder(_)) {
                    Mode::Value
                } else {
                    Mode::Root
                };
                return self.position(file, member.span, got);
            }
        }
        let property = file.source_text(member.property).unwrap_or_default();
        // `(a as any).call(null)`: no dispatch row sees through the cast.
        if matches!(property, "call" | "apply" | "bind") && member.object != at {
            return Position::Open(blockers::VALUE_BOUND_HELD);
        }
        if property == "bind" {
            let bound = ast
                .calls_with_peeled_callee(member.span)
                .next()
                .is_some_and(|bind| ast.calls_with_peeled_callee(bind.span).next().is_some());
            if !bound {
                return Position::Open(blockers::VALUE_BOUND_HELD);
            }
        }
        Position::Followed
    }

    /// A value in a literal's slot: the literal holds it there, unless a
    /// member of the literal may read it through `this`.
    fn held_in_literal(
        &mut self,
        file: &'a FileFacts,
        row: Span,
        slot: Slot,
        mode: Mode,
    ) -> Position {
        let held = if mode == Mode::Value {
            Mode::Holder(slot)
        } else {
            Mode::RootHolder(slot)
        };
        match enclosing_literal(file, row) {
            Some(literal) if !literal_reads_this(file, literal) => {
                self.position(file, literal, held)
            }
            _ => Position::Open(blockers::VALUE_IN_CARRIER),
        }
    }

    /// W's review H5: a bare child is what its parent does with it. An
    /// intrinsic element inserts it, tracked; a project component gets it as
    /// `props.children`; a primitive, a provider, a fragment or an unresolved
    /// tag hands it on where no row reads it.
    fn jsx_child(&mut self, file: &'a FileFacts, at: Span, mode: Mode) -> Position {
        let Some(index) = jsx_child_parent(file, at) else {
            return Position::Open(blockers::VALUE_IN_CARRIER);
        };
        // An element inserts and calls a bare accessor child: a source that
        // may be pending is read there with no row (review item 10).
        if intrinsic(file, index) {
            return if self.async_walk && mode == Mode::Value {
                Position::Open(blockers::VALUE_PENDING_IN_JSX)
            } else {
                Position::Followed
            };
        }
        let element = &file.ast.jsx_elements[index];
        let primitive = crate::exact_jsx_primitive_name(
            file,
            element,
            self.entities,
            self.symbol_names,
            self.lookup.dialect,
        );
        if primitive.is_some() || self.context_provider_tag(file, element.name.span) {
            return Position::Open(blockers::VALUE_IN_CARRIER);
        }
        match self
            .lookup
            .function_called_at(file.path.as_str(), element.name.span)
        {
            Some((component_file, component)) => {
                self.prop_reads(component_file, component, "children", mode)
            }
            None => Position::Open(blockers::VALUE_TO_UNRESOLVED_CALLEE),
        }
    }

    fn jsx_value(
        &mut self,
        file: &'a FileFacts,
        element: usize,
        slot: Option<usize>,
        mode: Mode,
    ) -> Position {
        let element = &file.ast.jsx_elements[element];
        let tag = file.source_text(element.name.span).unwrap_or_default();
        let intrinsic =
            element.member_object.is_none() && tag.chars().next().is_some_and(char::is_lowercase);
        let primitive = crate::exact_jsx_primitive_name(
            file,
            element,
            self.entities,
            self.symbol_names,
            self.lookup.dialect,
        );
        // `<Dynamic>` hands its props to a component the tag does not name.
        let known_tag = primitive
            .is_some_and(|name| name.primitive() != Some(solid_dialect::Primitive::Dynamic));
        if intrinsic || known_tag {
            return Position::Followed;
        }
        if self.context_provider_tag(file, element.name.span) {
            return self.context_readers(file, element.name.span, mode);
        }
        let Some((component_file, component)) = self
            .lookup
            .function_called_at(file.path.as_str(), element.name.span)
        else {
            return Position::Open(blockers::VALUE_TO_UNRESOLVED_CALLEE);
        };
        // A root's spread is the child's own props root. A literal's slots are
        // no attribute the walk can name.
        let Some(prop) = slot.and_then(|slot| file.source_text(element.attributes[slot].name))
        else {
            return if matches!(mode, Mode::Holder(_) | Mode::RootHolder(_)) {
                Position::Open(blockers::VALUE_IN_CARRIER)
            } else {
                Position::Followed
            };
        };
        self.prop_reads(component_file, component, prop, mode)
    }

    /// P12: a prop goes where its `props.<prop>` reads and its destructured
    /// slot put it, one hop.
    fn prop_reads(
        &mut self,
        file: &'a FileFacts,
        component: &'a solid_facts::ast::FunctionFact,
        prop: &str,
        mode: Mode,
    ) -> Position {
        let Some(parameter) = component.parameters.first() else {
            return Position::Followed;
        };
        if self.return_hops >= MAX_HOPS {
            return Position::Open(blockers::VALUE_FORWARDED);
        }
        self.return_hops += 1;
        let mut answer = Position::Followed;
        for name in &parameter.names {
            let identifier = parameter.shape == solid_facts::ast::BindingShape::Identifier;
            // A pattern names the prop by its key, not its local (review item
            // 3); a rest element or a nested pattern holds props no key names.
            if !identifier {
                match parameter
                    .object_slots
                    .iter()
                    .find(|slot| slot.local.span == name.span)
                {
                    Some(slot) if slot.property.as_str() == prop => {}
                    Some(_) => continue,
                    None => {
                        if answer == Position::Followed {
                            answer = Position::Open(blockers::VALUE_FORWARDED);
                        }
                        continue;
                    }
                }
            }
            for reference in self.references(file, name.span) {
                let at = if identifier {
                    // Another prop's static Get holds nothing of this one; any
                    // other use of the props object hands it on.
                    let member = file
                        .ast
                        .members
                        .iter()
                        .find(|member| file.ast.peel_ts_sugar_span(member.object) == reference);
                    let key = member.and_then(|member| {
                        match file
                            .ast
                            .literal_computed_members
                            .iter()
                            .find(|literal| literal.span == member.span)
                        {
                            Some(literal) => Some(literal.key.as_str()),
                            None if file.ast.computed_members.contains(&member.span) => None,
                            None => file.source_text(member.property),
                        }
                    });
                    match (member, key) {
                        (Some(member), Some(key)) if key == prop => member.span,
                        (Some(_), Some(_)) => continue,
                        _ => {
                            if answer == Position::Followed {
                                answer = Position::Open(blockers::VALUE_FORWARDED);
                            }
                            continue;
                        }
                    }
                } else {
                    reference
                };
                let position = self.position(file, at, mode);
                if answer == Position::Followed {
                    answer = position;
                }
            }
        }
        self.return_hops -= 1;
        answer
    }

    /// P12: a provider's value goes where each `useContext` of it puts it.
    fn context_readers(&mut self, file: &'a FileFacts, tag: Span, mode: Mode) -> Position {
        let Some(symbol) = self.lookup.callee_symbol(file, tag) else {
            return Position::Open(blockers::VALUE_TO_UNRESOLVED_CALLEE);
        };
        if self.return_hops >= MAX_HOPS {
            return Position::Open(blockers::VALUE_FORWARDED);
        }
        self.return_hops += 1;
        let mut answer = Position::Followed;
        for reference in self.lookup.symbol_references(symbol) {
            let Some((reader, at)) = self.place(&reference) else {
                answer = Position::Open(blockers::VALUE_USE_UNCLASSIFIED);
                break;
            };
            if !is_value_reference(reader, at) || jsx_name(reader, at) {
                continue;
            }
            let call = reader
                .ast
                .arguments_containing(at)
                .find(|(call, index)| {
                    *index == 0
                        && reader.ast.peel_ts_sugar_span(call.arguments[0].span) == at
                        && self.lookup.primitive_at_call(reader, call.span)
                            == Some(solid_dialect::Primitive::UseContext)
                })
                .map(|(call, _)| call.span);
            // A context handed on has readers no reference names.
            let position = match call {
                Some(call) => self.position(reader, call, mode),
                None => Position::Open(blockers::VALUE_FORWARDED),
            };
            if answer == Position::Followed {
                answer = position;
            }
        }
        self.return_hops -= 1;
        answer
    }

    /// `<Ctx value={…}>`: transport follows a provider's value to its readers.
    fn context_provider_tag(&self, file: &FileFacts, tag: Span) -> bool {
        let ast = &file.ast;
        ast.reference_declaration(tag)
            .and_then(|declaration| {
                ast.bindings
                    .iter()
                    .find(|binding| binding.names.iter().any(|name| name.span == declaration))
            })
            .and_then(|binding| binding.call_initializer)
            .and_then(|initializer| ast.call_at(initializer))
            .and_then(|call| self.lookup.primitive_at_call(file, call.span))
            == Some(solid_dialect::Primitive::CreateContext)
    }

    fn bound(&mut self, file: &'a FileFacts, at: Span, index: usize, mode: Mode) -> Position {
        let ast = &file.ast;
        let binding = &ast.bindings[index];
        let row = Holder::Binding(index);
        // A `let` holds the value where no row reads it. Its uses are walked
        // all the same, so each has a row of its own.
        let held = |answer: Position| {
            if binding.immutable {
                answer
            } else {
                Position::Open(blockers::VALUE_HELD_IN_BINDING)
            }
        };
        let identifier = binding.shape == solid_facts::ast::BindingShape::Identifier;
        // A pattern over a root is SC1003's; its slots hold links. Over a root a
        // helper received, no row sees it.
        if !identifier && mode == Mode::RootParameter {
            self.escape(file, at, row, mode, blockers::VALUE_ROOT_IN_HELPER);
            return held(Position::Open(blockers::VALUE_ROOT_IN_HELPER));
        }
        // The root's own reference only: reached through a slot, an alias or
        // a prop, no row sees the pattern's keys (review item 5).
        if !identifier && mode == Mode::Root {
            return held(if self.steps <= 1 {
                Position::Followed
            } else {
                self.escape(file, at, row, mode, blockers::VALUE_HELD_IN_BINDING);
                Position::Open(blockers::VALUE_HELD_IN_BINDING)
            });
        }
        let each = match mode {
            Mode::Holder(_) if !identifier => Mode::Value,
            Mode::RootHolder(_) if !identifier => Mode::Root,
            other => other,
        };
        let Some(names) = self.slot_names(binding, mode) else {
            self.escape(file, at, row, mode, blockers::VALUE_IN_CARRIER);
            return held(Position::Open(blockers::VALUE_IN_CARRIER));
        };
        let mut answer = Position::Followed;
        for name in names {
            // A name that is a source itself (ADR 0463's exact alias) is walked
            // from its own references, with its own rows, in the same mode: a
            // value as a value, a root as a root. A holder (a tuple) is not.
            if matches!(each, Mode::Value | Mode::Root)
                && self
                    .lookup
                    .entities()
                    .exact(&name.span.location(file.path.as_str()))
                    .is_some_and(|symbol| self.sources.contains(symbol.as_str()))
            {
                continue;
            }
            let position = self.name_uses(file, name.span, each);
            if answer == Position::Followed {
                answer = position;
            }
        }
        held(answer)
    }

    /// Where a declared name's references put the value it holds: in its
    /// file, and in every file that imports it.
    fn name_uses(&mut self, file: &'a FileFacts, name: Span, mode: Mode) -> Position {
        let mut answer = Position::Followed;
        for reference in self.references(file, name) {
            let position = self.position(file, reference, mode);
            if answer == Position::Followed {
                answer = position;
            }
        }
        // An exported binding is read in other files too.
        let symbol = self
            .lookup
            .entities()
            .exact(&name.location(file.path.as_str()));
        for reference in symbol
            .map(|symbol| self.lookup.symbol_references(symbol.as_str()))
            .unwrap_or_default()
        {
            if reference.path.as_ref() == file.path.as_str() {
                continue;
            }
            let position = match self.place(&reference) {
                None => Position::Open(blockers::VALUE_USE_UNCLASSIFIED),
                Some((holder, at)) if is_value_reference(holder, at) => {
                    self.position(holder, at, mode)
                }
                Some(_) => continue,
            };
            if answer == Position::Followed {
                answer = position;
            }
        }
        answer
    }

    /// A class field holds the value where no row reads it. Its uses are
    /// walked all the same, so each has a row: `this.<key>` in the class, and
    /// a member Get of an instance (`new K()`) or, for a static field, of the
    /// class. A reference of the class the walk cannot follow is a use too.
    fn class_field_uses(&mut self, file: &'a FileFacts, at: Span, mode: Mode) {
        let ast = &file.ast;
        let Some((class, element)) = ast.classes.iter().find_map(|class| {
            class
                .elements
                .iter()
                .find(|element| {
                    element.value.map(|value| ast.peel_ts_sugar_span(value)) == Some(at)
                })
                .map(|element| (class, element))
        }) else {
            return;
        };
        let slot = match element
            .key
            .filter(|_| !element.computed)
            .and_then(|key| file.source_text(key))
            .and_then(written_key)
        {
            Some(key) => self.key(key),
            None => Slot::Any,
        };
        let held = match mode {
            Mode::Value => Mode::Holder(slot),
            Mode::Root => Mode::RootHolder(slot),
            // A holder in a holder, or a helper's root: no member step names it.
            Mode::Holder(_) | Mode::RootHolder(_) | Mode::RootParameter | Mode::Escaped { .. } => {
                return;
            }
        };
        if !element.r#static {
            let reads = ast
                .members
                .iter()
                .filter(|member| {
                    class.span.contains(member.span)
                        && file.source_text(member.object) == Some("this")
                })
                .map(|member| member.object)
                .collect::<Vec<_>>();
            for this in reads {
                self.position(file, this, held);
            }
        }
        let Some(name) = &class.name else {
            return;
        };
        let mut references = self
            .references(file, name.span)
            .into_iter()
            .map(|reference| (file, reference))
            .collect::<Vec<_>>();
        let symbol = self
            .lookup
            .entities()
            .exact(&name.span.location(file.path.as_str()));
        for reference in symbol
            .map(|symbol| self.lookup.symbol_references(symbol.as_str()))
            .unwrap_or_default()
        {
            if reference.path.as_ref() == file.path.as_str() {
                continue;
            }
            match self.place(&reference) {
                Some(place) => references.push(place),
                None => self.uses.push((reference, blockers::VALUE_IN_CLASS_FIELD)),
            }
        }
        for (holder, reference) in references {
            if !is_value_reference(holder, reference) || class.span.contains(reference) {
                continue;
            }
            let construct = holder
                .ast
                .calls
                .iter()
                .find(|call| {
                    call.construct && holder.ast.peel_ts_sugar_span(call.callee) == reference
                })
                .map(|call| call.span);
            match (element.r#static, construct) {
                (false, Some(instance)) => {
                    self.position(holder, instance, held);
                }
                (true, _) => {
                    self.position(holder, reference, held);
                }
                (false, None) => self.uses.push((
                    reference.location(holder.path.as_str()),
                    blockers::VALUE_IN_CLASS_FIELD,
                )),
            }
        }
    }

    /// The names that may hold the walked slot: all of them, but for a static
    /// key the pattern names. `None`: a rest element holds a holder of its own.
    fn slot_names(
        &mut self,
        binding: &'a solid_facts::ast::BindingFact,
        mode: Mode,
    ) -> Option<Vec<&'a solid_facts::ast::NamedSpan>> {
        let all = || binding.names.iter().collect::<Vec<_>>();
        let (Mode::Holder(slot) | Mode::RootHolder(slot)) = mode else {
            return Some(all());
        };
        if binding.shape == solid_facts::ast::BindingShape::Identifier {
            return Some(all());
        }
        let mut named = Vec::new();
        for object in &binding.object_slots {
            let holds = match written_key(&object.property) {
                Some(key) => self.key(key) == slot,
                None => true,
            };
            named.push((holds, &object.local));
        }
        for (index, array) in binding.array_slots.iter().enumerate() {
            if let Some(array) = array {
                named.push((self.key(&index.to_string()) == slot, array));
            }
        }
        if named.len() != binding.names.len() {
            return None;
        }
        if slot == Slot::Any {
            return Some(all());
        }
        Some(
            named
                .into_iter()
                .filter(|(holds, _)| *holds)
                .map(|(_, name)| name)
                .collect(),
        )
    }

    /// A value a function returns: its callers are the handed net's. What a
    /// primitive's callback returns is the primitive's.
    fn returned(&mut self, file: &'a FileFacts, function: Option<Span>, mode: Mode) -> Position {
        let Some(function) = function else {
            return Position::Open(blockers::VALUE_USE_UNCLASSIFIED);
        };
        let primitive_callback = file
            .ast
            .arguments_containing(function)
            .find(|(call, index)| {
                file.ast.peel_ts_sugar_span(call.arguments[*index].span) == function
                    && self.lookup.primitive_at_call(file, call.span).is_some()
            })
            .map(|(call, index)| CallSlot {
                span: call.span,
                index,
            });
        if let Some(call) = primitive_callback {
            // `createRoot` answers what its callback answers.
            let primitive = self.lookup.primitive_at_call(file, call.span);
            match primitive
                .and_then(|primitive| self.lookup.dialect.returned_callback_value(primitive))
            {
                Some(slot) if slot == call.index => self.position(file, call.span, mode),
                _ => Position::Open(blockers::VALUE_FROM_PRIMITIVE_CALLBACK),
            }
        } else if let Some(call) = file.ast.calls_with_peeled_callee(function).next() {
            // An IIFE: its call has no row. Where its value goes is walked all
            // the same, so each use has one.
            self.position(file, call.span, mode);
            Position::Open(blockers::VALUE_HELD_IN_BINDING)
        } else {
            self.callers(file, function, mode)
        }
    }

    /// P12: what a function returns goes where each call puts it, one hop:
    /// the net follows calls, not what a root does past them. A function
    /// handed on has callers no row names.
    fn callers(&mut self, file: &'a FileFacts, function: Span, mode: Mode) -> Position {
        if self.return_hops >= MAX_HOPS {
            return Position::Open(blockers::VALUE_FORWARDED);
        }
        let Some(fact) = file.ast.function_at(function) else {
            return Position::Open(blockers::VALUE_USE_UNCLASSIFIED);
        };
        use crate::local_access::HandOff;
        // A method, a slot or a callback has callers no reference names.
        if crate::owners::own_method_name(file, fact).is_some() {
            return Position::Open(blockers::VALUE_TO_UNRESOLVED_CALLEE);
        }
        let Some(local) =
            crate::local_access::function_reference_hand_offs(file, fact, self.lookup)
        else {
            return Position::Open(blockers::VALUE_TO_UNRESOLVED_CALLEE);
        };
        let foreign = crate::local_access::foreign_reference_hand_offs(file, fact, self.lookup);
        // `ReturnType<typeof useX>` hands nothing over.
        if local
            .iter()
            .any(|(at, hand_off)| *hand_off != HandOff::Call && is_value_reference(file, *at))
            || foreign.iter().any(|(holder, at, hand_off)| {
                *hand_off != HandOff::Call && is_value_reference(holder, *at)
            })
        {
            return Position::Open(blockers::VALUE_TO_UNRESOLVED_CALLEE);
        }
        // An export's callers outside the project, or through a namespace,
        // are none the walk can visit.
        if crate::execution_role::module_exports(file, fact, self.lookup)
            && !self.exported.enumerable(file.path.as_str(), function)
        {
            return Position::Open(blockers::VALUE_FORWARDED);
        }
        // Every call of the function is a site the walk visits: a call the
        // call graph did not resolve to it is a caller no walk reaches, and an
        // empty walk proves nothing.
        let accounted = self
            .lookup
            .function_call_site_references(file.path.as_str(), function);
        let visited = |holder: &FileFacts, at: Span| {
            accounted
                .iter()
                .any(|(site, span)| site.path == holder.path && *span == at)
        };
        if local
            .iter()
            .any(|(at, hand_off)| *hand_off == HandOff::Call && !visited(file, *at))
            || foreign
                .iter()
                .any(|(holder, at, hand_off)| *hand_off == HandOff::Call && !visited(holder, *at))
        {
            return Position::Open(blockers::VALUE_TO_UNRESOLVED_CALLEE);
        }
        self.return_hops += 1;
        let mut answer = Position::Followed;
        for (caller, callee) in self
            .lookup
            .function_call_sites(file.path.as_str(), function)
        {
            let calls = caller
                .ast
                .calls
                .iter()
                .filter(|call| caller.ast.peel_ts_sugar_span(call.callee) == callee)
                .map(|call| call.span)
                .collect::<Vec<_>>();
            for call in calls {
                let position = if caller.ast.calls_with_peeled_callee(call).next().is_some() {
                    Position::Open(blockers::VALUE_TO_UNRESOLVED_CALLEE)
                } else {
                    self.position(caller, call, mode)
                };
                if answer == Position::Followed {
                    answer = position;
                }
            }
        }
        self.return_hops -= 1;
        answer
    }

    fn argument(&mut self, file: &'a FileFacts, call: usize, index: usize, mode: Mode) -> Position {
        let ast = &file.ast;
        let call = &ast.calls[call];
        if call.arguments[index].spread {
            return Position::Open(blockers::VALUE_IN_CARRIER);
        }
        if call.construct {
            return Position::Open(blockers::VALUE_THROUGH_NEW);
        }
        if let Some(primitive) = self.lookup.primitive_at_call(file, call.span) {
            let dialect = self.lookup.dialect;
            let view = matches!(mode, Mode::Root | Mode::RootParameter)
                && (dialect.merges_props_reactivity(primitive) || dialect.splits_props(primitive));
            // A slot that opens a window of its own reads the value by name,
            // and no row sees it.
            let arguments = call.arguments.len();
            if dialect.callback_may_open_strict_window(primitive, index, arguments)
                || dialect.reports_untracked_reads_at(primitive, index, arguments)
            {
                return Position::Open(blockers::VALUE_IN_CARRIER);
            }
            let callback = mode == Mode::Value
                && (dialect.callback_positions(primitive).contains(&index)
                    || dialect
                        .callback_executions(primitive)
                        .iter()
                        .any(|(argument, _)| *argument == index));
            // `mapArray(list, mapFn)` calls `list` only when its result is
            // read: the source goes where the result goes.
            if mode == Mode::Value
                && !callback
                && dialect.callback_requires_return_invocation(primitive, index)
            {
                return self.position(file, call.span, Mode::Value);
            }
            return if view || callback {
                Position::Followed
            } else {
                Position::Open(blockers::VALUE_IN_CARRIER)
            };
        }
        if let Some(resolved) =
            self.lookup
                .resolved_callee_call(file, call.callee)
                .filter(|resolved| {
                    resolved
                        .declaration
                        .as_ref()
                        .is_some_and(|declaration| declaration.standard_library)
                })
        {
            let callability = self
                .lookup
                .entity_at(file.path.as_str(), call.arguments[index].span)
                .and_then(|entity| entity.callability);
            // `Object.keys` runs no getter and answers no value.
            if crate::runtime_semantics::reads_keys_only(resolved) {
                return Position::Followed;
            }
            return match crate::runtime_semantics::argument_behavior(resolved, callability, index) {
                Some(RuntimeArgumentBehavior::FreshStackCallback) => Position::Followed,
                Some(RuntimeArgumentBehavior::ValueOnly)
                    if matches!(mode, Mode::Value)
                        && self.lookup.proven_primitive(file, call.span, 2) =>
                {
                    Position::Followed
                }
                _ => Position::Open(blockers::VALUE_IN_CARRIER),
            };
        }
        let Some((callee_file, callee, exact)) =
            crate::execution_role::project_callee_at_parameter(file, call, index, self.lookup)
        else {
            return Position::Open(blockers::VALUE_TO_UNRESOLVED_CALLEE);
        };
        let Some(parameter) = callee.parameters.get(index).filter(|parameter| {
            exact
                && parameter.shape == solid_facts::ast::BindingShape::Identifier
                && !reads_arguments(callee_file, callee.span)
        }) else {
            return Position::Open(blockers::VALUE_TO_UNRESOLVED_CALLEE);
        };
        let handed = match mode {
            Mode::Root | Mode::RootParameter => Mode::RootParameter,
            other => other,
        };
        // A parameter's call has its row at the one call that hands it; a
        // value handed on again has none.
        if self.parameter_hops > 0 && handed != Mode::RootParameter {
            return Position::Open(blockers::VALUE_FORWARDED);
        }
        self.parameter_hops += 1;
        let mut answer = Position::Followed;
        for name in &parameter.names {
            for reference in self.references(callee_file, name.span) {
                self.hop_starts
                    .insert((callee_file.path.to_string(), reference));
                let position = self.position(callee_file, reference, handed);
                if answer == Position::Followed {
                    answer = position;
                }
            }
        }
        self.parameter_hops -= 1;
        answer
    }
}

/// A member's key as written: a static name or a literal computed key.
/// `None`: a computed key no text names.
fn member_key<'f>(file: &'f FileFacts, member: &solid_facts::ast::MemberFact) -> Option<&'f str> {
    let ast = &file.ast;
    match ast
        .literal_computed_members
        .iter()
        .find(|literal| literal.span == member.span)
    {
        Some(literal) => Some(literal.key.as_str()),
        None if ast.computed_members.contains(&member.span) => None,
        None => file.source_text(member.property),
    }
}

/// What a function returns: each `return`'s argument in its own body, and an
/// arrow's expression body.
fn returned_values(file: &FileFacts, function: &solid_facts::ast::FunctionFact) -> Vec<Span> {
    let mut values = file
        .ast
        .returns
        .iter()
        .filter(|returned| {
            crate::owners::containing_ast_function(&file.ast, returned.span)
                .is_some_and(|holder| holder.span == function.span)
        })
        .filter_map(|returned| returned.argument)
        .collect::<Vec<_>>();
    if function.expression_body {
        values.push(function.body);
    }
    values
}

/// W5: whether the call at `call` calls `function` by its own name: the
/// callee is a reference whose declaration names that function, or a binding
/// whose initializer is that function literal.
fn own_function(file: &FileFacts, call: Span, function: Span) -> bool {
    let ast = &file.ast;
    let Some(callee) = ast
        .call_at(call)
        .map(|call| ast.peel_ts_sugar_span(call.callee))
    else {
        return false;
    };
    let Some(declaration) = ast.reference_declaration(callee) else {
        return false;
    };
    ast.functions.iter().any(|fact| {
        fact.span == function
            && fact
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration)
    }) || ast.bindings.iter().any(|binding| {
        binding.names.iter().any(|name| name.span == declaration)
            && binding
                .initializer
                .map(|value| ast.peel_ts_sugar_span(value))
                == Some(function)
    })
}

/// Whether `object` is the object of a member write's target.
fn written_target(file: &FileFacts, object: Span) -> bool {
    file.ast.assignments.iter().any(|assignment| {
        file.ast
            .member_at(file.ast.peel_ts_sugar_span(assignment.target))
            .is_some_and(|member| file.ast.peel_ts_sugar_span(member.object) == object)
    })
}

/// An unshadowed global object: `globalThis`, `window`, `self`, `global`.
fn global_object(file: &FileFacts, at: Span) -> bool {
    file.source_text(at)
        .is_some_and(|text| GLOBAL_OBJECTS.contains(&text))
        && file.ast.reference_declaration(at).is_none()
        && file
            .ast
            .identifiers_within(at)
            .any(|identifier| identifier.span == at && identifier.role == IdentifierRole::Reference)
}

fn assigned(file: &FileFacts, assignment: usize) -> Position {
    let ast = &file.ast;
    let target = ast.peel_ts_sugar_span(ast.assignments[assignment].target);
    match ast.member_at(target) {
        Some(member) if file.source_text(member.object) == Some("this") => {
            Position::Open(blockers::VALUE_IN_CLASS_FIELD)
        }
        Some(_) => Position::Open(blockers::VALUE_HELD_IN_MEMBER),
        None => Position::Open(blockers::VALUE_HELD_IN_BINDING),
    }
}

/// `export { at }` or `export default at`. An export's span is its whole
/// statement, so a position inside `export function` is not this (1c).
fn exported_directly(file: &FileFacts, at: Span) -> bool {
    file.ast.exports.iter().any(|export| {
        export.span.contains(at)
            && (export
                .specifiers
                .iter()
                .any(|specifier| specifier.local.span == at)
                || (export.kind == solid_facts::ast::ExportKind::Default
                    && file
                        .source
                        .get(export.span.start as usize..at.start as usize)
                        .is_some_and(|head| head.trim() == "export default")))
    })
}

/// The element whose child `at` is, when the innermost JSX container around it
/// is an element, not a fragment.
fn jsx_child_parent(file: &FileFacts, at: Span) -> Option<usize> {
    let ast = &file.ast;
    let size = |span: Span| span.end - span.start;
    let element = ast
        .jsx_elements
        .iter()
        .enumerate()
        .filter(|(_, element)| element.children.iter().any(|child| child.contains(at)))
        .min_by_key(|(_, element)| size(element.span))?;
    let fragment = ast
        .jsx_fragments
        .iter()
        .filter(|fragment| fragment.contains(at))
        .map(|fragment| size(*fragment))
        .min();
    fragment
        .is_none_or(|fragment| size(element.1.span) < fragment)
        .then_some(element.0)
}

/// A lowercase tag with no member object: the DOM element itself.
fn intrinsic(file: &FileFacts, element: usize) -> bool {
    let element = &file.ast.jsx_elements[element];
    element.member_object.is_none()
        && file
            .source_text(element.name.span)
            .and_then(|tag| tag.chars().next())
            .is_some_and(char::is_lowercase)
}

fn no_member_writes(_: &FileFacts) -> Vec<Span> {
    Vec::new()
}

/// The literal a property or array holder row names.
fn enclosing_literal(file: &FileFacts, row: Span) -> Option<Span> {
    let ast = &file.ast;
    if ast.array_literal_at(row).is_some() {
        return Some(row);
    }
    ast.object_get_shapes
        .iter()
        .filter(|shape| shape.span.contains(row))
        .min_by_key(|shape| shape.span.end - shape.span.start)
        .map(|shape| shape.span)
}

/// A static key as written, when its text is the key itself: no escape, and a
/// number only as a canonical index (`0x1`, `1.0` and `1e0` name `"1"`).
/// `None`: the key may name any slot.
fn written_key(text: &str) -> Option<&str> {
    let key = text.trim_matches(['"', '\'']);
    let numeric = key.starts_with(|c: char| c.is_ascii_digit() || c == '.');
    let canonical =
        key == "0" || (!key.starts_with('0') && key.bytes().all(|b| b.is_ascii_digit()));
    (!key.contains('\\') && (!numeric || canonical)).then_some(key)
}

/// The object literal `spread` is written in: none in an array or a call's
/// arguments, the nearest container either way.
fn object_spread_literal(file: &FileFacts, spread: Span) -> Option<Span> {
    let ast = &file.ast;
    let size = |span: Span| span.end - span.start;
    let object = enclosing_literal(file, spread)?;
    let array = ast
        .array_literal_elements
        .iter()
        .map(|literal| literal.span)
        .filter(|span| span.contains(spread))
        .map(size)
        .min();
    let call = ast
        .calls
        .iter()
        .filter(|call| {
            call.arguments
                .iter()
                .any(|argument| argument.span.contains(spread))
        })
        .map(|call| size(call.span))
        .min();
    (array.is_none_or(|array| size(object) < array) && call.is_none_or(|call| size(object) < call))
        .then_some(object)
}

fn literal_reads_this(file: &FileFacts, literal: Span) -> bool {
    file.ast.members.iter().any(|member| {
        literal.contains(member.span) && file.source_text(member.object) == Some("this")
    })
}

/// `arguments` reads what no parameter's references show.
fn reads_arguments(file: &FileFacts, function: Span) -> bool {
    file.ast.identifiers_within(function).any(|identifier| {
        identifier.role == IdentifierRole::Reference
            && file.source_text(identifier.span) == Some("arguments")
    })
}

/// A JSX tag's name, opening or closing.
fn jsx_name(file: &FileFacts, at: Span) -> bool {
    file.ast.jsx_elements.iter().any(|element| {
        element.name.span.contains(at) || element.closing_name.is_some_and(|name| name.contains(at))
    })
}

/// A reference that holds a value: not a type query, not a declaration.
fn is_value_reference(file: &FileFacts, span: Span) -> bool {
    !file
        .ast
        .type_queries
        .iter()
        .any(|query| query.contains(span))
        && (file.ast.identifiers_within(span).any(|identifier| {
            identifier.span == span && identifier.role == IdentifierRole::Reference
        }) || file
            .ast
            .object_properties
            .iter()
            .any(|property| property.shorthand_binding.is_some() && property.value == span))
}

#[cfg(test)]
mod tests {
    use super::*;
    use solid_facts::compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap};
    use solid_facts::core::{Generation, SourceHash, SourcePath};
    use std::sync::Arc;

    fn facts(source: &str) -> FileFacts {
        FileFacts {
            generation: Generation::new(1).unwrap(),
            path: SourcePath::new("app.tsx").unwrap(),
            source_hash: SourceHash::of(source),
            source: Arc::from(source),
            ast: Arc::new(solid_facts::ast::extract("app.tsx", source).unwrap()),
            compiler: Arc::new(ExecutionMap {
                compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
                source_hash: SourceHash::of(source),
                semantic_model: Default::default(),
                tracked_regions: Vec::new(),
                untracked_regions: Vec::new(),
                discarded_regions: Vec::new(),
                ownership_regions: Vec::new(),
                callback_roles: Vec::new(),
                jsx_operations: Vec::new(),
            }),
        }
    }

    fn span_of(source: &str, needle: &str) -> Span {
        let start = source.find(needle).expect("needle occurs in source");
        Span::new(
            u32::try_from(start).unwrap(),
            u32::try_from(start + needle.len()).unwrap(),
        )
    }

    const PLACEMENTS: &[&str] = &[
        "$\n",
        "function inner() { $ }\n",
        "export function App() { $ }\n",
        "export default function App() { $ }\n",
        "export const App = () => { $ };\n",
    ];

    /// 1c: an export's span is its whole statement, so a position inside one
    /// is no export. Each form is one no table holds; placed at module scope
    /// alone, this test would prove nothing about the export arm.
    #[test]
    fn a_form_no_table_holds_reaches_the_default_at_every_placement() {
        for form in [
            "throw HELD;",
            "void { [HELD]: 1 };",
            "void o[HELD];",
            "class A extends HELD {}",
        ] {
            for placement in PLACEMENTS {
                let source = placement.replace('$', form);
                let file = facts(&source);
                let at = span_of(&source, "HELD");
                let index = FileIndex::of(&file);
                assert!(!index.holders.contains_key(&at), "{source}");
                assert!(!exported_directly(&file, at), "{source}");
                assert!(!jsx_name(&file, at), "{source}");
            }
        }
    }

    #[test]
    fn only_a_direct_export_is_an_export() {
        for (source, exported) in [
            ("const HELD = 1;\nexport { HELD };\n", true),
            ("const HELD = 1;\nexport default HELD;\n", true),
            ("const HELD = 1;\nexport const a = [HELD];\n", false),
            (
                "const HELD = 1;\nexport function f() { return HELD; }\n",
                false,
            ),
            (
                "const HELD = 1;\nexport default function f() { throw HELD; }\n",
                false,
            ),
        ] {
            let file = facts(source);
            let at = source.rfind("HELD").unwrap();
            let at = Span::new(
                u32::try_from(at).unwrap(),
                u32::try_from(at + "HELD".len()).unwrap(),
            );
            assert_eq!(exported_directly(&file, at), exported, "{source}");
        }
    }

    #[test]
    fn a_value_nothing_takes_is_inert_and_a_coerced_one_is_not() {
        for (source, inert) in [
            ("HELD;\n", true),
            ("void HELD;\n", true),
            ("typeof HELD;\n", true),
            ("if (HELD === other) {}\n", true),
            ("if (other !== HELD) {}\n", true),
            ("if (HELD == other) {}\n", false),
            ("const a = () => HELD;\n", false),
            ("const a = HELD;\n", false),
        ] {
            let file = facts(source);
            let at = span_of(source, "HELD");
            let rows = FileIndex::of(&file);
            let found = rows
                .holders
                .get(&at)
                .is_some_and(|rows| rows.iter().any(|row| matches!(row, Holder::Inert)));
            assert_eq!(found, inert, "{source}");
        }
    }

    #[test]
    fn a_type_query_holds_no_value() {
        for source in [
            "const c = 1;\nlet x: ReturnType<typeof HELD>;\n",
            "const p = { a: 1 };\nlet x: typeof HELD.a;\n",
        ] {
            let file = facts(source);
            assert!(
                !is_value_reference(&file, span_of(source, "HELD")),
                "{source}"
            );
        }
    }

    /// A slot is told apart by its key only when the text is the key.
    #[test]
    fn only_a_key_written_as_itself_names_one_slot() {
        for (text, key) in [
            ("a", Some("a")),
            ("\"a\"", Some("a")),
            ("'b'", Some("b")),
            ("0", Some("0")),
            ("12", Some("12")),
            ("0x1", None),
            ("1.0", None),
            ("1e0", None),
            ("01", None),
            (".5", None),
            ("1n", None),
            ("\"\\x61\"", None),
        ] {
            assert_eq!(written_key(text), key, "{text}");
        }
    }

    #[test]
    fn a_function_may_read_its_arguments_past_its_parameters() {
        let source = "function f(a) { return arguments[0]; }\nfunction g(a) { return a; }\n";
        let file = facts(source);
        assert!(reads_arguments(
            &file,
            span_of(source, "function f(a) { return arguments[0]; }")
        ));
        assert!(!reads_arguments(
            &file,
            span_of(source, "function g(a) { return a; }")
        ));
    }
}
