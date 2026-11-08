// `early` runs before the alias's declarator, so its call throws
// `ReferenceError` instead of calling `refresh`: it names no primitive.
import { createMemo, createRenderEffect, refresh } from "solid-js";

function early(target: () => number) {
  refreshLate(target);
}
try {
  early(() => 1);
} catch {}
const refreshLate = refresh;

export function Late() {
  const m1 = createMemo(() => 1);
  createRenderEffect(() => 1, () => {
    refreshLate(m1);
  });
  return <div />;
}
