# withheld-object-member

**Claim (ADR 0320, review round 6).** A member call on an exact literal binding whose receiver census
(ADR 0299) does not hold (here another module assigns `api.read`) runs an implementation nothing selects: it
is an `SC9012` dispatch obligation wherever its reads could be reported (not in tracked JSX, an event or a
deferred callback). `DirectRead`: `SC9012` uncertifiable (`mutate.ts` replaces `api.read` with a function
reading a signal). `solid-js.d.ts`, `tsconfig.json`, `node_modules` are copied from `invocation-flow`;
`package.json` is private. Sources pass `tsc --noEmit` with the fixture stub.
