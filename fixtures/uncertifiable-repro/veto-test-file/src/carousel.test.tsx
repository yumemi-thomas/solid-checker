// skyjtx-website tests/presentations/about.test.tsx:68,82,95,135: a vitest test dispatches an event on an element it
// made inside a `createRoot` callback. The `it` callback is a test runner callback (ADR 0438), but the dispatch sits
// in the `createRoot` callback inside it, which that exclusion does not cover, so the veto holds (ADR 0356).
import { createRoot } from "solid-js";
import { describe, expect, it } from "vitest";
import { Carousel } from "./carousel";

describe("Carousel", () => {
  it("is a component", () => {
    expect(typeof Carousel).toBe("function");
  });

  it("reacts to a pointer event on an element", () => {
    createRoot((dispose) => {
      const el = document.createElement("div");
      el.dispatchEvent(new Event("pointermove", { bubbles: true }));
      expect(el.tagName).toBe("DIV");
      dispose();
    });
  });
});
