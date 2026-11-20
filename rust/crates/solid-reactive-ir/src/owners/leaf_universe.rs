//! Batch 11, slice 6: the silences of the owner-call universe that rest on the owner graph, a census or the role engine.
//!
//! The universe ([`crate::proof_universe`], table `owner-call`) lists every call of `onCleanup` and of a creation the dialect refuses under a leaf owner
//! (SC3001: CLEANUP_IN_FORBIDDEN_SCOPE, PRIMITIVE_IN_FORBIDDEN_SCOPE) or on an effect apply's stack with no owner set (SC6002: PRIMITIVE_IN_EFFECT_CALLBACK).
//! A site no row, SC9012, discard or named silence ends gets the code's own uncertifiable row ([`crate::site_accounting`]). The silences named here:
//!
//! - [`SiteSilence::OwnerCallNeverRuns`]: ADR 0458's query proves no execution runs the site (every code);
//! - [`SiteSilence::OwnerCallModuleScope`]: no function owns the site and no instance field's initializer holds it, so module evaluation runs it on a
//!   stack of its own: in no leaf owner's callback and on no effect's apply (every code);
//! - [`SiteSilence::LeafNoOpenerInProgram`]: the project names no primitive that opens a leaf scope, so there is no leaf owner to forbid the call
//!   (SC3001's two codes);
//! - [`SiteSilence::LeafOwnerAbsent`]: the context of the function that holds the site has an edge, no leaf bit, and is not the bare open-world seed of an
//!   export (SC3001's two codes);
//! - [`SiteSilence::OwnerCurrent`]: the same context gives an owner on every entry (owned, a leaf owner, or both: no unowned bit), so the effect callback's
//!   guard (`inEffectCallback && context === null`) does not meet the creation (SC6002);
//! - [`SiteSilence::EffectCreationRunsOffApply`]: the role engine places the site off every effect's apply (a handler, a deferred callback, a directive apply,
//!   rendering, module code, discarded code), and every layer around it is entered only the way that role says (SC6002);
//! - [`SiteSilence::EffectCreationOwnerSetAround`]: an exact primitive whose callback creates an owner wraps the site, every layer the climb crosses running in
//!   the primitive's synchronous run (SC6002).
//!
//! The graph's contexts are the entries it holds, and the role engine places a function from where it is written: they answer only where those are every entry
//! the program can make. Each of the graph's silences and the role silence rests on all of these facts of the site (a missing one is no silence, the site keeps its
//! uncertifiable row; the review of #220 found `EffectCreationRunsOffApply` and `EffectCreationOwnerSetAround`, then in `static_api`, asking less than the others):
//!
//! - the site is written directly in a function (not a parameter list, which runs when its argument is omitted; not after a suspension of an async function,
//!   which runs from a later task; not in a generator, or in a layer written inside one, whose body runs where an iterator is stepped: an arrow, an IIFE or a
//!   callback of an in-place primitive in a generator's body is the generator's code, up to the layer whose callback runs under an owner its primitive
//!   creates, which is current there whoever steps what is around it: B1, the re-review of #220);
//! - no layer around it is entered by a route the graph has no edge for ([`super::owner_emission::layer_entry_open_by_reference`], ADR 0868: a function stored,
//!   assigned, handed on or a method, a reference that is not a direct call or an inline run): a function that returns JSX is entered by renders alone only where every reference of its binding is a
//!   JSX tag (F1); a layer that is a generator is an open entry, written around the site or reached as the caller at the end of a call (B1, N1); and a callback
//!   an owner-creating primitive runs is entered by it alone only where nothing references it (N2). The primitive-wrapped silence needs no entry fact of its
//!   own beyond the last: the primitive's own run is the entry;
//! - no package the build does not read may run the function ([`crate::package_hand_off::UnreadPackageHandOff`], ADR 0967): such a package may call it
//!   under a leaf owner, or on an effect's apply, through an entry no edge shows. The census silence rests on this too: a package may open the leaf scope the
//!   project's own primitives do not.
//!
//! Nothing here ever files a row; a silence names the fact it rests on ([`PremiseKind`], each with the span of the code it is about). Where no silence applies for a group of
//! codes the pass writes a typed [`Decline`] to the proof log: the first fact of its order of questions that stops it (a package may run the function, a layer is entered by
//! reference, a leaf bit, an unowned entry, ...). A decline is no proof and ends nothing: it is never read by an ending or a silence, so the site keeps the uncertifiable row of
//! each code, and the gate accepts such a site only if that row stands and a decline at its span lists the code (coordinator-approved, owner delegated).

