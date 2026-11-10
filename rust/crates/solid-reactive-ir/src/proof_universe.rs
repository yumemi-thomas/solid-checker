//! ADR 0933: the proof log's site universe. Every place a reactive value is
//! read, written or acted on is a site, found from facts alone (the references
//! TypeScript resolved to each source symbol, and the syntax around them),
//! never from a producer's row, role or walk: a site no producer reached is a
//! site here all the same. Each site is written with how it ends in the final
//! program (a row, a census obligation, a compiler discard, or nothing), and
//! `scripts/proof-gate.mjs` fails a site that ends in nothing unless a proof
//! was recorded for it.
//!
//! The sites, per source symbol and each reference outside dependency source:
//!
//! - the callee of a direct call: of an accessor (a read), a setter (a write),
//!   an action;
//! - the first member Get on a store or props root, unless the chain it starts
//!   is a write path (a read);
//! - a member chain on a root that ends in an assignment, update, destructuring
//!   or iteration target (a store write), in a `delete` operand, or in the
//!   receiver of an in-place array method (`push`, `splice`, ...).
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
use crate::{HashMap, HashSet, Program, ReactiveSourceKind};

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

/// The syntax of one file the universe asks about, indexed once.
struct FileTables {
    /// Indexes into `members` by the peeled span of the member's object.
    members_by_object: HashMap<Span, Vec<usize>>,
    /// Peeled assignment, update, destructuring and iteration targets.
    written: HashSet<Span>,
    /// Peeled operands of `delete`.
    deleted: HashSet<Span>,
    /// Indexes into `calls` by each reference that is an operand of a call's
    /// composite callee (`(slot = r)()`, `(flag ? f : g)()`).
    composite_calls: HashMap<Span, Vec<usize>>,
    /// The identifiers the binder resolves to each declaration: a reference
    /// under a TypeScript wrapper (`(setStore as typeof setStore)`) carries no
    /// entity of its own, but its declaration is the symbol's.
    references_by_declaration: HashMap<Span, Vec<Span>>,
}

