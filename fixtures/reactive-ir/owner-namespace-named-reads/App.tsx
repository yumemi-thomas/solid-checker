import { lazy } from "solid-js";

// Solid's `lazy` reads only the `default` export of ./page. `helper`, the page's
// other export, is never handed out, so its detached callback stays dormant.
export const Page = lazy(() => import("./page"));

// `lazy` with an `export` option reads that export, `Named`, and only it.
export const NamedPage = lazy(() => import("./page"), { export: "Named" });

// A destructured read of an awaited import: `reached` is handed out by name,
// `neverCalled` in ./other is not.
export const reachedRead = (async () => {
  const { reached } = await import("./other");
  return reached;
})();

// A member read of an awaited import: `handedOut` is handed out by name,
// `unread` in ./member is not.
export const memberRead = (async () => (await import("./member")).handedOut)();

// An unnamed escape: `Object.keys` takes the names of ./escapes and nothing
// names an export, so `escaped` is uncertain, never a violation (ADR 0347 amendment).
export const escaped = (async () => Object.keys(await import("./escapes")))();
