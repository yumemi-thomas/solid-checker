import { createContext, useContext } from "solid-js";

// ADR 0577: the same function runs in the body (owned) and from a handler (no owner).
const NoDefault = createContext<number>();
const WithDefault = createContext<number>(1);

// SC4002 violation: the body's run throws ContextNotFoundError before the button exists;
// SC4001 is an obligation (the unowned run is not proven to be reached).
export function MixedProviderFree() {
  const h1 = () => {
    useContext(NoDefault);
  };
  h1();
  return <button onClick={h1}>go</button>;
}

// SC4001 violation, unchanged: the body's run returns the default, the click throws NoOwnerError.
export function MixedWithDefault() {
  const h2 = () => {
    useContext(WithDefault);
  };
  h2();
  return <button onClick={h2}>go</button>;
}
