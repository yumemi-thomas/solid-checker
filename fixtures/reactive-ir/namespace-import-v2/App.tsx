import * as Solid from "solid-js";

// The namespace spelling of the vocabulary: every probe here reaches a
// modelled primitive only through `Solid.*`, and `direct.tsx` is the same
// code through named imports. The two files must always report the same
// findings — a name resolvable as `import { x }` but not as `Solid.x` moves
// this snapshot.

const [items] = Solid.createSignal(["a"]);

// Control: `createEffect` predates the census-derived namespace list. This
// module-scope effect is unowned, so no-owner-effect firing here proves the
// namespace resolution path itself works.
Solid.createEffect(
  () => items(),
  () => {},
);

// `children` joined the namespace list with the census widening: it
// registers cleanup, so creating it inside the leaf owner `onSettled` is
// primitive-in-leaf-owner — through either import style.
export function Leaf() {
  Solid.onSettled(() => {
    Solid.children(() => null);
  });
  return null;
}

// Pinned limitation, not an assertion of correctness: JSX member tags are
// not resolved against the namespace vocabulary, so `<Solid.For>` and
// `<Solid.Repeat>` produce no control-flow classification today — silently,
// on both import styles (a direct `<For>` in `direct.tsx` is recognised).
// If member-tag resolution lands, this snapshot is where it shows up.
export function Rows() {
  return <Solid.For each={items()}>{(item) => <div>{item}</div>}</Solid.For>;
}

export function Cells(props: { total: number }) {
  return (
    <Solid.Repeat count={props.total}>{(index) => <div>{index}</div>}</Solid.Repeat>
  );
}
