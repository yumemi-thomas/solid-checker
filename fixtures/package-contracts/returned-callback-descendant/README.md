# A returned ancestor does not establish descendant execution

`Direct` returns the callable that invokes its parameter. `Dormant` instead
declares a nested mapper that nobody invokes. `Invoked` calls its mapper, but
needs an exact local invocation chain to establish its callback's execution.
Lexical containment inside the returned function alone proves neither case.

The corpus must preserve the directly returned callback and keep unsupported
descendant execution unknown. The native verifier must reject transplanting
the direct callback operation onto `Dormant`.
