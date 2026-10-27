// ai-memory-ui badge.tsx:37: the rest of props, an `omit` view whose declared
// keys include every listener of `<span>`, is spread onto the element.
import { omit } from "solid-js";
import type { ComponentProps } from "@solidjs/web";

type BadgeProps = ComponentProps<"span"> & { variant?: "default" | "outline" };

function Badge(props: BadgeProps) {
  const others = omit(props, "class", "variant");
  return <span class={`badge badge-${props.variant ?? "default"} ${props.class ?? ""}`} data-slot="badge" {...others} />;
}

export function Status(props: { label: string }) {
  return <Badge variant="outline">{props.label}</Badge>;
}
