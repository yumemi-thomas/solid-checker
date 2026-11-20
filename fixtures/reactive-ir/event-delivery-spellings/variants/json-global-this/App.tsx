import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1133 round 1 (F4): `globalThis.JSON.stringify(box, replacer)`: the replacer is handed every own enumerable value, Solid's stored handler among them.
export function Page() {
  let box!: HTMLDivElement;
  let link!: HTMLAnchorElement;
  let frame!: HTMLIFrameElement;
  createEffect(
    () => 1,
    () => {
      globalThis.JSON.stringify(box, (_key: string, value: unknown) => { if (typeof value === "function") (value as (event: Event) => void)(new Event("click")); return value; });
    },
  );
  return (
    <div ref={box} onClick={() => { count(); }}>
      <a ref={link} href="/x">x</a>
      <iframe ref={frame} />
    </div>
  );
}
