// app-game packages/typegpu-examples/1.1-webgpu-fundamentals/index.tsx:83 (and 10
// more examples): an effect's callback calls methods of a value that
// a project hook returns, typed by `@webgpu/types`, a package that ships
// declarations and no code, so there is no body to read behind them.
/// <reference types="@webgpu/types" />
import { createTrackedEffect } from "solid-js";

function useCanvas() {
  return { context: document.createElement("canvas").getContext("webgpu")! };
}

export function Frame() {
  const { context } = useCanvas();
  createTrackedEffect(() => {
    void context.getCurrentTexture().createView();
  });
  return null;
}
