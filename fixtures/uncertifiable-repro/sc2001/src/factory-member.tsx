// The consumer of factory-store.ts: `store.openPalette()` in a handler.
import { createPaletteStore } from "./factory-store";

export function Toolbar() {
  const store = createPaletteStore();
  return (
    <button onClick={() => store.openPalette()} aria-expanded={store.paletteOpen() ? "true" : "false"}>
      Open
    </button>
  );
}
