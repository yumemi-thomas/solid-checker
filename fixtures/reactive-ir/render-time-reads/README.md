# render-time-reads

**Claim (ADR 0204).** Three reads that run while a component renders, and that the checker used to leave uncertifiable or miss, are proven `strict-read-untracked` violations:

| Case | Finding | Why |
| --- | --- | --- |
| `outer()` | `SC1001` violation | `outer` calls `inner` directly in its body, and `inner` reads `count` directly in its body |
| `withDefault()` | `SC1001` violation | the call omits the argument, so the default `count()` runs during the call |
| `<Render label={() => count()} />` | `SC1001` violation | `Render` calls `props.label()` directly in its body |
| `withDefault(2)`, `lazyDefault()`, `deferred()` | none | the default does not run, only builds a function, or the read is in a closure nobody calls |
| `useBoth()` | `SC1001` violation | the same signal is read in a nested default and directly in the hook's body; the finding keeps the body read |
| `useFinisher()` | none | the read is in the default of a function the hook builds and never calls; it lies in the hook's body but in no code that runs there |
| `<RenderLive label={…} />` | none | `RenderLive` calls the prop only in JSX, which tracks (ADR 0218) |
| `<Render label={…} {...rest} />` | `SC1001` uncertifiable | a spread could replace the prop |

The `SC1001` on `props.label` in `Render` belongs to the props rules and is not part of this claim. The stubs are copied from `feedback-tiers`.
