//! ADR 0838: every pending read is accounted for. A read of a source not
//! proved synchronous may meet it pending, and each pending rule that could
//! speak there (SC5001's untracked or leaf read, SC5003's missing boundary)
//! either has a row of its own at the read, an SC9012 row at the exact read,
//! or a recorded [`PendingSafe`] reason that rules that rule out. Anything
//! else gets an uncertifiable row, blocker `async-read-unaccounted`.
//!
//! The check runs once, after every producer (local access, the
//! interprocedural summaries, transport, the synthesized and opened rows,
//! upstream) and before projection. It only adds rows, and only for a rule no
//! row decides at the read, so it cannot displace a violation.

use std::collections::HashSet;

use solid_facts::core::Span;
use typefacts::Location;

use crate::pipeline::{AnalysisContext, ProgramDraft};
use crate::{AsyncRead, ExecutionRole};

/// The pending rules a read of a pending source may raise. One rule's row
/// never accounts for the other's claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) enum PendingRule {
    /// SC5001: `PENDING_ASYNC_UNTRACKED_READ` (a strict-read window) and
    /// `PENDING_ASYNC_FORBIDDEN_SCOPE` (a leaf owner).
    Unsuspendable,
    /// SC5003: `ASYNC_OUTSIDE_LOADING_BOUNDARY` (a render effect meets the
    /// pending value with no `Loading` above it).
    Boundary,
    /// SC5008: `CAUGHT_REACTIVE_READ` (a computation's pass catches the read's
    /// not-ready throw and completes). Claimed only at a read inside a `try`
    /// whose handler does not only throw.
    Caught,
}

impl PendingRule {
    const ALL: [Self; 3] = [Self::Unsuspendable, Self::Boundary, Self::Caught];

    const fn code(self) -> &'static str {
        match self {
            Self::Unsuspendable => "SC5001",
            Self::Boundary => "SC5003",
            Self::Caught => "SC5008",
        }
    }
}

/// A recorded reason a pending rule cannot raise at a read. Closed: each
/// variant rules out exactly the rules [`Self::rules_out`] names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingSafe {
    /// The compiler deleted the read: it never runs.
    Discarded,
    /// `isPending(() => a())` / `latest(...)`: no untracked-read throw; the
    /// first load still throws to the boundary around it (review R18).
    PendingProbe,
    /// The read's call-attributed walk enumerated every position that runs
    /// it and each is decided (ADR 0838): a fresh stack, a discarded region, a
    /// served read, a covered probe. SC5008 stays with the read's own `try`.
    WalkDecided,
    /// A declared `loadingValue` served in the pass that created the source
    /// (ADR 0632): no pending value reaches the read.
    DeclaredServed,
    /// A tracked read (rendered JSX, a computation's own callback) suspends
    /// instead of throwing.
    SuspendsInComputation,
    /// No strict-read window and no leaf owner (ADR 0443's unwindowed map):
    /// the coded untracked-read throw needs `strictReadActive()`.
    OutsideStrictWindow,
    /// A fresh stack: a reviewed scheduler's callback (ADR 0435), or a host
    /// event delivery while no synchronous dispatch can deliver it inside a
    /// window or a pass (ADR 0356, 0437). No window, no render effect, no
    /// computation's pass.
    FreshStack,
    /// Not a render effect's read: `ASYNC_OUTSIDE_LOADING_BOUNDARY` is raised
    /// from a render effect only (`EFFECT_RENDER` in the pinned dev build).
    NotRenderEffect,
    /// A `Loading` boundary covers the read.
    UnderLoading,
    /// The source declares its first paint: it needs no boundary.
    DeclaredFirstPaint,
    /// The `try` runs where no computation's pass holds the throw: module
    /// evaluation, an event handler, an effect apply, or the body of a
    /// component only ever mounted as a tag (a body's read throws its own
    /// coded error, ADR 0640). `discover_caught_reads`' own exits.
    NoComputationPass,
}

impl PendingSafe {
    pub(crate) const fn rules_out(self) -> &'static [PendingRule] {
        match self {
            Self::Discarded | Self::DeclaredServed | Self::FreshStack => &PendingRule::ALL,
            // Review R18: a first load throws from the probe to the boundary
            // around it (rc.21 dev.js, `latest`/`isPending`), and SC5008's own
            // census does not exempt a probe.
            Self::PendingProbe | Self::SuspendsInComputation | Self::OutsideStrictWindow => {
                &[PendingRule::Unsuspendable]
            }
            Self::NotRenderEffect | Self::UnderLoading | Self::DeclaredFirstPaint => {
                &[PendingRule::Boundary]
            }
            Self::WalkDecided => &[PendingRule::Unsuspendable, PendingRule::Boundary],
            Self::NoComputationPass => &[PendingRule::Caught],
        }
    }
}

