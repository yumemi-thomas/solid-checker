// app-game cloud-storage-tree/CloudStorageContext.tsx:148-171: methods of a local `actions` object are handed to a context provider;
// a comment in one of them says "this folder", and the walk reads `this` in the text, so it takes the object's methods to use their receiver.
import { createContext, createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";

const FilesContext = createContext<{ actions: { load: (id: string) => void } }>();

export function Files(props: { children?: JSX.Element }) {
  const [loading, setLoading] = createSignal(false);
  const actions = {
    load(id: string) {
      // Load the tree for this folder
      setLoading(id !== "");
    },
  };
  return (
    <FilesContext value={{ actions }}>
      {loading() ? "loading" : props.children}
    </FilesContext>
  );
}
