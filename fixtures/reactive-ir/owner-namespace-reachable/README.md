# owner-namespace-reachable

**Claim (ADR 0347).** In a closed program (`.solid-checker/runtime.json` asserts `programBoundary: "closed"`), an export a namespace import hands out has a caller the analysis cannot see: a defect proven unowned whatever the caller does is reported, a caller-dependent one stays uncertifiable.

Reported: `handedOut` (`member.ts`), read as a value in `App.tsx`; its `runWithOwner(null, …)` callback runs with no owner.

Uncertain: `handedOutDirect` (`member.ts`; the owner depends on the unknown caller); `viaObject` (`objects.ts`; `Object.entries` hands the values out unnamed, so whether they run is unproven; the calling case is the known miss in `docs/open-work.md`).

Silent: `quiet`, `unused` (`member.ts`, not reached through a namespace); `namedValue` (a named import used as a value, not a hand-out); `setup` (`member.ts`, called as `member.setup()` from `Host`, so owned).

Stubs: published rc.13 `solid-js.d.ts`, as in `owner-invoked-literal`.
