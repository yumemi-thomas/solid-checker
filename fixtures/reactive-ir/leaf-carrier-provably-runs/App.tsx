import { onCleanup, onSettled } from "solid-js";

// ADR 0563: SC3001 (a leaf owner's forbidden call) is proven through a standard-library carrier only
// where it provably runs the callback.

// Violation (SC3001): `[1].forEach` runs the callback inside the leaf owner.
export function LeafNonEmpty() {
  onSettled(() => {
    [1].forEach(() => onCleanup(() => {}));
  });
  return <p />;
}

// Uncertifiable (SC3001): an empty or unknown-length receiver may run it zero times.
export function LeafEmpty() {
  onSettled(() => {
    ([] as number[]).forEach(() => onCleanup(() => {}));
  });
  return <p />;
}

export function LeafPropsReceiver(props: { xs: number[] }) {
  onSettled(() => {
    props.xs.forEach(() => onCleanup(() => {}));
  });
  return <p />;
}
