# ADR 0285: A forwarded parameter is called where its target calls it

- Status: accepted and implemented (2026-10-13).
- Owner: `SemanticLookup::parameter_completion_calls` (indexes.rs) and the
  call contribution in `interproc.rs` that records `invoked_parameters`.
- Found while analyzing a package's runtime source as project code
  (ADR 0284): `useLabel(source) { return readNow(source) }` was silent.

## Context

ADR 0277 claims an accessor passed by reference, `run(count)`, is read at the
call when `run` calls that parameter on every completion of its own body.
Both halves only saw a *direct* call of the parameter. In
`function f(cb) { return g(cb) }`, `f` never calls `cb` itself, so `cb` was
not in `f`'s `invoked_parameters`. The caller-side loop then never asked
about it, and `f(count)` in a component body reported nothing, not even an
obligation. This holds in plain TypeScript, not only in package JavaScript.
Compiled packages thread callbacks through helpers this way all the time.

## Decision

`f` calls its parameter `p` on every completion when a call `g(…, p, …)`
meets all of these:

- the call is written directly in `f`, on `f`'s every-completion path;
- the argument is the identifier `p` itself, unwritten, at an exactly mapped
  position (no earlier spread, an exact callee:
  `project_callee_at_parameter`);
- `g` is a function **declaration**;
- `g` calls that position on every completion, directly or by this same rule.

The rule is applied to a fixpoint, so a chain of forwards composes. The call
contribution then records `p` as invoked by `f`. The existing ADR 0277 path
claims the read at `f`'s caller, with no change to its own rules.

`g` must be a declaration because ADR 0218's review round 3 found a `const`
callee that may be in its temporal dead zone when the forward runs. A
declaration is initialized when its module is instantiated, even inside an
import cycle. The `project-callback-uses` case `ForwardedInlineBody` pins the
`const` refusal.

Summary parameter positions skip destructured parameters, but the completion
table's positions do not. A forward is therefore admitted only when no
destructured parameter precedes `p`.

## Consequences

- One-hop forwards and chains of them become proven SC1001 violations at the
  outer call.
- These stay silent, as a forward that is not proven to run was before:
  - a forward behind a branch, inside a closure, or in a timer;
  - a forward into a helper that calls its parameter on some paths only;
  - a forward into a `const` arrow;
  - a forward after a destructured parameter.
- No cache change. The completion table is already bound into
  `cross_file_proof_digest` (ADR 0277), so a fact this rule adds or drops
  moves the digest.

## Evidence

- **Fixture** `reactive-ir/parameter-forwarding-completion`: 2 violations,
  1 clean, 6 silent refusals. `tsc --noEmit` is clean.
- **Fixture** `reactive-ir/dependency-source` (ADR 0284) needs this rule for
  its `LabelNow` case.
- **Coverage:** no other snapshot moved.
- **rc.13 corpus:** release binary, browser host. Base mode is unchanged at
  562 violations and 3,731 uncertifiable. Dependency mode is also unchanged.
