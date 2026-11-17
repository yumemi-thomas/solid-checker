# stored-throw-held-callbacks

**Claim (ADR 1044).** A function held in a `const` copy (`const a = f`), or as a property of a closed `const` object literal
(ADR 0351: no spread, computed key, accessor, duplicate key, `this` or `__proto__`, not exported, every reference a plain
member read that is neither optional, written nor deleted), runs where a use of that holder is called or handed off. Each
use is classed as a reference of the function is (`hand_off_at`: a call, a carrier that runs it inline, a fresh-stack
scheduler, a primitive's argument, a further copy), so the stored throw ends where that use's does.

SC4001 (NO_OWNER_CONTEXT) violations, uncertifiable `computation-error-unobserved` before: `BundleNullOwner`
(`runWithOwner(null, b.cb)`), `BundleTimer` (`setTimeout(b.cb, 0)`), `AliasTimer`, `AliasForEach` (`[1].forEach(a)` in a
timer), `AliasOfAlias`. `BundleMethod` (method shorthand) was proved before and stays proved. Still obligations, from the
owner net (an object slot's function may run anywhere, `function-run-unplaced`): `MemberCall` and `AliasOfMember`. The
obligations of this ADR's own refusals are the variant `stored-throw-held-callbacks-open`, a separate program because a
write of the key `cb` anywhere in a program hands every function stored under it (ADR 0967).
