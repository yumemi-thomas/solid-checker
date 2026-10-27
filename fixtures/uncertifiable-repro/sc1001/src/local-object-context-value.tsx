// app-game cloud-storage-tree/CloudStorageContext.tsx:340 (rename, a method of the local `actions` object put in the provider's value):
// a method of a local object that reads reactive state is only handed on in a context value.
import { createContext, createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";

const TreeContext = createContext<{ actions: { rename: (name: string) => string } }>();

export function Tree(props: { children?: JSX.Element; prefix: string }) {
  const [renamed, setRenamed] = createSignal("");
  const actions = {
    rename(name: string) {
      setRenamed(name);
      return `${props.prefix}${renamed()}`;
    },
  };
  return <TreeContext value={{ actions }}>{props.children}</TreeContext>;
}
