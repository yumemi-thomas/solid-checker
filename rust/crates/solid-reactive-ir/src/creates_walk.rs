//! The generator's own `creates` walk: which call sites forbid *proposing*
//! that an export publishes no `create` operation.
//!
//! This is a **proposal** input, never a proof. `creates` is the domain of
//! published `create` operations — an export registering a version-1 resource
//! into a runtime outside the invocation
//! (`docs/package-contract-v2/semantic-model.md` § creates) — and the only
//! thing that may *close* it is the certifier's implementation census against
//! authenticated bytes (`docs/adr/0008-implementation-census-for-creates.md`).
//! What this walk decides is the weaker, earlier question: has the generator's
//! own analysis seen anything inside this export's implementation that a
//! `creates: []` proposal would contradict? If it has, the domain is left
//! `Unknown` and nothing is proposed; if it has not, a candidate is proposed
//! and the census has to prove it.
//!
//! Every disposition therefore fails **closed**, and silence is always "do not
//! propose":
//!
//! * A callee this build cannot resolve to a symbol refuses. "Unresolved" is
//!   never evidence of harmlessness.
//! * A callee that resolves to a **canonical dialect primitive** refuses unless
//!   some dialect's audited negative authority carries a `creates` denial for
//!   that spelling ([`solid_dialect::some_audit_denies_primitive`]). This is
//!   deliberately a name-level read, which is why it can only gate a proposal:
//!   the census re-asks the identity-bound form against the archive the callee's
//!   declaration actually resolves into.
//! * A callee bound to an **accepted dependency contract** refuses unless that
//!   contract closes `creates` empty. An open domain, or one carrying a `create`
//!   item, is exactly the counterexample a proposal may not ignore.
//!
//! Anything else — a default-library member, a caller-supplied parameter — is
//! not a counterexample this walk can name, and the census is what decides it.
//! The walk is lexical over the export's own implementation span, so a call
//! inside a nested closure counts too: it may run, and a closed domain asserts
//! a zero upper bound.
//!
//! A **module-local helper** is followed, to a fixpoint: a call whose callee
//! resolves to a project function whose own span contains a refusing call is
//! itself a refusing call. Without that step the gate would be purely lexical —
//! `export function f() { helper() }` beside `function helper() { createSignal() }`
//! would propose for `f` because no refusing call sits inside `f`'s bytes — and
//! the candidate would only be withheld or refused later, by name, at the
//! census. Following the edge here is still only a proposal input: the census
//! resolves the same callee against authenticated bytes and decides it again.
//! The edge followed is the IR's own resolved call edge
//! ([`crate::indexes::SemanticLookup::function_for_symbol`] on the callee
//! symbol), never the callee's name.
//!
//! # Every decline names itself
//!
//! A refusing call used to be a bare span, so the walk's answer for an export
//! was one bit and the *reason* was unrecoverable. That made "which dialect
//! audit would return the most candidates" a guess. Each refusing call now
//! carries a [`CreatesDeclineKind`], and [`CreatesProposalWalk::declines_for`]
//! answers, for one export's span, the whole set of blockers reachable from it
//! — lexically, and transitively through the local call edges the fixpoint
//! followed. The records are **measurement**, not evidence: they are never
//! encoded into a contract document, they certify nothing, and a kind is only
//! ever the reason *this build's own resolution* declined. Where a callee is
//! unresolved that is itself the recorded kind, with the call's location; a
//! package identity comes from the compiler's resolved declaration or from the
//! import statement the callee symbol is bound by, and never from the callee's
//! spelling.
//!
//! # And an unresolved callee names its shape
//!
//! `unresolved-callee` was half of every measured corpus decline while saying
//! only "something here did not resolve" — which cannot tell a resolver gap
//! from a callee no analysis of this module could ever decide. So the kind now
//! carries an [`UnresolvedCalleeShape`], classified from the syntax and binding
//! facts already in hand: Oxc's member, computed-member, identifier, parameter
//! and binding-initializer tables, and the IR's own entity lookups. **No producer or Type Facts demand is added**,
//! and no shape is guessed — a callee whose syntax no fact table names is
//! recorded as `other` with that syntactic kind rather than folded into a
//! neighbour. The kind's wire name is still `unresolved-callee`, so the shape
//! is strictly additive to every existing count.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use solid_dialect::CallClaimDomain;
use solid_facts::core::Span;

use crate::PrimitiveName;
use crate::pipeline::AnalysisContext;

/// How many *regions* [`CreatesProposalWalk::declines_for`] visits while
/// following the fixpoint's own call edges: the export's own span, then up to
/// seven local-helper hops beyond it.
///
/// The fixpoint itself is unbounded — it must be, or a refusing call eight hops
/// down would silently stop refusing — so this bounds only the *reporting*
/// walk. A blocker past this depth is simply not named; the export still
/// declines, on the `refusing-callee-fixpoint` record at the last hop that is.
const MAX_DECLINE_REPORT_DEPTH: usize = 8;

/// One resolved local call edge, as [`CreatesProposalWalk`] retains it:
/// `(call span, callee file, callee span)` under the caller's own file.
type LocalCallEdge = ((u32, u32), String, (u32, u32));

/// How long a spelling a shape record carries, in bytes of the source text.
///
/// A property name or an identifier is far shorter than this; the cap exists so
/// a pathological source cannot put a whole expression on the emitter's
/// tab-separated line.
const MAX_SPELLING_BYTES: usize = 64;

/// How many binding-alias hops [`roots_in_caller_parameter`] follows before it
/// gives up. `const p = props; const q = p; q.x()` is the shape it exists for,
/// and a chain longer than this is simply not classified `parameter-rooted`.
const MAX_PARAMETER_ALIAS_HOPS: usize = 4;