use super::*;
use crate::proof_log::{Premise, PremiseKind};
use crate::site_accounting::{SiteEnds, SiteSilence};

/// Why an owner-call site has no silence from this pass for a group of its codes: one typed fact, the first the producer's order of questions meets. A decline is
/// no proof and ends nothing (never read by an ending or a silence: the site keeps the uncertifiable row of each code); it goes to the proof log only, where the gate
/// accepts an owner-call site that ends in nothing if and only if the row stands and a decline at its span lists the code. The set is closed: a reason this pass finds is
/// named here or the site is a gate hit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Decline {
    /// A package the build does not read may run the function that holds the site (B3, ADR 0967): it may open a leaf scope around it or call it on an effect's apply.
    UnreadPackageMayRun,
    /// The site is in an instance field's initializer, which runs at each construction, which may be anywhere (no function owns it, and it is no module scope).
    InstanceFieldInitializer,
    /// The site is in a parameter list: it runs when a call omits its argument, which no entry places.
    ParameterList,
    /// The site is in a generator, or in a layer written inside one (up to the layer whose callback runs under an owner its primitive creates): its body runs where an
    /// iterator is stepped, which no owner edge follows.
    GeneratorBody,
    /// The site follows a suspension of its async function: it runs from a later task, which no owner edge follows.
    AfterSuspension,
    /// The owner graph holds no node for the function that holds the site.
    NoOwnerNode,
    /// No owner edge reaches the function (context zero): its entries are not known.
    NoEdgeIntoFunction,
    /// The function is an export whose only entry is the unseen caller of an open world (the bare unowned seed): who calls it, and under what owner, is not known.
    OpenWorldExport,
    /// The context has a leaf bit: some entry runs the site under a leaf owner (a leaf operation's row is what ends the site, and none stands here).
    LeafBitPresent,
    /// The context has an entry that gives no owner (or an owner that is only conventional): the creation may meet `context === null`.
    UnownedEntry,
    /// A layer around the site is entered by a route the graph has no edge for: a function stored, assigned, handed on or a method (ADR 0868).
    EntryByReference,
    /// The site is JSX or a creating call written in a component element's children or a prop's value (a getter, no function the element hands over between): it runs where the
    /// component reads the prop, on whatever stack that is, so nothing the function that holds it says places it (the review of #225, F2).
    GetterRunsWhereRead,
}

impl Decline {
    /// The reason's id in the proof log. One arm per reason, no wildcard: a new reason names its id or the build fails.
    #[deny(clippy::wildcard_enum_match_arm)]
    const fn id(self) -> &'static str {
        match self {
            Self::UnreadPackageMayRun => "unread-package-may-run",
            Self::InstanceFieldInitializer => "instance-field-initializer",
            Self::ParameterList => "parameter-list",
            Self::GeneratorBody => "generator-body",
            Self::AfterSuspension => "after-suspension",
            Self::NoOwnerNode => "no-owner-node",
            Self::NoEdgeIntoFunction => "no-edge-into-function",
            Self::OpenWorldExport => "open-world-export",
            Self::LeafBitPresent => "leaf-bit-present",
            Self::UnownedEntry => "unowned-entry",
            Self::EntryByReference => "entry-by-reference",
            Self::GetterRunsWhereRead => "getter-runs-where-read",
        }
    }
}

