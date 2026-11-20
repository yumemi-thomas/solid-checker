import { createEffect, createRenderEffect } from "solid-js";

// ADR 1139 r1 (Opus review F1, F2): an effect's apply dispatches into a shadow tree, whose listener runs with window.event unset, inside the
// apply's effect-callback window: a creation in the handler throws PRIMITIVE_IN_EFFECT_CALLBACK, whether the handler is written inline, named
// by its value, named by a member, or handed by a literal spread. The light twin reaches the owner check: NO_OWNER_EFFECT.
function deliver(box: HTMLElement, shadow: boolean) {
  if (shadow) document.createElement("div").attachShadow({ mode: "open" }).appendChild(box);
  else document.body.appendChild(box);
  createEffect(() => 1, () => { box.dispatchEvent(new Event("scroll")); });
}

export function Named() {
  const handle = () => { createRenderEffect(() => 1, () => {}); };
  deliver((<div onScroll={handle} />) as HTMLDivElement, true);
  return <div />;
}

export function Member() {
  const obj = { m: () => { createRenderEffect(() => 1, () => {}); } };
  deliver((<div onScroll={obj.m} />) as HTMLDivElement, true);
  return <div />;
}

export function Spread() {
  deliver((<div {...{ onScroll: () => { createRenderEffect(() => 1, () => {}); } }} />) as HTMLDivElement, true);
  return <div />;
}

export function LightNamed() {
  const handle = () => { createRenderEffect(() => 1, () => {}); };
  deliver((<div onScroll={handle} />) as HTMLDivElement, false);
  return <div />;
}
