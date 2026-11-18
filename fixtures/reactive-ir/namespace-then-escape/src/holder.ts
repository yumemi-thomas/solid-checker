// ADR 1071: an exported hook registers a cleanup in `onSettled`; its one caller, `createLoader`, is exported but nothing in the program calls it.
import { createSignal, onSettled } from "solid-js";

export const useIsMounted = (): (() => boolean) => {
  const [isMounted, setIsMounted] = createSignal(false);
  onSettled(() => {
    setIsMounted(true);
    return () => setIsMounted(false);
  });
  return isMounted;
};

export function createLoader() {
  return useIsMounted();
}

export function createPlain() {
  return 1;
}