/// The group of codes of a site a silence of this pass answers: SC3001's two (a leaf owner forbids the call) or SC6002's one (an effect's apply refuses the creation).
#[derive(Clone, Copy)]
enum Group {
    Leaf,
    Effect,
}

/// Whether a generator is a layer around `span`, the innermost layer first, up to the layer whose callback runs under an owner its primitive creates (the owner is
/// current there whoever steps what is around it): the site's code is then a generator's, which runs where an iterator is stepped. B1, the re-review of #220: the
/// test asked the innermost function only, so a creation in an arrow, an IIFE or an `untrack` callback written inside a generator stepped from an apply was plain, and
/// the role engine, which places a generator's body by the call that makes the iterator, certified it.
fn generator_layer_around(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let mut layers = file.ast.functions_owning(span).collect::<Vec<_>>();
    layers.sort_by_key(|function| function.span.end - function.span.start);
    for function in layers {
        if function.generator {
            return true;
        }
        // The owner is current there whoever steps what is around it only where nothing but the primitive enters the callback (N2: a named function expression
        // that saved itself is run by whoever calls it).
        if super::owner_emission::creates_owner_for(file, function.span, lookup)
            && super::owner_emission::referenced_only_by_entry(file, function, lookup)
        {
            return false;
        }
    }
    false
}

