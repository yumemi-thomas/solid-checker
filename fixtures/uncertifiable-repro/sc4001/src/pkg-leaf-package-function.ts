// app-game solid-dnd createDetectLayout.ts:12: a tracked effect resolves a MaybeAccessor option with a package helper.
import { access, type MaybeAccessor } from "@solid-primitives/utils";
import { createSignal, createTrackedEffect } from "solid-js";

export function createWidth(container: MaybeAccessor<HTMLElement | undefined>) {
  const [width, setWidth] = createSignal(0);
  createTrackedEffect(() => {
    const element = access(container);
    if (element) setWidth(element.clientWidth);
  });
  return width;
}
