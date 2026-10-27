// app-game browser-atlas App.tsx:793-803: a `.then` callback of a request started in a tracked
// effect compares a signal with the value read before (`if (selectedBackendId() === backendId)`).
import { createSignal, createTrackedEffect } from "solid-js";

export function Backup() {
  const [backend, setBackend] = createSignal("a");
  const [attempt, setAttempt] = createSignal("none");
  createTrackedEffect(() => {
    const id = backend();
    void fetch(`/backups/${id}`).then(() => {
      if (backend() === id) {
        setAttempt("done");
      }
    });
  });
  return <button onClick={() => setBackend("b")}>{attempt()}</button>;
}
