# leaf-scope-skippable-await

**Claim (ADR 0320, round 7).** The leaf-owner walk skips a call after an `await` its helper reaches on every path (a promise continuation: `missing-owner`'s question); an await that may be skipped (`x ??= await p`, `callback?.(await p)`, `table?.[await p]`, a labelled `break`, a destructuring default) is not such an await, so the call stays a `leaf-owner-forbidden-call` uncertifiable obligation (`UnconditionalAwait` none). The Type Facts `calls_after_await` scan treats the same constructs (`Owner.tsx`).
