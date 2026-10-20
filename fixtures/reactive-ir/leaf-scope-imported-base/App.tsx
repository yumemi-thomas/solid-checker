import { createSignal, onCleanup, onSettled } from "solid-js";
import { Shadowing, Transform } from "./base";

class Camera extends Transform {
  aspect = 1;
  perspective(options: { aspect: number }) {
    this.aspect = options.aspect;
  }
}

class Leaky extends Transform {
  start() {
    onCleanup(() => {});
  }
}

class Panel extends Shadowing {
  open(): number {
    return 1;
  }
}

// Clean: `camera` is exactly a `Camera`, whose imported base `Transform` is a
// project class this resolves; `perspective` creates nothing in the leaf
// scope.
export function ImportedBase() {
  const camera = new Camera();
  const [size] = createSignal(1);
  onSettled(() => {
    camera.perspective({ aspect: size() });
  });
  return <div />;
}

// Violation: the exact instance's own method registers a cleanup in the leaf
// scope, which the runtime throws on.
export function ImportedBaseForbidden() {
  const leaky = new Leaky();
  onSettled(() => {
    leaky.start();
  });
  return <div />;
}

// Uncertifiable: the imported base declares a member under a computed key,
// which may be `open`; the chain is not exact.
export function ImportedBaseShadowed() {
  const panel = new Panel();
  onSettled(() => {
    panel.open();
  });
  return <div />;
}
