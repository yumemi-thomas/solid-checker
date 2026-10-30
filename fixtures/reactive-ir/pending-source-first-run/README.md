# pending-source-first-run

**Claim (ADR 0510).** ADRs 0504-0505 prove an apply's first run meets an async memo of the component's own body pending. The source may also be:
- a module-scope `createMemo(async () => <literal>)` of a project module whose scope only creates nodes (memos and signals of literals, contexts), read under the program's first `render`, whose entry no module imports (`source.ts`, `ModuleRender`);
- a memo the mounting parent creates in its body and hands over as an exact prop (`PropParent`) or through one context provider whose value object holds it (`ContextParent`).

Each is pending until a microtask (`@solidjs/signals@2.0.0-rc.17` `dist/dev-shared.js:3640-3690`, `:4229-4242`), module evaluation, the entry's `render` and the parent's tree are one synchronous job, and an unread loading source joins no transaction (`:2194-2204`, `:4520`). So `pending-async-unsuspendable-read` (SC5001) is the claim.

Uncertifiable, one premise each: `ModuleSecond` (not the first render), `CallingParent` (the parent runs code), `WrittenParent` (the child writes the prop first), `LateParent` (`ctx` is read before its declaration), `ThrowInIife`, `ThrowInWrapper`, `ThrowInHelper` (a call before the read in a layer of the apply throws first; before ADR 0510 these were SC5001), `ThrowBeforeWrite` (the same for ADR 0503's write, SC2001 before).

Variants: violations `module-effect` (a user effect, ADR 0505), `same-file` (the component's own module), `context-effect` (a user effect under the provider), `settled-children` (a static tree with string attributes, a literal child and reads of literal signals and memos); uncertifiable `module-code`, `module-compute`, `module-await`, `entry-imported`, `body-code`, `provider-getter` (a getter in the provided value: no exact transport, SC9012), and the review's `body-member` (a member read of an `any` in the body throws first), `apply-member` (a member read before the read in the apply) and `second-root` (`warm.ts`, a root none imports, also loads the source); the three were SC5001 at the ADR 0510 commit.

SC5003 (ADR 0513): `module-render-read` (a render effect's compute reads the module memo, no boundary) is a violation; `module-render-await` (the same with `await 0` in `source.ts`) is uncertifiable, as the await may settle the memo before the read. The same for SC5001 (review): `module-body-settled` (a root reads the memo, the module awaits a timer, the body reads it: rc.17 warns `STRICT_READ_UNTRACKED` only), `module-body-await` (an unread memo: Chrome throws, a precision cost) and `module-for-await` (`for await` counts as a top-level await) are uncertifiable.

Stubs: `node_modules` and `jsx.d.ts` from `child-mount-first-run` (rc.17 published declarations), with `createContext`, `useContext`, `Context` and `Accessor` added to `solid-js` from `types/index.d.ts` and `types/client/core.d.ts`; `package.json` makes the program closed (context transport). The variants share them.
