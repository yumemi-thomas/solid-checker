import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1133 round 1 (F2): `.call` hands the same sink on (the twin the first cut also missed).
export function Page() {
  let box!: HTMLDivElement;
  let link!: HTMLAnchorElement;
  let frame!: HTMLIFrameElement;
  createEffect(
    () => 1,
    () => {
      box.insertAdjacentHTML.call(box, "beforeend", "<img src=x on" + "error=\"document.querySelector('div').$" + "$click()\">");
    },
  );
  return (
    <div ref={box} onClick={() => { count(); }}>
      <a ref={link} href="/x">x</a>
      <iframe ref={frame} />
    </div>
  );
}
