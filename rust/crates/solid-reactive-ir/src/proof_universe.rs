//! ADR 0933, 0936: the site universe. Every place a reactive value is read,
//! written or acted on is a site, found from facts alone (the references
//! TypeScript resolved to each source symbol, and the syntax around them),
//! never from a producer's row, role or walk: a site no producer reached is a
//! site here all the same. [`crate::site_accounting`] ends each site in the
//! final program (a row, a census obligation, a compiler discard, a typed
//! silence its producer named) or gives it an uncertifiable row; with the proof
//! log on it also writes the site with its ending, and
//! `scripts/proof-gate.mjs` fails a site that ends in nothing.
//!
//! The sites, per source symbol and each reference outside dependency source:
//!
//! - the callee of a direct call: of an accessor (a read), a setter (a write),
//!   an action;
//! - the first member Get on a store or props root, unless the chain it starts
//!   is a write path (a read);
//! - a member chain on a root that ends in an assignment, update, destructuring
//!   or iteration target (a store write), in a `delete` operand, or in the
//!   receiver of an in-place array method (`push`, `splice`, ...);
//! - every call of `onFlush` (SC3002), found from the calls' resolved
//!   primitives, not from a source.
//!
//! Each site carries the runtime codes it may raise ([`SiteCode`]: a read raises STRICT_READ_UNTRACKED; a setter call REACTIVE_WRITE_IN_OWNED_SCOPE and,
//! when its source may be an optimistic one that has not answered or its primitive is not resolved, OPTIMISTIC_WRITE_BEFORE_LOAD; and so on), and
//! each code ends on its own: by a row of the table that answers it, an SC9012 (a withholding covers any code that names a line), a finding of a
//! rule the dialect's policy says claims the code, or the compiler's discard (batch 11, slice 4, ADR 0967). A finding of another rule at the same
//! span (a style rule's, SC1002's) answers its own code and ends nothing here.
//!
//! A reference used as a value (passed, returned, stored) is a use the
//! value-use census (ADR 0692) follows or answers with an SC9012; it is not a
//! site here. What this does not list: a props binding destructured in a
//! parameter list, unless TypeScript gives the binding its own symbol among
//! the props sources; `Object.assign(store.a, x)` and the other standard
//! library writes.

use solid_dialect::{CodePolicy, Dialect, RuntimeCode};
use solid_facts::FileFacts;
use solid_facts::ast::MemberFact;
use solid_facts::core::Span;
use typefacts::Location;

use crate::pipeline::AnalysisContext;
use crate::proof_log::{self, SiteRecord};
use crate::{
    ExecutionRole, HashMap, HashSet, Program, ReactiveSourceKind, StaticDefect, StaticViolation,
};

/// The in-place `Array.prototype` methods: the language's own list, not a
/// table of the checker's.
const IN_PLACE_ARRAY_METHODS: [&str; 9] = [
    "push",
    "pop",
    "shift",
    "unshift",
    "splice",
    "sort",
    "reverse",
    "fill",
    "copyWithin",
];

/// What a source symbol is, from the discovery tables.
#[derive(Clone, Copy)]
struct Kind {
    /// A signal or memo accessor: its call is a read.
    accessor: bool,
    /// A setter: its call is a write.
    setter: bool,
    /// An action: its call is an action.
    action: bool,
    /// A store or props root: its members are read, written and deleted.
    root: bool,
    /// The root is a store (any member Get is a reactive read); else a props
    /// root, whose per-prop reactivity is a fact of the props index.
    store: bool,
    /// A setter whose call may raise OPTIMISTIC_WRITE_BEFORE_LOAD ([`optimistic_write_setter`]): its write site carries that code beside the
    /// owned-scope write's.
    optimistic_write: bool,
}

impl Kind {
    fn of(ctx: &AnalysisContext<'_>, symbol: &crate::SymbolId) -> Self {
        let store = ctx.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store);
        Self {
            accessor: ctx.accessors.contains_key(symbol) && !store,
            setter: ctx.setters.contains_key(symbol),
            action: ctx.actions.contains_key(symbol),
            root: store || ctx.prop_sources.contains_key(symbol),
            store,
            optimistic_write: optimistic_write_setter(ctx, symbol),
        }
    }
}

/// Whether a call of the setter `symbol` may raise OPTIMISTIC_WRITE_BEFORE_LOAD, on a dialect whose runtime refuses that write: its source is
/// (or an alias of it is) an optimistic creation that may not have answered (ADR 0636, the one definition the write producer asks as well:
/// [`crate::local_access::optimistic_async_setter`]), or the primitive the setter comes from is not resolved by it or by any alias holding its
/// declaration, so that nothing says the write is not optimistic (a missing fact is an obligation, never a clean).
fn optimistic_write_setter(ctx: &AnalysisContext<'_>, symbol: &crate::SymbolId) -> bool {
    let dialect = ctx.dialect;
    if !dialect.refuses_optimistic_write_before_load() {
        return false;
    }
    let Some(row) = ctx.setters.get(symbol) else {
        return false;
    };
    let resolved = |setter: &crate::SymbolId| {
        ctx.source_primitives
            .get(setter)
            .and_then(|primitive| dialect.primitive(primitive))
            .is_some()
    };
    crate::local_access::optimistic_async_setter(
        dialect,
        ctx.setters,
        ctx.async_sources,
        ctx.source_primitives,
        symbol.as_str(),
    ) || !(resolved(symbol)
        || ctx
            .setters
            .iter()
            .any(|(other, candidate)| candidate.1 == row.1 && resolved(other)))
}

