import { vi } from "vitest";

// The program installs fake timers, in a helper the test files call. The clock is global to the run, so the question is the program's, not the file's: the flushes that lose
// their reason are the ones in `Scheduler.ts`, the code under test, not any in this file.
export function useClock() {
  vi.useFakeTimers();
}
