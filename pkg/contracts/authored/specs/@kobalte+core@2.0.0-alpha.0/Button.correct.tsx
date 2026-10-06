import { createSignal } from "solid-js";
import { Button } from "@kobalte/core/button";

// The handler reads a signal. Kobalte runs it only when the click is
// dispatched, outside the render's strict-read window, so nothing warns.
export default function App() {
  const [count, setCount] = createSignal(0);
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
    </>
  );
}