/// The *shape* of a callee this build resolved to no symbol.
///
/// [`CreatesDeclineKind::UnresolvedCallee`] used to be a unit variant, so half
/// of every measured corpus decline said only "something here did not resolve".
/// That is not enough to tell a resolver gap from a genuinely undecidable
/// callee, so each unresolved callee is now classified from the syntax and
/// binding facts already in hand — Oxc's own member/computed/identifier tables
/// and the IR's entity lookups. **No producer or Type Facts demand is added**:
/// a shape is decided from facts the build already computed, or it is `Other`
/// carrying the callee expression's syntactic kind.
///
/// Every variant names *what was observed*, never what the callee does. The
/// decision order is fixed and total, and it matters because a call can satisfy
/// two predicates at once (`props[key]()` is both computed and parameter-rooted);
/// [`unresolved_callee_shape`] documents and implements exactly this order:
///
/// 1. [`Self::ComputedMember`]
/// 2. [`Self::ParameterRooted`]
/// 3. [`Self::MemberPropertyUnresolved`] / [`Self::MemberReceiverUnresolved`]
/// 4. [`Self::UndeclaredIdentifier`]
/// 5. [`Self::ExpressionCallee`]
/// 6. [`Self::Other`]
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum UnresolvedCalleeShape {
    /// A member call whose property is written as a computed access.
    ///
    /// **Decided by** the peeled callee span appearing in
    /// `AstFacts::computed_members`. There is no static property spelling at
    /// all, which is the whole content of the shape, so `receiver` carries the
    /// member's *object* text where that object is a plain identifier
    /// (`handlers[i]()` answers `handlers`) and is empty otherwise.
    ComputedMember { receiver: String },
    /// A member call whose receiver chain roots at a value this module's caller
    /// supplies.
    ///
    /// **Decided by** [`crate::indexes::SemanticLookup::member_callee_receiver`]
    /// answering a root symbol for the callee, and that symbol — or a symbol it
    /// reaches through at most [`MAX_PARAMETER_ALIAS_HOPS`] binding-initializer
    /// aliases — being a parameter name of a function whose *body contains this
    /// call*. `property` is the leaf property's own source text.
    ///
    /// This is the same notion the implementation census calls
    /// `parameter-rooted`, and the spelling is deliberately shared: the callee
    /// is not decidable from this module's bytes at all, because it is whatever
    /// the caller passed.
    ParameterRooted { property: String },
    /// A non-computed member call whose **receiver resolves** to a symbol and
    /// whose property does not.
    ///
    /// **Decided by** the peeled callee being a member fact,
    /// [`crate::indexes::SemanticLookup::entity_symbol`] answering for the
    /// member's (peeled) object span, and `callee_symbol` having answered
    /// nothing — so neither the resolved-call declaration nor an entity at the
    /// property span exists. `property` is the property's own source text: the
    /// name whose declaration is missing.
    MemberPropertyUnresolved { property: String },
    /// A non-computed member call whose **receiver itself** resolves to no
    /// symbol.
    ///
    /// **Decided by** the same member fact with no entity symbol at the
    /// object's peeled span. That covers both an unresolved identifier receiver
    /// and a receiver that is an expression no entity is recorded at
    /// (`factory().method()`). `property` is still the property's source text —
    /// the receiver has no name this record could carry, and what was called is
    /// worth more than nothing.
    MemberReceiverUnresolved { property: String },
    /// A bare identifier callee this build resolved to no symbol — in practice
    /// a global.
    ///
    /// **Decided by** the peeled callee being an identifier fact of this file
    /// with no entity symbol at its span.
    ///
    /// There is deliberately no sibling shape for "a call through an import the
    /// project did not accept". It was implemented and measured, and it cannot
    /// fire: an import of an unresolvable bare specifier, a deep subpath, or a
    /// missing default still gives its local binding an alias symbol, so such a
    /// callee resolves and never reaches this branch at all. A namespace
    /// import's member call reaches [`Self::MemberPropertyUnresolved`] instead,
    /// with the receiver resolved. An unaccepted dependency surfaces as a
    /// closure hazard at certification, which is a different decision from this
    /// walk's.
    UndeclaredIdentifier { identifier: String },
    /// A callee that is itself a call or a function expression: a higher-order
    /// result (`factory()()`) or an immediately-invoked function.
    ///
    /// **Decided by** the peeled callee span being exactly a `CallFact::span`
    /// or a `FunctionFact::span` of this file. `syntax` is which of the two.
    ExpressionCallee { syntax: &'static str },
    /// Everything else, carrying the callee expression's syntactic kind so no
    /// shape is silently lumped.
    ///
    /// **Decided by** which of this file's syntax tables holds the peeled
    /// callee span — `await-expression`, `conditional-expression`,
    /// `logical-expression`, `jsx-element` — and `unknown-expression` where
    /// none does. The name is drawn from a fixed vocabulary, never from source
    /// text.
    Other { syntax: &'static str },
}

impl UnresolvedCalleeShape {
    /// The stable wire name of this shape, as it reaches the proposal refusal
    /// audit and the ranking script.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::ComputedMember { .. } => "computed-member",
            Self::ParameterRooted { .. } => "parameter-rooted",
            Self::MemberPropertyUnresolved { .. } => "member-property-unresolved",
            Self::MemberReceiverUnresolved { .. } => "member-receiver-unresolved",
            Self::UndeclaredIdentifier { .. } => "undeclared-identifier",
            Self::ExpressionCallee { .. } => "expression-callee",
            Self::Other { .. } => "other",
        }
    }

    /// The one concrete string this shape observed, or `""` where it observed
    /// none.
    ///
    /// Shape-specific by construction, and each variant's doc comment says
    /// which string it is: a property name, a receiver identifier, a module
    /// specifier, an identifier, or — for [`Self::ExpressionCallee`] and
    /// [`Self::Other`] — the syntactic kind name, because there is no name in
    /// the source to carry and the syntax is the only thing observed.
    #[must_use]
    pub fn spelling(&self) -> &str {
        match self {
            Self::ComputedMember { receiver } => receiver,
            Self::ParameterRooted { property }
            | Self::MemberPropertyUnresolved { property }
            | Self::MemberReceiverUnresolved { property } => property,
            Self::UndeclaredIdentifier { identifier } => identifier,
            Self::ExpressionCallee { syntax } | Self::Other { syntax } => syntax,
        }
    }
}

