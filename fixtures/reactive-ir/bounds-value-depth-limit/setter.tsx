// ADR 0751: `is_own_setter` follows four `const` aliases of the setter; past them its callback is a window not proven
// either way, so a write in it is uncertifiable (`search-depth-limit`), never a violation (Chrome: quiet).
import { createStore } from "solid-js";

export function FourAliases() {
  const [store, setStore] = createStore({ a: 1 });
  const s1 = setStore; const s2 = s1; const s3 = s2; const s4 = s3;
  const bumpFour = () => { store.a = 2; };
  return <button onClick={() => s4(() => { bumpFour(); })}>{store.a}</button>;
}

export function SixAliases() {
  const [store, setStore] = createStore({ a: 1 });
  const s1 = setStore; const s2 = s1; const s3 = s2; const s4 = s3; const s5 = s4; const s6 = s5;
  const bumpSix = () => { store.a = 2; };
  return <button onClick={() => s6(() => { bumpSix(); })}>{store.a}</button>;
}
