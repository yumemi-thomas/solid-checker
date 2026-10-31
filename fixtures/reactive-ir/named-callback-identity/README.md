# named-callback-identity

`strict-read-untracked` (SC1001) for a function a primitive's callback position names through a binding other than the function's own: an exact `const` copy of its binding (`const apply = read; createEffect(c, apply)`), or the `const` a named function expression initializes (`const apply = function step() {}`, whose own name `step` is scoped inside it). For a copy: The copy names the function's value, as the solid-facts `local_alias_bindings` fact records only for a non-exported `const` whose initializer is the bare binding of a `const` (ADR 0463), so the named-callback index gives the copy the function's entry, following chains. Before, the copy named no function, the function looked never called, and its read was a silent clean (fuzzer recall grid, `callback:alias`, 2026-10-31).

Violations (rc.17 reports STRICT_READ_UNTRACKED at each, Chrome-witnessed by the recall grid):

- `AliasApply` (line 6): a copy as a `createEffect` apply.
- `AliasChainRenderApply` (line 16): a copy of a copy as a `createRenderEffect` apply.
- `NamedExpressionApply` (line 60): a named function expression's binding as a `createEffect` apply (fuzzer `callback:named`).

No SC1001:

- `AliasCompute` (line 27), `AliasMemo` (line 38): a copy as an effect's tracked compute or a memo's compute, where a read tracks.
- `NamedExpressionCompute` (line 69): the same binding as a tracked compute.

Uncertifiable, never SC1001 (ADR 0516: a function handed through a copy no callback edge follows has unplaced reads, a cost on these two):

- `AliasUsedBeforeInitialized` (line 81): the copy is handed to the effect from a nested function called before the copy's declarator runs, which throws a ReferenceError (no read); a copy names its function only when every reference to it follows its declarator in the declarator's own function.
- `LetSourceApply` (line 49): a copy of a `let`, which is not an alias fact; here the `let` was reassigned before the copy, so the apply holds the empty function and no read runs.

Declaration stubs: `callback-containment`'s `solid-js.d.ts` (published 2.0.0-rc.9 and rc.13 typings, as recorded there).
