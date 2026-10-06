# value-origin-member-dispatch

**Claim (ADR 0211).** A helper that invokes a member of its parameter (`list.filter(…)`) leaves the implementation to each call site. When the argument's origin fixes its class, it fixes the member that runs (`SemanticLookup::value_origin`):

- a fresh built-in value (an array literal, an array built by `Array.from`, `Object.keys`/`values`/`entries`, `split` or an array method of a proven array, or `new` of a reviewed value class such as `Date`, `Set` or `Map`) runs its prototype's member, which reads nothing reactive;
- an instance of exactly a project class (`new C(…)`) runs the method `C` declares, whose summary is used.

The origin is followed through `const` bindings only, and nothing applies where the program assigns a member of that name or writes a prototype.

| Case | Finding | Why |
| --- | --- | --- |
| `ArrayLiteral` | none | an array literal's `filter` |
| `ArrayBinding` | none | a `const` bound to an array, and `slice` of it |
| `FreshArrays` | none | `Array.from`, `Object.values`, `split` and `map` build plain arrays |
| `BuiltinInstances` | none | `Date` and `Set` built here |
| `ExactClass` | none | `Counter.bump` reads nothing |
| `ExactClassRead` | `strict-read-untracked` uncertifiable | `Live.now`'s read of `count` reaches the call site; that it runs during the call is proven only through plain calls (ADR 0204), not `live.now()` |
| `StoreArray` | `SC9012` uncertifiable | a store array is a proxy |
| `MutableBinding` | `SC9012` uncertifiable | a `let` |
| `InheritedMethod` | `SC9012` uncertifiable | inherited methods are not followed |
| `ReassignedMethod` | `SC9012` uncertifiable | the program assigns a `run` member |
| `ProxyValue` | `SC9012` uncertifiable | `Proxy` is not a value class |

The `exported-parameter-member-dispatch` obligations at `bump`, `peek` and `run` in `source.ts` are their declarations' own: the fixture is not a closed application, so a caller outside it may pass anything (ADR 0193).

`solid-js.d.ts` declares `createSignal` and `createStore` with the tuple shapes `dialect-solid-2` uses; no finding here depends on the setters' signatures.
