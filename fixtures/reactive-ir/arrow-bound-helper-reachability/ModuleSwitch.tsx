// Negative: a module-level declaration inside a switch case runs from a jump
// over `helper`'s declarator.
import { createRoot, createSignal } from "solid-js";

const [count, setCount] = createSignal(0);
const mode = Number(globalThis.location?.hash.slice(1) ?? 1);

// mode 1 jumps over `helper`'s declarator to `go()`: the root throws
// ReferenceError and setCount never runs. Only mode 1 calls go.
switch (mode) {
  case 0:
    const helper = () => {
      setCount(1);
    };
    break;
  case 1:
    go();
    break;
  default:
    function go() {
      createRoot(() => {
        helper();
      });
    }
}

export function Show() {
  return <button>{count()}</button>;
}
