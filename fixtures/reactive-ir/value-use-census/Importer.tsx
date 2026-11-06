// The other file of `Held.tsx`: an exported holder's readers are here. `useShown` is the control, read in JSX.
import { createSignal } from "solid-js";
import { aliased, holder, useHeld, useTaken } from "./Held";

function useShown() {
  const [shown] = createSignal(1);
  return { shown };
}

export function Importer() {
  const seen = useShown();
  const got = useHeld();
  try { throw aliased; } catch {}
  void Object.values(holder);
  try { throw got.held; } catch {}
  void [1].map(useTaken);
  return <p>{seen.shown()}</p>;
}
