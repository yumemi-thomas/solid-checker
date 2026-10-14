# tuple-slot-provenance

**Claim (ADR 0282).** `const [running, start, stop] = make(); start()` calls
exactly the closure in slot 1 of the array literal that `make`'s one
unconditional return builds. The array is fresh and the pattern consumes it in
the same statement, so the slot's closure is followed like `const { start } =
make()` (ADR 0273's object form), and ADRs 0275–0280 decide whether its read
is a violation. Only an exact `const` pattern of plain identifier slots bound
to that call's own result is admitted. The slot must be an inline arrow, or the
unwritten `const` arrow binding that the factory itself declares. Everything
else is refused: nothing is claimed, and an obligation that existed before
stays.

| Case | Finding | Why |
| --- | --- | --- |
| `Loop` | `SC1001` violation | the `createRAF` shape without its `isServer` guard; `stop` escaping into `onClick` replaces nothing |
| `LeadingHole` | `SC1001` violation | `[, start]` skips a position, every other stays exact |
| `DefaultImport` | `SC1001` violation | the same factory through `export { makeLoop as default }` |
| `SameFile` | `SC1001` violation | a factory in the component's own file |
| `InlineSlot` | `SC9012` uncertifiable | the slot is an inline arrow, which is outside the producer's function universe (as an inline object member is, `fp-summary-returned-closure`) |
| `InJsx`, `InMemo` | none | the call runs in a tracked scope |
| `Untracked` | none | `untrack` |
| `NoRead` | none | the slot reads nothing |
| `AccessorSlot` | `SC1001` violation (unchanged) | slot 0 is an accessor; it already resolved before this ADR and is not claimed twice |
| `OnlyAccessor` | none | only the accessor, in JSX |
| `Guarded` | none | two return sites behind an undecidable `flag` (slice 2: host constants) |
| `MutablePattern` | none | `let` |
| `DefaultedSlot`, `RestPattern`, `NestedPattern` | none | a default, a rest or a nested pattern |
| `HoledLiteral`, `SpreadLiteral` | none | a hole or a spread in the returned literal moves positions |
| `AliasedTuple` | none | `const t = make(); const [, s] = t` destructures a held array |
| `SpreadAccessor` | none | the returned literal spreads two plain functions before its accessor, so slot 1 is a plain function |
| `IndexedTuple` | `SC9012` uncertifiable (unchanged) | `t[1]()` keeps its computed-callee obligation |
| `StoredLiteral` | none | the literal is stored, then returned |
| `ExpressionSlot` | none | a `function` expression slot: arrows only, as in `member_value` |
| `SharedSlot` | none | a module binding, not one the factory declares |
| `ReplaceableSlot` | none | a `let` slot binding |
| `Forwarded` | none | `return makePeek()` forwards another factory's tuple |
| `Branched` | none | the initializer is a conditional, not one call |
| `ImportedSlot` | none | a module-level pattern in another file than the call |

`factories.ts:18` `onCleanup(stop)` is the existing open-world `SC4001` of an
exported factory, unrelated to the claim. `tuple-slot-context` holds the
context-provider refusal, which needs a closed program.

`solid-js.d.ts`, `tsconfig.json` and `node_modules` are copied from
`summary-completion-read`, unchanged; `createSignal`, `createMemo`, `onCleanup`
and `untrack` keep the signatures that stub copies from the published
typings. The cases are not type errors: `tsc --noEmit -p` passes.

## Round-3 bounded tuple authority

The mixed held/indexed/mutable callable shapes in this project now retain an
unknown visible gateway possibility under ADR 0282's bounded census. Its five
former SC1001 rows become SC9012 and four additional tuple calls remain explicit
uncertifiable candidates. The intact isolated tuple control lives in
`tuple-iteration-veto/cases.json`; this mixed project no longer establishes an
absence premise for every callable slot. This records conservative withholding,
not an observed iterator mutation.
