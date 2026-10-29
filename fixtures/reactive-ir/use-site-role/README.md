# use-site-role

**Claim (ADR 0315).** Code in a function literal a project callee runs only through one parameter (or a property of a closed options literal, or a component prop) takes the one role every call runs in, classified in the callee's file (`callee_use_site_role`); the callee may read other properties, test, call or forward it as exactly another component's prop; every call must be an event, deferred callback or tracked computation, the same for all (`SC1001` uncertifiable otherwise; write legality never asks). `createEffect` stub from published rc.13.

ADR 0506: `UsesWrite` (`writeOf({ bump: () => setCount(1) })`, whose memo compute calls `options.bump()`) is uncertifiable. Its violation was proven only by taking `value()` as a call of the arrow (the binding holds the memo, not the arrow), the same reading that made a write in an arrow a wrapper never runs a violation; the use-site arm places no write, so no proof is left.
