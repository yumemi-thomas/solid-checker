# fp-owner-component-ref

**Claim.** A `ref` callback written on a component is a prop the component
calls itself; it is not an ownerless ref application. Only an intrinsic
element's `ref` is applied by the runtime through `ref()` with no owner
(`@solidjs/web@2.0.0-rc.9`). The compiler's ref-application callback role is
honoured only on intrinsic tags (`owners::ref_application_on_intrinsic`: by
JSX semantics a lowercase, non-member tag), in the owner, execution-role and
directive-creation stages.

Before, `onCleanup` and `createEffect` inside `<Box ref={...}>` were proven
`missing-owner` and `primitive-in-directive-application` violations, as in the
real-app sweep (`solid-pixi` components in app-game, 3 findings). The
intrinsic `<input ref>` positive control stays a proven `missing-owner`.

`solid-js.d.ts` is copied from `fp-owner-loading-cover`. `App.tsx` passes
`tsc --noEmit` against the stub and the real rc.9 install.
