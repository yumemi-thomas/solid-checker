# returned-store-source

Research fixture intended for `fixtures/reactive-ir/returned-store-source/`.
The claim is a runtime source identity returned from one exact project function:
all normal completions return the same unwritten binding initialized directly
by a dialect source primitive in that function invocation. Caller bindings are
distinct source identities, even when they call the same function.

These are source-derived predictions, not captured checker results. `SC1001`
below refers to the authored consumer read, except where explicitly stated.
"none (refused)" does not certify the read as static: it records a remaining
missing-source gap. Existing callable/type/contract paths are not withdrawn.

| Case | SC1001 | Why |
| --- | --- | --- |
| `LocalStore` | violation | control: direct local store Get used as a signal seed |
| `SameFile` | violation | exact same-file hook, bare returned store binding |
| `CrossFile` | violation | named import resolves to the hook declaration in `hook.ts` |
| `NamespaceImport` | violation | actual namespace import and exact exported property symbol |
| `ExportAlias` | violation | exported alias resolves to the same declaration |
| `ArrowAndPrimitiveNamespace` | violation | immutable arrow function and namespace source primitive |
| `SameIdentity` | violation | conditional early return and terminal return name one binding |
| `ExhaustiveIdentity` | violation | both exhaustive branches return one binding |
| `TransparentReturn` | violation | parentheses and `as` preserve the exact value |
| `ReturnedCount` | violation | returned signal getter; already supported by existing accessor paths |
| `FreshCalls` | two violations | `first` and `second` have distinct symbols and caller declaration locations |
| `Sampled` | none | explicit `untrack` samples the seed |
| `Tracked` | none | compute callback and JSX Gets are tracked; apply only receives a plain string |
| `DifferentIdentity` | none (refused) | two different bindings, even though both are stores |
| `PlainAlternative` | none (refused) | a plain return is not the same source binding |
| `Fallthrough` | none (refused) | implicit undefined completion; consumer narrows before reading |
| `BareReturn` | none (refused) | explicit undefined return; consumer narrows before reading |
| `AsyncReturn` | none from this proof | binding holds a Promise; fulfilled store is read in its callback, not as Promise.search |
| `GeneratorReturn` | none (refused) | generator result is an iterator completion; `done` is narrowed before reading its return value |
| `WrittenBinding` | none (refused) | source root is mutable and replaced with a plain value |
| `PassedThroughCall` | none (refused) | `return identity(state)` is a call, not the source binding |
| `EscapedRoot` | none (refused) | root is passed to another call before the bare return |
| `EscapedAlias` | none (refused) | root acquires an alias that escapes; no untracked alias trust |
| `ReturnedMember` | none at `items.length` from this proof | `return state.items` is a Get, not a bare root; the existing read summary may report the factory invocation |
| `FinallyReplacement` | none (refused) | try/finally can override the completion; no completion cover |
| `UnmodeledLoop` | none (refused) | bounded proof does not model loops |
| `CapturedNotFresh` | none from this proof | the source belongs to the module, not this invocation |
| `PlainAndUnresolved` | none from this proof | structural object types establish no reactive Get; declaration-only hook has no implementation |
| `ShadowedHook` | none | same spelling, different exact function and plain result |
| `ShadowedNamespace` | none | local `Hooks` object is not the namespace import |
| `ShadowedPrimitive` | none | local `createStore` names no dialect primitive |
| `ReassignedFunction` | none (refused) | imported declaration's binding is reassigned in its owning file |
| `ReassignedLocalFunction` | none (refused) | a mutable function binding cannot authenticate its initial function value |
| `WrittenCaller` | none (refused) | the caller root is replaced; immutable call-result gate rejects it |
| `OptionalCaller` | none (refused) | optional call and conditional callable identity are outside this slice |
| `ConditionalReturnConsumer` | violation | proven store read in component body condition; also predicts SC1004 `components-return-once` |
| `ShowPreference` | none | JSX is tracked; predicts SC8015 `prefer-show` on its reactive branch |
| `ForPreference` | none | JSX is tracked; predicts SC8014 `prefer-for` on standard Array.map of a reactive store member |

`App` renders every case to supply actual JSX component use evidence.
The guard cases deliberately use plain structural return types where possible,
so a type-only source label cannot conceal the missing runtime proof. An existing
typed accessor or accepted contract can still independently classify a refused
value; refusal of this proof must not suppress those existing findings.

`solid-js.d.ts`, `tsconfig.json`, and every file under `node_modules/` are copied
byte-for-byte from `prop-head-get`. That stub states which rc.13 signatures are
byte-faithful and which options/overloads were reduced. All calls here stay
within those published signatures: no extra effect argument, invalid cleanup,
access of a Promise as a store, or unnarrowed optional/iterator value is used.
Static type review only: no `tsc` was run, including against real rc.13 types.
The integration lead must verify both type environments before committing.

Cache regression to run during integration: retain a session for this project,
then edit only `hook.ts`'s `useLocation` to return a plain object (keep the same
structural return type and similar spans). `CrossFile`, `NamespaceImport`,
`ExportAlias`, and `FreshCalls` must lose the newly proved reads; restore the
bare store return and they must regain them. Repeat with a changed return
binding, a new plain alternative, a write, and removal of the callee file.
Compare incremental results with a new session each time, including the three
read-set consumers above. Do not assert correctness from equal read summaries.
