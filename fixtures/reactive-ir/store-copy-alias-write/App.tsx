import { createStore } from "solid-js";

const plain = [{ id: 1, v: 1 }];

export function App() {
  const [st] = createStore({ items: [{ id: 1, v: 1 }] });
  return (
    <>
      <button onClick={() => { const c = st.items.slice(); c[0].v = 5; }}>copy element</button>
      <button onClick={() => { const c = plain.slice(); c[0].v = 5; }}>plain copy element</button>
      <button onClick={() => { st.items[0].v = 5; }}>direct element</button>
      <button onClick={() => { st.items.slice()[0].v = 5; }}>inline copy element</button>
    </>
  );
}
