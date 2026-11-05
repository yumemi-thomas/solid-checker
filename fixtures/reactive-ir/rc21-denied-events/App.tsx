import { createSignal } from "solid-js";

export function App() {
  const [count, setCount] = createSignal(0);
  return (
    <div>
      <input onBlur={() => { console.log(count()); setCount(1); }} onChange={() => { console.log(count()); setCount(1); }} onCompositionEnd={() => { console.log(count()); setCount(1); }} />
      <p onFocusOut={() => { console.log(count()); setCount(1); }} onFocusIn={() => { console.log(count()); setCount(1); }} tabindex="0">x</p>
      <div popover="auto" onBeforeToggle={() => { console.log(count()); setCount(1); }}>x</div>
      <iframe onLoad={() => { console.log(count()); setCount(1); }} />
      <form onFormData={() => { console.log(count()); setCount(1); }} />
      <input onFocus={() => { console.log(count()); setCount(1); }} />
    </div>
  );
}
