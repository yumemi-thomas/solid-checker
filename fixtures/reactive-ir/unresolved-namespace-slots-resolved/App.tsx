import { createEffect, createMemo, onCleanup } from "solid-js";
import * as S from "solid-js";

// ADR 1054: each form copies a Solid export into a binding of the program's own. Where `solid-js` resolves, each names the export
// exactly (ADR 0459, review D36) and its claim is proved; where it does not, the copy names nothing: SC9012 import-unresolved.
const { createMemo: memoSlot, createEffect: effectSlot, onCleanup: cleanupSlot } = S;
const memoMember = S.createMemo;
const memoAlias = createMemo;
const holder = { memo: createMemo };
const memoHeld = holder.memo;

// Module scope, no owner: SC4001 through a slot.
cleanupSlot(() => {});

// SC7002 (a `sync: true` memo whose compute returns a Promise), one per form.
export function Slot() {
  memoSlot(async () => 1, { sync: true });
  return null;
}
export function Member() {
  memoMember(async () => 2, { sync: true });
  return null;
}
export function Alias() {
  memoAlias(async () => 3, { sync: true });
  return null;
}
export function Held() {
  memoHeld(async () => 4, { sync: true });
  return null;
}

// SC7001 through a slot: an effect with no effect function.
export function SlotEffect() {
  effectSlot(() => 1, undefined as unknown as () => void);
  return null;
}

// Controls: the import itself and the namespace member call (unresolved: SC9012 only, as before ADR 1054).
export function Direct() {
  createMemo(async () => 5, { sync: true });
  return null;
}
export function Namespace() {
  S.createMemo(async () => 6, { sync: true });
  return null;
}

// Shadowed: a parameter named like the namespace, destructured: not the import in either project.
export function Shadowed(props: { S?: { createMemo: (fn: () => Promise<number>, options: { sync: boolean }) => void } }) {
  const S = props.S ?? { createMemo: () => {} };
  const { createMemo: localMemo } = S;
  localMemo(async () => 7, { sync: true });
  return null;
}

// Keep the direct imports used where the project resolves.
void createEffect;
void onCleanup;
