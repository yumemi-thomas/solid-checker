# Returned package values and callback provenance

Keeps direct, generic, wrapped and reactive return relationships. ADR 0023 changes
`createDeferredProxy` only: storing a constructor parameter in a retained handler does not prove when
that parameter is invoked, so the callback is uncertifiable (SC9005 at its constructor argument) rather
than acquiring a queued invocation claim from retention. The finding concerns execution timing, not a
TypeScript error: the Proxy handler and constructor use ordinary standard-library types, and the isolated
example passes pinned TypeScript 5.9.3 with `--strict --noEmit --target ES2022`.
