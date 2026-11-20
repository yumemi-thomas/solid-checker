import { Show, createSignal } from "solid-js";

// ADR 1148 (round 1, F1): the control of `change-retyped-attr-node`, the same program without the two lines that reach the `Attr` node. The control stays a checkbox,
// which `Grab`'s `textContent = ""` removes without delivering anything: Chrome is quiet (runtime-oracle `change-attr-node-control`) and the proof stands.
function Grab() {
  (document.getElementById("wrap") as HTMLElement).textContent = "";
  return <span />;
}

export function Probe() {
  const [n, setN] = createSignal(0);
  const [show, setShow] = createSignal(false);
  return (
    <div>
      <div id="wrap">
        <input id="box" type="checkbox" onChange={() => setN(n() + 1)} />
      </div>
      <button
        id="go"
        onClick={() => {
          (document.getElementById("box") as HTMLInputElement).focus();
          document.execCommand("insertText", false, "x");
          setShow(true);
        }}
      >
        go
      </button>
      <Show when={show()}>
        <Grab />
      </Show>
      <p>{n()}</p>
    </div>
  );
}
