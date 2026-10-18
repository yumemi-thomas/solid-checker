# ADR 0320: An unresolved alternative withholds the whole value

- Status: accepted and implemented (2026-10-17). A soundness repair, not a
  reduction: it adds uncertifiable findings and removes none.
- Owners: `member_value_candidates`, `factory_member_value_candidates`,
  `binding_stays_closed`, `parameter_only_member_accessed_consulting`,
  `objects_may_be_assigned_into`, `own_member_may_be_replaced`,
  `member_resolution_digest` and `value_origin` (`indexes.rs`);
  `forward_parameter_member_invocations` and the parameter index of
  `ParameterMemberInvocation` (`interproc.rs`).
- Relation: tightens ADR 0211 (built-in and class origins), ADR 0314 (exact
  member resolution through literals and factories) and the per-call-site
  parameter member dispatch of ADR 0202/0203.

## Context

A callee that invokes a member of its parameter (`helper(o)` running
`o.run()`) has its dispatch decided at each call site from the argument
passed there. The resolver merged alternatives without checking that each
resolved: `cond ? { run() {} } : opaque` resolved to the literal's `run`
alone, as did `const held = cond ? … : opaque`, a cast of the conditional, and
`{ run() {}, ...partial }` (a spread after the key may replace it). Three more
defects of the same kind sat beside it:

- a `const` holding a literal was trusted while the object was handed to
  unknown code (`mutate(held)`), had its member assigned (`held.run = …`) or
  copied into (`Object.assign(held, …)`), or was reached through a method that
  hands `this` on; the callee itself could hand its parameter on before the
  call, and `value_origin` (ADR 0211) had the same blind spot for project
  instances and built-in objects;
- a function that hands its own parameter to a member-invoking callee
  (`forward(o) { helper(o) }`) raised the dispatch obligation nowhere when it
  is not a rendering function: `forward(opaque)` from a component was clean;
- the parameter index of a recorded member invocation counted only
  identifier parameters, so after a destructured parameter
  (`shifted({ run }, o)`) the call site resolved the wrong argument.

Each was reproduced against `/tmp/sc-final4` and `/tmp/sc-final10` (all
clean there) before the fix.

## Decision

1. **Alternatives.** A value's implementations are trusted only when every
   part resolved: both arms of a conditional, every return of a factory, the
   initializer of a binding. One unresolved part withholds the whole value; a
   resolved union keeps every candidate, so `equivalent_summary_reads`
   decides divergence. Logical operands (`??`, `||`, `&&`) stay unresolved.
