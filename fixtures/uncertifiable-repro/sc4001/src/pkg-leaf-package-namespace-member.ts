// app-game typegpu-examples 2-webgpu-inter-stage-variables/index.tsx:137: a tracked effect calls a function
// through a namespace import of a package (`d.vec4f(...)`, `import * as d from "typegpu/data"`).
import * as utils from "@solid-primitives/utils";
import { createSignal, createTrackedEffect } from "solid-js";

export function createWidth(container: utils.MaybeAccessor<HTMLElement | undefined>) {
  const [width, setWidth] = createSignal(0);
  createTrackedEffect(() => {
    const element = utils.access(container);
    if (element) setWidth(element.clientWidth);
  });
  return width;
}
