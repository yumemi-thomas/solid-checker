# Exact project namespace exports (NS)

Only the namespace import binding and the canonical symbol at the property's own token establish a target (names and shapes do not): `NamespaceForbidden`, `NamespaceConstForbidden`, `ReexportForbidden`, `WrappedNamespaceForbidden` are SC3001 violations, `NamespaceClean` none; shadowed namespaces, computed members, aliased namespaces, mutable exports, unspecialized callbacks and default parameters are SC9012 (never a name-based target or a clean result). `solid-js.d.ts` is byte-for-byte `leaf-scope-host-callbacks`'.
