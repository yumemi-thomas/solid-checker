# Exact project namespace exports (NS)

Candidate cross-file fixture; not executed. Only the namespace import binding
and the canonical symbol at the property's own token establish a target. Names,
a compatible signature, or a same-shaped object do not.

| Case | Role | Expected leaf finding |
| --- | --- | --- |
| NamespaceForbidden | positive | SC3001 violation: exact exported function registers cleanup |
| NamespaceConstForbidden | positive | SC3001: const has a direct arrow initializer |
| NamespaceClean | negative | none: exact exported function is plain |
| ReexportForbidden | re-export | SC3001 when the existing canonical alias index resolves renamed to forbidden |
| WrappedNamespaceForbidden | wrapper | SC3001: wrappers preserve receiver and property identity |
| ShadowedNamespace | shadowing | SC9012 uncertifiable: local object is not namespace binding |
| ComputedMember | computed boundary | SC9012: computed calls deliberately refused, including const keys |
| AliasedNamespace | alias boundary | SC9012: first slice requires the import binding itself |
| MutableExports | live binding / opaque wrapper | SC9012 for each call: mutable/reassigned values and wrap result are not exact |
| UnspecializedCallback | callback boundary | SC9012: no argument-to-helper-parameter specialization |
| DefaultParameterStillOpen | default parameter | SC9012: current walk conservatively keeps default evaluation open |

Expected findings are predictions, not observed snapshots. If the wrapped or
re-export property has no exact fact, the required fallback is SC9012, never a
name-based target or a clean result.

solid-js.d.ts is copied byte-for-byte from leaf-scope-host-callbacks. The
cleanup/callback signatures are unchanged; real published-type tsc validation
has not run. Before promotion add per-fixture .gitignore exceptions and record
only reviewed coverage output. No snapshot is supplied.
