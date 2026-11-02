# handed-residuals

ADR 0537: four residual families of the full-6 grid (2026-11-01), each an rc.17 misuse the checker certified clean or gave the wrong rule.

1. Writes through a helper: `wrapSet()` in a named function handed to `[1].forEach(hf)` or project code inside `runWithOwner` in a handler, or a member's function value in a handler's root. WriteOwnerForEach, RefreshOwnerProject and WriteRootMember are obligations, as the direct write there is (ADR 0535). WriteHandlerForEach stays legal.
2. A tagged template calls its tag in a leaf owner's callback: LeafTagged is SC3001; LeafTaggedInHelper is an obligation (the template is in no completion census). LeafTaggedPlain and LeafTaggedShadowed stay legal.
3. A hook-result async read in a named function handed on inside `<Show>` (mounted by `main.tsx`, ADR 0504): HookRunShow and HookMemberShow are obligations, not SC5003; DirectRunShow keeps its obligation and HookJsxShow its SC5003.
4. Reads after an await: AwaitDepth15 (15 in-place layers) and the `call`, `apply` and tagged hand-offs of a helper are SC1002; AwaitLoopTagged is an obligation; HandlerAwaitCall stays legal.

The stub is strict-read-async-apply's, plus rc.17's `createRoot`, `createOwner` (its `Root` reduced to an empty `Owner`), `runWithOwner` and `refresh`. App.tsx type-checks against published solid-js rc.17 with strict ES2022 settings. Package bytes are not copied here.