/// The rule a reported row projects to (`async_read_wording`'s order).
pub(crate) fn reported_rule(row: &AsyncRead) -> Option<PendingRule> {
    if !crate::projection::async_read_reported(row) {
        return None;
    }
    if row.apply_first_run_pending
        || row.invocation_context_unproven
        || row.callee_callback_timing
        || row.leaf_owner.is_some()
    {
        return Some(PendingRule::Unsuspendable);
    }
    match row.execution {
        ExecutionRole::ModuleInitialization | ExecutionRole::UntrackedRendering => {
            Some(PendingRule::Unsuspendable)
        }
        ExecutionRole::TrackedJsx | ExecutionRole::UntrackedUnwindowed => {
            Some(PendingRule::Boundary)
        }
        ExecutionRole::Unknown
        | ExecutionRole::Unreported
        | ExecutionRole::DeferredCallback
        | ExecutionRole::UntrackedCallback
        | ExecutionRole::EffectApply
        | ExecutionRole::EventCallback
        | ExecutionRole::DirectiveApply
        | ExecutionRole::DiscardedRendering => None,
    }
}

/// The reasons a row's decided role records for the rules it does not report.
/// A role is a reason only where a source says so (ADR 0838's table); any
/// other role leaves its rules to a row of their own.
fn row_reasons(lookup: &crate::indexes::SemanticLookup<'_>, row: &AsyncRead) -> Vec<PendingSafe> {
    let deferred = lookup
        .file_by_path(row.location.path.as_ref())
        .zip(u32::try_from(row.location.start_byte).ok())
        .zip(u32::try_from(row.location.end_byte).ok())
        .is_some_and(|((file, start), end)| {
            crate::execution_role::fresh_stack_callback_call(file, Span::new(start, end), lookup)
                .is_some()
        });
    let mut reasons = role_reasons(lookup, row.execution, row.leaf_owner.is_some(), deferred);
    // R2h: a body's role is a reason only for a body that runs as one. A
    // function whose component identity is uncertain (it returns JSX and is
    // also entered another way, `[1].forEach(K1)`) may run in its caller's
    // render effect (Chrome rc.21: ASYNC_OUTSIDE_LOADING_BOUNDARY).
    if row.execution == ExecutionRole::UntrackedRendering
        && component_identity_uncertain(lookup, row)
    {
        reasons.retain(|reason| *reason != PendingSafe::NotRenderEffect);
    }
    if row.under_loading {
        reasons.push(PendingSafe::UnderLoading);
    }
    if row.declared_loading {
        reasons.push(PendingSafe::DeclaredFirstPaint);
    }
    reasons
}

/// Whether a function around the row's position may be a component and is
/// not proven one (`ComponentStatus::Uncertain`), or would be one but for a
/// reference that enters it another way (an escaped component, analyzed as
/// one with its body's violations withdrawn to obligations).
fn component_identity_uncertain(
    lookup: &crate::indexes::SemanticLookup<'_>,
    row: &AsyncRead,
) -> bool {
    let Some(((file, start), end)) = lookup
        .file_by_path(row.location.path.as_ref())
        .zip(u32::try_from(row.location.start_byte).ok())
        .zip(u32::try_from(row.location.end_byte).ok())
    else {
        return true;
    };
    let span = Span::new(start, end);
    let escaped = lookup.escaped_component_functions();
    file.ast.functions.iter().any(|function| {
        function.span.contains(span)
            && (lookup.function_component_status(file, function)
                == crate::indexes::ComponentStatus::Uncertain
                || escaped.contains(&(file.path.as_str(), function.span)))
    })
}

