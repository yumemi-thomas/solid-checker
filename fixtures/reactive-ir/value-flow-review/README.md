# value-flow-review

Regression cases for the 2026-10-06 soundness review of ADRs 0209 to 0214 (ADR 0215). Each one is a reproducer the review found to be certified clean, or reported, wrongly; each now stays an `SC9012` obligation, except `GeneratorMethod`, which is now clean. The program is closed (a private `package.json`), so the pass-through discharges (ADR 0213, 0214) apply.

| Case | File | Result | Why |
| --- | --- | --- | --- |
| `PrototypeKey` | `Leaf.tsx` | `SC9012` | `Patched.prototype[key]` with `const key = "run"` names `run` |
| `ConstructorReturn` | `Leaf.tsx` | `SC9012` | the constructor returns another object |
| `GeneratorMethod` | `Leaf.tsx` | none | calling a generator runs none of its body |
| `DefaultParameter` | `Leaf.tsx` | `SC9012` | the invoked method's default may run |
| `ThenableWithoutCallback` | `Leaf.tsx` | `SC9012` | a `PromiseLike`'s `then` is user code |
| `ReassignedHelper` | `Leaf.tsx` | `SC9012` | a reassigned `let` is not its initializer |
| `GetterArgument` | `Leaf.tsx` | `SC9012` | `Object.values` runs the literal's getter |
| `Rejoined` | `App.tsx` | `SC9012` | `list.join` is assigned |
| `OverwrittenPrevious` | `App.tsx` | `SC9012` | the updater writes its previous value |
| `Destructured` | `App.tsx` | `SC9012` | `[list] = …` rebinds the parameter |
| `LocalArray` | `App.tsx` | `SC9012` | a local `Array` shadows the global |
| `ProtocolSeparator` | `App.tsx` | `SC9012` | `split` calls the separator's `Symbol.split` |
| `MemoOptions` | `App.tsx` | `SC9012` | a memo with options |
| `Aliased` | `App.tsx` | `SC9012` | `props` is aliased |

The `SC4001` at `Defaulted`'s default is the missing-owner rule on that `onCleanup`, and the `SC1001` findings are the body reads of `items()`; neither is part of these claims. Each class uses its own method name, because a name written anywhere (`Patched.prototype[key]`) refuses that name everywhere. The scaffolding is copied from `parameter-pass-through-dispatch`.
