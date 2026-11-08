// Review D36: a binding that holds a primitive exactly names it, so the call
// is that primitive's for every rule. Every other shape stays unresolved: an
// SC9012 obligation, never a violation.
import * as S from "solid-js";
import { createMemo, createRenderEffect, createEffect, createRoot, flush, refresh } from "solid-js";

// Exact: the namespace's member, a default that is the same member, a `const`
// of the named import, a closed holder's member.
const refreshMember = S.refresh;
const { refresh: refreshDefault = S.refresh } = S;
const refreshAlias = refresh;
const refreshBox = { m: refresh };
const refreshBoxed = refreshBox.m;
const flushBox = { m: flush };
const flushBoxed = flushBox.m;

// Not exact: a written `let`, a default naming another value, a holder that
// escapes or is written, an object that is no namespace, an exported alias.
const other = ((target: () => unknown) => Promise.resolve(target())) as typeof S.refresh;
let refreshLet = S.refresh;
refreshLet = other;
const { refresh: refreshOther = other } = S;
const escapingBox = { m: refresh };
const refreshEscaped = escapingBox.m;
(globalThis as { held?: unknown }).held = escapingBox;
const writtenBox = { m: refresh };
const refreshWrittenBox = writtenBox.m;
writtenBox.m = other;
const local = { refresh: other };
const refreshLocal = local.refresh;
export const refreshExported = refresh;

export function Exact() {
  const m1 = createMemo(() => 1);
  createRenderEffect(() => 1, () => {
    refreshMember(m1);
    refreshDefault(m1);
    refreshAlias(m1);
    refreshBoxed(m1);
  });
  createEffect(() => 1, () => {
    flushBoxed();
  });
  return <div />;
}

// The same calls in a root, whose body runs on its call: the render effect's
// first apply runs on the call (ADR 0503) under the root, an owned scope.
createRoot(() => {
  const m1 = createMemo(() => 1);
  createRenderEffect(() => 1, () => {
    refreshMember(m1);
    refreshDefault(m1);
    refreshAlias(m1);
    refreshBoxed(m1);
    refresh(m1);
  });
});
createRoot(() => {
  const m1 = createMemo(() => 1);
  createRenderEffect(() => 1, () => {
    refreshLet(m1);
    refreshEscaped(m1);
  });
});

// The direct calls in the same places: each exact alias answers as these do.
export function Direct() {
  const m1 = createMemo(() => 1);
  createRenderEffect(() => 1, () => {
    refresh(m1);
  });
  createEffect(() => 1, () => {
    flush();
  });
  return <div />;
}

export function NotExact() {
  const m1 = createMemo(() => 1);
  createRenderEffect(() => 1, () => {
    refreshLet(m1);
    refreshOther(m1);
    refreshEscaped(m1);
    refreshWrittenBox(m1);
    refreshLocal(m1);
    refreshExported(m1);
  });
  return <div />;
}
