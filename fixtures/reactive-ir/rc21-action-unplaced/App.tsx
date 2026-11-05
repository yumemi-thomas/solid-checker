import { action } from "solid-js";

const go = action(function* () {});

// Violation: a component body is a children-capable owner.
export function Direct() {
  go();
  return <i />;
}

// Obligations: no call edge reaches the function holding the call, yet a
// render runs it (Chrome, rc.21: ACTION_CALLED_IN_OWNED_SCOPE each).
function viaGetter() {
  go();
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

const tag = (_: TemplateStringsArray) => {
  go();
  return 1;
};
export function Tagged() {
  tag`x`;
  return <i />;
}

const slots = new Map([["g", () => go()]]);
export function MapSlot() {
  slots.get("g")!();
  return <i />;
}

class Constructed {
  constructor() {
    go();
  }
}
class FieldInit {
  x = go();
}
export function Instances() {
  new Constructed();
  new FieldInit();
  return <i />;
}

// Silent: module initialization has no owner (a static block, a static field).
class Boot {
  static {
    go();
  }
  static y = go();
}
export const booted = Boot;
