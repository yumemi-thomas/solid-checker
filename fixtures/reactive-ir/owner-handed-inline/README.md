# owner-handed-inline

**Claim (ADR 0561).** A function a reviewed standard-library carrier runs before it returns (`runtime_semantics`'s `InlineCallback` rows: `forEach`, `map`, `sort`, `Array.from`'s map function, `Reflect.apply`'s target) or a tagged template's tag runs on the caller's stack, under the owner of the code around the call, as a direct call of it would. The owner pass gives it that edge.

Violations: `missing-context-provider` (SC4002) in `ForEach`, `MapLiteral`, `FromMapper`, `SortComparator`, `ReflectApply`, `TaggedTemplate`, `NamespaceRead` and `Reassigned` (a written `let`, as its direct call is: ADR 0468's may-run edge); `missing-owner` (SC4001) in `HandlerForEach` (a click handler has no owner, and the carrier keeps it) and `Timer` (a fresh-stack scheduler, unchanged).

Silent: `Provided` (a provider above the reader).

Obligations (`missing-owner` uncertifiable): `ShadowedForEach` (a local `forEach` is not the standard library's), `MemberMap` (a project `map` method), `GeneratorHanded` (a generator runs none of its body when called).

Premises: the closed program is the `private` manifest. Runtime: rc.17 throws ContextNotFoundError at each SC4002 line (fuzzer cells `mx-recall` CONTEXT_NOT_FOUND `handoff-{foreach,map,from,sort,reflect,tagged}`).
