import {
  merge,
  omit,
  type Component,
} from "solid-js";

export const ReactiveHelpers: Component<{
  name?: string;
  class?: string;
}> = source => {
  const props = merge({ name: "Anonymous" }, source);
  const rest = omit(props, "name");
  return <h1 {...rest}>{props.name}</h1>;
};

export function plainHelper(props: { name: string }) {
  const name = props.name;
  return name.toUpperCase();
}
