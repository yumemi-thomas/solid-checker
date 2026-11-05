import { flush } from "solid-js";

export function App() {
  return (
    <div>
      <input onBlur={() => flush()} onChange={() => flush()} />
      <iframe onLoad={() => flush()} />
      <button onClick={() => flush()}>go</button>
    </div>
  );
}
