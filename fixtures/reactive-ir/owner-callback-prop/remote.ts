// A child component in another file: the callback-prop owner edge is same-file only (ADR 0562).
export function Remote(props: { run: () => void }) {
  props.run();
  return null;
}