/// Why one call site forbids a `creates: []` proposal.
///
/// Exactly the dispositions the walk itself distinguishes, and nothing else. A
/// kind is never a claim about what the callee *does*: `DialectSilent` says no
/// dialect's audit denies the domain for that spelling, which is silence, and
/// `UnresolvedCallee` says this build resolved no symbol, which is ignorance.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CreatesDeclineKind {
    /// A canonical dialect primitive for which no dialect's audited negative
    /// authority carries a `creates` denial row.
    ///
    /// **The measured blocker.** `export` is the dialect's own canonical
    /// spelling for the primitive — the exact key
    /// [`solid_dialect::some_audit_denies_primitive`] was asked about, so an
    /// audit row added for it is what would clear this record. `package` is a
    /// resolved module specifier reduced to its package: the compiler's
    /// `ResolvedDeclaration::origin_module` for the callee, else the specifier
    /// of the import statement the callee symbol is bound by, else empty. It is
    /// never derived from the spelling.
    DialectSilent { package: String, export: String },
    /// A callee bound to an accepted dependency contract whose `creates` is
    /// open, or closed over a `create` item. Both are counterexamples a
    /// proposal may not ignore. The identity is the contract binding's own
    /// package and imported export name.
    CreatePublishingCallee { package: String, export: String },
    /// A callee this build resolved to no symbol at all.
    ///
    /// There is still no callee *identity* to name — that is precisely the
    /// refusal — but the callee expression's **shape** is observable from the
    /// syntax and binding facts already in hand, and it is what distinguishes a
    /// resolver gap from a genuinely undecidable call. See
    /// [`UnresolvedCalleeShape`].
    UnresolvedCallee { shape: UnresolvedCalleeShape },
    /// The propagated case: the callee resolves to a project function whose own
    /// span contains a refusing call, so this call refuses too.
    ///
    /// `declaration` is that function's exact declaration site,
    /// `path:start:end`, from the resolved call edge — never a name.
    RefusingCalleeFixpoint { declaration: String },
}

impl CreatesDeclineKind {
    /// The stable wire name of this kind, as it reaches the proposal refusal
    /// audit and the ranking script.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::DialectSilent { .. } => "dialect-silent",
            Self::CreatePublishingCallee { .. } => "create-publishing-callee",
            Self::UnresolvedCallee { .. } => "unresolved-callee",
            Self::RefusingCalleeFixpoint { .. } => "refusing-callee-fixpoint",
        }
    }

    /// The callee's resolved package, or `""` where this kind names none.
    #[must_use]
    pub fn package(&self) -> &str {
        match self {
            Self::DialectSilent { package, .. } | Self::CreatePublishingCallee { package, .. } => {
                package
            }
            Self::UnresolvedCallee { .. } | Self::RefusingCalleeFixpoint { .. } => "",
        }
    }

    /// The callee's resolved export name, or `""` where this kind names none.
    #[must_use]
    pub fn callee_export(&self) -> &str {
        match self {
            Self::DialectSilent { export, .. } | Self::CreatePublishingCallee { export, .. } => {
                export
            }
            Self::UnresolvedCallee { .. } | Self::RefusingCalleeFixpoint { .. } => "",
        }
    }

    /// The refusing callee's declaration site, or `""` where this kind names
    /// none.
    #[must_use]
    pub fn declaration(&self) -> &str {
        match self {
            Self::RefusingCalleeFixpoint { declaration } => declaration,
            Self::DialectSilent { .. }
            | Self::CreatePublishingCallee { .. }
            | Self::UnresolvedCallee { .. } => "",
        }
    }

    /// The unresolved callee's observed shape name, or `""` where this kind
    /// names none.
    ///
    /// Additive on purpose: [`Self::name`] still answers `unresolved-callee`
    /// for every shape, so a report or sidecar written before the shapes
    /// existed carries the same `declinedClosuresByKind` counts it always did.
    #[must_use]
    pub const fn shape(&self) -> &'static str {
        match self {
            Self::UnresolvedCallee { shape } => shape.name(),
            Self::DialectSilent { .. }
            | Self::CreatePublishingCallee { .. }
            | Self::RefusingCalleeFixpoint { .. } => "",
        }
    }

    /// The one concrete string the shape observed, or `""` where this kind
    /// carries no shape. See [`UnresolvedCalleeShape::spelling`].
    #[must_use]
    pub fn shape_spelling(&self) -> &str {
        match self {
            Self::UnresolvedCallee { shape } => shape.spelling(),
            Self::DialectSilent { .. }
            | Self::CreatePublishingCallee { .. }
            | Self::RefusingCalleeFixpoint { .. } => "",
        }
    }
}

/// One refusing call site, with the reason it refuses.
///
/// Ordered by location first so a report is stable across builds; the kind
/// breaks ties for the (possible) two records of one span, where a fixpoint
/// marker sits beside the callee's own blocker.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CreatesDecline {
    /// File of the refusing **call**, not of the callee.
    pub path: String,
    pub start: u32,
    pub end: u32,
    pub kind: CreatesDeclineKind,
}

impl CreatesDecline {
    /// `path:start:end` of the refusing call.
    #[must_use]
    pub fn location(&self) -> String {
        let Self {
            path, start, end, ..
        } = self;
        format!("{path}:{start}:{end}")
    }
}

