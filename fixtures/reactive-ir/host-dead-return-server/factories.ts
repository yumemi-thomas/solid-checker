import { createSignal } from "solid-js";
import { isServer } from "@solidjs/web";

// `createLoop`'s guard in a project file. Only a selected package's files are
// folded under the declared host (ADR 0284); a project file is analyzed as
// written, so this factory keeps two return sites.
export function createProjectLoop(callback: () => void) {
  const idle = () => {};
  if (isServer) return [() => false, idle, idle] as const;
  const [running, setRunning] = createSignal(false, { ownedWrite: true });
  const start = () => {
    if (running()) return;
    setRunning(true);
    callback();
  };
  return [running, start, idle] as const;
}
