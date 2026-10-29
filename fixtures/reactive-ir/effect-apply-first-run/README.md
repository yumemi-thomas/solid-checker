# effect-apply-first-run

**Claim (ADR 0505).** On rc.17 a `createEffect` apply's first run is queued at the call and run by the tail `flush()` of the `render` that mounted the component (`@solidjs/web@2.0.0-rc.17` `dist/web.dev.js:542-568`). A pending render effect in that flush, or a transaction an earlier pass left, holds it (`@solidjs/signals@2.0.0-rc.17` `dist/dev-shared.js:1690-1733`, `:2180`). Where nothing can hold the flush, the apply meets an async memo created earlier in the body still pending, and `read()` throws `PENDING_ASYNC_UNTRACKED_READ`. So `pending-async-unsuspendable-read` (SC5001) is the claim, and the strict read is withdrawn.

Nothing can hold the flush when:
- the component renders a static intrinsic tree;
- the body runs nothing but the effect, the source and literal primitives;
- no project module's scope runs code;
- every module loaded is Solid's own or an analyzed project file;
- the entry runs only standard-library code before this `render`.

Violations: `Proven` (top level), `variants/layered` (a tuple taken apart and an inert local function in the body, a closure in the apply).

Uncertifiable (SC1001 stays), one premise each:
- `RenderedSecond`: an earlier `render`;
- `fragment`;
- `component-child`;
- `expression-child`;
- `helper-call`: a body call of a helper;
- `member-read`: a body member read;
- `throws`: a body `throw`;
- `guarded`: a read the first run skips;
- `module-code`: module-scope code;
- `entry-code`: project code the entry calls before `render`;
- `declaration-import`: a module with only a declaration file.

Stubs: `node_modules` and `jsx.d.ts` copied from `render-apply-first-run` (rc.17 published declarations); the variants share them.
