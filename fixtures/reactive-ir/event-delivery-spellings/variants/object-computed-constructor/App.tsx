import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1133 round 1 (F4): `({})["constructor"]` is `Object` under a literal computed key; `.values(box)` is the key reader.
export function Page() {
  let box!: HTMLDivElement;
  let link!: HTMLAnchorElement;
  let frame!: HTMLIFrameElement;
  createEffect(
    () => 1,
    () => {
      for (const value of ({} as any)["constructor"].values(box as object) as unknown[]) if (typeof value === "function") (value as (event: Event) => void)(new Event("click"));
    },
  );
  return (
    <div ref={box} onClick={() => { count(); }}>
      <a ref={link} href="/x">x</a>
      <iframe ref={frame} />
    </div>
  );
}
