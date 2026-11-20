import { Show, createSignal } from "solid-js";

// ADR 1148 (round 2, N1): escapes that spell no name of the family (a string with a tab and an accented letter) retype nothing: the control of this family, certified. ADR 1152 holds a program that writes an identifier, or a template that is one word, with an escape uncertifiable as a whole, so those two forms of `change-escaped-benign` live there.
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
          const cafe = `abc`.length + label.length;
          setN(cafe);
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
