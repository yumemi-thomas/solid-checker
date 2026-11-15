import { lazy, lazy as load } from "solid-js";
import * as S from "solid-js";
import { render } from "@solidjs/web";
import * as AlsoModule from "./Also";

// ADR 0992: a `lazy` loader renders the export it names under an owner (`solid-js@2.0.0-rc.21` `dist/solid.dev.js:1440`: `createMemo(() => createComponent(Comp, props), { sync: true })`),
// so a component whose only entry is that render is entered by a render, as a tag's is, and what its body creates has an owner. Only where the loader is exactly `() => import("…")`
// of one project module, `lazy` is Solid's by its import binding, the export is `default` (or `options.export`) and a function nothing rebinds, and no other read names it.
// Each component calls `onCleanup` in its body: the owner emission's SC4001 site (ADR 0970), an uncertifiable row where an entry may run it with no owner.

const PageLazy = lazy(() => import("./Page"));
const NamedLazy = lazy(() => import("./Named"), { export: "Named" });

// Open: a loader through a variable; a loader through `.then`; an alias and a namespace of `lazy`; a local `lazy`.
function pick() {
  return () => import("./Picked");
}
const loader = pick();
const PickedLazy = lazy(loader);
const ThenLazy = lazy(() => import("./ThenShape").then((m) => ({ default: m.ThenShape })));
const AliasLazy = load(() => import("./Aliased"), { export: "ViaAlias" });
const NamespaceLazy = S.lazy(() => import("./Aliased"), { export: "ViaNamespace" });
function lazyLocal<T>(fn: () => Promise<{ default: T }>) {
  return fn;
}
const mine = lazyLocal(() => import("./Mine"));
void mine;

// Open: an export rebound later; an export another read names.
const LateLazy = lazy(() => import("./Reassigned"), { export: "Late" });
const AlsoLazy = lazy(() => import("./Also"));
const alsoRef = AlsoModule.default;
void alsoRef;

function App() {
  return (
    <div>
      <PageLazy title="page" />
      <NamedLazy />
      <PickedLazy />
      <ThenLazy />
      <AliasLazy />
      <NamespaceLazy />
      <LateLazy />
      <AlsoLazy />
    </div>
  );
}

render(() => <App />, document.body);
