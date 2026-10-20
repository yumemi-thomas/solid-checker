export type Opts = { onClose: () => void; other: () => void };
export function useThing(o: Opts) {
  o.onClose();
}