/// The syntax of one file the universe asks about that no cached index of the
/// file's facts answers. The members by object, the calls by callee and the
/// references of a declaration are the facts' own lazily built indexes
/// (`AstFacts`), kept as long as the file's facts are.
struct FileTables {
    /// Peeled assignment, update, destructuring and iteration targets.
    written: HashSet<Span>,
    /// Peeled operands of `delete`.
    deleted: HashSet<Span>,
    /// Indexes into `calls` by each reference that is an operand of a call's
    /// composite callee (`(slot = r)()`, `(flag ? f : g)()`).
    composite_calls: HashMap<Span, Vec<usize>>,
}

impl FileTables {
    fn of(file: &FileFacts) -> Self {
        let ast = &file.ast;
        let mut composite_calls: HashMap<Span, Vec<usize>> = HashMap::default();
        for (index, call) in ast.calls.iter().enumerate() {
            if call.construct {
                continue;
            }
            // A callee that is an identifier, a member or a call is no
            // composite (the walk's own first test, answered by indexed
            // lookups, so the walk runs only for the few callees that are
            // parenthesized conditionals, sequences or assignments).
            let root = ast.peel_ts_sugar_span(call.callee);
            if ast.member_at(root).is_some()
                || ast.call_at(root).is_some()
                || ast
                    .identifiers
                    .binary_search_by_key(&root, |identifier| identifier.span)
                    .is_ok()
            {
                continue;
            }
            for operand in ast.composite_callee_references(call.callee) {
                composite_calls.entry(operand).or_default().push(index);
            }
        }
        Self {
            composite_calls,
            written: ast
                .write_targets
                .iter()
                .map(|target| ast.peel_ts_sugar_span(*target))
                .collect(),
            deleted: ast
                .deleted_targets
                .iter()
                .map(|target| ast.peel_ts_sugar_span(*target))
                .collect(),
        }
    }
}

/// What the chain of members that starts at a root's first Get ends in.
enum ChainEnd {
    /// Nothing writes through it: the first Get is a read. The chain's link
    /// spans are the places a row for the read may sit.
    Read(Vec<Span>),
    /// A member of the chain is an assignment, update, destructuring or
    /// iteration target.
    Assignment(Span),
    /// A member of the chain is the operand of a `delete`.
    Delete(Span),
    /// A member of the chain is the callee of an in-place array method call.
    Mutator { call: Span, callee: Span },
}

fn chain_end(file: &FileFacts, tables: &FileTables, first: &MemberFact) -> ChainEnd {
    let ast = &file.ast;
    let mut link = first;
    let mut links = Vec::new();
    // A chain is as long as the file has members; the bound keeps a malformed
    // table from looping.
    for _ in 0..=ast.members.len() {
        links.push(link.span);
        let peeled = ast.peel_ts_sugar_span(link.span);
        if tables.written.contains(&peeled) {
            return ChainEnd::Assignment(link.span);
        }
        if tables.deleted.contains(&peeled) {
            return ChainEnd::Delete(link.span);
        }
        let method = file.source_text(link.property).unwrap_or("");
        if IN_PLACE_ARRAY_METHODS.contains(&method)
            && let Some(call) = ast
                .calls_with_peeled_callee(peeled)
                .find(|call| !call.construct)
        {
            return ChainEnd::Mutator {
                call: call.span,
                callee: call.callee,
            };
        }
        match ast.members_with_peeled_object(peeled).next() {
            Some(next) => link = next,
            None => return ChainEnd::Read(links),
        }
    }
    ChainEnd::Read(links)
}

/// A Solid dev diagnostic a site of this universe may raise: one variant per code the universe's tables carry, closed so that every code has the
/// uncertifiable row it gets when nothing ends it ([`SiteCode::home`]) and the runtime code the dialect's policy is asked about
/// ([`SiteCode::runtime_code`]).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum SiteCode {
    /// STRICT_READ_UNTRACKED: a reactive read where nothing tracks it.
    StrictRead,
    /// REACTIVE_WRITE_IN_OWNED_SCOPE: a setter call under an owner.
    ReactiveWrite,
    /// OPTIMISTIC_WRITE_BEFORE_LOAD: an optimistic setter called before its source answered.
    OptimisticWrite,
    /// ACTION_CALLED_IN_OWNED_SCOPE.
    ActionCall,
    /// STORE_WRITE_OUTSIDE_SETTER.
    StoreWrite,
    /// ON_FLUSH_IN_OWNED_SCOPE.
    OnFlush,
}

/// The uncertifiable row a code gets when nothing ends it: the rule's id, name and blocker, and the noun its message uses.
pub(crate) struct Home {
    pub(crate) id: &'static str,
    pub(crate) rule: &'static str,
    pub(crate) blocker: &'static str,
    pub(crate) noun: &'static str,
}

impl SiteCode {
    /// Every code of the universe, in the order the proof log writes them.
    pub(crate) const ALL: [Self; 6] = [
        Self::StrictRead,
        Self::ReactiveWrite,
        Self::OptimisticWrite,
        Self::ActionCall,
        Self::StoreWrite,
        Self::OnFlush,
    ];

    pub(crate) const fn runtime_code(self) -> RuntimeCode {
        match self {
            Self::StrictRead => RuntimeCode::StrictReadUntracked,
            Self::ReactiveWrite => RuntimeCode::ReactiveWriteInOwnedScope,
            Self::OptimisticWrite => RuntimeCode::OptimisticWriteBeforeLoad,
            Self::ActionCall => RuntimeCode::ActionCalledInOwnedScope,
            Self::StoreWrite => RuntimeCode::StoreWriteOutsideSetter,
            Self::OnFlush => RuntimeCode::OnFlushInOwnedScope,
        }
    }

