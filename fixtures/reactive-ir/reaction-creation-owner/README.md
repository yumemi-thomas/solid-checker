# reaction-creation-owner

ADR 0549: rc.17's `createReaction` captures the owner current at its call, and every `track()` runs `runWithOwner(owner, () => effect(…))` (`@solidjs/signals@2.0.0-rc.17` `dist/dev.js:2630-2651`): with no owner, `effect()` warns NO_OWNER_EFFECT at each `track()` (Fable's precision report 2026-11-02, probe R09). `trackModule` (created at module scope) is a violation where it is tracked at module scope and in the handler; `trackShared`, created at `helpers.ts`'s top level, too. `trackOwned` (created in the body) and `fromHook` (created by a hook the body calls) are owned wherever they are tracked: silent. `trackLet` is a `let`: the call is not followed (silent, a known miss in open-work).

The stub reduces the published rc.17 declarations; `@solidjs/signals` is pinned to rc.17 for the release key. The complete source also type-checks against published solid-js rc.17 with strict ES2022 settings. Package bytes are not copied here.
