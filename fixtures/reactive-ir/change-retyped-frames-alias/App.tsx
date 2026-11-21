import { createSignal } from "solid-js";
import { createFrame } from "@frames/client";

// ADR 1148 (round 1): the same frames tier as `change-retyped-frames`, reached by a `paths` alias instead of its package specifier. The dialect audits the specifier
// `@solidjs/web/frames/client`, and an alias is no such specifier: the import resolves into a file of the runtime package by a name no audit lists, which is unknown and
// taken as writing. (This comment spells no name of the DOM's attribute API: the import is the whole difference.)
export function Probe() {
  const [n, setN] = createSignal(0);
  return (
    <div>
      <div id="wrap">
        <input id="box" type="checkbox" onChange={() => setN(n() + 1)} />
      </div>
      <button id="go" onClick={() => createFrame(document.getElementById("wrap")!)}>
        go
      </button>
      <p>{n()}</p>
    </div>
  );
}