/// Call sites inside which a `creates: []` proposal may not be made, per file.
///
/// [`Self::default`] is the value of a build that never ran the walk, and it
/// proposes **nothing**: `walked` is false, so every span is refused. That is
/// the fail-closed default a serialized [`crate::Program`] round-trip must
/// land on, since the field is deliberately not carried on the wire.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CreatesProposalWalk {
    walked: bool,
    refusals: BTreeMap<String, Vec<(u32, u32)>>,
    /// Why each refusing call refuses, keyed by the call's file. Measurement
    /// only: nothing in the proposal decision reads it.
    declines: BTreeMap<String, Vec<CreatesDecline>>,
    /// The resolved local call edges the fixpoint followed, keyed by the
    /// caller's file: `(call span, callee file, callee span)`. Retained so
    /// [`Self::declines_for`] can name the blockers *inside* a helper an export
    /// only reaches through a call, which is where the fixpoint's own refusal
    /// came from.
    local_edges: BTreeMap<String, Vec<LocalCallEdge>>,
}

impl CreatesProposalWalk {
    /// Whether the walk ran and found no refusing call inside `span` of `path`.
    ///
    /// The containment is lexical and inclusive of nested callables, so a
    /// factory whose returned closure calls `render` does not propose.
    #[must_use]
    pub fn proposes(&self, path: &str, span: (u64, u64)) -> bool {
        if !self.walked {
            return false;
        }
        let (start, end) = span;
        !self.refusals.get(path).is_some_and(|spans| {
            spans.iter().any(|(call_start, call_end)| {
                u64::from(*call_start) >= start && u64::from(*call_end) <= end
            })
        })
    }

    /// Every blocker that makes `span` of `path` decline a `creates: []`
    /// proposal, sorted and deduplicated.
    ///
    /// Two sources, and both are needed for the set to be readable as "what
    /// would have to change":
    ///
    /// * every refusing call **lexically inside** the span, with its own kind;
    /// * transitively, the refusing calls inside every project function the
    ///   span reaches through a resolved local call edge — the same edges the
    ///   fixpoint followed to decide that the span refuses at all. Without this
    ///   an export whose only refusing call is a helper's `createEffect` would
    ///   report `refusing-callee-fixpoint` and name no primitive, and the
    ///   measurement this exists for would be empty on exactly the shape it was
    ///   built to count.
    ///
    /// A propagated record keeps **its own** location, inside the helper. The
    /// walk that reaches it is bounded by [`MAX_DECLINE_REPORT_DEPTH`] and by a
    /// visited set, so a recursive or mutually recursive helper terminates.
    ///
    /// Empty when the walk never ran: a build with no walk names no blocker,
    /// which is not the same claim as `proposes` returning `false` there.
    #[must_use]
    pub fn declines_for(&self, path: &str, span: (u64, u64)) -> Vec<CreatesDecline> {
        if !self.walked {
            return Vec::new();
        }
        let mut collected = BTreeSet::<CreatesDecline>::new();
        let mut visited = BTreeSet::<(&str, (u32, u32))>::new();
        let start = u32::try_from(span.0).unwrap_or(u32::MAX);
        let end = u32::try_from(span.1).unwrap_or(u32::MAX);
        let mut frontier = vec![(path, (start, end))];
        visited.insert((path, (start, end)));
        for _ in 0..MAX_DECLINE_REPORT_DEPTH {
            let mut next = Vec::new();
            for (region_path, region) in frontier.drain(..) {
                for decline in self.declines.get(region_path).into_iter().flatten() {
                    if region.0 <= decline.start && decline.end <= region.1 {
                        collected.insert(decline.clone());
                    }
                }
                for (call, callee_path, callee) in
                    self.local_edges.get(region_path).into_iter().flatten()
                {
                    if region.0 <= call.0
                        && call.1 <= region.1
                        && visited.insert((callee_path.as_str(), *callee))
                    {
                        next.push((callee_path.as_str(), *callee));
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            frontier = next;
        }
        collected.into_iter().collect()
    }

    /// How many call sites the walk refused, across every file. Diagnostic
    /// only.
    #[must_use]
    pub fn refused_calls(&self) -> usize {
        self.refusals.values().map(Vec::len).sum()
    }
}

/// The module specifier each imported symbol was bound from, over the whole
/// project: `symbol -> specifier`.
///
/// The second of the two resolved answers a `dialect-silent` record's package
/// half may come from, after the compiler's own
/// `ResolvedDeclaration::origin_module`. It is a *binding* fact — the import
/// statement whose local name this exact symbol is the entity of — and not a
/// name match: two files importing the same spelling from two specifiers have
/// two symbols and two entries. A symbol bound by two imports of the same name
/// cannot happen in one module; across modules the symbols differ, so the map
/// is keyed uniquely and the first writer wins only for a symbol some file
/// re-declares, which is not a case this map is asked about.
fn imported_modules_by_symbol<'a>(ctx: &AnalysisContext<'a>) -> HashMap<&'a str, &'a str> {
    let mut modules = HashMap::new();
    for file in &ctx.facts.files {
        for import in &file.ast.imports {
            if import.type_only {
                continue;
            }
            for binding in &import.bindings {
                if binding.type_only {
                    continue;
                }
                if let Some(symbol) = ctx.semantic_lookup.entity_symbol(file, binding.local.span) {
                    modules.entry(symbol).or_insert(import.module.as_str());
                }
            }
        }
    }
    modules
}

/// The per-file tables [`unresolved_callee_shape`] needs and no existing index
/// already provides, built once per file instead of once per call.
///
/// Everything here is derived from facts the build already computed; nothing
/// demands anything new from the producer.
struct FileShapeFacts<'a> {
    /// Spans that are exactly a call expression, for the `expression-callee`
    /// decision. `calls_by_callee` cannot answer it: the question is whether
    /// the *callee* span is itself a call, not whether a call has that callee.
    call_spans: HashSet<Span>,
    /// Spans that are exactly a function/arrow expression, for the same
    /// decision.
    function_spans: HashSet<Span>,
    /// `binding symbol -> the symbol of the identifier that binding was
    /// initialized from`, for [`roots_in_caller_parameter`]'s alias hops. Only
    /// a direct `const p = q` initializer is recorded, because
    /// `BindingFact::initializer_identifier` is exactly that fact and nothing
    /// weaker is inferred.
    initializer_aliases: HashMap<&'a str, &'a str>,
}

fn file_shape_facts<'a>(
    ctx: &AnalysisContext<'a>,
    file: &'a solid_facts::FileFacts,
) -> FileShapeFacts<'a> {
    let mut initializer_aliases = HashMap::new();
    for binding in &file.ast.bindings {
        let Some(initializer) = binding.initializer_identifier.as_ref() else {
            continue;
        };
        let Some(source) = ctx.semantic_lookup.entity_symbol(file, initializer.span) else {
            continue;
        };
        for name in &binding.names {
            if let Some(symbol) = ctx.semantic_lookup.entity_symbol(file, name.span) {
                initializer_aliases.entry(symbol).or_insert(source);
            }
        }
    }
    FileShapeFacts {
        call_spans: file.ast.calls.iter().map(|call| call.span).collect(),
        function_spans: file
            .ast
            .functions
            .iter()
            .map(|function| function.span)
            .collect(),
        initializer_aliases,
    }
}

