# Bounds audit (Track W, from 8ddc1ce24)

Question per bound: when the search stops at its limit, what does it answer? Past the bound the result is unknown, so the
answer must be an uncertifiable with a named blocker, never a clean and never a violation from the partial path.

Scope: `rust/crates/solid-reactive-ir/src` and `rust/dialects/solid-v2/rules/src` (the rules crate has no bound: `lib.rs`
and `rules.rs` only word and project rows). Not counted (not bounds): semantic arity (`take(index + 1)`, `take(handlers)`, executor
`take(2)`), quote and literal-length tests, `len() > 1` ambiguity tests, worker and shard limits, table id ranges, owner-bit
flags, timing samples. Fixpoints bounded by their own size (`symbols.rs` `0..=targets.len()` ×5, `owners/bindings.rs:272`,
`lib.rs:2167` `0..summaries.len()`, a shortest-path relaxation) terminate at the fixed point: sound, one row each below.

Polarity is the key. A **proof** search (true or `Some` proves safety, a run, an identity; false or `None` withholds) is
sound when cut: it proves less. A **risk** search (true opens a finding) cut to false is a silent clean. A **safety**
search cut to "no exemption" is a violation from a partial path. "probe" = measured through the release checker
(`~/EDF/tmp/flipw/bounds/probes`, runner `runp.mjs`); "read" = from the code and its callers.

Blockers added (`blockers.rs`): `search-depth-limit` (a chain of calls, promises, layers or values longer than the walk
follows), `search-budget-exhausted` (a work budget, a site cap or a round cap ran out).

## Summary

Counted per site (a row listing `:61,119,125` is three).

| Family | Sites | Answered clean or partial before | Fixed | ADR |
|---|---:|---:|---:|---|
| A. Static stack and phase walks (calls, promise chains, layers) | 36 | 9 | 8 | 0750 |
| B. Value, alias and member chains (incl. proof-only searches) | 73 | 4 (+3 unverified) | 3 (`calls_reach` under 0752) | 0751, 0752 |
| C. Caller, site and holder walks | 12 | 0 | no change | |
| D. Work budgets and round caps | 19 | 1 | 1 | 0752 |
| E. Write-role climbs (ADR 0532) and size-bounded fixpoints | 17 | 0 | no change | |
| Total | 157 | 14 | 12 | |

One more answered a violation from a partial path (`is_own_setter`, a measured false positive), counted in B. Open: 2 known
(`dynamic_component_entry`, `derived_objects`) plus the unverified ones below.

## Measured

| Probe | Shape | Base answer | Bound |
|---|---|---|---|
| `aa-then-long`, `aa-then-outer` | `.then` chain of 10, read in the first or last handler | SC1002 gone (3 deep: SC1002/u) | after_await `expand` MAX_DEPTH 6; native_promise MAX_DEPTH 6 |
| `ep-layers-15`, `c2008-13` | effect apply → 13-15 helpers → `createMemo` / `flush()` | certified (12 deep: SC6002/v, SC2008/v) | effect_phase MAX_LAYERS 12 |
| `hv-8` | `props.r.m0…m7.read()` after await | SC9012 handed-read gone (2 deep: U) | local_access `handed_value` HANDED_DEPTH 6 |
| `pm-12` | `o.read()` forwarded through 12 helpers from an apply | certified (3 deep: SC9012/u) | interproc/refresh FORWARDING_ROUNDS 8 |
| `ws-6` | `setStore` through 6 `const` aliases, store write in its callback | **SC2003/v, Chrome quiet** (1 alias: certified) | shared_reactivity `is_own_setter` depth 4 |
| `c2001-70`, `c2006-70`, `d2002-20`, `d3002-20` | 20-70 helpers | U (climb-exhausted / ADR 0560) | sound |
| `c1002-15`, `c3001-15`, `c5003-09`, `c5006-09`, `d1001-20` | 9-20 helpers | U or V proven | sound |
| `al-12`, `hk-12` | 12 `const` aliases, 12 returning hooks | V / U | compiler symbols, no bound reached |

## A. Static stack and phase walks

