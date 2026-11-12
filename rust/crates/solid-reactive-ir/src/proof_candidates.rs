//! Batch 11, slice 0 (measurement only): the candidate sites of the rules that have no site universe.
//!
//! [`crate::proof_universe`] lists the sites of SC1001, SC2001, SC2002, SC2003 and SC3002, and each must end in a row, an obligation, a discard or a named silence
//! ([`crate::site_accounting`], ADR 0936). The rules whose dev diagnostics have a code of their own and no such list (SC1002, SC2004 to SC2009, SC3001, SC5006, SC5007, SC6001,
//! SC6002, SC7001, SC7002) answer for the calls they were asked about only through their producers: a call a producer returned from without a row is a clean nobody accounted for.
//!
//! `SOLID_CHECKER_IR_PROOF_CANDIDATES=1`, with the proof log on, writes one `candidate` line for each place a producer's own first test holds: the call whose resolved
//! primitive the dialect gives the rule's role (`resolve`, `until`, `flush`, the effect and computation primitives, a static `dynamic`), a `Loading` boundary's `on`, each
//! reference the flush census holds, and (SC1002, by syntax alone) each read of the universe written after a dominating await of an async function. SC3001's and SC2009's candidates are
//! read from the log's own lines by the fold (the owner accounting's leaf-owner sites, the optimistic setter calls), and SC6001's are not listed. The line says where a finding of the rule would stand (`at`, the callee or the attribute) and the other spans it may stand at (`alt`), and the facts of
//! the producer's first test that tell its sites apart (`facts`: a `sync` option's state, whether `on` is proven not to be a function). It says nothing of how the site ends:
//! `scripts/lib/candidates.mjs` reads that from the same log's `verdict`, `silent` and `site` lines, so the findings are byte-identical with and without the flag
//! (`proof_log_process.rs` pins it) and a candidate's would-be ending (a finding of the rule, another rule's finding, a named silence, nothing) is a fold of what the checker did.
//!
//! The first tests are the producers' own (`static_api.rs`, `loading_hold.rs`), repeated here by the dialect's questions and not shared with them: the dry run must not touch a
//! producer. Where a producer asks more of the program than its first test (a client build, a walk that places the call), the candidate does not, so a count of candidates with no
//! finding is an upper bound of the sites a universe would have to end, never an estimate of the rows it would add.

use std::sync::OnceLock;

use solid_dialect::{Dialect, OptionLiteral, Primitive};
use solid_facts::FileFacts;
use solid_facts::ast::{ArgumentValueKind, CallFact};
use solid_facts::core::Span;

use crate::pipeline::AnalysisContext;
use crate::proof_log::{self, CandidateRecord};
use crate::proof_universe::Site;
use crate::{HashMap, HashSet};

/// Whether the dry run is asked for (`SOLID_CHECKER_IR_PROOF_CANDIDATES` set and not `0`) and the proof log is on: read once.
pub(crate) fn enabled() -> bool {
    static ASKED: OnceLock<bool> = OnceLock::new();
    proof_log::enabled()
        && *ASKED.get_or_init(|| {
            std::env::var_os("SOLID_CHECKER_IR_PROOF_CANDIDATES")
                .is_some_and(|value| !value.is_empty() && value != "0")
        })
}

const fn yes(flag: bool) -> &'static str {
    if flag { "yes" } else { "no" }
}

/// One candidate of a call: the rule, the table its producer's first test names, and the facts of that test.
struct Candidate {
    rule: &'static str,
    table: &'static str,
    facts: Vec<(&'static str, &'static str)>,
}

/// The candidates of one call of the primitive `kind` (the dialect's classification of the call, as `lookup.primitive_at_call` answers):
/// the first test of each `static_api.rs` arm that has no universe.
fn call_candidates(
    dialect: &dyn Dialect,
    file: &FileFacts,
    call: &CallFact,
    kind: Primitive,
) -> Vec<Candidate> {
    let mut found = Vec::new();
    let mut add = |rule, table, facts| {
        found.push(Candidate { rule, table, facts });
    };
    if kind == Primitive::Resolve {
        add("SC2004", "resolve", vec![]);
    }
    if kind == Primitive::Until {
        add("SC2005", "until", vec![]);
    }
    if matches!(
        kind,
        Primitive::DynamicStatic | Primitive::DynamicUnknownForm
    ) && !call.arguments.is_empty()
    {
        add(
            "SC2007",
            "dynamic-static",
            vec![(
                "form",
                if kind == Primitive::DynamicStatic {
                    "static"
                } else {
                    "unknown"
                },
            )],
        );
    }
    if dialect.throws_inside_action_step(kind) {
        add("SC2006", "flush-in-action", vec![]);
    }
    if dialect.warns_inside_effect_phase(kind) {
        add("SC2008", "flush-in-effect", vec![]);
    }
    let first_argument_is_function = call.arguments.first().is_some_and(|argument| {
        !argument.spread
            && matches!(
                argument.value,
                ArgumentValueKind::Function | ArgumentValueKind::AsyncFunction
            )
    });
    if dialect.throws_creating_in_effect_callback(kind, first_argument_is_function) {
        add(
            "SC6002",
            "create-in-effect",
            vec![("function", yes(first_argument_is_function))],
        );
    }
    if dialect.effect_function_slot(kind).is_some() {
        add("SC7001", "effect-function-slot", vec![]);
    }
    if dialect.supports_sync_option(kind)
        && let Some(options) = dialect.options_argument(kind)
    {
        // The producer's two readings of the `sync` option: a boolean property of the literal, and the option's literal answer.
        let property_true = call.arguments.get(options).is_some_and(|argument| {
            argument
                .boolean_properties
                .iter()
                .any(|property| file.source_text(property.name) == Some("sync") && property.value)
        });
        let state = if property_true {
            "true"
        } else {
            match crate::call_option_literal(file, call, options, "sync") {
                OptionLiteral::True => "true",
                OptionLiteral::False => "false",
                OptionLiteral::Absent => "absent",
                OptionLiteral::Unknown => "unknown",
            }
        };
        add("SC7002", "sync-option", vec![("sync", state)]);
    }
    found
}

