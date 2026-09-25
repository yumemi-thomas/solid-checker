# A returned ancestor does not establish descendant execution

`Direct` returns the callable that invokes its parameter. `Dormant` instead
declares a nested mapper that nobody invokes. `Invoked` calls its mapper, but
needs an exact local invocation chain to establish its callback's execution.
Lexical containment inside the returned function alone proves neither case.

The corpus must preserve the directly returned callback and keep unsupported
descendant execution unknown. `Direct`'s row is `queued` and
`ambient-at-execution`: the returned closure runs on its caller's stack, so
"after the export returns" proves nothing about the listener (ways-to-improve
step 7). The native verifier must reject transplanting
the direct callback operation onto `Dormant`.
