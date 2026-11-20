import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1133 round 1 (F2): `Reflect.apply(box.insertAdjacentHTML, …)` writes markup with an event attribute, the sink the written call is; Reflect.apply only calls a function whose own read is checked, and this one is read as a value.
export function Page() {
  let box!: HTMLDivElement;
  let link!: HTMLAnchorElement;
  let frame!: HTMLIFrameElement;
  createEffect(
    () => 1,
    () => {
      Reflect.apply(box.insertAdjacentHTML, box, ["beforeend", "<img src=x on" + "error=\"document.querySelector('div').$" + "$click()\">"]);
    },
  );
  return (
    <div ref={box} onClick={() => { count(); }}>
      <a ref={link} href="/x">x</a>
      <iframe ref={frame} />
    </div>
  );
}