/// Records the silences of every owner-call site of the project's files in `ends`, and the declines (proof log only) of the groups of codes none answers. `client_build` is
/// the build SC6002's creation sites are asked on ([`crate::static_api::client_build`]).
#[allow(clippy::too_many_arguments)]
pub(super) fn silences(
    files: &[&solid_facts::FileFacts],
    order: &[PathId],
    graph: &OwnerGraph,
    lookup: &SemanticLookup<'_>,
    entities: &crate::EntitySymbols,
    symbol_names: &crate::HashMap<crate::SymbolId, crate::SymbolId>,
    client_build: bool,
    ends: &mut SiteEnds,
) {
    use crate::proof_universe::{SiteCode, owner_call_codes};
    let dialect = lookup.dialect;
    let reach = crate::execution_role::SiteReach::new(lookup);
    // The files as the lookup holds them (the package hand-off question is asked of those).
    let by_path: std::collections::HashMap<&str, &solid_facts::FileFacts> = lookup
        .files()
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    // ADR 1147: the owner codes' question: a child of an exact context provider runs under the provider's children memo, wherever it is read.
    let unread = crate::package_hand_off::UnreadPackageHandOff::for_owner_questions(lookup, &by_path);
    let no_leaf_opener = !lookup.project_opens_leaf_scope();
    let logging = crate::proof_log::enabled();
    // ADR 1147: under the proof log only, the question without the owner arm, to name the arm where a site ends that it alone let through (monotone.mjs M2b).
    let plain = logging.then(|| crate::package_hand_off::UnreadPackageHandOff::new(lookup, &by_path));
    // ADR 1031: the path id of each listed file, for a getter answered at reads written in another file.
    let pids: std::collections::HashMap<&str, PathId> = files
        .iter()
        .zip(order)
        .map(|(listed, pid)| (listed.path.as_str(), *pid))
        .collect();
    let question = Question {
        graph,
        reach: &reach,
        unread: &unread,
        no_leaf_opener,
        lookup,
        entities,
        symbol_names,
        pids: &pids,
    };
    // The callback positions the role engine takes as placed, per file, for the role question (computed at the first creation that asks it).
    let mut allowed_by_path: std::collections::HashMap<String, Option<Vec<Span>>> =
        std::collections::HashMap::new();
    for (listed, pid) in files.iter().zip(order) {
        if lookup.in_dependency_source(listed.path.as_str()) {
            continue;
        }
        let Some(file) = by_path.get(listed.path.as_str()).copied() else {
            continue;
        };
        let ast = &file.ast;
        // The sites of the file this pass answers for, as (the span the role, the entry and the graph are asked about, the span the site is reported at, its codes): the
        // calls of the owner-call universe, and (B2, the re-review of #220) its JSX creation sites.
        let mut candidates: Vec<(Span, Span, &'static [SiteCode], bool)> = Vec::new();
        for call in &ast.calls {
            let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
                continue;
            };
            let codes = owner_call_codes(dialect, file, call, primitive, client_build);
            if !codes.is_empty() {
                // A creating call written as a component element's getter runs where the component reads the prop, as a JSX creation does (the review of #225, F2).
                let getter = crate::owners::written_in_getter(file, lookup, call.span);
                candidates.push((call.span, call.callee, codes, getter));
            }
        }
        let jsx_codes = crate::proof_universe::jsx_site_codes(client_build);
        for jsx in crate::owners::universe_jsx_sites(file, lookup) {
            candidates.push((jsx.span, jsx.span, jsx_codes, jsx.getter));
        }
        for (span, site, codes, getter) in candidates {
            let leaf = codes.iter().any(|code| {
                matches!(
                    code,
                    SiteCode::CleanupForbidden | SiteCode::PrimitiveForbidden
                )
            });
            let effect = codes.contains(&SiteCode::EffectCreation);
            // What a group of codes is declined for: written to the proof log, and (ADR 1065, B1a) to the blocker census with the first hand-off leaf.
            let decline = |group: Group, reason: Decline| {
                crate::blocker_census::record(|| {
                    let location = site.location(file.path.shared());
                    let group_name = match group {
                        Group::Leaf => "leaf",
                        Group::Effect => "effect",
                    };
                    let mut blockers = vec![format!("cause|creation|{group_name}|{}", reason.id())];
                    if matches!(reason, Decline::UnreadPackageMayRun)
                        && let Some(leaf) = unread.may_run_leaf(file, span)
                    {
                        blockers.push(format!("handoff|{leaf}"));
                    }
                    crate::blocker_census::CensusRow::at(&location, "owner-call", blockers)
                });
                if !logging {
                    return;
                }
                let names = codes
                    .iter()
                    .filter(|code| match group {
                        Group::Leaf => !matches!(code, SiteCode::EffectCreation),
                        Group::Effect => matches!(code, SiteCode::EffectCreation),
                    })
                    .map(|code| code.name())
                    .collect::<Vec<_>>();
                crate::proof_log::declined(
                    "owner-call",
                    &site.location(file.path.shared()),
                    reason.id(),
                    &names,
                );
            };
            // No execution runs the site: no code of it can be raised.
            if matches!(
                reach.site(file, span),
                crate::execution_role::Reach::Unreached(_)
            ) {
                ends.end_at(
                    file,
                    site,
                    SiteSilence::OwnerCallNeverRuns,
                    PremiseKind::SiteReach,
                    || Some(span),
                );
                continue;
            }
            // The spans the hand-off is asked at: the reads of a getter, or the site itself.
            let mut asked_at: Vec<(&solid_facts::FileFacts, Span)> = vec![(file, span)];
            let answers = if getter {
                // A getter's JSX or creating call runs where the component reads the prop (the review of #225, F2): the function that holds it, module evaluation and
                // every entry of that function say nothing of that stack. ADR 1031: where every read of the prop in the project component the element renders is on the
                // component's own stack under its render's owner, the getter runs at those reads, so each group is answered at each read as for a site written there,
                // and ends only where every read that may run ends it.
                match crate::owners::getter_read_sites(file, lookup, span) {
                    Some(reads) => {
                        asked_at = reads.clone();
                        answer_at_reads(&question, &reads, leaf, effect, &mut allowed_by_path)
                    }
                    None => Answers {
                        leaf: leaf.then_some(Err(Decline::GetterRunsWhereRead)),
                        effect: effect.then_some(Err(Decline::GetterRunsWhereRead)),
                    },
                }
            } else {
                let allowed = allowed_by_path.entry(file.path.to_string()).or_default();
                let (leaf_answer, effect_answer) =
                    question.answer(file, *pid, span, site, leaf, effect, allowed);
                let own = |answer: Option<Outcome>| {
                    answer.map(|outcome| {
                        outcome
                            .map(|(silence, kind, at)| (silence, vec![Premise::at(kind, Some(at))]))
                    })
                };
                Answers {
                    leaf: own(leaf_answer),
                    effect: own(effect_answer),
                }
            };
            for (answer, group) in [(answers.leaf, Group::Leaf), (answers.effect, Group::Effect)] {
                match answer {
                    None => {}
                    Some(Err(reason)) => decline(group, reason),
                    Some(Ok((silence, premises))) => {
                        // ADR 1147: the arm names itself where the question without it declines (a package may run the code).
                        if let Some(plain) = &plain
                            && asked_at.iter().any(|(at_file, at)| plain.may_run(at_file, *at))
                        {
                            crate::proof_log::decision(
                                "owner-call",
                                file.path.as_str(),
                                site,
                                crate::ExecutionRole::Unknown,
                                Some((crate::callback_return::CONTEXT_PROVIDER_CHILDREN_OWNED, false)),
                                &[crate::callback_return::CONTEXT_PROVIDER_CHILDREN_OWNED],
                                Some(false),
                            );
                        }
                        ends.end_with(file, site, silence, || premises);
                    }
                }
            }
        }
    }
}

