import { createSignal } from "solid-js";

export interface HealthOptions {
  readonly now: () => number;
  readonly schedule: (delayMs: number, run: () => void) => void;
}

export function createHealth(options: HealthOptions): () => number {
  const [contact, setContact] = createSignal(options.now());
  const first = contact();
  options.schedule(1000, () => {
    setContact(options.now() + first);
  });
  return contact;
}
