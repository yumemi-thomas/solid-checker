import { createEffect, createMemo, onCleanup } from "solid-js";
import * as S from "solid-js";

// ADR 1060: `solid-js` here is whatever the tsconfig `paths` mapping lands on (see README.md).
const { createMemo: memoSlot } = S;

onCleanup(() => {});

export function Direct() {
  createMemo(async () => 1, { sync: true });
  return null;
}
export function Namespace() {
  S.createMemo(async () => 2, { sync: true });
  return null;
}
export function Slot() {
  memoSlot(async () => 3, { sync: true });
  return null;
}
export function MissingEffect() {
  createEffect(() => 1, undefined as unknown as () => void);
  return null;
}
