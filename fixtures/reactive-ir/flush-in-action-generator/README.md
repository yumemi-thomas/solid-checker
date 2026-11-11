# flush-in-action-generator

**Claim (ADR 0961).** A generator no walk proves to be an action's step body, written in a program that names `action`, may run in a step: a flush in it is an uncertifiable row of SC2006 (and SC2008: it may also run in an effect's apply), never silence. The generator literal written as the action's own argument keeps the walk's proof (`direct`: a violation); a generator nothing steps (`discardedBody`, `quiet`) is silent.

`App.tsx`: the generator handed to a helper that makes the action (`useSave(function* …)`, `useSave(namedBody)`), delegated to by `yield*` (one level and two), and the iterator an action's step keeps (R1): `Array.from`, `new Set`, `.next()` on the call or on a stored iterator, a loop over a stored iterator. Chrome (rc.21) throws `[FLUSH_IN_ACTION]` at every flush but the discarded and the quiet ones.
