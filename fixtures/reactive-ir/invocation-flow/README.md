# invocation-flow

**Claim (ADR 0319).** In a closed program, code in a synchronous non-component function literal takes the one role every invocation runs in, found by following the literal forward through `const` bindings, object members/spreads, returns, exact project callee arguments, component props and `on...`/spread attributes on intrinsic elements; invocations must agree and write legality never asks. Event-only flows (and mixed non-reporting roles, ADR 0323) are clean; `this` siblings, effect apply, body, async, reassigned, opaque and hazard flows are `SC1001` uncertifiable.