impl FileTables {
    fn of(file: &FileFacts) -> Self {
        let ast = &file.ast;
        let mut members_by_object: HashMap<Span, Vec<usize>> = HashMap::default();
        for (index, member) in ast.members.iter().enumerate() {
            members_by_object
                .entry(ast.peel_ts_sugar_span(member.object))
                .or_default()
                .push(index);
        }
        let mut composite_calls: HashMap<Span, Vec<usize>> = HashMap::default();
        for (index, call) in ast.calls.iter().enumerate() {
            if call.construct {
                continue;
            }
            for operand in ast.composite_callee_references(call.callee) {
                composite_calls.entry(operand).or_default().push(index);
            }
        }
        let mut references_by_declaration: HashMap<Span, Vec<Span>> = HashMap::default();
        for identifier in &ast.identifiers {
            if identifier.role == solid_facts::ast::IdentifierRole::Reference
                && let Some(declaration) = ast.reference_declaration(identifier.span)
            {
                references_by_declaration
                    .entry(declaration)
                    .or_default()
                    .push(identifier.span);
            }
        }
        Self {
            members_by_object,
            composite_calls,
            references_by_declaration,
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
        let next = tables
            .members_by_object
            .get(&peeled)
            .and_then(|indexes| indexes.first())
            .map(|index| &ast.members[*index]);
        match next {
            Some(next) => link = next,
            None => return ChainEnd::Read(links),
        }
    }
    ChainEnd::Read(links)
}

/// One site: where it is, what kind it is, and the other spans a row that
/// answers for it may sit at.
struct Site {
    table: &'static str,
    /// What the source is: `accessor`, `store`, `props`, `setter` or `action`.
    source: &'static str,
    via: &'static str,
    at: Span,
    alternates: Vec<Span>,
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
    for index in tables
        .members_by_object
        .get(&reference)
        .into_iter()
        .flatten()
    {
        let first = &ast.members[*index];
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

/// How the final program ends the sites at a span: each row, census
/// obligation, defect and violation, by file and byte span.
struct Endings<'p> {
    at: EndingsAt<'p>,
    /// The store-write findings (SC2003 violations, direct-mutation defects) by
    /// file: a write through a chain is reported from the chain's root through
    /// the wrappers around it (ADR 0922), which starts inside the chain-end
    /// member's span and ends where it ends.
    mutations: HashMap<&'p str, Vec<MutationEnding>>,
}

/// The endings by file and byte span: each a class and its detail.
type EndingsAt<'p> = HashMap<(&'p str, u64, u64), Vec<(&'static str, String)>>;

/// A store-write finding's byte span, class and detail.
type MutationEnding = (u64, u64, &'static str, String);

fn note<'p>(at: &mut EndingsAt<'p>, location: &'p Location, class: &'static str, detail: String) {
    at.entry((
        location.path.as_ref(),
        location.start_byte,
        location.end_byte,
    ))
    .or_default()
    .push((class, detail));
}

impl<'p> Endings<'p> {
    fn of(program: &'p Program) -> Self {
        let mut at = HashMap::default();
        for read in &program.reads {
            note(
                &mut at,
                &read.location,
                "row",
                format!("read:{:?}", read.execution),
            );
            // A row attributed to another place (a helper's call) answers for
            // the read it came from.
            if let Some(origin) = &read.origin {
                note(
                    &mut at,
                    origin,
                    "row",
                    format!("origin-of-read:{:?}", read.execution),
                );
            }
        }
        for read in &program.async_reads {
            note(
                &mut at,
                &read.location,
                "row",
                format!("async-read:{:?}", read.execution),
            );
            for origin in [read.origin.as_ref(), read.pending_origin.as_ref()]
                .into_iter()
                .flatten()
            {
                note(
                    &mut at,
                    origin,
                    "row",
                    format!("origin-of-async-read:{:?}", read.execution),
                );
            }
        }
        for write in &program.writes {
            note(
                &mut at,
                &write.location,
                "row",
                format!("write:{:?}", write.execution),
            );
        }
        for action in &program.actions {
            note(
                &mut at,
                &action.location,
                "row",
                format!("action:{:?}", action.execution),
            );
        }
        let mut mutations: HashMap<&'p str, Vec<MutationEnding>> = HashMap::default();
        for defect in &program.static_defects {
            let family = defect.kind.dedup_identity();
            let class = if family == "reactive-dispatch-unresolved" {
                "census"
            } else {
                "defect"
            };
            if family == "no-store-mutation-outside-setter" {
                mutations
                    .entry(defect.location.path.as_ref())
                    .or_default()
                    .push((
                        defect.location.start_byte,
                        defect.location.end_byte,
                        class,
                        format!("{family}:{}", defect.analysis_context),
                    ));
            }
            note(
                &mut at,
                &defect.location,
                class,
                format!("{family}:{}", defect.analysis_context),
            );
        }
        for violation in &program.static_violations {
            if violation.id == "SC2003" {
                mutations
                    .entry(violation.location.path.as_ref())
                    .or_default()
                    .push((
                        violation.location.start_byte,
                        violation.location.end_byte,
                        "violation",
                        violation.id.clone(),
                    ));
            }
            note(
                &mut at,
                &violation.location,
                "violation",
                violation.id.clone(),
            );
        }
        Self { at, mutations }
    }

    /// The site's ending: its row, else a census obligation, a defect or a
    /// violation at it, else the compiler's discard, else nothing.
    fn end(&self, file: &FileFacts, site: &Site) -> (&'static str, String) {
        let path = file.path.as_str();
        let found = std::iter::once(site.at)
            .chain(site.alternates.iter().copied())
            .flat_map(|span| {
                self.at
                    .get(&(path, u64::from(span.start), u64::from(span.end)))
                    .into_iter()
                    .flatten()
            })
            .collect::<Vec<_>>();
        for class in ["row", "census", "defect", "violation"] {
            if let Some((_, detail)) = found.iter().find(|(candidate, _)| *candidate == class) {
                return (class, detail.clone());
            }
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
            return (*class, detail.clone());
        }
        if crate::execution_role::discarded_region_contains(file, site.at) {
            return ("discarded", String::new());
        }
        ("none", String::new())
    }
}

/// Writes one `site` line for each site of the universe, with its ending in
/// `program`. Runs once per build, after the program is final, only with the
/// proof log on.
pub(crate) fn record(ctx: &AnalysisContext<'_>, program: &Program) {
    if !proof_log::enabled() {
        return;
    }
    let lookup = ctx.semantic_lookup;
    let endings = Endings::of(program);
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
    let mut found: Vec<(&FileFacts, Site)> = Vec::new();
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
        let mut siblings = Vec::new();
        for (file, span) in &references {
            let file_tables = tables
                .entry(file.path.as_str())
                .or_insert_with(|| FileTables::of(file));
            let declaration = file.ast.reference_declaration(*span).unwrap_or(*span);
            for sibling in file_tables
                .references_by_declaration
                .get(&declaration)
                .into_iter()
                .flatten()
            {
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
    for (file, site) in found {
        let (ending, detail) = endings.end(file, &site);
        proof_log::site(&SiteRecord {
            table: site.table,
            source: site.source,
            via: site.via,
            location: &site.at.location(file.path.shared()),
            alternates: &site.alternates,
            end: ending,
            detail: &detail,
        });
    }
}
