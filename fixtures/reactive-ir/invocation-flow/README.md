# invocation-flow

**Claim (ADR 0319).** In a closed program, code directly in a synchronous,
non-component function literal takes the one role every invocation of that
literal runs in, found by following the literal forward
(`execution_role::invocation_flow_role`): through `const` bindings, object
literal members and spreads, returns (back to the exact call a callee was
entered through, or to every call of a local function), arguments of exact
project callees, project component props (context-sensitively, through the
element that rendered each activation), and spreads or `on…` attributes on
intrinsic elements, which the host dispatches as events. An invocation that
is not itself an event, a deferred callback or a tracked computation takes the
role of the plain function it is written in, found the same way. Every
invocation must have the same role. Write legality never asks this arm.

| Case | Finding | Why |
| --- | --- | --- |
| `SpreadHandlers` | none | `onActivate` runs from `handleKeyDown`, which runs only from the `onKeyDown` listener of the object `rowProps` returns, spread onto a `div` |
| `ForwardedRows` | none | the `onClick` of the object `rowProps` returns reaches the row `div`'s spread through `Tree`, `Level` and the recursive `Branch`; `isProtected` is absent (and no file can give a props object that member) and `children` is forwarded to the caller's arrow |
| `ClickRetained` | none | the retained closure runs only in a click handler |
| `ThisSibling` | `SC1001` uncertifiable | a method in the options literal uses `this`, which may reach `onActivate` |
| `AppliedRetained` | `SC1001` uncertifiable | an effect's apply callback runs the closure |
| `MixedRetained` | clean (ADR 0323) | a timer and a click: two roles, neither reporting; until ADR 0323 a mix was refused and this was `SC1001` uncertifiable |
| `BodyRetained` | `SC1001` uncertifiable | the component body runs the closure |
| `AbsentSibling` | `SC1001` uncertifiable | the closure calls the absent `log` with the options object as `this`; an inherited `Object.prototype.log` could run any member |
| `AsyncRetained` | `SC1001` uncertifiable | the call follows an `await` |
| `ReassignedRetained` | `SC1001` uncertifiable | the binding holding the closure is reassigned |
| `OpaqueRetained` | `SC1001` uncertifiable | `opaque` has no body in the program |
| `HazardRows` | `SC1001` uncertifiable | the recursive element gives `label`, which `HazardTree` calls on its props, a function that may use `this` |

`counterexamples.tsx` pins the review counterexamples to the first version
(each was certified by it; each must stay uncertifiable):

| Case | Finding | Why |
| --- | --- | --- |
| `CyclePath` | `SC1001` uncertifiable | the recursive element moves the callback `a → b → c`; the level holding it as `c` calls it in its body. The recursion cut applies only to the same element *and* the same followed prop |
| `MutualCycle` | `SC1001` uncertifiable | the same through `Ping` and `Pong` rendering each other |
| `NestedInMethod` | `SC1001` uncertifiable | the function expression nested in the `onKeyDown` method carries the method's name in the facts but is not the method; it is registered and called in `Registered`'s body |
| `SeenOrigin` | `SC1001` uncertifiable | two returns reach the same call with different literals; the second gives `log` a `this`-using function. A holder is revisited per origin |
| `SpreadThisGetter` | `SC1001` uncertifiable | the literal the handler object is spread into has a getter calling it through `this` |
| `QuotedKey`, `SingleQuotedMember`, `DoubleQuotedMember`, `NumericKey`, `TemplateKey`, `DestructuredQuoted` | `SC1001` uncertifiable | the hook calls the callback in its caller's body through a key spelled differently from the member reading it (`"run"`/`.run`, `'run'`/`['run']`, `1`/`[1]`, `` [`run`] ``, `{ "run": run }`); keys compare by static name |
| `OverrideQuoted` | none | a later `"run": noop` replaces the spread `run`, so the body's `merged.run()` calls `noop`; the callback runs only on click |
| `OverrideOther` | `SC1001` uncertifiable | the later key is `"other"`, so `merged.run()` still runs the callback in the body |
| `ProtoAlias`, `QuotedProtoAlias` | `SC1001` uncertifiable | `__proto__: base` (quoted or not) sets the prototype, so `held.run` is `base.run`, run in the body |
| `SuperCall` | `SC1001` uncertifiable | `m` calls `super.fire()`, which runs the prototype's `fire` with `held` as `this`; a method mentioning `super` uses its receiver |
| `SuperSetPrototype` | `SC1001` uncertifiable | the same with `Object.setPrototypeOf`; the holder becomes an argument |

Script files, CommonJS and `globalThis` are pinned in `invocation-flow-scripts`.

`solid-js.d.ts`, `tsconfig.json` and `node_modules` are copied from
`use-site-role`; `package.json` is private, which closes the program
(ADR 0193). The sources pass `tsc --noEmit` with the fixture stub.

ADR 0320 review round 6 gave `SpreadHandlers` and `ClickRetained` an `SC9012`
dispatch obligation: the options callback is called inside a closure of an
exported factory, recorded as a use of the factory's parameter. ADR 0321
resolves that dispatch to the arrow written in the argument literal, so the
obligation is gone; the arrow's read keeps the role this ADR proves.
