//! Props reactivity: per-component prop classification, forwarding, and the index built from it.

use super::*;

/// How each use of one prop is classified once the component's callers are
/// enumerated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PropUse {
    /// Every visible call site passes a static value: the prop compiles to a
    /// plain property and reading it can never warn or misbehave.
    Static,
    /// Some visible call site passes a proven reactive expression: the prop
    /// is signal-backed and untracked reads are proven runtime warnings.
    Reactive,
    /// Neither provable: the component escapes enumeration, or a passed value
    /// resolves to nothing the engine can classify.
    Unknown,
}

impl PropUse {
    fn worst(self, other: Self) -> Self {
        match (self, other) {
            (Self::Reactive, _) | (_, Self::Reactive) => Self::Reactive,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            _ => Self::Static,
        }
    }
}

/// Caller-proven reactivity of one component's props.
///
/// rc.0 ground truth (probed): `devComponent` opens a strict-read window with
/// `untrack(() => Comp(props), '<Name>')`, and `STRICT_READ_UNTRACKED` fires
/// only when a prop *getter* reads reactive state inside it. `{ title:
/// "Hello" }` never warns; `{ get title() { return sig() } }` warns. Whether
/// a prop is signal-backed is therefore a fact about the component's callers,
/// which this records per prop name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PropsReactivity {
    /// Every in-project use is an enumerated JSX call site with no spread
    /// attributes. `reactive` holds the prop names some call site passes a
    /// proven reactive expression for; `unresolved` holds the names whose
    /// passed value could be proven neither way. Every other prop is proven
    /// static.
    Enumerated {
        reactive: BTreeSet<String>,
        unresolved: BTreeSet<String>,
        /// Prop names for which at least one exact JSX call site passes a
        /// proven accessor value. Reading the property is static, but calling
        /// that value performs a reactive read and must stay distinguishable
        /// from ordinary prop backing.
        accessor_values: BTreeSet<String>,
        /// What the call sites pass for each prop a call of the prop's value
        /// may act through ([`PassedSources`]).
        passed: PassedSources,
    },
    /// The component escapes enumeration — exported, referenced outside JSX,
    /// or spread into at a call site — so no prop can be proven *static*.
    ///
    /// The JSX call sites that are visible still prove what they pass, and
    /// that direction of the proof survives the escape. "Some caller passes a
    /// reactive expression" is monotone under adding callers: a consumer
    /// outside the project can add a call site, never remove the one written
    /// here, so an untracked read of a witnessed prop is a proven defect on
    /// that path. "Every caller passes a static value" is the opposite — one
    /// unseen caller falsifies it — which is why it needs complete
    /// enumeration and is never concluded here.
    ///
    /// `reactive` holds the witnessed prop names and `accessor_values` the
    /// names some visible call site passes a proven accessor for. Every other
    /// prop is a proof obligation.
    Escaping {
        reactive: BTreeSet<String>,
        accessor_values: BTreeSet<String>,
        passed: PassedSources,
    },
}

/// ADR 0509: per prop name, the exact sources (a primitive's accessor, setter
/// or action binding) the visible call sites pass as the attribute's whole
/// value, and whether some visible site passes anything else. A call of
/// `props.name(…)` calls one of these sources at the site that passed it:
/// "some caller passes this setter" is monotone under adding callers, as the
/// `reactive` witness is. A component with an escaping or unclassified value
/// keeps every call of the prop an obligation.
pub(crate) type PassedSources = BTreeMap<String, PassedSource>;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PassedSource {
    pub(crate) sources: BTreeSet<SymbolId>,
    /// Some visible site passes a value that is not an exact source.
    pub(crate) other: bool,
    /// Some visible site passes a getter that calls an async source
    /// (`a={data()}`): reading the prop reads it, pending or not.
    pub(crate) async_getter: bool,
}

/// The per-declaration classification map, keyed by the props parameter's
/// declaration location (the same location `prop_sources` carries, so aliases
/// and `merge` results resolve to their component's classification) and by
/// the parameter pattern span (how the destructure rule addresses a
/// destructured parameter).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PropsReactivityIndex {
    /// Whether the dialect asked for caller proof at all. When false, every
    /// query answers [`PropUse::Reactive`] — the upstream over-approximation
    /// the 1.x catalog pins for eslint-plugin-solid parity.
    pub(super) caller_proof: bool,
    pub(super) by_declaration: HashMap<Location, PropsReactivity>,
}

impl PropsReactivityIndex {
    pub(crate) fn prop_use(&self, declaration: &Location, name: &str) -> PropUse {
        if !self.caller_proof {
            return PropUse::Reactive;
        }
        self.by_declaration
            .get(declaration)
            .map_or(PropUse::Unknown, |classification| {
                classification_use(classification, name)
            })
    }

    /// The classification of a whole-object use (aliasing, spreading, or
    /// destructuring with a rest element): reactive if any prop is, unknown
    /// if any prop is, static only when every caller-passed value is.
    pub(crate) fn object_use(&self, declaration: &Location) -> PropUse {
        if !self.caller_proof {
            return PropUse::Reactive;
        }
        match self.by_declaration.get(declaration) {
            None => PropUse::Unknown,
            Some(PropsReactivity::Escaping { reactive, .. }) => {
                if reactive.is_empty() {
                    PropUse::Unknown
                } else {
                    PropUse::Reactive
                }
            }
            Some(PropsReactivity::Enumerated {
                reactive,
                unresolved,
                ..
            }) => {
                if !reactive.is_empty() {
                    PropUse::Reactive
                } else if !unresolved.is_empty() {
                    PropUse::Unknown
                } else {
                    PropUse::Static
                }
            }
        }
    }

