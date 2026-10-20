# component-function-child

**Claim (ADR 0326).** A function written as a component's sole child is that component's `children` prop; the compiler's `Deferred` projection says nothing about when it runs, so ADR 0319's walk follows it through `children`: called in tracked JSX or an event handler it takes that role (clean); otherwise `SC1001` uncertifiable (ADR 0328), including body calls and the review shapes pinned by `prop-body-call-guard`. Control-flow callbacks are unchanged. Oracle: `runtime-oracle/oracle-children-body`.
