import { omit } from "solid-js";
import type { PolymorphicProps } from "./polymorphic";

// A project function that happens to be named `Dynamic`: not the dialect's.
function Dynamic(props: PolymorphicProps & { component: string }) {
  props.onClick?.(new MouseEvent("click"));
  return <span>{props.component}</span>;
}

export function ShadowRoot(props: PolymorphicProps) {
  const others = omit(props, "as");
  return <Dynamic {...others} component="button" />;
}
