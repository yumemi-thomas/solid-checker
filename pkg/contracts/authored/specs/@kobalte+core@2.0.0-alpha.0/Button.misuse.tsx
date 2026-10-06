import { createSignal } from "solid-js";
import { Button } from "@kobalte/core/button";

// The misuse: the signal read in the component body, where the strict-read
// window is open. The handler is the correct twin's.
export default function App() {
  const [count, setCount] = createSignal(0);
  // The same signal read in the body: the window is live on this page.
  const initial = count();
  return (
    <>
      <Button
        id="target"
        onClick={() => {
          setCount(count() + 1);
          document.getElementById("clicked")!.textContent = "clicked";
        }}
      >
        press
      </Button>
      <p id="clicked">idle</p>
      <p>{initial}</p>
    </>
  );
}