    /// The code as Solid's dev dist spells it, for the proof log.
    pub(crate) const fn name(self) -> &'static str {
        self.runtime_code().name()
    }

    /// The home rule of the code: the row of a site no ending, silence or discard accounts for.
    pub(crate) const fn home(self) -> Home {
        match self {
            Self::StrictRead => Home {
                id: "SC1001",
                rule: "strict-read-untracked",
                blocker: crate::blockers::STRICT_READ_UNACCOUNTED,
                noun: "read",
            },
            Self::ReactiveWrite => Home {
                id: "SC2001",
                rule: "no-write-in-pure-computation",
                blocker: crate::blockers::WRITE_UNACCOUNTED,
                noun: "write",
            },
            Self::OptimisticWrite => Home {
                id: "SC2009",
                rule: "optimistic-write-before-load",
                blocker: crate::blockers::OPTIMISTIC_WRITE_UNACCOUNTED,
                noun: "optimistic write",
            },
            Self::ActionCall => Home {
                id: "SC2002",
                rule: "action-called-in-owned-scope",
                blocker: crate::blockers::ACTION_UNACCOUNTED,
                noun: "action call",
            },
            Self::StoreWrite => Home {
                id: "SC2003",
                rule: "no-store-mutation-outside-setter",
                blocker: crate::blockers::STORE_WRITE_UNACCOUNTED,
                noun: "store write",
            },
            Self::OnFlush => Home {
                id: "SC3002",
                rule: "on-flush-in-owned-scope",
                blocker: crate::blockers::ON_FLUSH_UNACCOUNTED,
                noun: "onFlush call",
            },
        }
    }
}

/// The codes of the sites of each table. A write site whose setter may be an optimistic one has the second list.
const READ_CODES: &[SiteCode] = &[SiteCode::StrictRead];
const WRITE_CODES: &[SiteCode] = &[SiteCode::ReactiveWrite];
const OPTIMISTIC_WRITE_CODES: &[SiteCode] = &[SiteCode::ReactiveWrite, SiteCode::OptimisticWrite];
const ACTION_CODES: &[SiteCode] = &[SiteCode::ActionCall];
const STORE_WRITE_CODES: &[SiteCode] = &[SiteCode::StoreWrite];
const ON_FLUSH_CODES: &[SiteCode] = &[SiteCode::OnFlush];

/// One site: where it is, what kind it is, and the other spans a row that
/// answers for it may sit at.
pub(crate) struct Site {
    /// `read`, `write`, `action`, `store-write` or `on-flush`.
    pub(crate) table: &'static str,
    /// What the source is: `accessor`, `store`, `props`, `setter`, `action` or
    /// `primitive`.
    pub(crate) source: &'static str,
    /// The syntactic form: `call`, `member`, `assignment`, `delete`, `mutator`.
    pub(crate) via: &'static str,
    pub(crate) at: Span,
    pub(crate) alternates: Vec<Span>,
    /// The codes the site may raise; each ends (or gets its row) on its own.
    pub(crate) codes: &'static [SiteCode],
}

/// The sites that start at `reference`, a reference of a source of `kind`.
fn sites_at(file: &FileFacts, tables: &FileTables, kind: Kind, reference: Span) -> Vec<Site> {
    let ast = &file.ast;
    let mut sites = Vec::new();
    // The callee is the reference, or a composite whose operand it is.
    let composite = tables
        .composite_calls
        .get(&reference)
        .into_iter()
        .flatten()
        .map(|index| &ast.calls[*index]);
    for call in ast
        .calls_with_peeled_callee(reference)
        .filter(|call| call.direct_callee && !call.construct)
        .chain(composite)
    {
        for (applies, table, source, codes) in [
            (
                kind.setter,
                "write",
                "setter",
                if kind.optimistic_write {
                    OPTIMISTIC_WRITE_CODES
                } else {
                    WRITE_CODES
                },
            ),
            (kind.action, "action", "action", ACTION_CODES),
            (kind.accessor, "read", "accessor", READ_CODES),
        ] {
            if applies {
                sites.push(Site {
                    table,
                    source,
                    via: "call",
                    at: call.span,
                    alternates: vec![call.callee, reference],
                    codes,
                });
            }
        }
    }
    if !kind.root {
        return sites;
    }
    let source = if kind.store { "store" } else { "props" };
    for first in ast.members_with_peeled_object(reference) {
        sites.push(match chain_end(file, tables, first) {
            ChainEnd::Read(links) => {
                // A row for the read sits at any link of the chain, or at a
                // call that has a link as its callee (`props.onClick()`).
                let mut alternates = Vec::new();
                for link in links {
                    alternates.push(link);
                    let peeled = ast.peel_ts_sugar_span(link);
                    for call in ast
                        .calls_with_peeled_callee(peeled)
                        .filter(|call| !call.construct)
                    {
                        alternates.push(call.span);
                        alternates.push(call.callee);
                    }
                }
                Site {
                    table: "read",
                    source,
                    via: "member",
                    at: first.span,
                    alternates,
                    codes: READ_CODES,
                }
            }
            ChainEnd::Assignment(at) => Site {
                table: "store-write",
                source,
                via: "assignment",
                at,
                alternates: Vec::new(),
                codes: STORE_WRITE_CODES,
            },
            ChainEnd::Delete(at) => Site {
                table: "store-write",
                source,
                via: "delete",
                at,
                alternates: Vec::new(),
                codes: STORE_WRITE_CODES,
            },
            ChainEnd::Mutator { call, callee } => Site {
                table: "store-write",
                source,
                via: "mutator",
                at: call,
                alternates: vec![callee],
                codes: STORE_WRITE_CODES,
            },
        });
    }
    sites
}

/// How a site ends in the program: the class of the row, obligation or
/// finding that stands at it, in this order of precedence, or the compiler's
/// discard of the code.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum EndClass {
    /// A row of a table (a read, an async read, a write or an action).
    Row,
    /// An SC9012 obligation: a census row.
    Census,
    /// A defect of any other kind.
    Defect,
    /// A static violation.
    Violation,
    /// The compiler deleted the code.
    Discarded,
}

