# ADR 0221: A prop head and an async prefix run at the call

- Status: accepted and implemented (2026-10-07).
- Owners:
  - G1: `local_access` (the prop head Get) and
    `execution_role::direct_control_flow_body_role`;
  - G2: `interproc::body_site_runs_during_call` and the activated-default
    collector in `interprocedural_result_reads_for_file`;
  - the shared store-key question `Dialect::store_key_warns_strict_read`,
    answered only for exact `@solidjs/signals` rc.13.
- Relation: extends ADR 0201 (reads proven to run during a call), ADR 0204
  (defaults) and ADR 0216 (per-property forwarding). Each is queried, not
  replaced.

## Context

Of the 18 runtime-confirmed corpus twins
(`fixtures/app-patterns-misuse/corpus-twins.json`), the checker proved 4,
left 11 uncertifiable and was silent on 3. Two shapes covered six of them:

- **G1:** `props.snapshot?.messages[i]` evaluates `props.snapshot` first.
  The analysis recorded only the longest member path, so the head Get, the
  read the runtime warns about, was never a read.
- **G2:** an async function called without `await` runs its body on the
  caller's stack up to the first possible suspension. ADR 0201 refused every
  async callee, and ADR 0204 every nested helper's default.

## Decision

1. **The head of a props member chain is a Get.** The receiver must be an
   exact identifier with only transparent TypeScript wrappers peeled. Its
   key must be the static name or the parser's cooked literal; a dynamic key
   is unknown. The ADR 0216 fixpoint answers whether that property is
   reactive, static or unknown. A view, an alias, or a root that is written,
   deleted, iterated or escapes is unknown.
2. **The exact children literal of a dialect control-flow primitive runs
   while rendering.** The tag must be the primitive by symbol identity or as
   a namespace member, never by spelling. The literal must be its sole
   child, with no spread or `children` attribute, and the site must be in
   the literal's own body. This admits a read; it assigns no role.
3. **An ordinary call of an async project function runs its prefix.** The
   call must be exact, plain and outside JSX, and the target's value must be
   the declaration. A read in the callee's own body runs during the call
   only if:
   - it ends before every `await` and implicit suspension of that function;
   - no loop around it contains a suspension;
   - a suspending function has no own `switch` test.

   Binding patterns, assignment targets, class bodies written inside the
   function, and JSX are refused. The same holds at every transitive hop, and
   generators are refused.
4. **An omitted argument activates its default at this invocation.** This
   holds only for an exact plain call with no spread. Accessor calls and
   store Gets written directly in the initializer are direct rows, with the
   authored initializer as their origin. A default derives no global
   parameter taint.
5. **New async and default paths need a runtime source witness:** an
   unwritten binding initialized by a dialect source primitive. Type-only
   `Accessor` or `Store` labels are not witnesses. A store Get needs a known
   string key the dialect says warns: `then`, symbol and dynamic keys are
   unknown.
6. **Two existing gates were tightened along the way.**
   - A JSX tag that resolves to another symbol, such as a local `For` that
     shadows the import, names no primitive (`jsx_primitive_name`). Before
     this, such a child was a proven violation: a false positive
     (`shadowed-control-flow-tag`).
   - `components-return-once` reports a violation only for a proven read in
     the controlling test. An uncertifiable read there now gives an
     uncertifiable finding.
7. **A changed file holding call-role syntax disables result-cache reuse.**
   A moved `await` changes directness while the summary rows can stay equal.
   The edited, added or removed file counts if, before or after the edit, it
   holds:
   - an async function;
   - a parameter default;
   - a function declaration whose name is written;
   - a function held by a non-`const` binding.

   Other edits reuse as before (`interproc::call_role_syntax`).

## Consequences

- Twins: 4 proven, 11 uncertifiable, 3 silent became 10 proven,
  8 uncertifiable, 0 silent. No correct twin gained an `SC1001` finding.
- rc.13 corpus, release binary, browser host, against ADR 0219:
  - violations rose from 195 to 222. Each added `SC1001` site was checked
    against its caller, and each passes a reactive value (`r()`, `pick()`,
    `lastStroke()`, a non-keyed `For` item);
  - five `components-return-once` violations became uncertifiable;
  - uncertifiable rose from 3,368 to 3,443, because every `props.a.b` head
    with an unknown backing is a new obligation;
  - `prefer-show` and `prefer-for` gained findings where a prop head now
    proves a reactive condition or list.
- The base misuse ledger is unchanged: 31 of 33 detected at runtime and 29
  proven violations.
- Not yet:
  - `Cooked` (an escaped literal key behind `as`) and `WrappedExactCall`
    remain unproven positives;
  - await operands, pattern sub-defaults, explicit `undefined`, transitive
    defaults, prop-backed defaults, and async member, JSX or class entry are
    not proven.

## Evidence

- Proposal and risk review by a Codex agent:
  `rust/target/research/volume/prefix/` (README.md, risks.md,
  corpus-impact.md). The integration found and fixed three defects the
  proposal did not run:
  - the class-region refusal also refused a method's own body;
  - a shadowed `For` was still treated as the primitive;
  - uncertain reads drove `components-return-once` violations.
- Session test `incremental_result_cache_follows_a_moved_await`: a warm
  session equals a cold build across before → after → before → after
  edits of one callee.
- Fixtures: `prop-head-get`, `async-call-prefix` and
  `shadowed-control-flow-tag`. Each type-checks against the published rc.13
  typings and against its stub.
