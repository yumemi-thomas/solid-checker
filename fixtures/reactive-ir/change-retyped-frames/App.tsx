import { createSignal } from "solid-js";
import { createFrame } from "@solidjs/web/frames/client";

// ADR 1148 (round 1, F2): the frames tier morphs the children it finds with the server's markup (`frames/dist/wire.js`, lines 73-97), `type`
// included, so a server that answers with a text input in place of this checkbox retypes it. The dialect audits `@solidjs/web/frames/client` as writing any element,
// so the import withdraws the proof; the runtime exclusion of the file hid it before. (This comment spells no name of the DOM's attribute API: the import is the
// whole difference.)
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
