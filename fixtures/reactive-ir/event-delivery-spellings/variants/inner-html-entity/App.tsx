import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1133 round 1 (F5): the same reference in `innerHTML` markup (the twin the first cut also missed).
export function Page() {
  let box!: HTMLDivElement;
  let link!: HTMLAnchorElement;
  let frame!: HTMLIFrameElement;
  createEffect(
    () => 1,
    () => {
      box.innerHTML = '<a href="&#106;avascript:void 0">x</a>';
    },
  );
  return (
    <div ref={box} onClick={() => { count(); }}>
      <a ref={link} href="/x">x</a>
      <iframe ref={frame} />
    </div>
  );
}
