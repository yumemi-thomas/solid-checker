import { Show, createSignal } from "solid-js";

// ADR 1148 (round 2, N1): escapes that spell no name of the family (a string with a tab and an accented letter, an accented identifier, a template with an escaped letter) retype nothing: the control of this family, certified.
// Chrome (runtime-oracle `change-escaped-benign`): the control stays a checkbox, which `Grab`'s `textContent = ""` removes without delivering anything: quiet.
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
          const label = "tab\there\u00e9";
          const caf\u00e9 = `a\u0042c`.length + label.length;
          setN(caf\u00e9);
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
