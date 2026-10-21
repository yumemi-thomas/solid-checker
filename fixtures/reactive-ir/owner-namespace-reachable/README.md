# owner-namespace-reachable

**Claim (ADR 0347).** In a closed program (`.solid-checker/runtime.json` asserts `programBoundary: "closed"`), an exported function that a namespace import hands out has a caller the analysis cannot see, so it is reachable with an unknown caller. A defect proven unowned whatever the caller does is then reported; a defect that depends on the caller stays uncertifiable.

Reported, the namespace hand-out reaching a detached callback:
- `handedOut` (`member.ts`): `member.handedOut` is read as a value in `App.tsx`, and its `runWithOwner(null, …)` callback runs with no owner.

Uncertifiable, not a violation:
- `handedOutDirect` (`member.ts`), handed out as a value, whose effect sits directly in its body. Its owner depends on the unknown caller.
- `viaObject` (`objects.ts`): `Object.entries(objects)` hands the namespace's values out with no name the program reads. Its detached callback is reached only through that unnamed escape, so whether it runs is unproven (ADR 0347 amendment). The values are never called here; the case where the program calls them is the recorded known miss in `docs/open-work.md`.

Silent, the dormancy a closed program keeps:
- `quiet`, `namedValue` and `unused` (`member.ts`): nothing reads them through a namespace. `namedValue` is a named import used as a value, which is not a namespace hand-out.
- `setup` (`member.ts`): `member.setup()` is a namespace call from the component `Host`, so its caller is known and it is owned.

Stub declarations are copied from published rc.13 (`solid-js.d.ts`, the same declarations as `owner-invoked-literal`).
