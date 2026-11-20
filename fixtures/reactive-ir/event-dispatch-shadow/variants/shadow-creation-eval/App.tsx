import { createEffect, createRenderEffect } from "solid-js";

// ADR 1139 (the narrowing's fail-closed half): the shadow root is attached by code built from a string, which no scan reads, and an effect's
// apply dispatches to the element: the creation in the named handler may throw PRIMITIVE_IN_EFFECT_CALLBACK first, so it stays uncertifiable.
export function S() {
  const handle = () => { createRenderEffect(() => 1, () => {}); };
  const box = (<div onScroll={handle} />) as HTMLDivElement;
  eval('document.createElement("div").attachShadow({ mode: "open" }).appendChild(box)');
  createEffect(() => 1, () => { box.dispatchEvent(new Event("scroll")); });
  return <div />;
}
