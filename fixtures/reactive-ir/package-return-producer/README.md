# Returned package values and callback provenance

The fixture keeps direct, generic, wrapped and reactive return relationships.
ADR 0023 changes createDeferredProxy only: storing a constructor parameter in
a retained handler does not itself prove when that parameter is invoked.
The callback is now explicitly uncertifiable (SC9005 at its constructor
argument), rather than acquiring a queued invocation claim from retention.
Proving the constructor-to-trap execution relationship is future work.

The new finding concerns execution timing, not a TypeScript error. The Proxy
handler and constructor use ordinary standard-library types; no fixture Solid
signature supplies this conclusion. Other return cases retain their findings.

The isolated Proxy/handler example passes the repository's pinned TypeScript
5.9.3 with --strict --noEmit --target ES2022 against its real standard library.
