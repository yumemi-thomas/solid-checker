import { flush } from "solid-js";

// `@types/jest` declares `jest.useFakeTimers` as a function of the global `jest` namespace (no import): the property resolves to that declaration, so the program installs fake
// timers and the `setTimeout` callback's flush keeps the uncertifiable row of each code (three rows).
jest.useFakeTimers();

test("advances a fake clock", () => {
  setTimeout(() => {
    flush();
  }, 10);
  jest.advanceTimersByTime(10);
});
