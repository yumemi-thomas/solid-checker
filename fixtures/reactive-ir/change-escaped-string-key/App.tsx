import { Show, createSignal } from "solid-js";

// ADR 1148 (round 2, N1): the XPath entry point and the result's node accessor are string keys written with an escape.
// Chrome (runtime-oracle `change-escaped-string-key`): the retyped, dirty control is removed by `Grab`'s `textContent = ""` while it is focused, which delivers `change` inside `Grab`'s owned body, so the setter and the read warn (REACTIVE_WRITE_IN_OWNED_SCOPE, STRICT_READ_UNTRACKED).
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
          const found = (document as any)["ev\u0061luate"]("//input[@id='box']/@type", document, null, 9, null);
          const a = found["singleNodeV\u0061lue"] as Attr;
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
