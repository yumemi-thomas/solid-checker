# Closed callback-use summaries of project consumers

A reactive read inside a callback that a component body passes to an exact
project function (or a static one-segment callback prop) is not a strict read
when every runtime use of that parameter is closed and none runs on the body's
stack. The closed uses are:

- a tracked compute slot (`createMemo`, an effect's compute, function-form
  `createSignal`);
- exact `untrack`;
- a compiler-tracked JSX expression;
- an audited fresh-stack scheduler (`queueMicrotask`);
- exact forwarding to another closed summary.

| Case | `SC1001` |
| --- | --- |
| DirectCompute, MultipleComputes, EffectCompute, ComputedSignal, ExplicitUntrack, MixedSafe, FreshQueue, CrossFileTracked, CrossFileMixed, MemoProp, JsxProp, ForwardedMemoProp | clean |
| IndependentObjectSlots: `key` | clean |
| SiblingArrowInvoke, SiblingConstArrow: the consumer invokes a sibling field that the site passes as an arrow, or names by a `const` arrow | clean |
| IndependentObjectSlots: `load` (stored) | uncertifiable |
| InlineBody, SafeAndStrict, EagerRenderProp (the child calls the prop in its body, ADR 0204) | violation |
| ForwardedInlineBody: an expression-bodied identifier chain ending in a body-time call. It warns at runtime, but the chain's `const` callees may be uninitialized when it runs, so it is not proven | uncertifiable |
| SiblingInvoke (a method calling `this.key()`), SiblingValueOf (an inherited `valueOf` returns the object), SiblingTag (a template tag), SiblingNamedInvoke (a named function), LabelledUntrack (`untrack(read, "label")` warns) | uncertifiable |
| SiblingAbsent (an absent sibling may be inherited from an extended `Object.prototype`), SiblingOverwritten, PrototypeWritten (a field written after construction), JsxPrototype (a `__proto__` attribute), LabelledInsideMemo (a labelled `untrack` under a memo), ComponentInsideMemo (a component entered inside a memo opens its own strict window) | uncertifiable |
| GatedInline (a throwing default parameter), GateArgument (an argument in its temporal dead zone) | uncertifiable, never a violation |
| SiblingAbsentRead: an absent sibling is read, which runs any getter project code installed on `Object.prototype` | uncertifiable |
| FunctionSubject: the callback is a `function` expression, which can hand its receiver to code that calls it again | uncertifiable |
| UnnamedRows, NamedRows (`mapArray` with `name` wraps the map function in a labelled window), Caught (`Promise.catch` calls its receiver's `then`) | uncertifiable |
| DirectLabelledUntrack | nothing reported; a known missed detection (docs/precision-backlog.md) |
| StoredCallback, ReturnedCallback, ReturnedUntrackCapture, UnknownPackage, SynchronousDomDispatch, StoredCallbackProp, WholePropsEscape, NamespaceTarget, ComputedTarget, Cycle, AsyncConsumer, DefaultedConsumer, SpreadOverride | uncertifiable |

The summary never clears another rule's obligation: the `SC9005`, `SC9012`
and `SC4001` findings in `wrappers.tsx` and `forwarding.ts` stay.

`unknown-package` is a synthetic external boundary whose declaration admits a
callback. It has no runtime implementation and no contract, so its invocation
behaviour stays unknown.

The callback slots of the local `solid-js` stub keep the shapes of the
published `@solidjs/signals` declarations. The fixture also type-checks clean
against the published `solid-js@2.0.0-rc.13` and `@solidjs/web@2.0.0-rc.13`
declarations.
