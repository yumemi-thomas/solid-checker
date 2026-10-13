# ADR 0283: Getter reads through a parameter

- Status: accepted and implemented (2026-10-13).
- Owners:
  - `solid-reactive-ir/src/property_gets.rs`: `parameter_get_sites`,
    `parameter_gets`, `parameter_reads`, and the getter loop
    `getter_reads` that ADR 0225/0274 now share;
  - the hook beside `forwarded_enumerations` in
    `interproc::interprocedural_result_reads_for_file`.
- Extends ADR 0225 and ADR 0274 to a project function's own Gets. Reuses
  ADR 0276's completion sites and ADR 0278's exact callee mapping. No
  contract, facts schema or receipt change.
- Design: `rust/target/research/dep-mode/getter-param.md` (Plug A, slice 1).

## Context

`readsMember({ get a() { return s(); } })` in a component body, with
`function readsMember(o) { return o.a; }`, reads `s` untracked while the
component renders. Solid warns `STRICT_READ_UNTRACKED` for it. The checker
only reported the getter's read as uncertifiable: the getter is a stored
literal handed to a consumer not proven to run it. ADR 0225 and ADR 0274
prove getter reads only for a package value copy named by a contract. A
project function reading a member of its parameter had no such fact.

## Decision

1. **A parameter Get is a body fact.** A project function Gets `p.key` on
   every call when all of these hold:
   - the function is neither async nor a generator, carries no directive
     and neither does its module, and its value is current (ADR 0221);
   - it never names `arguments`, holds no escaped identifier, and its file
     holds no direct `eval`;
   - `p` is a plain, unwritten, default-less identifier parameter;
   - every reference to `p` is the object of a static, non-optional member
     that is no callee or tag and lies in no write, `delete`,
     destructuring or iteration target. So `p` never escapes, is never
     aliased and is never written through;
   - the member `p.key` is a completion site of the function
     (`site_on_every_completion`, ADR 0276) inside its body.
2. **The getter runs at the call.** At a call in a rendering function, an
   argument that is ADR 0225's exact receiver runs its `key` getter when:
   - the call is valid, not a construction, has one candidate and no
     transported callable, and has no contract row for the callee;
   - `project_callee_at_parameter` maps the argument exactly to the
     parameter (ADR 0278);
   - the callee has a summary node;
   - no getter of the receiver mentions `this` or `super`, holds an escaped
     identifier or holds direct `eval`. Every getter counts, because the
     function may Get another key first.
3. **Only the getter's every-completion reads count.** An accessor call in
   the getter body is emitted only when it is on the getter's own
   every-completion path. A read in a branch arm, after an early `return`,
   or in a nested closure is left out, not emitted as uncertain. It is
   located at the call, with the getter's read as origin and the call's
   execution role, as in ADR 0225. Anything unproven emits nothing, so
   today's uncertifiable read at the getter remains.
4. **A named receiver refuses direct `eval`.** `receiver` now refuses a
   `const` receiver when its file holds direct `eval`, which can delete a
   getter with no static reference. This applies to the contract path too.
5. **Result reuse.** Every evaluated (call, callee) pair records
   `FunctionSource(callee)`, refusals included. That is the callee file's
   source hash. The caller already holds a getter literal, which is
   call-role syntax. A callee without a summary node is refused: its
   dependency would stay `Missing` across every edit.

## Consequences

- The contract path keeps the premise for getter bodies. A probe against
  probus-hk's virtual-core 3.17.8 install
  (`rust/target/research/adr-0283/case.tsx`) reports
  `get count() { return flag ? rows() : 0; }` and an early-return getter
  as violations at `new Virtualizer(…)`. A branch inside an inline `map`
  callback is reported too. ADR 0275 keeps a branch written in a caller's
  callback under the statically-present-code premise, so this is not a
  false violation under it. The new path is stricter by choice, and the
  fixture's `CallbackBranch` control pins the asymmetry. Aligning the two
  is an owner decision.
- The getter's own uncertifiable read stays next to each new violation, as
  in ADR 0274.
- rc.13 corpus (browser host): 562 → 562 violations, 3,731 → 3,731
  uncertifiable, nothing moved. Which rule refuses each corpus getter
  literal was not audited. The design report expects the real shapes to
  need spreads, constructors or forwarding chains (slices 3 and 4).
- Not yet:
  - argument-sensitive member sites (ADR 0280's walker; slice 2);
  - object spread and `for…in` copies of the parameter;
  - constructors and `this.method` dispatch;
  - forwarding the parameter to another helper;
  - computed literal keys (`o["a"]`);
  - getters that call a project helper or read a member;
  - dropping the getter's residual uncertifiable read.

## Evidence

- Fixture `reactive-ir/getter-parameter-read`: ten proven calls, four
  clean controls and 37 refusals that stay uncertifiable, plus the
  `CallbackBranch` control. `tsc --noEmit` passes. No other fixture moved.
- Unit tests in `property_gets.rs`:
  `a_parameter_yields_gets_only_when_every_reference_is_a_static_get`,
  `a_getter_read_counts_only_on_the_getters_every_completion_path` and the
  `eval` rows of `named_receivers_use_binder_identity_and_refuse_escapes`.
- Cache: `sessions_process::incremental_result_cache_follows_a_helper_parameter_get`
  edits only the helper (`return o.a` to an alias and back) and compares the
  retained result with a fresh build. It fails when the `FunctionSource`
  dependency is removed.
- Runtime, using probus-hk's installed rc.13 development runtime
  (`rust/target/research/adr-0283/runtime.mjs`, `--conditions=browser
  --conditions=development`):
  - the literal, named-receiver and second-Get misuses each raise one
    `STRICT_READ_UNTRACKED`;
  - `untrack`, `createMemo`, `readsBranch(false, …)` and another key
    raise none.
