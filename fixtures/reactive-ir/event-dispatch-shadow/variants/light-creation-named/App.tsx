import { createEffect, createRenderEffect } from "solid-js";
// ADR 1139 r1, the light twin of shadow-creation-named: a light-tree delivery sets window.event, so the creation reaches the owner check
// (Chrome rc.21: NO_OWNER_EFFECT) and the SC4001 violation stands.
export function S() {
  const handle = () => { createRenderEffect(() => 1, () => {}); };
  const box = (<div onScroll={handle} />) as HTMLDivElement;
  document.body.appendChild(box);
  createEffect(() => 1, () => { box.dispatchEvent(new Event("scroll")); });
  return <div />;
}
