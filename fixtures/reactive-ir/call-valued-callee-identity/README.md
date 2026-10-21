A callee that is itself a call names the value that call returns, and nothing
in the callee's span names that value (ADR 0346). The entity and the resolved
call at `make(() => {})` describe the inner call, so `make(() => {})(() => s(1))`
must not treat `make` as the callee of the outer call, which would make
`() => s(1)` run inline in the component. Those two negatives, and the
`const g = make(() => {}); g(() => s(1))` control whose `make` result no fact
models, are unknown (uncertifiable), never a violation. The identifier callees
`ok` and `make` with a direct argument are the positive controls: each runs
its argument inline, a violation.
