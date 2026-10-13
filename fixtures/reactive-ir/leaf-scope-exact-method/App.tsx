import { createMemo, createSignal, onCleanup, onSettled } from "solid-js";

class Camera {
  aspect = 1;
  perspective(options: { aspect: number }) {
    this.aspect = options.aspect;
    this.update();
  }
  update() {
    this.aspect = Math.max(this.aspect, 0);
  }
}

class Leaky {
  start() {
    onCleanup(() => {});
  }
}

class Base {
  run() {}
}
class Derived extends Base {
  run() {
    createMemo(() => 1);
  }
}

// Clean: `camera` is exactly a `Camera`, so `perspective` and the `update` it
// calls through `this` are the methods that run, and neither creates a
// primitive or registers a cleanup in the leaf scope.
export function ExactInstance() {
  const camera = new Camera();
  const [size] = createSignal(1);
  onSettled(() => {
    camera.perspective({ aspect: size() });
  });
  return <div />;
}

// Violation: the exact instance's method registers a cleanup in the leaf
// scope, which the runtime throws on.
export function ExactInstanceForbidden() {
  const leaky = new Leaky();
  onSettled(() => {
    leaky.start();
  });
  return <div />;
}

// Uncertifiable: a caller-supplied camera may be another object.
export function SuppliedInstance(props: { camera: Camera }) {
  onSettled(() => {
    props.camera.perspective({ aspect: 1 });
  });
  return <div />;
}

// Uncertifiable: the annotation names `Base.run`, while `Derived.run` runs.
export function WidenedInstance() {
  const instance: Base = new Derived();
  onSettled(() => {
    instance.run();
  });
  return <div />;
}

// Uncertifiable: a `let` may hold another object when the callback runs.
export function ReassignableInstance() {
  let camera = new Camera();
  onSettled(() => {
    camera.perspective({ aspect: 1 });
  });
  camera = new Camera();
  return <div />;
}
