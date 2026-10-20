# value-origin-member-dispatch

**Claim (ADR 0211).** A helper invoking a parameter member leaves the implementation to each call site; when the argument's origin fixes its class it fixes the member: a fresh built-in (array literal, `Array.from`, `Object.keys/values/entries`, `split`, `new Date/Set/Map`) runs its prototype's member (nothing reactive); an exact project class instance runs `C`'s declared method. Origins follow `const` only and any assignment of the member name or prototype write voids them; stores, `let`, inherited/reassigned members and `Proxy` are `SC9012`.
