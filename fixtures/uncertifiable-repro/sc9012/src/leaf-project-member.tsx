// donegeon-client TeamSettingsRoute.tsx:30 (BoardStorePage.tsx:236): createTrackedEffect calls a member of a
// context object typed by a project type literal, so the member's body is not known.
import { createContext, createSignal, createTrackedEffect, useContext } from "solid-js";

type ToastApi = { error: (message: string) => number };

const ToastContext = createContext<ToastApi>({ error: () => -1 });

export function Settings() {
  const toast = useContext(ToastContext);
  const [failure, setFailure] = createSignal("");
  createTrackedEffect(() => {
    const message = failure().trim();
    if (!message) return;
    toast.error(message);
  });
  return <button onClick={() => setFailure("boom")}>fail</button>;
}
