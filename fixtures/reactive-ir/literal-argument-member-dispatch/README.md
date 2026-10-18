# Primitive receivers select built-in members

## Nested parameter-member paths

`Nested.tsx` calls the cross-file helpers in `nested-source.ts`. A nested
dispatch such as `input.source.read()` can now select a source function when
every intermediate is a fresh closed data literal. Local const and runtime
let aliases are admitted only when each has no writes and one reference, in
its own function frame, after initialization. The callee's exact parameter may only call that same
full path, without arguments; forwarding, retaining, returning or replacing
any prefix withholds the proof. The walk permits at most eight path segments
and sixteen value-resolution steps. Any project eval vetoes it.

- `NestedLiteral`, `NestedBinding`, `NestedAliases`, `NestedNamespace` and `NestedMethod`
  discharge the dispatch obligation using the exact inert implementation.
- `NestedReactiveRead` and `NestedWrongLevel` retain SC1001 uncertifiable
  reads through the selected function. Selecting a method does not prove its
  read executes on every path. A same-named inert method on the outer object
  must never mask the reading implementation on the inner object.
- Unknown values, getters, spreads, reassigned bindings, escaped/replaced
  objects, receiver-dependent methods, callee escape/write,
  conditional values and factory-produced functions retain SC9012.
- `NestedMutable` uses an initialized, unwritten local let and is clean.
  `NestedReassignedLet` retains SC9012 after replacing its initializer.
  Ambient/namespace lets and captured/escaped bindings receive no new proof.
- `NestedDefault` is clean: the exact supplied fresh object is never
  undefined, so the parameter default cannot replace it. Explicit undefined
  and unknown arguments retain SC9012 (`NestedDefaultUndefined` and
  `NestedDefaultUnknown`); no behavior is inferred from the default body.
- `NestedMethodArguments` stays open: this slice does not instantiate a
  selected method's callback arguments. Exported helper entrypoints retain
  their independent unknown-caller obligations.
- `NestedComputedPrefix` retains its obligation: the named prefix before a
  computed access is not the implementation of the complete member call,
  even when the prefix itself happens to be callable.

The new sources pass strict and loose TypeScript checks against published
Solid 2.0.0-rc.13 typings without the fixture stub. Armed process tests assert
the finding kind. A retained-session regression changes the helper's escape
behavior and adds/removes eval in another file, requiring unchanged callers
to withdraw and restore the proof and retained/cold IR to agree.

A helper that invokes a member of its parameter (`key.replace(...)`) leaves
the choice of implementation to each call site. When the argument there is a
literal (string, number, boolean, bigint, template or RegExp literal), it is a
fresh value whose members all come from its built-in prototype, and no
built-in reads reactive state, so the call site carries no dispatch
obligation. The same holds when Type Facts resolves the helper's member call to
a primitive wrapper's standard-library method (`String.toUpperCase`), whatever
argument arrives (ADR 0190).

- `StringLiteral`, `TemplateLiteral`: clean (previously `SC9012`
  uncertifiable).
- `TypedString`: clean; `shout`'s `message.toUpperCase()` is
  `String.toUpperCase`.
- `ChainedBuiltin`: clean; `message.trim().split(" ")` takes no argument at
  `trim`, so its resolved call is demanded because its root is a parameter.
- `FormattedBuiltin`, `CommentedBuiltin`, `WrappedBuiltin`: clean; multiline
  chains, comments and transparent wrappers preserve the exact receiver
  declaration used to demand the existing primitive built-in proof.
- `FormattedObject`: `Date.getTime` remains uncertifiable even with the
  resolved declaration; formatting conveys no runtime identity.
- `ObjectBuiltin`: `Date.getTime` is an object type's method, which a
  subclass can override, so `SC9012` stays.
- `ObjectArgument`: resolved through the object's own member, unchanged.
- `UnknownArgument`: a caller-supplied value with no selectable
  implementation stays `SC9012` uncertifiable.
- `LiteralWithReadingReplacer`: the literal does not settle what the callee's
  own replacer callback reads. `fill` reads `name()` inside the
  `String.prototype.replace` replacer, synchronously, during the call.

The stub `solid-js.d.ts` declares only `createSignal`, whose tuple shape is
the published one; no finding here depends on another signature.

`Optional.tsx` exercises selected signatures behind optional chaining. A
nullable primitive receiver retains the selected `String` method and its
argument mapping; nullish alternatives select no implementation. A nullable
`Date` still supplies no exact runtime member implementation, and a replacer
callback's reactive reads must remain visible. The sources pass strict and
loose TypeScript checks against published Solid RC.13 typings without the stub.

## Local dispatch caller contexts

`TrackedDispatch.tsx` isolates the `pageSlice(items)` shape sampled in the
application corpus. Its `readonly T[]` parameter supplies no proof that
`.slice` has its native implementation. Instead, exact local caller contexts
can make the untracked-read dispatch obligation unnecessary at this site.

- `DirectTracked` is the existing direct JSX control.
- `LocalTracked`, `ChainedTracked`, `ComponentProperty`, `WrappedCallee` and
  the inner `ShadowedHelper` declaration are clean. The caller proof follows
  exact immutable local arrows into tracked JSX or compiler-proven component
  property getters, including transparent TypeScript call wrappers.
- `MixedCalls`, `EscapedHelper`, `RecursiveHelper`,
  `MutableHelper`, `StoredCaller`, `GetterDefaultCaller`, `JsxDefaultCaller`
  and the outer `ShadowedHelper` declaration
  keep SC9012 uncertifiable. One safe reference cannot discharge an unsafe or
  unknown sibling reference.
  Callback parameter defaults do not run when the surrounding JSX constructs
  the callback, including inside a compiler-proven component-property getter.
- `AsyncHelper` is clean (ADR 0320): its `pageSlice` call follows an await
  the async helper reaches on every path, so it runs on a later task outside
  any render window or tracking scope, where the hidden reads decide no claim.
  `NestedDefaultUndefined` keeps SC9012 as `parameter-default-unresolved`:
  `undefined` selects the callee's default, whose members the call does not
  choose.
- `ClearedTracking` is an existing explicit-untrack control. The new proof
  does not turn its callback into a tracked region or alter write/owner rules.
- The armed direct-eval process twin inserts `eval("visible()")` into a private
  copy and checks that the original obligation returns. No dynamic code is
  inserted in the committed fixture, and no reference absence is trusted alone.
- Generated 32- and 33-helper twins pin the recursion limit, including a shorter
  caller proof cached before the longer one is checked. Cache reuse must retain
  the full path depth and cannot turn an over-budget traversal into a proof.

The added `untrack` declaration is copied from the published Solid RC.13
typings. `TrackedDispatch.tsx` passes `tsc --noEmit` against the real published
RC.13 packages without this fixture stub. The armed native tests assert exact
call-argument spans and the uncertifiable finding kind.

`LocalEvent`, `ChainedEvent` and `LocalTimer` extend the same exact-caller proof
to the event/deferred contexts that already exclude a direct dispatch call.
`MixedEvent` retains its obligation because an additional eager call executes
in the component body. `EventDefaultCaller` keeps the conservative default-
initializer refusal. The 32-helper bound, exact-reference census and eval,
escape, mutation and recursion vetoes still apply. No method identity or
execution role is inferred from these exclusions.

`DeclarationDispatch.tsx` applies the same proof to synchronous local function
declarations. The declaration called only from tracked JSX is clean; a
declaration also handed to an unknown consumer remains uncertifiable. This
does not close callers of top-level or exported helpers.
