# async-carrier-call-attributed

**Claim (ADR 0564).** ADR 0462/0490's call-attributed walk (an async read in a named local function, read where that function's caller runs) climbs out of a named nested function no named-callback role places instead of taking its lexical role (a flow child's, which `async_execution_role` promotes to rendered JSX), and follows the inline carriers that function is handed to by name, anchoring the row at the carrier; ADR 0563's predicate decides whether the carrier runs it.

Violations (SC5001 at the carrier): `FlowForCarrier` (`[1].forEach(hf)` in a `<For>` child), `FlowForCall` (`hf.call(null)`), `BodyCarrierTagged` (``hf`x` `` in the body).

Uncertifiable, never a violation: `FlowCarrierEmpty` (an empty receiver), `FlowShadowedForEach` (a project `forEach`), `FlowLocalReflect` (a local `Reflect`), `FlowMemberAssigned` (a member's value), `FlowNeverCalled` (ADR 0509's net). No SC5003 row (each had one on 644bd1a81): `FlowCarrierTimer`, `FlowNeverCalled`, `FlowMemberAssigned`. Silent for SC5001: `FlowCarrierInMemo` (a memo compute suspends); its SC1001 is uncertifiable (ADR 0920).

Runtime: rc.17 throws `PENDING_ASYNC_UNTRACKED_READ` at each violation (fuzzer cells mx-recall PENDING_ASYNC_UNTRACKED_READ flow-* × wrapper × handoff-{foreach,…,tagged}). Stub: `async-unsuspendable-call-attributed`'s. `tsc --noEmit` clean.
