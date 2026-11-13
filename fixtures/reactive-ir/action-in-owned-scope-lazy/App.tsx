import { Loading, lazy } from "solid-js";

declare const moduleName: string;

const LazyAction = lazy(() => import("fz-lazy"));
const LazyNamed = lazy(() => import("fz-lazy-named").then((m) => ({ default: m.View })));
const LazyClear = lazy(() => import("fz-lazy-clear"));
const LazyLocal = lazy(() => import("./Local"));
const LazyVendor = lazy(() => import("./view.vendor.js"));
const LazyComputed = lazy(() => import(moduleName));

// B5: a component a package holds, loaded when the loader's promise settles, renders under an owner like any other.
export function Action() {
  return (
    <Loading fallback={null}>
      <LazyAction />
    </Loading>
  );
}
export function Named() {
  return (
    <Loading fallback={null}>
      <LazyNamed />
    </Loading>
  );
}
// A relative module the program does not analyze (a shim stands for its JavaScript) is code nobody reads, as an unread package is.
export function Vendor() {
  return (
    <Loading fallback={null}>
      <LazyVendor />
    </Loading>
  );
}
// A module named by anything but a literal may be any package's.
export function Computed() {
  return (
    <Loading fallback={null}>
      <LazyComputed />
    </Loading>
  );
}
// Silent: the package is clear, the module is the project's, and `preload` is the dialect's wrapper (the module evaluates later, on no stack of this call).
export function Clear() {
  return (
    <Loading fallback={null}>
      <LazyClear />
    </Loading>
  );
}
export function Local() {
  return (
    <Loading fallback={null}>
      <LazyLocal />
    </Loading>
  );
}
export function Preloaded() {
  void LazyAction.preload();
  return null;
}