/// One source string, reduced to something an emitter line can carry.
///
/// Whitespace-free and bounded: a property name, identifier, or specifier is
/// already both, and a pathological span must not be able to inject a tab or a
/// newline into the record. Truncation is on a character boundary so the result
/// stays valid UTF-8, and an empty result stays empty.
fn spelling_of(text: &str) -> String {
    let trimmed: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if trimmed.len() <= MAX_SPELLING_BYTES {
        return trimmed;
    }
    let mut cut = MAX_SPELLING_BYTES;
    while cut > 0 && !trimmed.is_char_boundary(cut) {
        cut -= 1;
    }
    trimmed[..cut].to_owned()
}

/// Whether `span` is exactly one of this file's identifier facts.
fn is_identifier(file: &solid_facts::FileFacts, span: Span) -> bool {
    file.ast
        .identifiers
        .binary_search_by_key(&span, |identifier| identifier.span)
        .is_ok()
}

/// Whether `root` — or a symbol it reaches through at most
/// [`MAX_PARAMETER_ALIAS_HOPS`] binding-initializer aliases — is a parameter
/// name of a function whose **body contains** `callee`.
///
/// Containment is what makes the answer about *this* call: a parameter of an
/// unrelated function is a different symbol, and requiring the containing
/// function keeps the record from claiming a scope it did not check. Both
/// ordinary and rest parameter names count; a destructured parameter's every
/// bound name counts, because each is a caller-supplied value.
fn roots_in_caller_parameter<'a>(
    ctx: &AnalysisContext<'a>,
    file: &'a solid_facts::FileFacts,
    callee: Span,
    root: &str,
    shape_facts: &FileShapeFacts<'a>,
) -> bool {
    let mut current = root;
    let mut visited = HashSet::new();
    for _ in 0..MAX_PARAMETER_ALIAS_HOPS {
        if !visited.insert(current) {
            return false;
        }
        let is_parameter = file.ast.functions_body_containing(callee).any(|function| {
            let mut declared = function
                .parameters
                .iter()
                .flat_map(|parameter| parameter.names.iter())
                .chain(function.rest_parameter_names.iter());
            declared.any(|name| {
                ctx.semantic_lookup
                    .entity_symbol(file, name.span)
                    .is_some_and(|symbol| symbol == current)
            })
        });
        if is_parameter {
            return true;
        }
        match shape_facts.initializer_aliases.get(current) {
            Some(next) => current = next,
            None => return false,
        }
    }
    false
}

/// The syntactic kind of a callee expression, from a fixed vocabulary.
///
/// Decided by which of this file's own syntax tables holds the exact span, in
/// the order most specific first. `unknown-expression` is the honest answer
/// where no table does — a new syntax the fact tables do not name is not
/// silently folded into one that exists.
fn callee_syntax_kind(
    file: &solid_facts::FileFacts,
    span: Span,
    shape_facts: &FileShapeFacts<'_>,
) -> &'static str {
    if shape_facts.call_spans.contains(&span) {
        return "call-expression";
    }
    if shape_facts.function_spans.contains(&span) {
        return "function-expression";
    }
    if file.ast.awaits.binary_search(&span).is_ok() {
        return "await-expression";
    }
    if file
        .ast
        .conditional_expressions
        .iter()
        .any(|conditional| conditional.span == span)
    {
        return "conditional-expression";
    }
    if file
        .ast
        .logical_expressions
        .iter()
        .any(|logical| logical.span == span)
    {
        return "logical-expression";
    }
    if file
        .ast
        .jsx_elements
        .iter()
        .any(|element| element.span == span)
    {
        return "jsx-element";
    }
    "unknown-expression"
}

