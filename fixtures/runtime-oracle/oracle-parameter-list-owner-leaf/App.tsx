import { onCleanup, onSettled } from "solid-js";

// rc.17 runs an `onSettled` callback with no arguments (`untrack(callback)`,
// `callback()` out of band: @solidjs/signals dist/dev.js), so a default in the
// callback's own parameter list runs in the leaf scope.
export function SettledDefault() {
  onSettled((x: unknown = onCleanup(() => {})) => {
    void x;
  });
  return <div />;
}
export function SettledBody() {
  onSettled(() => {
    onCleanup(() => {});
  });
  return <div />;
}
