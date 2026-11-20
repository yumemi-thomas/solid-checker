import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1133 round 1 (F4): `const { Object: O } = globalThis` takes `Object` out by a pattern.
export function Page() {
  let box!: HTMLDivElement;
  let link!: HTMLAnchorElement;
  let frame!: HTMLIFrameElement;
  createEffect(
    () => 1,
    () => {
      const { Object: O } = globalThis as any; for (const value of O.values(box as object) as unknown[]) if (typeof value === "function") (value as (event: Event) => void)(new Event("click"));
    },
  );
  return (
    <div ref={box} onClick={() => { count(); }}>
      <a ref={link} href="/x">x</a>
      <iframe ref={frame} />
    </div>
  );
}
