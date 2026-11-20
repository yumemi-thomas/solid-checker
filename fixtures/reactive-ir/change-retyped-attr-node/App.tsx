import { Show, createSignal } from "solid-js";

// ADR 1148 (round 1, F1): an `Attr` node's `value` retypes the control and no key `type` is written. `document.evaluate` reaches the node, so the proof is withdrawn.
// Chrome (runtime-oracle `change-attr-node-retype`): the retyped, dirty control is removed by `Grab`'s `textContent = ""` while it is focused, which delivers `change`
// inside `Grab`'s owned body, so the setter below writes in an owned scope: REACTIVE_WRITE_IN_OWNED_SCOPE.
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
          const a = document.evaluate("//input[@id='box']/@type", document, null, 9, null).singleNodeValue as Attr;
          a.value = "text";
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
