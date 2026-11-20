import { createEffect, createRenderEffect } from "solid-js";
// ADR 1139 r1 (Opus review, F1): a handler named by its value (`onScroll={handle}`), in a shadow tree, delivered by `dispatchEvent` from an effect's apply: the creation
// throws PRIMITIVE_IN_EFFECT_CALLBACK first (Chrome rc.21), so no missing-owner violation stands there (creation-throws-first).
export function S() {
  const handle = () => { createRenderEffect(() => 1, () => {}); };
  const box = (<div onScroll={handle} />) as HTMLDivElement;
  document.createElement("div").attachShadow({ mode: "open" }).appendChild(box);
  createEffect(() => 1, () => { box.dispatchEvent(new Event("scroll")); });
  return <div />;
}
