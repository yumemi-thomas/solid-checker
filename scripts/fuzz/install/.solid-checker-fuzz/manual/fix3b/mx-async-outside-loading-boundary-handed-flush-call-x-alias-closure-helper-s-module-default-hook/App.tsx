import { createSignal, flush } from "solid-js";
import useVals from "./helpers";

function h1(cb: () => unknown) {
  cb();
  return 1;
}

export function C1() {
  const { a1 } = useVals();
  const useIt = a1;
  const K1 = () => {
    flush(() => {
      h1(() => {
        const f1 = () => {
          useIt();
        };
        f1();
      });
    });
    return <span />;
  };
  return (
    <div>
      <p data-view="v1">{(() => {
        K1.call(null);
        return 1;
      })()}</p>
    </div>
  );
}
