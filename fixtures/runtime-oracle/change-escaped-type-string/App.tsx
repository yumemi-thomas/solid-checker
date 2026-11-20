import { Show, createSignal } from "solid-js";

// ADR 1148 (round 2, N1): the property written is `type`, a string key written with an escape (the write's literal key is decoded: this form was vetoed before the facts held the decoded words).
// Chrome (runtime-oracle `change-escaped-type-string`): the retyped, dirty control is removed by `Grab`'s `textContent = ""` while it is focused, which delivers `change` inside `Grab`'s owned body, so the setter and the read warn (REACTIVE_WRITE_IN_OWNED_SCOPE, STRICT_READ_UNTRACKED).
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
          const box = document.getElementById("box") as HTMLInputElement;
          box["t\u0079pe"] = "text";
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