    /// Whether invoking this prop value is proven to invoke an accessor at
    /// any enumerated JSX call site. This is separate from [`Self::prop_use`]:
    /// passing `items` as `<List items={items}>` stores a stable function in
    /// the prop, while `props.items()` still subscribes when evaluated.
    pub(crate) fn accessor_value_use(&self, declaration: &Location, name: &str) -> PropUse {
        if !self.caller_proof {
            return PropUse::Unknown;
        }
        match self.by_declaration.get(declaration) {
            Some(
                PropsReactivity::Enumerated {
                    accessor_values, ..
                }
                | PropsReactivity::Escaping {
                    accessor_values, ..
                },
            ) if accessor_values.contains(name) => PropUse::Reactive,
            // Only a complete caller set can prove the value is *not* an
            // accessor; an escaping component keeps the obligation.
            Some(PropsReactivity::Enumerated { .. }) => PropUse::Static,
            None | Some(PropsReactivity::Escaping { .. }) => PropUse::Unknown,
        }
    }

    /// ADR 0509: the exact sources the call sites pass for `name`, and
    /// whether they are all a call of `props.name(…)` can reach: every caller
    /// enumerated, and none passing anything else.
    pub(crate) fn passed_sources(
        &self,
        declaration: &Location,
        name: &str,
    ) -> (Vec<SymbolId>, bool) {
        if !self.caller_proof {
            return (Vec::new(), false);
        }
        match self.by_declaration.get(declaration) {
            Some(PropsReactivity::Enumerated { passed, .. }) => {
                passed.get(name).map_or((Vec::new(), true), |passed| {
                    (passed.sources.iter().cloned().collect(), !passed.other)
                })
            }
            Some(PropsReactivity::Escaping { passed, .. }) => (
                passed
                    .get(name)
                    .map(|passed| passed.sources.iter().cloned().collect())
                    .unwrap_or_default(),
                false,
            ),
            None => (Vec::new(), false),
        }
    }

    /// ADR 0509: whether some visible call site passes `name` a getter that
    /// calls an async source.
    pub(crate) fn async_getter(&self, declaration: &Location, name: &str) -> bool {
        match self.by_declaration.get(declaration) {
            Some(
                PropsReactivity::Enumerated { passed, .. }
                | PropsReactivity::Escaping { passed, .. },
            ) => passed.get(name).is_some_and(|passed| passed.async_getter),
            None => false,
        }
    }

    /// The worst classification across an enumerated set of prop names.
    /// Without caller proof every destructure keeps the upstream answer —
    /// including the empty pattern, which binds nothing but is still the
    /// shape the 1.x rule reports.
    pub(crate) fn names_use<'n>(
        &self,
        declaration: &Location,
        names: impl Iterator<Item = &'n str>,
    ) -> PropUse {
        if !self.caller_proof {
            return PropUse::Reactive;
        }
        names.fold(PropUse::Static, |worst, name| {
            worst.worst(self.prop_use(declaration, name))
        })
    }

    /// The per-symbol view the incremental cache fingerprints: the
    /// classification behind one props symbol's declaration, if any.
    pub(crate) fn for_declaration(&self, declaration: &Location) -> Option<&PropsReactivity> {
        self.by_declaration.get(declaration)
    }
}

/// A project function a JSX tag renders: its file's path and its span.
pub(super) type RenderedAt = (Arc<str>, Span);

/// A JSX element: its file's slot key and its ordinal there.
pub(super) type RenderingAt = (crate::base::FileId, u32);

/// A declaration's path and byte range.
pub(super) type DeclarationAt = (Arc<str>, u64, u64);

/// The source tables and indexes the component pass (pass 10) reads, each
/// read recorded at its key ([`super::trace`]): a component's rows are kept
/// while every key they read answers as it did.
pub(super) struct ComponentInputs<'s> {
    pub(super) files: &'s [FileFacts],
    pub(super) accessors: &'s HashMap<SymbolId, (SymbolId, Location)>,
    pub(super) source_kinds: &'s HashMap<SymbolId, ReactiveSourceKind>,
    pub(super) prop_sources: &'s HashMap<SymbolId, (SymbolId, Location)>,
    pub(super) setters: &'s HashMap<
        SymbolId,
        (
            SymbolId,
            Location,
            crate::OwnedWriteOption,
            ReactiveSourceKind,
        ),
    >,
    pub(super) actions: &'s HashMap<SymbolId, (SymbolId, Location)>,
    pub(super) async_sources: &'s HashSet<SymbolId>,
    /// Runtime sources no file writes.
    pub(super) runtime_sources: &'s HashSet<SymbolId>,
    /// The JSX elements rendering each project function, in file order:
    /// each element's file slot key and ordinal.
    pub(super) uses: &'s BTreeMap<RenderedAt, Arc<Vec<RenderingAt>>>,
    /// The slot a file key names in this build.
    pub(super) slots: &'s HashMap<crate::base::FileId, usize>,
    /// The symbols some analyzed file exports.
    pub(super) exported: &'s BTreeMap<SymbolId, ()>,
    /// The prop sources attached to each declaration, sorted, built on first
    /// use.
    pub(super) attached: std::sync::OnceLock<HashMap<DeclarationAt, Vec<SymbolId>>>,
}

