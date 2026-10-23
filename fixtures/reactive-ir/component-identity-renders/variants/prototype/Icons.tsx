import { createSignal } from "solid-js";
import { Dynamic } from "@solidjs/web";

const [count] = createSignal(0);

// Uncertifiable: an accessor installed on `Object.prototype` runs on a member
// read that falls through to it (`icons[name]` with a key the literal lacks),
// with the map as `this`, and can call a stored value plainly.
Object.defineProperty(Object.prototype, "missing", {
  get(this: { archive?: () => unknown }) {
    return this.archive?.();
  },
});

function Archive() {
  const value = count();
  return <b>{value}</b>;
}

const icons: Record<string, () => unknown> = { archive: Archive };

export function IconMap(props: { name: string }) {
  return (
    <div>
      <Archive />
      <Dynamic component={icons[props.name]} />
    </div>
  );
}