| Site | Bound | At the limit | Sound | Action |
|---|---|---|---|---|
| static_rules/after_await.rs:1876 `expand` | MAX_DEPTH 6 | returns: the chain's handlers are never reported | **no** (probe) | fixed: SC9012 obligation at the call, `search-depth-limit` |
| static_rules/after_await.rs:1139 `visit_call` | MAX_REGION_DEPTH 22 | `push_unfollowed` obligation | yes | |
| static_rules/after_await.rs:1711 `visit_template` | MAX_REGION_DEPTH 22 | ReactiveCallbackUnresolved obligation | yes | |
| static_rules/after_await.rs:1422 `report_helper_reads` | depth > 3 | returns; the interprocedural read stays (probe c1002-15 U) | yes | |
| static_rules/after_await.rs:2241 `settlement` | MAX_DEPTH 6 | `Unknown` → handler conditional | yes | |
| static_rules/after_await.rs:2409 `handler_fulfills` | MAX_DEPTH 6 | false → not proven fulfilled → conditional | yes | |
| static_rules/after_await.rs:2575 `pending_promise` | MAX_DEPTH 6 | false → not on flight → conditional | yes | |
| static_rules/native_promise.rs:120 `native_promise_claim` | MAX_DEPTH 6 | false: an undecided receiver reads as a thenable, `expand` returns | **no** (probe) | fixed: tri-state `claim`; handlers HandlerMayNotRun, `search-depth-limit`, receiver expanded |
| static_rules/pre_await.rs:317 `scan` | MAX_DEPTH 6 | `opaque = true` (may have read) | yes | |
| static_rules/pre_await.rs:913,922 `primitive_value` | proven_primitive 6 | proof only | yes | |
| effect_phase.rs:194 `layer` | MAX_LAYERS 12 | `None`: not in an effect phase | **no** (probe) | fixed: `EffectPhaseOrigin::undecided`, SC2008/SC6002 U |
| effect_phase.rs:334 `prop_layer` | BUDGET 400 | found → mixed; none found → `None` | **no** (none found) | fixed: undecided, `search-budget-exhausted` |
| effect_phase.rs:945 `named_layer` | BUDGET 400 | same | **no** (none found) | fixed: same |
| effect_phase.rs:577 `follow_export` | depth 8 | `Chain::Unknown` → instance unproven → `None` | **no** | fixed: undecided, `search-depth-limit` |
| static_api.rs:1548 `runs_unplaced_through_callers` | 4 helpers | false: ADR 0708-0710's net does not ask | **no** | fixed: answers its blocker; callers left past 4 (or a cut in-place climb) → `search-depth-limit` |
| static_api.rs:1415 `action_step_body_unplaced` | 4 | `None`; ADR 0708's else arm asks the net | yes, once the net is fixed | |
| static_api.rs:1456 `action_step_body_through` | 4 | `None`; same | yes, same | |
| static_rules.rs:1006 `region_calls_handed` | 3 regions | `None`: no handed read after an await | **no** | fixed: the unread helper's call, SC9012 U `search-depth-limit` |
| execution_role.rs:598 `dynamic_component_entry` | 32 climbs | uses the span reached as the entry: partial path | **no** | open: fix is `Some(false)` (Unknown role, not `None`, which keeps UntrackedRendering); no crossing input built (probe `dyn-34` never reaches it) |
| cleanup.rs:514 `callback_argument_function` | 8 | `None` → UnresolvedCallback obligation | yes | |
| cleanup.rs:976,1640,1872 `*_forbidden_operations` | 8 | false = incomplete → UnresolvedCallback (probe c3001-15) | yes | |
| interproc/callback_chain.rs:121 `synchronous_enclosure` | 8 | `None`: no ADR 0183 enclosure, general path | yes | |
| interproc/result_reads.rs:2373,2424 `enter_directly_called` | MAX_DEPTH 8 | enters fewer: proves less | yes | |
| interproc/result_reads.rs:80,82 `local_dispatch_exclusion_depth` | 32 | `None`: no exclusion (a discharge) | yes | |
| local_access.rs:8262 `first_rejection` | HELPER_WRITE_DEPTH 8 | `None` → paths followed → unplaced (ADR 0560) | yes | |
| local_access.rs:8428 `helper_call_write_execution` | HELPER_WRITE_DEPTH 8 | `unplaced = true` | yes | |
| local_access.rs:4255 `push_call_attributed_async_reads` | CALL_ATTRIBUTED_DEPTH 4 | no row; the read's own row stays (probe c5003-09 U) | yes | |
| local_access.rs:7313,7347 `carried_leaf_owner`, `gated_leaf_calls` | CARRIED_LEAF_DEPTH 4 | own position answers (probe c5006-09 U) | yes, measured | |
| owners/creation_pass.rs:68,168 `creator_of`, `runs_in_pass` | CREATION_DEPTH 6 | `None` / false: no creating-pass proof (V needs it) | yes | |