/// What one question of a group of codes comes to: a silence with the premise it rests on (its kind and the span of the code it is about), or a decline.
type Outcome = Result<(SiteSilence, PremiseKind, Span), Decline>;

/// The answer of each group of codes of a site, with the premises an end rests on.
#[derive(Default)]
struct Answers {
    leaf: Option<Result<(SiteSilence, Vec<Premise>), Decline>>,
    effect: Option<Result<(SiteSilence, Vec<Premise>), Decline>>,
}

/// What every question of the pass is asked against, fixed for the program.
struct Question<'q, 'l, 'a> {
    graph: &'q OwnerGraph,
    reach: &'q crate::execution_role::SiteReach<'l, 'a>,
    unread: &'q crate::package_hand_off::UnreadPackageHandOff<'q, 'a>,
    no_leaf_opener: bool,
    lookup: &'l SemanticLookup<'a>,
    entities: &'q crate::EntitySymbols,
    symbol_names: &'q crate::HashMap<crate::SymbolId, crate::SymbolId>,
    pids: &'q std::collections::HashMap<&'q str, PathId>,
}

impl<'a> Question<'_, '_, 'a> {
    /// The answer of each open group (`leaf`, `effect`) for code written at `span` of `file` (reported at `site`), the site ADR 0458's query does not prove
    /// unreached: the questions the pass asks of every site, in its order (module scope, an unread package, the census, a plain site, the role, the graph).
    #[allow(clippy::too_many_arguments)]
    fn answer(
        &self,
        file: &'a solid_facts::FileFacts,
        pid: PathId,
        span: Span,
        site: Span,
        leaf: bool,
        effect: bool,
        allowed: &mut Option<Vec<Span>>,
    ) -> (Option<Outcome>, Option<Outcome>) {
        let both = |reason: Decline| (leaf.then_some(Err(reason)), effect.then_some(Err(reason)));
        let ast = &file.ast;
        // Module evaluation: one run, on a stack of its own (an instance field's initializer runs at each construction, which may be anywhere: written directly in one it is
        // no part of the function the class is written in, which says nothing of the stack of a construction, the review of #225, B3).
        let in_field = crate::local_access::in_instance_field_initializer_directly(file, span);
        let Some(function) = ast.innermost_function_owning(span).filter(|_| !in_field) else {
            if in_field {
                return both(Decline::InstanceFieldInitializer);
            }
            let end = Ok((
                SiteSilence::OwnerCallModuleScope,
                PremiseKind::ModuleScope,
                site,
            ));
            return (leaf.then_some(end), effect.then_some(end));
        };
        // A package the build does not read may run the function (B3): no census and no entry the graph holds answers for it.
        if self.unread.may_run(file, span) {
            return both(Decline::UnreadPackageMayRun);
        }
        // The groups of codes still open: the census answers SC3001's pair for a program with no leaf opener.
        let mut leaf_answer = None;
        let mut open_leaf = leaf;
        if leaf && self.no_leaf_opener {
            leaf_answer = Some(Ok((
                SiteSilence::LeafNoOpenerInProgram,
                PremiseKind::ProgramCensus,
                site,
            )));
            open_leaf = false;
        }
        let mut effect_answer = None;
        let mut open_effect = effect;
        if !open_leaf && !open_effect {
            return (leaf_answer, effect_answer);
        }
        // Every answer below speaks for the site only where it is plain: the graph's context and the role engine's placement say where code written
        // directly in a function runs, not a parameter list, a generator's body or a later task.
        let plain = if crate::owners::in_parameter_list(ast, span) {
            Err(Decline::ParameterList)
        } else if generator_layer_around(file, span, self.lookup) {
            Err(Decline::GeneratorBody)
        } else if crate::execution_role::may_follow_suspension(file, span) {
            Err(Decline::AfterSuspension)
        } else {
            Ok(())
        };
        if let Err(reason) = plain {
            return (
                leaf_answer.or(open_leaf.then_some(Err(reason))),
                open_effect.then_some(Err(reason)),
            );
        }
        // Whether a layer around the site is entered by a route the graph has no edge for, asked once and without the component-render premise
        // (ADR 0868, and the ruling: a package that renders a component is no more trusted than one that calls it).
        let caller_unplaced = |caller_file: &solid_facts::FileFacts, at: Span| {
            super::owner_emission::caller_holds_no_context(
                self.graph,
                self.reach,
                self.lookup,
                caller_file,
                at,
            )
        };
        let mut entry_open_known: Option<bool> = None;
        let mut entry_open = || {
            *entry_open_known.get_or_insert_with(|| {
                super::owner_emission::layer_entry_open_by_reference(
                    file,
                    span,
                    self.lookup,
                    caller_unplaced,
                )
            })
        };
        // SC6002's two silences that rest on no graph context (they were `static_api`'s until the review of #220, which found each asking less than the
        // others: F2, F3). The role engine places the site off every effect's apply (a handler, a deferred callback, a directive apply, rendering, module
        // code, discarded code), and every layer around the site is entered only the way that role says; or an exact primitive's callback that creates
        // an owner wraps it.
        if open_effect {
            let allowed = allowed.get_or_insert_with(|| {
                crate::execution_role::allowed_callback_spans(file, self.lookup)
            });
            if crate::static_api::runs_off_every_apply(
                file,
                span,
                allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            ) && !entry_open()
            {
                effect_answer = Some(Ok((
                    SiteSilence::EffectCreationRunsOffApply,
                    PremiseKind::RoleDoesNotReport,
                    site,
                )));
                open_effect = false;
            } else if let Some(wrapper) =
                crate::static_api::owner_created_around_call(file, span, self.lookup)
            {
                effect_answer = Some(Ok((
                    SiteSilence::EffectCreationOwnerSetAround,
                    PremiseKind::OwnerSetAround,
                    wrapper,
                )));
                open_effect = false;
            }
        }
        if !open_leaf && !open_effect {
            return (leaf_answer, effect_answer);
        }
        let Some((node, context)) = self.graph.node_context(pid, function.span) else {
            return (
                leaf_answer.or(open_leaf.then_some(Err(Decline::NoOwnerNode))),
                effect_answer.or(open_effect.then_some(Err(Decline::NoOwnerNode))),
            );
        };
        // Each group asks the context its own question.
        let leaf_context = if !open_leaf {
            None
        } else if context == 0 {
            Some(Err(Decline::NoEdgeIntoFunction))
        } else if context & (OWNER_CONTEXT_LEAF | OWNER_CONTEXT_LEAF_OWNED) != 0 {
            Some(Err(Decline::LeafBitPresent))
        } else if open_world_unowned_only(context, node) {
            Some(Err(Decline::OpenWorldExport))
        } else {
            Some(Ok(()))
        };
        let effect_context = if !open_effect {
            None
        } else if context == 0 {
            Some(Err(Decline::NoEdgeIntoFunction))
        } else if context & !(OWNER_CONTEXT_OWNED | OWNER_CONTEXT_LEAF | OWNER_CONTEXT_LEAF_OWNED)
            != 0
        {
            Some(Err(Decline::UnownedEntry))
        } else {
            Some(Ok(()))
        };
        // The context says no entry is the forbidden one; it is every entry only where no layer around the site is entered by a route the graph has no
        // edge for.
        let entry_is_open = (matches!(leaf_context, Some(Ok(())))
            || matches!(effect_context, Some(Ok(()))))
            && entry_open();
        let close =
            |answer: Option<Result<(), Decline>>, silence: SiteSilence, kind: PremiseKind| {
                answer.map(|answer| match answer {
                    Err(reason) => Err(reason),
                    Ok(()) if entry_is_open => Err(Decline::EntryByReference),
                    Ok(()) => Ok((silence, kind, function.span)),
                })
            };
        (
            leaf_answer.or(close(
                leaf_context,
                SiteSilence::LeafOwnerAbsent,
                PremiseKind::OwnerContextNoLeaf,
            )),
            effect_answer.or(close(
                effect_context,
                SiteSilence::OwnerCurrent,
                PremiseKind::OwnerContextCurrent,
            )),
        )
    }
}

