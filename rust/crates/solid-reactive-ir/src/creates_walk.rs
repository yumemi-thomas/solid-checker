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

use std::collections::BTreeMap;

use solid_dialect::CallClaimDomain;
use solid_facts::core::Span;

use crate::PrimitiveName;
use crate::pipeline::AnalysisContext;

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

    /// How many call sites the walk refused, across every file. Diagnostic
    /// only.
    #[must_use]
    pub fn refused_calls(&self) -> usize {
        self.refusals.values().map(Vec::len).sum()
    }
}

pub(crate) fn collect_project(ctx: &AnalysisContext<'_>) -> CreatesProposalWalk {
    let mut refusals = BTreeMap::<String, Vec<(u32, u32)>>::new();
    // Every call whose callee resolves to a project function, so the fixpoint
    // below can follow the edge: (caller file, call span) -> (callee file,
    // callee function span).
    let mut local_edges = Vec::<(&str, (u32, u32), &str, Span)>::new();
    for file in &ctx.facts.files {
        let primitives = ctx.semantic_lookup.primitives(file);
        for (index, call) in file.ast.calls.iter().enumerate() {
            if call_refuses_a_creates_proposal(
                ctx,
                file,
                call.callee,
                primitives.calls.get(index).and_then(Option::as_ref),
            ) {
                refusals
                    .entry(file.path.to_string())
                    .or_default()
                    .push((call.span.start, call.span.end));
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
    CreatesProposalWalk {
        walked: true,
        refusals,
    }
}

fn call_refuses_a_creates_proposal(
    ctx: &AnalysisContext<'_>,
    file: &solid_facts::FileFacts,
    callee: Span,
    primitive: Option<&PrimitiveName>,
) -> bool {
    // A canonical primitive is decided by the dialect tables and nothing else.
    // The audits are the only negative authority about a primitive, and their
    // silence — an unaudited dialect, a withheld row, a domain no dialect has
    // admitted — is "do not propose".
    if let Some(PrimitiveName::Known(_, spelling)) = primitive {
        return !solid_dialect::some_audit_denies_primitive(spelling, CallClaimDomain::Creates);
    }
    let Some(symbol) = ctx.semantic_lookup.callee_symbol(file, callee) else {
        return true;
    };
    ctx.semantic_lookup
        .contract_creates_closed_empty(symbol)
        .is_some_and(|closed_empty| !closed_empty)
}
