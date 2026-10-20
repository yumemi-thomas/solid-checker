import { createSignal } from "solid-js";

// Module-level state read by the callbacks, as an application's stores are.
export const [request, setRequest] = createSignal<string | undefined>(undefined);
