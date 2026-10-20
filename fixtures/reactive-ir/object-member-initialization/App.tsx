import { onCleanup } from "solid-js";
export function App() {
  return <button onClick={() => {
    function enter() { timer.useTicker(); }
    try { enter(); } catch {}
    const timer = { useTicker() { onCleanup(() => {}); } };
  }}>start</button>;
}

// A use after initialization in this very handler remains an exact owner edge.
export function Initialized() {
  return <button onClick={() => {
    const stable = { initializedTicker() { onCleanup(() => {}); } };
    stable.initializedTicker();
  }}>start</button>;
}
// Captures remain refused even when this call happens after initialization.
export function Captured() {
  return <button onClick={() => {
    const captured = { capturedTicker() { onCleanup(() => {}); } };
    function later() { captured.capturedTicker(); }
    later();
  }}>start</button>;
}
// Switch clauses share one lexical scope; textual order proves no dominance.
export function SharedCase(value: number) {
  return <button onClick={() => {
    switch (value) {
      case 0:
        const shared = { sharedTicker() { onCleanup(() => {}); } };
        break;
      case 1:
        function enterCase() { shared.sharedTicker(); }
        try { enterCase(); } catch {}
    }
  }}>start</button>;
}
