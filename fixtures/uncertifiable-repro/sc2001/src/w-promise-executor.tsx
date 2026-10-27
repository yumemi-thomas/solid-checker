// app-game browser-atlas App.tsx:290-298: the executor of `new Promise` writes the dialog state
// that carries `resolve`.
import { createSignal } from "solid-js";

type Prompt = { title: string; resolve: (value: string | null) => void };

export function Organizer() {
  const [dialog, setDialog] = createSignal<Prompt | null>(null);
  async function requestName(kind: string): Promise<string | null> {
    return new Promise<string | null>((resolve) => {
      setDialog({ title: `Create ${kind}`, resolve });
    });
  }
  return <button onClick={() => void requestName("group")}>{dialog()?.title ?? "New"}</button>;
}
