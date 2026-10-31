# loading-on-outside-hold

**Claim (ADR 0517).** `loading-on-outside-hold` (SC5006) is an obligation, never a violation: rc.17 warns `LOADING_ON_OUTSIDE_HOLD` when a `<Loading on>` re-armed by a change waits on a source an outside frame reader holds in the same flush, which rests on runtime state the facts do not carry. A boundary with `on` under which an async source is read that is also referenced outside it, or under which a component renders, is uncertifiable instead of a certified clean.

`App.tsx`: `OutsideRead` (rc.17 in Chrome warns after the click; `scripts/fuzz` recall grid), `ComponentChild`, `ChildrenAttribute` and `HookSource` (a memo a hook in another module returns) are uncertifiable at `on`; `InsideOnly` (no outside reader) and `NoOn` (no re-arm) carry no SC5006.

Stub: `Loading`'s `on?: any` as in the published rc.17 `types/client/flow.d.ts`.
