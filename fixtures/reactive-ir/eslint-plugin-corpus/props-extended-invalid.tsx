import {
  Match,
  Switch,
  merge,
  type Component,
} from "solid-js";

export const Assigned: Component<{ name: string }> = props => {
  let name = "";
  name = props.name;
  return <h1>{name}</h1>;
};

export const MergeRead: Component<{ name: string }> = props => {
  const merged = merge({ name: "Anonymous" }, props);
  const name = merged.name;
  return <h1>{name}</h1>;
};

export const NamedControlFlow: Component<{ name: string }> = props => {
  const render = () => {
    const name = props.name;
    return <span>{name}</span>;
  };
  return <Switch><Match when={true}>{render}</Match></Switch>;
};