/// The shape of a callee [`crate::indexes::SemanticLookup::callee_symbol`]
/// answered nothing for.
///
/// **The order is the contract**, because a call can satisfy two predicates at
/// once. It is, exactly:
///
/// 1. a computed member access — one syntax fact, and no static property
///    spelling exists at all;
/// 2. a member chain rooted in a caller-supplied parameter — the callee is not
///    decidable from this module's bytes, whatever the property is;
/// 3. a non-computed member, split by whether the *receiver* resolved;
/// 4. a bare identifier with no symbol;
/// 5. a callee that is itself a call or a function expression;
/// 6. anything else, carrying its syntactic kind.
///
/// Every step reads a fact the build already has. Nothing here resolves a
/// symbol the walk did not already fail to resolve, and no step is a claim
/// about what the callee *does*.
fn unresolved_callee_shape<'a>(
    ctx: &AnalysisContext<'a>,
    file: &'a solid_facts::FileFacts,
    callee: Span,
    shape_facts: &FileShapeFacts<'a>,
) -> UnresolvedCalleeShape {
    let peeled = file.ast.peel_ts_sugar_span(callee);
    let member = file
        .ast
        .members
        .binary_search_by_key(&peeled, |member| member.span)
        .ok()
        .map(|index| &file.ast.members[index]);
    if let Some(member) = member {
        let object = file.ast.peel_ts_sugar_span(member.object);
        if file.ast.computed_members.binary_search(&peeled).is_ok() {
            // No static property spelling exists, so the record carries the
            // receiver where it is nameable and nothing where it is not.
            let receiver = is_identifier(file, object)
                .then(|| file.source_text(object).map(spelling_of))
                .flatten()
                .unwrap_or_default();
            return UnresolvedCalleeShape::ComputedMember { receiver };
        }
        let property = file
            .source_text(member.property)
            .map(spelling_of)
            .unwrap_or_default();
        // `member_callee_receiver` pins the chain's root to a plain identifier
        // with a resolved entity, which is exactly the root the parameter
        // question is about; it answers `None` for anything weaker.
        if ctx
            .semantic_lookup
            .member_callee_receiver(file, peeled)
            .is_some_and(|(root, _)| {
                roots_in_caller_parameter(ctx, file, callee, root.as_str(), shape_facts)
            })
        {
            return UnresolvedCalleeShape::ParameterRooted { property };
        }
        return if ctx.semantic_lookup.entity_symbol(file, object).is_some() {
            UnresolvedCalleeShape::MemberPropertyUnresolved { property }
        } else {
            UnresolvedCalleeShape::MemberReceiverUnresolved { property }
        };
    }
    if is_identifier(file, peeled) {
        return UnresolvedCalleeShape::UndeclaredIdentifier {
            identifier: file
                .source_text(peeled)
                .map(spelling_of)
                .unwrap_or_default(),
        };
    }
    match callee_syntax_kind(file, peeled, shape_facts) {
        syntax @ ("call-expression" | "function-expression") => {
            UnresolvedCalleeShape::ExpressionCallee { syntax }
        }
        syntax => UnresolvedCalleeShape::Other { syntax },
    }
}

pub(crate) fn collect_project(ctx: &AnalysisContext<'_>) -> CreatesProposalWalk {
    let imported_modules = imported_modules_by_symbol(ctx);
    let mut refusals = BTreeMap::<String, Vec<(u32, u32)>>::new();
    let mut declines = BTreeMap::<String, Vec<CreatesDecline>>::new();
    // Every call whose callee resolves to a project function, so the fixpoint
    // below can follow the edge: (caller file, call span) -> (callee file,
    // callee function span).
    let mut local_edges = Vec::<(&str, (u32, u32), &str, Span)>::new();
    for file in &ctx.facts.files {
        let primitives = ctx.semantic_lookup.primitives(file);
        let shape_facts = file_shape_facts(ctx, file);
        for (index, call) in file.ast.calls.iter().enumerate() {
            if let Some(kind) = creates_proposal_decline(
                ctx,
                file,
                call.callee,
                primitives.calls.get(index).and_then(Option::as_ref),
                &imported_modules,
                &shape_facts,
            ) {
                refusals
                    .entry(file.path.to_string())
                    .or_default()
                    .push((call.span.start, call.span.end));
                declines
                    .entry(file.path.to_string())
                    .or_default()
                    .push(CreatesDecline {
                        path: file.path.to_string(),
                        start: call.span.start,
                        end: call.span.end,
                        kind,
                    });
                continue;
            }
            if let Some((target_file, target)) = ctx
                .semantic_lookup
                .callee_symbol(file, call.callee)
                .and_then(|symbol| ctx.semantic_lookup.function_for_symbol(symbol))
            {
                local_edges.push((
                    file.path.as_str(),
                    (call.span.start, call.span.end),
                    target_file.path.as_str(),
                    target.span,
                ));
            }
        }
    }
    // Fixpoint over the local call edges: a call into a function whose span
    // contains a refusing call refuses too. Monotone and finite — every
    // iteration adds at least one span or stops — so it terminates.
    loop {
        let mut added = false;
        for (caller_path, call, callee_path, callee) in &local_edges {
            let already = refusals
                .get(*caller_path)
                .is_some_and(|spans| spans.contains(call));
            if already {
                continue;
            }
            let callee_refuses = refusals.get(*callee_path).is_some_and(|spans| {
                spans
                    .iter()
                    .any(|(start, end)| callee.start <= *start && *end <= callee.end)
            });
            if callee_refuses {
                refusals
                    .entry((*caller_path).to_owned())
                    .or_default()
                    .push(*call);
                declines
                    .entry((*caller_path).to_owned())
                    .or_default()
                    .push(CreatesDecline {
                        path: (*caller_path).to_owned(),
                        start: call.0,
                        end: call.1,
                        kind: CreatesDeclineKind::RefusingCalleeFixpoint {
                            declaration: format!("{callee_path}:{}:{}", callee.start, callee.end),
                        },
                    });
                added = true;
            }
        }
        if !added {
            break;
        }
    }
    for spans in refusals.values_mut() {
        spans.sort_unstable();
        spans.dedup();
    }
    for records in declines.values_mut() {
        records.sort();
        records.dedup();
    }
    let mut edges_by_path = BTreeMap::<String, Vec<LocalCallEdge>>::new();
    for (caller_path, call, callee_path, callee) in local_edges {
        edges_by_path
            .entry(caller_path.to_owned())
            .or_default()
            .push((call, callee_path.to_owned(), (callee.start, callee.end)));
    }
    for edges in edges_by_path.values_mut() {
        edges.sort();
        edges.dedup();
    }
    CreatesProposalWalk {
        walked: true,
        refusals,
        declines,
        local_edges: edges_by_path,
    }
}

