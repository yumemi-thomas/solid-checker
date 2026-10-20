import { createSignal } from "solid-js";
import { Base } from "./Base";

const [count] = createSignal(0);

function List() {
  const value = count();
  return <b>{value}</b>;
}

// Uncertifiable: the target is an imported class whose static accessors this
// file cannot see, so the member may be received and called plainly.
export const Tabs = Object.assign(Base, { List });

export function Screen() {
  return (
    <div>
      <Tabs.List />
    </div>
  );
}