2. **Literals.** The member is the last own property with that static key
   (ADR 0319's `property_static_key`); no spread may follow it, no getter or
   key without a static name (computed, or a `__proto__:` prototype setter)
   may be in the literal, and no function in it may use its receiver
   (`this`/`super`). With no own property, exactly one spread supplies it.
3. **Bindings.** A `const` holds its initializer only when it is not a
   parameter (a default holds only for an omitted argument), not exported, not
   a script global, and every other reference to it anywhere is a non-computed
   member access it does not assign, or an argument of an exact project callee
   that uses that parameter only so (followed four calls deep). The same rule
   guards `value_origin`'s bindings of non-primitive values.
4. **Callee.** The per-call-site resolution (and `value_origin`'s) applies
   only when the callee uses the parameter that way too.
5. **Census.** No assignment of the member's name (or through an unknown
   computed key) and no reflective definition of it may exist in the program.
   Code copying keys into an existing object (`assign`, `setPrototypeOf`,
   `defineProperty`/`defineProperties` on a target that is not a fresh
   literal or function, any other use of `Reflect` or of those names)
   withholds project-class instances, whose constructor can hand `this` on.
6. **Forwarding.** A function that is not a rendering function and hands its
   own parameter, whole, to an exact project callee invoking a member of it
   invokes that member too; recomputed from the merged graph every build, at
   most eight forwarding steps. The forwarded invocation's context is the
   forwarding call's, and an inner invocation not reported where it runs is
   not forwarded.
7. **Index.** A member invocation's parameter index is the declaration's,
   which is the argument's at every call.
8. **Caches.** The interprocedural result records each function whose source
   an answer read (factories, callees the object is handed to) as a
   `FunctionSource` dependency; untracked consumers keep an answer only when
   everything it read is in their own file. The program-wide census is bound
   into `member_resolution_digest`, which keys the late stages and the
   interprocedural graph cache but not source discovery, so a member write
   elsewhere does not discard unrelated work.

## Review round 6 (2026-10-18)

Five more families of unproven clean claims, each pinned in its own project:

1. **Escaping object methods** (`escaping-object-methods`). A shorthand
   method's reads were claimed only where the method is visibly called; an
   object literal handed to unknown code, enumerated (`Object.values`),
   returned or stored let its method run unseen. An object-literal method is
   now a named helper only while every use of the literal is a member call
   the analysis resolves (the literal is a call's receiver, a `const` of this
   module whose every reference is one, or an argument of an exact callee in
   this file that only reads members of its parameter); otherwise its reads
   are stored-callback reads, uncertifiable like a function-expression
   property's (`object_method_escapes`, `owners.rs`).
2. **Withheld member calls** (`withheld-object-member`). A member call on an
   exact literal binding whose ADR 0299 receiver census fails (a member
   assigned in another module, the object handed on) was skipped as a call
   that reads nothing. It is now an `SC9012` obligation
   (`withheld-object-member`) wherever its reads could be reported.
3. **Unmodeled parameter uses** (`parameter-use-forms`). A non-rendering
   function's parameter handed on in a form no call site resolves -- an alias
   that reaches a call, a wrapper, spread, array element, conditional or
   non-test logical value that is a call's argument, an argument of a callee
   that is not an exact project function (or a forwarding call in another
   function, such as a returned closure), `arguments` -- records an
   `unmodeled` invocation: its callers raise `SC9012`
   (`parameter-use-unmodeled`) unless the argument is a primitive or nullish
   value, or a function (the callback rules' question). An audited
   standard-library call that only reads the value, and a dialect primitive,
   hand nothing on; a test, arithmetic or template use hands nothing on
   (implicit conversions are an analysis-wide premise); a bare `return` gives
   the object back to the caller.
4. **Unknown-key writes** (`instance-key-writes`). An assignment through a
   non-literal key on a receiver that is not a proven built-in value
   (`for (const t of all) t[key] = v`) now counts in the key-copier census,
   so a project instance a constructor registered is not trusted. Writes into
   a proven built-in (`xs[i] = v` on an array) do not count.
5. **Extracted members** (`parameter-use-forms`). A member read off a
   parameter whose value may be called -- destructured, extracted, handed to
   another call (`queueMicrotask(o.run)`) -- is a possible invocation of that
   member, resolved at each call site like a call of it; a destructured
   parameter whose local is called or handed on is `unmodeled`.

A member call on the parameter inside a nested named function (or a
returned closure) is now recorded for the enclosing function too; its own
context (ADR 0319's flow role included) decides whether it is reported.

Self-checked, unchanged: a generator factory, a factory with a non-literal
branch (`make(kind)`), an early `return;`, `structuredClone`, and a subclass
in another file returned as its base type all stay uncertifiable. A call on
an ambient value (`declare const opaque`) is trusted everywhere, in a helper
or a component body alike: that is the package-boundary premise, not this
family.

## Narrowing (2026-10-18): where an obligation decides nothing

Round 6 is kept; an obligation is now excluded where no rule could report the
reads it hides, with the same machinery for the direct, forwarded
(`unmodeled`), extracted and withheld families (`obligation-exclusion-boundary`):

1. **After an await.** Code after an await its async function reaches on
   every path (`unconditional_awaits`: no conditional, loop or `try` between
   entry and await) runs on a later task, outside any owner, render window or
   tracking scope (`follows_unconditional_await`). Code after an await the
   function may skip is excluded only when the function's own ADR 0319 answer
   is an event, deferred or tracked role, so it runs in that role or later
   (`await_continuation_decides_nothing`). ADR 0319's flow now answers async
   literals for the code before their first await.
2. **Named callbacks hold for every invocation.** `named_callback_roles` gives
   a function named at a callback position (`onClick={press}`,
   `untrack(read)`) that position's role, and a read inside it is claimed at a
   direct call through the helper's summary; a dispatch obligation is not. So
   `press(); <button onClick={press}>` lost the obligation in every binary
   since ADR 0202, and ADR 0319's `site_role` trusted the same role. An
   excluded role now holds only when each such function between the site and
   its component has an ADR 0319 answer that is itself excluded, or every
   reference to it, in its own module, is an event-handler attribute value or
   a proven callback argument of a known primitive (`excluded_role_holds`,
   `named_callbacks_within_component`, `named_callback_census_holds`). A
   direct call, a host hand-off (`addEventListener(handler)`), an export or a
   direct `eval` refuses.
3. **Primitive values.** An argument that evaluates to a primitive carries
   no project member: it settles an `unmodeled` use and an extracted-member
   path at the call site. (First shipped on the compiler's declared
   primitive-value domain; review round 7 below replaced that with a proof
   from the expression itself.)
4. **Defaults.** `undefined`, written or omitted, selects the parameter's
   default initializer, whose members the call does not choose: it is now an
   obligation (`parameter-default-unresolved`) instead of a nullish value that
   settles everything. Omission was silent before round 6 too.

`FwdAsync` (review round 6, `b3`) is now clean by rule 1: its helper call
follows `await 0`. Its sibling `FwdAsyncBefore` stays uncertifiable.

## Independent review round 7 (2026-10-18)

Three unproven clean claims (repros `r6/b10`..`b15`), all fixed and pinned in
`obligation-exclusion-boundary` and `leaf-scope-skippable-await`:

1. **Skippable awaits read as unconditional.** `x ??= await p`, `||=`,
   `&&=`, `g?.(await p)`, `t?.[await p]`, a destructuring default and an
   await inside a labelled block that a `break label` (directly, nested, or
   through `try`/`finally`) may leave all counted as reached on every path,
   so the narrowing above excluded what followed them. Fixed in the fact
   itself, so every consumer benefits (AST facts schema 64): a logical
   assignment's right side, a destructuring default (`AssignmentPattern`,
   `AssignmentTargetWithDefault`) and a labelled statement's body now raise
   `conditional_flow_depth`, and an await inside an optional chain's
   skippable argument or key ranges is not recorded. `straight_line_calls`
   gains the first three for free. The other readers were affected too: the
   leaf-owner walk (`cleanup.rs`) skipped a forbidden call after such an
   await as an ownerless continuation (now `SC3001` uncertifiable), and
   `reactive-read-after-await` took it as its boundary. The Type Facts
   producer's `calls_after_await` scan (`async_flow.go`), which
   `missing-owner` reads, had the same gaps and the same fix (labelled
   statements, logical assignments, optional chains, binding and assignment
   defaults are branches); `onCleanup` after `x ??= await p` in a component
   is no longer a `missing-owner` violation.
2. **A declared primitive type settled an argument.** The compiler's
   primitive-value domain follows `as`/`<T>` assertions, branded and
   template-literal types, user type predicates and declared return types,
   none of which proves a value. `SemanticLookup::proven_primitive` now
   requires the argument's own expression to produce a primitive other than
   `undefined`: a literal or template, an operator whose result is always
   primitive (new AST table `primitive_results`: arithmetic, shift, bitwise,
   relational, equality, `in`, `instanceof`, prefix `+ - ~ ! typeof delete`,
   `++`/`--`), `.length` of a proven built-in value, an audited
   standard-library primitive result (`String(x)`, `Number(x)`,
   `parseInt`, `Math.*`, ...), a conditional or logical expression all of
   whose results are, or an unexported `const` of the same file holding one.
   Parentheses are followed; any assertion refuses. A parameter's member
   handed on is no longer settled by its type either: the member path is
   resolved at each caller, like a call of it.
3. **Members called on destructured locals.** `function f({ o }) { o.run() }`,
   `const { o } = p; o.run()`, renamed, nested, rest and array patterns, and
   defaults raised nothing. A local destructured from a parameter (in its
   pattern or in the body) that is called, has a member called, or is handed
   on is now a use of the parameter: a plain `key: local` slot gives the
   member path (`["o", "run"]`, resolved at callers like `box.o.run()`), and
   anything else (a default, nested pattern, rest, computed key, a member
   read handed on) is `unmodeled` (`destructured_parameter_uses`,
   `local_dispatch_suffixes`). A local only read as data hands nothing on.

Sibling sweep, no further hole found: the leaf walk's `exact_instance_method`
requires an identifier binding initialized by `new`, so a destructured or
defaulted parameter receiver is never trusted (probed: uncertifiable); ADR
0211 `value_origin` refuses parameters and non-identifier bindings; ADR 0319's
flow keeps a callback delivered through a destructured parameter
uncertifiable in a component, whether the helper calls it, binds it to an
event, or both (probed; binding-only stays `SC1001` uncertifiable, a
precision gap, not a clean claim). Precision left as is: the nested-path
resolution needs an identifier parameter, so `f({ o: { run() {} } })` through
a destructuring stays uncertifiable where `box.o.run()` resolves.

## Primitive-value proofs (2026-10-18)

Round 7's strict `proven_primitive` is kept; in a closed program the final
attribution pass (`CallGraph::value_is_primitive`, `attribution.rs`)
discharges a forwarded `parameter-use-unmodeled` obligation, and a handed-on
member of a fresh literal argument (`parameter-member-target-unresolved`,
`parameter-member-targets-diverge`), when the value is proven primitive and
not `undefined` from what produces it, across functions and files:

| Producer | Proven when | Refuses |
| --- | --- | --- |
| its own expression | `proven_primitive` | any assertion |
| `a ? b : c`, `a ?? b`, `a \|\| b`, `a && b` | every result is | |
| `const x = …` in the same file | unwritten, initializer is | a parameter, a destructuring |
| a parameter | ADR 0213's census (entered only through call expressions, no spread before it) and every call passes one; a missing argument only under a proven default | rest, `arguments`, a written parameter, an async or generator function, a written `undefined` |
| an effect apply callback's first parameter | `createEffect`/`createRenderEffect` with exactly a compute literal and an apply literal; every return of the compute is | options forms, a default |
| `props.name` | ADR 0214's census (`props` only read, the component rendered only through tags), and every tag passes the attribute as a string, a boolean or an expression that is | a spread, a duplicate, a missing attribute (`undefined`), `children` |
| a signal accessor `x()` | read in its own file; the initial value is; every setter reference in its file is a call writing one, or an updater whose returns are with the previous value assumed | no initial value (`undefined`), `createSignal(fn)`, a setter referenced in another file or not as a callee, extra slots |
| a bare memo accessor `m()` | every return of its compute literal is | options |
| an exact project function's result | `function_value_is_current`; every own return is, and one is in straight-line flow (new `ReturnFact::straight_line`, facts schema 64), so nothing falls through to `undefined` | methods, computed callees, async, generators |
| a literal argument's member | the callee only reads members of that parameter; the last same-key property of a closed literal (no getter, computed key or spread) is | |

Store fields and DOM property reads were measured and left out: two and none
of the added findings respectively. The open-program side keeps every
obligation (the census needs the closed program).

Measured against `/tmp/sweep-final12.json`: 616 added before, **592** after;
the discharge settles 24 (`EffectApplyPrimitive`-style computes, helpers
called with literals, literal members, project functions returning
templates). What remains is rooted outside any fact the analysis has:

| Remaining family | Count | Why no producer proof |
| --- | --- | --- |
| `props.x` | 192 | 96 tags pass a `<Show>`/`<For>` render-callback accessor or a call whose value comes from a package (`useParams()`, `useQuery`), 39 components are rendered by a router or lazily (no tag to enumerate), the rest pass package-rooted identifiers |
| result of an unaudited call (and consts of one) | ~66 | method calls, package functions, project functions whose returns are not all proven or not straight-line |
| parameter of a local helper | ~58 | the helper is also handed out as a value (an event prop, a callback) or its callers pass package-rooted values |
| parameter of a callback a package invokes | ~53 | `queryFn`, `useSelector`, `createMutation`, timers: the package supplies the value |
| member of a variable or literal | ~70 | object fields (`tower.x`), literal members holding props |
| effect apply parameter | 24 | the compute returns a prop or a package value |
| parameter of a callback prop | 20 | the child component's invocation would have to be enumerated |
| signal or memo accessor | ~11 | a setter handed on, or a write that is not proven |

## Proof obligations

| Decision | Premise | Cases |
| --- | --- | --- |
| "These are the implementations" | every alternative (conditional arm, factory return, binding initializer) resolved; a union keeps every candidate | `Conditional`, `ConstConditional`, `Cast`, `FactoryConditional`, `TwoLiterals` |
| "The literal's own member" | last own property by static key; nothing after it can replace it; no getter, computed or `__proto__:` key; no receiver-using function | `SpreadAfter`, `SpreadBefore`, `ReceiverMethod`; ADR 0319 r2/r3 key and `__proto__` cases |
| "The binding holds its initializer" | `const`, not a parameter, not exported, not a script global; every other reference is a member access or a closed callee argument | `Reassigned`, `MutatedByCall`, `EscapedInstance`, `EscapedArray`, `interprocedural-methods-v2` |
| "The callee does not hand it on" | the parameter is used only as the object of non-computed, unassigned member accesses, or forwarded whole to a callee of which that holds | `LeakyCallee`, `ForwardedResolved` |
| "Nothing replaces the member" | no assignment of the name, no reflective definition; no key copier for class instances | `member-value-copiers` |
| "The obligation is raised" | a non-rendering helper's forwarded dispatch reaches its callers | `Forwarded`, `ForwardedResolved` |
| "This argument is that parameter" | invocation index = declaration index | `ShiftedParameter` |
| "A cached answer is current" | `FunctionSource` for every consulted function; census in the member digest | `member_resolution_edits_match_cold` |
| "This method's reads are claimed where it is called" | every use of its literal is a resolved member call | `escaping-object-methods` |
| "A withheld call reads nothing" | never: a failed receiver census is an obligation | `withheld-object-member`, `withheld_member_edits_match_cold` |
| "The callers see every use of the parameter" | each use is a member call, a whole forwarding to an exact callee, a test or arithmetic, a bare return, or an audited value-only/primitive argument; anything else is `unmodeled` | `parameter-use-forms`, `parameter_use_edits_match_cold` |
| "No code copies keys into this instance" | no copier call, and no assignment through an unknown key on a receiver not proven built-in | `instance-key-writes` |
| "A member read is not an invocation" | its value is not called, handed to a call, or bound to a name that is | `parameter-use-forms` (`Extracted`, `HandedMember`) |
| "Code after this await runs later" | the await is reached on every path, or every invocation of the function is excluded | `PostAwait`, `ConditionalAwait`, `EventOnlyAsyncHandler`, `AsyncHandlerAlsoCalledInBody` |
| "A named callback runs only in its position's role" | ADR 0319 answer excluded, or every same-module reference is such a position | `EventOnlyHelper`, `HelperAlsoCalledInBody`, `HelperPassedToHost`, `MemberHelperAlsoCalledInBody` |
| "This argument carries no member" | its own expression yields a primitive other than `undefined`; no assertion on the way | `PrimitiveArgument`, `DeclaredPrimitiveCall`, `AssertedString`, `AssertedBrand`, `PredicateNarrowed`, `HandedTypedPrimitiveMember`, `DefaultUndefined` |
| "This await dominates what follows" | no conditional, loop, switch, `try`, logical assignment, optional chain link, destructuring default or labelled body between entry and it | `LogicalAssignmentAwait`, `OptionalCallAwait`, `OptionalKeyAwait`, `LabelledBreakAwait`, `NestedLabelledBreakAwait`, `DestructuringDefaultAwait`, `leaf-scope-skippable-await` |
| "A destructured local is not the caller's object" | never: its calls, member calls and hand-offs are the parameter's | `DestructuredField`, `DestructuredInBody`, `DestructuredDefault`, `DestructuredNested`, `DestructuredRest`, `DestructuredArray` |

Audited and left as they are: `value_origin`'s conditional and logical
handling already require every operand (`&&` of arms); `call_origin` and
`returns_builtin` require every return; `closed_properties` (nested paths)
refuses any open literal; `accessor_provenance`'s `closed_dependency_objects`
is all-or-nothing; `string_code_reference`/`string_code_slot` are "may"
vetoes (an alternative counts when either may, which is the fail-closed
direction); `dependency_getters`/`derived_objects` is a may-set explicitly
documented as never a proof (ADR 0289): an unknown alternative yields fewer
obligations there, not a clean claim it makes; `object_members.rs`,
`argument_values.rs` and `value_identity.rs` merge no alternatives; ADR
0319's flow follows conditionals and logical operands forward (every arm is a
holder), which is the sound direction.

## Consequences

Independent review round 7, against `/tmp/sweep-final12.json` (3,558
uncertifiable, 553 violations): **616 uncertifiable added** (all `SC9012`:
418 unmodeled parameter uses, 189 member targets, 6 defaults, 3 diverging),
39 removed (38 keep a finding at the same line under another member or
callee; one, a data-only destructuring default in `createKeyHold`'s options,
now raises nothing), **0 violations changed**: 3,558 to 4,135. By fix
(switched off one at a time): the primitive proof about 580 (about 406 at
call-site arguments, about 175 parameter members handed on, mostly typed
string or number IDs that only their type proves), the destructured locals
11, the skippable awaits and defaults the rest. Fixtures: `leaf-source-call-
targets` and `transported-parameter-flow` regain the narrowing's obligation
on an apply-callback parameter typed `number`/`string`.

Round 7, against the same `/tmp/sweep-final11.json`: **982 uncertifiable
added** (655 `SC9012` runtime target, 309 `SC9012` member dispatches, 18
`SC1001`), 69 removed (event-only async handlers before their first await,
post-await continuations, primitive arguments), **0 violations changed**:
2,645 to 3,558 (round 6 alone: 4,377). By analysis context: unmodeled
parameter uses 505, extracted members 267, withheld member calls 113, default
initializers 46, diverging targets 22, nested paths 9, reads 20. The named
callback rule adds 28 and the default rule 52 over the narrowed count (both
are holes older than round 6); the narrowing removed 1,850 of round 6's.
Pinned fixtures: `leaf-source-call-targets` and `transported-parameter-flow`
lose round 6's obligation on a primitive argument, and
`literal-argument-member-dispatch` loses `AsyncHelper` (rule 1). Review
repros: none lost a finding; `r2f`, `r2g` and `r2h` gain uncertifiable
`SC1001`/`SC5001` where a named callback is also run by `untrack` or
`createRoot` in the render window, which the flow used to read off its
named position.

Where the remaining 982 sit (62 sampled across 25 apps, 50 read): 18 are
calls in a component body into a project hook that hands its argument to a
package (`createMutation(api.x)`, `useRouterState({ select })`,
`createVirtualizer({...})`): the render window, decided by a package nobody
analyzed. 7 are inside a callback handed to a package (`queryFn`,
`useSelector`, `createVao`), whose invocation time no fact gives. 25 are in a
nested handler or closure whose invocations ADR 0319 cannot enumerate: props
of library components (Kobalte, Corvu), project components whose props are
spread or merged before reaching an element, `addEventListener(h as any)`,
ref callbacks. That last family is mostly noise -- most of those handlers run
as events -- and nothing excludes it because the only fact that would
(`named_callback_roles`, the flow census) stops at a package boundary whose
source is not analyzed by default and for which no contract is accepted.

Round 6, against `/tmp/sweep-final11.json` (2,645 uncertifiable, 553
violations): **1,752 uncertifiable added** (1,246 `SC9012` "runtime target
cannot be selected", 488 `SC9012` member dispatches, 18 `SC1001` reads), 11
removed (each location still holds a finding, under another message), **0
violations changed**: 2,645 to 4,377. By family (one family switched off at a
time): unmodeled parameter uses 1,076, extracted members 443, withheld member
calls 133, escaping object methods 18, unknown-key writes 0, interactions the
rest. The list is `/tmp/sc-opt/321-added.txt`. Pinned fixtures: 21 moved,
each only gaining uncertifiable findings (52 in all); `invocation-flow`'s
`SpreadHandlers` and `ClickRetained` keep their clean read (ADR 0319) but
gain an `SC9012`: the options callback is dispatched inside a closure of an
exported factory, whose context-free role the flow does not prove.

Round 1 (ADR 0320's first slice):

49-project corpus against `/tmp/sweep-final10.json` (2,541 uncertifiable, 553
violations): **112 uncertifiable added** (all SC9012: 22 where the receiver is
shared with, or handed on by, code that may replace the member; 90 forwarded
dispatches a non-rendering helper used to drop), **8 removed** (the same eight
locations now name a different unresolved member or callee), **0 violations
added or removed**. Total 2,645 uncertifiable. The added list is
`/tmp/sc-opt/320-added.txt`.

Pinned fixtures that moved: `interprocedural-methods-v2` (two `SC1001` reads
through the exported `objectReader` become `SC9012` obligations; still
uncertifiable) and `project-callback-uses` (`SiblingInvoke` gains an `SC9012`
obligation). New fixtures: `member-value-alternatives`, `member-value-copiers`.

Known gaps, not changed here:

- A function passed as a value to a standard-library call
  (`[opaque].forEach(helper)`) has its parameter's member dispatched by an
  invisible caller; no obligation is raised (reproduced, clean in every
  binary). Raising one at every value escape over-reports callbacks a host
  runs as events (measured: +77 corpus findings, mostly `useMutation(fn)`-style
  registrations), so it needs the invocation role of the escaped function,
  which no fact carries today.
- An exported `const` object is no longer resolved through its initializer;
  doing so soundly needs a cache dependency on every file referencing it.
- A literal property whose value is an arrow function has no summary symbol,
  so such dispatches stay unresolved (pre-existing, conservative).
