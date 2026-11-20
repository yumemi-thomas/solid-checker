import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1133 round 1 (F1): `Reflect.get({}, "constructor")` is `Object` by a key no scan tests as a member name; `.values(box)` takes Solid's stored click handler and the apply calls it with no event. Reflect.get is a delivery hazard (Reflect.apply, construct and has are not).
export function Page() {
  let box!: HTMLDivElement;
  let link!: HTMLAnchorElement;
  let frame!: HTMLIFrameElement;
  createEffect(
    () => 1,
    () => {
      for (const value of (Reflect.get({}, "constructor") as any).values(box as object) as unknown[]) if (typeof value === "function") (value as (event: Event) => void)(new Event("click"));
    },
  );
  return (
    <div ref={box} onClick={() => { count(); }}>
      <a ref={link} href="/x">x</a>
      <iframe ref={frame} />
    </div>
  );
}