/// Why this call site refuses a `creates: []` proposal, or `None` when it does
/// not refuse.
///
/// The dispositions and their order are unchanged from the bare-boolean form;
/// only the answer got a name.
fn creates_proposal_decline<'a>(
    ctx: &AnalysisContext<'a>,
    file: &'a solid_facts::FileFacts,
    callee: Span,
    primitive: Option<&PrimitiveName>,
    imported_modules: &HashMap<&str, &str>,
    shape_facts: &FileShapeFacts<'a>,
) -> Option<CreatesDeclineKind> {
    // A canonical primitive is decided by the dialect tables and nothing else.
    // The audits are the only negative authority about a primitive, and their
    // silence — an unaudited dialect, a withheld row, a domain no dialect has
    // admitted — is "do not propose".
    if let Some(PrimitiveName::Known(_, spelling)) = primitive {
        if solid_dialect::some_audit_denies_primitive(spelling, CallClaimDomain::Creates) {
            return None;
        }
        return Some(CreatesDeclineKind::DialectSilent {
            // Two resolved answers, in order, and no third: the compiler's own
            // `origin_module` for the callee's resolved declaration, then the
            // specifier of the import statement this exact callee *symbol* is
            // the binding of. Empty where neither answers — a primitive is
            // recognized by dialect vocabulary, which says nothing about which
            // archive the callee reached, so guessing one from the spelling
            // would invent the very identity the census exists to bind.
            package: ctx
                .semantic_lookup
                .callee_origin_module(file, callee)
                .or_else(|| {
                    ctx.semantic_lookup
                        .callee_symbol(file, callee)
                        .and_then(|symbol| imported_modules.get(symbol).copied())
                })
                .map(package_of_module)
                .unwrap_or_default(),
            export: (*spelling).to_owned(),
        });
    }
    let Some(symbol) = ctx.semantic_lookup.callee_symbol(file, callee) else {
        // No identity to name -- that is the refusal -- but the callee
        // expression's shape is observable, and it is what tells a resolver gap
        // from a genuinely undecidable call.
        return Some(CreatesDeclineKind::UnresolvedCallee {
            shape: unresolved_callee_shape(ctx, file, callee, shape_facts),
        });
    };
    if ctx
        .semantic_lookup
        .contract_creates_closed_empty(symbol)
        .is_some_and(|closed_empty| !closed_empty)
    {
        let (package, export) = ctx
            .semantic_lookup
            .contract_export_identity(symbol)
            .unwrap_or_default();
        return Some(CreatesDeclineKind::CreatePublishingCallee {
            package: package.to_owned(),
            export: export.to_owned(),
        });
    }
    None
}

