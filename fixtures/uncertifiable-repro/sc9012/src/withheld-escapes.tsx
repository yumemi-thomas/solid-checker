// app-game ui-components-examples/cloud-storage-tree/CloudStorageContext.tsx:310: a provider's local object of
// actions calls its own members from closures and is handed to a context, and one member is a reference.
import { createContext, createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";

interface Actions {
  refresh(): number;
  closeDialog(): number;
}

const ActionsContext = createContext<{ actions: Actions }>();

export function Provider(props: { children: JSX.Element }) {
  const [version] = createSignal(0);
  const refresh = () => version() + 1;
  const actions: Actions = {
    refresh,
    closeDialog() {
      return actions.refresh();
    },
  };
  return <ActionsContext value={{ actions }}>{props.children}</ActionsContext>;
}
