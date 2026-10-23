import { createSignal } from "solid-js";

const [count] = createSignal(0);

// Uncertifiable: called plainly in tracked JSX and rendered through an alias
// (`<Rendered />` runs it as a component: an untracked body read). A method's
// references are no census of its entries, so the one call it can see proves
// nothing (ADR 0372: the `aliased.badge` reference stays apart from the
// method's declaration).
const aliased = {
  badge() {
    const value = count();
    return <b>{value}</b>;
  },
};

const Rendered = aliased.badge;

// Uncertifiable: the holder leaves the module, so code not seen here may
// render `leaked.item` as a component.
const leaked = {
  item() {
    const value = count();
    return <b>{value}</b>;
  },
};

export function Panel() {
  return (
    <p>
      {aliased.badge()}
      <Rendered />
      {leaked.item()}
    </p>
  );
}

export function leak() {
  return leaked;
}
