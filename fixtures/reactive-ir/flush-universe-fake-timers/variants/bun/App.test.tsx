import { flush } from "solid-js";
import { jest, test } from "bun:test";

// `bun:test` exports a `jest` namespace (`bun-types/test.d.ts`) whose `useFakeTimers` is a function declaration: the program installs fake timers, and the `setTimeout`
// callback's flush keeps the uncertifiable row of each code (three rows).
test("advances a fake clock", () => {
  jest.useFakeTimers();
  setTimeout(() => {
    flush();
  }, 10);
});
