import { createSignal } from "solid-js";

export function App() {
  const [count, setCount] = createSignal(0);
  return (
    <div>
      <button onClick={() => { console.log(count()); setCount(1); }} onPointerDown={() => { console.log(count()); setCount(1); }}>x</button>
      <input onKeyDown={() => { console.log(count()); setCount(1); }} onInput={() => { console.log(count()); setCount(1); }} />
      <form onSubmit={() => { console.log(count()); setCount(1); }} />
    </div>
  );
}
