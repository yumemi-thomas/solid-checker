# getter-parameter-read

**Claim (ADR 0283).** A project function that Gets `p.key` of its parameter `p` on its every-completion path runs the `key` getter of the exact literal a call passes there, on the caller's stack. When the parameter is only ever read that way, the getter's own every-completion accessor reads are proven at the call: an `SC1001` violation located at the call, with the getter's read as origin. Anything unproven adds nothing, so the getter keeps its existing uncertifiable read (`callee_callback_timing`) and nothing becomes a violation. That uncertifiable read also stays next to each new violation, as in ADR 0274.

| Case | Finding | Why |
| --- | --- | --- |
| `Member`, `Arrow`, `SameFile` | `SC1001` violation at the call | `return o.a` runs the getter |
| `SecondGet` | violation | `o.b` (a data property) then `o.a` |
| `NullishLeft` | violation | the left operand of `??` always runs |
| `NamedReceiver`, `WrappedReceiver` | violation | ADR 0225's exact receiver: a non-escaping `const`, or `as T` |
| `GetterLocal` | violation | the read is a `const` initializer in the getter |
| `NamespaceMember`, `ObjectMethod` | violation | an exact member callee (ADR 0279) |
| `Untracked`, `InMemo` | none | the call runs untracked by request, or tracked |
| `DataProperty` | none | no getter runs |
| `MethodValue`, `OtherKey` | `SC1001` uncertifiable | the Get obtains a function value, or reads another key |
| `Branch`, `Closure`, `Jsx`, `Default`, `GuardThrows` | uncertifiable | the Get is not on every completion (a branch, a returned closure, JSX, a parameter default, a `never` guard) |
| `Keeps`, `Aliases`, `Destructures`, `WritesParam`, `Computed`, `Optional`, `WritesMember`, `Deletes`, `Arguments`, `MethodCall`, `Forwards` | uncertifiable | a reference to the parameter is not a static Get, or the parameter is not a plain unwritten identifier |
| `Async`, `Generator`, `Directive`, `Eval` | uncertifiable | the function is refused whole |
| `Setter`, `SpreadInside`, `ComputedKey`, `Prototype`, `LetReceiver`, `AliasedReceiver`, `LoopedReceiver` | uncertifiable | the receiver is not exact (ADR 0225 rule 3) |
| `SharedReceiver` | none | an imported module-level receiver is not exact; its getter is not written in a component |
| `GetterBranch`, `GetterEarlyReturn`, `GetterClosure` | uncertifiable | the getter's read is not on the getter's own every-completion path |
| `GetterReceiver` | uncertifiable | a getter mentions `this` |
| `CallForm`, `SpreadArgument`, `Replaceable`, `Construct` | uncertifiable | the argument-to-parameter mapping is not exact, or a constructor |
| `CallbackBranch` | `SC1001` violation | control, another path: a branch in a callback the caller wrote stays under the premise (ADR 0275) |

`methodCall`'s own `o.m()` keeps its existing `SC9012` dispatch obligation in `helpers.tsx`.

`solid-js.d.ts`, `tsconfig.json` and `node_modules` are copied byte for byte from `summary-completion-read` (rc.9 signatures, see the header of `solid-js.d.ts`). Every helper signature is project code. `tsc --noEmit -p tsconfig.json` passes.

## ADR 0299 review

ObjectMethod's direct api.read proof is now withheld: this project contains
visible eval, which already vetoed the shared literal target map. The older
symbol-based summary path cannot bypass that veto. The getter callback's
existing invocation-context obligation remains. Other getter paths retain
their results.
