// Clean. Mirrors kui frontend/packages/feature-messages/src/transport.ts:95-103: createRoot owns an effect
// created outside any component and hands back its dispose, which a later call uses to stop it.
import { createEffect, createRoot, createSignal } from "solid-js";

export function Transport() {
  const [connection, setConnection] = createSignal("open");
  const [log, setLog] = createSignal("");
  const dispose = createRoot((dispose) => {
    createEffect(connection, (current) => {
      console.log("connection", current);
    });
    return dispose;
  });
  return (
    <>
      <button type="button" onClick={() => setConnection(connection() === "open" ? "closed" : "open")}>
        {connection()}
      </button>
      <button type="button" onClick={() => { dispose(); setLog("stopped"); }}>{log()}</button>
    </>
  );
}