/// The package a resolved module specifier names: its first segment, or the
/// first two when the specifier is scoped.
///
/// A specifier, never a path — the input is the compiler's own
/// `ResolvedDeclaration::origin_module`.
fn package_of_module(module: &str) -> String {
    let mut parts = module.split('/');
    let Some(first) = parts.next().filter(|part| !part.is_empty()) else {
        return String::new();
    };
    if !first.starts_with('@') {
        return first.to_owned();
    }
    match parts.next().filter(|part| !part.is_empty()) {
        Some(scoped) => format!("{first}/{scoped}"),
        None => first.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CreatesDecline, CreatesDeclineKind, CreatesProposalWalk, LocalCallEdge,
        UnresolvedCalleeShape, package_of_module, spelling_of,
    };
    use std::collections::BTreeMap;

    /// One edge as a test writes it: caller file, call span, callee file,
    /// callee span.
    type TestEdge = (&'static str, (u32, u32), &'static str, (u32, u32));

    fn decline(path: &str, start: u32, end: u32, kind: CreatesDeclineKind) -> CreatesDecline {
        CreatesDecline {
            path: path.to_owned(),
            start,
            end,
            kind,
        }
    }

    fn walk(
        refusals: &[(&str, (u32, u32))],
        declines: Vec<CreatesDecline>,
        edges: &[TestEdge],
    ) -> CreatesProposalWalk {
        let mut refusal_map = BTreeMap::<String, Vec<(u32, u32)>>::new();
        for (path, span) in refusals {
            refusal_map
                .entry((*path).to_owned())
                .or_default()
                .push(*span);
        }
        let mut decline_map = BTreeMap::<String, Vec<CreatesDecline>>::new();
        for record in declines {
            decline_map
                .entry(record.path.clone())
                .or_default()
                .push(record);
        }
        let mut edge_map = BTreeMap::<String, Vec<LocalCallEdge>>::new();
        for (caller, call, callee_path, callee) in edges {
            edge_map.entry((*caller).to_owned()).or_default().push((
                *call,
                (*callee_path).to_owned(),
                *callee,
            ));
        }
        CreatesProposalWalk {
            walked: true,
            refusals: refusal_map,
            declines: decline_map,
            local_edges: edge_map,
        }
    }

    #[test]
    fn a_walk_that_never_ran_names_no_blocker_and_proposes_nothing() {
        let empty = CreatesProposalWalk::default();
        assert!(!empty.proposes("/p/index.js", (0, 100)));
        assert!(empty.declines_for("/p/index.js", (0, 100)).is_empty());
    }

    #[test]
    fn declines_are_reported_lexically_and_through_the_local_call_edges() {
        // export function f() { helper() }   spans 0..40, the call at 22..30
        // function helper() { createEffect() }  spans 60..120, the call at 80..96
        let silent = CreatesDeclineKind::DialectSilent {
            package: "solid-js".into(),
            export: "createEffect".into(),
        };
        let walk = walk(
            &[("/p/index.js", (22, 30)), ("/p/index.js", (80, 96))],
            vec![
                decline(
                    "/p/index.js",
                    22,
                    30,
                    CreatesDeclineKind::RefusingCalleeFixpoint {
                        declaration: "/p/index.js:60:120".into(),
                    },
                ),
                decline("/p/index.js", 80, 96, silent.clone()),
            ],
            &[("/p/index.js", (22, 30), "/p/index.js", (60, 120))],
        );
        // The export's own span contains only the fixpoint record; the
        // primitive that actually blocks it is inside the helper.
        let reported = walk.declines_for("/p/index.js", (0, 40));
        assert_eq!(reported.len(), 2, "{reported:?}");
        assert_eq!(reported[0].kind.name(), "refusing-callee-fixpoint");
        assert_eq!(reported[0].location(), "/p/index.js:22:30");
        assert_eq!(reported[1].kind, silent);
        assert_eq!(
            reported[1].location(),
            "/p/index.js:80:96",
            "a propagated record keeps its own location"
        );
        assert!(!walk.proposes("/p/index.js", (0, 40)));
    }

    #[test]
    fn a_mutually_recursive_helper_pair_terminates() {
        let walk = walk(
            &[("/p/a.js", (10, 20))],
            vec![decline(
                "/p/a.js",
                10,
                20,
                CreatesDeclineKind::UnresolvedCallee {
                    shape: UnresolvedCalleeShape::UndeclaredIdentifier {
                        identifier: "externalGlobal".into(),
                    },
                },
            )],
            &[
                ("/p/a.js", (10, 20), "/p/a.js", (0, 30)),
                ("/p/a.js", (25, 28), "/p/a.js", (0, 30)),
            ],
        );
        let reported = walk.declines_for("/p/a.js", (0, 30));
        assert_eq!(reported.len(), 1);
        assert_eq!(reported[0].kind.name(), "unresolved-callee");
    }

    #[test]
    fn a_spelling_drops_whitespace_and_is_bounded() {
        assert_eq!(spelling_of("onChange"), "onChange");
        // A tab or newline would break the emitter's own line format.
        assert_eq!(spelling_of("on\tChange\n"), "onChange");
        let long = "a".repeat(200);
        assert_eq!(spelling_of(&long).len(), 64);
        assert_eq!(spelling_of(""), "");
    }

    #[test]
    fn every_shape_names_itself_and_the_one_string_it_observed() {
        let cases = [
            (
                UnresolvedCalleeShape::ComputedMember {
                    receiver: "handlers".into(),
                },
                "computed-member",
                "handlers",
            ),
            (
                UnresolvedCalleeShape::ParameterRooted {
                    property: "onChange".into(),
                },
                "parameter-rooted",
                "onChange",
            ),
            (
                UnresolvedCalleeShape::MemberPropertyUnresolved {
                    property: "read".into(),
                },
                "member-property-unresolved",
                "read",
            ),
            (
                UnresolvedCalleeShape::MemberReceiverUnresolved {
                    property: "method".into(),
                },
                "member-receiver-unresolved",
                "method",
            ),
            (
                UnresolvedCalleeShape::UndeclaredIdentifier {
                    identifier: "externalGlobal".into(),
                },
                "undeclared-identifier",
                "externalGlobal",
            ),
            (
                UnresolvedCalleeShape::ExpressionCallee {
                    syntax: "call-expression",
                },
                "expression-callee",
                "call-expression",
            ),
            (
                UnresolvedCalleeShape::Other {
                    syntax: "conditional-expression",
                },
                "other",
                "conditional-expression",
            ),
        ];
        for (shape, name, spelling) in cases {
            assert_eq!(shape.name(), name);
            assert_eq!(shape.spelling(), spelling, "{name}");
            let kind = CreatesDeclineKind::UnresolvedCallee { shape };
            // The kind's own wire name is unchanged, which is what keeps every
            // existing `declinedClosuresByKind` count intact.
            assert_eq!(kind.name(), "unresolved-callee");
            assert_eq!(kind.shape(), name);
            assert_eq!(kind.shape_spelling(), spelling);
        }
        // A kind that carries no shape says so, rather than naming one.
        let fixpoint = CreatesDeclineKind::RefusingCalleeFixpoint {
            declaration: "/p/index.js:0:10".into(),
        };
        assert_eq!(fixpoint.shape(), "");
        assert_eq!(fixpoint.shape_spelling(), "");
    }

    #[test]
    fn a_module_specifier_reduces_to_its_package() {
        assert_eq!(package_of_module("solid-js"), "solid-js");
        assert_eq!(package_of_module("solid-js/web"), "solid-js");
        assert_eq!(package_of_module("@solidjs/signals"), "@solidjs/signals");
        assert_eq!(package_of_module("@solidjs/web/client"), "@solidjs/web");
        assert_eq!(package_of_module("@solidjs"), "@solidjs");
        assert_eq!(package_of_module(""), "");
    }

    #[test]
    fn every_kind_names_only_the_identity_it_carries() {
        let silent = CreatesDeclineKind::DialectSilent {
            package: "solid-js".into(),
            export: "createEffect".into(),
        };
        assert_eq!(silent.name(), "dialect-silent");
        assert_eq!(silent.package(), "solid-js");
        assert_eq!(silent.callee_export(), "createEffect");
        assert_eq!(silent.declaration(), "");

        let publishing = CreatesDeclineKind::CreatePublishingCallee {
            package: "@solid-primitives/timer".into(),
            export: "makeTimer".into(),
        };
        assert_eq!(publishing.name(), "create-publishing-callee");
        assert_eq!(publishing.package(), "@solid-primitives/timer");
        assert_eq!(publishing.callee_export(), "makeTimer");

        let unresolved = CreatesDeclineKind::UnresolvedCallee {
            shape: UnresolvedCalleeShape::MemberPropertyUnresolved {
                property: "read".into(),
            },
        };
        assert_eq!(unresolved.name(), "unresolved-callee");
        assert_eq!(unresolved.package(), "");
        assert_eq!(unresolved.callee_export(), "");
        assert_eq!(unresolved.declaration(), "");
        assert_eq!(unresolved.shape(), "member-property-unresolved");
        assert_eq!(unresolved.shape_spelling(), "read");

        let fixpoint = CreatesDeclineKind::RefusingCalleeFixpoint {
            declaration: "/p/index.js:60:120".into(),
        };
        assert_eq!(fixpoint.name(), "refusing-callee-fixpoint");
        assert_eq!(fixpoint.declaration(), "/p/index.js:60:120");
        assert_eq!(fixpoint.package(), "");
    }
}
