import {
  mergeProps,
  splitProps,
  type Component,
} from "solid-js";

export const ReactiveHelpers: Component<{
  name?: string;
  class?: string;
}> = source => {
  const props = mergeProps({ name: "Anonymous" }, source);
  const rest = splitProps(props, ["name"])[1];
  return <h1 {...rest}>{props.name}</h1>;
};

export function plainHelper(props: { name: string }) {
  const name = props.name;
  return name.toUpperCase();
}
