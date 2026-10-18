# obligation-exclusion-boundary

**Claim (ADR 0320, review round 7).** A dispatch obligation (the unmodeled
parameter use, an extracted member, a withheld object member) is kept only
where the reads it hides could decide a claim, and every exclusion holds for
every invocation of the code it excludes:

- code after an await its async function reaches on every path runs on a
  later task: excluded (`PostAwait`);
- code after an await the function may skip is excluded only when every
  invocation of that function is an event, a deferred callback or a tracked
  computation (`EventOnlyAsyncHandler`); otherwise it may run synchronously in
  the caller's role (`ConditionalAwait`, `AsyncHandlerAlsoCalledInBody`);
- code before the first await runs in the caller's role
  (`AsyncHelperCalledInBody`);
- a function named at a callback position (`onClick={press}`) takes that
  position's role only when its ADR 0319 invocation-flow answer agrees or every
  reference to it is such a position: a direct call (`HelperAlsoCalledInBody`,
  `HandlerAlsoInvokedInBody`, `MemberHelperAlsoCalledInBody`) or a hand-off to
  a host function (`HelperPassedToHost`) keeps the obligation;
- a value proven primitive by its own expression (a literal, template,
  operator result, `.length` of a built-in value, audited standard-library
  result, or a `const` holding one) carries no project member and cannot be
  invoked (`PrimitiveArgument`). A declared type proves nothing: a call
  typed `number` (`DeclaredPrimitiveCall`), an `as` assertion to a string or
  a branded string (`AssertedString`, `AssertedBrand`), a user type predicate
  (`PredicateNarrowed`) and a parameter member typed `string` handed on
  (`HandedTypedPrimitiveMember`) keep the obligation, as objects do
  (`ObjectTypedArgument`, `HandedFunctionMember`) (round 7);
- `undefined`, written or omitted, selects the callee's default, whose members
  the call does not choose (`DefaultOmitted`, `DefaultUndefined`);
- an await is reached on every path only with no construct that can skip it
  before it: a logical assignment (`LogicalAssignmentAwait`), an optional
  call's argument or key (`OptionalCallAwait`, `OptionalKeyAwait`), a
  labelled block a `break` may leave, through `try`/`finally` too
  (`LabelledBreakAwait`, `NestedLabelledBreakAwait`), a destructuring default
  (`DestructuringDefaultAwait`) (round 7, AST facts schema 64);
- a member called on a local destructured from the parameter, in its pattern
  or in the body, at any depth, is the caller's object's
  (`DestructuredField`, `DestructuredInBody`); a default, nested pattern, rest
  or array element is unmodeled (`DestructuredDefault`, `DestructuredNested`,
  `DestructuredRest`, `DestructuredArray`); a data read hands nothing on
  (`DestructuredDataOnly`) (round 7).

| Case | Finding |
| --- | --- |
| `PostAwait`, `EventOnlyHelper`, `EventOnlyAsyncHandler`, `MemberEventOnlyHelper`, `PrimitiveArgument`, `DestructuredDataOnly` | none |
| every other case | `SC9012` uncertifiable (`DestructuringDefaultAwait` holds one) |

`solid-js.d.ts`, `tsconfig.json` and `node_modules` are copied from
`parameter-use-forms` (itself from `invocation-flow`); `package.json` is
private. The sources pass `tsc --noEmit` with the fixture stub.
