# pending-read-closed-bundle

**Claim (ADR 1009).** ADR 0667 opens a possibly async read written in a function that is an object member, because nothing says where such a function runs. A function handed from a closed bundle has a place: it is the value of a plain data property of a `const` object literal, the object is not exported (no declaration, `export { x as y }`, `export default x`, no script-global), and every reference to the object is a non-computed read of that one key handed whole to a primitive, never a write. The property must also be the key's FINAL value: no spread after it in the literal, no later computed or otherwise nameless key, no later property of the same key (the review's F1; `property_is_final_value`, shared with `origin_member_ignores_receiver`). Then the primitive's callback position is the only thing that runs it, and a read in it (or in a closure, an IIFE or a helper's callback written inside it) takes that position's role.

Violations (SC5001, PENDING_ASYNC_FORBIDDEN_SCOPE; Chrome reports each): `Settled`, `Tracked`, `NestedClosure`, `NestedLiteral`, `TwoProperties`, and the hand-off written through sugar or a namespace: `NonNull` (`bundle.key!`), `CastAny` (`bundle.key as any`), `NamespaceCall` (`S.onSettled(bundle.key)`).

Obligations (`invocation-context-unproven`), with what Chrome does:
- Chrome silent, so a violation would be a false positive: `SpreadAfter` (`{ run, ...extra }` replaces it), `ComputedAfter` (`{ run, [k]: … }`), `ShadowedLocal` and `ShadowedParameter` (`onSettled` is a local or a parameter: not the primitive), `NotPrimitive` (`keep(bundle.key)`).
- Chrome reports, the checker keeps the obligation (known misses, fail closed): `ObjectPassed`, `PropertyWritten`, `ComputedKey`, `LetObject`, `Method`, `MemberAlsoCalled`, `MemberAliased`, `NestedBundle`, `WrappedInArrow` (`onSettled(() => bundle.key())`), `QuotedKey` (`{ "run": … }`), `Destructured` (`const { run } = bundle`), `SpreadBefore` and `ComputedBefore` (a spread or computed key earlier in the literal), the exported module-level bundles (`exportedBundle`, `renamedBundle`, default) and the unexported module-level `moduleBundle`.

No violation: `TrackedCompute`, the same bundle handed to a memo's compute, which may suspend.

Runtime: rc.21 Chrome, oracle case `fixtures/runtime-oracle/pending-read-closed-bundle`. Premises: the closed program is the `private` manifest.
