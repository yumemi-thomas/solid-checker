import { createMemo, createSignal } from "solid-js";

export function C1() {
  const a1 = createMemo(async () => 1);
  return (
    <div>
      <p data-view="v1">{(() => {
        {
          const hf = (_s: TemplateStringsArray) => {
            a1();
          };
          hf`x`;
        }
        return 1;
      })()}</p>
    </div>
  );
}

export { C1 as App };