impl EndClass {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Row => "row",
            Self::Census => "census",
            Self::Defect => "defect",
            Self::Violation => "violation",
            Self::Discarded => "discarded",
        }
    }
}

/// What the row that ends a site is, for the log only (its text is built when
/// the log is on).
#[derive(Clone, Copy)]
pub(crate) enum Detail<'p> {
    Read(ExecutionRole),
    OriginOfRead(ExecutionRole),
    AsyncRead(ExecutionRole),
    OriginOfAsyncRead(ExecutionRole),
    Write(ExecutionRole),
    Action(ExecutionRole),
    Defect(&'p StaticDefect),
    Violation(&'p StaticViolation),
    Nothing,
}

impl Detail<'_> {
    pub(crate) fn text(self) -> String {
        match self {
            Self::Read(role) => format!("read:{role:?}"),
            Self::OriginOfRead(role) => format!("origin-of-read:{role:?}"),
            Self::AsyncRead(role) => format!("async-read:{role:?}"),
            Self::OriginOfAsyncRead(role) => format!("origin-of-async-read:{role:?}"),
            Self::Write(role) => format!("write:{role:?}"),
            Self::Action(role) => format!("action:{role:?}"),
            Self::Defect(defect) => format!(
                "{}:{}",
                defect.kind.dedup_identity(),
                defect.analysis_context
            ),
            Self::Violation(violation) => violation.id.clone(),
            Self::Nothing => String::new(),
        }
    }
}

/// What stands at a span and ends a site, as far as the code asked about is concerned: which codes it answers.
#[derive(Clone, Copy, Debug)]
pub(crate) enum EndedBy<'p> {
    /// A row of one of the program's tables (a read, an async read, a write, an action): the codes of this universe the table answers.
    Table {
        name: &'static str,
        answers: &'static [SiteCode],
    },
    /// An SC9012 obligation: a withholding at a position covers any code that names a line there.
    Census,
    /// A finding (a violation or a defect) of the rule `name`, an obligation when `uncertain`.
    Rule { name: &'p str, uncertain: bool },
    /// The compiler deleted the code.
    Discarded,
}

impl EndedBy<'_> {
    /// The proof log's word for it: `table:<name>`, `census`, `rule:<name>` (`:u` for an obligation) or `discarded`.
    pub(crate) fn text(&self) -> String {
        match self {
            Self::Table { name, .. } => format!("table:{name}"),
            Self::Census => "census".to_owned(),
            Self::Rule { name, uncertain } => {
                format!("rule:{name}{}", if *uncertain { ":u" } else { "" })
            }
            Self::Discarded => "discarded".to_owned(),
        }
    }
}

/// An [`EndedBy`] that owns its rule's name: what a pass that removes a row hands the accounting (which asks it later, per code), so that the site the
/// row stood at ends for the codes the removed row would have answered, and for no other (batch 11, slice 4).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RemovedBy {
    /// A row of one of the program's tables, and the codes of this universe it answers.
    Table {
        name: &'static str,
        answers: &'static [SiteCode],
    },
    /// An SC9012 obligation.
    Census,
    /// A finding of the rule `name`, an obligation when `uncertain`.
    Rule {
        name: std::sync::Arc<str>,
        uncertain: bool,
    },
}

impl RemovedBy {
    /// The ending a removed row was, owned.
    pub(crate) fn of(by: &EndedBy<'_>) -> Option<Self> {
        match by {
            EndedBy::Table { name, answers } => Some(Self::Table { name, answers }),
            EndedBy::Census => Some(Self::Census),
            EndedBy::Rule { name, uncertain } => Some(Self::Rule {
                name: (*name).into(),
                uncertain: *uncertain,
            }),
            // The compiler's discard is no row: nothing removes it.
            EndedBy::Discarded => None,
        }
    }

    /// The ending a removed violation was.
    pub(crate) fn violation(violation: &StaticViolation) -> Self {
        Self::of(&by_rule(&violation.rule, violation.uncertain)).unwrap_or(Self::Census)
    }

    /// The ending a removed defect was: its family's rule.
    pub(crate) fn defect(defect: &StaticDefect) -> Self {
        Self::of(&by_rule(defect.kind.family().rule_name(), defect.uncertain))
            .unwrap_or(Self::Census)
    }

    /// The ending, borrowed, for [`credits`].
    pub(crate) fn by(&self) -> EndedBy<'_> {
        match self {
            Self::Table { name, answers } => EndedBy::Table { name, answers },
            Self::Census => EndedBy::Census,
            Self::Rule { name, uncertain } => EndedBy::Rule {
                name,
                uncertain: *uncertain,
            },
        }
    }
}

/// The rule SC9012 is reported under: its findings are census obligations, whatever family or channel they reach the report by.
const CENSUS_RULE: &str = "reactive-dispatch-unresolved";

/// A finding of the rule `name` as what ends a site: SC9012's are the census, the others a claim of their own rule.
pub(crate) fn by_rule(name: &str, uncertain: bool) -> EndedBy<'_> {
    if name == CENSUS_RULE {
        EndedBy::Census
    } else {
        EndedBy::Rule { name, uncertain }
    }
}

pub(crate) const READ_ROW: EndedBy<'static> = EndedBy::Table {
    name: "read",
    answers: READ_CODES,
};
/// An async read's row answers the pending codes (ADR 0838 accounts for them): none of this universe's.
pub(crate) const ASYNC_READ_ROW: EndedBy<'static> = EndedBy::Table {
    name: "async-read",
    answers: &[],
};
pub(crate) const WRITE_ROW: EndedBy<'static> = EndedBy::Table {
    name: "write",
    answers: WRITE_CODES,
};
pub(crate) const ACTION_ROW: EndedBy<'static> = EndedBy::Table {
    name: "action",
    answers: ACTION_CODES,
};

