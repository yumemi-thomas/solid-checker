import { createContext, createStore, useContext, type Store } from "solid-js";

// ADR 1147, arm 3 (`context-provided-store-tuple`): a context whose one value is a fresh `[store, { actions }]` literal. Each reader's slot-0 name is
// the store itself, and each action's local is only called, so a store read in a reader has its store rows and a write in an action is placed by the
// readers' calls. Any other form keeps its rows (SC9012 at the value and the reads, SC2001 at the write).

type Cart = { items: string[] };

// Negative (ADR 1137's veto): the positive's shape, in a program that builds code from a string, which may replace what no text shows.
eval("0");
const CartContext = createContext<readonly [Store<Cart>, { add: (item: string) => void }]>();
function CartProvider(props: { children?: JSX.Element }) {
  const [cart, setCart] = createStore<Cart>({ items: [] });
  const add = (item: string) =>
    setCart((draft) => {
      draft.items.push(item);
    });
  return <CartContext value={[cart, { add }] as const}>{props.children}</CartContext>;
}
function AddButton() {
  const [, { add }] = useContext(CartContext);
  return <button onClick={() => add("apple")}>add</button>;
}
function Count() {
  const [cart] = useContext(CartContext);
  return <span>{cart.items.length}</span>;
}
export function Shop() {
  return (
    <CartProvider>
      <AddButton />
      <Count />
    </CartProvider>
  );
}

