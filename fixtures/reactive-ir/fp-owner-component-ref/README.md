# fp-owner-component-ref

A `ref` callback written on a component is a prop the component calls itself, not an ownerless ref application: only an intrinsic element's `ref` is applied by the runtime with no owner (`@solidjs/web@2.0.0-rc.9`), and the compiler's ref-application callback role is honoured only on intrinsic tags (`owners::ref_application_on_intrinsic`), so `onCleanup`/`createEffect` in `<Box ref={...}>` are not `missing-owner`/directive-creation violations; `<input ref>` stays a proven `missing-owner`. Stub from `fp-owner-loading-cover`; `tsc` clean against real rc.9.