impl ComponentInputs<'_> {
    fn attached_index(&self) -> &HashMap<DeclarationAt, Vec<SymbolId>> {
        self.attached.get_or_init(|| {
            let mut index = HashMap::<DeclarationAt, Vec<SymbolId>>::default();
            for (symbol, (_, declaration)) in self.prop_sources {
                index
                    .entry((
                        Arc::clone(&declaration.path),
                        declaration.start_byte,
                        declaration.end_byte,
                    ))
                    .or_default()
                    .push(symbol.clone());
            }
            for symbols in index.values_mut() {
                symbols.sort_unstable();
            }
            index
        })
    }

    /// What `key` answers now, as [`super::trace::Dep::State`] records it;
    /// `None` for a key this pass does not read.
    pub(super) fn answer(&self, key: &super::trace::StateKey) -> Option<u64> {
        use super::trace::{StateKey, fingerprint, kind_fingerprint};
        Some(match key {
            StateKey::Accessor(symbol) => u64::from(self.accessors.contains_key(symbol)),
            StateKey::Kind(symbol) => kind_fingerprint(self.source_kinds.get(symbol)),
            StateKey::PropSource(symbol) => fingerprint(&self.prop_sources.get(symbol)),
            StateKey::Setter(symbol) => u64::from(self.setters.contains_key(symbol)),
            StateKey::Action(symbol) => u64::from(self.actions.contains_key(symbol)),
            StateKey::Async(symbol) => u64::from(self.async_sources.contains(symbol)),
            StateKey::Runtime(symbol) => u64::from(self.runtime_sources.contains(symbol)),
            StateKey::Uses(path, span) => fingerprint(&self.uses.get(&(Arc::clone(path), *span))),
            StateKey::Exported(symbol) => u64::from(self.exported.contains_key(symbol)),
            StateKey::Attached(path, start, end) => {
                fingerprint(&self.attached_index().get(&(Arc::clone(path), *start, *end)))
            }
            StateKey::Runtime0(_) | StateKey::Kind0(_) => return None,
        })
    }

    fn note(&self, key: super::trace::StateKey) {
        if super::trace::recording() {
            let answer = self.answer(&key).expect("a key the component pass reads");
            super::trace::note(super::trace::Dep::State(key, answer));
        }
    }

    fn is_accessor(&self, symbol: &SymbolId) -> bool {
        self.note(super::trace::StateKey::Accessor(symbol.clone()));
        self.accessors.contains_key(symbol)
    }

    fn is_setter(&self, symbol: &SymbolId) -> bool {
        self.note(super::trace::StateKey::Setter(symbol.clone()));
        self.setters.contains_key(symbol)
    }

    fn is_async(&self, symbol: &SymbolId) -> bool {
        self.note(super::trace::StateKey::Async(symbol.clone()));
        self.async_sources.contains(symbol)
    }

    fn is_action(&self, symbol: &SymbolId) -> bool {
        self.note(super::trace::StateKey::Action(symbol.clone()));
        self.actions.contains_key(symbol)
    }

    fn kind(&self, symbol: &SymbolId) -> Option<ReactiveSourceKind> {
        self.note(super::trace::StateKey::Kind(symbol.clone()));
        self.source_kinds.get(symbol).copied()
    }

    fn prop_source(&self, symbol: &SymbolId) -> Option<&(SymbolId, Location)> {
        self.note(super::trace::StateKey::PropSource(symbol.clone()));
        self.prop_sources.get(symbol)
    }

    fn is_runtime(&self, symbol: &SymbolId) -> bool {
        self.note(super::trace::StateKey::Runtime(symbol.clone()));
        self.runtime_sources.contains(symbol)
    }

    fn exported(&self, symbol: &SymbolId) -> bool {
        self.note(super::trace::StateKey::Exported(symbol.clone()));
        self.exported.contains_key(symbol)
    }

    /// The JSX elements rendering the function at `span` of `file`, each
    /// element's file recorded.
    fn uses_of<'f>(
        &'f self,
        file: &FileFacts,
        span: solid_facts::core::Span,
    ) -> Vec<(&'f FileFacts, &'f solid_facts::ast::JsxElementFact)> {
        self.note(super::trace::StateKey::Uses(file.path.shared(), span));
        self.uses
            .get(&(file.path.shared(), span))
            .map(|uses| {
                uses.iter()
                    .filter_map(|(key, ordinal)| {
                        let use_file = self.files.get(*self.slots.get(key)?)?;
                        super::trace::read_file(use_file);
                        Some((use_file, use_file.ast.jsx_elements.get(*ordinal as usize)?))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// [`crate::local_access::props_root_attachments_are_views`], recording
    /// which prop sources the declaration has.
    fn attachments_are_views(
        &self,
        file: &FileFacts,
        declaration: &Location,
        symbol: &SymbolId,
        entities: &EntitySymbols,
        lookup: &SemanticLookup<'_>,
    ) -> bool {
        self.note(super::trace::StateKey::Attached(
            Arc::clone(&declaration.path),
            declaration.start_byte,
            declaration.end_byte,
        ));
        crate::local_access::props_root_attachments_are_views(
            file,
            self.prop_sources,
            declaration,
            symbol,
            entities,
            lookup,
        )
    }
}

/// Records that the current computation read the entity rows of
/// `declaration`'s file, when that is not `file`.
fn read_declaration_file(lookup: &SemanticLookup<'_>, file: &FileFacts, declaration: &Location) {
    if *declaration.path != *file.path.as_str() {
        super::trace::read_path(lookup, declaration.path.as_ref());
    }
}

/// One possible component's classification from its callers (pass 10), and
/// what its call sites forward into it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ComponentRow {
    /// The props parameter's pattern, and its name when it has one: both key
    /// the classification.
    pub(super) pattern: Location,
    pub(super) name: Option<Location>,
    pub(super) classification: PropsReactivity,
    pub(super) forwarded: ForwardedProps,
    pub(super) spreads: Vec<SpreadForward>,
}

/// Pass 10's rows of one file: each possible component of one parameter,
/// classified from its JSX call sites.
pub(super) fn component_rows(
    runtime: &crate::RuntimeEnvironment,
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    entities: &EntitySymbols,
    inputs: &ComponentInputs<'_>,
    statuses: Option<&[crate::indexes::ComponentIdentity]>,
    file: &FileFacts,
) -> Vec<ComponentRow> {
    let mut rows = Vec::new();
    for (ordinal, function) in file.ast.functions.iter().enumerate() {
        let Some(parameter) = function.parameters.first() else {
            continue;
        };
        if function.parameters.len() > 1
            || super::post_rows::component_status(lookup, statuses, file, ordinal, function)
                == crate::indexes::ComponentStatus::No
        {
            continue;
        }
        let (classification, forwarded, spreads) =
            classify_one_component(runtime, facts, lookup, entities, inputs, file, function);
        rows.push(ComponentRow {
            pattern: location(file.path.shared(), parameter.pattern),
            name: parameter
                .names
                .first()
                .map(|name| location(file.path.shared(), name.span)),
            classification,
            forwarded,
            spreads,
        });
    }
    rows
}

/// [`PropsReactivityIndex`] from every file's component rows in file order.
pub(super) fn props_reactivity_index<'r>(
    rows: impl IntoIterator<Item = &'r ComponentRow>,
) -> PropsReactivityIndex {
    let mut by_declaration = HashMap::default();
    let mut pending = Vec::new();
    for row in rows {
        pending.push((
            row.pattern.clone(),
            row.forwarded.clone(),
            row.spreads.clone(),
        ));
        if let Some(name) = &row.name {
            pending.push((name.clone(), row.forwarded.clone(), row.spreads.clone()));
        }
        by_declaration.insert(row.pattern.clone(), row.classification.clone());
        if let Some(name) = &row.name {
            by_declaration.insert(name.clone(), row.classification.clone());
        }
    }
    resolve_forwarded_props(&mut by_declaration, &pending);
    PropsReactivityIndex {
        caller_proof: true,
        by_declaration,
    }
}

/// What one component's classification says about the prop `name`.
pub(super) fn classification_use(classification: &PropsReactivity, name: &str) -> PropUse {
    match classification {
        // Witnessed reactive survives the escape; nothing else does.
        PropsReactivity::Escaping { reactive, .. } => {
            if reactive.contains(name) {
                PropUse::Reactive
            } else {
                PropUse::Unknown
            }
        }
        PropsReactivity::Enumerated {
            reactive,
            unresolved,
            ..
        } => {
            if reactive.contains(name) {
                PropUse::Reactive
            } else if unresolved.contains(name) {
                PropUse::Unknown
            } else {
                PropUse::Static
            }
        }
    }
}

/// Settles forwarded props (`<Inner value={props.value} />`): the receiving
/// prop is reactive when the forwarded parent prop is, unresolved when that is
/// unresolved or unknown, and otherwise contributes nothing. This is the least
/// fixpoint from "contributes nothing", so a prop is proven static only when
/// no chain of forwards reaches a reactive or unresolved value. A forward into
/// a component whose own classification has no entry is unknown.
///
/// A spread of the parent's own props (`<Inner {...props} />`) carries only
/// the reactive direction: every parent prop proven reactive becomes reactive
/// here unless the element sets that name after the spread. The receiving
/// component already escapes, so nothing is proven static through it.
pub(super) fn resolve_forwarded_props(
    by_declaration: &mut HashMap<Location, PropsReactivity>,
    pending: &[(Location, ForwardedProps, Vec<SpreadForward>)],
) {
    loop {
        let mut changed = false;
        for (declaration, _, spreads) in pending {
            for (source, overridden) in spreads {
                let Some(
                    PropsReactivity::Enumerated { reactive, .. }
                    | PropsReactivity::Escaping { reactive, .. },
                ) = by_declaration.get(source)
                else {
                    continue;
                };
                let witnessed: Vec<String> = reactive
                    .iter()
                    .filter(|name| !overridden.contains(*name))
                    .cloned()
                    .collect();
                if let Some(
                    PropsReactivity::Enumerated { reactive, .. }
                    | PropsReactivity::Escaping { reactive, .. },
                ) = by_declaration.get_mut(declaration)
                {
                    for name in witnessed {
                        changed |= reactive.insert(name);
                    }
                }
            }
        }
        for (declaration, forwarded, _) in pending {
            for (name, sources) in forwarded {
                let mut verdict = PropUse::Static;
                for (source, prop, exact) in sources {
                    let source_use = match by_declaration
                        .get(source)
                        .map_or(PropUse::Unknown, |classification| {
                            classification_use(classification, prop)
                        }) {
                        // A static head says nothing about the rest of a
                        // longer chain (`props.a.b` with `a` a store or an
                        // object with getters).
                        PropUse::Static if !exact => PropUse::Unknown,
                        other => other,
                    };
                    verdict = match (verdict, source_use) {
                        (PropUse::Reactive, _) | (_, PropUse::Reactive) => PropUse::Reactive,
                        (PropUse::Unknown, _) | (_, PropUse::Unknown) => PropUse::Unknown,
                        _ => PropUse::Static,
                    };
                }
                let Some(classification) = by_declaration.get_mut(declaration) else {
                    continue;
                };
                changed |= match (classification, verdict) {
                    (_, PropUse::Static) => false,
                    (
                        PropsReactivity::Enumerated { reactive, .. }
                        | PropsReactivity::Escaping { reactive, .. },
                        PropUse::Reactive,
                    ) => reactive.insert(name.clone()),
                    (
                        PropsReactivity::Enumerated {
                            unresolved,
                            reactive,
                            ..
                        },
                        PropUse::Unknown,
                    ) => !reactive.contains(name) && unresolved.insert(name.clone()),
                    (PropsReactivity::Escaping { .. }, PropUse::Unknown) => false,
                };
            }
        }
        if !changed {
            break;
        }
    }
}

pub(super) fn classify_one_component(
    runtime: &crate::RuntimeEnvironment,
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    entities: &EntitySymbols,
    inputs: &ComponentInputs<'_>,
    file: &FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> (PropsReactivity, ForwardedProps, Vec<SpreadForward>) {
    use solid_facts::core::Span;
    let mut forwarded = ForwardedProps::new();
    let name = crate::owners::component_binding_name(file, function).or(function.name.as_ref());
    let Some(symbol) = name.and_then(|name| entities.get(&location(file.path.shared(), name.span)))
    else {
        // An anonymous component value (a HOC argument, say) has no symbol to
        // enumerate references through, so there are no call sites to witness
        // either way.
        return (
            PropsReactivity::Escaping {
                reactive: BTreeSet::new(),
                accessor_values: BTreeSet::new(),
                passed: PassedSources::new(),
            },
            forwarded,
            Vec::new(),
        );
    };
    // Escape hatches below only forfeit the *static* half of the proof. Each
    // one sets this flag and keeps scanning, because the JSX call sites that
    // are visible still witness what they pass.
    let mut escapes = false;
    // Exported: callers outside the project can pass anything as well --
    // unless the user has asserted that there is no outside. That assertion
    // removes only the assumption of an *unseen* caller; every reference below
    // must still resolve to a use this analysis understands.
    let closed = runtime.program_is_closed();
    if !closed && inputs.exported(symbol) {
        escapes = true;
    }
    let component_uses = inputs.uses_of(file, function.span);
    // Every reference must be the declaration itself or one of the resolved
    // JSX uses; anything else (passed as a value, aliased, re-exported) hands
    // the component to callers this cannot see. An empty reference list is
    // the absence of the fact, not proof of no references.
    let references = super::trace::references(lookup, symbol.as_str());
    if references.is_empty() {
        escapes = true;
    }
    let name_text = name
        .and_then(|name| file.source_text(name.span))
        .unwrap_or_default();
    for reference in &references {
        let (Ok(start), Ok(end)) = (
            u32::try_from(reference.start_byte),
            u32::try_from(reference.end_byte),
        ) else {
            escapes = true;
            continue;
        };
        let span = Span::new(start, end);
        let own_declaration = *reference.path == *file.path.as_str()
            && (function.span.contains(span) || name.is_some_and(|name| name.span == span));
        if own_declaration {
            continue;
        }
        let jsx_use = component_uses.iter().any(|(use_file, element)| {
            *reference.path == *use_file.path.as_str()
                && (element.name.span == span
                    || (element.span.contains(span)
                        && use_file.source_text(span) == Some(name_text)))
        });
        if !jsx_use {
            // Passed as a value, aliased, or re-exported: this hands the
            // component to callers that cannot be seen. It does not unwrite
            // the JSX call sites that can.
            //
            // An export specifier is the one reference a closed program
            // disposes of: `export { Card }` reaches an importer only if an
            // importer exists, and a closed program says none does outside the
            // files analyzed here -- where every importer's use is itself a
            // reference in this list. Aliasing and passing as a value still
            // escape under a closed program, because the analyzer still does
            // not know what the receiver does with the component.
            if !(closed && {
                super::trace::read_path(lookup, reference.path.as_ref());
                reference_is_export_specifier(facts, span, reference.path.as_ref())
            }) {
                escapes = true;
            }
        }
    }
    let mut reactive = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut accessor_values = BTreeSet::new();
    let mut passed = PassedSources::new();
    let mut spreads = Vec::new();
    for (use_file, element) in component_uses {
        // A spread hands over an object whose properties this cannot
        // enumerate — and, because a later spread wins over an earlier
        // explicit attribute, it can also overwrite one of this element's own
        // attributes with a static value. So this element witnesses nothing
        // in either direction; other elements still do.
        //
        // One shape still witnesses the reactive direction: the last spread
        // is the enclosing component's own props object. rc.13's `merge`
        // resolves each key lazily from the last source that has it, so a
        // name no later attribute or child sets reads the parent's getter.
        if !element.spreads.is_empty() {
            escapes = true;
            if let Some(forward) = spread_of_own_props(lookup, entities, inputs, use_file, element)
            {
                spreads.push(forward);
            }
            continue;
        }
        for attribute in &element.attributes {
            let Some(attribute_name) = use_file.source_text(attribute.local_name) else {
                escapes = true;
                continue;
            };
            let value_use = if attribute.namespace.is_some() {
                PropUse::Unknown
            } else {
                match attribute.value_kind {
                    solid_facts::ast::JsxAttributeValueKind::Boolean
                    | solid_facts::ast::JsxAttributeValueKind::String => PropUse::Static,
                    solid_facts::ast::JsxAttributeValueKind::Element
                    | solid_facts::ast::JsxAttributeValueKind::Fragment => PropUse::Unknown,
                    solid_facts::ast::JsxAttributeValueKind::Expression => {
                        match attribute.expression {
                            Some(expression) => classify_passed_expression(
                                lookup,
                                entities,
                                inputs,
                                use_file,
                                expression,
                                forwarded.entry(attribute_name.to_owned()).or_default(),
                            ),
                            None => PropUse::Unknown,
                        }
                    }
                }
            };
            if attribute.namespace.is_none()
                && attribute.value_kind == solid_facts::ast::JsxAttributeValueKind::Expression
                && attribute.expression.is_some_and(|expression| {
                    passed_expression_is_accessor(lookup, entities, inputs, use_file, expression)
                })
            {
                accessor_values.insert(attribute_name.to_owned());
            }
            let entry = passed.entry(attribute_name.to_owned()).or_default();
            match attribute
                .expression
                .filter(|_| {
                    attribute.namespace.is_none()
                        && attribute.value_kind
                            == solid_facts::ast::JsxAttributeValueKind::Expression
                })
                .and_then(|expression| {
                    passed_source(lookup, entities, inputs, use_file, expression)
                }) {
                Some(source) => {
                    entry.sources.insert(source);
                }
                // A function literal written at the site: the call graph
                // follows a call of the prop into its body (the visible-
                // literal path), so it is not an unknown value.
                None if attribute.expression.is_some_and(|expression| {
                    use_file
                        .ast
                        .function_at(use_file.ast.peel_ts_sugar_span(expression))
                        .is_some()
                }) => {}
                None => entry.other = true,
            }
            if attribute.namespace.is_none()
                && attribute.value_kind == solid_facts::ast::JsxAttributeValueKind::Expression
                && attribute.expression.is_some_and(|expression| {
                    getter_calls_async_source(lookup, entities, inputs, use_file, expression)
                })
            {
                entry.async_getter = true;
            }
            match value_use {
                PropUse::Static => {}
                PropUse::Reactive => {
                    reactive.insert(attribute_name.to_owned());
                }
                PropUse::Unknown => {
                    unresolved.insert(attribute_name.to_owned());
                }
            }
        }
        // Several children compile to an array whose dynamic entries are memo
        // accessors: reading `props.children` reads none of them. Only a
        // single child is the getter's own expression.
        let several = element
            .children
            .iter()
            .filter(|child| {
                use_file
                    .source_text(**child)
                    .is_some_and(|text| !text.trim().is_empty())
            })
            .count()
            > 1;
        for child in &element.children {
            let mut child_forwards = Vec::new();
            let child_use = classify_passed_expression(
                lookup,
                entities,
                inputs,
                use_file,
                *child,
                &mut child_forwards,
            );
            if several {
                if child_use != PropUse::Static || !child_forwards.is_empty() {
                    unresolved.insert("children".to_owned());
                }
                continue;
            }
            forwarded
                .entry("children".to_owned())
                .or_default()
                .extend(child_forwards);
            match child_use {
                PropUse::Static => {}
                PropUse::Reactive => {
                    reactive.insert("children".to_owned());
                }
                PropUse::Unknown => {
                    unresolved.insert("children".to_owned());
                }
            }
        }
    }
    if escapes {
        // `unresolved` is dropped on purpose: without a complete caller set
        // every prop that is not a proven reactive witness is unresolved
        // anyway, so recording the distinction would only invite a reader to
        // treat the complement as static.
        return (
            PropsReactivity::Escaping {
                reactive,
                accessor_values,
                passed,
            },
            forwarded,
            spreads,
        );
    }
    (
        PropsReactivity::Enumerated {
            reactive,
            unresolved,
            accessor_values,
            passed,
        },
        forwarded,
        spreads,
    )
}

/// Per prop name, the parent props a call site forwards into it
/// (`<Inner value={props.value} />` inside `Outer`): the parent's props
/// declaration and the prop name read there.
pub(super) type ForwardedProps = BTreeMap<String, Vec<Forward>>;

/// One forwarded parent prop: the parent's props declaration, the prop name
/// at the head of the forwarded chain, and whether the chain is exactly that
/// one member (`props.a`, not `props.a.b`).
pub(super) type Forward = (Location, String, bool);

/// A call site whose last spread is the enclosing component's own props
/// object (`<Inner {...props} label="x" />` inside `Outer`): the parent's
/// props declaration, and the names the element sets after that spread
/// (attributes, `children`), which the spread cannot supply.
pub(super) type SpreadForward = (Location, BTreeSet<String>);

/// The forward a spread call site makes when its last spread is exactly the
/// props parameter of a component (the props object itself, still current,
/// with nothing but proven views and const aliases attached to its
/// declaration — the same identity `props.x` forwarding requires). Any other spread, or an
/// attribute whose name cannot be read, forwards nothing.
fn spread_of_own_props(
    lookup: &SemanticLookup<'_>,
    entities: &EntitySymbols,
    inputs: &ComponentInputs<'_>,
    file: &FileFacts,
    element: &solid_facts::ast::JsxElementFact,
) -> Option<SpreadForward> {
    let spread = element
        .spreads
        .iter()
        .max_by_key(|spread| spread.span.start)?;
    let argument = file.ast.peel_ts_sugar_span(spread.argument);
    if !file.ast.identifiers.iter().any(|identifier| {
        identifier.span == argument
            && identifier.role == solid_facts::ast::IdentifierRole::Reference
    }) {
        return None;
    }
    let symbol = entities
        .get(&location(file.path.shared(), argument))
        .cloned()
        .or_else(|| super::trace::binding_symbol(lookup, file.path.as_str(), argument))?;
    let (_, declaration) = inputs.prop_source(&symbol)?;
    read_declaration_file(lookup, file, declaration);
    if entities.get(declaration) != Some(&symbol)
        || !props_root_is_current_through_spreads(
            lookup,
            entities,
            file,
            declaration,
            &symbol,
            SPREAD_RECEIVER_DEPTH,
        )
        || !inputs.attachments_are_views(file, declaration, &symbol, entities, lookup)
    {
        return None;
    }
    let mut overridden = BTreeSet::new();
    for attribute in &element.attributes {
        if attribute.span.start > spread.span.start {
            overridden.insert(file.source_text(attribute.local_name)?.to_owned());
        }
    }
    if !element.children.is_empty() {
        overridden.insert("children".to_owned());
    }
    Some((declaration.clone(), overridden))
}

/// How many lone-spread receivers deep the inertness proof follows before it
/// gives up (an unproven chain forwards nothing).
const SPREAD_RECEIVER_DEPTH: usize = 4;

/// Whether a props root still holds the getters its callers wrote, allowing
/// one more kind of whole-object use than [`crate::local_access::props_root_is_current`]:
/// a JSX spread into a component. When a source other than the props is
/// provably never falsy, the compiler's `mergeProps(...)` is a view whose `set`
/// and `deleteProperty` are no-ops on rc.13
/// ([`crate::local_access::component_element_builds_view`]). Otherwise the
/// callee may get the object itself (`createComponent(Child, props)`, or a
/// `mergeProps` of an `undefined` second spread), so the receiver's own props
/// root must be proven inert the same way.
fn props_root_is_current_through_spreads(
    lookup: &SemanticLookup<'_>,
    entities: &EntitySymbols,
    file: &FileFacts,
    declaration: &Location,
    symbol: &SymbolId,
    depth: usize,
) -> bool {
    let Some(uses) = crate::local_access::props_root_unaccounted_uses(
        file,
        declaration,
        symbol,
        entities,
        lookup,
    ) else {
        return false;
    };
    uses.iter().all(|span| {
        let Some(element) = file.ast.jsx_elements.iter().find(|element| {
            element
                .spreads
                .iter()
                .any(|spread| file.ast.peel_ts_sugar_span(spread.argument) == *span)
        }) else {
            return false;
        };
        // Several sources are a `mergeProps` view only when one is provably
        // never falsy; otherwise `merge` can return the spread props object
        // itself (a second spread of an `undefined` identifier), exactly as
        // a lone spread does, and the receiver must be proven inert.
        crate::local_access::component_element_builds_view(file, element)
            || (depth > 0
                && lone_spread_receiver_is_inert(lookup, entities, file, element, depth - 1))
    })
}

/// Whether the component a lone spread hands its props object to can neither
/// write, delete nor alias it beyond further proven-inert spreads.
fn lone_spread_receiver_is_inert(
    lookup: &SemanticLookup<'_>,
    entities: &EntitySymbols,
    file: &FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    depth: usize,
) -> bool {
    let Some((target_file, target)) = super::trace::called_at(lookup, file, element.name.span)
    else {
        return false;
    };
    if target.rest_parameter
        || target_file.ast.identifiers.iter().any(|identifier| {
            target.body.contains(identifier.span)
                && target_file.source_text(identifier.span) == Some("arguments")
        })
    {
        return false;
    }
    match target.parameters.as_slice() {
        [] => true,
        [parameter] if parameter.shape == solid_facts::ast::BindingShape::Identifier => {
            let Some(name) = parameter.names.first() else {
                return false;
            };
            let declaration = location(target_file.path.shared(), name.span);
            let Some(symbol) = entities.get(&declaration) else {
                return false;
            };
            props_root_is_current_through_spreads(
                lookup,
                entities,
                target_file,
                &declaration,
                symbol,
                depth,
            )
        }
        _ => false,
    }
}

fn passed_expression_is_accessor(
    lookup: &SemanticLookup<'_>,
    entities: &EntitySymbols,
    inputs: &ComponentInputs<'_>,
    file: &FileFacts,
    expression: solid_facts::core::Span,
) -> bool {
    let expression = file.ast.peel_ts_sugar_span(expression);
    entities
        .get(&location(file.path.shared(), expression))
        .cloned()
        .or_else(|| super::trace::binding_symbol(lookup, file.path.as_str(), expression))
        .is_some_and(|symbol| inputs.is_accessor(&symbol))
}

/// ADR 0509: whether an attribute's expression calls an async source
/// anywhere outside a nested function (the compiled getter runs it).
fn getter_calls_async_source(
    lookup: &SemanticLookup<'_>,
    entities: &EntitySymbols,
    inputs: &ComponentInputs<'_>,
    file: &FileFacts,
    expression: solid_facts::core::Span,
) -> bool {
    file.ast.calls.iter().any(|call| {
        expression.contains(call.span)
            && !file.ast.functions.iter().any(|function| {
                expression.contains(function.span) && function.span.contains(call.span)
            })
            && {
                let callee = file.ast.peel_ts_sugar_span(call.callee);
                entities
                    .get(&location(file.path.shared(), callee))
                    .cloned()
                    .or_else(|| super::trace::binding_symbol(lookup, file.path.as_str(), callee))
                    .is_some_and(|symbol| inputs.is_async(&symbol))
            }
    })
}

/// ADR 0509: the exact source an attribute's whole value is: an identifier
/// bound to a primitive's accessor, setter or action.
fn passed_source(
    lookup: &SemanticLookup<'_>,
    entities: &EntitySymbols,
    inputs: &ComponentInputs<'_>,
    file: &FileFacts,
    expression: solid_facts::core::Span,
) -> Option<SymbolId> {
    let expression = file.ast.peel_ts_sugar_span(expression);
    if !file.ast.identifiers.iter().any(|identifier| {
        identifier.span == expression
            && identifier.role == solid_facts::ast::IdentifierRole::Reference
    }) {
        return None;
    }
    let symbol = entities
        .get(&location(file.path.shared(), expression))
        .cloned()
        .or_else(|| super::trace::binding_symbol(lookup, file.path.as_str(), expression))?;
    (inputs.is_setter(&symbol) || inputs.is_action(&symbol) || inputs.is_accessor(&symbol))
        .then_some(symbol)
}

/// Whether a reference span is an export specifier's local name in the file it
/// appears in. Only an exact specifier span counts; a re-export with a module
/// clause is a different fact and is not one of these.
pub(super) fn reference_is_export_specifier(
    facts: &ProjectFacts,
    span: solid_facts::core::Span,
    path: &str,
) -> bool {
    facts
        .files
        .iter()
        .filter(|file| file.path.as_str() == path)
        .any(|file| {
            file.ast.exports.iter().any(|export| {
                export.module.is_none()
                    && export
                        .specifiers
                        .iter()
                        .chain(export.declarations.iter())
                        .any(|specifier| specifier.local.span == span)
            })
        })
}

/// Classifies one expression a call site passes for a prop (or renders as a
/// child).
///
/// Reactive proof: a call to a proven accessor, a member chain rooted at a
/// proven store or props object, or a store/props object passed whole — the
/// compiled getter re-evaluates the expression per read, so those subscribe.
/// Static proof: everything in the expression resolves to values that are not
/// reactive (bindings, function values — including nested function literals,
/// whose bodies run later, not in the getter). Anything unresolvable, any
/// call the engine cannot classify, and JSX evaluated inside the getter stay
/// unknown.
fn classify_passed_expression(
    lookup: &SemanticLookup<'_>,
    entities: &EntitySymbols,
    inputs: &ComponentInputs<'_>,
    file: &FileFacts,
    expression: solid_facts::core::Span,
    forwards: &mut Vec<Forward>,
) -> PropUse {
    use solid_facts::core::Span;
    let expression = file.ast.peel_ts_sugar_span(expression);
    // Identifiers that are a member chain's root; the chain decides them.
    let mut chain_roots = Vec::new();
    let nested_functions: Vec<Span> = file
        .ast
        .functions_within(expression)
        .map(|function| function.span)
        .collect();
    // A nested function runs when called, and a JSX element or fragment
    // written in the value runs its own reads in its own tracked effects
    // (`<span>{count()}</span>` passed as `children`): neither is evaluated
    // by the getter the compiler makes of this value. Their reads witness
    // nothing here; the JSX itself makes the value unknown below.
    let nested_jsx: Vec<Span> = file
        .ast
        .jsx_within(expression)
        .map(|element| element.span)
        .chain(
            file.ast
                .jsx_fragments
                .iter()
                .copied()
                .filter(|fragment| expression.contains(*fragment)),
        )
        .collect();
    let inside_nested = |span: Span| {
        nested_functions
            .iter()
            .any(|function| function.contains(span))
    };
    let inside_nested_or_jsx =
        |span: Span| inside_nested(span) || nested_jsx.iter().any(|element| element.contains(span));
    let mut result = PropUse::Static;
    // Member chains rooted at a proven store or props object are reactive;
    // rooted at anything resolvable they are static; unresolvable roots are
    // unknown.
    for member in &file.ast.members {
        if !expression.contains(member.span)
            || inside_nested_or_jsx(member.span)
            // Only chain roots: prefixes repeat the same root.
            || file
                .ast
                .members
                .iter()
                .any(|candidate| expression.contains(candidate.span) && candidate.object == member.span)
        {
            continue;
        }
        let mut root = member.object;
        let mut head = member;
        while let Some(inner) = file.ast.member_at(file.ast.peel_ts_sugar_span(root)) {
            head = inner;
            root = inner.object;
        }
        let root = file.ast.peel_ts_sugar_span(root);
        chain_roots.push(root);
        let symbol = entities
            .get(&location(file.path.shared(), root))
            .cloned()
            // The binder fallback covers plain locals entity facts skip.
            .or_else(|| super::trace::binding_symbol(lookup, file.path.as_str(), root));
        match symbol {
            Some(symbol) if inputs.kind(&symbol) == Some(ReactiveSourceKind::Store) => {
                let key = if file.ast.computed_members.binary_search(&head.span).is_ok() {
                    file.ast
                        .literal_computed_members
                        .iter()
                        .find(|literal| literal.span == head.span)
                        .map(|literal| literal.key.as_ref())
                } else {
                    file.source_text(head.property)
                };
                if inputs.is_runtime(&symbol)
                    && key.is_some_and(|key| lookup.dialect.store_key_warns_strict_read(key))
                {
                    return PropUse::Reactive;
                }
                result = PropUse::Unknown;
            }
            // A parent's prop forwarded is exactly as reactive as that prop is
            // in the parent: decided once every component is classified.
            Some(symbol) if inputs.prop_source(&symbol).is_some() => {
                let (_, declaration) = inputs.prop_sources[&symbol].clone();
                let declaration = &declaration;
                read_declaration_file(lookup, file, declaration);
                // Only the props parameter itself: a view or alias attached
                // to it never certifies a read of its own (its keys are not
                // mapped back to the parent's props), and a destructured
                // binding attached to it still blocks the forward.
                if entities.get(declaration) != Some(&symbol)
                    || !crate::local_access::props_root_is_current(
                        file,
                        declaration,
                        &symbol,
                        entities,
                        lookup,
                    )
                    || !inputs.attachments_are_views(file, declaration, &symbol, entities, lookup)
                {
                    result = PropUse::Unknown;
                    continue;
                }
                let exact = head.span == member.span;
                let name = if file.ast.computed_members.binary_search(&head.span).is_ok() {
                    file.ast
                        .literal_computed_members
                        .iter()
                        .find(|literal| literal.span == head.span)
                        .map(|literal| literal.key.to_string())
                } else {
                    file.source_text(head.property).map(str::to_owned)
                };
                match name {
                    Some(name) => forwards.push((declaration.clone(), name, exact)),
                    None => result = PropUse::Unknown,
                }
            }
            Some(_) => {}
            None => result = PropUse::Unknown,
        }
    }
    for call in file.ast.calls_within(expression) {
        if inside_nested_or_jsx(call.span) {
            continue;
        }
        let callee = entities.get(&location(file.path.shared(), call.callee));
        if callee.is_some_and(|symbol| inputs.is_accessor(symbol) && inputs.is_runtime(symbol)) {
            return PropUse::Reactive;
        }
        // Any other call may read reactive state each time the compiled
        // getter re-evaluates: not provable either way.
        result = PropUse::Unknown;
    }
    if file
        .ast
        .jsx_within(expression)
        .any(|element| !inside_nested(element.span))
        || file
            .ast
            .jsx_fragments
            .iter()
            .any(|fragment| expression.contains(*fragment) && !inside_nested(*fragment))
    {
        result = PropUse::Unknown;
    }
    for identifier in file.ast.identifiers_within(expression) {
        if identifier.role != solid_facts::ast::IdentifierRole::Reference
            || inside_nested_or_jsx(identifier.span)
            || chain_roots.contains(&identifier.span)
        {
            continue;
        }
        let symbol = entities
            .get(&location(file.path.shared(), identifier.span))
            .cloned()
            .or_else(|| super::trace::binding_symbol(lookup, file.path.as_str(), identifier.span));
        match symbol {
            // A store passed whole is a reference: reading the prop hands over
            // the proxy and reads no key. The receiver's own key reads are
            // what is reactive, and this classification does not follow them.
            Some(symbol) if inputs.kind(&symbol) == Some(ReactiveSourceKind::Store) => {
                result = PropUse::Unknown;
            }
            // A props object passed whole forwards every prop; which ones the
            // receiver reads, and whether they are live, is not followed.
            Some(symbol) if inputs.prop_source(&symbol).is_some() => result = PropUse::Unknown,
            Some(_) => {}
            None => result = PropUse::Unknown,
        }
    }
    result
}
