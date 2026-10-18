# prototype-write-narrowed

**Claim (ADR 0314).** A write through a prototype under a key no proof names
(`R.prototype[key] = …`) vetoes the exact-member proofs of ADR 0209 and 0211
only for the objects whose prototype chain holds that prototype
(`SemanticLookup::member_may_be_reassigned_on_class`,
`member_may_be_reassigned_on_builtin`). It used to veto every such proof in
the program.

`source.ts` holds the two writer shapes the census narrows:

- `R` is a class, a function declaration, or a `const` bound to a function
  or class expression, declared in the writing file and never rebound
  (`Widget.prototype[key]`, and ecsy's shape in `defineRecord`): that
  constructor's instances, subclasses included;
- `R` is a parameter of a static method every access to which is a direct
  call through the class's own name, passing at that parameter a global
  whose value only TypeScript's default libraries declare
  (`OriginHelper.store(HTMLCanvasElement, …)`, spector's
  `storePrototypeOriginFunction`): those host prototypes. A host declared
  only in `lib.dom.d.ts` is in no ECMAScript value's chain and no project
  class's.

| Case | Finding | Why |
| --- | --- | --- |
| `ExactClass` | none | no written prototype is in `Counter`'s chain |
| `BuiltinInstances` | none | `Date`, `Set`: ECMAScript prototypes only |
| `WrittenClass` | `SC9012` uncertifiable | `Widget.prototype` is written |
| `WrittenBase` | `SC9012` uncertifiable | `Gadget`'s chain holds `Widget.prototype` |
| `ExtendsConstructor` | `SC9012` uncertifiable | `Derived` extends a call no class resolves |

The `exported-parameter-member-dispatch` obligations in `source.ts` are the
helpers' own: the fixture is not a closed application (ADR 0193).
`prototype-write-object-host` and `prototype-write-escaped-helper` are the
refusals. The sources pass `tsc --noEmit` with the fixture stub, whose
`createSignal`/`createStore` no case reads; `HTMLCanvasElement` and
`OffscreenCanvas` come from TypeScript's own `lib.dom.d.ts`.
