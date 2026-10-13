# helper-prop-read

**Claim (ADR 0222).** A component-local helper's own prop Get counts at each
exact local call proven to run the helper's body (ADR 0201, 0204, 0221
directness). That row has the call's execution role and the authored Get as
its origin. Its backing is the ADR 0216 `PropsReactivityIndex` answer, after
the ADR 0221 prop-head root guards. Defaults keep their ADR 0204
authored-site proof.

An unproven use (an escape, a JSX prop, an unproven call) adds no new
finding: the read keeps whatever the definition site answered before this
path existed. A helper called only in tracked, sampled or event scopes adds
nothing.

The table is the checker's SC1001 result on this fixture. "Uncertifiable"
never means a runtime violation was established. Independent mutation,
dispatch, async and structural findings can also exist.

| Case | SC1001 | Why |
| --- | --- | --- |
| `ReactiveBody` | 2 violations | exact synchronous Get; each call is UntrackedRendering |
| `ReactiveApply` | violation | call is in the effect apply callback, not compute |
| `NamespaceApply` | violation | namespace primitive resolved by identity |
| `MixedRoles` | 2 violations | body and apply calls retain distinct roles; JSX, memo and sample calls add nothing, including no definition-site obligation |
| `StaticBody`, `StaticApply` | none | this property's backing is Static |
| `UnknownApply` (both callers) | uncertifiable | helper-backed and unknown external incoming values have no reactive witness |
| `MixedCaller` | violation | a direct signal caller witnesses this prop; Unknown at another caller does not erase that witness |
| `PerProperty` | violation | a live sibling never makes `plain` reactive |
| `PlainInner` through `PlainMiddle`, `PlainOuter` | none | the existing forwarding fixpoint proves the exact head static |
| `ForwardedHelper` through `Forwarder` | violation | cross-file forwarding keeps a witnessed reactive head |
| `RootEscapes` | uncertifiable | passing the props object whole opens root identity |
| `AliasedRoot` | uncertifiable | an alias is not the exact props parameter |
| `WrittenRoot`, `WrittenProperty` | uncertifiable | incoming classification cannot certify a replaced root/property |
| `MergedRoot` | uncertifiable | a merge view can supply different keys |
| `DynamicKey` | uncertifiable | no dynamic key is resolved by its spelling |
| `LiteralKey` | violation | the cooked literal names the exact property |
| `WrappedRoot` | none | existing transparent-wrapper resolution is queried, never bypassed. Not proven yet: a recall gap. |
| `WrappedCall` | none | the current raw `direct_callee` fact rejects this cast call; no new wrapper-specific call proof. An unproven use adds nothing, as at HEAD (ADR 0222). |
| `PassedHelper` | violation | a value argument escapes without invalidating the immutable direct target |
| `ReturnedHelper` | violation | returning the helper opens a separate timing obligation |
| `StoredHelper` | violation | shorthand storage opens a separate timing obligation |
| `AliasedHelper` | none | alias dispatch does not prove this new prop-call path. An unproven use adds nothing, as at HEAD (ADR 0222). |
| `HandlerHelper` | none | the exact compiler EventHandler value span proves the use non-strict |
| `JsxPropHelper` | none | JSX passes a function; it does not prove when its body runs. An unproven use adds nothing, as at HEAD (ADR 0222). |
| `TrackedJsx` | none | a compiler-proven tracked call is not promoted to a strict read |
| `MemoCompute`, `EffectCompute` | none | tracked compute retains its own role; no violation proof |
| `Sampled` | none | the exact untrack boundary supplies a non-strict role |
| `EventCall` | none | the exact event callback and synchronous own-body site prove a non-strict invocation despite containing JSX |
| `MixedNamedCaller` | none | the named event handler also has a body-time call; its deferred named role must not silently erase the strict path. An unproven use adds nothing, as at HEAD (ADR 0222). |
| `AsyncPrefix` | violation | the leaf and caller both pass the shared synchronous-prefix proof |
| `AsyncSuffix` | none | the Get follows an own await. An unproven use adds nothing, as at HEAD (ADR 0222). |
| `SuspendedCaller` | none | a possible await occurs before the call in its immediate caller. An unproven use adds nothing, as at HEAD (ADR 0222). |
| `SuspensionLoop` | none | a later iteration can evaluate the Get after suspension. An unproven use adds nothing, as at HEAD (ADR 0222). |
| `GeneratorHelper` | none | a generator call does not enter the body. An unproven use adds nothing, as at HEAD (ADR 0222). |
| `NestedRead` | none | constructing/returning a closure proves no invocation of its Get. An unproven use adds nothing, as at HEAD (ADR 0222). |
| `NestedDefault` | violation | keep HEAD's existing authored-site ADR 0204 proof; no projected call row |
| `Transitive` | none | this patch refuses a second prop-call level rather than adding another chain walker. An unproven use adds nothing, as at HEAD (ADR 0222). |
| `ShadowedTarget` | none | the outer helper has no uses; the inner call names a different exact declaration |
| `ShadowedFor` | none | a local For is not a dialect primitive; its callback receives no primitive entry proof. |
| `MethodCaller` | none | method dispatch does not establish the new prop-call entry path; the method's own body is not rejected merely for being in its class. An unproven use adds nothing, as at HEAD (ADR 0222). |
| `ReactiveCondition` | none; `components-return-once` violation | the proven controlling call read is reported by `components-return-once`, which owns this site |
| `UncertainCondition` | none; `components-return-once` violation | escape does not erase this proven call; `components-return-once` reports it |
| `UncertainPreference`, `UncertainList` | none | timing uncertainty is in `read.uncertain`, which the preference read index gates. An unproven use adds nothing, as at HEAD (ADR 0222). |
| `UnusedHelper` | none | absence of a runtime reference creates no timing obligation |
| `DeclarationBody` | violation | validity is demanded for the exact nested function declaration's zero-argument call |
| `NamedSampled`, `NamedTracked` | none | exact primitive callback value uses prove sampling/tracking; no guessed dispatch |
| `NamedApply` | none | effect apply's Deferred word still means a strict-read window; the helper returns void, so this is not a TypeScript cleanup-return error. Not proven yet: a recall gap. |
| `TrackedAndEscaped` | none | tracked call adds nothing; only the retained function value opens timing. An unproven use adds nothing, as at HEAD (ADR 0222). |

`solid-js.d.ts`, `tsconfig.json`, all three node_modules manifests, and the
signals `index.d.ts` were copied byte-for-byte from `prop-head-get`.
`memo.d.ts` adds the rc.13 `MemoOptions` and `createMemo` declarations copied
from `render-time-reads/solid-js.d.ts`; the base stub is unchanged. These
declarations retain the exact compute/result and effect cleanup signatures.

Type review: every effect apply callback returns void, every async helper has
a compatible Promise result, reassigned helpers keep the same `() => string`
signature, dynamic keys are restricted to declared keys, and JSX callbacks
have explicit compatible signatures. No async function is used as an effect
apply callback. The project enables strict mode without unused-local checks.
The integrated fixture's invalid `WrittenDeclaration` was already removed:
reassigning a function declaration would duplicate TypeScript TS2630. The
remaining `WrittenHelper` uses a reassignable `let` with a compatible signature.
TypeScript and published-typing checks were prohibited in this research, so
this review does not claim they ran or passed. The integrating agent must run
both before promoting any snapshot.

The original `error-menu-apply-browse-helper-prop` misuse twin uses the
`getProjectId()` incoming expression: predict SC1001 **uncertifiable** at
`browse("")`. Its untrack twin should stay without SC1001. A twin/caller
variant using direct `projectId()` should be a **violation** at that call.
