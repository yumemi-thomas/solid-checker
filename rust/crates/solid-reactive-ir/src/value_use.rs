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
}

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
    rows: BTreeMap<String, Vec<(u64, u64)>>,
    /// ADR 0811: the store member writes SC2003 decides in a file, asked
    /// once per file the walk reaches a root's member in.
    member_writes: &'l dyn Fn(&FileFacts) -> Vec<Span>,
    decided_writes: HashMap<String, Vec<Span>>,
    /// What a call of the walked value does.
    carried: Carried,
    steps: usize,
    parameter_hops: usize,
    return_hops: usize,
    /// Whether an exported function's callers are all in view.
    exported: crate::attribution::ExportedCallers<'l, 'a>,
    /// The parameter references a hop starts from: a call of one has its row
    /// at the call that hands the value. An alias or pattern past it has none.
    hop_starts: HashSet<(String, Span)>,
    /// The walked source may be pending: a tracked position is SC5003's and
    /// SC5001's question then, which only a row answers (W's review H3).
    async_walk: bool,
}

impl<'l, 'a> Census<'l, 'a> {
    pub(crate) fn new<'x>(
        lookup: &'l SemanticLookup<'a>,
        entities: &'l crate::EntitySymbols,
        symbol_names: &'l HashMap<crate::SymbolId, crate::SymbolId>,
        sources: HashSet<&'l str>,
        resolved: &'l HashMap<Location, crate::interproc::accessor_provenance::Callable>,
        exported: crate::attribution::ExportedCallers<'l, 'a>,
        located: impl Iterator<Item = &'x Location>,
    ) -> Self {
        let mut rows = BTreeMap::<String, Vec<(u64, u64)>>::default();
        for location in located {
            rows.entry(location.path.to_string())
                .or_default()
                .push((location.start_byte, location.end_byte));
        }
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
            member_writes: &no_member_writes,
            decided_writes: HashMap::default(),
            carried: Carried::Either,
            steps: 0,
            parameter_hops: 0,
            return_hops: 0,
            exported,
            hop_starts: HashSet::default(),
            async_walk: true,
        }
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
        self.walked.clear();
        self.steps = 0;
        let mode = match (store, tuple) {
            (false, false) => Mode::Value,
            (false, true) => Mode::Holder(Slot::Any),
            (true, false) => Mode::Root,
            (true, true) => Mode::RootHolder(Slot::Any),
        };
        match self.position(file, call, mode) {
            Position::Followed => None,
            Position::Open(blocker) => Some(blocker),
        }
    }

    fn reference_use(&mut self, reference: &Location, mode: Mode) -> Option<&'static str> {
        // A reference this cannot read is no silence (1c).
        let Some((file, span)) = self.place(reference) else {
            return Some(blockers::VALUE_USE_UNCLASSIFIED);
        };
        if !is_value_reference(file, span) {
            return None;
        }
        self.walked.clear();
        self.steps = 0;
        match self.position(file, span, mode) {
            Position::Followed => None,
            Position::Open(blocker) => Some(blocker),
        }
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
            return Position::Open(blockers::VALUE_USE_UNCLASSIFIED);
        }
        let key = (file.path.to_string(), at, mode);
        match self.walked.get(&key) {
            Some(Some(answer)) => return *answer,
            Some(None) => return Position::Open(blockers::VALUE_USE_UNCLASSIFIED),
            None => {}
        }
        self.walked.insert(key.clone(), None);
        let holders = self.holders(file, at);
        // A value several tables name is as open as the worst of them.
        let mut answer = if holders.is_empty() {
            self.unheld(file, at, mode)
        } else {
            Position::Followed
        };
        for row in holders {
            answer = self.through(file, at, row, mode);
            if answer != Position::Followed {
                break;
            }
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
            Holder::MemberObject(member) => self.member(file, at, member, mode),
            Holder::JsxValue(element, slot) => self.jsx_value(file, element, Some(slot), mode),
            Holder::JsxSpread(element) => self.jsx_value(file, element, None, mode),
            Holder::Binding(binding) => self.bound(file, binding, mode),
            Holder::Assigned(assignment) => assigned(file, assignment),
            Holder::Returned(returned) => {
                let function = crate::owners::containing_ast_function(ast, returned);
                self.returned(file, function.map(|function| function.span), mode)
            }
            Holder::ArrowBody(function) => self.returned(file, Some(function), mode),
            Holder::ArrayElement(literal, index) => {
                // A literal in a literal: no row walks two member steps.
                if matches!(mode, Mode::Holder(_) | Mode::RootHolder(_)) {
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
                    return Position::Open(blockers::VALUE_IN_CARRIER);
                }
                let slot = match text.and_then(written_key) {
                    Some(key) => self.key(key),
                    None => Slot::Any,
                };
                self.held_in_literal(file, property.span, slot, mode)
            }
            Holder::Spread => Position::Open(blockers::VALUE_IN_CARRIER),
            // `{ ...state }` copies a holder's slots into a holder: what it
            // held stays one member step away.
            Holder::ObjectSpread(spread) => match (mode, object_spread_literal(file, spread)) {
                (Mode::Holder(_) | Mode::RootHolder(_), Some(literal))
                    if !literal_reads_this(file, literal) =>
                {
                    self.position(file, literal, mode)
                }
                _ => Position::Open(blockers::VALUE_IN_CARRIER),
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
                    Position::Open(blockers::VALUE_THROUGH_LOGICAL)
                }
            }
            Holder::Argument(call, index) => self.argument(file, call, index, mode),
            Holder::TaggedSubstitution => Position::Open(blockers::VALUE_TO_UNRESOLVED_CALLEE),
            Holder::ClassField => Position::Open(blockers::VALUE_IN_CLASS_FIELD),
            Holder::ParameterDefault => Position::Open(blockers::VALUE_AS_PARAMETER_DEFAULT),
            Holder::Awaited => Position::Open(blockers::VALUE_THROUGH_AWAIT),
            Holder::Yielded => Position::Open(blockers::VALUE_THROUGH_GENERATOR),
            Holder::Iterated => Position::Open(blockers::VALUE_IN_CARRIER),
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
    pub(crate) fn with_member_writes(
        mut self,
        member_writes: &'l dyn Fn(&FileFacts) -> Vec<Span>,
    ) -> Self {
        self.member_writes = member_writes;
        self
    }

    /// ADR 0811: whether a store member write SC2003 decided stands on the
    /// chain of `member`.
    fn write_decided(&mut self, file: &FileFacts, member: Span) -> bool {
        let member_writes = self.member_writes;
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
            Mode::Holder(slot) | Mode::RootHolder(slot) => {
                // A standard method of the holder hands the slot on through
                // what it answers, unless a callback takes it, which the
                // handed net follows (`handed_value`).
                let carried = ast
                    .calls_with_peeled_callee(member.span)
                    .next()
                    .is_some_and(|call| {
                        self.lookup
                            .standard_library_declaration(file, call, typefacts::CallKind::Call)
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
        if intrinsic(file, index) {
            return Position::Followed;
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
        'names: for name in &parameter.names {
            let identifier = parameter.shape == solid_facts::ast::BindingShape::Identifier;
            if !identifier && file.source_text(name.span) != Some(prop) {
                continue;
            }
            for reference in self.references(file, name.span) {
                let at = if identifier {
                    match file.ast.members.iter().find(|member| {
                        file.ast.peel_ts_sugar_span(member.object) == reference
                            && file.source_text(member.property) == Some(prop)
                    }) {
                        Some(member) => member.span,
                        None => continue,
                    }
                } else {
                    reference
                };
                answer = self.position(file, at, mode);
                if answer != Position::Followed {
                    break 'names;
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
            answer = match call {
                Some(call) => self.position(reader, call, mode),
                None => Position::Open(blockers::VALUE_FORWARDED),
            };
            if answer != Position::Followed {
                break;
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

    fn bound(&mut self, file: &'a FileFacts, binding: usize, mode: Mode) -> Position {
        let ast = &file.ast;
        let binding = &ast.bindings[binding];
        if !binding.immutable {
            return Position::Open(blockers::VALUE_HELD_IN_BINDING);
        }
        let identifier = binding.shape == solid_facts::ast::BindingShape::Identifier;
        // A pattern over a root is SC1003's; its slots hold links. Over a root a
        // helper received, no row sees it.
        if !identifier && mode == Mode::RootParameter {
            return Position::Open(blockers::VALUE_ROOT_IN_HELPER);
        }
        if !identifier && mode == Mode::Root {
            return Position::Followed;
        }
        let each = match mode {
            Mode::Holder(_) if !identifier => Mode::Value,
            Mode::RootHolder(_) if !identifier => Mode::Root,
            other => other,
        };
        let Some(names) = self.slot_names(binding, mode) else {
            return Position::Open(blockers::VALUE_IN_CARRIER);
        };
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
            for reference in self.references(file, name.span) {
                let position = self.position(file, reference, each);
                if position != Position::Followed {
                    return position;
                }
            }
            // An exported binding is read in other files too.
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
                let Some((holder, at)) = self.place(&reference) else {
                    return Position::Open(blockers::VALUE_USE_UNCLASSIFIED);
                };
                if !is_value_reference(holder, at) {
                    continue;
                }
                let position = self.position(holder, at, each);
                if position != Position::Followed {
                    return position;
                }
            }
        }
        Position::Followed
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
        } else if file.ast.calls_with_peeled_callee(function).next().is_some() {
            // An IIFE: its call has no row.
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
                answer = if caller.ast.calls_with_peeled_callee(call).next().is_some() {
                    Position::Open(blockers::VALUE_TO_UNRESOLVED_CALLEE)
                } else {
                    self.position(caller, call, mode)
                };
                if answer != Position::Followed {
                    break;
                }
            }
            if answer != Position::Followed {
                break;
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
        'references: for name in &parameter.names {
            for reference in self.references(callee_file, name.span) {
                self.hop_starts
                    .insert((callee_file.path.to_string(), reference));
                answer = self.position(callee_file, reference, handed);
                if answer != Position::Followed {
                    break 'references;
                }
            }
        }
        self.parameter_hops -= 1;
        answer
    }
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
