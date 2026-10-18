# factory-member-dispatch

**Claim (ADR 0314).** A helper that invokes a member of its parameter
(`tokens.waitForToken()`) leaves the implementation to each call site
(ADR 0211). When the argument is a `const` bound to the result of a plain call
of one exact, current, synchronous project function, and every value that
function returns is an object literal written in the return itself (a block
body's `return { … }`, or a concise arrow body that is the literal), the
member is the one each literal holds as its own property
(`SemanticLookup::factory_member_value_symbols`). Every literal must hold it;
no assignment in the program may write a member of that name; no reflective
definition may name it or use a key no literal names.

The same change makes the existing literal resolution exact
(`member_value_symbols`): only a `const` naming its whole initializer is
followed; the object must be exactly an object literal; only its own
properties and spreads count, not those of a literal nested in it or inside a
call's arguments; a getter, or any computed key, resolves nothing.
`ArgumentLiteral` was clean before: the property in `wrapTokens`'s argument
was read as a member of its result.

| Case | Finding | Why |
| --- | --- | --- |
| `FactoryResult` | none | `createTokens`'s literal holds `waitForToken` |
| `ArrowFactoryResult` | none | the same through a concise arrow body |
| `GetterResult` | `SC9012` uncertifiable | a getter's value is not the getter |
| `SharedResult` | `SC9012` uncertifiable | one return is a caller's object |
| `NestedResult` | `SC9012` uncertifiable | `waitForToken` is a nested literal's |
| `ArgumentLiteral` | `SC9012` uncertifiable | the argument's property is not the result's |
| `MethodResult` | `SC9012` uncertifiable | a method call is selected by its receiver |
| `MutableBinding` | `SC9012` uncertifiable | a `let` |
| `DestructuredBinding` | `SC9012` uncertifiable | a destructured name is a member of its initializer |
| `ReassignedMember` | `SC9012` uncertifiable | the program assigns a `rename` member |

Only the per-call-site parameter member resolution follows factories; a
direct `x.method()` callee is not resolved through them. Every literal holds
methods: a property whose value is an arrow (`waitForToken: () => 1`) is not
resolved by the existing summary lookup, so the negatives would otherwise
prove nothing. The `exported-parameter-member-dispatch` obligations in
`source.ts` are the helpers' own (ADR 0193). The sources pass `tsc --noEmit`
with the fixture stub, whose declarations no case reads.
