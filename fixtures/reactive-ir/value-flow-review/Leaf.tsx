import { onCleanup, onSettled } from "solid-js";

// SC9012: a `const` key names `run`, written through the prototype.
class Patched {
  run() {}
}
const key = "run";
Patched.prototype[key] = () => {
  onCleanup(() => {});
};
export function PrototypeKey() {
  const patched = new Patched();
  onSettled(() => {
    patched.run();
  });
  return <div />;
}

// SC9012: the constructor hands back another object.
class Swapped {
  constructor() {
    return { swap: () => onCleanup(() => {}) };
  }
  swap() {}
}
export function ConstructorReturn() {
  const swapped = new Swapped();
  onSettled(() => {
    swapped.swap();
  });
  return <div />;
}

// Clean: calling a generator runs none of its body.
class Lazy {
  *produce() {
    onCleanup(() => {});
  }
}
export function GeneratorMethod() {
  const lazy = new Lazy();
  onSettled(() => {
    lazy.produce();
  });
  return <div />;
}

// SC9012: the method's default runs when the call omits the argument.
class Defaulted {
  prepare(_value = onCleanup(() => {})) {}
}
export function DefaultParameter() {
  const defaulted = new Defaulted();
  onSettled(() => {
    defaulted.prepare();
  });
  return <div />;
}

// SC9012: a `PromiseLike`'s `then` is user code, handed nothing or not.
function pending(): PromiseLike<number> {
  return Promise.resolve(1);
}
export function ThenableWithoutCallback() {
  onSettled(() => {
    pending().then();
  });
  return <div />;
}

// SC9012: `let` names another function by the time it is called.
export function ReassignedHelper() {
  let step = () => {};
  step = () => onCleanup(() => {});
  onSettled(() => {
    step();
  });
  return <div />;
}

// SC9012: an object literal's getter runs when `Object.values` reads it.
export function GetterArgument() {
  onSettled(() => {
    Object.values({
      get value() {
        onCleanup(() => {});
        return 1;
      },
    });
  });
  return <div />;
}
