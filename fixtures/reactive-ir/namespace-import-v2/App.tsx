import * as Solid from "solid-js";

// The namespace spelling of the vocabulary: every probe here reaches a
// modelled primitive only through `Solid.*`, and `direct.tsx` is the same
// code through named imports. The two files must always report the same
// findings — a name resolvable as `import { x }` but not as `Solid.x` moves
// this snapshot.

const [items] = Solid.createSignal(["a"]);

// Control: `createEffect` predates the census-derived namespace list. This
// module-scope effect is unowned, so missing-owner   firing here proves the
// namespace resolution path itself works.
Solid.createEffect(
  () => items(),
  () => {},
);

// `children` joined the namespace list with the census widening: it
// registers cleanup, so creating it inside an owner-backed `onSettled` (the
// component body proves the owner) is leaf-owner-forbidden-call — through
// either import style. The JSX return is what proves `Leaf` a component.
export function Leaf() {
  Solid.onSettled(() => {
    Solid.children(() => null);
  });
  return <div />;
}

// JSX member tags are resolved against the namespace vocabulary, so these
// callbacks exercise the same control-flow and children-parameter paths as
// the named-import twin in `direct.tsx`.
export function Rows() {
  return (
    <Solid.For each={items()}>
      {(item, index) => {
        const current = index();
        return <div>{current}</div>;
      }}
    </Solid.For>
  );
}

export function Cells(props: { total: number }) {
  return (
    <Solid.Repeat count={props.total}>{(index) => <div>{index}</div>}</Solid.Repeat>
  );
}

export function Conditional() {
  return <Solid.Show when={items()}>{() => <div>{items()}</div>}</Solid.Show>;
}