/// What a decided role rules out, and why:
/// - `DiscardedRendering`: the compiler deleted the read.
/// - `TrackedJsx`: a tracked read suspends (SC5001 out).
/// - `DeferredCallback` where a fact proves the deferral (`deferred`: the
///   position is a callback handed to a reviewed fresh-stack scheduler, ADR
///   0435, `fresh_stack_callback_call`): no strict-read window, no render
///   effect, no computation's pass (all three out). The role alone is no
///   proof: control flow gives it to functions run in place (ADR 0834, 0836;
///   review D2).
/// - `EventCallback`, while no synchronous dispatch can deliver it inside a
///   window or an owned pass (ADR 0356, 0435, 0437's veto): a host event
///   delivery on an empty stack (all three out).
/// - `UntrackedUnwindowed`: no strict-read window (ADR 0443; SC5001 out).
/// - `ModuleInitialization`, `UntrackedRendering`: no render effect (a body's
///   pending read throws its own coded error, dev-shared.js
///   `throwPendingUntrackedRead`); `EffectApply`: an apply is no render (ADR
///   0614). SC5003 out.
///
/// `UntrackedCallback` (an untracked read still suspends its computation),
/// `Unreported` (a write-classification answer: a setter's draft runs in the
/// caller's window, review D2), `DirectiveApply` (a ref callback runs while
/// the element is created; not probed) and `Unknown` rule nothing out.
pub(crate) fn role_reasons(
    lookup: &crate::indexes::SemanticLookup<'_>,
    role: ExecutionRole,
    leaf_owner: bool,
    deferred: bool,
) -> Vec<PendingSafe> {
    match role {
        ExecutionRole::DiscardedRendering => vec![PendingSafe::Discarded],
        ExecutionRole::TrackedJsx => vec![PendingSafe::SuspendsInComputation],
        ExecutionRole::DeferredCallback if !leaf_owner && deferred => {
            vec![PendingSafe::FreshStack]
        }
        ExecutionRole::EventCallback
            if !leaf_owner
                && !lookup.synchronous_event_dispatch_present()
                && !lookup.synchronous_event_read_dispatch_present() =>
        {
            vec![PendingSafe::FreshStack]
        }
        ExecutionRole::UntrackedUnwindowed if !leaf_owner => {
            vec![PendingSafe::OutsideStrictWindow]
        }
        ExecutionRole::ModuleInitialization
        | ExecutionRole::UntrackedRendering
        | ExecutionRole::EffectApply => vec![PendingSafe::NotRenderEffect],
        _ => Vec::new(),
    }
}

/// Whether a path whose position took `role` is decided for every pending
/// rule, so a climb from it that places nothing is no cut.
pub(crate) fn role_decides_path(
    lookup: &crate::indexes::SemanticLookup<'_>,
    role: ExecutionRole,
    deferred: bool,
) -> bool {
    let reasons = role_reasons(lookup, role, false, deferred);
    PendingRule::ALL.iter().all(|rule| {
        reasons
            .iter()
            .any(|reason| reason.rules_out().contains(rule))
    })
}

/// ADR 0926: where one census start's walks met its source's value at a row,
/// or left it open (`value_use.rs` [`crate::value_use::StartAnswer`]): a call
/// of the value, a use the walk ended at, or the start itself.
pub(crate) struct PendingReach {
    /// The source, `None` for a creating call no binding holds.
    pub(crate) symbol: Option<crate::SymbolId>,
    /// The census walked the source as one that may be pending.
    pub(crate) async_walk: bool,
    pub(crate) accessor: std::sync::Arc<str>,
    pub(crate) declaration: Location,
    pub(crate) at: Vec<Location>,
}

/// One read of a pending source.
struct Site {
    location: Location,
    /// The callee or member object span the read is made through: an SC9012
    /// row there is at the read.
    through: Option<Location>,
    accessor: std::sync::Arc<str>,
    declaration: Location,
}

