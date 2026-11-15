import { createMemo } from "solid-js";
import { Portal, Portal as Overlay, render } from "@solidjs/web";
import * as Web from "@solidjs/web";
import { Shadowed } from "./Shadowed";

// ADR 0992: `Portal` reads its `children` in `createMemo(() => [startMarker, props.children])` under the owner `createOwner()` made (`@solidjs/web@2.0.0-rc.21` `dist/web.dev.js:2389-2399`),
// so what a creation written directly in them makes has an owner and a tracked read: the dialect says so (`Dialect::component_entry`), and the children are no getter that runs
// where an unknown reader calls it (ADR 0971's `getter-runs-where-read`). `mount` is no such slot: the render effect's apply reads it again under `untrack(mount)` (`:2402`).

// The tip's panel (fixtures/uncertifiable-repro `clean-portal-show`): the tracked `{text()}` is a render effect created under the portal's memo. Silent.
function Tip() {
  const text = createMemo(() => "tip text");
  return (
    <Portal>
      <div role="tooltip">{text()}</div>
    </Portal>
  );
}

// The same tag through an alias and through a namespace: the tag resolves to the declaration, not to the spelling. Silent.
function Aliased() {
  const text = createMemo(() => "aliased");
  return (
    <Overlay>
      <div role="tooltip">{text()}</div>
    </Overlay>
  );
}
function Namespaced() {
  const text = createMemo(() => "namespaced");
  return (
    <Web.Portal>
      <div role="tooltip">{text()}</div>
    </Web.Portal>
  );
}

// `mount` is read again in the render effect's apply: a creation written there is a getter that runs on an apply's stack. Still the code's uncertifiable rows.
function Mounted() {
  return <Portal mount={createMemo(() => document.body)()}>mounted</Portal>;
}

function App() {
  return (
    <div>
      <Tip />
      <Aliased />
      <Namespaced />
      <Mounted />
      <Shadowed />
    </div>
  );
}

render(() => <App />, document.body);
