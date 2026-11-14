import { Loading, lazy } from "solid-js";

declare const moduleName: string;

const LazyAction = lazy(() => import("fz-lazy"));
const LazyNamed = lazy(() => import("fz-lazy-named").then((m) => ({ default: m.View })));
const LazyClear = lazy(() => import("fz-lazy-clear"));
const LazyLocal = lazy(() => import("./Local"));
const LazyVendor = lazy(() => import("./view.vendor.js"));
const LazyComputed = lazy(() => import(moduleName));
// A project module that only hands a package's export on is the package's code, whatever form the hand-off takes.
const LazyReexportDefault = lazy(() => import("./view-default"));
const LazyReexportNamed = lazy(() => import("./view-named"));
const LazyReexportStar = lazy(() => import("./view-star").then((m) => ({ default: m.View })));
const LazyReexportThen = lazy(() => import("./view-then").then((m) => ({ default: m.View })));
const LazyReexportImport = lazy(() => import("./view-import"));
const LazyReexportClear = lazy(() => import("./view-clear"));
const LazyReexportLocal = lazy(() => import("./view-local"));

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
// A relative module that re-exports a package's component: `export { default } from "pkg"`, `export { View as default } from "pkg"`, `export * from "pkg"` read with
// `.then((m) => ({ default: m.View }))`, `export { View } from "pkg"` read the same way, and `import View from "pkg"; export default View`. The row stands at the element,
// where Chrome throws, never in the module.
export function ReexportDefault() {
  return (
    <Loading fallback={null}>
      <LazyReexportDefault />
    </Loading>
  );
}
export function ReexportNamed() {
  return (
    <Loading fallback={null}>
      <LazyReexportNamed />
    </Loading>
  );
}
export function ReexportStar() {
  return (
    <Loading fallback={null}>
      <LazyReexportStar />
    </Loading>
  );
}
export function ReexportThen() {
  return (
    <Loading fallback={null}>
      <LazyReexportThen />
    </Loading>
  );
}
export function ReexportImport() {
  return (
    <Loading fallback={null}>
      <LazyReexportImport />
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
// A module that hands on a clear package's component, or the project's own, is silent too.
export function ReexportClear() {
  return (
    <Loading fallback={null}>
      <LazyReexportClear />
    </Loading>
  );
}
export function ReexportLocal() {
  return (
    <Loading fallback={null}>
      <LazyReexportLocal />
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
