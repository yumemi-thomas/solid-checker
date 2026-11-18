# hook-member-helper-read

**Claim (ADR 1093).** A call of a getter a hook's returned literal carries (`const thing = useThing(); thing.useIt()`), made in a local helper a JSX child reads, is the read of that getter: it hands over no value, so ADR 0771's obligation for reads "this call makes through a value it hands over" (`parameter-call-argument-unproven`) does not apply, and the read is the helper's, claimed where the JSX child runs it (tracked).

| component | outcome |
|---|---|
| `Shown` | proved: no row (before: SC9012 `parameter-call-argument-unproven`) |
| `BodyHelper` (the helper called in the body) | SC1001 violation |
| `BodyRead` (the member read in the body) | SC1001 violation |

`solid-js.d.ts` is `async-creation-pass`'s.
