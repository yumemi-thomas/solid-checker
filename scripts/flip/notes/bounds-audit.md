# Status: audit done, fixes NOT landed. Patch: scratch `w0/p-bounds.diff`, `w0/gen-chain.py`, fixture `w0/hold/fixtures/walk-bounds`. W landed bounds commits after this was written (ADRs 0750, 0751): reconcile first.

# Bounds audit, Track R (ADR 0695)

Every bound in `solid-reactive-ir` and `rust/dialects/solid-v2/rules/`, what it answers at the limit. "Fail closed" means the limit refuses a proof or opens an obligation. "Fixed" names what the limit answered before this audit. The rules crate holds no bound.

How it was checked: reading, then helper, forwarded-callback, alias, holder, hook, prop, component, async-return and leaf chains at depths 2 to 70 (`w0/chaingen*.mjs` in scratch), with Chrome on every certified row. The generator keeps the shapes as `chain-*` and `forward-*` layers.

| Bound | Where | At the limit | Verdict |
|---|---|---|---|
| `HANDED_DEPTH` 6 | `handed_value` | was false, a silent clean | **Fixed**: true, only opens obligations |
| `depth` | `handed_container` | was false, a silent clean | **Fixed**: true |
| `depth` 3 | `may_run_in_leaf_owner_within` | was false, no obligation | **Fixed**: true |
| `depth` 4 | `value_reaches` | was false; every caller opens on true | **Fixed**: true |
| `depth` 4 | `may_throw_not_ready` | was its own name check, so sync | **Fixed**: a project call past the bound may throw |
| `depth` 3 | `region_calls_handed` | was `None`, no obligation | **Fixed**: the region is the site |
| `CALL_ATTRIBUTED_DEPTH` 4 | `push_call_attributed_async_reads` | was `continue`, certified, Chrome `PENDING_ASYNC_FORBIDDEN_SCOPE` at 5 and 7 | **Fixed**: unplaced async rows |
| `MAX_DEPTH` 6 | `expand`, after-await | was `return`, certified, Chrome `UNTRACKED_READ_AFTER_AWAIT` | **Fixed**: `push_unfollowed`; a sync helper in the chain is followed |
| `MAX_LAYERS` 12, `BUDGET` 400 | `effect_phase` | was `None`, certified, Chrome `FLUSH_IN_EFFECT_CALLBACK`, `PRIMITIVE_IN_EFFECT_CALLBACK` | **Fixed**: a mixed origin at the site. A forwarded callback, never followed, is a mixed origin too |
| depth 24 | `derived_objects` | was dropped from a may-list | **Fixed**: the site stands for the unread objects |
| `allowed-unplaced-nested` | `semantic_role_before_awaits` | a silent default, no bound, found by the chains: certified, Chrome warns | **Fixed**: `Unknown` unless a fresh-stack scheduler holds it |
| `MAX_ALIASES`, `MAX_FUNCTIONS` | `completeness.rs` | open | Fail closed |
| `MAX_STEPS` | `value_use.rs` | open | Fail closed |
| `HELPER_WRITE_DEPTH` 8 | write walk | `Unknown`, `unplaced` | Fail closed, ADR 0532 |
| `STEP_BUDGET` | `write_components.rs` | `Unknown` | Fail closed |
| `DEPTH_LIMIT` 64 | `site_reach.rs` | `runs` true | Fail closed: "not proven unreached" |
| `UNPLACED_WALK_BUDGET` | `runs_unplaced_through_callers` | true | Fail closed, ADR 0674 |
| `CALL_WALK_BUDGET` | `calls_reach` | `value_reaches`, now true at its own bound | Fail closed |
| `FLOW_BUDGET` 800 | `invocation_flow.rs` | the walk answers `None`: no role | Fail closed, ADR 0319 |
| climb 16 and 8 | `climb_in_place_layers` | `exhausted`, read as `Unknown` | Fail closed, ADR 0532 |
| `MAX_REGION_DEPTH` | after-await regions | `push_unfollowed`, a defect | Fail closed |
| `MAX_DEPTH` 6 | `pre_await::scan` | `reads.opaque` | Fail closed |
| `MAX_DEPTH` 6 | `native_promise_claim`, `settlement`, `pending_promise`, `handler_fulfills` | false or `Unknown`: no proof | Fail closed |
| `MOUNT_DEPTH` 4 | `mount_sites_within` | its direct sites only | Fail closed: an empty list refuses the first-run proof, and each listed site is a mount that exists |
| `CREATION_DEPTH` 6 | `runs_in_pass` | false: not proven in the pass, U | Fail closed |
| `PROVIDED_DEPTH` 8 | `function_provided` | false: not proven provided, U (`comp-ctx-provided` 9 and up) | Fail closed |
| `depth` | `alias_census` | `unfollowed` | Fail closed |
| `WINDOW_DEPTH` 3 | `named_function_standing` | `Unproven` | Fail closed |
| `depth` | `action_step_body_through`, `_unplaced` | `None`: not in a step. Chains reach V to 13, U past it | Fail closed by the walk above it, `step-flush` |
| `depth` | `helper_forbidden_operations`, `function_`, `tag_` | false: incomplete, an obligation | Fail closed |
| `depth` 8 | `callback_argument_function` | `None`: unresolved, `leaf-callback-opaque` | Fail closed |
| `CARRIED_LEAF_DEPTH` 4 | leaf carriers | not carried: open | Fail closed |
| `MAX_NODES`, `MAX_CONTEXT_DEPTH` | `project_consumer` | `None`: no proof | Fail closed |
| `value`, `context_origin`, depth 24 | `resolver_values` | `None`: no transport, the handed net answers | Fail closed |
| `closed_dependency_objects` 32 | resolver | `None`: no proof | Fail closed |
| `effective_value_return`, `returned_source_through`, `closed_source` | sources | `None`: no source row, the census and handed net answer | Fail closed |
| `proven_primitive`, `value_origin`, `value_is_primitive`, `returns_primitive`, `prop_holds_builtin`, `parameter_holds_builtin`, `prototype_is_plain`, `settled_value`, `class_instance_is_exact_in`, `value_is_function`, `origin_member_ignores_receiver`, `carried_into_callback`, `parameter_only_member_accessed_within`, `separate_iteration_target`, `created_accessor`, `mutation_target_projected`, `mutation_parameter_inputs`, `context_read_only_argument`, `read_of_module_hook_within`, `hook_result_within`, `callback_result_binding_chain` | proofs | false or `None`: the proof is refused | Fail closed |
| `may_be_object`, `class_chain_prototype_written_in`, `string_code_reference`, `string_code_slot`, `span_may_make_computation` | hazards | true | Fail closed |
| `SPREAD_RECEIVER_DEPTH`, `VIEW_CALL_DEPTH`, `ALIAS_DEPTH`, `PASS_THROUGH_DEPTH`, `SETTLED_DEPTH`, `DEPTH` | proofs of a view, alias or value | false: not proven | Fail closed |
| `MAX_ALIAS` 4 | `execution_paths::classify` | `None`: no arm | Fail closed: live code |
| `MAX_ALIAS_HOPS` 8 | `registration_root_runner` | an alias left: `None` | Fail closed |
| `follow_export` depth 8 | `effect_phase` | `Chain::Unknown` | Fail closed |
| `MEMBER_CALLEE_PATH_LIMIT` 32 | `member_callee_receiver` | a true prefix, a weaker claim | Fail closed, pinned by test |
| `depth > 8` | `typed_sources` candidates | stops adding candidates of a typed call | Fail closed: a candidate is a typed obligation, none means the census answers |
| `depth > 32` | `route_components` | stops collecting routes | Fail closed: a route it misses is a component with no mount, which reads as unresolved |
| `depth > 8` | `compound` literals | `None` | Fail closed |