## B. Value, alias and member chains

| Site | Bound | At the limit | Sound | Action |
|---|---|---|---|---|
| local_access.rs:206 `handed_value` | HANDED_DEPTH 6 | false: not handed, no obligation | **no** (probe) | fixed: true (the callers' own obligation names it, e.g. `handed-read-after-await`) |
| upstream_compat/shared_reactivity.rs:3220,3229,3286 `is_own_setter` | 4 aliases | not the setter: its callback opens no window, a write there is V | **no: false positive** (probe, Chrome) | fixed: `Windows::undecided`, dropped → `Unproven(search-depth-limit)` |
| upstream_compat/shared_reactivity.rs:3084 `store_and_aliases` | 4 aliases | fewer intermediate aliases; the proven store `symbol` is always in the list (probes sa-7, sb-7: V and certified as at 2) | yes | |
| upstream_compat/shared_reactivity.rs:2745 `handed_container` | 6 | false → parameter flow, which opens past FLOW_DEPTH | yes | |
| upstream_compat/shared_reactivity.rs:2813 | `depth < 6` | semantic (symbol test below the root) | n/a | |
| upstream_compat/shared_reactivity.rs:1932,2151 `ParameterFlow` | FLOW_DEPTH 6 | `open = true` | yes | |
| upstream_compat/shared_reactivity.rs:2279 `call_value` | 6 | false → `handed_container` / open | yes | |
| upstream_compat/shared_reactivity.rs:3488 `named_function_standing` | WINDOW_DEPTH 3 | `Unproven` | yes | named: `Unproven(search-depth-limit)` |
| local_access.rs:3218 `closed_source` | 6 | `None`: the strengthening only | yes | |
| local_access.rs:5090 `value_reaches` | 4 | false; mixed polarity at its callers (`value_reaches_async_source` both ways) | unverified | open (see "Not fixed") |
| local_access.rs:3379,3417 `calls_reach` | CALL_WALK_BUDGET 512 | falls back to `value_reaches` 4 | **no** (risk search, SC2009) | fixed: past the budget it may reach (SC2009 U, `optimistic-reach-unresolved`); `discover_optimistic_reach` now returns early with no async optimistic source |
| local_access.rs:4948 `span_may_make_computation` | 4 | true | yes | |
| local_access.rs:2845 `may_run_in_leaf_owner_within` | 3 | false: not in a leaf owner (risk) | unverified (probe lo-5: U by the general obligation) | open: no crossing input found |
| local_access.rs:1202 `view_hides_key` | 8 | false: the read stays a read | yes | |
| local_access.rs:480 `call_builds_view` | VIEW_CALL_DEPTH 8 | false: an alias (the conservative reading) | yes | |
| source_discovery/props_reactivity.rs:984,1046 | SPREAD_RECEIVER_DEPTH 4 | not proven inert | yes | |
| source_discovery/typed_sources.rs:73 `push_view_candidates` | 8 | fewer candidates | unverified | open |
| source_discovery/typed_sources.rs:334 `declaration` | 4 | `None`: no typed source | unverified (compiler symbols cover aliases, probe al-12) | |
| source_discovery/returned_sources.rs:105,441, post_rows.rs:629 | RETURNED_SOURCE_FORWARDS 4 | `None` (probe hk-12: SC9012 U) | yes, measured | |
| accessor_provenance/resolver_values.rs:14 `value` | depth | `None` | yes (proof) | |
| accessor_provenance/resolver_values.rs:522 `context_origin` | depth | `None` (derivation for a census) | unverified | open |
| accessor_provenance/resolver_dependencies.rs:328 `derived_objects` | 24 | fewer objects, fewer getter obligations | **no** (read; 24 deep) | open |
| accessor_provenance/resolver_dependencies.rs:53,513 | depth | `None` / false: proof | yes | |
| accessor_provenance/mutation_sites.rs:42 | depth | false (negative proof only) | yes | |
| accessor_provenance/mutation_sites.rs:349,644 | depth | true (may be string code) | yes | |
| accessor_provenance/projection_helpers.rs:35,465 | depth | false: no complete join | yes | |
| accessor_provenance/resolver_queries.rs:87 | depth | refuses | yes | |
| indexes/primitive_proof.rs:23,201,241 | depth | false / `None`: proof | yes | |
| indexes/member_values.rs:13,653 | depth | `None` / false: proof | yes | |
| indexes/components.rs:807 `alias_rendered_function` | ALIAS_DEPTH 8 | `None`: no rendered function proven | yes | |
| indexes/census.rs:1638,1785 `alias_census` | ALIAS_DEPTH 8 | `unfollowed = true` | yes | |
| indexes/callees.rs:732 `member_callee_receiver` | MEMBER_CALLEE_PATH_LIMIT 32 | a prefix: a weaker claim (documented) | yes | |
| indexes/callees.rs:787 `nested_member_value_symbols_at` | path 8 | refuses | yes | |
| indexes/compound.rs:392 `literal_tree` | 8 | `None`: not closed | yes | |
| indexes/compound.rs:751 `may_be_object` | depth | true | yes | |
| indexes/callee_world.rs:182 / :221 | depth | true (may be written) / false (not exact) | yes | |
| indexes/test_runner.rs:135 | MAX_ALIAS_HOPS 8 | `None` | yes | |
| installed_members.rs:61,119,125,139,152,210,263,273 | DEPTH 6 | unresolved → not clear → obligation | yes | |
| execution_paths.rs:198,332 `classify` | MAX_ALIAS 4 | `None`: no build test, both arms run | yes | |
| execution_role/read_context.rs:472 `value_is_function` | depth | false | unverified | |
| execution_role/pending_sources.rs:859,1040 | depth | `None` / false: proof of a pending source | yes | |
| execution_role/flow_helpers.rs:311 | depth | false: no receiver proof | yes | |
| execution_role/callee_roles.rs:2381 `carried_into_callback` | depth | false: no proof | yes | |
| attribution.rs (12 sites: 94,102,899,947,1142,1476,1477,1588,1698,1722,1874) | PASS_THROUGH_DEPTH 4 | false: no discharge | yes | |
| interproc/obligations.rs:630 `direct_protocol_parameters` | hops 64 | stops at a member, not a parameter: no iterate use | unverified (64 members) | |
| upstream_compat/mod.rs:373 `entire_delimited` | depth | false | yes (syntax) | |
| route_components.rs:564 `definitions` | 32 | fewer route definitions | unverified | |
| name_matcher.rs:192,366,434 | 32 / INSTRUCTION_BUDGET 20,000 | pattern refused (`None`) | yes (option error) | |

## C. Caller, site and holder walks

| Site | Bound | At the limit | Sound | Action |
|---|---|---|---|---|
| execution_role/site_reach.rs:130,140,263,335 | DEPTH_LIMIT 64, walk bound | `Undecided` (ADR 0560) | yes | model |
| execution_role/callee_roles.rs:2317 `action_step_call_sites` | 9 holders | `None`: no ADR 0547 proof | yes | |
| owners/async_context.rs:154,171 `mount_sites_within` | MOUNT_DEPTH 4 | fewer mounts; every consumer is `any` (a proof) | yes | |
| owners/context_census.rs:469,479 `function_provided` | PROVIDED_DEPTH 8 | false → SC4002 obligation | yes | |
| execution_role/write_roles.rs:747 `runs_in_disposal_within` | 4 | `Unplaced` | yes | |
| project_consumer.rs:362,671 | MAX_NODES 128, MAX_CONTEXT_DEPTH 32 | `None`: no consumer proof | yes | |
| effect_phase / static_api caller walks | see A | | | |

## D. Work budgets and round caps

| Site | Bound | At the limit | Sound | Action |
|---|---|---|---|---|
| interproc/refresh.rs:48,93,573 forwarding loop | FORWARDING_ROUNDS 8 | chains past 8 truncated ("retained, not fixed") | **no** (probe pm-12) | fixed: `widen_truncated_forwarding` (parameter_use.rs), unmodeled use on owners still growing and their forwarders, fixed point, both loops; SC9012 `parameter-use-unmodeled` |
| execution_role/write_components.rs:72,79,126,141,323 | STEP_BUDGET 2^20, WALK_STEP_BOUND 2^16 | `Unknown` (ADR 0560) | yes | |
| local_access.rs:7984 `exhausted` | walk bound | `Unknown` climb | yes | |
| execution_role/invocation_flow.rs:17,584,765; event_literals.rs:196,223,699 | FLOW_BUDGET 800 | nothing proven (flow_memos doc) | yes | |
| effect_phase.rs:57 | BUDGET 400 | see A | | |
| local_access.rs:93 | CALL_WALK_BUDGET 512 | see B | | |
| interproc/reads.rs:2316,2342 | SMALL_RETURN_COUNT 16 | grouping strategy, same answer | n/a | |
| indexes.rs:981,993,1421,1433,1449-1456; indexes/completion.rs:333 | PROBED_ARGUMENTS 8, PROBED_RESULT_SLOTS 4 | dialect probe range: no primitive has a callback slot past 8 (dialect fact) | yes | |

## E. Write-role climbs (ADR 0532)

`execution_role/write_roles.rs` `for _ in 0..16` at 688, 1338, 1373, 1476, 1547, 3037, 3198, 3956, 4438, `0..32` at 3141: past
the bound each answers no proof (`Unplaced`, `true` = a root or unread, `Unrun`, `false`, `None`), as ADR 0532 reviewed. Plus
`owners/bindings.rs:272`, `symbols.rs` ×5, `lib.rs:2167` (fixpoints bounded by their size). Probes c2001-70, d2002-20,
d3002-20: U. No change.

## Not fixed (open)

Listed in docs/open-work.md section 1.

- `execution_role.rs:598` `dynamic_component_entry` (32 climbs, partial path). The fix is `Some(false)` past the loop (an
  Unknown read role; `None` would keep `UntrackedRendering`). Not built: no crossing input (probe `dyn-34` never reaches it).
- `local_access.rs:2845` `may_run_in_leaf_owner_within` (3): a risk search cut to false; probe `lo-5` is already uncertifiable
  through the general dispatch obligation, so no crossing input shows it.
- `local_access.rs:5090` `value_reaches` (4): callers use it both ways (`value_reaches_async_source` with `&&` and `!`); a fix
  needs a tri-state at each caller (R's async-read code).
- `accessor_provenance/resolver_dependencies.rs:328` `derived_objects` (24): fewer getter obligations past 24 hops.
- Unverified, low reach: `typed_sources.rs:73` (8 views), `resolver_values.rs:522` (`context_origin`), `route_components.rs:564`
  (32 nested routes), `interproc/obligations.rs:630` (64 members), `read_context.rs:472`.

Also seen, not bounds: `cleanup.rs:224` drops the UnresolvedCallback obligation when the call has no validity fact (a missing
fact answers silent); `effect_phase.rs` `instance_of`/`follow_export` `Unknown` for an unresolved hop answers silent (only
the depth arm is now undecided); after_await `expand` returns silently for an unresolved or non-standard call.

## Progress

- Family A done (ADR 0750): fixture `bounds-search-depth-limit`, process test `diagnostics_process::walks_past_their_depth_limit_are_uncertifiable` (base binary lacks every asserted row). Witnesses 220 of 335, same FAIL list. Corpus 0 moved (`~/EDF/tmp/flipw/bounds/sweep-A.json`, bins `bA`), wall +3%. Coverage: only the new fixture.
- Family B done (ADR 0751): fixture `bounds-value-depth-limit`, ownership case `no-store-mutation-outside-setter/setter-alias-limit/001`, process test `value_chains_past_their_limit_are_uncertifiable`. Witnesses 220 same list. Corpus +14 SC9012 handed-member-dispatch (app-game ogl/solid-three/three-examples, probus-hk vehicleArt), each a receiver chain reaching a parameter or `this` one step past the old cut; 0 V moved (`sweep-B.json`, bins `bB`). Upstream: SC2003's store windows are product-owned (eslint-plugin-solid has no such rule).
- Family D done (ADR 0752): fixture `bounds-budget-limit`, process tests `searches_past_their_budget_are_uncertifiable` and
  `backend_process::forwarding_refresh_process::a_forwarding_chain_past_the_round_cap_widens_like_a_fresh_build` (kept vs fresh).
  Witnesses 220 same list. Corpus +258 SC2009 U, all inferay `ui` (its store uses createOptimistic; calls in test files whose
  reach passes 512 nodes); first cut without the async-optimistic-source guard gave +365 across 5 projects that never use
  createOptimistic (the primitive check passes on the typings). Forwarding widening moved nothing in the corpus. Wall 135 s.
- All families done. Remaining open items above.
