// compass-ui store.ts:1750 (the store is assigned in src/test-router.tsx:47, `store = createAppStore(...)`): the object a factory
// returns is assigned to a `let`, which the forward walk of its members' invocations does not follow.
import { createSignal } from "solid-js";

export function createPaletteStore() {
  const [open, setOpen] = createSignal(false);
  return {
    isOpen: () => open(),
    openPalette: () => {
      setOpen(true);
    },
  };
}

let store: ReturnType<typeof createPaletteStore> | undefined;

export function installStore(): void {
  store = createPaletteStore();
}

export const currentStore = () => store;
