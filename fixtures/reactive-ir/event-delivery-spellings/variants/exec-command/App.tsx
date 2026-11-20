import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1133 round 1 (adjacent spelling of F5): `execCommand("insertHTML", …)` parses markup into the editable element as `insertAdjacentHTML` does.
export function Page() {
  let box!: HTMLDivElement;
  let link!: HTMLAnchorElement;
  let frame!: HTMLIFrameElement;
  createEffect(
    () => 1,
    () => {
      box.contentEditable = "true"; box.focus(); document.execCommand("insertHTML", false, "<img src=x on" + "error=\"document.querySelector('div').$" + "$click()\">");
    },
  );
  return (
    <div ref={box} onClick={() => { count(); }}>
      <a ref={link} href="/x">x</a>
      <iframe ref={frame} />
    </div>
  );
}
