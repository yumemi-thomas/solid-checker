import { createReaction, createRenderEffect } from "solid-js";

// Positive: createRenderEffect hands its second argument to the node as is;
// a non-callable one fails at the first run (`node._effectFn is not a
// function`), measured in Chrome (flip witnesses f01, f02).
export function RenderUndefined() {
  createRenderEffect(() => 1, undefined as any);
  return <i />;
}
export function RenderNumber() {
  createRenderEffect(() => 1, 3 as any);
  return <i />;
}
// Positive: no bundle is read there, so an `{ effect }` literal is the value
// called.
export function RenderBundle() {
  createRenderEffect(() => 1, { effect: () => {} } as any);
  return <i />;
}
// Uncertifiable: createReaction reads its effect function only when the
// tracker it returns runs (flip witness f03); the call alone proves nothing.
export function ReactionUndefined() {
  const track = createReaction(undefined as any);
  track(() => 1);
  return <i />;
}
// Negative: a callable effect function, and a reaction's bundle.
export function RenderCallable() {
  createRenderEffect(() => 1, () => {});
  return <i />;
}
export function ReactionCallable() {
  createReaction(() => {});
  createReaction({ effect: () => {}, error: () => {} });
  return <i />;
}
// Shadowed: a local function of the same name is not Solid's.
export function Shadowed() {
  const createRenderEffect = (_compute: () => number, _apply: unknown) => {};
  createRenderEffect(() => 1, undefined);
  return <i />;
}
