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
//! A reference used as a value (passed, returned, stored) is a use the
//! value-use census (ADR 0692) follows or answers with an SC9012; it is not a
//! site here. What this does not list: a props binding destructured in a
//! parameter list, unless TypeScript gives the binding its own symbol among
//! the props sources; `Object.assign(store.a, x)` and the other standard
//! library writes.

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
        }
    }
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
        for (applies, table, source) in [
            (kind.setter, "write", "setter"),
            (kind.action, "action", "action"),
            (kind.accessor, "read", "accessor"),
        ] {
            if applies {
                sites.push(Site {
                    table,
                    source,
                    via: "call",
                    at: call.span,
                    alternates: vec![call.callee, reference],
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
                }
            }
            ChainEnd::Assignment(at) => Site {
                table: "store-write",
                source,
                via: "assignment",
                at,
                alternates: Vec::new(),
            },
            ChainEnd::Delete(at) => Site {
                table: "store-write",
                source,
                via: "delete",
                at,
                alternates: Vec::new(),
            },
            ChainEnd::Mutator { call, callee } => Site {
                table: "store-write",
                source,
                via: "mutator",
                at: call,
                alternates: vec![callee],
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

/// The final program's rows, census obligations, defects and violations by file
/// and byte span: the best class standing at each (the first of equal classes).
pub(crate) struct Endings<'p> {
    at: HashMap<(&'p str, u64, u64), (EndClass, Detail<'p>)>,
    /// The store-write findings (SC2003 violations, direct-mutation defects) by
    /// file: a write through a chain is reported from the chain's root through
    /// the wrappers around it (ADR 0922), which starts inside the chain-end
    /// member's span and ends where it ends.
    mutations: HashMap<&'p str, Vec<(u64, u64, EndClass, Detail<'p>)>>,
}

fn note<'p>(
    at: &mut HashMap<(&'p str, u64, u64), (EndClass, Detail<'p>)>,
    location: &'p Location,
    class: EndClass,
    detail: Detail<'p>,
) {
    let key = (
        location.path.as_ref(),
        location.start_byte,
        location.end_byte,
    );
    match at.entry(key) {
        std::collections::hash_map::Entry::Vacant(slot) => {
            slot.insert((class, detail));
        }
        std::collections::hash_map::Entry::Occupied(mut slot) => {
            if class < slot.get().0 {
                slot.insert((class, detail));
            }
        }
    }
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
        for read in &program.reads {
            note(
                &mut at,
                &read.location,
                EndClass::Row,
                Detail::Read(read.execution),
            );
            // A row attributed to another place (a helper's call) answers for
            // the read it came from.
            if let Some(origin) = &read.origin {
                note(
                    &mut at,
                    origin,
                    EndClass::Row,
                    Detail::OriginOfRead(read.execution),
                );
            }
        }
        for read in &program.async_reads {
            note(
                &mut at,
                &read.location,
                EndClass::Row,
                Detail::AsyncRead(read.execution),
            );
            for origin in [read.origin.as_ref(), read.pending_origin.as_ref()]
                .into_iter()
                .flatten()
            {
                note(
                    &mut at,
                    origin,
                    EndClass::Row,
                    Detail::OriginOfAsyncRead(read.execution),
                );
            }
        }
        for write in &program.writes {
            note(
                &mut at,
                &write.location,
                EndClass::Row,
                Detail::Write(write.execution),
            );
        }
        for action in &program.actions {
            note(
                &mut at,
                &action.location,
                EndClass::Row,
                Detail::Action(action.execution),
            );
        }
        let mut mutations: HashMap<&'p str, Vec<(u64, u64, EndClass, Detail<'p>)>> =
            HashMap::default();
        for defect in &program.static_defects {
            let family = defect.kind.dedup_identity();
            let class = if family == "reactive-dispatch-unresolved" {
                EndClass::Census
            } else {
                EndClass::Defect
            };
            if family == "no-store-mutation-outside-setter" {
                mutations
                    .entry(defect.location.path.as_ref())
                    .or_default()
                    .push((
                        defect.location.start_byte,
                        defect.location.end_byte,
                        class,
                        Detail::Defect(defect),
                    ));
            }
            note(&mut at, &defect.location, class, Detail::Defect(defect));
        }
        for violation in &program.static_violations {
            if violation.id == "SC2003" {
                mutations
                    .entry(violation.location.path.as_ref())
                    .or_default()
                    .push((
                        violation.location.start_byte,
                        violation.location.end_byte,
                        EndClass::Violation,
                        Detail::Violation(violation),
                    ));
            }
            note(
                &mut at,
                &violation.location,
                EndClass::Violation,
                Detail::Violation(violation),
            );
        }
        Self { at, mutations }
    }

    /// The site's ending: its row, else a census obligation, a defect or a
    /// violation at it, else the compiler's discard, else nothing.
    pub(crate) fn end(&self, file: &FileFacts, site: &Site) -> Option<(EndClass, Detail<'p>)> {
        let path = file.path.as_str();
        let mut best: Option<(EndClass, Detail<'p>)> = None;
        for span in std::iter::once(site.at).chain(site.alternates.iter().copied()) {
            if let Some((class, detail)) =
                self.at
                    .get(&(path, u64::from(span.start), u64::from(span.end)))
                && best.is_none_or(|(held, _)| *class < held)
            {
                best = Some((*class, *detail));
            }
        }
        if best.is_some() {
            return best;
        }
        // A store write is reported from the chain's root through its wrappers:
        // a finding that ends where the chain-end member ends and starts inside it.
        if site.table == "store-write"
            && let Some((_, _, class, detail)) = self
                .mutations
                .get(path)
                .into_iter()
                .flatten()
                .find(|(start, end, _, _)| {
                    *end == u64::from(site.at.end) && *start >= u64::from(site.at.start)
                })
        {
            return Some((*class, *detail));
        }
        if crate::execution_role::discarded_region_contains(file, site.at) {
            return Some((EndClass::Discarded, Detail::Nothing));
        }
        None
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
                    },
                ));
            }
        }
    }
}

/// Writes one `site` line: the site and how the final program ends it.
pub(crate) fn log_site(file: &FileFacts, site: &Site, end: &str, detail: &str) {
    proof_log::site(&SiteRecord {
        table: site.table,
        source: site.source,
        via: site.via,
        location: &site.at.location(file.path.shared()),
        alternates: &site.alternates,
        end,
        detail,
    });
}