/// Whether an ending answers `code` under the dialect's policy. `None` is no policy: the code has no ending credits, never "every rule credits it"
/// (fail closed). One `match` over [`EndedBy`], no wildcard: a table answers the codes it names, an SC9012 every code that is not placeless, a
/// finding the rules the policy says claim the code (or, an obligation, decline it), the compiler's discard every code.
pub(crate) fn credits(policy: Option<&CodePolicy>, code: SiteCode, by: &EndedBy<'_>) -> bool {
    let Some(policy) = policy else {
        return false;
    };
    match by {
        EndedBy::Table { answers, .. } => answers.contains(&code),
        EndedBy::Census => !policy.placeless,
        EndedBy::Rule { name, uncertain } => {
            policy.claiming.iter().any(|claimed| claimed == name)
                || (*uncertain && policy.declining.iter().any(|declined| declined == name))
        }
        EndedBy::Discarded => true,
    }
}

/// One ending that stands at a span: its class (the order of precedence), what it is, and the row's text for the log.
#[derive(Clone, Copy)]
pub(crate) struct Ending<'p> {
    pub(crate) class: EndClass,
    pub(crate) by: EndedBy<'p>,
    pub(crate) detail: Detail<'p>,
}

/// How one code of a site ends: the strongest ending that answers it, and (for the log) the strongest that stands at the site and does not.
pub(crate) struct End<'p> {
    pub(crate) credited: Option<Ending<'p>>,
    pub(crate) uncredited: Option<Ending<'p>>,
}

/// The final program's rows, census obligations, defects and violations by file and byte span: every ending standing at each, with what it is.
pub(crate) struct Endings<'p> {
    at: HashMap<(&'p str, u64, u64), Vec<Ending<'p>>>,
    /// The store-write findings (SC2003 violations, direct-mutation defects) by
    /// file: a write through a chain is reported from the chain's root through
    /// the wrappers around it (ADR 0922), which starts inside the chain-end
    /// member's span and ends where it ends.
    mutations: HashMap<&'p str, Vec<(u64, u64, Ending<'p>)>>,
}

fn note<'p>(
    at: &mut HashMap<(&'p str, u64, u64), Vec<Ending<'p>>>,
    location: &'p Location,
    ending: Ending<'p>,
) {
    at.entry((
        location.path.as_ref(),
        location.start_byte,
        location.end_byte,
    ))
    .or_default()
    .push(ending);
}

impl<'p> Endings<'p> {
    pub(crate) fn of(program: &'p Program) -> Self {
        let mut at = HashMap::default();
        let rows = program.reads.len()
            + program.async_reads.len()
            + program.writes.len()
            + program.actions.len()
            + program.static_defects.len()
            + program.static_violations.len();
        at.reserve(rows);
        let row = |by: EndedBy<'p>, detail: Detail<'p>| Ending {
            class: EndClass::Row,
            by,
            detail,
        };
        for read in &program.reads {
            note(
                &mut at,
                &read.location,
                row(READ_ROW, Detail::Read(read.execution)),
            );
            // A row attributed to another place (a helper's call) answers for
            // the read it came from.
            if let Some(origin) = &read.origin {
                note(
                    &mut at,
                    origin,
                    row(READ_ROW, Detail::OriginOfRead(read.execution)),
                );
            }
        }
        for read in &program.async_reads {
            note(
                &mut at,
                &read.location,
                row(ASYNC_READ_ROW, Detail::AsyncRead(read.execution)),
            );
            for origin in [read.origin.as_ref(), read.pending_origin.as_ref()]
                .into_iter()
                .flatten()
            {
                note(
                    &mut at,
                    origin,
                    row(ASYNC_READ_ROW, Detail::OriginOfAsyncRead(read.execution)),
                );
            }
        }
        for write in &program.writes {
            note(
                &mut at,
                &write.location,
                row(WRITE_ROW, Detail::Write(write.execution)),
            );
        }
        for action in &program.actions {
            note(
                &mut at,
                &action.location,
                row(ACTION_ROW, Detail::Action(action.execution)),
            );
        }
        let mut mutations: HashMap<&'p str, Vec<(u64, u64, Ending<'p>)>> = HashMap::default();
        for defect in &program.static_defects {
            let family = defect.kind.family();
            let by = by_rule(family.rule_name(), defect.uncertain);
            let ending = Ending {
                class: if matches!(by, EndedBy::Census) {
                    EndClass::Census
                } else {
                    EndClass::Defect
                },
                by,
                detail: Detail::Defect(defect),
            };
            if family == crate::StaticDefectFamily::DirectMutation {
                mutations
                    .entry(defect.location.path.as_ref())
                    .or_default()
                    .push((defect.location.start_byte, defect.location.end_byte, ending));
            }
            note(&mut at, &defect.location, ending);
        }
        for violation in &program.static_violations {
            let ending = Ending {
                class: EndClass::Violation,
                by: by_rule(&violation.rule, violation.uncertain),
                detail: Detail::Violation(violation),
            };
            if violation.id == "SC2003" {
                mutations
                    .entry(violation.location.path.as_ref())
                    .or_default()
                    .push((
                        violation.location.start_byte,
                        violation.location.end_byte,
                        ending,
                    ));
            }
            note(&mut at, &violation.location, ending);
        }
        Self { at, mutations }
    }

    /// How `code` of the site ends: the strongest ending, over the spans a row for the site may sit at, that answers the code under the
    /// dialect's policy; else a store write's chain finding that does, else the compiler's discard. An ending that stands at the site and
    /// answers another code (a style finding's, SC1002's) is kept apart, for the log: it ends nothing.
    pub(crate) fn end(
        &self,
        dialect: &dyn Dialect,
        file: &FileFacts,
        site: &Site,
        code: SiteCode,
    ) -> End<'p> {
        let policy = dialect.code_policy(code.runtime_code());
        let path = file.path.as_str();
        let mut found = End {
            credited: None,
            uncredited: None,
        };
        let stronger = |slot: &Option<Ending<'p>>, ending: &Ending<'p>| {
            slot.as_ref().is_none_or(|held| ending.class < held.class)
        };
        for span in std::iter::once(site.at).chain(site.alternates.iter().copied()) {
            for ending in self
                .at
                .get(&(path, u64::from(span.start), u64::from(span.end)))
                .into_iter()
                .flatten()
            {
                let slot = if credits(policy.as_ref(), code, &ending.by) {
                    &mut found.credited
                } else {
                    &mut found.uncredited
                };
                // The strongest ending of the spans a site may be reported at; the first of equal classes stays.
                if stronger(slot, ending) {
                    *slot = Some(*ending);
                }
            }
        }
        if found.credited.is_some() {
            return found;
        }
        // A store write is reported from the chain's root through its wrappers:
        // a finding that ends where the chain-end member ends and starts inside it.
        if site.table == "store-write"
            && let Some((_, _, ending)) =
                self.mutations
                    .get(path)
                    .into_iter()
                    .flatten()
                    .find(|(start, end, ending)| {
                        *end == u64::from(site.at.end)
                            && *start >= u64::from(site.at.start)
                            && credits(policy.as_ref(), code, &ending.by)
                    })
        {
            found.credited = Some(*ending);
            return found;
        }
        if crate::execution_role::discarded_region_contains(file, site.at) {
            found.credited = Some(Ending {
                class: EndClass::Discarded,
                by: EndedBy::Discarded,
                detail: Detail::Nothing,
            });
        }
        found
    }
}

