# use-site-role

**Claim (ADR 0315).** Code in a function literal a project callee runs only through one parameter (or a property of a closed options literal, or a component prop) takes the one role every call runs in, classified in the callee's file (`callee_use_site_role`); the callee may read other properties, test, call or forward it as exactly another component's prop; every call must be an event, deferred callback or tracked computation, the same for all (`SC1001` uncertifiable otherwise; write legality never asks). `createEffect` stub from published rc.13.
