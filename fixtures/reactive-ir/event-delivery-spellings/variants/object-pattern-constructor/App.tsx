import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1133 round 1 (F4): `const { constructor: C } = {}` takes `Object` out of a plain object by a pattern.
export function Page() {
  let box!: HTMLDivElement;
  let link!: HTMLAnchorElement;
  let frame!: HTMLIFrameElement;
  createEffect(
    () => 1,
    () => {
      const { constructor: C } = {} as any; for (const value of C.values(box as object) as unknown[]) if (typeof value === "function") (value as (event: Event) => void)(new Event("click"));
    },
  );
  return (
    <div ref={box} onClick={() => { count(); }}>
      <a ref={link} href="/x">x</a>
      <iframe ref={frame} />
    </div>
  );
}
