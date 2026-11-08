import { createSignal } from "solid-js";

// ADR 0840: the program's own `map` (a class element) is a census site, so a
// standard `map` on a receiver no fact proves real is withheld: its prefer-for
// stays uncertifiable beside the impostor row. The class's own `map` is no
// `Array#map`, so it gets no prefer-for row at all.
class ReactiveCollection {
  map(callback: (item: string) => string) {
    return [callback("custom")];
  }
}

export function App() {
  const [items] = createSignal(["a", "b"]);
  const [customCollection] = createSignal(new ReactiveCollection());
  return (
    <main>
      <ul>{items().map((item) => item)}</ul>
      <ul>{customCollection().map((item) => item)}</ul>
    </main>
  );
}