/// SC1002's candidates, by syntax alone and so approximately: the reads of the universe written after a dominating await (`AstFacts::dominating_awaits`, the region an await
/// dominates in its own function) of an async function that holds them, directly (`nested=no`) or inside a function written in it (`nested=yes`: a callback, a handler,
/// a helper literal: some of them run on the continuation, as an `Array#filter` callback does, and some never do), and whether that async function is written as the first
/// argument of a primitive that runs a computation (the dialect gives it an effect function slot or a `sync` option): the shape of a tracked async computation. The producer
/// (`static_rules/after_await.rs`) reaches further (a function handed by name, the continuation of helpers) and asks more (a source tracked before the first suspension is exempt,
/// a nested function that runs later is outside the continuation), so this is neither a subset nor a superset of what it walks: `computation=yes nested=no` is the part it
/// certainly starts from.
fn read_after_await_candidates(ctx: &AnalysisContext<'_>, universe: &[(&FileFacts, Site)]) {
    let lookup = ctx.semantic_lookup;
    let dialect = lookup.dialect;
    // Per file, the spans of the first arguments of the computation primitives' calls (the async functions among them are the tracked computations).
    let mut computations: HashMap<&str, HashSet<Span>> = HashMap::default();
    for (file, site) in universe {
        if site.table != "read" {
            continue;
        }
        // The async function whose dominating await the read follows: the await belongs to that function (not to one nested in it), and the read lies in the region it dominates.
        let Some(owner) = file
            .ast
            .dominating_awaits
            .iter()
            .find_map(|(await_at, region)| {
                if !region.contains(site.at) {
                    return None;
                }
                crate::owners::containing_ast_function(&file.ast, *await_at)
                    .filter(|owner| owner.r#async && owner.span.contains(site.at))
            })
        else {
            continue;
        };
        let nested = crate::owners::containing_ast_function(&file.ast, site.at)
            .is_none_or(|innermost| innermost.span != owner.span);
        let handed = computations.entry(file.path.as_str()).or_insert_with(|| {
            file.ast
                .calls
                .iter()
                .filter(|call| {
                    lookup
                        .primitive_at_call(file, call.span)
                        .is_some_and(|kind| {
                            dialect.supports_sync_option(kind)
                                || dialect.effect_function_slot(kind).is_some()
                        })
                })
                .filter_map(|call| call.arguments.first())
                .map(|argument| file.ast.peel_ts_sugar_span(argument.span))
                .collect()
        });
        proof_log::candidate(&CandidateRecord {
            rule: "SC1002",
            table: "read-after-await",
            path: file.path.as_str(),
            at: site.at,
            alternates: &site.alternates,
            facts: &[
                ("via", site.via),
                ("computation", yes(handed.contains(&owner.span))),
                ("nested", yes(nested)),
            ],
        });
    }
}

/// Writes the `candidate` lines of the build's program (a build with the proof log and the dry run on): the calls of every file outside dependency source, the `Loading`
/// boundaries' `on`, and the flush census. Read-only: it asks the lookup what the producers asked, and returns nothing.
pub(crate) fn log(ctx: &AnalysisContext<'_>, universe: &[(&FileFacts, Site)]) {
    let lookup = ctx.semantic_lookup;
    let dialect = lookup.dialect;
    for file in crate::base::reads::file_slots(ctx.facts) {
        if lookup.in_dependency_source(file.path.as_str()) {
            continue;
        }
        for call in &file.ast.calls {
            let Some(kind) = lookup.primitive_at_call(file, call.span) else {
                continue;
            };
            let candidates = call_candidates(dialect, file, call, kind);
            if candidates.is_empty() {
                continue;
            }
            // The compiler's discard of the code ends any site in it, as for the universe's sites (`proof_universe::Endings::end`).
            let discarded = crate::execution_role::discarded_region_contains(file, call.callee);
            for mut candidate in candidates {
                candidate.facts.push(("discarded", yes(discarded)));
                proof_log::candidate(&CandidateRecord {
                    rule: candidate.rule,
                    table: candidate.table,
                    path: file.path.as_str(),
                    at: call.callee,
                    alternates: &[call.span],
                    facts: &candidate.facts,
                });
            }
        }
        crate::loading_hold::log_candidates(file, lookup);
    }
    read_after_await_candidates(ctx, universe);
    for site in lookup.flush_sites() {
        proof_log::candidate(&CandidateRecord {
            rule: "flush",
            table: "census",
            path: &site.path,
            at: site.span,
            alternates: &[],
            facts: &[
                ("called", yes(site.called)),
                ("caller", yes(site.caller.is_some())),
                ("drainsNothing", yes(site.drains_nothing)),
            ],
        });
    }
}

#[cfg(test)]
mod tests {
    use solid_facts::core::Span;

    use super::*;

    #[test]
    fn a_span_is_all_a_candidate_names() {
        // The record names no behaviour: a rule, a table, a place, the other places, and the facts of a first test.
        let alternates = [Span::new(0, 9)];
        let record = CandidateRecord {
            rule: "SC2004",
            table: "resolve",
            path: "/p/App.tsx",
            at: Span::new(0, 7),
            alternates: &alternates,
            facts: &[("form", "static")],
        };
        assert_eq!(record.alternates.len(), 1);
        assert_eq!(yes(true), "yes");
        assert_eq!(yes(false), "no");
    }
}