pub(crate) fn account_pending_reads(
    ctx: &AnalysisContext<'_>,
    pending: &dyn Fn(&str) -> bool,
    declared: &(dyn Fn(&str) -> bool + Sync),
    walk_decided: &HashSet<Location>,
    reach: &[PendingReach],
    draft: &mut ProgramDraft,
) {
    // Review R28: the pass runs inline in `collect_project`, which every
    // build recomputes; its raw file reads record nothing. Memoizing it
    // needs recorded reads first.
    debug_assert!(!crate::base::reads::recording());
    let lookup = ctx.semantic_lookup;
    let mut sites = Vec::<Site>::new();
    let mut seen = HashSet::<Location>::new();
    let mut add = |site: Site, sites: &mut Vec<Site>| {
        if seen.insert(site.location.clone()) {
            sites.push(site);
        }
    };
    let span_of = |location: &Location| {
        Some(Span::new(
            u32::try_from(location.start_byte).ok()?,
            u32::try_from(location.end_byte).ok()?,
        ))
    };
    // (1) Every direct call of a pending accessor, every member Get of a
    // pending store root.
    let mut symbols = ctx
        .accessors
        .iter()
        .filter(|(symbol, _)| pending(symbol.as_str()))
        .collect::<Vec<_>>();
    symbols.sort_unstable_by(|left, right| left.0.cmp(right.0));
    for (symbol, (name, declaration)) in symbols {
        let store = ctx.source_kinds.get(symbol) == Some(&crate::ReactiveSourceKind::Store);
        for reference in lookup.symbol_references(symbol.as_str()) {
            let Some(file) = lookup.file_by_path(reference.path.as_ref()) else {
                continue;
            };
            let Some(span) = span_of(&reference) else {
                continue;
            };
            if store {
                let mut as_object = false;
                for member in file
                    .ast
                    .members
                    .iter()
                    .filter(|member| file.ast.peel_ts_sugar_span(member.object) == span)
                {
                    as_object = true;
                    add(
                        Site {
                            location: member.span.location(file.path.as_str()),
                            through: Some(reference.clone()),
                            accessor: format!("{name}.").into(),
                            declaration: declaration.clone(),
                        },
                        &mut sites,
                    );
                }
                // R2h: a root handed on whole (`Object.keys(s1)`, a spread, an
                // argument) is read where it is handed: the reference is a site.
                if !as_object
                    && reference != *declaration
                    && file.ast.identifiers.iter().any(|identifier| {
                        identifier.span == span
                            && identifier.role == solid_facts::ast::IdentifierRole::Reference
                    })
                {
                    add(
                        Site {
                            location: reference.clone(),
                            through: Some(reference.clone()),
                            accessor: name.to_string().into(),
                            declaration: declaration.clone(),
                        },
                        &mut sites,
                    );
                }
                continue;
            }
            for call in file
                .ast
                .calls_with_peeled_callee(span)
                .filter(|call| !call.construct)
            {
                add(
                    Site {
                        location: call.span.location(file.path.as_str()),
                        through: Some(reference.clone()),
                        accessor: format!("{name}()").into(),
                        declaration: declaration.clone(),
                    },
                    &mut sites,
                );
            }
        }
    }
    // (2) Every call transport resolves to a pending accessor.
    let mut transported = draft
        .source_call_targets
        .iter()
        .filter_map(|(callee, target)| match target {
            crate::interproc::accessor_provenance::Callable::Accessor(symbol)
                if pending(symbol.as_str()) =>
            {
                Some((callee, ctx.accessors.get(symbol)?))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    transported.sort_unstable_by_key(|(callee, _)| {
        (callee.path.clone(), callee.start_byte, callee.end_byte)
    });
    for (callee, (name, declaration)) in transported {
        let Some(file) = lookup.file_by_path(callee.path.as_ref()) else {
            continue;
        };
        let Some(span) = span_of(callee) else {
            continue;
        };
        for call in file.ast.calls.iter().filter(|call| call.callee == span) {
            add(
                Site {
                    location: call.span.location(file.path.as_str()),
                    through: Some(callee.clone()),
                    accessor: format!("{name}()").into(),
                    declaration: declaration.clone(),
                },
                &mut sites,
            );
        }
    }
    // (3) Every read row of a pending source.
    for read in &draft.reads {
        if !matches!(read.kind.as_ref(), "accessor" | "store-path") {
            continue;
        }
        if ctx
            .entities
            .get(&read.declaration)
            .is_some_and(|symbol| pending(symbol.as_str()))
        {
            add(
                Site {
                    location: read.location.clone(),
                    through: None,
                    accessor: read.accessor.clone(),
                    declaration: read.declaration.clone(),
                },
                &mut sites,
            );
        }
    }
    // (4) ADR 0926: every SC9012 row a pending source's census walk met: the
    // read through a value no row follows (`useIt()` after
    // `const { useIt } = { ["useIt"]: a1 }`, a bare JSX child). The row stands
    // for the read's position; SC5003 and SC5008, whose codes have none, ask
    // there too.
    let held_at = draft
        .static_defects
        .iter()
        .filter(|defect| {
            defect.uncertain
                && defect.kind.family() == crate::StaticDefectFamily::ReactiveDispatchUnresolved
        })
        .map(|defect| defect.location.clone())
        .collect::<crate::HashSet<_>>();
    let throughs = sites
        .iter()
        .filter_map(|site| site.through.clone())
        .collect::<crate::HashSet<_>>();
    for reach in reach.iter().filter(|reach| {
        reach.async_walk
            || reach
                .symbol
                .as_ref()
                .is_some_and(|symbol| pending(symbol.as_str()))
    }) {
        for at in &reach.at {
            // The row stands at the use, or at the callee of the call the
            // use is: the site is the call, as for a direct call.
            let call = lookup
                .file_by_path(at.path.as_ref())
                .zip(span_of(at))
                .and_then(|(file, span)| {
                    file.ast
                        .calls
                        .iter()
                        .find(|call| (call.callee == span || call.span == span) && !call.construct)
                        .map(|call| {
                            (
                                call.span.location(file.path.as_str()),
                                call.callee.location(file.path.as_str()),
                            )
                        })
                });
            let through = [
                Some(at.clone()),
                call.as_ref().map(|(_, callee)| callee.clone()),
            ]
            .into_iter()
            .flatten()
            .find(|held| held_at.contains(held) && !throughs.contains(held));
            let Some(through) = through else {
                continue;
            };
            add(
                Site {
                    location: call.map_or_else(|| at.clone(), |(call, _)| call),
                    through: Some(through),
                    accessor: reach.accessor.clone(),
                    declaration: reach.declaration.clone(),
                },
                &mut sites,
            );
        }
    }
    if sites.is_empty() {
        return;
    }
    // What each site asks of the draft, indexed once: the async read rows by
    // location, pending origin and origin (by position, so a site's rows keep
    // the draft's order), the cut locations, the uncertain dispatch rows'
    // locations, and SC5008's spans by path. None of them changes below.
    let mut at = crate::HashMap::<&Location, Vec<usize>>::default();
    let mut by_pending_origin = crate::HashMap::<&Location, Vec<usize>>::default();
    let mut by_origin = crate::HashMap::<&Location, Vec<usize>>::default();
    let mut cut_at = crate::HashSet::<&Location>::default();
    for (index, row) in draft.async_reads.iter().enumerate() {
        at.entry(&row.location).or_default().push(index);
        if let Some(origin) = &row.pending_origin {
            by_pending_origin.entry(origin).or_default().push(index);
        }
        if let Some(origin) = &row.origin {
            by_origin.entry(origin).or_default().push(index);
        }
        if row.walk_cut {
            cut_at.insert(&row.location);
        }
    }
    let mut caught_spans = crate::HashMap::<&str, Vec<(u64, u64)>>::default();
    for violation in draft
        .static_violations
        .iter()
        .filter(|violation| violation.id == "SC5008")
    {
        caught_spans
            .entry(violation.location.path.as_ref())
            .or_default()
            .push((violation.location.start_byte, violation.location.end_byte));
    }
    // What each site asks of the build alone (its reasons, the caught claim,
    // a loose match): asked on the workers, in the sites' order.
    let asked = crate::pipeline::parallel_balanced_results(&sites, |site| {
        let file = lookup.file_by_path(site.location.path.as_ref());
        let span = span_of(&site.location);
        let mut reasons = site_reasons(ctx, declared, file, span, site);
        let caught_claim = match (file, span) {
            (Some(file), Some(span)) => caught_claim(ctx, file, span, &mut reasons),
            _ => false,
        };
        let loose_match = file.zip(span).is_some_and(|(file, span)| {
            crate::owners::read_in_loose_match(
                file,
                span,
                ctx.entities,
                ctx.symbol_names,
                lookup.dialect,
            )
        });
        (file, span, reasons, caught_claim, loose_match)
    });
    let mut added = Vec::new();
    let mut caught_rows = Vec::new();
    for (site, (file, span, site_reasons, caught_claim, loose_match)) in
        sites.into_iter().zip(asked)
    {
        // Review R6: a row a walk placed at a call stands for its read only
        // when that walk placed every path; a cut walk left its read rows of
        // its own, and those account for it.
        let cut = cut_at.contains(&site.location);
        let mut positions = at.get(&site.location).cloned().unwrap_or_default();
        if !cut {
            for index in [&by_pending_origin, &by_origin]
                .into_iter()
                .filter_map(|index| index.get(&site.location))
            {
                positions.extend(index);
            }
            positions.sort_unstable();
            positions.dedup();
        }
        let rows = positions
            .into_iter()
            .map(|index| &draft.async_reads[index])
            .collect::<Vec<_>>();
        // An SC9012 row at the read or its reference stands for SC5001,
        // whose code has the read's position (review R5). ADR 0926: not for
        // SC5003 or SC5008, whose codes have none: each names its own row.
        let held = held_at.contains(&site.location)
            || site
                .through
                .as_ref()
                .is_some_and(|through| held_at.contains(through));
        // The site's reasons and the caught claim's (asked above), and the
        // walk's: a set the rules below only search.
        let mut reasons = site_reasons;
        // A row at the read, or a walk that cut it, keeps the row logic.
        if rows.is_empty() && !cut && walk_decided.contains(&site.location) {
            reasons.push(PendingSafe::WalkDecided);
        }
        let caught_row = caught_spans
            .get(site.location.path.as_ref())
            .is_some_and(|spans| {
                spans.iter().any(|(start, end)| {
                    *start < site.location.end_byte && site.location.start_byte < *end
                })
            });
        for rule in PendingRule::ALL {
            if rule == PendingRule::Caught && (!caught_claim || caught_row) {
                continue;
            }
            if held && rule == PendingRule::Unsuspendable {
                continue;
            }
            if reasons
                .iter()
                .any(|reason| reason.rules_out().contains(&rule))
            {
                continue;
            }
            let decided = rows.iter().any(|row| {
                reported_rule(row) == Some(rule)
                    || row_reasons(lookup, row)
                        .iter()
                        .filter(|reason| {
                            !(loose_match && **reason == PendingSafe::SuspendsInComputation)
                        })
                        .any(|reason| reason.rules_out().contains(&rule))
                    || (rule == PendingRule::Unsuspendable && opened_by_strict_read(draft, row))
            });
            if decided {
                continue;
            }
            if let (Some(file), Some(span)) = (file, span) {
                crate::role_log::pending(rule.code(), file.path.as_str(), span, "unaccounted");
            }
            if rule == PendingRule::Caught {
                let mut row = unaccounted_caught(&site);
                if held {
                    crate::blockers::PENDING_READ_UNRESOLVED.clone_into(&mut row.analysis_context);
                }
                caught_rows.push(row);
            } else {
                let mut row = unaccounted_row(&site, rule);
                row.source_module_unresolved = source_from_unresolved_import(ctx, &site);
                row.read_unresolved = held;
                added.push(row);
            }
        }
    }
    draft.async_reads.extend(added);
    draft.static_violations.extend(caught_rows);
}

/// ADR 0891: the site's source is created by a primitive whose first argument
/// is a call of, or a reference to, an import that resolves to nothing.
fn source_from_unresolved_import(ctx: &AnalysisContext<'_>, site: &Site) -> bool {
    let Some(file) = ctx
        .semantic_lookup
        .file_by_path(site.declaration.path.as_ref())
    else {
        return false;
    };
    let (Ok(start), Ok(end)) = (
        u32::try_from(site.declaration.start_byte),
        u32::try_from(site.declaration.end_byte),
    ) else {
        return false;
    };
    let span = Span::new(start, end);
    let created = file
        .ast
        .bindings
        .iter()
        .find(|binding| binding.names.iter().any(|name| name.span == span))
        .and_then(|binding| file.ast.call_at(binding.call_initializer?));
    let Some(argument) = created.and_then(|call| call.arguments.first()) else {
        return false;
    };
    let value = file.ast.peel_ts_sugar_span(argument.span);
    let value = file
        .ast
        .call_at(value)
        .map_or(value, |call| file.ast.peel_ts_sugar_span(call.callee));
    let Some(declared) = file.ast.reference_declaration(value) else {
        return false;
    };
    file.ast
        .imports
        .iter()
        .flat_map(|import| &import.bindings)
        .any(|binding| {
            binding.local.span == declared
                && ctx
                    .entities
                    .at(file.path.as_str(), declared)
                    .is_none_or(|symbol| ctx.semantic_lookup.symbol_undeclared(symbol.as_str()))
        })
}

/// SC5008's own row shape (`discover_caught_reads`), uncertain.
fn unaccounted_caught(site: &Site) -> crate::StaticViolation {
    crate::StaticViolation {
        id: "SC5008".into(),
        rule: "caught-reactive-read".into(),
        message: format!(
            "this read of {} sits in a try whose catch may swallow its suspension or error, \
             and no fact says where its pass runs",
            site.accessor
        ),
        hint: "Branch on readiness with isPending(), show ahead with latest(), or let the read \
               suspend; handle errors with <Errored>."
            .into(),
        location: site.location.clone(),
        analysis_context: crate::blockers::ASYNC_READ_UNACCOUNTED.to_owned(),
        fixes: vec![],
        uncertain: true,
    }
}

/// Reasons evaluable at the read itself, whatever row stands there.
fn site_reasons(
    ctx: &AnalysisContext<'_>,
    declared: &(dyn Fn(&str) -> bool + Sync),
    file: Option<&solid_facts::FileFacts>,
    span: Option<Span>,
    site: &Site,
) -> Vec<PendingSafe> {
    let (Some(file), Some(span)) = (file, span) else {
        return Vec::new();
    };
    let lookup = ctx.semantic_lookup;
    let mut reasons = Vec::new();
    if crate::execution_role::discarded_region_contains(file, span) {
        reasons.push(PendingSafe::Discarded);
    }
    let callee = file.ast.call_at(span).map_or(span, |call| call.callee);
    if crate::execution_role::pending_accessor_probe(file, callee, lookup) {
        reasons.push(PendingSafe::PendingProbe);
    }
    // A reviewed scheduler's callback (ADR 0435): the role table's fact, read at
    // the site for a read no producer has a row for (a transported call).
    if crate::execution_role::fresh_stack_callback_call(file, callee, lookup).is_some() {
        reasons.push(PendingSafe::FreshStack);
    }
    if crate::owners::read_loading_cover(lookup, file, span, ctx.symbol_names)
        == crate::owners::LoadingCover::Covered
    {
        reasons.push(PendingSafe::UnderLoading);
    }
    if crate::execution_role::computation_read_suspends(
        file,
        callee,
        ctx.entities,
        ctx.symbol_names,
        lookup,
    ) {
        // `tracked_computation_read` excludes a render computation: this one
        // is no render effect, and its own result is a pending source whose
        // readers are sites of their own (`over_async`, ADR 0802).
        reasons.push(PendingSafe::SuspendsInComputation);
        reasons.push(PendingSafe::NotRenderEffect);
    }
    // ADR 0892: never with `NotRenderEffect` (review D20): a flow component's
    // prop is read for the render it feeds, and the boundary claim stands.
    if crate::owners::read_in_flow_prop(file, span, ctx.entities, ctx.symbol_names, lookup.dialect)
    {
        reasons.push(PendingSafe::SuspendsInComputation);
    }
    if ctx
        .entities
        .get(&site.declaration)
        .is_some_and(|symbol| declared(symbol.as_str()))
        && crate::execution_role::served_in_creating_pass(file, span, &site.declaration, lookup)
    {
        reasons.push(PendingSafe::DeclaredServed);
    }
    reasons
}

/// Whether the read sits in a `try` whose handler may swallow its throw, and
/// if so, the reason no computation's pass holds it, where one is proven.
fn caught_claim(
    ctx: &AnalysisContext<'_>,
    file: &solid_facts::FileFacts,
    span: Span,
    reasons: &mut Vec<PendingSafe>,
) -> bool {
    let Some(caught) = file
        .ast
        .try_catches
        .iter()
        .filter(|caught| {
            caught.block.contains(span)
                && !crate::local_access::handler_only_throws(file, caught.handler)
        })
        .min_by_key(|caught| caught.block.end - caught.block.start)
    else {
        // R2h: a method's or getter's body runs where a member access or a
        // method call enters it (`held.v` in a `try`), which no `try` around
        // the read shows: with a swallowing `try` in the file, the claim
        // stays open.
        return file.ast.functions.iter().any(|function| {
            function.span.contains(span)
                && (function.method_name.is_some()
                    || file
                        .ast
                        .object_properties
                        .iter()
                        .any(|property| !property.data && property.value.contains(function.span)))
        }) && file
            .ast
            .try_catches
            .iter()
            .any(|caught| !crate::local_access::handler_only_throws(file, caught.handler));
    };
    let lookup = ctx.semantic_lookup;
    let role = crate::execution_role::read_execution_role(
        file,
        caught.block,
        &crate::execution_role::allowed_callback_spans(file, lookup),
        ctx.entities,
        ctx.symbol_names,
        lookup,
    );
    let proven = match role {
        ExecutionRole::ModuleInitialization | ExecutionRole::EffectApply => true,
        ExecutionRole::EventCallback => !lookup.synchronous_event_dispatch_present(),
        ExecutionRole::UntrackedRendering => {
            crate::owners::containing_ast_function(&file.ast, caught.block)
                .is_some_and(|function| mounted_only(file, function))
        }
        _ => false,
    };
    if proven {
        reasons.push(PendingSafe::NoComputationPass);
    }
    true
}

/// A function bound to a name never written, not exported, and referenced
/// only as a JSX tag: nothing calls its body as a plain function.
fn mounted_only(file: &solid_facts::FileFacts, function: &solid_facts::ast::FunctionFact) -> bool {
    let Some(binding) = crate::owners::function_binding_name(file, function) else {
        return false;
    };
    let name = file.source_text(binding.span).unwrap_or_default();
    if file.ast.exports.iter().any(|export| {
        export.span.contains(binding.span)
            || export
                .specifiers
                .iter()
                .any(|specifier| file.source_text(specifier.local.span) == Some(name))
    }) {
        return false;
    }
    let mut references = file.ast.identifiers.iter().filter(|identifier| {
        identifier.role == solid_facts::ast::IdentifierRole::Reference
            && file.ast.reference_declaration(identifier.span) == Some(binding.span)
    });
    references.all(|reference| {
        file.ast
            .jsx_elements
            .iter()
            .any(|element| element.name.span == reference.span)
    }) && !file
        .ast
        .assignments
        .iter()
        .any(|assignment| file.ast.reference_declaration(assignment.target) == Some(binding.span))
}

/// ADR 0509: a reported strict read at the row's read opens it, and the
/// projection gives its pending side an SC5001 uncertifiable there.
fn opened_by_strict_read(draft: &ProgramDraft, row: &AsyncRead) -> bool {
    row.async_provenance
        && !row.apply_first_run_pending
        && draft.reads.iter().any(|read| {
            read.location == row.location
                && (read.execution.reports_untracked_read() || read.execution.reports_unplaced())
                && !read.project_consumer_non_strict
                && read.execution != ExecutionRole::ModuleInitialization
        })
}

fn unaccounted_row(site: &Site, rule: PendingRule) -> AsyncRead {
    let (execution, invocation_context_unproven) = match rule {
        PendingRule::Unsuspendable => (ExecutionRole::Unknown, true),
        PendingRule::Boundary | PendingRule::Caught => (ExecutionRole::TrackedJsx, false),
    };
    AsyncRead {
        accessor: site.accessor.clone(),
        location: site.location.clone(),
        declaration: site.declaration.clone(),
        execution,
        leaf_owner: None,
        under_loading: false,
        async_provenance: true,
        declared_loading: false,
        options_opaque: false,
        provenance_open: false,
        caught: false,
        component_prop: false,
        ssr_client_hole: false,
        server_rendering_unresolved: false,
        host_callback_timing: false,
        callee_callback_timing: false,
        invocation_context_unproven,
        leaf_call_site_gates: Vec::new(),
        mount_unresolved: false,
        settle_unproven: false,
        call_attributed: false,
        carrier_run_unproven: false,
        creation_pass_unproven: false,
        root_pass_unproven: false,
        apply_first_run_pending: false,
        pending_origin: None,
        pending_apply: None,
        origin: None,
        unaccounted: true,
        read_unresolved: false,
        source_module_unresolved: false,
        walk_cut: false,
    }
}

#[cfg(test)]
mod tests {
    use super::{PendingRule, PendingSafe};

    /// ADR 0838's table: a reason that rules out the untracked-read claim
    /// never accounts for the boundary claim, and the other way round.
    #[test]
    fn each_reason_rules_out_only_its_rules() {
        use PendingRule::{Boundary, Caught, Unsuspendable};
        let table = [
            (
                PendingSafe::Discarded,
                &[Unsuspendable, Boundary, Caught][..],
            ),
            (PendingSafe::PendingProbe, &[Unsuspendable]),
            (PendingSafe::WalkDecided, &[Unsuspendable, Boundary]),
            (
                PendingSafe::DeclaredServed,
                &[Unsuspendable, Boundary, Caught],
            ),
            (PendingSafe::FreshStack, &[Unsuspendable, Boundary, Caught]),
            (PendingSafe::SuspendsInComputation, &[Unsuspendable]),
            (PendingSafe::OutsideStrictWindow, &[Unsuspendable]),
            (PendingSafe::NotRenderEffect, &[Boundary]),
            (PendingSafe::UnderLoading, &[Boundary]),
            (PendingSafe::DeclaredFirstPaint, &[Boundary]),
            (PendingSafe::NoComputationPass, &[Caught]),
        ];
        for (reason, rules) in table {
            assert_eq!(reason.rules_out(), rules, "{reason:?}");
        }
    }
}
