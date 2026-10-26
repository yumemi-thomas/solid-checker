// compass-ui store.ts:1750: a factory returns closures over its signals in an
// object; another module calls them as members of the result.
import { createSignal } from "solid-js";

export function createPaletteStore() {
  const [paletteOpen, setPaletteOpen] = createSignal(false);
  const openPalette = () => {
    if (paletteOpen()) return;
    setPaletteOpen(true);
  };
  return { paletteOpen, openPalette, closePalette: () => setPaletteOpen(false) };
}
