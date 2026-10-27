// app-game solid-props-proxy/Props.tsx:12: createTrackedEffect calls `props.ref?.()` on a generic component's
// props, whose member symbol is not a declaration (props is `{ ref } & ComponentProps<T>`).
import type { ComponentProps, JSX, ValidComponent } from "@solidjs/web";
import { createTrackedEffect, omit } from "solid-js";

export function Props<T extends ValidComponent>(
  props: { ref: (props: ComponentProps<T> | null) => void } & ComponentProps<T>,
): JSX.Element {
  const rest = omit(props, "ref");
  createTrackedEffect(() => {
    props.ref?.(rest);
  });
  return null;
}