/// ADR 1031: the answer of each open group for a getter that runs at `reads` (the prop's reads in the component the element renders, each on the component's own
/// stack under its render's owner): each group is asked at each read as for code written there, and ends only where every read that may run ends it, on the premises
/// of all of them (each in the file it is about). A read ADR 0458's query proves unreached runs nothing; a getter no read runs keeps `getter-runs-where-read` (no
/// placement for code that never runs, ADR 1021's ruling).
fn answer_at_reads<'a>(
    question: &Question<'_, '_, 'a>,
    reads: &[(&'a solid_facts::FileFacts, Span)],
    leaf: bool,
    effect: bool,
    allowed_by_path: &mut std::collections::HashMap<String, Option<Vec<Span>>>,
) -> Answers {
    let unplaced = || Answers {
        leaf: leaf.then_some(Err(Decline::GetterRunsWhereRead)),
        effect: effect.then_some(Err(Decline::GetterRunsWhereRead)),
    };
    let mut leaf_outcomes = Vec::new();
    let mut effect_outcomes = Vec::new();
    for (read_file, read) in reads {
        if matches!(
            question.reach.site(read_file, *read),
            crate::execution_role::Reach::Unreached(_)
        ) {
            continue;
        }
        let Some(pid) = question.pids.get(read_file.path.as_str()).copied() else {
            return unplaced();
        };
        let allowed = allowed_by_path
            .entry(read_file.path.to_string())
            .or_default();
        let (leaf_answer, effect_answer) =
            question.answer(read_file, pid, *read, *read, leaf, effect, allowed);
        let path = read_file.path.shared();
        leaf_outcomes.extend(leaf_answer.map(|outcome| (outcome, path.clone())));
        effect_outcomes.extend(effect_answer.map(|outcome| (outcome, path.clone())));
    }
    if leaf && leaf_outcomes.is_empty() || effect && effect_outcomes.is_empty() {
        return unplaced();
    }
    // A decline at one read declines the group (the first reason met); every read's silence ends it, on the premises of all of them.
    let fold = |outcomes: Vec<(Outcome, std::sync::Arc<str>)>| {
        let mut premises = Vec::new();
        let mut silence = None;
        for (outcome, path) in outcomes {
            let (each, kind, at) = outcome?;
            silence.get_or_insert(each);
            premises.push(Premise::in_file(kind, at, path));
        }
        silence
            .map(|silence| (silence, premises))
            .ok_or(Decline::GetterRunsWhereRead)
    };
    Answers {
        leaf: leaf.then(|| fold(leaf_outcomes)),
        effect: effect.then(|| fold(effect_outcomes)),
    }
}
