# foreach-carrier

**Claim (ADR 1124).** A reference to a named callback that is a whole element of a spread-free array literal, the direct receiver of
the standard `Array.prototype.forEach` (Type Facts, no impostor veto), whose first argument is an inline arrow with one plain,
never-written parameter called only in its own body, is a call of the callback at that `forEach` call: the entry census counts it
(ADR 1119), and it is classified as a read written at the call would be. Clean: `MemoCarriedInHandler`, `SettledCarriedInTimer`,
`EffectComputeCarriedInHandler`. Uncertifiable where the call reports (`MemoCarriedInBody`, `MemoCarriedInApply`) and in every twin:
`TwinMap`, `TwinNonLiteral`, `TwinReassigned`, `TwinPassedOn`, `TwinNested`, `TwinSpread`, `TwinSecondParameter`, `TwinAlias` (a
carried alias is no entry: its call would be classified by nothing). Chrome (`runtime-oracle/foreach-carrier`): quiet at the clean
three, covered elsewhere. Stub: `named-callback-escaped-entry`'s.

**Round 2 (review U1).** The carried calls are enumerated over `write_roles::entry_bindings`, the census's list: a named function
expression's `const` (`[cb]` with `const cb = function inner() {}`) and its same-named binding (`function cb`) carry entries the census
counts and the roles classify. Uncertifiable: `NamedMemoCarriedInBody`, `NamedSameCarriedInBody`, `NamedSettledCarriedInBody`,
`NamedEffectCarriedInApply`. Clean: `NamedMemoCarriedInHandler`. The reviewed head certified the first four clean.
