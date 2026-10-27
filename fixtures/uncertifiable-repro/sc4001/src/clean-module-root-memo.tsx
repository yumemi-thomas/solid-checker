// Clean. Mirrors probus-hk src/stores/clock.ts:67: a memo created at module scope inside its own
// createRoot, so it has an owner and lives as long as the app; components read it in JSX.
import { createMemo, createRoot, createSignal } from "solid-js";

const [now, setNow] = createSignal(0, { ownedWrite: true });

export const minute = createRoot(() => createMemo(() => Math.floor(now() / 60_000)));

export function Clock() {
  return (
    <button type="button" onClick={() => setNow(now() + 60_000)}>
      {minute()}
    </button>
  );
}
