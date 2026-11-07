import * as S from "solid-js";
import { flush, onCleanup, onSettled } from "solid-js";

const box = { m: onCleanup };
const held = box.m;
const { onSettled: settled = S.onSettled } = S;
const later = flush.bind(null);

// Each call resolves to a Solid primitive through a name no rule reads.
export function Aliases() {
  setTimeout(() => {
    held(() => {});
    settled(() => {});
    later();
  }, 0);
  return <i />;
}

// Direct calls need no such row.
export function Direct() {
  onCleanup(() => {});
  onSettled(() => {});
  return <i />;
}
