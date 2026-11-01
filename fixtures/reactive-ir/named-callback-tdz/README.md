# named-callback-tdz

**Claim (ADR 0517).** A primitive's callback position that names a `const` function from a nested function run before the function's declarator (`const early = () => createMemo(write); early(); const write = ...`) throws `ReferenceError` instead of handing the function over (rc.17 in Chrome: "Cannot access 'write' before initialization", no Solid diagnostic; `tsc` is silent in a nested function). The function's role there is unknown, so a violation in its body is uncertifiable. And a violation written after an unordered mention in the same function (`[1].forEach(() => { r(); count(); })`, `r` declared later) is uncertifiable: the mention may throw first.

`Early.tsx` (uncertifiable): a memo compute that writes (`SC2001`), an effect apply that reads (`SC1001`), and a read after an early mention (`SC1001`).

`Late.tsx` (violations, each confirmed by rc.17 in Chrome: `REACTIVE_WRITE_IN_OWNED_SCOPE`, `STRICT_READ_UNTRACKED`): the nested function written before the declarator and called after it, and the read before the early mention.

Probes: `scripts/fuzz/probes/tdz/`.
