import { vi } from "vitest";

// The same helper with the declarations that do not install a clock: `useRealTimers` restores the host's timers and `advanceTimersByTime` advances a clock somebody else
// installed. Neither is `useFakeTimers` (the declaration's name decides, not the package), so the program installs no fake timers and the three flushes in `Scheduler.ts` end on
// `FlushSafe::FreshStack`, silent, as the parent's end on nothing and keep their rows.
export function useClock() {
  vi.useRealTimers();
  vi.advanceTimersByTime(10);
}
