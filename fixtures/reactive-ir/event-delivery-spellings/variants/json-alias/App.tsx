import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1133 round 1 (F4): `const J = JSON` hands `JSON` on; any name may then be `JSON.stringify` with a replacer.
export function Page() {
  let box!: HTMLDivElement;
  let link!: HTMLAnchorElement;
  let frame!: HTMLIFrameElement;
  createEffect(
    () => 1,
    () => {
      const J = JSON; J.stringify(box, (_key: string, value: unknown) => { if (typeof value === "function") (value as (event: Event) => void)(new Event("click")); return value; });
    },
  );
  return (
    <div ref={box} onClick={() => { count(); }}>
      <a ref={link} href="/x">x</a>
      <iframe ref={frame} />
    </div>
  );
}
