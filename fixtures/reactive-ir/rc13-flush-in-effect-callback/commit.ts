import { flush } from "solid-js";

// Called from a createEffect apply in App.ts: the flush below runs on that
// apply's stack, whichever file it is written in.
export function commitFromOtherFile(): void {
  flush();
}
