import { createEffect, createRenderEffect } from "solid-js";
import { fire, mount } from "shadow-host";

// ADR 1139 (the narrowing's fail-closed half): a stand-in package the build does not read attaches the shadow root and dispatches, called
// from an effect's apply: the creation in the named handler may throw PRIMITIVE_IN_EFFECT_CALLBACK first, so it stays uncertifiable.
export function S() {
  const handle = () => { createRenderEffect(() => 1, () => {}); };
  const box = (<div onScroll={handle} />) as HTMLDivElement;
  mount(box);
  createEffect(() => 1, () => { fire(box); });
  return <div />;
}
