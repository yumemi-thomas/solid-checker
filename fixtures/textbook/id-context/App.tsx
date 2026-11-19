import { createContext, createStore, useContext, type Store } from "solid-js";
import type { JSX } from "@solidjs/web";

type Cart = { items: string[] };
type CartContextValue = readonly [Store<Cart>, { add: (item: string) => void }];
const CartContext = createContext<CartContextValue>();

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
  return (
    <button type="button" onClick={() => add("apple")}>
      add
    </button>
  );
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
