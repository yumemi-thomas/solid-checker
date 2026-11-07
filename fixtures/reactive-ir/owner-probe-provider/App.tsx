import { createContext, getOwner, useContext } from "solid-js";

const Ctx = createContext<number>();

// The probe proves an owner, and with no provider anywhere the read throws
// ContextNotFoundError there: SC4002, not silence.
export function Guarded() {
  (async () => {
    if (getOwner()) {
      useContext(Ctx);
    }
  })();
  return <div />;
}