/// Every site of the universe, each once, with the file it is in: the sites of
/// the references of each source symbol outside dependency source, and the
/// calls of `onFlush`.
pub(crate) fn sites<'a>(ctx: &AnalysisContext<'a>) -> Vec<(&'a FileFacts, Site)> {
    let lookup = ctx.semantic_lookup;
    let mut symbols = ctx
        .accessors
        .keys()
        .chain(ctx.setters.keys())
        .chain(ctx.actions.keys())
        .chain(ctx.prop_sources.keys())
        .collect::<Vec<_>>();
    symbols.sort_unstable();
    symbols.dedup();
    let mut tables: HashMap<&str, FileTables> = HashMap::default();
    // The sites by file, span, table and way, with the alternates every
    // reference of a site added (a composite callee has one per operand).
    type SiteKey = (String, u64, u64, &'static str, &'static str);
    let mut found: Vec<(&'a FileFacts, Site)> = Vec::new();
    let mut index: HashMap<SiteKey, usize> = HashMap::default();
    for symbol in symbols {
        let kind = Kind::of(ctx, symbol);
        // Every reference TypeScript resolved to the symbol, and the identifiers
        // the binder resolves to the same declaration.
        let mut references = lookup
            .symbol_references(symbol.as_str())
            .iter()
            .filter_map(|reference| {
                let file = lookup.file_by_path(reference.path.as_ref())?;
                let start = u32::try_from(reference.start_byte).ok()?;
                let end = u32::try_from(reference.end_byte).ok()?;
                Some((file, Span::new(start, end)))
            })
            .collect::<Vec<_>>();
        let mut known = references
            .iter()
            .map(|(file, span)| (file.path.as_str(), *span))
            .collect::<HashSet<_>>();
        // The anchors the binder resolves from: the references, and the symbol's
        // own declarations (a symbol whose only use sits under a wrapper has no
        // reference of its own).
        let mut anchors = references.clone();
        if let Some(declared) = ctx.facts.typescript.symbol(symbol.as_str()) {
            for declaration in declared.declarations() {
                if let Some(file) = lookup.file_by_path(declaration.location.path.as_ref())
                    && let (Ok(start), Ok(end)) = (
                        u32::try_from(declaration.location.start_byte),
                        u32::try_from(declaration.location.end_byte),
                    )
                {
                    anchors.push((file, Span::new(start, end)));
                }
            }
        }
        let mut siblings = Vec::new();
        for (file, span) in &anchors {
            let declaration = file.ast.reference_declaration(*span).unwrap_or(*span);
            for sibling in file.ast.references_to(declaration) {
                if known.insert((file.path.as_str(), *sibling)) {
                    siblings.push((*file, *sibling));
                }
            }
        }
        references.extend(siblings);
        for (file, reference) in references {
            if lookup.in_dependency_source(file.path.as_str()) {
                continue;
            }
            let file_tables = tables
                .entry(file.path.as_str())
                .or_insert_with(|| FileTables::of(file));
            for site in sites_at(file, file_tables, kind, reference) {
                let key = (
                    file.path.as_str().to_owned(),
                    u64::from(site.at.start),
                    u64::from(site.at.end),
                    site.table,
                    site.via,
                );
                match index.get(&key) {
                    Some(&at) => {
                        for alternate in site.alternates {
                            if !found[at].1.alternates.contains(&alternate) {
                                found[at].1.alternates.push(alternate);
                            }
                        }
                        // The code lists of one table nest (a write site's second list holds the first), so the union is the longer one.
                        if site.codes.len() > found[at].1.codes.len() {
                            found[at].1.codes = site.codes;
                        }
                    }
                    None => {
                        index.insert(key, found.len());
                        found.push((file, site));
                    }
                }
            }
        }
    }
    on_flush_sites(ctx, &mut found);
    found
}

/// SC3002's sites: each call whose resolved primitive is `onFlush`, outside
/// dependency source, whatever its callee is spelled (found from the calls
/// like ADR 0886's owner sites).
fn on_flush_sites<'a>(ctx: &AnalysisContext<'a>, found: &mut Vec<(&'a FileFacts, Site)>) {
    let lookup = ctx.semantic_lookup;
    for file in crate::base::reads::file_slots(ctx.facts) {
        if lookup.in_dependency_source(file.path.as_str()) {
            continue;
        }
        for call in &file.ast.calls {
            if lookup.primitive_at_call(file, call.span) == Some(solid_dialect::Primitive::OnFlush)
            {
                found.push((
                    file,
                    Site {
                        table: "on-flush",
                        source: "primitive",
                        via: "call",
                        at: call.callee,
                        alternates: vec![call.span],
                        codes: ON_FLUSH_CODES,
                    },
                ));
            }
        }
    }
}

/// Writes one `site` line per code of the site: the site, the code, and how the final program ends it (`credited`: whether the ending answers the
/// code; a `none` line names, as `by` and `detail`, the strongest ending that stands at the site and does not).
pub(crate) fn log_site(file: &FileFacts, site: &Site, code: SiteCode, end: &End<'_>) {
    let (class, ending, credited) = match (&end.credited, &end.uncredited) {
        (Some(ending), _) => (ending.class.name(), Some(ending), true),
        (None, other) => ("none", other.as_ref(), false),
    };
    proof_log::site(&SiteRecord {
        table: site.table,
        source: site.source,
        via: site.via,
        code: code.name(),
        location: &site.at.location(file.path.shared()),
        alternates: &site.alternates,
        end: class,
        detail: &ending.map_or_else(String::new, |ending| ending.detail.text()),
        by: &ending.map_or_else(String::new, |ending| ending.by.text()),
        credited,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, source: &str) -> FileFacts {
        let ast = solid_facts::ast::extract(path, source).unwrap();
        FileFacts {
            generation: solid_facts::core::Generation::new(1).unwrap(),
            path: solid_facts::core::SourcePath::new(path).unwrap(),
            source_hash: ast.source.hash.clone(),
            source: source.into(),
            compiler: std::sync::Arc::new(solid_facts::compiler::ExecutionMap {
                compiler_facts_protocol: solid_facts::compiler::COMPILER_FACTS_PROTOCOL,
                source_hash: ast.source.hash.clone(),
                semantic_model: Default::default(),
                tracked_regions: vec![],
                untracked_regions: vec![],
                discarded_regions: vec![],
                ownership_regions: vec![],
                callback_roles: vec![],
                jsx_operations: vec![],
            }),
            ast: std::sync::Arc::new(ast),
        }
    }

    fn violation(rule: &str, id: &str, start: u64, end: u64, uncertain: bool) -> StaticViolation {
        StaticViolation {
            id: id.into(),
            rule: rule.into(),
            message: String::new(),
            hint: String::new(),
            location: Location {
                path: "/p/App.tsx".into(),
                start_byte: start,
                end_byte: end,
            },
            analysis_context: String::new(),
            fixes: vec![],
            uncertain,
        }
    }

    fn site(table: &'static str, at: Span, codes: &'static [SiteCode]) -> Site {
        Site {
            table,
            source: "accessor",
            via: "call",
            at,
            alternates: Vec::new(),
            codes,
        }
    }

    const SOURCE: &str = "export const a = 1; export const b = 2; export const c = 3;\n";

    #[test]
    fn the_classes_rank_a_row_before_a_census_obligation_before_a_defect_before_a_violation() {
        assert!(EndClass::Row < EndClass::Census);
        assert!(EndClass::Census < EndClass::Defect);
        assert!(EndClass::Defect < EndClass::Violation);
        assert!(EndClass::Violation < EndClass::Discarded);
        for class in [
            EndClass::Row,
            EndClass::Census,
            EndClass::Defect,
            EndClass::Violation,
            EndClass::Discarded,
        ] {
            assert!(!class.name().is_empty());
        }
    }

    /// One mapping from an ending to the codes it answers, no wildcard over [`EndedBy`]: a table answers the codes it names, an SC9012 every code that is
    /// not placeless, a finding the rules the policy says claim the code (an obligation also the ones it declines), the compiler's discard every code.
    #[test]
    fn an_ending_answers_the_codes_the_policy_names() {
        let solid = solid_dialect::Solid2;
        let strict = solid.code_policy(SiteCode::StrictRead.runtime_code());
        let write = solid.code_policy(SiteCode::ReactiveWrite.runtime_code());
        let rule = |name| EndedBy::Rule {
            name,
            uncertain: false,
        };
        // The read rule and the rules the policy lists for the code claim it; a style rule does not, and neither does another code's rule.
        assert!(credits(
            strict.as_ref(),
            SiteCode::StrictRead,
            &rule("strict-read-untracked")
        ));
        assert!(credits(
            strict.as_ref(),
            SiteCode::StrictRead,
            &rule("no-destructure")
        ));
        assert!(!credits(
            strict.as_ref(),
            SiteCode::StrictRead,
            &rule("prefer-show")
        ));
        assert!(!credits(
            strict.as_ref(),
            SiteCode::StrictRead,
            &rule("no-write-in-pure-computation")
        ));
        assert!(credits(
            write.as_ref(),
            SiteCode::ReactiveWrite,
            &rule("no-write-in-pure-computation")
        ));
        assert!(!credits(
            write.as_ref(),
            SiteCode::ReactiveWrite,
            &rule("strict-read-untracked")
        ));
        // A table answers the codes it names.
        assert!(credits(strict.as_ref(), SiteCode::StrictRead, &READ_ROW));
        assert!(!credits(write.as_ref(), SiteCode::ReactiveWrite, &READ_ROW));
        assert!(credits(write.as_ref(), SiteCode::ReactiveWrite, &WRITE_ROW));
        assert!(!credits(
            strict.as_ref(),
            SiteCode::StrictRead,
            &ASYNC_READ_ROW
        ));
        // An SC9012 covers a code that names a line; the discard covers every code.
        assert!(credits(
            strict.as_ref(),
            SiteCode::StrictRead,
            &EndedBy::Census
        ));
        assert!(credits(
            write.as_ref(),
            SiteCode::ReactiveWrite,
            &EndedBy::Discarded
        ));
        // No policy is no ending: the code is held, whatever stands.
        for by in [
            READ_ROW,
            EndedBy::Census,
            EndedBy::Discarded,
            rule("strict-read-untracked"),
        ] {
            assert!(!credits(None, SiteCode::StrictRead, &by));
        }
    }

    #[test]
    fn a_declining_rule_ends_only_the_code_it_declines_and_only_as_an_obligation() {
        let policy = CodePolicy {
            claiming: &["claims"],
            declining: &["declines"],
            placeless: false,
        };
        let by = |name, uncertain| EndedBy::Rule { name, uncertain };
        assert!(credits(
            Some(&policy),
            SiteCode::StrictRead,
            &by("claims", false)
        ));
        assert!(credits(
            Some(&policy),
            SiteCode::StrictRead,
            &by("declines", true)
        ));
        // A violation of a declining rule is a wrong rule: it ends nothing.
        assert!(!credits(
            Some(&policy),
            SiteCode::StrictRead,
            &by("declines", false)
        ));
        assert!(!credits(
            Some(&policy),
            SiteCode::StrictRead,
            &by("other", true)
        ));
        // A placeless code names no authored line: an SC9012 at a line says nothing of it.
        let placeless = CodePolicy {
            placeless: true,
            ..policy
        };
        assert!(!credits(
            Some(&placeless),
            SiteCode::StrictRead,
            &EndedBy::Census
        ));
    }

    /// The point of the change: a finding of another rule at the site's span (a style rule's, SC1002's) answered its own code, and it used to end the site.
    #[test]
    fn a_finding_of_another_rule_at_a_read_site_ends_none_of_its_codes() {
        let source = file("/p/App.tsx", SOURCE);
        let at = Span::new(4, 9);
        let read = site("read", at, READ_CODES);
        let solid = solid_dialect::Solid2;
        let mut program = Program::default();
        program
            .static_violations
            .push(violation("prefer-show", "SC8001", 4, 9, false));
        let endings = Endings::of(&program);
        let end = endings.end(&solid, &source, &read, SiteCode::StrictRead);
        assert!(end.credited.is_none());
        let stands = end
            .uncredited
            .expect("the style finding stands at the site");
        assert_eq!(stands.by.text(), "rule:prefer-show");
        // The read rule's own finding at the same span ends it.
        program
            .static_violations
            .push(violation("strict-read-untracked", "SC1001", 4, 9, false));
        let endings = Endings::of(&program);
        let end = endings.end(&solid, &source, &read, SiteCode::StrictRead);
        let credited = end.credited.expect("the read rule's finding ends the read");
        assert_eq!(credited.by.text(), "rule:strict-read-untracked");
    }

    #[test]
    fn a_row_of_another_table_at_the_span_does_not_end_a_site() {
        let source = file("/p/App.tsx", SOURCE);
        let at = Span::new(4, 9);
        let solid = solid_dialect::Solid2;
        let mut program = Program::default();
        // An uncertifiable SC1001 stands where a setter is called: the code of the write is not the read's.
        program
            .static_violations
            .push(violation("strict-read-untracked", "SC1001", 4, 9, true));
        let endings = Endings::of(&program);
        let write = site("write", at, WRITE_CODES);
        assert!(
            endings
                .end(&solid, &source, &write, SiteCode::ReactiveWrite)
                .credited
                .is_none()
        );
        let action = site("action", at, ACTION_CODES);
        assert!(
            endings
                .end(&solid, &source, &action, SiteCode::ActionCall)
                .credited
                .is_none()
        );
    }

    #[test]
    fn a_store_write_through_a_wrapper_is_ended_by_its_rules_finding_and_only_by_it() {
        let source = file("/p/App.tsx", SOURCE);
        let at = Span::new(10, 20);
        let solid = solid_dialect::Solid2;
        let write = site("store-write", at, STORE_WRITE_CODES);
        // The write is reported from the chain's root, inside the member's span and ending where it ends (ADR 0922).
        let mut program = Program::default();
        program
            .static_violations
            .push(violation("prefer-show", "SC2003", 12, 20, false));
        let end = Endings::of(&program).end(&solid, &source, &write, SiteCode::StoreWrite);
        assert!(end.credited.is_none());
        let mut program = Program::default();
        program.static_violations.push(violation(
            "no-store-mutation-outside-setter",
            "SC2003",
            12,
            20,
            false,
        ));
        let end = Endings::of(&program).end(&solid, &source, &write, SiteCode::StoreWrite);
        assert!(end.credited.is_some());
    }

    #[test]
    fn every_code_has_a_runtime_code_a_home_rule_and_a_blocker() {
        let mut seen = std::collections::BTreeSet::new();
        for code in SiteCode::ALL {
            assert!(seen.insert(code.name()), "{code:?} repeats a runtime code");
            let home = code.home();
            assert!(home.id.starts_with("SC") && !home.rule.is_empty() && !home.noun.is_empty());
            assert!(home.blocker.ends_with("-unaccounted"), "{code:?}");
            // The dialect has a policy for the code, and its home rule claims it.
            let policy = solid_dialect::Solid2
                .code_policy(code.runtime_code())
                .unwrap_or_else(|| panic!("{code:?} has no policy"));
            assert!(policy.claiming.contains(&home.rule), "{code:?}");
        }
    }

    #[test]
    fn the_codes_of_a_write_site_nest_so_that_a_merged_site_keeps_the_longer_list() {
        assert!(OPTIMISTIC_WRITE_CODES.starts_with(WRITE_CODES));
        assert!(OPTIMISTIC_WRITE_CODES.len() > WRITE_CODES.len());
    }
}
