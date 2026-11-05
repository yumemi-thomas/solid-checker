import { createMemo, createSignal, refresh } from "solid-js";

const [count, setCount] = createSignal(0);
const [, setOwned] = createSignal(0, { ownedWrite: true });
const total = createMemo(() => count());

// Violation: a component body is an owned scope.
export function Direct() {
  setCount(1);
  return <i />;
}

// Obligations: no call edge reaches the function holding the write, yet a
// render runs it (Chrome, rc.21: REACTIVE_WRITE_IN_OWNED_SCOPE each).
function viaGetter() {
  setCount(2);
}
const holder = {
  get value() {
    viaGetter();
    return 1;
  },
};
export function Getter() {
  holder.value;
  return <i />;
}

const api = {
  go() {
    setCount(3);
  },
};
export function Method() {
  api.go();
  return <i />;
}

const slots = new Map([["g", () => setCount(4)]]);
export function MapSlot() {
  slots.get("g")!();
  return <i />;
}

class Constructed {
  constructor() {
    setCount(5);
  }
}
class FieldInit {
  x = setCount(6);
}
export function Instances() {
  new Constructed();
  new FieldInit();
  return <i />;
}

const reloader = {
  go() {
    refresh(total);
  },
};
export function Reload() {
  const m = createMemo(() => {
    reloader.go();
    return 1;
  });
  return <i>{m()}</i>;
}

// Violation: the write sits directly in a tag the component body runs.
const tag = (_: TemplateStringsArray) => {
  setCount(7);
  return 1;
};
export function Tagged() {
  tag`x`;
  return <i />;
}

// Silent: module initialization has no owner (top level, a static block, a
// static field); `ownedWrite` is legal in an owned scope; a class nothing
// names never runs.
setCount(8);
class Boot {
  static {
    setCount(9);
  }
  static y = setCount(10);
}
export const booted = Boot;
const owned = {
  go() {
    setOwned(1);
  },
};
export function Owned() {
  owned.go();
  return <i />;
}
class Unused {
  run() {
    setCount(11);
  }
}
