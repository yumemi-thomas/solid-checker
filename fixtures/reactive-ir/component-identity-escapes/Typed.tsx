import { createSignal, Show, type Component } from "solid-js";

const [count] = createSignal(0);
const [items] = createSignal(["a", "b"]);

// Uncertifiable: an exact `Component` type says nothing about how the function
// is invoked. A tag renders it, and `map` still calls it plainly (`tsc`
// accepts that), where no strict-read window opens.
const Typed: Component<{}> = () => {
  const prefix = count();
  return <li>{prefix}</li>;
};

// Positive: the same type with a tag and no other reference.
const TypedOnly: Component<{}> = () => {
  const prefix = count();
  return <li>{prefix}</li>;
};

function badge(value: number) {
  return <b>{value}</b>;
}

// Positive: no JSX return (a helper's element), typed, rendered by a tag only.
const TypedHelper: Component<{}> = () => {
  const value = count();
  return badge(value);
};

// Uncertifiable: the same shape handed to `map` as well.
const TypedHelperMapped: Component<{}> = () => {
  const value = count();
  return badge(value);
};

// Positive: typed, no JSX return and no tag; the type and the export prove it.
export const TypedBare: Component<{}> = () => {
  const value = count();
  return badge(value);
};

// Uncertifiable: typed, no JSX return and no tag, but handed to `map`.
const TypedBareMapped: Component<{}> = () => {
  const value = count();
  return badge(value);
};

export function TypedPage() {
  return (
    <ul>
      <Show when={false}>
        <Typed />
      </Show>
      {items().map(Typed)}
      <TypedOnly />
      <TypedHelper />
      <TypedHelperMapped />
      {items().map(TypedHelperMapped)}
      {items().map(TypedBareMapped)}
    </ul>
  );
}
