// jandibat-web packages/solid-relay/src/utils/useIsMounted.ts:5 (kui kernel/src/media.ts:37 alike): an
// exported hook registers a cleanup in `onSettled`; its callers are exported functions of a package
// the program loads with a dynamic `import()` (dynamic-import-barrel.ts), so they are not enumerable
// even though the program is closed.
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
