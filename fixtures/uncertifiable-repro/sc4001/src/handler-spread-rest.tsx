// ai-memory-ui badge.tsx:37: the rest of props, an `omit` view whose declared
// keys include every listener of `<span>`, is spread onto the element.
import { omit } from "solid-js";
import type { ComponentProps } from "@solidjs/web";

type BadgeProps = ComponentProps<"span"> & { variant?: "default" | "outline" };

function Badge(props: BadgeProps) {
  const others = omit(props, "class", "variant");
  return <span class={`badge badge-${props.variant ?? "default"} ${props.class ?? ""}`} data-slot="badge" {...others} />;
}

// As in the source (`export { Badge, badgeVariants }`): the component is the
// module's export, so its callers are not only the ones written here (ADR
// 1129 resolves an `omit` view's spread from its callers only when every
// caller is a JSX site of this program).
export { Badge };

export function Status(props: { label: string }) {
  return <Badge variant="outline">{props.label}</Badge>;
}
