# ref-factory-call

**Claim (ADR 1209).** `ref={dir()}` on an intrinsic element is a two-phase ref: the compiler evaluates the factory call inline, exactly once, where the element's markup is evaluated (`RefFactory`, schedule `Inline`), and runs the function it returns later in the directive-apply phase. When no other compiler operation contains the call (the markup is evaluated in the component's own body), the call runs during the body (`body_site_runs_during_call`'s `inline_ref_factory_site`), so a prop read in `dir`'s body, directly or in an IIFE, is untracked in the render: SC1001 violation at `dir()` (Chrome: STRICT_READ_UNTRACKED at the read).

Violations: `Read`, `ReadIife` (ADR 1184's census shape, uncertifiable before). Clean: `Untracked`. Uncertifiable (fail closed): `InChild` (the element is a component's child: its markup is evaluated where the children are read), `Conditional` (one arm of a conditional ref: not an inline exactly-once factory).
