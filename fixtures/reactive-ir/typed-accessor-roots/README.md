# typed-accessor-roots

**Claim (ADR 0294).** A dialect type alias never supplies the reactive-read premise. `Accessor<T>` is `() => T`, which every plain function satisfies, so a value typed `Accessor<T>` is proven a source only by what it holds: a source it provably holds (through a `const` alias, a closed literal member, a tuple slot of an exact factory) is a proven root; a function literal it provably holds (directly, or every return of an exact factory) is decided by that function's own body; anything else is a typed candidate, whose read in a reporting role is an `SC9012` obligation, never an `SC1001`. The effect-apply descriptor shortcut follows the same rule, and the type-only interprocedural summary rows are gone.

| Case | Finding | Why |
| --- | --- | --- |
| `LiteralMember`, `InterfaceMember`, `LocalVariable`, `ModuleVariable`, `ReturnedVariable`, `PlainUntyped` | none | the value is a plain function the project wrote |
| `PlainParameter` | none | the helper's argument is a plain function; the typed summary row is gone |
| `EffectApplyPlain` | none | the shortcut's callee holds a plain function |
| `AliasedSignal` | `SC1001` violation | `const get: Accessor<number> = count` holds the signal |
| `ReturnedSignal` | `SC1001` violation (existing) | ADR 0222's returned source |
| `ParameterSignal` | `SC1001` violation (existing) | the argument is the signal; the duplicate typed summary row is gone |
| `DeclaredValue` | `SC9012` uncertifiable | `declare const opaque: Accessor<string>` |
| `ClassMember` | `SC9012` uncertifiable | a class field is not judged |
| `DestructuredProp` | `SC9012` uncertifiable | a destructured parameter is not judged (its `SC1003` is unchanged) |
| `DeclarationMember` | none | ADR 0299 follows the exact imported const object in `impl.ts`; its own function returns a plain string despite the `.d.ts` interface |
| `EffectApplyWritten` | `SC9012` uncertifiable | the shortcut: a written binding has no typed entry |

Review round 1:

| Case | Finding | Why |
| --- | --- | --- |
| `SpreadTupleSlot` | `SC9012` uncertifiable | a spread before the slot: syntactic element 1 is not runtime slot 1, so the tuple-slot judge refuses it |
| `ExactTupleSlot` | `SC1001` violation | no spread: slot 1 is the signal |
| `VisibleReadingMember` | `SC1001` violation at `readHolder()` | ADR 0299 follows the exact stable member's function through the synchronous helper; its body reads the locally created signal |
| `DirectReadingMember` | `SC1001` violation | ADR 0299 follows the same exact member in the component body; no duplicate type-only obligation |
| `InertHeldMember` | none | `() => 1` provably runs nothing |

Before ADR 0294 every case but `PlainUntyped` was an `SC1001` violation, and `ParameterSignal` had a second, typed summary row. `Store<T> = T` follows the same rule in source discovery, but Type Facts does not keep that alias on a declared value here, so no case reaches it.

`solid-js.d.ts`, `tsconfig.json` and `node_modules` are copied from `returned-store-proxy`: an ambient `declare module "solid-js"` whose `Accessor` is byte-faithful to `@solidjs/signals@2.0.0-rc.13` `dist/types/signals.d.ts:55`. The ambient origin is what lets the fallback see the alias; against the real installed rc.13 packages Type Facts gives the alias no origin module and the fallback is never reached (research `store-fn-and-typed-accessor.md`). `tsc --noEmit` passes against the stub and against installed rc.13 (`strict`, `skipLibCheck: false`).
