import { createSignal, onCleanup } from "solid-js";
const [, set] = createSignal(0);

// Open-world steps: neither absence of calls nor absence of local steps is
// evidence that the body never runs. Both operations need named obligations.
export function* exportedGenerator() {
  yield 1;
  set(1);
  onCleanup(() => {});
}
function* heldGenerator() {
  yield 1;
  set(2);
  onCleanup(() => {});
}
export const exportedIterator = heldGenerator();

function* listedGenerator() {
  yield 1;
  set(3);
  onCleanup(() => {});
}
export { listedGenerator as renamedGenerator };

// A same-named local declaration is not the exported binding above.
export function localShadow() {
  function* listedGenerator() {
    yield 1;
    set(4);
    onCleanup(() => {});
  }
  void listedGenerator;
}

// Negative: an unexported, unreferenced generator is never stepped locally.
function* neverStepped() {
  yield 1;
  set(5);
  onCleanup(() => {});
}
