// app-game ogl-examples draw-modes/draw-modes.tsx:23 (`camera.perspective(..)`, ogl/core/camera.ts:86): a leaf
// callback calls a method of an exact instance, and that method calls a method of one of its fields
// (`this.projectionMatrix.fromPerspective(..)`), a receiver the walk does not enter.
import { createSignal, createTrackedEffect } from "solid-js";

class Matrix {
  fromPerspective(aspect: number) {
    return aspect;
  }
}

class Camera {
  projectionMatrix = new Matrix();
  perspective(aspect: number) {
    this.projectionMatrix.fromPerspective(aspect);
  }
}

export function View() {
  const camera = new Camera();
  const [aspect, setAspect] = createSignal(1);
  createTrackedEffect(() => {
    camera.perspective(aspect());
  });
  return <canvas onClick={() => setAspect(2)} />;
}
