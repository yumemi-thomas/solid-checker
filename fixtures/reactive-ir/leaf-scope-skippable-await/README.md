# leaf-scope-skippable-await

**Claim (ADR 0320, review round 7).** The leaf-owner walk
(`cleanup.rs`) skips a call that follows an `await` its helper reaches on
every path: it runs from a promise continuation, which is `missing-owner`'s
question. An await that may be skipped is not such an await, so the call can
still run synchronously in the leaf scope and stays a proof obligation.

| Case | `leaf-owner-forbidden-call` | Why |
| --- | --- | --- |
| `NullishAssignment` | uncertifiable | `value ??= await p` skips the await when `value` is set |
| `OptionalCallArgument` | uncertifiable | `callback?.(await p)` evaluates no argument when `callback` is nullish |
| `OptionalKey` | uncertifiable | `table?.[await p]` evaluates no key when `table` is nullish |
| `LabelledBreak` | uncertifiable | `break skip` leaves the labelled block before its await |
| `DestructuringDefault` | uncertifiable | a destructuring default runs only for `undefined` |
| `UnconditionalAwait` | none | every run awaits first (control) |

Every case also carries `missing-owner` (`SC4001`) for `onCleanup` in a
helper whose call site is a leaf owner's callback; that rule's answer is
unchanged here. The Type Facts producer's `calls_after_await` scan gained the
same skippable constructs (labelled statements, logical assignments, optional
chains, destructuring defaults), so `missing-owner` no longer reads a call
after `x ??= await p` as a continuation.

`Owner.tsx` pins that producer change: `onCleanup` after `x ??= await p`
(`NullishAwaitInComponent`) or after a labelled block a `break` may leave
(`LabelledAwaitInComponent`) is no `missing-owner` violation, while after an
await every run reaches (`UnconditionalAwaitInComponent`) it is.

`solid-js.d.ts` and `tsconfig.json` are copied from
`leaf-scope-helper-completion`. The sources pass `tsc --noEmit` with the
fixture stub.
