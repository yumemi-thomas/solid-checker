import { onCleanup, onSettled } from "solid-js";

// SC9012: inside a method, a reassigned `let` is not its initializer.
class Stepper {
  step() {
    let next = function () {};
    next = () => onCleanup(() => {});
    next();
  }
}
export function NestedReassigned() {
  const stepper = new Stepper();
  onSettled(() => {
    stepper.step();
  });
  return <div />;
}

// SC9012: a destructuring pattern writes `turn`.
class Turner {
  turn() {}
}
export function DestructuredMemberWrite() {
  const turner = new Turner();
  [turner.turn] = [() => onCleanup(() => {})];
  onSettled(() => {
    turner.turn();
  });
  return <div />;
}

// SC9012: a loop head writes `spin`.
class Spinner {
  spin() {}
}
export function LoopHeadMemberWrite() {
  const spinner = new Spinner();
  for (spinner.spin of [() => onCleanup(() => {})]) {
    break;
  }
  onSettled(() => {
    spinner.spin();
  });
  return <div />;
}

// SC9012: a default nested in a destructured parameter runs on entry, for a
// generator too.
class Unpacker {
  unpack({ value = onCleanup(() => {}) }: { value?: unknown }) {
    return value;
  }
  *stream({ value = onCleanup(() => {}) }: { value?: unknown }) {
    yield value;
  }
}
export function PatternDefault() {
  const unpacker = new Unpacker();
  onSettled(() => {
    unpacker.unpack({});
  });
  return <div />;
}
export function GeneratorPatternDefault() {
  const unpacker = new Unpacker();
  onSettled(() => {
    unpacker.stream({});
  });
  return <div />;
}

// SC9012: the getter runs when `Object.values` reads the bound literal.
const record = {
  get value() {
    onCleanup(() => {});
    return 1;
  },
};
export function BoundGetter() {
  onSettled(() => {
    Object.values(record);
  });
  return <div />;
}
