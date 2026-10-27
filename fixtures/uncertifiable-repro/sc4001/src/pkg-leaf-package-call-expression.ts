// app-game typegpu-examples 1.1-webgpu-fundamentals/index.tsx:81 (`pipeline.withColorAttachment(..).draw(3)`): a tracked
// effect calls a method on the value a package method returns, without binding it first (`vi` is the set's one
// fluent package API).
import { createSignal, createTrackedEffect } from "solid-js";
import { vi } from "vitest";

export function createSpy() {
  const spy = vi.fn();
  const [count] = createSignal(0);
  createTrackedEffect(() => {
    spy.mockReturnValue(count()).mockClear();
  });
  return spy;
}
